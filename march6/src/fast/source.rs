//! Host-side seed assembler and test convenience reader. Uses the same canonical
//! definitions as the March stream compiler, never graph-based language identity.
//! The active CLI uses `stream::compile` and its March-defined interpreter.
//! Do not add user-facing syntax policy here; stream words own their input.
//!
//! Missing stack inputs are inferred. Public arguments and results are ordered
//! bottom-to-top: `: subtract - ;` receives `[left, right]`. Quotations are closed
//! words with explicit inferred inputs, not lexical closures. This reader is
//! scaffolding, not a self-hosting claim.
//!
//! Surface forms:
//! - `: name body ;` installs a closed word with an inferred stack signature.
//! - `[ body ]`, `quote name`, and `' name` produce closed quotations; `call`
//!   currently requires a statically known quote, including through shuffles.
//!   `apply N M` dynamically applies a quotation with N inputs and M outputs.
//! - `ctx key` reads the current invocation's immutable context lazily.
//! - `condition true-value false-value select` demands only the chosen value.
//! - `family name inputs outputs guard body ... ;` installs ordered clauses.
//!   Guard and body names must already exist and have compatible signatures.
//! - `recur inputs outputs` calls the enclosing word or selected family. Its
//!   explicit signature is provisional and validated by runtime operations.
//! - `pair`, `first`, `second` construct and project lazy immutable pairs.
//! - Parentheses contain comments, not checked stack-effect declarations;
//!   `--` starts a line comment; legacy backslash comments are also accepted.
//!
//! No forward names, lexical captures, or effects are
//! provided. Builtin names cannot be redefined through this reader yet.

use super::{Literal, Op, Program, Slot, WordId};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError(pub String);

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SourceError {}

#[derive(Clone, Debug)]
struct Token {
    text: String,
    line: usize,
}

const MAX_EXPLICIT_STACK: usize = 4096;
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;

fn stack_count(token: Token) -> Result<usize, SourceError> {
    token
        .text
        .parse::<usize>()
        .ok()
        .filter(|&n| n <= MAX_EXPLICIT_STACK)
        .ok_or_else(|| {
            SourceError(format!(
                "line {}: expected stack count between 0 and {MAX_EXPLICIT_STACK}, got '{}'",
                token.line, token.text
            ))
        })
}

fn definition_name(token: &Token) -> Result<(), SourceError> {
    // Builtins have fixed syntax in this provisional reader. Refuse names that
    // would be bound successfully but could never subsequently be referenced.
    if matches!(
        token.text.as_str(),
        "[" | "]"
            | ":"
            | ";"
            | "true"
            | "false"
            | "unit"
            | "ctx"
            | "quote"
            | "'"
            | "dup"
            | "drop"
            | "swap"
            | "over"
            | "+"
            | "-"
            | "*"
            | "eq?"
            | "lt?"
            | "select"
            | "pair"
            | "first"
            | "second"
            | "call"
            | "apply"
            | "recur"
            | "family"
    ) || token.text.parse::<i64>().is_ok()
    {
        return Err(SourceError(format!(
            "line {}: '{}' is reserved syntax, not a definition name",
            token.line, token.text
        )));
    }
    Ok(())
}

fn tokenize(source: &str) -> Result<Vec<Token>, SourceError> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    let mut line = 1;
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                let start = line;
                let mut text = String::from("\"");
                let mut closed = false;
                while let Some(c) = chars.next() {
                    text.push(c);
                    if c == '\n' {
                        line += 1;
                    }
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\'
                        && let Some(next) = chars.next()
                    {
                        text.push(next);
                        if next == '\n' {
                            line += 1;
                        }
                    }
                }
                if !closed {
                    return Err(SourceError(format!(
                        "line {start}: unterminated text literal"
                    )));
                }
                if chars
                    .peek()
                    .is_some_and(|c| !c.is_whitespace() && !"[]:;()\\".contains(*c))
                {
                    return Err(SourceError("text literal needs a separator".into()));
                }
                tokens.push(Token { text, line: start });
            }
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '\\' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '(' => {
                let start = line;
                let mut depth = 1;
                for c in chars.by_ref() {
                    match c {
                        '\n' => line += 1,
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth != 0 {
                    return Err(SourceError(format!("line {start}: unclosed comment")));
                }
            }
            '[' | ']' | ':' | ';' => tokens.push(Token {
                text: ch.to_string(),
                line,
            }),
            ')' => return Err(SourceError(format!("line {line}: unexpected ')'"))),
            _ => {
                let mut text = ch.to_string();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || "[]:;()\\".contains(c) {
                        break;
                    }
                    text.push(chars.next().unwrap());
                }
                if text == "--" {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            line += 1;
                            break;
                        }
                    }
                } else {
                    tokens.push(Token { text, line });
                }
            }
        }
    }
    Ok(tokens)
}

#[derive(Clone, Copy)]
struct StackValue {
    slot: Slot,
    quote: Option<WordId>,
}

#[derive(Clone, Default)]
pub(super) struct Body {
    ops: Vec<Op>,
    stack: Vec<StackValue>,
    inputs: usize,
    definition: Option<Vec<super::definition::Item>>,
}

impl Body {
    pub(super) fn tuple(&mut self, count: usize, unpack: bool) {
        if self.record(if unpack {
            super::definition::Item::Untuple(count)
        } else {
            super::definition::Item::Tuple(count)
        }) {
            return;
        }
        if unpack {
            let tuple = self.take(1)[0].slot;
            let checked = self.emit(Op::TupleCheck { tuple, count }, None).slot;
            for i in 0..count {
                let index = self.emit(Op::Const(Literal::Int(i as i64)), None).slot;
                self.push_op(
                    Op::Data {
                        primitive: super::data::Primitive::Nth,
                        arguments: vec![checked, index],
                    },
                    None,
                );
            }
        } else {
            let fields = self.take(count).into_iter().map(|v| v.slot).collect();
            self.push_op(Op::Tuple(fields), None);
        }
    }
    pub(super) fn operation_count(&self) -> usize {
        self.ops.len()
    }
    pub(super) fn composed() -> Self {
        Self {
            definition: Some(Vec::new()),
            ..Self::default()
        }
    }
    fn record(&mut self, item: super::definition::Item) -> bool {
        if let Some(items) = &mut self.definition {
            items.push(item);
            true
        } else {
            false
        }
    }
    pub(super) fn dynamic(&mut self, inputs: usize, outputs: usize, recur: bool) {
        if self.record(if recur {
            super::definition::Item::Recur { inputs, outputs }
        } else {
            super::definition::Item::Apply { inputs, outputs }
        }) {
            return;
        }
        let function = if recur {
            None
        } else {
            Some(self.take(1)[0].slot)
        };
        let arguments = self.take(inputs).into_iter().map(|v| v.slot).collect();
        let op = if let Some(function) = function {
            Op::Apply {
                function,
                arguments,
                outputs,
            }
        } else {
            Op::Recur { arguments }
        };
        let call = self.emit(op, None).slot;
        for output in 0..outputs {
            self.push_op(Op::Project { call, output }, None);
        }
    }
    pub(super) fn static_call(&mut self, program: &Program) -> Result<(), SourceError> {
        if self.record(super::definition::Item::StaticCall) {
            return Ok(());
        }
        let word = self.take(1)[0].quote.ok_or_else(|| {
            SourceError("call needs a known quotation; use apply for dynamic code".into())
        })?;
        self.lower_call(program, word)
    }
    fn emit(&mut self, op: Op, quote: Option<WordId>) -> StackValue {
        let slot = self.ops.len();
        self.ops.push(op);
        StackValue { slot, quote }
    }

    pub(super) fn push_op(&mut self, op: Op, quote: Option<WordId>) {
        let recorded = match &op {
            Op::Const(value) => self.record(super::definition::Item::Literal(*value)),
            Op::Context(key) => self.record(super::definition::Item::Context(key.clone())),
            _ => false,
        };
        if recorded {
            return;
        }
        debug_assert!(
            self.definition.is_none(),
            "composed builders accept definition items, not register operations"
        );
        let value = self.emit(op, quote);
        self.stack.push(value);
    }

    fn ensure(&mut self, count: usize) {
        while self.stack.len() < count {
            let arg = self.emit(Op::Arg(self.inputs), None);
            self.inputs += 1;
            self.stack.insert(0, arg);
        }
    }

    fn take(&mut self, count: usize) -> Vec<StackValue> {
        self.ensure(count);
        self.stack.split_off(self.stack.len() - count)
    }

    pub(super) fn call(&mut self, program: &Program, word: WordId) -> Result<(), SourceError> {
        if self.record(super::definition::Item::Word(word)) {
            return Ok(());
        }
        self.lower_call(program, word)
    }
    fn lower_call(&mut self, program: &Program, word: WordId) -> Result<(), SourceError> {
        let (inputs, outputs) = program.signature(word).map_err(runtime_error)?;
        // Inline semantic primitive leaves only in the derived graph. Keep the
        // original word reference in the canonical sequence, including shuffles.
        if matches!(
            program.definition(word).map_err(runtime_error)?,
            Some(super::definition::Definition::Primitive(_))
        ) {
            let args = self.take(inputs);
            let primitive = program.word(word).map_err(runtime_error)?;
            let mut values: Vec<StackValue> = Vec::new();
            for op in &primitive.ops {
                let value = match *op {
                    Op::Arg(n) => args[n],
                    Op::Const(v) => self.emit(Op::Const(v), None),
                    Op::Binary(b, a, c) => {
                        self.emit(Op::Binary(b, values[a].slot, values[c].slot), None)
                    }
                    Op::Pair(a, b) => self.emit(Op::Pair(values[a].slot, values[b].slot), None),
                    Op::First(a) => self.emit(Op::First(values[a].slot), None),
                    Op::Second(a) => self.emit(Op::Second(values[a].slot), None),
                    Op::Data {
                        primitive,
                        ref arguments,
                    } => self.emit(
                        Op::Data {
                            primitive,
                            arguments: arguments.iter().map(|&i| values[i].slot).collect(),
                        },
                        None,
                    ),
                    Op::Select {
                        condition,
                        when_true,
                        when_false,
                    } => self.emit(
                        Op::Select {
                            condition: values[condition].slot,
                            when_true: values[when_true].slot,
                            when_false: values[when_false].slot,
                        },
                        None,
                    ),
                    _ => unreachable!("runtime primitive leaf"),
                };
                values.push(value);
            }
            self.stack
                .extend(primitive.outputs.iter().map(|&s| values[s]));
            return Ok(());
        }
        let arguments = self.take(inputs).into_iter().map(|v| v.slot).collect();
        let call = self.emit(Op::Call { word, arguments }, None).slot;
        for output in 0..outputs {
            self.push_op(Op::Project { call, output }, None);
        }
        Ok(())
    }

    pub(super) fn finish(mut self, program: &mut Program) -> Result<WordId, SourceError> {
        let items = self.definition.take().ok_or_else(|| {
            SourceError("only a composed definition can be installed by the source builder".into())
        })?;
        program
            .add_definition(super::definition::Definition::Sequence(items))
            .map_err(runtime_error)
    }
    pub(super) fn lowered(mut self) -> (usize, Vec<Op>, Vec<Slot>) {
        // Each newly discovered missing input lies below earlier inputs.
        for op in &mut self.ops {
            if let Op::Arg(index) = op {
                *index = self.inputs - 1 - *index;
            }
        }
        let outputs = self.stack.into_iter().map(|v| v.slot).collect();
        (self.inputs, self.ops, outputs)
    }
}

fn runtime_error(error: impl fmt::Display) -> SourceError {
    SourceError(error.to_string())
}

struct Reader<'a> {
    program: &'a mut Program,
    tokens: Vec<Token>,
    cursor: usize,
}

impl Reader<'_> {
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }

    fn required(&mut self, description: &str) -> Result<Token, SourceError> {
        self.next()
            .ok_or_else(|| SourceError(format!("expected {description}, found end of source")))
    }

    fn named(&mut self, description: &str) -> Result<WordId, SourceError> {
        let token = self.required(description)?;
        self.program.lookup(&token.text).ok_or_else(|| {
            SourceError(format!(
                "line {}: unknown word '{}'",
                token.line, token.text
            ))
        })
    }

    fn family(&mut self) -> Result<(), SourceError> {
        let name = self.required("family name")?;
        definition_name(&name)?;
        let inputs = self.required("family input count")?;
        let outputs = self.required("family output count")?;
        let inputs = stack_count(inputs)?;
        let outputs = stack_count(outputs)?;
        let mut clauses = Vec::new();
        loop {
            if self.tokens.get(self.cursor).is_some_and(|t| t.text == ";") {
                self.cursor += 1;
                break;
            }
            clauses.push((self.named("guard word or ';'")?, self.named("body word")?));
        }
        let word = self
            .program
            .add_source_family(inputs, outputs, clauses, true)
            .map_err(runtime_error)?;
        self.program.bind(&name.text, word).map_err(runtime_error)
    }

    fn body(
        &mut self,
        end: Option<&str>,
        definitions: bool,
        depth: usize,
    ) -> Result<WordId, SourceError> {
        if depth > 128 {
            return Err(SourceError("quotation nesting exceeds 128".into()));
        }
        let mut body = Body::composed();
        while let Some(token) = self.next() {
            let text = token.text.as_str();
            if Some(text) == end {
                return body.finish(self.program);
            }
            let local_error =
                |message: String| SourceError(format!("line {}: {message}", token.line));
            if text.starts_with('"') {
                let (value, _) = super::data::read_text(text, 0).map_err(runtime_error)?;
                let id = self.program.intern_text(&value).map_err(runtime_error)?;
                body.push_op(Op::Const(Literal::Text(id)), None);
                continue;
            }
            match text {
                ":" if definitions => {
                    let name = self.required("definition name")?;
                    definition_name(&name)?;
                    let word = self.body(Some(";"), false, depth + 1)?;
                    self.program.bind(&name.text, word).map_err(runtime_error)?;
                }
                "family" if definitions => self.family()?,
                "[" => {
                    let word = self.body(Some("]"), false, depth + 1)?;
                    body.push_op(Op::Const(Literal::Quote(word)), Some(word));
                }
                "]" | ";" | ":" => return Err(local_error(format!("unexpected '{text}'"))),
                "ctx" => {
                    let key = self.required("context key")?;
                    if "[]:;".contains(&key.text) {
                        return Err(local_error("invalid context key".into()));
                    }
                    body.push_op(Op::Context(key.text), None);
                }
                "quote" | "'" => {
                    let word = self.named("quoted word name")?;
                    body.push_op(Op::Const(Literal::Quote(word)), Some(word));
                }
                "call" => {
                    body.static_call(self.program)?;
                }
                "tuple" | "untuple" => {
                    let count = stack_count(self.required("tuple arity")?)?;
                    body.tuple(count, text == "untuple");
                }
                "apply" => {
                    let inputs = stack_count(self.required("apply input count")?)?;
                    let outputs = stack_count(self.required("apply output count")?)?;
                    body.dynamic(inputs, outputs, false);
                }
                "recur" => {
                    let inputs = self.required("recur input count")?;
                    let outputs = self.required("recur output count")?;
                    let inputs = stack_count(inputs)?;
                    let outputs = stack_count(outputs)?;
                    // The runtime checks these projections against the enclosing
                    // word/family signature. No unfinished name lookup is needed.
                    body.dynamic(inputs, outputs, true);
                }
                _ => {
                    if let Ok(value) = text.parse::<i64>() {
                        body.push_op(Op::Const(Literal::Int(value)), None);
                    } else if let Some(word) = self.program.lookup(text) {
                        body.call(self.program, word)?;
                    } else {
                        return Err(local_error(format!("unknown word '{text}'")));
                    }
                }
            }
        }
        if let Some(end) = end {
            return Err(SourceError(format!(
                "expected '{end}', found end of source"
            )));
        }
        body.finish(self.program)
    }
}

/// Compile definitions and a final expression, returning the expression word.
/// Earlier successful definitions remain installed if a later form fails.
pub fn compile(program: &mut Program, source: &str) -> Result<WordId, SourceError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SourceError("source exceeds 4 MiB limit".into()));
    }
    super::definition::install_runtime(program).map_err(runtime_error)?;
    compile_composed(program, source)
}
pub(super) fn compile_composed(program: &mut Program, source: &str) -> Result<WordId, SourceError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SourceError("source exceeds 4 MiB limit".into()));
    }
    Reader {
        program,
        tokens: tokenize(source)?,
        cursor: 0,
    }
    .body(None, true, 0)
}
