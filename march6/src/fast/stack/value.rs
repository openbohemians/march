//! Fully evaluated, independently owned values. Sharing is data sharing, not
//! suspended computation. No executor handles, epochs, or demand cells.
use super::{Error, FrozenNode, FrozenValue, Machine};
use crate::Cid;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Unit,
    Text(Arc<str>),
    Quote(Cid),
    Tuple(Tuple),
}

#[derive(Clone)]
pub struct Tuple(Arc<TupleData>);
struct TupleData {
    fields: Vec<Value>,
}

impl Tuple {
    pub fn fields(&self) -> &[Value] {
        &self.0.fields
    }
    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

// The last owner releases an arbitrarily deep acyclic value iteratively.
// Shared children are only decremented; no recursive owned-tree destruction.
impl Drop for TupleData {
    fn drop(&mut self) {
        let mut todo = std::mem::take(&mut self.fields);
        while let Some(value) = todo.pop() {
            if let Value::Tuple(Tuple(node)) = value
                && let Some(mut child) = Arc::into_inner(node)
            {
                todo.append(&mut child.fields);
            }
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn show(
            v: &Value,
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            left: &mut usize,
        ) -> fmt::Result {
            if depth == 32 || *left == 0 {
                return write!(f, "…");
            }
            *left -= 1;
            match v {
                Value::Int(n) => write!(f, "Int({n})"),
                Value::Bool(b) => write!(f, "Bool({b})"),
                Value::Unit => write!(f, "Unit"),
                Value::Text(s) => write!(f, "Text({s:?})"),
                Value::Quote(cid) => write!(f, "Quote({cid:?})"),
                Value::Tuple(t) => {
                    write!(f, "Tuple([")?;
                    for (i, v) in t.fields().iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        show(v, f, depth + 1, left)?;
                        if *left == 0 {
                            break;
                        }
                    }
                    write!(f, "])")
                }
            }
        }
        show(self, f, 0, &mut 256)
    }
}
impl fmt::Debug for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Value::Tuple(self.clone()).fmt(f)
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        equal(self, other, |_| Ok(())).unwrap()
    }
}
impl Eq for Value {}

pub(super) fn equal(
    a: &Value,
    b: &Value,
    mut charge: impl FnMut(usize) -> Result<(), Error>,
) -> Result<bool, Error> {
    let mut todo = vec![(a, b)];
    let mut seen = HashSet::new();
    while let Some((a, b)) = todo.pop() {
        charge(1)?;
        match (a, b) {
            (Value::Int(a), Value::Int(b)) if a == b => (),
            (Value::Bool(a), Value::Bool(b)) if a == b => (),
            (Value::Unit, Value::Unit) => (),
            (Value::Quote(a), Value::Quote(b)) if a == b => (),
            (Value::Text(a), Value::Text(b)) => {
                if !Arc::ptr_eq(a, b) {
                    charge(a.len().min(b.len()))?;
                    if a != b {
                        return Ok(false);
                    }
                }
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                if a.shares_storage(b) {
                    continue;
                }
                if a.fields().len() != b.fields().len() {
                    return Ok(false);
                }
                if seen.insert((a.id(), b.id())) {
                    charge(a.fields().len())?;
                    todo.extend(a.fields().iter().zip(b.fields()).rev());
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

impl Machine<'_> {
    pub(super) fn check_tuple_size(&self, fields: usize) -> Result<(), Error> {
        if fields != 0
            && (self.stats.tuple_nodes >= self.value_node_limit
                || fields
                    > self
                        .tuple_field_limit
                        .saturating_sub(self.stats.tuple_fields))
        {
            return Err(Error::StorageLimit);
        }
        Ok(())
    }
    pub(super) fn make_tuple(&mut self, fields: Vec<Value>) -> Result<Value, Error> {
        // March's existing unit is the zero-field tuple, including its CID.
        if fields.is_empty() {
            return Ok(Value::Unit);
        }
        self.check_tuple_size(fields.len())?;
        self.charge(fields.len())?;
        self.stats.tuple_nodes = self
            .stats
            .tuple_nodes
            .checked_add(1)
            .ok_or(Error::StorageLimit)?;
        self.stats.tuple_fields = self
            .stats
            .tuple_fields
            .checked_add(fields.len())
            .ok_or(Error::StorageLimit)?;
        if self.stats.tuple_nodes > self.value_node_limit
            || self.stats.tuple_fields > self.tuple_field_limit
        {
            return Err(Error::StorageLimit);
        }
        Ok(Value::Tuple(Tuple(Arc::new(TupleData { fields }))))
    }

    pub(super) fn reserve_text(&mut self, bytes: usize) -> Result<(), Error> {
        self.charge(bytes)?;
        let total = self
            .stats
            .text_bytes_allocated
            .checked_add(bytes)
            .ok_or(Error::StorageLimit)?;
        if total > self.text_byte_limit {
            return Err(Error::StorageLimit);
        }
        self.stats.text_bytes_allocated = total;
        self.stats.text_allocations += 1;
        Ok(())
    }

    pub(super) fn freeze_value(&mut self, root: &Value) -> Result<FrozenValue, Error> {
        enum Visit<'a> {
            Value(&'a Value),
            Tuple(&'a Tuple),
        }
        let mut todo = vec![Visit::Value(root)];
        let mut tuples = HashMap::new();
        let mut nodes = Vec::new();
        let mut cids: Vec<Cid> = Vec::new();
        let mut results = Vec::new();
        let mut fields_seen = 0usize;
        while let Some(visit) = todo.pop() {
            self.charge(1)?;
            let (node, bytes, tuple) = match visit {
                Visit::Value(Value::Tuple(t)) => {
                    if let Some(&i) = tuples.get(&t.id()) {
                        results.push(i);
                        continue;
                    }
                    fields_seen = fields_seen
                        .checked_add(t.fields().len())
                        .ok_or(Error::StorageLimit)?;
                    if fields_seen > self.tuple_field_limit {
                        return Err(Error::StorageLimit);
                    }
                    self.charge(t.fields().len())?;
                    todo.push(Visit::Tuple(t));
                    todo.extend(t.fields().iter().rev().map(Visit::Value));
                    continue;
                }
                Visit::Value(Value::Int(n)) => {
                    let mut b = vec![0];
                    b.extend(n.to_le_bytes());
                    (FrozenNode::Int(*n), b, None)
                }
                Visit::Value(Value::Bool(b)) => (FrozenNode::Bool(*b), vec![1, u8::from(*b)], None),
                Visit::Value(Value::Unit) => (FrozenNode::Unit, vec![2], None),
                Visit::Value(Value::Quote(cid)) => {
                    let mut b = vec![3];
                    b.extend(cid.0);
                    (FrozenNode::Quote(*cid), b, None)
                }
                Visit::Value(Value::Text(s)) => {
                    if s.len() > self.text_byte_limit {
                        return Err(Error::StorageLimit);
                    }
                    self.charge(s.len())?;
                    let mut b = vec![6];
                    super::super::text_bytes(&mut b, s);
                    (FrozenNode::Text(s.clone()), b, None)
                }
                Visit::Tuple(t) => {
                    let at = results.len() - t.fields().len();
                    let fields: Vec<usize> = results.drain(at..).collect();
                    let mut b = Vec::new();
                    if fields.len() == 2 {
                        b.push(4);
                    } else {
                        b.push(5);
                        super::super::put(&mut b, fields.len());
                    }
                    for &i in &fields {
                        b.extend(cids[i].0);
                    }
                    (FrozenNode::Tuple(fields.into()), b, Some(t.id()))
                }
            };
            if nodes.len() >= self.value_node_limit {
                return Err(Error::StorageLimit);
            }
            let cid = Cid::digest(b"march-fast-value-v1", &bytes);
            let i = nodes.len();
            nodes.push(node);
            cids.push(cid);
            results.push(i);
            if let Some(id) = tuple {
                tuples.insert(id, i);
            }
        }
        self.stats.frozen_nodes += nodes.len();
        Ok(FrozenValue::from_frozen_nodes(nodes, *cids.last().unwrap()))
    }

    pub(super) fn import_value(&mut self, frozen: &FrozenValue) -> Result<Value, Error> {
        if frozen.nodes().len() > self.value_node_limit {
            return Err(Error::StorageLimit);
        }
        let mut values: Vec<Value> = Vec::with_capacity(frozen.nodes().len());
        for node in frozen.nodes() {
            self.charge(1)?;
            let value = match node {
                FrozenNode::Int(n) => Value::Int(*n),
                FrozenNode::Bool(b) => Value::Bool(*b),
                FrozenNode::Unit => Value::Unit,
                FrozenNode::Quote(cid) => {
                    if !self.program.identities.contains_key(cid) {
                        return Err(Error::Store("stored code is absent from program".into()));
                    }
                    Value::Quote(*cid)
                }
                FrozenNode::Text(s) => {
                    if s.len() > self.text_byte_limit {
                        return Err(Error::StorageLimit);
                    }
                    Value::Text(s.clone())
                }
                FrozenNode::Tuple(fields) => {
                    self.check_tuple_size(fields.len())?;
                    self.charge(fields.len())?;
                    self.make_tuple(fields.iter().map(|&i| values[i].clone()).collect())?
                }
            };
            values.push(value);
        }
        Ok(values.pop().unwrap())
    }

    pub(super) fn tuple_fields(value: &Value) -> Result<&[Value], Error> {
        match value {
            Value::Unit => Ok(&[]),
            Value::Tuple(t) => Ok(t.fields()),
            _ => Err(Error::Type("expected tuple")),
        }
    }
    pub(super) fn index(value: Value) -> Result<usize, Error> {
        match value {
            Value::Int(n) => {
                usize::try_from(n).map_err(|_| Error::Type("negative or oversized index"))
            }
            _ => Err(Error::Type("expected integer index")),
        }
    }
    pub(super) fn text(value: &Value) -> Result<&str, Error> {
        match value {
            Value::Text(t) => Ok(t),
            _ => Err(Error::Type("expected text")),
        }
    }
    pub(super) fn path(&mut self, value: &Value) -> Result<Vec<Arc<str>>, Error> {
        let parts = match value {
            Value::Text(t) => vec![t.clone()],
            Value::Tuple(t) => {
                // Reject huge path shapes before allocating a second field vector.
                if t.fields().len() > 64 {
                    return Err(Error::Store("too many namespace components".into()));
                }
                t.fields()
                    .iter()
                    .map(|v| match v {
                        Value::Text(s) => Ok(s.clone()),
                        _ => Err(Error::Type("store path fields must be text")),
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => return Err(Error::Type("store path must be text or tuple of text")),
        };
        super::super::store::check_path(
            &parts.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
            false,
        )?;
        self.charge(parts.iter().map(|s| s.len()).sum())?;
        Ok(parts)
    }
}
