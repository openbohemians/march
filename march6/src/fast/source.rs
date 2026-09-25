//! Provisional host-side Forth reader for the conventional execution spike.
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
//!   backslash starts a comment extending to the end of the line.
//!
//! No forward names, lexical captures, or effects are
//! provided. Builtin names cannot be redefined through this reader yet.

use super::{Binary, Literal, Op, Program, Slot, WordId};
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
            | "="
            | "<"
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
                tokens.push(Token { text, line });
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

#[derive(Default)]
struct Body {
    ops: Vec<Op>,
    stack: Vec<StackValue>,
    inputs: usize,
}

impl Body {
    fn emit(&mut self, op: Op, quote: Option<WordId>) -> StackValue {
        let slot = self.ops.len();
        self.ops.push(op);
        StackValue { slot, quote }
    }

    fn push_op(&mut self, op: Op, quote: Option<WordId>) {
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

    fn call(&mut self, program: &Program, word: WordId) -> Result<(), SourceError> {
        let (inputs, outputs) = program.signature(word).map_err(runtime_error)?;
        let arguments = self.take(inputs).into_iter().map(|v| v.slot).collect();
        let call = self.emit(Op::Call { word, arguments }, None).slot;
        for output in 0..outputs {
            self.push_op(Op::Project { call, output }, None);
        }
        Ok(())
    }

    fn finish(mut self, program: &mut Program) -> Result<WordId, SourceError> {
        // Each newly discovered missing input lies below earlier inputs.
        for op in &mut self.ops {
            if let Op::Arg(index) = op {
                *index = self.inputs - 1 - *index;
            }
        }
        let outputs = self.stack.into_iter().map(|v| v.slot).collect();
        program
            .add_word(self.inputs, self.ops, outputs)
            .map_err(runtime_error)
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
            .add_family(inputs, outputs, clauses)
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
        let mut body = Body::default();
        while let Some(token) = self.next() {
            let text = token.text.as_str();
            if Some(text) == end {
                return body.finish(self.program);
            }
            let local_error =
                |message: String| SourceError(format!("line {}: {message}", token.line));
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
                "true" => body.push_op(Op::Const(Literal::Bool(true)), None),
                "false" => body.push_op(Op::Const(Literal::Bool(false)), None),
                "unit" => body.push_op(Op::Const(Literal::Unit), None),
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
                "dup" => {
                    body.ensure(1);
                    body.stack.push(*body.stack.last().unwrap());
                }
                "drop" => {
                    body.take(1);
                }
                "swap" => {
                    body.ensure(2);
                    let n = body.stack.len();
                    body.stack.swap(n - 1, n - 2);
                }
                "over" => {
                    body.ensure(2);
                    body.stack.push(body.stack[body.stack.len() - 2]);
                }
                "+" | "-" | "*" | "=" | "<" => {
                    let args = body.take(2);
                    let binary = match text {
                        "+" => Binary::Add,
                        "-" => Binary::Sub,
                        "*" => Binary::Mul,
                        "=" => Binary::Eq,
                        "<" => Binary::Lt,
                        _ => unreachable!(),
                    };
                    body.push_op(Op::Binary(binary, args[0].slot, args[1].slot), None);
                }
                "select" => {
                    let args = body.take(3);
                    body.push_op(
                        Op::Select {
                            condition: args[0].slot,
                            when_true: args[1].slot,
                            when_false: args[2].slot,
                        },
                        // Even equal quoted branches must not allow static
                        // `call` to silently bypass demand on the condition.
                        None,
                    );
                }
                "pair" => {
                    let args = body.take(2);
                    body.push_op(Op::Pair(args[0].slot, args[1].slot), None);
                }
                "first" | "second" => {
                    let value = body.take(1)[0];
                    body.push_op(
                        if text == "first" {
                            Op::First(value.slot)
                        } else {
                            Op::Second(value.slot)
                        },
                        None,
                    );
                }
                "call" => {
                    let quote = body.take(1)[0].quote.ok_or_else(|| {
                        local_error(
                            "'call' requires a statically known quotation in this spike".into(),
                        )
                    })?;
                    body.call(self.program, quote)?;
                }
                "apply" => {
                    let inputs = stack_count(self.required("apply input count")?)?;
                    let outputs = stack_count(self.required("apply output count")?)?;
                    let function = body.take(1)[0].slot;
                    let arguments = body.take(inputs).into_iter().map(|v| v.slot).collect();
                    let call = body
                        .emit(
                            Op::Apply {
                                function,
                                arguments,
                                outputs,
                            },
                            None,
                        )
                        .slot;
                    for output in 0..outputs {
                        body.push_op(Op::Project { call, output }, None);
                    }
                }
                "recur" => {
                    let inputs = self.required("recur input count")?;
                    let outputs = self.required("recur output count")?;
                    let inputs = stack_count(inputs)?;
                    let outputs = stack_count(outputs)?;
                    // The runtime checks these projections against the enclosing
                    // word/family signature. No unfinished name lookup is needed.
                    let arguments = body.take(inputs).into_iter().map(|v| v.slot).collect();
                    let call = body.emit(Op::Recur { arguments }, None).slot;
                    for output in 0..outputs {
                        body.push_op(Op::Project { call, output }, None);
                    }
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
    Reader {
        program,
        tokens: tokenize(source)?,
        cursor: 0,
    }
    .body(None, true, 0)
}
