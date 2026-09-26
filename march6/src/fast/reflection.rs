//! Bounded canonical definition descriptors, not execution-graph reflection.
//! CID text crosses Program snapshots; local WordIds never cross this boundary.
use super::*;
use definition::{Definition, Item};

pub(super) enum Tree {
    Int(i64),
    Bool(bool),
    Unit,
    Text(String),
    Code(Cid),
    Tuple(Vec<Tree>),
}
fn bad(message: &str) -> Error {
    Error::Compiler(format!("definition descriptor: {message}"))
}
fn text(tree: &Tree) -> Result<&str, Error> {
    match tree {
        Tree::Text(s) => Ok(s),
        _ => Err(bad("expected text")),
    }
}
fn fields(tree: &Tree) -> Result<&[Tree], Error> {
    match tree {
        Tree::Unit => Ok(&[]),
        Tree::Tuple(v) => Ok(v),
        _ => Err(bad("expected tuple")),
    }
}
fn record(tree: &Tree) -> Result<(&str, &[Tree]), Error> {
    let (tag, args) = fields(tree)?
        .split_first()
        .ok_or_else(|| bad("empty record"))?;
    Ok((text(tag)?, args))
}
fn count(tree: &Tree) -> Result<usize, Error> {
    match tree {
        Tree::Int(n) if (0..=4096).contains(n) => Ok(*n as usize),
        _ => Err(bad("stack count must be an integer between 0 and 4096")),
    }
}
fn parse_cid(s: &str) -> Result<Cid, Error> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad("CID must be 64 lowercase hexadecimal characters"));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
    }
    Ok(Cid(bytes))
}
pub(super) fn value_cid(value: &Value) -> Result<Cid, Error> {
    match value {
        Value::Text(s) => parse_cid(s),
        Value::Quote(cid) => Ok(*cid),
        _ => Err(bad("expected CID text or quotation")),
    }
}
pub(super) fn resolve(program: &Program, cid: Cid) -> Result<WordId, Error> {
    program
        .identities
        .get(&cid)
        .copied()
        .ok_or_else(|| bad("unknown code CID in supplied state"))
}
fn reference(program: &Program, tree: &Tree) -> Result<WordId, Error> {
    let cid = match tree {
        Tree::Code(cid) => *cid,
        _ => parse_cid(text(tree)?)?,
    };
    resolve(program, cid)
}

pub(super) fn definition(program: &mut Program, tree: &Tree) -> Result<Definition, Error> {
    let (tag, args) = record(tree)?;
    match (tag, args) {
        ("primitive", [name]) => Ok(Definition::Primitive(text(name)?.into())),
        ("kernel", [name]) => Ok(Definition::Kernel(stream::Primitive::from_name(text(
            name,
        )?)?)),
        ("sequence", [items]) => {
            let mut out = Vec::new();
            for item in fields(items)? {
                let (tag, args) = record(item)?;
                out.push(match (tag, args) {
                    ("word", [cid]) => Item::Word(reference(program, cid)?),
                    ("int", [Tree::Int(n)]) => Item::Literal(Literal::Int(*n)),
                    ("bool", [Tree::Bool(b)]) => Item::Literal(Literal::Bool(*b)),
                    ("unit", []) => Item::Literal(Literal::Unit),
                    ("text", [s]) => Item::Literal(Literal::Text(program.intern_text(text(s)?)?)),
                    ("quote", [cid]) => Item::Literal(Literal::Quote(reference(program, cid)?)),
                    ("context", [key]) => Item::Context(text(key)?.into()),
                    ("call", []) => Item::StaticCall,
                    ("apply", [n, m]) => Item::Apply {
                        inputs: count(n)?,
                        outputs: count(m)?,
                    },
                    ("recur", [n, m]) => Item::Recur {
                        inputs: count(n)?,
                        outputs: count(m)?,
                    },
                    ("tuple", [n]) => Item::Tuple(count(n)?),
                    ("untuple", [n]) => Item::Untuple(count(n)?),
                    _ => {
                        return Err(bad(
                            "unknown item tag, wrong arity, or invalid payload type",
                        ));
                    }
                });
            }
            Ok(Definition::Sequence(out))
        }
        ("family", [clauses]) => {
            let mut out = Vec::new();
            for clause in fields(clauses)? {
                let [guard, body] = fields(clause)? else {
                    return Err(bad("family clause needs guard and body CIDs"));
                };
                out.push((reference(program, guard)?, reference(program, body)?));
            }
            Ok(Definition::Family(out))
        }
        _ => Err(bad("unknown definition tag or wrong arity")),
    }
}

struct Builder<'a> {
    fuel: &'a mut usize,
    nodes: usize,
    bytes: usize,
}
impl Builder<'_> {
    fn node(&mut self, bytes: usize) -> Result<(), Error> {
        self.nodes = self.nodes.checked_sub(1).ok_or(Error::StorageLimit)?;
        self.bytes = self.bytes.checked_sub(bytes).ok_or(Error::StorageLimit)?;
        *self.fuel = self
            .fuel
            .checked_sub(bytes.saturating_add(1))
            .ok_or(Error::Budget)?;
        Ok(())
    }
    fn text(&mut self, s: &str) -> Result<Tree, Error> {
        self.node(s.len())?;
        Ok(Tree::Text(s.into()))
    }
    fn integer(&mut self, n: i64) -> Result<Tree, Error> {
        self.node(0)?;
        Ok(Tree::Int(n))
    }
    fn tuple(&mut self, fields: Vec<Tree>) -> Result<Tree, Error> {
        self.node(0)?;
        Ok(Tree::Tuple(fields))
    }
    fn record(&mut self, tag: &str, args: Vec<Tree>) -> Result<Tree, Error> {
        let mut fields = vec![self.text(tag)?];
        fields.extend(args);
        self.tuple(fields)
    }
    fn cid(&mut self, p: &Program, word: WordId) -> Result<Tree, Error> {
        self.text(&p.cid(word)?.to_string())
    }
    fn item(&mut self, p: &Program, item: &Item) -> Result<Tree, Error> {
        let (tag, args) = match item {
            Item::Word(w) => ("word", vec![self.cid(p, *w)?]),
            Item::Literal(Literal::Quote(w)) => ("quote", vec![self.cid(p, *w)?]),
            Item::Literal(Literal::Int(n)) => ("int", vec![self.integer(*n)?]),
            Item::Literal(Literal::Bool(b)) => {
                self.node(0)?;
                ("bool", vec![Tree::Bool(*b)])
            }
            Item::Literal(Literal::Unit) => ("unit", vec![]),
            Item::Literal(Literal::Text(id)) => ("text", vec![self.text(p.text(*id)?)?]),
            Item::Context(key) => ("context", vec![self.text(key)?]),
            Item::StaticCall => ("call", vec![]),
            Item::Apply { inputs, outputs } | Item::Recur { inputs, outputs } => (
                if matches!(item, Item::Apply { .. }) {
                    "apply"
                } else {
                    "recur"
                },
                vec![
                    self.integer(*inputs as i64)?,
                    self.integer(*outputs as i64)?,
                ],
            ),
            Item::Tuple(n) | Item::Untuple(n) => (
                if matches!(item, Item::Tuple(_)) {
                    "tuple"
                } else {
                    "untuple"
                },
                vec![self.integer(*n as i64)?],
            ),
        };
        self.record(tag, args)
    }
}
pub(super) fn describe(
    p: &Program,
    word: WordId,
    fuel: &mut usize,
    nodes: usize,
    bytes: usize,
) -> Result<Tree, Error> {
    let mut b = Builder { fuel, nodes, bytes };
    let definition = p
        .definition(word)?
        .ok_or_else(|| bad("graph-only word is not a language definition"))?;
    let (tag, payload) = match definition {
        Definition::Primitive(name) => ("primitive", b.text(name)?),
        Definition::Kernel(op) => ("kernel", b.text(op.name())?),
        Definition::Sequence(items) => {
            let mut out = Vec::new();
            for item in items {
                out.push(b.item(p, item)?);
            }
            ("sequence", b.tuple(out)?)
        }
        Definition::Family(clauses) => {
            let mut out = Vec::new();
            for (guard, body) in clauses {
                let a = b.cid(p, *guard)?;
                let c = b.cid(p, *body)?;
                out.push(b.tuple(vec![a, c])?);
            }
            ("family", b.tuple(out)?)
        }
    };
    b.record(tag, vec![payload])
}

enum Visit {
    Need(usize, usize),
    Read(usize, usize),
    Fields(usize),
}
pub(super) struct Transfer {
    dst: usize,
    state: stream::StateHandle,
    todo: Vec<Visit>,
    values: Vec<Tree>,
    nodes: usize,
    bytes: usize,
}
impl Transfer {
    pub(super) fn new(dst: usize, root: usize, state: stream::StateHandle) -> Self {
        Self {
            dst,
            state,
            todo: vec![Visit::Need(root, 0)],
            values: vec![],
            nodes: 0,
            bytes: 0,
        }
    }
}
impl Executor<'_> {
    pub(super) fn import_descriptor(&mut self, tree: Tree) -> Result<Datum, Error> {
        // Only our shallow, validated output schema reaches this recursive importer.
        self.data_charge(1)?;
        match tree {
            Tree::Int(n) => Ok(Datum::Int(n)),
            Tree::Bool(b) => Ok(Datum::Bool(b)),
            Tree::Unit => Ok(Datum::Unit),
            Tree::Text(s) => self.store_text(s),
            Tree::Code(_) => Err(bad("unexpected quoted output")),
            Tree::Tuple(fields) => {
                let mut cells = Vec::new();
                for field in fields {
                    let value = self.import_descriptor(field)?;
                    if self.cells.len() >= self.cell_limit {
                        return Err(Error::StorageLimit);
                    }
                    self.next_identity += 1;
                    cells.push(self.cells.len());
                    self.cells.push(Cell {
                        state: 2,
                        value,
                        frame: usize::MAX,
                        slot: 0,
                        identity: self.next_identity,
                    });
                    self.stats.peak_cells = self.stats.peak_cells.max(self.cells.len());
                }
                self.make_tuple(cells)
            }
        }
    }
    pub(super) fn construct_step(&mut self, mut transfer: Box<Transfer>) -> Result<(), Error> {
        match transfer.todo.pop() {
            Some(Visit::Need(cell, depth)) => {
                if depth > 3 {
                    return Err(bad("descriptor nesting exceeds schema depth"));
                }
                transfer.nodes += 1;
                if transfer.nodes > self.cell_limit {
                    return Err(Error::StorageLimit);
                }
                transfer.todo.push(Visit::Read(cell, depth));
                self.work.push(Task::Construct(transfer));
                self.work.push(Task::Need(cell));
                return Ok(());
            }
            Some(Visit::Read(cell, depth)) => {
                let value = self.value(cell);
                if self.is_tuple(value) && value != Datum::Unit {
                    let n = self.tuple_len(value)?;
                    if n > self
                        .cell_limit
                        .saturating_sub(transfer.nodes + transfer.todo.len())
                    {
                        return Err(Error::StorageLimit);
                    }
                    transfer.todo.push(Visit::Fields(n));
                    for i in (0..n).rev() {
                        transfer
                            .todo
                            .push(Visit::Need(self.tuple_field(value, i)?, depth + 1));
                    }
                } else {
                    transfer.values.push(match value {
                        Datum::Int(n) => Tree::Int(n),
                        Datum::Bool(b) => Tree::Bool(b),
                        Datum::Unit => Tree::Unit,
                        Datum::Quote(w) => Tree::Code(self.program.cid(w)?),
                        Datum::Text(t) => {
                            let n = self.text_value(t)?.len();
                            transfer.bytes =
                                transfer.bytes.checked_add(n).ok_or(Error::StorageLimit)?;
                            if transfer.bytes > self.text_byte_limit {
                                return Err(Error::StorageLimit);
                            }
                            self.data_charge(n)?;
                            Tree::Text(self.text_value(t)?.to_owned())
                        }
                        _ => {
                            return Err(bad(
                                "compiler capabilities and call bundles are not descriptor data",
                            ));
                        }
                    });
                }
            }
            Some(Visit::Fields(n)) => {
                let fields = transfer.values.split_off(transfer.values.len() - n);
                transfer.values.push(Tree::Tuple(fields));
            }
            None => {
                let tree = transfer
                    .values
                    .pop()
                    .ok_or_else(|| bad("missing descriptor"))?;
                let kernel = self
                    .kernel
                    .as_mut()
                    .ok_or_else(|| bad("construction outside stream session"))?;
                let state = kernel.construct(transfer.state, &tree)?;
                self.ready(transfer.dst, Datum::CompilerState(state));
                return Ok(());
            }
        }
        self.work.push(Task::Construct(transfer));
        Ok(())
    }
}
