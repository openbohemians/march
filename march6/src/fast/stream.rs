//! Stream-fed compiler nucleus. WORD only splits on whitespace; the interpreter
//! and input-consuming defining words are March code in stream-seed.march.
use super::*;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

const SEED: &str = include_str!("stream-seed.march");
static STATE_IDS: AtomicU64 = AtomicU64::new(1);

/// Session-local immutable compiler-state reference. Cannot be fabricated in
/// March, serialized, or confused with an integer or code quotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateHandle(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
// Persistent semantic tags: explicit assignments, never renumber or reuse.
pub enum Primitive {
    Word = 0,
    Eof = 1,
    Number = 2,
    IsNumber = 3,
    NumberValue = 4,
    Find = 5,
    Compiling = 6,
    Immediate = 7,
    EmitNumber = 8,
    CompileCall = 9,
    Execute = 10,
    Begin = 11,
    End = 12,
    MarkImmediate = 13,
    EndSource = 14,
    EmitLiteral = 15,
    SkipLine = 16,
    ReadUntil = 17,
    Quote = 18,
    Context = 19,
    BeginQuote = 20,
    EndQuote = 21,
    StaticCall = 22,
    Count = 23,
    Recur = 24,
    Apply = 25,
    WordByteEquals = 26,
    FamilyBegin = 27,
    FamilyInputs = 28,
    FamilyOutputs = 29,
    FamilyGuard = 30,
    FamilyBody = 31,
    FamilyEnd = 32,
    IsText = 33,
    EmitText = 34,
    Tuple = 35,
    Untuple = 36,
    CidOf = 37,
    Describe = 38,
    Construct = 39,
    LastCid = 40,
    Bind = 41,
}
const PRIMITIVES: &[(Primitive, &str)] = &[
    (Primitive::Word, "stream.word"),
    (Primitive::Eof, "stream.eof?"),
    (Primitive::Number, "stream.number"),
    (Primitive::IsNumber, "stream.number?"),
    (Primitive::NumberValue, "stream.number-value"),
    (Primitive::Find, "stream.find"),
    (Primitive::Compiling, "stream.compiling?"),
    (Primitive::Immediate, "stream.immediate?"),
    (Primitive::EmitNumber, "stream.emit-number"),
    (Primitive::CompileCall, "stream.compile-call"),
    (Primitive::Execute, "stream.execute"),
    (Primitive::Begin, "stream.begin"),
    (Primitive::End, "stream.end"),
    (Primitive::MarkImmediate, "stream.immediate"),
    (Primitive::EndSource, "stream.end-source"),
    (Primitive::EmitLiteral, "stream.emit-literal"),
    (Primitive::SkipLine, "stream.skip-line"),
    (Primitive::ReadUntil, "stream.read-until"),
    (Primitive::Quote, "stream.quote"),
    (Primitive::Context, "stream.context"),
    (Primitive::BeginQuote, "stream.begin-quote"),
    (Primitive::EndQuote, "stream.end-quote"),
    (Primitive::StaticCall, "stream.call"),
    (Primitive::Count, "stream.count"),
    (Primitive::Recur, "stream.recur"),
    (Primitive::Apply, "stream.apply"),
    (Primitive::WordByteEquals, "stream.word-byte="),
    (Primitive::FamilyBegin, "stream.family-begin"),
    (Primitive::FamilyInputs, "stream.family-inputs"),
    (Primitive::FamilyOutputs, "stream.family-outputs"),
    (Primitive::FamilyGuard, "stream.family-guard"),
    (Primitive::FamilyBody, "stream.family-body"),
    (Primitive::FamilyEnd, "stream.family-end"),
    (Primitive::IsText, "stream.text?"),
    (Primitive::EmitText, "stream.emit-text"),
    (Primitive::Tuple, "stream.tuple"),
    (Primitive::Untuple, "stream.untuple"),
    (Primitive::CidOf, "stream.cid-of"),
    (Primitive::Describe, "stream.describe"),
    (Primitive::Construct, "stream.construct"),
    (Primitive::LastCid, "stream.last-cid"),
    (Primitive::Bind, "stream.bind"),
];

#[cfg(test)]
mod primitive_identity_tests {
    use super::*;

    #[test]
    fn compiler_state_remains_noncomparable_across_types_and_inside_tuples() {
        let mut p = seed().unwrap();
        let w = compile(&mut p, "eq?").unwrap();
        let nodes = [
            InputNode::CompilerState(StateHandle(0)),
            InputNode::Scalar(Literal::Int(1)),
            InputNode::Tuple(vec![1]),
            InputNode::Tuple(vec![0]),
            InputNode::Scalar(Literal::Unit),
        ];
        for roots in [[0, 0], [0, 1], [1, 0], [0, 2], [2, 0], [2, 3], [0, 4]] {
            let mut e = Executor::new(&p);
            let h = e
                .start_graph(w, &nodes, &roots, &Context::new(), 1000)
                .unwrap();
            assert!(matches!(e.force(h[0]), Err(Error::Type(_))));
        }
    }

    #[test]
    fn compiler_state_inside_tuple_has_no_content_identity() {
        let mut p = seed().unwrap();
        let w = compile(&mut p, "dup").unwrap();
        let mut e = Executor::new(&p);
        let h = e
            .start_graph(
                w,
                &[
                    InputNode::CompilerState(StateHandle(0)),
                    InputNode::Tuple(vec![0]),
                ],
                &[1],
                &Context::new(),
                1000,
            )
            .unwrap();
        assert!(matches!(e.content_id(h[0]), Err(Error::Type(_))));
        let before = store::Store::new();
        assert!(matches!(
            e.store_put(&before, &["private"], h[0]),
            Err(Error::Type(_))
        ));
        assert!(before.is_empty());
    }

    #[test]
    fn kernel_semantic_tags_and_names_are_pinned() {
        let names = [
            "stream.word",
            "stream.eof?",
            "stream.number",
            "stream.number?",
            "stream.number-value",
            "stream.find",
            "stream.compiling?",
            "stream.immediate?",
            "stream.emit-number",
            "stream.compile-call",
            "stream.execute",
            "stream.begin",
            "stream.end",
            "stream.immediate",
            "stream.end-source",
            "stream.emit-literal",
            "stream.skip-line",
            "stream.read-until",
            "stream.quote",
            "stream.context",
            "stream.begin-quote",
            "stream.end-quote",
            "stream.call",
            "stream.count",
            "stream.recur",
            "stream.apply",
            "stream.word-byte=",
            "stream.family-begin",
            "stream.family-inputs",
            "stream.family-outputs",
            "stream.family-guard",
            "stream.family-body",
            "stream.family-end",
        ];
        let names: Vec<_> = names
            .into_iter()
            .chain([
                "stream.text?",
                "stream.emit-text",
                "stream.tuple",
                "stream.untuple",
                "stream.cid-of",
                "stream.describe",
                "stream.construct",
                "stream.last-cid",
                "stream.bind",
            ])
            .collect();
        assert_eq!(PRIMITIVES.len(), names.len());
        let mut p = Program::new();
        for (tag, name) in names.into_iter().enumerate() {
            let &(primitive, _) = PRIMITIVES.iter().find(|(_, n)| *n == name).unwrap();
            assert_eq!(primitive as u8, tag as u8);
            assert_eq!(Primitive::decode(tag as u8).unwrap(), primitive);
            let word = p
                .add_definition(definition::Definition::Kernel(primitive))
                .unwrap();
            assert_eq!(
                p.cid(word).unwrap(),
                Cid::digest(definition::DOMAIN, &[1, tag as u8])
            );
        }
    }
}
impl Primitive {
    pub fn arity(self) -> usize {
        match self {
            Self::Bind => 3,
            Self::CidOf | Self::Describe | Self::Construct => 2,
            Self::EmitLiteral
            | Self::ReadUntil
            | Self::WordByteEquals
            | Self::FamilyInputs
            | Self::FamilyOutputs => 2,
            _ => 1,
        }
    }
    pub(crate) fn decode(tag: u8) -> Result<Self, Error> {
        PRIMITIVES
            .iter()
            .find(|(p, _)| *p as u8 == tag)
            .map(|(p, _)| *p)
            .ok_or_else(|| Error::Image("unknown compiler primitive".into()))
    }
    pub(super) fn name(self) -> &'static str {
        PRIMITIVES.iter().find(|(op, _)| *op == self).unwrap().1
    }
    pub(super) fn from_name(name: &str) -> Result<Self, Error> {
        PRIMITIVES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(op, _)| *op)
            .ok_or_else(|| fail("unknown kernel primitive"))
    }
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub fuel: usize,
    pub states: usize,
    pub cells: usize,
    pub nesting: usize,
    pub kernel_depth: usize,
    pub source_bytes: usize,
    pub code_words: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 2_000_000,
            states: 16_384,
            cells: 1_000_000,
            nesting: 128,
            kernel_depth: 64,
            source_bytes: 4 * 1024 * 1024,
            code_words: 100_000,
        }
    }
}

#[derive(Clone)]
enum Definition {
    Named(String),
    Quotation,
}
#[derive(Clone)]
struct Frame {
    definition: Definition,
    body: source::Body,
}
#[derive(Clone)]
struct Family {
    name: String,
    inputs: Option<usize>,
    outputs: Option<usize>,
    guard: Option<WordId>,
    clauses: Vec<(WordId, WordId)>,
}
#[derive(Clone)]
struct State {
    program: Rc<Program>,
    input: Rc<str>,
    cursor: usize,
    word: Option<(usize, usize)>,
    number: Option<i64>,
    entry: Option<(WordId, bool)>,
    outer: source::Body,
    frames: Vec<Frame>,
    last: Option<String>,
    last_cid: Option<Cid>,
    count: Option<usize>,
    family: Option<Family>,
}
impl State {
    fn text(&self) -> Result<&str, Error> {
        self.word
            .map(|(a, b)| &self.input[a..b])
            .ok_or_else(|| fail("expected input word, found EOF"))
    }
    fn body(&mut self) -> &mut source::Body {
        self.frames
            .last_mut()
            .map(|f| &mut f.body)
            .unwrap_or(&mut self.outer)
    }
    fn compile_call(&mut self, word: WordId) -> Result<(), Error> {
        let program = self.program.clone();
        self.body().call(&program, word).map_err(|e| fail(e.0))
    }
    fn literal(&mut self, value: Literal) {
        self.body().push_op(
            Op::Const(value),
            match value {
                Literal::Quote(w) => Some(w),
                _ => None,
            },
        );
    }
}
fn fail(message: impl Into<String>) -> Error {
    Error::Compiler(message.into())
}
fn count(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|&n| n <= 4096)
        .ok_or_else(|| fail("stack count must be between 0 and 4096"))
}

/// Compiler states form an immutable, bounded session arena. Old snapshots
/// stay valid for the session; no mutation is exposed through a StateHandle.
pub(super) struct Kernel {
    states: HashMap<u64, State>,
    limits: Limits,
    depth: usize,
}
impl Kernel {
    pub(super) fn describe(
        &self,
        args: &[Value],
        fuel: &mut usize,
        cells: usize,
        bytes: usize,
    ) -> Result<reflection::Tree, Error> {
        let Some(Value::CompilerState(state)) = args.first() else {
            return Err(Error::Type("compiler primitive needs explicit state"));
        };
        let p = &self.state(*state)?.program;
        let cid = reflection::value_cid(&args[1])?;
        let word = reflection::resolve(p, cid)?;
        reflection::describe(p, word, fuel, cells, bytes)
    }
    pub(super) fn construct(
        &mut self,
        handle: StateHandle,
        tree: &reflection::Tree,
    ) -> Result<StateHandle, Error> {
        let mut state = self.state(handle)?.clone();
        let p = Rc::make_mut(&mut state.program);
        let definition = reflection::definition(p, tree)?;
        let word = p.add_definition(definition)?;
        state.last_cid = Some(p.cid(word)?);
        self.insert(state)
    }
    fn insert(&mut self, state: State) -> Result<StateHandle, Error> {
        if self.states.len() >= self.limits.states || state.program.len() > self.limits.code_words {
            return Err(Error::StorageLimit);
        }
        let id = STATE_IDS.fetch_add(1, Ordering::Relaxed);
        self.states.insert(id, state);
        Ok(StateHandle(id))
    }
    fn state(&self, handle: StateHandle) -> Result<&State, Error> {
        self.states
            .get(&handle.0)
            .ok_or_else(|| fail("foreign compiler state"))
    }
    fn execute(
        &mut self,
        word: WordId,
        state: StateHandle,
        fuel: &mut usize,
    ) -> Result<StateHandle, Error> {
        if self.depth >= self.limits.kernel_depth {
            return Err(Error::StorageLimit);
        }
        let program = self.state(state)?.program.clone();
        if program.signature(word)? != (1, 1) {
            return Err(fail("compiler word needs state -> state signature"));
        }
        let effects = program.effects(word)?;
        if effects.reads || effects.writes {
            return Err(fail(
                "runtime store operations are unavailable during compiler execution",
            ));
        }
        let context = Context::from([
            ("compiler".into(), Literal::Bool(true)),
            (
                "compiling".into(),
                Literal::Bool(!self.state(state)?.frames.is_empty()),
            ),
        ]);
        let cells = self.limits.cells;
        self.depth += 1;
        let result = {
            let mut e = Executor::new(&program);
            e.cell_limit = cells;
            e.argument_limit = cells;
            e.kernel = Some(self);
            let result = e
                .start_graph(
                    word,
                    &[InputNode::CompilerState(state)],
                    &[0],
                    &context,
                    *fuel,
                )
                .and_then(|h| e.force(h[0]));
            *fuel = e.remaining;
            result
        };
        self.depth -= 1;
        match result? {
            Value::CompilerState(s) => {
                self.state(s)?;
                Ok(s)
            }
            _ => Err(fail("compiler word did not return a compiler state")),
        }
    }
    pub(super) fn invoke(
        &mut self,
        op: Primitive,
        args: &[Value],
        fuel: &mut usize,
    ) -> Result<Value, Error> {
        let Some(Value::CompilerState(handle)) = args.first() else {
            return Err(Error::Type("compiler primitive needs explicit state"));
        };
        let handle = *handle;
        let old = self.state(handle)?;
        let integer = || match args.get(1) {
            Some(Value::Int(n)) => Ok(*n),
            _ => Err(Error::Type("compiler primitive needs integer")),
        };
        // Predicates return ordinary March values. Control flow is in the seed,
        // not in a native read/classify/dispatch loop.
        match op {
            Primitive::CidOf => {
                let Some(Value::Text(name)) = args.get(1) else {
                    return Err(Error::Type("word name must be text"));
                };
                let word = old
                    .program
                    .lookup(name)
                    .ok_or_else(|| fail(format!("unknown word '{name}'")))?;
                return Ok(Value::Text(old.program.cid(word)?.to_string()));
            }
            Primitive::LastCid => {
                return old
                    .last_cid
                    .map(|cid| Value::Text(cid.to_string()))
                    .ok_or_else(|| fail("no constructed definition"));
            }
            Primitive::IsText => return Ok(Value::Bool(old.text()?.starts_with('"'))),
            Primitive::Eof => return Ok(Value::Bool(old.word.is_none())),
            Primitive::IsNumber => return Ok(Value::Bool(old.number.is_some())),
            Primitive::NumberValue => {
                return old
                    .number
                    .map(Value::Int)
                    .ok_or_else(|| fail("input word is not a number"));
            }
            Primitive::Compiling => return Ok(Value::Bool(!old.frames.is_empty())),
            Primitive::Immediate => {
                return Ok(Value::Bool(
                    old.entry.ok_or_else(|| fail("no dictionary entry"))?.1,
                ));
            }
            Primitive::WordByteEquals => {
                return Ok(Value::Bool(
                    old.text()?.as_bytes()
                        == [u8::try_from(integer()?).map_err(|_| fail("invalid byte"))?],
                ));
            }
            _ => (),
        }
        let mut state = old.clone();
        match op {
            Primitive::Bind => {
                let Some(Value::Text(name)) = args.get(1) else {
                    return Err(Error::Type("word name must be text"));
                };
                let cid = reflection::value_cid(&args[2])?;
                let word = reflection::resolve(&state.program, cid)?;
                Rc::make_mut(&mut state.program).bind(name, word)?;
                state.last = Some(name.clone());
            }
            Primitive::EmitText => {
                let start = state.word.ok_or_else(|| fail("expected text"))?.0;
                let (text, end) = data::read_text(&state.input, start)?;
                if state.input[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| !c.is_whitespace())
                {
                    return Err(fail("text literal needs a whitespace separator"));
                }
                let id = Rc::make_mut(&mut state.program).intern_text(&text)?;
                state.cursor = end;
                state.literal(Literal::Text(id));
            }
            Primitive::Tuple | Primitive::Untuple => {
                let n = count(state.number.ok_or_else(|| fail("expected tuple arity"))?)?;
                state.body().tuple(n, op == Primitive::Untuple);
            }
            Primitive::Word => {
                let mut start = state.cursor;
                for ch in state.input[start..].chars() {
                    if !ch.is_whitespace() {
                        break;
                    }
                    start += ch.len_utf8();
                }
                let mut end = start;
                for ch in state.input[start..].chars() {
                    if ch.is_whitespace() {
                        break;
                    }
                    end += ch.len_utf8();
                }
                state.cursor = end;
                state.word = (start != end).then_some((start, end));
                state.number = None;
                state.entry = None;
            }
            Primitive::Number => {
                let text = state.text()?;
                let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
                state.number = if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                    Some(text.parse().map_err(|_| Error::Overflow)?)
                } else {
                    None
                };
            }
            Primitive::Find => {
                let name = state.text()?;
                let word = state
                    .program
                    .lookup(name)
                    .ok_or_else(|| fail(format!("unknown word '{name}'")))?;
                state.entry = Some((word, state.program.is_immediate(name)));
            }
            Primitive::EmitNumber => state.literal(Literal::Int(
                state.number.ok_or_else(|| fail("no parsed number"))?,
            )),
            Primitive::CompileCall => {
                state.compile_call(state.entry.ok_or_else(|| fail("no dictionary entry"))?.0)?
            }
            Primitive::Execute => {
                let (word, immediate) = state.entry.ok_or_else(|| fail("no dictionary entry"))?;
                if immediate {
                    return self.execute(word, handle, fuel).map(Value::CompilerState);
                }
                // Lazy interpretation composes pending work on the outer stack;
                // observing the resulting entry demands it in the normal VM.
                state.compile_call(word)?;
            }
            Primitive::Begin => {
                if !state.frames.is_empty() || state.family.is_some() {
                    return Err(fail("nested named definition"));
                }
                let name = state.text()?.to_owned();
                state.frames.push(Frame {
                    definition: Definition::Named(name),
                    body: source::Body::composed(),
                });
            }
            Primitive::End | Primitive::EndQuote => {
                let frame = state
                    .frames
                    .pop()
                    .ok_or_else(|| fail("no open definition"))?;
                if matches!(
                    (&frame.definition, op),
                    (Definition::Named(_), Primitive::EndQuote)
                        | (Definition::Quotation, Primitive::End)
                ) {
                    return Err(fail("mismatched definition terminator"));
                }
                let word = frame
                    .body
                    .finish(Rc::make_mut(&mut state.program))
                    .map_err(|e| fail(e.0))?;
                match frame.definition {
                    Definition::Named(name) => {
                        Rc::make_mut(&mut state.program).bind(&name, word)?;
                        state.last = Some(name);
                    }
                    Definition::Quotation => state.literal(Literal::Quote(word)),
                }
            }
            Primitive::MarkImmediate => {
                let name = state
                    .last
                    .as_ref()
                    .ok_or_else(|| fail("no completed definition"))?;
                Rc::make_mut(&mut state.program).mark_immediate(name)?;
            }
            Primitive::EndSource => {
                if !state.frames.is_empty() || state.family.is_some() {
                    return Err(fail("unfinished definition at EOF"));
                }
            }
            Primitive::EmitLiteral => {
                let literal = match args.get(1) {
                    Some(Value::Text(text)) => {
                        Literal::Text(Rc::make_mut(&mut state.program).intern_text(text)?)
                    }
                    Some(Value::Int(n)) => Literal::Int(*n),
                    Some(Value::Bool(b)) => Literal::Bool(*b),
                    Some(Value::Unit) => Literal::Unit,
                    Some(Value::Quote(cid)) => Literal::Quote(
                        *state
                            .program
                            .identities
                            .get(cid)
                            .ok_or_else(|| fail("unknown quotation"))?,
                    ),
                    _ => return Err(Error::Type("compiler literal must be scalar or quotation")),
                };
                state.literal(literal);
            }
            Primitive::SkipLine | Primitive::ReadUntil => {
                let byte = if op == Primitive::SkipLine {
                    b'\n'
                } else {
                    u8::try_from(integer()?)
                        .ok()
                        .filter(|b| b.is_ascii())
                        .ok_or_else(|| fail("delimiter must be ASCII"))?
                };
                match state.input.as_bytes()[state.cursor..]
                    .iter()
                    .position(|&b| b == byte)
                {
                    Some(n) => state.cursor += n + 1,
                    None if op == Primitive::SkipLine => state.cursor = state.input.len(),
                    None => return Err(fail("input delimiter not found")),
                }
            }
            Primitive::Quote => {
                let word = state
                    .program
                    .lookup(state.text()?)
                    .ok_or_else(|| fail("unknown quoted word"))?;
                state.literal(Literal::Quote(word));
            }
            Primitive::Context => {
                let key = state.text()?.to_owned();
                state.body().push_op(Op::Context(key), None);
            }
            Primitive::BeginQuote => {
                if state.frames.len() >= self.limits.nesting {
                    return Err(Error::StorageLimit);
                }
                state.frames.push(Frame {
                    definition: Definition::Quotation,
                    body: source::Body::composed(),
                });
            }
            Primitive::StaticCall => {
                let program = state.program.clone();
                state.body().static_call(&program).map_err(|e| fail(e.0))?;
            }
            Primitive::Count => {
                state.count = Some(count(
                    state.number.ok_or_else(|| fail("expected stack count"))?,
                )?);
            }
            Primitive::Recur | Primitive::Apply => {
                let inputs = state
                    .count
                    .take()
                    .ok_or_else(|| fail("missing input count"))?;
                let outputs = count(state.number.ok_or_else(|| fail("expected output count"))?)?;
                state
                    .body()
                    .dynamic(inputs, outputs, op == Primitive::Recur);
            }
            Primitive::FamilyBegin => {
                if !state.frames.is_empty() || state.family.is_some() {
                    return Err(fail("family definition must be top level"));
                }
                state.family = Some(Family {
                    name: state.text()?.into(),
                    inputs: None,
                    outputs: None,
                    guard: None,
                    clauses: Vec::new(),
                });
            }
            Primitive::FamilyInputs | Primitive::FamilyOutputs => {
                let value = count(integer()?)?;
                let family = state
                    .family
                    .as_mut()
                    .ok_or_else(|| fail("no open family"))?;
                if op == Primitive::FamilyInputs {
                    family.inputs = Some(value);
                } else {
                    family.outputs = Some(value);
                }
            }
            Primitive::FamilyGuard | Primitive::FamilyBody => {
                let word = state.entry.ok_or_else(|| fail("no dictionary entry"))?.0;
                let family = state
                    .family
                    .as_mut()
                    .ok_or_else(|| fail("no open family"))?;
                if op == Primitive::FamilyGuard {
                    family.guard = Some(word);
                } else {
                    family.clauses.push((
                        family.guard.take().ok_or_else(|| fail("missing guard"))?,
                        word,
                    ));
                }
            }
            Primitive::FamilyEnd => {
                let family = state.family.take().ok_or_else(|| fail("no open family"))?;
                let program = Rc::make_mut(&mut state.program);
                let word = program.add_source_family(
                    family.inputs.ok_or_else(|| fail("missing family inputs"))?,
                    family
                        .outputs
                        .ok_or_else(|| fail("missing family outputs"))?,
                    family.clauses,
                    true,
                )?;
                program.bind(&family.name, word)?;
                state.last = Some(family.name);
            }
            _ => unreachable!(),
        }
        self.insert(state).map(Value::CompilerState)
    }
}

/// Create the initial dictionary. Only this one-time seed build uses the older
/// host reader; subsequent source goes through the persisted March interpreter.
pub fn seed() -> Result<Program, Error> {
    let mut program = Program::new();
    for &(primitive, name) in PRIMITIVES {
        let word = program.add_definition(super::definition::Definition::Kernel(primitive))?;
        program.bind(name, word)?;
    }
    // Runtime primitives are ordinary dictionary entries, not reader cases.
    super::definition::install_runtime(&mut program)?;
    source::compile_composed(&mut program, SEED).map_err(|e| fail(e.0))?;
    for (name, implementation) in [
        (":", "seed.colon"),
        (";", "stream.end"),
        ("immediate", "stream.immediate"),
        ("--", "stream.skip-line"),
        ("\\", "stream.skip-line"),
        ("(", "seed.comment"),
        ("'", "seed.quote"),
        ("quote", "seed.quote"),
        ("ctx", "seed.context"),
        ("[", "stream.begin-quote"),
        ("]", "stream.end-quote"),
        ("call", "stream.call"),
        ("recur", "seed.recur"),
        ("apply", "seed.apply"),
        ("family", "seed.family"),
        ("tuple", "seed.tuple"),
        ("untuple", "seed.untuple"),
    ] {
        let word = program
            .lookup(implementation)
            .ok_or_else(|| fail("missing seed word"))?;
        program.bind(name, word)?;
        program.mark_immediate(name)?;
    }
    Ok(program)
}

/// Interpret source into canonical pending code. Numeric recognition precedes
/// lookup in the March seed, even if the dictionary contains the same spelling.
/// Compilation is atomic: failures do not change the supplied Program.
pub fn compile(program: &mut Program, input: &str) -> Result<WordId, Error> {
    compile_with_limits(program, input, &Limits::default())
}
pub fn compile_with_limits(
    program: &mut Program,
    input: &str,
    limits: &Limits,
) -> Result<WordId, Error> {
    if input.len() > limits.source_bytes {
        return Err(fail("source exceeds configured byte limit (default 4 MiB)"));
    }
    let interpreter = program
        .lookup("stream.interpret")
        .ok_or_else(|| fail("image has no stream interpreter; load a stream seed"))?;
    let state = State {
        program: Rc::new(program.clone()),
        input: Rc::from(input),
        cursor: 0,
        word: None,
        number: None,
        entry: None,
        outer: source::Body::composed(),
        frames: Vec::new(),
        last: None,
        last_cid: None,
        count: None,
        family: None,
    };
    let mut kernel = Kernel {
        states: HashMap::new(),
        limits: limits.clone(),
        depth: 0,
    };
    let initial = kernel.insert(state)?;
    let mut fuel = limits.fuel;
    let result = kernel.execute(interpreter, initial, &mut fuel)?;
    let mut state = kernel.state(result)?.clone();
    if !state.frames.is_empty() || state.family.is_some() {
        return Err(fail("unfinished definition"));
    }
    let entry = state
        .outer
        .finish(Rc::make_mut(&mut state.program))
        .map_err(|e| fail(e.0))?;
    if state.program.len() > limits.code_words {
        return Err(Error::StorageLimit);
    }
    *program = (*state.program).clone();
    Ok(entry)
}
