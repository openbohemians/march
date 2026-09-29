//! Conventional execution spike: immutable CAS code, local register identities,
//! shared lazy slots, explicit contexts. No interaction-net dependency.
use crate::Cid;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

mod collect;
pub mod data;
pub mod definition;
mod image;
mod input;
mod reflection;
pub use input::InputNode;
pub mod source;
pub mod stack;
pub mod state;
pub mod store;
pub mod stream;
mod tail;

pub type WordId = usize;
pub type Slot = usize;
pub type Context = BTreeMap<String, Literal>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Literal {
    Int(i64),
    Bool(bool),
    Unit,
    Quote(WordId),
    Text(usize), // Program-local text handle; canonical encoding contains bytes.
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Binary {
    Add,
    Sub,
    Mul,
    Eq,
    Lt,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    StoreRead(Slot),
    StoreWrite {
        value: Slot,
        path: Slot,
    },
    Tuple(Vec<Slot>),
    TupleCheck {
        tuple: Slot,
        count: usize,
    },
    Data {
        primitive: data::Primitive,
        arguments: Vec<Slot>,
    },
    Arg(usize),
    Const(Literal),
    Context(String),
    Binary(Binary, Slot, Slot),
    Select {
        condition: Slot,
        when_true: Slot,
        when_false: Slot,
    },
    Call {
        word: WordId,
        arguments: Vec<Slot>,
    },
    Apply {
        function: Slot,
        arguments: Vec<Slot>,
        outputs: usize,
    },
    Project {
        call: Slot,
        output: usize,
    },
    Recur {
        arguments: Vec<Slot>,
    },
    Pair(Slot, Slot),
    First(Slot),
    Second(Slot),
    /// Pure compiler-state primitive, available only inside a stream session.
    Kernel {
        primitive: stream::Primitive,
        arguments: Vec<Slot>,
    },
    /// Created by add_family: ordered, pure guards; instantiate chosen body only.
    Dispatch {
        clauses: Vec<(WordId, WordId)>,
        arguments: Vec<Slot>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidCode(String),
    UnknownWord(WordId),
    Arity { expected: usize, actual: usize },
    OutputArity { expected: usize, actual: usize },
    Type(&'static str),
    Overflow,
    MissingContext(String),
    NoClause,
    Cycle,
    Budget,
    StorageLimit,
    StaleHandle,
    CollectionBusy,
    Output(usize),
    Image(String),
    Compiler(String),
    Store(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    epoch: u64,
    cell: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Unit,
    Quote(Cid),
    Pair(Handle, Handle),
    Tuple(Vec<Handle>),
    Text(String),
    CompilerState(stream::StateHandle),
}

impl Value {
    /// Identity at an explicit observation boundary, never the execution loop.
    /// A tuple/pair needs its owning Executor's content_id to demand its fields.
    pub fn scalar_cid(&self) -> Option<Cid> {
        let mut bytes = Vec::new();
        match self {
            Self::Int(n) => {
                bytes.push(0);
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            Self::Bool(b) => {
                bytes.push(1);
                bytes.push(u8::from(*b));
            }
            Self::Unit => bytes.push(2),
            Self::Quote(cid) => {
                bytes.push(3);
                bytes.extend_from_slice(&cid.0);
            }
            Self::Text(s) => {
                bytes.push(6);
                text_bytes(&mut bytes, s);
            }
            Self::Pair(..) | Self::Tuple(_) | Self::CompilerState(_) => return None,
        }
        Some(Cid::digest(b"march-fast-value-v1", &bytes))
    }
}

#[derive(Clone, Debug)]
struct Word {
    definition: Option<definition::Definition>,
    inputs: usize,
    ops: Vec<Op>,
    outputs: Vec<Slot>,
    cid: Cid,
    fast: Option<FastPlan>,
    tail: Option<Box<tail::TailPlan>>,
    cycle_tracking: bool,
    effects: state::Effects,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    texts: Vec<std::sync::Arc<str>>,
    text_ids: HashMap<std::sync::Arc<str>, usize>,
    text_bytes: usize,
    words: Vec<Word>,
    identities: HashMap<Cid, WordId>,
    // Persistent HAMTs share untouched paths across compiler-state snapshots.
    // Hash iteration order is never a canonical image or definition order.
    names: imbl::HashMap<String, WordId>,
    immediate: imbl::HashSet<String>,
}

fn put(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n as u64).to_le_bytes());
}
fn text_bytes(out: &mut Vec<u8>, s: &str) {
    put(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.words.len()
    }
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
    fn word(&self, id: WordId) -> Result<&Word, Error> {
        self.words.get(id).ok_or(Error::UnknownWord(id))
    }
    pub fn signature(&self, id: WordId) -> Result<(usize, usize), Error> {
        let w = self.word(id)?;
        Ok((w.inputs, w.outputs.len()))
    }
    pub fn cid(&self, id: WordId) -> Result<Cid, Error> {
        Ok(self.word(id)?.cid)
    }
    pub fn lookup(&self, name: &str) -> Option<WordId> {
        self.names.get(name).copied()
    }
    pub fn bind(&mut self, name: &str, word: WordId) -> Result<(), Error> {
        self.word(word)?;
        self.names.insert(name.into(), word);
        self.immediate.remove(name);
        Ok(())
    }
    pub fn mark_immediate(&mut self, name: &str) -> Result<(), Error> {
        let word = self
            .lookup(name)
            .ok_or_else(|| Error::Compiler(format!("unknown word '{name}'")))?;
        if self.signature(word)? != (1, 1) {
            return Err(Error::Compiler(
                "compiler word must have signature state -> state".into(),
            ));
        }
        self.immediate.insert(name.into());
        Ok(())
    }
    pub fn is_immediate(&self, name: &str) -> bool {
        self.immediate.contains(name)
    }
    pub fn is_fast(&self, word: WordId) -> Result<bool, Error> {
        Ok(self.word(word)?.fast.is_some())
    }
    pub fn is_tail_loop(&self, word: WordId) -> Result<bool, Error> {
        Ok(self.word(word)?.tail.is_some())
    }

    /// Build an in-memory graph fixture for evaluator experiments/tests. This
    /// is not March source identity; fixtures cannot be stored in code images.
    pub fn add_word(
        &mut self,
        inputs: usize,
        ops: Vec<Op>,
        outputs: Vec<Slot>,
    ) -> Result<WordId, Error> {
        self.add_lowered(inputs, ops, outputs, None)
    }

    fn add_lowered(
        &mut self,
        inputs: usize,
        ops: Vec<Op>,
        outputs: Vec<Slot>,
        definition: Option<definition::Definition>,
    ) -> Result<WordId, Error> {
        if inputs > 65535 || ops.len() > 1_000_000 || outputs.len() > 65535 {
            return Err(Error::InvalidCode("word size limit".into()));
        }
        for (i, op) in ops.iter().enumerate() {
            if dependencies(op).iter().any(|&s| s >= i) {
                return Err(Error::InvalidCode(
                    "register dependencies must precede their user".into(),
                ));
            }
            match op {
                Op::Data {
                    primitive,
                    arguments,
                } if primitive.arity() != arguments.len() => {
                    return Err(Error::InvalidCode("data primitive arity".into()));
                }
                Op::Const(Literal::Text(t)) => {
                    self.text(*t)?;
                }
                Op::Kernel {
                    primitive,
                    arguments,
                } if arguments.len() != primitive.arity() => {
                    return Err(Error::InvalidCode("compiler primitive arity".into()));
                }
                Op::Arg(n) if *n >= inputs => {
                    return Err(Error::InvalidCode("argument index".into()));
                }
                Op::Const(Literal::Quote(w)) => {
                    self.word(*w)?;
                }
                Op::Call { word, arguments } => {
                    let expected = self.word(*word)?.inputs;
                    if expected != arguments.len() {
                        return Err(Error::Arity {
                            expected,
                            actual: arguments.len(),
                        });
                    }
                }
                Op::Apply {
                    arguments, outputs, ..
                } if arguments.len() > 65535 || *outputs > 65535 => {
                    return Err(Error::InvalidCode("dynamic call signature size".into()));
                }
                Op::Dispatch { clauses, arguments } => {
                    if clauses.is_empty() {
                        return Err(Error::InvalidCode("empty family".into()));
                    }
                    let count = self.word(clauses[0].1)?.outputs.len();
                    let writes = clauses.iter().try_fold(false, |writes, &(_, body)| {
                        Ok::<_, Error>(writes || self.word(body)?.effects.writes)
                    })?;
                    for &(guard, body) in clauses {
                        if self.word(guard)?.effects.writes
                            || (writes
                                && self
                                    .word(guard)?
                                    .ops
                                    .iter()
                                    .any(|op| matches!(op, Op::Recur { .. })))
                        {
                            return Err(Error::InvalidCode(
                                "state-writing guards are not supported".into(),
                            ));
                        }
                        if self.signature(guard)? != (arguments.len(), 1)
                            || self.signature(body)? != (arguments.len(), count)
                        {
                            return Err(Error::InvalidCode("family clause signature".into()));
                        }
                    }
                }
                _ => (),
            }
        }
        if outputs.iter().any(|&s| s >= ops.len()) {
            return Err(Error::InvalidCode("output index".into()));
        }
        let (ops, remap) = canonical_ops(self, ops);
        let outputs: Vec<_> = outputs.into_iter().map(|s| remap[s]).collect();
        let cid = if let Some(definition) = &definition {
            Cid::digest(definition::DOMAIN, &self.encode_definition(definition)?)
        } else {
            Cid::digest(
                b"march-evaluator-graph-v1",
                &self.encode_word(inputs, &ops, &outputs)?,
            )
        };
        if let Some(&id) = self.identities.get(&cid) {
            return Ok(id);
        }
        let effects = state::effects(self, &ops);
        let fast = if effects.reads || effects.writes {
            None
        } else {
            fast_plan(self, &ops, &outputs)
        };
        let tail = if effects.reads || effects.writes {
            None
        } else {
            tail::compile(self, inputs, &ops, &outputs).map(Box::new)
        };
        // Static call dependencies are acyclic. A dynamic recursive cycle must
        // traverse a recur, dynamic application, or family dispatch boundary.
        let cycle_tracking = ops.iter().any(|op| {
            matches!(
                op,
                Op::Recur { .. } | Op::Apply { .. } | Op::Dispatch { .. }
            )
        });
        let id = self.words.len();
        self.words.push(Word {
            definition,
            inputs,
            ops,
            outputs,
            cid,
            fast,
            tail,
            cycle_tracking,
            effects,
        });
        self.identities.insert(cid, id);
        Ok(id)
    }

    pub fn add_family(
        &mut self,
        inputs: usize,
        outputs: usize,
        clauses: Vec<(WordId, WordId)>,
    ) -> Result<WordId, Error> {
        self.add_source_family(inputs, outputs, clauses, false)
    }
    fn add_source_family(
        &mut self,
        inputs: usize,
        outputs: usize,
        clauses: Vec<(WordId, WordId)>,
        composed: bool,
    ) -> Result<WordId, Error> {
        if inputs > 65535 || outputs > 65535 || (composed && clauses.len() > 100_000) {
            return Err(Error::InvalidCode("family size limit".into()));
        }
        for &(guard, body) in &clauses {
            if self.word(guard)?.effects.writes {
                return Err(Error::InvalidCode(
                    "state-writing or recursive guards are not supported".into(),
                ));
            }
            if composed && (self.definition(guard)?.is_none() || self.definition(body)?.is_none()) {
                return Err(Error::InvalidCode(
                    "family cannot reference graph-only evaluator fixtures".into(),
                ));
            }
            if self.signature(guard)? != (inputs, 1) || self.signature(body)? != (inputs, outputs) {
                return Err(Error::InvalidCode("family clause signature".into()));
            }
        }
        let definition = composed.then(|| definition::Definition::Family(clauses.clone()));
        let mut ops: Vec<_> = (0..inputs).map(Op::Arg).collect();
        let call = ops.len();
        ops.push(Op::Dispatch {
            clauses,
            arguments: (0..inputs).collect(),
        });
        let results: Vec<_> = (0..outputs)
            .map(|output| {
                let s = ops.len();
                ops.push(Op::Project { call, output });
                s
            })
            .collect();
        self.add_lowered(inputs, ops, results, definition)
    }

    fn encode_literal(&self, out: &mut Vec<u8>, value: Literal) -> Result<(), Error> {
        match value {
            Literal::Text(t) => {
                out.push(4);
                text_bytes(out, self.text(t)?);
            }
            Literal::Int(n) => {
                out.push(0);
                out.extend_from_slice(&n.to_le_bytes());
            }
            Literal::Bool(b) => {
                out.push(1);
                out.push(u8::from(b));
            }
            Literal::Unit => out.push(2),
            Literal::Quote(w) => {
                out.push(3);
                out.extend_from_slice(&self.cid(w)?.0);
            }
        }
        Ok(())
    }
    fn encode_word(&self, inputs: usize, ops: &[Op], outputs: &[Slot]) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        put(&mut out, inputs);
        put(&mut out, ops.len());
        for op in ops {
            match op {
                Op::StoreRead(path) => {
                    out.push(17);
                    put(&mut out, *path);
                }
                Op::StoreWrite { value, path } => {
                    out.push(18);
                    put(&mut out, *value);
                    put(&mut out, *path);
                }
                Op::Tuple(fields) => {
                    out.push(14);
                    slots_bytes(&mut out, fields);
                }
                Op::TupleCheck { tuple, count } => {
                    out.push(15);
                    put(&mut out, *tuple);
                    put(&mut out, *count);
                }
                Op::Data {
                    primitive,
                    arguments,
                } => {
                    out.push(16);
                    out.push(*primitive as u8);
                    slots_bytes(&mut out, arguments);
                }
                Op::Kernel {
                    primitive,
                    arguments,
                } => {
                    out.push(13);
                    out.push(*primitive as u8);
                    slots_bytes(&mut out, arguments);
                }
                Op::Arg(n) => {
                    out.push(0);
                    put(&mut out, *n);
                }
                Op::Const(v) => {
                    out.push(1);
                    self.encode_literal(&mut out, *v)?;
                }
                Op::Context(s) => {
                    out.push(2);
                    text_bytes(&mut out, s);
                }
                Op::Binary(b, a, c) => {
                    out.push(3);
                    out.push(*b as u8);
                    put(&mut out, *a);
                    put(&mut out, *c);
                }
                Op::Select {
                    condition,
                    when_true,
                    when_false,
                } => {
                    out.push(4);
                    for s in [condition, when_true, when_false] {
                        put(&mut out, *s);
                    }
                }
                Op::Call { word, arguments } => {
                    out.push(5);
                    out.extend_from_slice(&self.cid(*word)?.0);
                    slots_bytes(&mut out, arguments);
                }
                Op::Apply {
                    function,
                    arguments,
                    outputs,
                } => {
                    out.push(12);
                    put(&mut out, *function);
                    slots_bytes(&mut out, arguments);
                    put(&mut out, *outputs);
                }
                Op::Project { call, output } => {
                    out.push(6);
                    put(&mut out, *call);
                    put(&mut out, *output);
                }
                Op::Recur { arguments } => {
                    out.push(7);
                    slots_bytes(&mut out, arguments);
                }
                Op::Pair(a, b) => {
                    out.push(8);
                    put(&mut out, *a);
                    put(&mut out, *b);
                }
                Op::First(a) => {
                    out.push(9);
                    put(&mut out, *a);
                }
                Op::Second(a) => {
                    out.push(10);
                    put(&mut out, *a);
                }
                Op::Dispatch { clauses, arguments } => {
                    out.push(11);
                    put(&mut out, clauses.len());
                    for (a, b) in clauses {
                        out.extend_from_slice(&self.cid(*a)?.0);
                        out.extend_from_slice(&self.cid(*b)?.0);
                    }
                    slots_bytes(&mut out, arguments);
                }
            }
        }
        slots_bytes(&mut out, outputs);
        Ok(out)
    }
    pub fn context_cid(&self, context: &Context) -> Result<Cid, Error> {
        let mut bytes = Vec::new();
        put(&mut bytes, context.len());
        for (key, value) in context {
            text_bytes(&mut bytes, key);
            self.encode_literal(&mut bytes, *value)?;
        }
        Ok(Cid::digest(b"march-fast-context-v1", &bytes))
    }
}
fn slots_bytes(out: &mut Vec<u8>, slots: &[Slot]) {
    put(out, slots.len());
    for &s in slots {
        put(out, s);
    }
}
fn dependencies(op: &Op) -> Vec<Slot> {
    match op {
        Op::StoreRead(path) => vec![*path],
        Op::StoreWrite { value, path } => vec![*value, *path],
        Op::Tuple(fields) => fields.clone(),
        Op::TupleCheck { tuple, .. } => vec![*tuple],
        Op::Data { arguments, .. } => arguments.clone(),
        Op::Binary(_, a, b) | Op::Pair(a, b) => vec![*a, *b],
        Op::Select {
            condition,
            when_true,
            when_false,
        } => vec![*condition, *when_true, *when_false],
        Op::Call { arguments, .. }
        | Op::Recur { arguments }
        | Op::Dispatch { arguments, .. }
        | Op::Kernel { arguments, .. } => arguments.clone(),
        Op::Project { call, .. } => vec![*call],
        Op::First(a) | Op::Second(a) => vec![*a],
        Op::Apply {
            function,
            arguments,
            ..
        } => std::iter::once(*function)
            .chain(arguments.iter().copied())
            .collect(),
        _ => vec![],
    }
}

// Structural hash-consing is paid once at construction, not on invocation.
// State-dependent applications cannot be merged across intervening writes.
fn canonical_ops(program: &Program, ops: Vec<Op>) -> (Vec<Op>, Vec<Slot>) {
    let mut canonical = Vec::new();
    let mut remap = Vec::with_capacity(ops.len());
    let mut seen = HashMap::new();
    for mut op in ops {
        let rewrite = |s: &mut Slot| *s = remap[*s];
        match &mut op {
            Op::StoreRead(path) => rewrite(path),
            Op::StoreWrite { value, path } => {
                rewrite(value);
                rewrite(path);
            }
            Op::Tuple(fields)
            | Op::Data {
                arguments: fields, ..
            } => {
                for s in fields {
                    rewrite(s);
                }
            }
            Op::TupleCheck { tuple, .. } => rewrite(tuple),
            Op::Binary(_, a, b) | Op::Pair(a, b) => {
                rewrite(a);
                rewrite(b);
            }
            Op::Select {
                condition,
                when_true,
                when_false,
            } => {
                rewrite(condition);
                rewrite(when_true);
                rewrite(when_false);
            }
            Op::Call { arguments, .. }
            | Op::Kernel { arguments, .. }
            | Op::Recur { arguments }
            | Op::Dispatch { arguments, .. } => {
                for s in arguments {
                    rewrite(s);
                }
            }
            Op::Apply {
                function,
                arguments,
                ..
            } => {
                rewrite(function);
                for s in arguments {
                    rewrite(s);
                }
            }
            Op::Project { call, .. } => rewrite(call),
            Op::First(s) | Op::Second(s) => rewrite(s),
            _ => (),
        }
        let state_sensitive = state::sensitive(program, &op);
        let slot = if let Some(&s) = seen.get(&op).filter(|_| !state_sensitive) {
            s
        } else {
            let s = canonical.len();
            if !state_sensitive {
                seen.insert(op.clone(), s);
            }
            canonical.push(op);
            s
        };
        remap.push(slot);
    }
    (canonical, remap)
}

#[derive(Clone, Copy, Debug)]
enum FastOp {
    Arg(Slot, usize),
    Const(Slot, Literal),
    Binary(Slot, Binary, Slot, Slot),
}

#[derive(Clone, Debug)]
struct FastPlan {
    ops: Vec<FastOp>,
    outputs: Vec<Slot>,
}

// Compile demand through closed scalar calls, keeping a distinct virtual frame
// per call site. This is bounded inlining, not eager argument evaluation or
// content-equality memoization. Shared output projections reuse the same frame.
fn fast_plan<'a>(program: &'a Program, ops: &'a [Op], outputs: &[Slot]) -> Option<FastPlan> {
    type Key = (usize, Slot);
    struct Scope<'a> {
        ops: &'a [Op],
        arguments: Option<Vec<Key>>,
        resolved: Vec<Option<Slot>>,
        calls: HashMap<Slot, usize>,
    }
    #[derive(Clone, Copy)]
    enum Job {
        Need(Key),
        Binary(Key, Binary, Key, Key),
        Alias(Key, Key),
    }
    const MAX_INLINE_SLOTS: usize = 16_384;
    if ops.len() > MAX_INLINE_SLOTS {
        return None;
    }
    let mut scopes = vec![Scope {
        ops,
        arguments: None,
        resolved: vec![None; ops.len()],
        calls: HashMap::new(),
    }];
    let mut size = ops.len();
    let mut result = Vec::new();
    let mut todo: Vec<_> = outputs.iter().rev().map(|&s| Job::Need((0, s))).collect();
    while let Some(job) = todo.pop() {
        match job {
            Job::Alias((f, s), (g, t)) => scopes[f].resolved[s] = Some(scopes[g].resolved[t]?),
            Job::Binary((f, s), b, (af, a), (bf, c)) => {
                let dst = result.len();
                result.push(FastOp::Binary(
                    dst,
                    b,
                    scopes[af].resolved[a]?,
                    scopes[bf].resolved[c]?,
                ));
                scopes[f].resolved[s] = Some(dst);
            }
            Job::Need((f, s)) => {
                if scopes[f].resolved[s].is_some() {
                    continue;
                }
                match &scopes[f].ops[s] {
                    Op::Arg(i) => {
                        if let Some(args) = &scopes[f].arguments {
                            let key = *args.get(*i)?;
                            todo.push(Job::Alias((f, s), key));
                            todo.push(Job::Need(key));
                        } else {
                            let dst = result.len();
                            result.push(FastOp::Arg(dst, *i));
                            scopes[f].resolved[s] = Some(dst);
                        }
                    }
                    Op::Const(v) => {
                        let dst = result.len();
                        result.push(FastOp::Const(dst, *v));
                        scopes[f].resolved[s] = Some(dst);
                    }
                    Op::Binary(b, a, c) => {
                        // LIFO: demand left before right, just like step's
                        // Op::Binary arm. tail::compile relies on this order
                        // when proving the first guard's initial demand.
                        todo.push(Job::Binary((f, s), *b, (f, *a), (f, *c)));
                        todo.push(Job::Need((f, *c)));
                        todo.push(Job::Need((f, *a)));
                    }
                    Op::Project { call, output } => {
                        let (call, output) = (*call, *output);
                        let Op::Call { word, arguments } = &scopes[f].ops[call] else {
                            return None;
                        };
                        let w = program.words.get(*word)?;
                        let slot = *w.outputs.get(output)?;
                        let child = if let Some(&child) = scopes[f].calls.get(&call) {
                            child
                        } else {
                            size = size.checked_add(w.ops.len())?;
                            if size > MAX_INLINE_SLOTS || scopes.len() >= 1024 {
                                return None;
                            }
                            let args = arguments.iter().map(|&a| (f, a)).collect();
                            let child = scopes.len();
                            scopes.push(Scope {
                                ops: &w.ops,
                                arguments: Some(args),
                                resolved: vec![None; w.ops.len()],
                                calls: HashMap::new(),
                            });
                            scopes[f].calls.insert(call, child);
                            child
                        };
                        todo.push(Job::Alias((f, s), (child, slot)));
                        todo.push(Job::Need((child, slot)));
                    }
                    _ => return None,
                }
            }
        }
    }
    Some(FastPlan {
        ops: result,
        outputs: outputs
            .iter()
            .map(|&s| scopes[0].resolved[s])
            .collect::<Option<Vec<_>>>()?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Datum {
    Int(i64),
    Bool(bool),
    Unit,
    Quote(WordId),
    Pair(usize, usize),
    Tuple(usize),
    Text(data::TextRef),
    Frame(usize),
    CompilerState(stream::StateHandle),
}
impl From<Literal> for Datum {
    fn from(l: Literal) -> Self {
        match l {
            Literal::Int(n) => Self::Int(n),
            Literal::Bool(b) => Self::Bool(b),
            Literal::Unit => Self::Unit,
            Literal::Quote(w) => Self::Quote(w),
            Literal::Text(t) => Self::Text(data::TextRef::Program(t)),
        }
    }
}
fn binary(op: Binary, a: Datum, b: Datum) -> Result<Datum, Error> {
    match (op, a, b) {
        (Binary::Add, Datum::Int(a), Datum::Int(b)) => {
            a.checked_add(b).map(Datum::Int).ok_or(Error::Overflow)
        }
        (Binary::Sub, Datum::Int(a), Datum::Int(b)) => {
            a.checked_sub(b).map(Datum::Int).ok_or(Error::Overflow)
        }
        (Binary::Mul, Datum::Int(a), Datum::Int(b)) => {
            a.checked_mul(b).map(Datum::Int).ok_or(Error::Overflow)
        }
        (Binary::Lt, Datum::Int(a), Datum::Int(b)) => Ok(Datum::Bool(a < b)),
        (Binary::Eq, Datum::Int(a), Datum::Int(b)) => Ok(Datum::Bool(a == b)),
        (Binary::Eq, Datum::Bool(a), Datum::Bool(b)) => Ok(Datum::Bool(a == b)),
        (Binary::Eq, Datum::Unit, Datum::Unit) => Ok(Datum::Bool(true)),
        (Binary::Eq, Datum::Quote(a), Datum::Quote(b)) => Ok(Datum::Bool(a == b)),
        _ => Err(Error::Type("binary operand types")),
    }
}

#[derive(Clone, Copy)]
struct Cell {
    state: u8,
    value: Datum,
    frame: usize,
    slot: usize,
    // Stable semantic argument identity, independent of a future storage move.
    // Parameter aliases inherit the supplied cell's identity without forcing it.
    identity: u64,
}
struct Frame {
    word: WordId,
    base: usize,
    arguments: Vec<usize>,
    recur: WordId,
    key_hash: u64,
    identities: Vec<u64>,
    state_input: usize,
}
enum Task {
    State(state::Task),
    Construct(Box<reflection::Transfer>),
    Data(usize, usize), // destination, next argument phase
    TupleCheck(usize, usize, usize),
    EqualFinish(usize),
    EqualValues(usize, usize),
    EqualFields(Datum, Datum, usize),
    Need(usize),
    Copy(usize, usize),
    Binary(usize, Binary, usize, usize),
    Choose(usize, usize, usize, usize),
    Project(usize, usize, usize),
    Field(usize, usize, bool),
    Apply(usize, usize),
    Guard(usize, usize, usize), // destination, clause index, guard result cell
    Kernel(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub steps: usize,
    pub primitive_ops: usize,
    pub memo_hits: usize,
    pub calls: usize,
    pub peak_cells: usize,
    pub peak_frames: usize,
    pub peak_arguments: usize,
    pub fast_runs: usize,
    pub tail_iterations: usize,
    pub peak_registers: usize,
    pub collections: usize,
    pub collected_cells: usize,
    pub collected_frames: usize,
    pub collection_steps: usize,
}

/// Current invocation storage, separate from cumulative/high-water counters.
/// Vector bytes count structural capacities, not RSS or live payload bytes.
/// They exclude allocator overhead, hash-table buckets, owned context/error
/// payloads, collector scratch space, and immutable Program code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Storage {
    pub state_versions: usize,
    pub state_links: usize,
    pub tuple_fields: usize,
    pub text_bytes: usize,
    /// Allocated slots, including reserved but unreachable slots of live frames.
    pub cells: usize,
    /// Non-padding slots; not a reachability count before collection.
    pub live_cells: usize,
    pub frames: usize,
    pub arguments: usize,
    pub cell_capacity: usize,
    pub frame_capacity: usize,
    pub task_capacity: usize,
    pub failures: usize,
    pub active_demands: usize,
    /// Structural vector capacity; excludes shared persistent-store allocations.
    pub vector_bytes: usize,
}
static EPOCH: AtomicU64 = AtomicU64::new(1);

/// An invocation-scoped workspace, not a persistent heap. Starting a new run
/// invalidates old handles; dropping/resetting it releases all pending cells.
pub struct Executor<'p> {
    tuples: Vec<Vec<usize>>,
    tuple_fields: usize,
    texts: Vec<std::sync::Arc<str>>,
    text_bytes: usize,
    comparison: bool,
    pub tuple_field_limit: usize,
    pub text_byte_limit: usize,
    program: &'p Program,
    kernel: Option<&'p mut stream::Kernel>,
    cells: Vec<Cell>,
    frames: Vec<Frame>,
    work: Vec<Task>,
    failures: HashMap<usize, Error>,
    context: Context,
    remaining: usize,
    epoch: u64,
    next_identity: u64,
    aborted: Option<Error>,
    stats: Stats,
    registers: Vec<Datum>,
    argument_slots: usize,
    active_demands: HashMap<(u64, Slot), Vec<usize>>,
    state: Option<state::Execution>,
    pub cell_limit: usize,
    pub argument_limit: usize,
}
impl<'p> Executor<'p> {
    pub fn new(program: &'p Program) -> Self {
        Self {
            tuples: Vec::new(),
            tuple_fields: 0,
            texts: Vec::new(),
            text_bytes: 0,
            comparison: false,
            tuple_field_limit: 1_000_000,
            text_byte_limit: 32 * 1024 * 1024,
            program,
            kernel: None,
            cells: Vec::new(),
            frames: Vec::new(),
            work: Vec::new(),
            failures: HashMap::new(),
            context: Context::new(),
            remaining: 0,
            epoch: 0,
            next_identity: 0,
            aborted: None,
            stats: Stats::default(),
            registers: Vec::new(),
            argument_slots: 0,
            active_demands: HashMap::new(),
            state: None,
            cell_limit: 1_000_000,
            argument_limit: 1_000_000,
        }
    }
    pub fn stats(&self) -> &Stats {
        &self.stats
    }
    pub fn storage(&self) -> Storage {
        let (state_versions, state_links, state_bytes) = self
            .state
            .as_ref()
            .map_or((0, 0, 0), state::Execution::storage);
        Storage {
            state_versions,
            state_links,
            tuple_fields: self.tuple_fields,
            text_bytes: self.text_bytes,
            cells: self.cells.len(),
            live_cells: self.cells.iter().filter(|c| c.state != 4).count(),
            frames: self.frames.len(),
            arguments: self.argument_slots,
            cell_capacity: self.cells.capacity(),
            frame_capacity: self.frames.capacity(),
            task_capacity: self.work.capacity(),
            failures: self.failures.len(),
            active_demands: self.active_demands.values().map(Vec::len).sum(),
            vector_bytes: state_bytes
                + self.tuples.capacity() * std::mem::size_of::<Vec<usize>>()
                + self
                    .tuples
                    .iter()
                    .map(|v| v.capacity() * std::mem::size_of::<usize>())
                    .sum::<usize>()
                + self.texts.capacity() * std::mem::size_of::<std::sync::Arc<str>>()
                + self.cells.capacity() * std::mem::size_of::<Cell>()
                + self.frames.capacity() * std::mem::size_of::<Frame>()
                + self.work.capacity() * std::mem::size_of::<Task>()
                + self.registers.capacity() * std::mem::size_of::<Datum>()
                + self
                    .frames
                    .iter()
                    .map(|f| {
                        f.arguments.capacity() * std::mem::size_of::<usize>()
                            + f.identities.capacity() * std::mem::size_of::<u64>()
                    })
                    .sum::<usize>()
                + self
                    .active_demands
                    .values()
                    .map(|v| v.capacity() * std::mem::size_of::<usize>())
                    .sum::<usize>(),
        }
    }
    fn reset(&mut self, budget: usize) {
        self.state = None;
        self.tuples.clear();
        self.tuple_fields = 0;
        self.texts.clear();
        self.text_bytes = 0;
        self.cells.clear();
        self.frames.clear();
        self.work.clear();
        self.failures.clear();
        self.remaining = budget;
        self.aborted = None;
        self.stats = Stats::default();
        self.argument_slots = 0;
        self.active_demands.clear();
        self.next_identity = 0;
        self.epoch = EPOCH.fetch_add(1, Ordering::Relaxed);
    }
    fn validate_args(&self, word: WordId, args: &[Literal]) -> Result<(), Error> {
        let expected = self.program.word(word)?.inputs;
        if args.len() != expected {
            return Err(Error::Arity {
                expected,
                actual: args.len(),
            });
        }
        for a in args {
            if let Literal::Text(t) = a {
                self.program.text(*t)?;
            }
            if let Literal::Quote(w) = a {
                self.program.word(*w)?;
            }
        }
        Ok(())
    }
    fn frame(
        &mut self,
        word: WordId,
        arguments: Vec<usize>,
        recur: WordId,
    ) -> Result<usize, Error> {
        self.frame_with_state(word, arguments, recur, 0)
    }
    fn frame_with_state(
        &mut self,
        word: WordId,
        mut arguments: Vec<usize>,
        recur: WordId,
        state_input: usize,
    ) -> Result<usize, Error> {
        let w = self.program.word(word)?;
        if w.inputs != arguments.len() {
            return Err(Error::Arity {
                expected: w.inputs,
                actual: arguments.len(),
            });
        }
        if self.cells.len().saturating_add(w.ops.len()) > self.cell_limit
            || self.frames.len() >= self.cell_limit
        {
            return Err(Error::StorageLimit);
        }
        if self.argument_slots.saturating_add(arguments.len()) > self.argument_limit {
            return Err(Error::StorageLimit);
        }
        self.argument_slots += arguments.len();
        self.stats.peak_arguments = self.stats.peak_arguments.max(self.argument_slots);
        // Collapse only parameter aliases, without forcing argument values.
        // Equal values reached by different computations are NOT merged here.
        for arg in &mut arguments {
            loop {
                let cell = self.cells[*arg];
                if cell.frame == usize::MAX {
                    break;
                }
                let parent = &self.frames[cell.frame];
                match self.program.word(parent.word)?.ops[cell.slot] {
                    Op::Arg(i) => {
                        *arg = parent.arguments[i];
                        assert_ne!(
                            *arg,
                            usize::MAX,
                            "live parameter refers to a collected argument"
                        );
                    }
                    _ => break,
                }
            }
        }
        let identities: Vec<_> = if w.cycle_tracking {
            arguments.iter().map(|&a| self.cells[a].identity).collect()
        } else {
            Vec::new()
        };
        let key_hash = if w.cycle_tracking {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (word, recur, &identities, state_input).hash(&mut hasher);
            hasher.finish()
        } else {
            0
        };
        let id = self.frames.len();
        let base = self.cells.len();
        let first_identity = self.next_identity;
        self.next_identity = first_identity
            .checked_add(w.ops.len() as u64)
            .ok_or(Error::StorageLimit)?;
        self.cells.reserve(w.ops.len());
        for slot in 0..w.ops.len() {
            let identity = match w.ops[slot] {
                Op::Arg(i) => self.cells[arguments[i]].identity,
                _ => first_identity + slot as u64 + 1,
            };
            self.cells.push(Cell {
                state: 0,
                value: Datum::Unit,
                frame: id,
                slot,
                identity,
            });
        }
        self.frames.push(Frame {
            word,
            base,
            arguments,
            recur,
            key_hash,
            identities,
            state_input,
        });
        self.attach_state_frame(id, state_input)?;
        self.stats.calls += 1;
        self.stats.peak_cells = self.stats.peak_cells.max(self.cells.len());
        self.stats.peak_frames = self.stats.peak_frames.max(self.frames.len());
        Ok(id)
    }
    pub fn start(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
    ) -> Result<Vec<Handle>, Error> {
        self.reset(budget);
        self.validate_args(word, args)?;
        if args.len() > self.cell_limit {
            return Err(Error::StorageLimit);
        }
        self.context.clone_from(context);
        self.next_identity = args.len() as u64;
        self.cells
            .extend(args.iter().enumerate().map(|(i, &v)| Cell {
                state: 2,
                value: v.into(),
                frame: usize::MAX,
                slot: 0,
                identity: i as u64 + 1,
            }));
        let root = self.frame(word, (0..args.len()).collect(), word)?;
        let base = self.frames[root].base;
        if self.program.word(word)?.effects.reads || self.program.word(word)?.effects.writes {
            self.enable_state(store::Store::new())?;
        }
        Ok(self
            .program
            .word(word)?
            .outputs
            .iter()
            .map(|&s| Handle {
                epoch: self.epoch,
                cell: base + s,
            })
            .collect())
    }
    pub fn run(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
    ) -> Result<Vec<Value>, Error> {
        let mut result = Vec::new();
        self.run_into(word, args, context, budget, &mut result)?;
        Ok(result)
    }
    pub fn run_into(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
        result: &mut Vec<Value>,
    ) -> Result<(), Error> {
        result.clear();
        self.reset(budget);
        self.validate_args(word, args)?;
        let program = self.program;
        let w = program.word(word)?;
        if let Some(plan) = &w.fast {
            if plan.ops.len() > self.cell_limit {
                return Err(Error::StorageLimit);
            }
            self.registers.resize(plan.ops.len(), Datum::Unit);
            self.stats.peak_registers = plan.ops.len();
            self.stats.fast_runs = 1;
            self.stats.calls = 1;
            for op in &plan.ops {
                if self.remaining == 0 {
                    result.clear();
                    return Err(Error::Budget);
                }
                self.remaining -= 1;
                self.stats.steps += 1;
                match *op {
                    FastOp::Arg(dst, arg) => self.registers[dst] = args[arg].into(),
                    FastOp::Const(dst, v) => self.registers[dst] = v.into(),
                    FastOp::Binary(dst, b, a, c) => {
                        self.stats.primitive_ops += 1;
                        self.registers[dst] =
                            self.scalar_binary(b, self.registers[a], self.registers[c])?;
                    }
                }
            }
            for &s in &plan.outputs {
                result.push(self.external(self.registers[s])?);
            }
            return Ok(());
        }
        if let Some(plan) = &w.tail {
            let value = self.run_tail(plan, args)?;
            result.push(self.external(value)?);
            return Ok(());
        }
        let handles = self.start(word, args, context, budget)?;
        for h in handles {
            match self.force(h) {
                Ok(v) => result.push(v),
                Err(e) => {
                    result.clear();
                    return Err(e);
                }
            }
        }
        if self.state.is_some()
            && let Err(e) = self.finish_state()
        {
            result.clear();
            return Err(e);
        }
        Ok(())
    }
    fn ready(&mut self, cell: usize, value: Datum) {
        if self.cells[cell].state == 1
            && self.program.words[self.frames[self.cells[cell].frame].word].cycle_tracking
        {
            let c = self.cells[cell];
            let key = (self.frames[c.frame].key_hash, c.slot);
            let bucket = self.active_demands.get_mut(&key).expect("active demand");
            bucket.retain(|&id| id != cell);
            if bucket.is_empty() {
                self.active_demands.remove(&key);
            }
        }
        self.cells[cell].state = 2;
        self.cells[cell].value = value;
    }
    fn value(&self, cell: usize) -> Datum {
        debug_assert_eq!(self.cells[cell].state, 2);
        self.cells[cell].value
    }
    fn external(&self, value: Datum) -> Result<Value, Error> {
        Ok(match value {
            Datum::Text(t) => Value::Text(self.text_value(t)?.to_owned()),
            Datum::Tuple(t) => Value::Tuple(
                self.tuples[t]
                    .iter()
                    .map(|&cell| Handle {
                        epoch: self.epoch,
                        cell,
                    })
                    .collect(),
            ),
            Datum::Int(n) => Value::Int(n),
            Datum::Bool(b) => Value::Bool(b),
            Datum::Unit => Value::Unit,
            Datum::Quote(w) => Value::Quote(self.program.cid(w)?),
            Datum::CompilerState(s) => Value::CompilerState(s),
            Datum::Pair(a, b) => Value::Pair(
                Handle {
                    epoch: self.epoch,
                    cell: a,
                },
                Handle {
                    epoch: self.epoch,
                    cell: b,
                },
            ),
            Datum::Frame(_) => return Err(Error::Type("internal call bundle must be projected")),
        })
    }
    pub fn force(&mut self, handle: Handle) -> Result<Value, Error> {
        if handle.epoch != self.epoch || handle.cell >= self.cells.len() {
            return Err(Error::StaleHandle);
        }
        if let Some(e) = &self.aborted {
            return Err(e.clone());
        }
        self.work.clear();
        self.work.push(Task::Need(handle.cell));
        self.drain_work()?;
        self.external(self.value(handle.cell))
    }
    fn drain_work(&mut self) -> Result<(), Error> {
        if let Some(error) = self.state.as_ref().and_then(|state| state.failure.as_ref()) {
            self.work.clear();
            return Err(error.clone());
        }
        while let Some(task) = self.work.pop() {
            if self.remaining == 0 {
                self.aborted = Some(Error::Budget);
                self.work.clear();
                return Err(Error::Budget);
            }
            self.remaining -= 1;
            self.stats.steps += 1;
            if let Err(e) = self.step(task) {
                if let Some(state) = &mut self.state {
                    state.failure = Some(e.clone());
                }
                if matches!(e, Error::StorageLimit | Error::Budget) {
                    self.aborted = Some(e.clone());
                } else {
                    for (i, c) in self.cells.iter_mut().enumerate() {
                        if c.state == 1 {
                            c.state = 3;
                            self.failures.insert(i, e.clone());
                        }
                    }
                    self.active_demands.clear();
                }
                self.work.clear();
                return Err(e);
            }
        }
        Ok(())
    }
    /// Deep observation gives an immutable value identity independent of local
    /// handles and invocation history. This intentionally demands every field;
    /// an infinite value is bounded by the invocation's remaining fuel/storage.
    /// Code images do not yet persist these runtime values or pending cells.
    pub fn content_id(&mut self, handle: Handle) -> Result<Cid, Error> {
        if handle.epoch != self.epoch || handle.cell >= self.cells.len() {
            return Err(Error::StaleHandle);
        }
        enum Visit {
            Need(Handle),
            Fields(Handle, Vec<Handle>),
        }
        let mut todo = vec![Visit::Need(handle)];
        let mut memo = HashMap::new();
        while let Some(visit) = todo.pop() {
            if let Some(e) = &self.aborted {
                return Err(e.clone());
            }
            if self.remaining == 0 {
                self.aborted = Some(Error::Budget);
                return Err(Error::Budget);
            }
            self.remaining -= 1;
            self.stats.steps += 1;
            match visit {
                Visit::Need(h) => {
                    if memo.contains_key(&h.cell) {
                        continue;
                    }
                    match self.force(h)? {
                        Value::Pair(a, b) => {
                            todo.push(Visit::Fields(h, vec![a, b]));
                            todo.push(Visit::Need(b));
                            todo.push(Visit::Need(a));
                        }
                        Value::Tuple(fields) => {
                            todo.push(Visit::Fields(h, fields.clone()));
                            todo.extend(fields.into_iter().rev().map(Visit::Need));
                        }
                        value => {
                            if let Value::Text(text) = &value
                                && let Err(e) = self.data_charge(text.len())
                            {
                                self.aborted = Some(e.clone());
                                return Err(e);
                            }
                            memo.insert(
                                h.cell,
                                value.scalar_cid().ok_or(Error::Type(
                                    "compiler state has no persistent value CID",
                                ))?,
                            );
                        }
                    }
                }
                Visit::Fields(h, fields) => {
                    let mut bytes = Vec::new();
                    if fields.len() == 2 {
                        bytes.push(4);
                    } else {
                        bytes.push(5);
                        put(&mut bytes, fields.len());
                    }
                    for field in fields {
                        bytes.extend_from_slice(&memo[&field.cell].0);
                    }
                    memo.insert(h.cell, Cid::digest(b"march-fast-value-v1", &bytes));
                }
            }
        }
        Ok(memo[&handle.cell])
    }
    fn copy_later(&mut self, dst: usize, src: usize) {
        self.work.push(Task::Copy(dst, src));
        self.work.push(Task::Need(src));
    }
    fn start_guard(&mut self, dst: usize, clause: usize) -> Result<(), Error> {
        let c = self.cells[dst];
        let parent = &self.frames[c.frame];
        let Op::Dispatch { clauses, arguments } = &self.program.word(parent.word)?.ops[c.slot]
        else {
            unreachable!()
        };
        let Some(&(guard, _)) = clauses.get(clause) else {
            return Err(Error::NoClause);
        };
        let args = arguments.iter().map(|s| parent.base + s).collect();
        let recur = parent.word;
        let input = self.state_before(dst);
        let frame = self.frame_with_state(guard, args, recur, input)?;
        let out = self.frames[frame].base + self.program.word(guard)?.outputs[0];
        self.work.push(Task::Guard(dst, clause, out));
        self.work.push(Task::Need(out));
        Ok(())
    }
    fn step(&mut self, task: Task) -> Result<(), Error> {
        match task {
            Task::State(task) => self.state_step(task)?,
            Task::Construct(transfer) => self.construct_step(transfer)?,
            Task::Need(dst) => {
                let c = self.cells[dst];
                match c.state {
                    2 => {
                        self.stats.memo_hits += 1;
                        return Ok(());
                    }
                    1 => return Err(Error::Cycle),
                    3 => return Err(self.failures[&dst].clone()),
                    4 => return Err(Error::CollectionBusy), // Collected frame padding.
                    _ => (),
                }
                // Recursive demand for a different output can be productive.
                // Detect the same code slot under the same arguments/binder,
                // verifying full keys so hash collisions cannot imply cycles.
                let frame = &self.frames[c.frame];
                let key = (frame.key_hash, c.slot);
                if self.program.words[frame.word].cycle_tracking {
                    if let Some(bucket) = self.active_demands.get(&key) {
                        for &other in bucket {
                            let prior = &self.frames[self.cells[other].frame];
                            if prior.word == frame.word
                                && prior.recur == frame.recur
                                && prior.identities == frame.identities
                                && prior.state_input == frame.state_input
                            {
                                return Err(Error::Cycle);
                            }
                        }
                    }
                    self.active_demands.entry(key).or_default().push(dst);
                }
                self.cells[dst].state = 1;
                let program = self.program;
                let f = &self.frames[c.frame];
                let base = f.base;
                match &program.word(f.word)?.ops[c.slot] {
                    Op::StoreRead(_) | Op::StoreWrite { .. } => self.schedule_store(dst)?,
                    Op::Tuple(fields) => {
                        let fields = fields.iter().map(|s| base + s).collect();
                        let value = self.make_tuple(fields)?;
                        self.ready(dst, value);
                    }
                    Op::TupleCheck { tuple, count } => {
                        self.work.push(Task::TupleCheck(dst, base + tuple, *count));
                        self.work.push(Task::Need(base + tuple));
                    }
                    Op::Data { .. } => self.work.push(Task::Data(dst, 0)),
                    Op::Kernel { arguments, .. } => {
                        self.work.push(Task::Kernel(dst));
                        for slot in arguments.iter().rev() {
                            self.work.push(Task::Need(base + slot));
                        }
                    }
                    Op::Arg(i) => {
                        let src = f.arguments[*i];
                        assert_ne!(
                            src,
                            usize::MAX,
                            "live parameter refers to a collected argument"
                        );
                        self.copy_later(dst, src);
                    }
                    Op::Const(v) => self.ready(dst, (*v).into()),
                    Op::Context(key) => {
                        let v = *self
                            .context
                            .get(key)
                            .ok_or_else(|| Error::MissingContext(key.clone()))?;
                        if let Literal::Quote(w) = v {
                            self.program.word(w)?;
                        }
                        if let Literal::Text(t) = v {
                            self.program.text(t)?;
                        }
                        self.ready(dst, v.into());
                    }
                    Op::Binary(b, a, c) => {
                        let (a, c) = (base + a, base + c);
                        // Keep left-first demand aligned with fast_plan;
                        // the tail-loop strictness proof depends on it.
                        self.work.push(Task::Binary(dst, *b, a, c));
                        self.work.push(Task::Need(c));
                        self.work.push(Task::Need(a));
                    }
                    Op::Select {
                        condition,
                        when_true,
                        when_false,
                    } => {
                        let c = base + condition;
                        self.work
                            .push(Task::Choose(dst, c, base + when_true, base + when_false));
                        self.work.push(Task::Need(c));
                    }
                    Op::Pair(a, b) => self.ready(dst, Datum::Pair(base + a, base + b)),
                    Op::First(a) | Op::Second(a) => {
                        let second = matches!(&program.word(f.word)?.ops[c.slot], Op::Second(_));
                        let src = base + a;
                        self.work.push(Task::Field(dst, src, second));
                        self.work.push(Task::Need(src));
                    }
                    Op::Call { word, arguments } => {
                        let args = arguments.iter().map(|s| base + s).collect();
                        let input = self.state_before(dst);
                        let id = self.frame_with_state(*word, args, *word, input)?;
                        self.ready(dst, Datum::Frame(id));
                    }
                    Op::Apply { function, .. } => {
                        let src = base + function;
                        self.work.push(Task::Apply(dst, src));
                        self.work.push(Task::Need(src));
                    }
                    Op::Recur { arguments } => {
                        let target = f.recur;
                        let args = arguments.iter().map(|s| base + s).collect();
                        let input = self.state_before(dst);
                        let id = self.frame_with_state(target, args, target, input)?;
                        self.ready(dst, Datum::Frame(id));
                    }
                    Op::Project { call, output } => {
                        let src = base + call;
                        self.work.push(Task::Project(dst, src, *output));
                        self.work.push(Task::Need(src));
                    }
                    Op::Dispatch { .. } => self.start_guard(dst, 0)?,
                }
            }
            Task::Copy(dst, src) => self.ready(dst, self.value(src)),
            Task::Kernel(dst) => {
                let cell = self.cells[dst];
                let frame = &self.frames[cell.frame];
                let Op::Kernel {
                    primitive,
                    arguments,
                } = &self.program.words[frame.word].ops[cell.slot]
                else {
                    unreachable!()
                };
                let primitive = *primitive;
                let descriptor = arguments.get(1).map(|s| frame.base + s);
                if primitive == stream::Primitive::Construct {
                    if self.kernel.is_none() {
                        return Err(Error::Compiler(
                            "compiler primitive outside stream session".into(),
                        ));
                    }
                    let Datum::CompilerState(state) = self.value(frame.base + arguments[0]) else {
                        return Err(Error::Type("compiler primitive needs explicit state"));
                    };
                    self.work
                        .push(Task::Construct(Box::new(reflection::Transfer::new(
                            dst,
                            descriptor.unwrap(),
                            state,
                        ))));
                    return Ok(());
                }
                let args = arguments
                    .iter()
                    .map(|s| self.external(self.value(frame.base + s)))
                    .collect::<Result<Vec<_>, _>>()?;
                let kernel = self.kernel.as_mut().ok_or_else(|| {
                    Error::Compiler("compiler primitive outside stream session".into())
                })?;
                if primitive == stream::Primitive::Describe {
                    let tree = kernel.describe(
                        &args,
                        &mut self.remaining,
                        self.cell_limit,
                        self.text_byte_limit,
                    )?;
                    let value = self.import_descriptor(tree)?;
                    self.ready(dst, value);
                    return Ok(());
                }
                let result = kernel.invoke(primitive, &args, &mut self.remaining)?;
                let value = match result {
                    Value::Int(n) => Datum::Int(n),
                    Value::Bool(b) => Datum::Bool(b),
                    Value::Unit => Datum::Unit,
                    Value::CompilerState(s) => Datum::CompilerState(s),
                    Value::Text(text) => self.store_text(text)?,
                    _ => return Err(Error::Type("invalid compiler primitive result")),
                };
                self.ready(dst, value);
            }
            Task::Apply(dst, src) => {
                let Datum::Quote(word) = self.value(src) else {
                    return Err(Error::Type("application needs closed quotation"));
                };
                if self.program.word(word)?.effects.writes {
                    return Err(Error::Type(
                        "apply currently accepts only non-writing quotations",
                    ));
                }
                let c = self.cells[dst];
                let frame = &self.frames[c.frame];
                let Op::Apply {
                    arguments, outputs, ..
                } = &self.program.word(frame.word)?.ops[c.slot]
                else {
                    unreachable!()
                };
                let (input_count, output_count) = self.program.signature(word)?;
                if input_count != arguments.len() {
                    return Err(Error::Arity {
                        expected: input_count,
                        actual: arguments.len(),
                    });
                }
                if output_count != *outputs {
                    return Err(Error::OutputArity {
                        expected: *outputs,
                        actual: output_count,
                    });
                }
                let args = arguments.iter().map(|s| frame.base + s).collect();
                let input = self.state_before(dst);
                let id = self.frame_with_state(word, args, word, input)?;
                self.ready(dst, Datum::Frame(id));
            }
            Task::Binary(dst, b, a, c) => {
                self.stats.primitive_ops += 1;
                if b == Binary::Eq && (self.is_tuple(self.value(a)) || self.is_tuple(self.value(c)))
                {
                    self.comparison = true;
                    self.work.push(Task::EqualFinish(dst));
                    self.work.push(Task::EqualValues(a, c));
                } else {
                    let value = self.scalar_binary(b, self.value(a), self.value(c))?;
                    self.ready(dst, value);
                }
            }
            Task::Choose(dst, c, t, f) => {
                let Datum::Bool(b) = self.value(c) else {
                    return Err(Error::Type("condition must be Boolean"));
                };
                self.copy_later(dst, if b { t } else { f });
            }
            Task::Field(dst, src, second) => {
                let field = self.tuple_field(self.value(src), usize::from(second))?;
                self.copy_later(dst, field);
            }
            Task::Data(dst, phase) => self.data_step(dst, phase)?,
            Task::TupleCheck(dst, src, count) => {
                let value = self.value(src);
                let actual = self.tuple_len(value)?;
                if actual != count {
                    return Err(Error::Arity {
                        expected: count,
                        actual,
                    });
                }
                self.ready(dst, value);
            }
            Task::EqualFinish(dst) => self.ready(dst, Datum::Bool(self.comparison)),
            Task::EqualValues(a, b) => self.equal_values(self.value(a), self.value(b))?,
            Task::EqualFields(a, b, index) => self.equal_fields(a, b, index)?,
            Task::Project(dst, src, output) => {
                let Datum::Frame(frame) = self.value(src) else {
                    return Err(Error::Type("expected call bundle"));
                };
                let f = &self.frames[frame];
                let &slot = self
                    .program
                    .word(f.word)?
                    .outputs
                    .get(output)
                    .ok_or(Error::Output(output))?;
                self.copy_later(dst, f.base + slot);
            }
            Task::Guard(dst, clause, guard) => {
                let Datum::Bool(selected) = self.value(guard) else {
                    return Err(Error::Type("guard must be Boolean"));
                };
                if !selected {
                    self.start_guard(dst, clause + 1)?;
                } else {
                    let c = self.cells[dst];
                    let f = &self.frames[c.frame];
                    let Op::Dispatch { clauses, arguments } =
                        &self.program.word(f.word)?.ops[c.slot]
                    else {
                        unreachable!()
                    };
                    let body = clauses[clause].1;
                    let args = arguments.iter().map(|s| f.base + s).collect();
                    let recur = f.word;
                    let input = self.state_before(dst);
                    let frame = self.frame_with_state(body, args, recur, input)?;
                    self.ready(dst, Datum::Frame(frame));
                }
            }
        }
        Ok(())
    }
}
