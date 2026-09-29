//! Evaluated persistent state. This host boundary does not yet thread state
//! through March words or replace compiler/context state with one store.
use super::*;
use merkle_champ::{ChampMap, Identify};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;

/// Fully evaluated, program-independent nodes. Tuple edges point backwards in
/// a finite DAG; code is identified by CID, never by a local WordId.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrozenNode {
    Int(i64),
    Bool(bool),
    Unit,
    Text(Arc<str>),
    Quote(Cid),
    Tuple(Box<[usize]>),
}

/// A frozen value owns no executor handles or mutable memo cells. Its root is
/// the last node. A flat DAG preserves sharing and avoids recursive destruction
/// even for deeply nested data. Only the evaluator's checked export builds it.
#[derive(Clone, Debug)]
pub struct FrozenValue {
    nodes: Arc<[FrozenNode]>,
    cid: Cid,
}

impl FrozenValue {
    pub(super) fn from_frozen_nodes(nodes: Vec<FrozenNode>, cid: Cid) -> Self {
        Self {
            nodes: nodes.into(),
            cid,
        }
    }
    pub fn cid(&self) -> Cid {
        self.cid
    }

    pub fn nodes(&self) -> &[FrozenNode] {
        &self.nodes
    }

    pub fn root(&self) -> usize {
        self.nodes.len() - 1
    }

    /// Prepare ordinary structured inputs for another invocation. The target
    /// Program must already contain referenced code; this never substitutes a
    /// current name binding for a stored code CID. No code is executed here.
    pub fn to_input_graph(&self, program: &Program) -> Result<Vec<InputNode>, Error> {
        self.nodes
            .iter()
            .map(|node| {
                Ok(match node {
                    FrozenNode::Int(n) => InputNode::Scalar(Literal::Int(*n)),
                    FrozenNode::Bool(b) => InputNode::Scalar(Literal::Bool(*b)),
                    FrozenNode::Unit => InputNode::Scalar(Literal::Unit),
                    FrozenNode::Text(s) => InputNode::Text(s.to_string()),
                    FrozenNode::Quote(cid) => {
                        let word = program.identities.get(cid).copied().ok_or_else(|| {
                            Error::Store(format!("stored code {cid} is absent from target program"))
                        })?;
                        InputNode::Scalar(Literal::Quote(word))
                    }
                    FrozenNode::Tuple(fields) => InputNode::Tuple(fields.to_vec()),
                })
            })
            .collect()
    }
}

// Persistent state compares canonical value identities, not incidental DAG
// sharing or local node numbering. This has the usual CAS collision assumption.
impl PartialEq for FrozenValue {
    fn eq(&self, other: &Self) -> bool {
        self.cid == other.cid
    }
}
impl Eq for FrozenValue {}

impl Identify for FrozenValue {
    fn identify(&self, hasher: &mut Sha256) {
        hasher.update(b"march/frozen-value/v1");
        hasher.update(self.cid.0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Value(FrozenValue),
    Namespace(Store),
}

impl Identify for Entry {
    fn identify(&self, hasher: &mut Sha256) {
        match self {
            Self::Value(value) => {
                hasher.update([0]);
                value.identify(hasher);
            }
            Self::Namespace(store) => {
                hasher.update([1]);
                hasher.update(store.cid().0);
            }
        }
    }
}

/// A nested text-key namespace snapshot. Each path component is an exact UTF-8
/// string: dots are not parsed, and Unicode is not normalized. Namespace nodes
/// and values cannot occupy the same path. Updates return new snapshots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    entries: ChampMap<String, Entry>,
}

fn bad(message: &str) -> Error {
    Error::Store(message.into())
}

pub(super) fn check_path(path: &[&str], allow_root: bool) -> Result<(), Error> {
    if (!allow_root && path.is_empty()) || path.len() > 64 {
        return Err(bad(
            "path requires 1..=64 components (root allowed for namespace lookup)",
        ));
    }
    if path.iter().any(|s| s.is_empty() || s.len() > 4096)
        || path.iter().map(|s| s.len()).sum::<usize>() > 65536
    {
        return Err(bad("empty or oversized namespace path component"));
    }
    Ok(())
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of direct children, not a recursive count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Exact snapshot identity. CHAMP caches unchanged subtree identities;
    /// this never calls the evaluator or demands a suspended computation.
    pub fn cid(&self) -> Cid {
        Cid::digest(b"march-fast-store-v1", &self.entries.identity())
    }

    pub fn namespace(&self, path: &[&str]) -> Result<Option<&Store>, Error> {
        check_path(path, true)?;
        let mut current = self;
        for part in path {
            match current.entries.get(&part.to_string()) {
                Some(Entry::Namespace(next)) => current = next,
                Some(Entry::Value(_)) => return Err(bad("value used as a namespace")),
                None => return Ok(None),
            }
        }
        Ok(Some(current))
    }

    pub fn get(&self, path: &[&str]) -> Result<Option<&FrozenValue>, Error> {
        check_path(path, false)?;
        let Some(parent) = self.namespace(&path[..path.len() - 1])? else {
            return Ok(None);
        };
        match parent.entries.get(&path[path.len() - 1].to_string()) {
            Some(Entry::Value(value)) => Ok(Some(value)),
            Some(Entry::Namespace(_)) => Err(bad("namespace used as a value")),
            None => Ok(None),
        }
    }

    /// Direct child names in deterministic trie order (not lexical order).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(name, _)| name.as_str())
    }

    /// Insert already-frozen data. Missing namespaces are created. A conflicting
    /// namespace/value path is an error, never a destructive implicit conversion.
    pub fn with(&self, path: &[&str], value: FrozenValue) -> Result<Self, Error> {
        check_path(path, false)?;
        self.check_write(path)?;
        Ok(self.insert_at(path, value))
    }

    pub(super) fn check_write(&self, path: &[&str]) -> Result<(), Error> {
        let mut current = self;
        for (i, part) in path.iter().enumerate() {
            match current.entries.get(&part.to_string()) {
                None => return Ok(()),
                Some(Entry::Value(_)) if i + 1 == path.len() => return Ok(()),
                Some(Entry::Namespace(next)) if i + 1 < path.len() => current = next,
                _ => return Err(bad("namespace/value path conflict")),
            }
        }
        Ok(())
    }

    fn insert_at(&self, path: &[&str], value: FrozenValue) -> Self {
        let key = path[0].to_string();
        let entry = if path.len() == 1 {
            Entry::Value(value)
        } else {
            let empty = Self::new();
            let child = match self.entries.get(&key) {
                Some(Entry::Namespace(child)) => child,
                None => &empty,
                _ => unreachable!("path checked before insertion"),
            };
            Entry::Namespace(child.insert_at(&path[1..], value))
        };
        Self {
            entries: self.entries.update(key, entry),
        }
    }

    /// Remove a value and prune now-empty namespace ancestors. Missing paths
    /// are a no-op; namespace deletion is deliberately not implicit.
    pub fn without(&self, path: &[&str]) -> Result<Self, Error> {
        if self.get(path)?.is_none() {
            return Ok(self.clone());
        }
        Ok(self.remove_at(path))
    }

    fn remove_at(&self, path: &[&str]) -> Self {
        let key = path[0].to_string();
        let mut next = self.clone();
        if path.len() == 1 {
            next.entries.remove(&key);
        } else {
            let Some(Entry::Namespace(child)) = self.entries.get(&key) else {
                unreachable!("path checked before removal")
            };
            let child = child.remove_at(&path[1..]);
            if child.is_empty() {
                next.entries.remove(&key);
            } else {
                next.entries.insert(key, Entry::Namespace(child));
            }
        }
        next
    }
}

impl Executor<'_> {
    /// Recursively evaluate finite data into a detached immutable DAG, using
    /// the invocation's remaining fuel and storage bounds. Quotes stop the
    /// traversal: their bodies are not called. Compiler capabilities are rejected.
    pub fn freeze(&mut self, handle: Handle) -> Result<FrozenValue, Error> {
        let result = self.freeze_inner(handle);
        if let Err(e @ (Error::Budget | Error::StorageLimit)) = &result {
            self.aborted = Some(e.clone());
        }
        result
    }

    /// A host state transition, not a March stack primitive. Validate the path,
    /// freeze completely, then publish the new snapshot. Failure leaves `store`
    /// unchanged, though evaluation may have memoized work in this invocation.
    pub fn store_put(
        &mut self,
        store: &Store,
        path: &[&str],
        value: Handle,
    ) -> Result<Store, Error> {
        check_path(path, false)?;
        store.check_write(path)?;
        let value = self.freeze(value)?;
        store.with(path, value)
    }

    fn freeze_inner(&mut self, handle: Handle) -> Result<FrozenValue, Error> {
        if handle.epoch != self.epoch || handle.cell >= self.cells.len() {
            return Err(Error::StaleHandle);
        }
        enum Visit {
            Need(Handle),
            Fields(Handle, Vec<Handle>),
        }
        let mut todo = vec![Visit::Need(handle)];
        let mut memo: HashMap<usize, usize> = HashMap::new();
        let mut visiting = HashSet::new();
        let mut nodes = Vec::new();
        let mut cids: Vec<Cid> = Vec::new();
        let mut fields_used = 0usize;
        let mut bytes_used = 0usize;
        while let Some(visit) = todo.pop() {
            if let Some(e) = &self.aborted {
                return Err(e.clone());
            }
            self.data_charge(1)?;
            let (h, node, cid) = match visit {
                Visit::Need(h) => {
                    if memo.contains_key(&h.cell) {
                        continue;
                    }
                    if !visiting.insert(h.cell) {
                        return Err(Error::Cycle);
                    }
                    if visiting.len() + nodes.len() > self.cell_limit {
                        return Err(Error::StorageLimit);
                    }
                    let value = self.force(h)?;
                    if let Value::Text(text) = &value {
                        bytes_used = bytes_used
                            .checked_add(text.len())
                            .ok_or(Error::StorageLimit)?;
                        if bytes_used > self.text_byte_limit {
                            return Err(Error::StorageLimit);
                        }
                        self.data_charge(text.len())?;
                    }
                    let scalar_cid = value.scalar_cid();
                    let node = match value {
                        Value::Pair(a, b) => FrozenNode::Tuple(vec![a.cell, b.cell].into()),
                        Value::Tuple(fields) => {
                            FrozenNode::Tuple(fields.into_iter().map(|h| h.cell).collect())
                        }
                        Value::Int(n) => FrozenNode::Int(n),
                        Value::Bool(b) => FrozenNode::Bool(b),
                        Value::Unit => FrozenNode::Unit,
                        Value::Text(text) => FrozenNode::Text(text.into()),
                        Value::Quote(cid) => FrozenNode::Quote(cid),
                        Value::CompilerState(_) => {
                            return Err(Error::Type("compiler state cannot be stored"));
                        }
                    };
                    if let FrozenNode::Tuple(fields) = node {
                        fields_used = fields_used
                            .checked_add(fields.len())
                            .ok_or(Error::StorageLimit)?;
                        if fields_used > self.tuple_field_limit {
                            return Err(Error::StorageLimit);
                        }
                        self.data_charge(fields.len())?;
                        let fields: Vec<_> = fields
                            .iter()
                            .map(|&cell| Handle {
                                epoch: self.epoch,
                                cell,
                            })
                            .collect();
                        todo.push(Visit::Fields(h, fields.clone()));
                        todo.extend(fields.into_iter().rev().map(Visit::Need));
                        continue;
                    }
                    (h, node, scalar_cid.expect("non-capability scalar"))
                }
                Visit::Fields(h, fields) => {
                    let mut bytes = Vec::new();
                    if fields.len() == 2 {
                        bytes.push(4);
                    } else {
                        bytes.push(5);
                        put(&mut bytes, fields.len());
                    }
                    let indices: Vec<_> = fields.iter().map(|h| memo[&h.cell]).collect();
                    for &i in &indices {
                        bytes.extend_from_slice(&cids[i].0);
                    }
                    (
                        h,
                        FrozenNode::Tuple(indices.into()),
                        Cid::digest(b"march-fast-value-v1", &bytes),
                    )
                }
            };
            visiting.remove(&h.cell);
            memo.insert(h.cell, nodes.len());
            nodes.push(node);
            cids.push(cid);
        }
        Ok(FrozenValue {
            cid: cids[memo[&handle.cell]],
            nodes: nodes.into(),
        })
    }
}
