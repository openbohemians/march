//! The symbolic stack machine (docs/MACHINE.md): it runs the explicit form,
//! in which `.` applies, on a stack of judgments, and leaves code for the
//! machine behind.
//!
//! A judgment is a value's type, and its value when that is known at compile
//! time. A known value has no code until a value at run time is needed:
//! it is *materialized* then, so constants fold through every word, and the
//! code holds only what must happen at run time.

use crate::code::{Blob, Cid, Op, Primitive as P};
use crate::error::{Error, Kind, Level, Pos, Result, Warning};
use crate::prims::{self, Effects, PRIMS, Prim};
use crate::read::{Piece, Tok, Token};
use crate::types::*;
use std::collections::HashMap;
use std::rc::Rc;

/// What is known of a value at compile time.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// Nothing: the value is on the machine's stack.
    Run,
    /// An integer: an integer literal, an i64, or money in cents.
    Int(i128),
    /// A decimal literal: its digits and how many follow the point.
    Dec(i128, u32),
    Float(f64),
    Str(Rc<str>),
    /// A type, as a value.
    Type(Type),
    /// A name not yet applied, and where it was written, where errors in
    /// applying it are reported.
    Name(Rc<str>, Pos),
    /// A quotation.
    Quote(Rc<[Token]>),
    /// A value known at compile time whose cell is on the machine's stack:
    /// an input a value pattern matched, inside its clause. Copied, it is the
    /// known value; folded, its cell is dropped; else it stays as it is.
    Pinned(Box<Val>),
    /// A run: zero or more values of the type, on the machine's stack, as
    /// many as the code inside an array literal left, counted only by its
    /// gather. Nothing reaches beneath a run.
    Many,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Jdg {
    pub ty: Type,
    pub val: Val,
}

impl Jdg {
    /// Whether the value is known at compile time, and so has no cell on
    /// the machine's stack yet.
    pub fn known(&self) -> bool {
        !matches!(self.val, Val::Run | Val::Many | Val::Pinned(_))
    }

    /// A copy of the judgment that needs no cell, if its value is known:
    /// a pinned value's copy is its known value.
    fn known_copy(&self) -> Option<Jdg> {
        match &self.val {
            Val::Pinned(v) => Some(Jdg {
                ty: self.ty,
                val: (**v).clone(),
            }),
            _ if self.known() => Some(self.clone()),
            _ => None,
        }
    }
}

/// Code as the stage emits it. Labels keep branches relative until the code
/// is sealed (docs/MACHINE.md).
#[derive(Clone, Debug, PartialEq)]
pub enum Ins {
    Op(Op),
    /// A string literal: a data object and `text`.
    Str(Rc<str>),
    /// A place a branch goes to.
    Label(u32),
    /// A branch to a label.
    Jump(u32),
    /// A branch to a label if the value on top, taken, is 0.
    JumpZero(u32),
}

/// A guard in a signature: a word that looks at `arity` inputs, from input
/// `slot`, without taking them, and leaves a flag (TYPES.md 2.15).
#[derive(Clone, Debug)]
pub struct Guard {
    pub test: Test,
    pub slot: usize,
    pub arity: usize,
    pub pos: Pos,
}

/// What a guard tests: a quotation that leaves a flag, `[ positive?. ]`, or,
/// for a value pattern, that its input equals a value (`< 0 >`).
#[derive(Clone, Debug)]
pub enum Test {
    Quote(Rc<[Token]>),
    Equals(Jdg),
}

/// Guards are equal by what they say, not where they were written, so a
/// clause written again replaces itself.
impl PartialEq for Test {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (Test::Quote(a), Test::Quote(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.tok == y.tok)
            }
            (Test::Equals(a), Test::Equals(b)) => a == b,
            _ => false,
        }
    }
}

/// Tokens as they are written, for a message.
fn source_text(toks: &[Token]) -> String {
    let mut out = String::new();
    for t in toks {
        let word = match &t.tok {
            Tok::Apply => {
                out.push('.');
                continue;
            }
            Tok::Name(s) => s.to_string(),
            Tok::Int(n) => n.to_string(),
            Tok::Dec(d, s) => prims::show_dec(*d, *s),
            Tok::Str(s) => format!("{s:?}"),
            Tok::Open(b) | Tok::Close(b) => (*b as char).to_string(),
            Tok::Dashes => "--".into(),
            Tok::Template(_) => "\"…\"".into(),
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&word);
    }
    out
}

impl Guard {
    /// The guard as a message names it.
    fn name(&self) -> String {
        match &self.test {
            Test::Quote(q) => format!("the guard `[ {} ]`", source_text(q)),
            Test::Equals(j) => format!("the value pattern {}", value_text(j)),
        }
    }
}

/// A value as it is written.
fn value_text(j: &Jdg) -> String {
    match &j.val {
        Val::Int(n) if j.ty == MONEY => prims::show_dec(*n, 2),
        Val::Int(_) if j.ty == NIL => "nil".into(),
        Val::Int(0) if j.ty == BOOL => "false".into(),
        Val::Int(_) if j.ty == BOOL => "true".into(),
        Val::Int(n) => n.to_string(),
        Val::Dec(d, s) => prims::show_dec(*d, *s),
        Val::Float(x) => format!("{x:?}"),
        Val::Str(s) => format!("{s:?}"),
        _ => "a value".into(),
    }
}

/// An open array or map literal: its bracket, where its elements start, the
/// floor outside it, and where each `_.` that pulls a value from below it is
/// written, in reading order; it takes as many inputs.
struct Literal {
    bracket: u8,
    start: usize,
    floor: usize,
    pulls: Vec<Pos>,
}

/// Where each `_.` in a string's holes is.
fn template_pulls(pieces: &[Piece]) -> Vec<Pos> {
    pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Hole(toks) => Some(pulls_in(toks)),
            Piece::Text(_) => None,
        })
        .flatten()
        .collect()
}

/// Where each `_.` in a literal's words is, not counting those of literals
/// inside it.
fn pulls_in(toks: &[Token]) -> Vec<Pos> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i].tok {
            Tok::Open(b'(' | b'{') => i = matching(toks, i),
            Tok::Name(n) if &**n == "_" && toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) => {
                out.push(toks[i].pos);
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// A signature: the input patterns, deepest first, and the outputs it
/// promises, if it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Sig {
    pub ins: Vec<Type>,
    pub guards: Vec<Guard>,
    pub outs: Option<Vec<Type>>,
}

impl PartialEq for Guard {
    fn eq(&self, o: &Self) -> bool {
        (&self.test, self.slot, self.arity) == (&o.test, o.slot, o.arity)
    }
}

impl Sig {
    /// The context a clause is chosen by: its inputs and guards.
    fn context(&self) -> (&[Type], &[Guard]) {
        (&self.ins, &self.guards)
    }
    /// Whether input `k` is a value pattern.
    fn value(&self, k: usize) -> bool {
        self.guards
            .iter()
            .any(|g| g.slot == k && matches!(g.test, Test::Equals(_)))
    }
}

impl Clause {
    fn guarded(&self) -> bool {
        self.sig.as_ref().is_some_and(|s| !s.guards.is_empty())
    }
}

/// One way a family's application can go: its code, and what it leaves, if
/// it returns.
struct Alt {
    code: Vec<Ins>,
    stack: Option<Vec<Jdg>>,
    /// The label its guards' tests branch to when they fail.
    skip: Option<u32>,
}

type Candidate = (i32, Rc<Clause>, Env);

/// A family and the types it is applied to.
type Key = (Rc<str>, Vec<Type>);

/// What became of an alternative of a choice.
enum Outcome {
    /// Its guard fails at compile time.
    Dropped,
    /// Compiled; and whether it is taken for certain.
    Taken(Alt, bool),
    /// It reached the recursion before its results' types were known.
    Waits(bool),
}

/// An instance being compiled: a family, its input types, and the types of
/// its results when it calls itself, once known.
struct Frame {
    name: Rc<str>,
    ins: Vec<Type>,
    ghost: Option<Vec<Type>>,
    /// Whether a recursive call used the ghost.
    used: bool,
}

/// What the effect pass knows of a value.
enum Item {
    Name(Rc<str>),
    Quote(usize, usize),
    Other,
}

/// The candidate that matches best, the first defined among equals.
fn primary(cands: &[Candidate]) -> Option<Candidate> {
    let top = cands.iter().map(|c| c.0).max()?;
    cands.iter().find(|c| c.0 == top).cloned()
}

/// Whether the code from `i` returns at once, perhaps after branches.
fn returns(ops: &[Op], mut i: usize) -> bool {
    for _ in 0..ops.len() {
        match ops.get(i) {
            Some(Op::Return) => return true,
            Some(Op::Branch(t)) => i = *t as usize,
            _ => return false,
        }
    }
    false
}

fn take(items: &mut Vec<Item>, ins: &mut usize, k: usize) {
    if items.len() >= k {
        items.truncate(items.len() - k);
    } else {
        *ins += k - items.len();
        items.clear();
    }
}

/// One definition: a context and a body (doc/design/TYPES.md 2.15).
#[derive(Debug)]
pub struct Clause {
    pub sig: Option<Sig>,
    pub body: Rc<[Token]>,
    pub at: Pos,
    /// Defined by the core vocabulary: its errors are reported where it is
    /// applied, since its own source is not the program's.
    pub core: bool,
}

/// What a name means. A name may be a type and have clauses too: the
/// clauses of a type's name are how literals become it.
#[derive(Default)]
struct Word {
    ty: Option<Type>,
    con: Option<Con>,
    prim: Option<(Prim, Rc<Sig>)>,
    clauses: Vec<Rc<Clause>>,
}

/// Words applied inside words, at most this deep.
const DEPTH: u32 = 64;
/// Tokens evaluated in one compilation, at most.
const STEPS: u64 = 10_000_000;

pub struct Stage {
    pub types: Types,
    words: HashMap<Rc<str>, Word>,
    /// The names of the types that have them, for their conversions.
    type_names: HashMap<Type, Rc<str>>,
    pub stack: Vec<Jdg>,
    pub code: Vec<Ins>,
    /// The judgments below this are out of reach: a signature's inputs or an
    /// array literal's start.
    floor: usize,
    /// Open array and map literals.
    literals: Vec<Literal>,
    /// Families being applied, with the types they were applied to, so that
    /// one applying itself to the same types is caught.
    active: Vec<Key>,
    labels: u32,
    /// Instances being compiled, innermost last.
    frames: Vec<Frame>,
    /// Instances compiled: their code's identity and their results' types.
    instances: HashMap<Key, (Cid, Vec<Type>, Effects)>,
    /// Code and data made while compiling, in the order made, so a callee
    /// comes before its callers; the session publishes them.
    pub blobs: Vec<Blob>,
    /// The effects of the code compiled so far, in the word or guard being
    /// compiled.
    pub effects: Effects,
    pos: Pos,
    steps: u64,
    depth: u32,
    /// Set while the core vocabulary is read.
    pub core: bool,
    /// Warnings found, for the session to report.
    pub warnings: Vec<Warning>,
    /// Where the program applied the core clause being evaluated, if one
    /// is: its warnings are reported there, as its errors are.
    site: Option<Pos>,
    /// Set while a bracket's code runs, where a word of one letter is a type
    /// variable.
    in_bracket: u32,
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

/// The index of the bracket that closes the one opened at `i`.
fn matching(toks: &[Token], i: usize) -> usize {
    let Tok::Open(o) = toks[i].tok else {
        unreachable!("an opening bracket")
    };
    let mut depth = 0;
    for (j, t) in toks.iter().enumerate().skip(i) {
        match t.tok {
            Tok::Open(b) if b == o => depth += 1,
            Tok::Close(b) if b == closer(o) => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
            _ => {}
        }
    }
    unreachable!("the reader checks that brackets nest")
}

fn closer(o: u8) -> u8 {
    match o {
        b'[' => b']',
        b'(' => b')',
        b'{' => b'}',
        _ => b'>',
    }
}

fn values(n: usize) -> String {
    match n {
        1 => "1 value".into(),
        n => format!("{n} values"),
    }
}

fn float_bits(x: f64) -> u64 {
    if x.is_nan() {
        f64::NAN.to_bits()
    } else {
        x.to_bits()
    }
}

impl Stage {
    pub fn new() -> Self {
        let mut s = Self {
            types: Types::new(),
            words: HashMap::new(),
            type_names: HashMap::new(),
            stack: Vec::new(),
            code: Vec::new(),
            floor: 0,
            literals: Vec::new(),
            active: Vec::new(),
            pos: Pos::default(),
            steps: 0,
            depth: 0,
            core: false,
            labels: 0,
            frames: Vec::new(),
            instances: HashMap::new(),
            blobs: Vec::new(),
            effects: 0,
            warnings: Vec::new(),
            site: None,
            in_bracket: 0,
        };
        for (i, (_, name)) in Base::ALL.iter().enumerate() {
            let name: Rc<str> = (*name).into();
            s.word(&name).ty = Some(i as Type);
            s.type_names.insert(i as Type, name);
        }
        for (con, name) in [
            (Con::Ary, "ary"),
            (Con::Vec, "vec"),
            (Con::Map, "map"),
            (Con::Or, "or"),
        ] {
            s.word(&name.into()).con = Some(con);
        }
        let atom = s.types.intern(Term::Atom);
        s.word(&"atom".into()).ty = Some(atom);
        let tuple = s.types.intern(Term::AnyTuple);
        s.word(&"tuple".into()).ty = Some(tuple);
        let none = crate::symbols::Symbols::parse("").expect("an empty table");
        for p in PRIMS {
            let toks = crate::read::read(p.sig, &none).expect("a primitive's signature reads");
            let sig = s
                .bracket(&toks[1..toks.len() - 1])
                .expect("a primitive's signature is well formed");
            s.word(&p.name.into()).prim = Some((p.prim, Rc::new(sig)));
        }
        s
    }

    fn word(&mut self, name: &Rc<str>) -> &mut Word {
        self.words.entry(name.clone()).or_default()
    }

    fn err(&self, kind: Kind, msg: impl Into<String>) -> Error {
        Error::new(kind, Some(self.pos), msg)
    }

    /// Forgets the instances compiled, as when their code could not be
    /// published.
    pub fn forget_instances(&mut self) {
        self.instances.clear();
    }

    /// Starts a compilation whose inputs are values on the machine's stack.
    pub fn begin(&mut self, inputs: &[Type]) {
        self.stack = inputs.iter().map(|&ty| Jdg { ty, val: Val::Run }).collect();
        self.code.clear();
        self.floor = 0;
        self.literals.clear();
        self.active.clear();
        self.frames.clear();
        self.effects = 0;
        self.steps = 0;
        self.depth = 0;
    }

    /// Ends a compilation: every value left is made a value at run time, and
    /// their types are returned.
    pub fn finish(&mut self) -> Result<Vec<Type>> {
        for i in 0..self.stack.len() {
            self.materialize(i)?;
        }
        Ok(self.stack.iter().map(|j| j.ty).collect())
    }

    fn emit(&mut self, op: Op) {
        self.code.push(Ins::Op(op));
    }

    fn label(&mut self) -> u32 {
        self.labels += 1;
        self.labels
    }

    fn push(&mut self, ty: Type, val: Val) {
        self.stack.push(Jdg { ty, val });
    }

    /// Checks that `n` values are within reach.
    fn need(&self, n: usize) -> Result<()> {
        let have = self.stack.len() - self.floor;
        if self.stack[self.stack.len() - n.min(have)..]
            .iter()
            .any(|j| j.val == Val::Many)
        {
            return Err(self.err(
                Kind::Mismatch,
                "a run of values, left in an array literal, can only be gathered by it",
            ));
        }
        if have < n {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "needs {} here, and {} in reach",
                    values(n),
                    match have {
                        1 => "1 is".to_string(),
                        h => format!("{h} are"),
                    }
                ),
            ));
        }
        Ok(())
    }

    fn pop(&mut self) -> Result<Jdg> {
        self.need(1)?;
        Ok(self.stack.pop().expect("checked"))
    }

    /// Evaluates tokens of the explicit form.
    pub fn run(&mut self, toks: &[Token]) -> Result<()> {
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            self.pos = t.pos;
            self.steps += 1;
            if self.steps > STEPS {
                return Err(self.err(Kind::Limit, "compiling took too many steps"));
            }
            match &t.tok {
                Tok::Int(n) => self.push(INT_LIT, Val::Int(*n)),
                Tok::Dec(d, s) => self.push(DEC_LIT, Val::Dec(*d, *s)),
                Tok::Str(s) => self.push(STRING, Val::Str(s.clone())),
                Tok::Template(pieces) => self.template(pieces, t.pos)?,
                Tok::Name(s)
                    if &**s == "_" && toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) =>
                {
                    self.pull(t.pos)?;
                    i += 1;
                }
                // In a bracket, a word of one letter is a type variable: its
                // one rule of its own.
                Tok::Name(s)
                    if self.in_bracket > 0
                        && s.len() == 1
                        && s.as_bytes()[0].is_ascii_lowercase() =>
                {
                    let v = self.types.intern(Term::Var(s.as_bytes()[0] - b'a'));
                    self.push(TYPE, Val::Type(v));
                }
                // And a word ending in `?` is a guard, as if quoted: it can be
                // no symbol (Thomas, 2026-10-09).
                Tok::Name(s) if self.in_bracket > 0 && s.ends_with('?') => {
                    let q: Rc<[Token]> = vec![
                        t.clone(),
                        Token {
                            tok: Tok::Apply,
                            pos: t.pos,
                        },
                    ]
                    .into();
                    self.push(QUOTE, Val::Quote(q));
                }
                Tok::Name(s) => self.push(SYMBOL, Val::Name(s.clone(), t.pos)),
                Tok::Apply => self.apply()?,
                Tok::Open(b'[') => {
                    let j = matching(toks, i);
                    self.push(QUOTE, Val::Quote(toks[i + 1..j].into()));
                    i = j;
                }
                Tok::Open(b'<') => {
                    let j = matching(toks, i);
                    let sig = self.bracket(&toks[i + 1..j])?;
                    self.pos = t.pos;
                    self.annotate_top(&sig)?;
                    i = j;
                }
                // An array of types, in a bracket, is a tuple type: no code.
                Tok::Open(b'(') if self.in_bracket > 0 => {
                    self.literals.push(Literal {
                        bracket: b'(',
                        start: self.stack.len(),
                        floor: self.floor,
                        pulls: Vec::new(),
                    });
                    self.floor = self.stack.len();
                }
                Tok::Open(b) => {
                    let pulls = pulls_in(&toks[i + 1..matching(toks, i)]);
                    self.need(pulls.len())?;
                    self.emit(Op::Prim(P::Mark));
                    self.literals.push(Literal {
                        bracket: *b,
                        start: self.stack.len(),
                        floor: self.floor,
                        pulls,
                    });
                    self.floor = self.stack.len();
                }
                Tok::Close(b) => self.close_literal(*b)?,
                Tok::Dashes => unreachable!("only inside brackets"),
            }
            i += 1;
        }
        Ok(())
    }

    /// `.`: applies the value on top.
    fn apply(&mut self) -> Result<()> {
        let f = self.pop()?;
        match f.val {
            Val::Name(n, at) => {
                self.pos = at;
                self.apply_word(&n)
            }
            // A type applied is the type: `to` converts to one.
            Val::Type(t) => {
                self.push(TYPE, Val::Type(t));
                Ok(())
            }
            Val::Quote(q) => {
                self.enter()?;
                let r = self.run(&q);
                self.depth -= 1;
                r
            }
            _ => Err(self.err(
                Kind::Mismatch,
                format!("{} cannot be applied", self.describe(&f)),
            )),
        }
    }

    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > DEPTH {
            self.depth -= 1;
            return Err(self.err(Kind::Limit, format!("words applied more than {DEPTH} deep")));
        }
        Ok(())
    }

    fn apply_word(&mut self, name: &Rc<str>) -> Result<()> {
        let Some(w) = self.words.get(name) else {
            return Err(self.err(Kind::NoWord, format!("no word `{name}`")));
        };
        // `map` builds a type from types, and maps over an array otherwise.
        let typeish = self.stack.len() > self.floor && {
            let j = &self.stack[self.stack.len() - 1];
            match &j.val {
                Val::Type(_) => true,
                Val::Name(n, _) => self.words.get(n).is_some_and(|w| w.ty.is_some()),
                _ => j.ty == NIL,
            }
        };
        if let Some(c) = w.con
            && (typeish || (w.prim.is_none() && w.clauses.is_empty()))
        {
            self.construct(c)
        } else if let Some((p, sig)) = w.prim.clone() {
            self.prim(name, p, &sig)
        } else if w.ty == Some(NIL) {
            // A type with one value is that value, `nil.`, which stands for
            // its type where a type is wanted.
            self.push(NIL, Val::Int(0));
            Ok(())
        } else if let Some(t) = w.ty {
            // A type applied is the type: types are operators, as `ary` is
            // (Thomas, 2026-10-09), and `to` converts a value to one.
            self.push(TYPE, Val::Type(t));
            Ok(())
        } else if !w.clauses.is_empty() {
            self.family(name)
        } else {
            Err(self.err(Kind::NoWord, format!("no word `{name}`")))
        }
    }

    /// How a judgment reads in a message.
    fn describe(&self, j: &Jdg) -> String {
        match &j.val {
            Val::Run => self.types.name(j.ty),
            Val::Int(n) if j.ty == MONEY => format!("{} (money)", prims::show_dec(*n, 2)),
            Val::Int(_) if j.ty == NIL => "nil".into(),
            Val::Int(0) if j.ty == BOOL => "false".into(),
            Val::Int(_) if j.ty == BOOL => "true".into(),
            Val::Int(n) => format!("{n} ({})", self.types.name(j.ty)),
            Val::Dec(d, s) => format!("{} (dec#)", prims::show_dec(*d, *s)),
            Val::Float(x) => format!("{x:?} (f64)"),
            Val::Str(s) => format!("{s:?}"),
            Val::Type(t) => format!("the type `{}`", self.types.name(*t)),
            Val::Name(n, _) => format!("the word `{n}`"),
            Val::Quote(_) => "a quotation".into(),
            Val::Many => format!("a run of {}", self.types.name(j.ty)),
            Val::Pinned(v) => self.describe(&Jdg {
                ty: j.ty,
                val: (**v).clone(),
            }),
        }
    }

    /// The types of the top `n` values, for a message.
    fn top_types(&self, n: usize) -> String {
        let n = n.min(self.stack.len() - self.floor);
        if n == 0 {
            return "no values".into();
        }
        self.stack[self.stack.len() - n..]
            .iter()
            .map(|j| self.types.name(j.ty))
            .collect::<Vec<_>>()
            .join(" ")
    }

    // ---- Literals and annotation ----

    /// The clause converting a literal type to `t`, if `t` has one.
    fn conversion(&self, t: Type, lit: Type) -> Option<Rc<Clause>> {
        let name = self.type_names.get(&t)?;
        self.words[name]
            .clauses
            .iter()
            .find(|c| c.sig.as_ref().is_some_and(|s| s.ins == [lit]))
            .cloned()
    }

    fn accepts(&self, t: Type, lit: Type) -> bool {
        self.conversion(t, lit).is_some()
    }

    /// The type a literal takes when nothing says (TYPES.md 2.10).
    fn default_of(lit: Type) -> Type {
        if lit == DEC_LIT { F64 } else { I64 }
    }

    /// Gives the judgment at `i` the type `t`: it has it already; or it is a
    /// vec and `t` an array of the same elements, which forgets the length;
    /// or it is a literal, converted by `t`'s clause for it.
    fn annotate(&mut self, i: usize, t: Type) -> Result<()> {
        let have = self.stack[i].ty;
        // A class is checked, and the value keeps its type.
        let fits = |s: &mut Self| s.match_slot(t, have, &mut Env::default()).is_some();
        if have == t || (self.types.is_class(t) && fits(self)) {
            return Ok(());
        }
        // A union is a type of values too, so a tuple forgets its positions
        // into an array of one before patterns are let through.
        if self.forget_tuple(i, t)? {
            return Ok(());
        }
        if self.types.is_pattern(t) && !self.types.is_class(t) {
            return Ok(());
        }
        if self.forgets(t, have) {
            self.stack[i].ty = t;
            return Ok(());
        }
        if self.types.is_literal(have) {
            let Some(c) = self.conversion(t, have) else {
                return Err(self.err(
                    Kind::Literal,
                    format!(
                        "{} cannot become {}",
                        self.describe(&self.stack[i]),
                        self.types.name(t)
                    ),
                ));
            };
            // A literal is known, so it has no code: it can be converted on
            // top of the stack and put back.
            let j = self.stack.remove(i);
            self.stack.push(j);
            let floor = self.floor;
            self.floor = self.stack.len() - 1;
            let at = self.pos;
            let r = self
                .clause(&c, Env::default())
                .map_err(|e| self.blame(&c, e, at));
            self.pos = at;
            self.floor = floor;
            r?;
            let j = self.stack.pop().expect("a conversion leaves one value");
            assert!(j.known() && j.ty == t, "a conversion folds");
            self.stack.insert(i, j);
            return Ok(());
        }
        Err(self.err(
            Kind::Mismatch,
            format!(
                "expected {}, found {}",
                self.types.name(t),
                self.describe(&self.stack[i])
            ),
        ))
    }

    /// Whether `t` is `have` with the lengths of some vecs forgotten: the
    /// same values, at run time, with a less precise type.
    fn forgets(&self, t: Type, have: Type) -> bool {
        t == have
            || match (self.types.term(t), self.types.term(have)) {
                (Term::Ary(e), Term::Vec(_, f) | Term::Ary(f)) => self.forgets(e, f),
                (Term::Vec(n, e), Term::Vec(m, f)) => n == m && self.forgets(e, f),
                (Term::Map(k, v), Term::Map(k2, v2)) => self.forgets(k, k2) && self.forgets(v, v2),
                (Term::Base(Base::I64), Term::Base(Base::Bool)) => true,
                _ => false,
            }
    }

    /// A bracket in a body: it types the values on top.
    fn annotate_top(&mut self, sig: &Sig) -> Result<()> {
        if !sig.guards.is_empty() {
            return Err(self.err(
                Kind::Syntax,
                "guards and value patterns belong in a signature, first in a quotation",
            ));
        }
        if sig.outs.is_some() {
            return Err(self.err(
                Kind::Syntax,
                "`--` belongs in a signature, first in a quotation",
            ));
        }
        self.need(sig.ins.len())?;
        let Some((_, env)) = self.match_sig(sig) else {
            let want: Vec<_> = sig.ins.iter().map(|&t| self.types.name(t)).collect();
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "expected {}, found {}",
                    want.join(" "),
                    self.top_types(sig.ins.len())
                ),
            ));
        };
        self.settle(sig, &env)
    }

    // ---- Matching ----

    /// Matches a signature's inputs against the values on top: how well, as
    /// a score (TYPES.md 2.8), and what its variables bind.
    fn match_sig(&mut self, sig: &Sig) -> Option<(i32, Env)> {
        let n = sig.ins.len();
        if self.stack.len() < self.floor + n {
            return None;
        }
        let base = self.stack.len() - n;
        let mut env = Env::default();
        let mut score = 0;
        for k in 0..n {
            let have = self.stack[base + k].ty;
            // An untyped value pattern, as `< 0 >`, matches a value of any
            // type its literal becomes.
            let p = sig.ins[k];
            if sig.value(k) && self.types.is_literal(p) {
                let fits = have == p
                    || self.accepts(have, p)
                    || (self.types.is_literal(have) && self.accepts(p, have));
                if !fits {
                    return None;
                }
                score += if have == p { 8 } else { 4 };
                continue;
            }
            score += self.match_slot(sig.ins[k], have, &mut env)?;
        }
        Some((score, env))
    }

    /// A literal matches its own type best, then its default type, then any
    /// type it converts to; anything else must unify.
    fn match_slot(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        if let Term::Or(..) = self.types.term(p) {
            return self.match_union(p, have, env);
        }
        if self.types.is_literal(have) {
            return match self.types.term(p) {
                Term::Var(v) => self.bind(v, have, env),
                Term::Atom => Some(2),
                _ if p == have => Some(8),
                _ if self.accepts(p, have) => Some(if p == Self::default_of(have) { 4 } else { 2 }),
                _ => None,
            };
        }
        self.unify(p, have, env)
    }

    /// A union matches as its best member does, and scores just below it, so
    /// that a clause for that member alone is more specific.
    fn match_union(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        // A value of a union type matches when each of its types does, and
        // as well as the worst.
        if self.is_union(have) {
            let mut worst = i32::MAX;
            for h in self.types.members(have) {
                worst = worst.min(self.match_union(p, h, env)?);
            }
            return Some(worst);
        }
        let mut best: Option<(i32, Env)> = None;
        for m in self.types.members(p) {
            let mut e = env.clone();
            if let Some(s) = self.match_slot(m, have, &mut e)
                && best.as_ref().is_none_or(|b| s > b.0)
            {
                best = Some((s, e));
            }
        }
        let (s, e) = best?;
        *env = e;
        Some(s - 1)
    }

    fn bind(&mut self, v: u8, have: Type, env: &mut Env) -> Option<i32> {
        let v = v as usize;
        match env.0[v] {
            None => {
                env.0[v] = Some(have);
                Some(0)
            }
            Some(b) if b == have => Some(0),
            // A variable bound to a literal takes a type the literal becomes,
            // or the literal's own if that holds the other, as an i64 holds a
            // bool: `3 true. max.` is 3.
            Some(b) if self.types.is_literal(b) && self.accepts(have, b) => {
                let own = Self::default_of(b);
                env.0[v] = Some(if self.forgets(own, have) { own } else { have });
                Some(0)
            }
            Some(b) if self.types.is_literal(have) && self.accepts(b, have) => {
                let own = Self::default_of(have);
                if self.forgets(own, b) {
                    env.0[v] = Some(own);
                }
                Some(0)
            }
            // One is the other less precisely known, a bool an i64: the
            // variable takes the wider.
            Some(b) if self.forgets(have, b) => {
                env.0[v] = Some(have);
                Some(0)
            }
            Some(b) if self.forgets(b, have) => Some(0),
            _ => None,
        }
    }

    /// Scores 8 for each part of the pattern that is not a variable, 4 where
    /// a vec stands for an array, 2 for an atom, and a union 1 less than its
    /// best member.
    fn unify(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        match (self.types.term(p), self.types.term(have)) {
            (Term::Var(v), _) => self.bind(v, have, env),
            (Term::Or(..), _) => self.match_union(p, have, env),
            // An atom: any type at run time that is not a container.
            (Term::Atom, Term::Base(b)) => {
                (!matches!(b, Base::Type | Base::Symbol | Base::Quote)).then_some(2)
            }
            (Term::Atom, Term::Or(..)) => {
                let atoms = self.types.members(have).into_iter().all(|m| {
                    matches!(self.types.term(m),
                        Term::Base(b) if !matches!(b, Base::Type | Base::Symbol | Base::Quote))
                });
                atoms.then_some(2)
            }
            (Term::Tuple(_), Term::Tuple(_)) => {
                let ps = self.types.elements(p).expect("a tuple").to_vec();
                let hs = self.types.elements(have).expect("a tuple").to_vec();
                if ps.len() != hs.len() {
                    return None;
                }
                let mut s = 8;
                for (p, h) in ps.into_iter().zip(hs) {
                    s += self.unify(p, h, env)?;
                }
                Some(s)
            }
            // A vec is a tuple whose types are one.
            (Term::Tuple(_), Term::Vec(n, e)) => {
                let ps = self.types.elements(p).expect("a tuple").to_vec();
                if self.types.term(n) != Term::Nat(ps.len() as u64) {
                    return None;
                }
                let mut s = 8;
                for p in ps {
                    s += self.unify(p, e, env)?;
                }
                Some(s)
            }
            (Term::AnyTuple, Term::Tuple(_)) => Some(4),
            // A tuple where an array is wanted forgets its positions, at a
            // cost: an array of the union of its types (TYPES.md 2.5).
            (Term::Ary(e), Term::Tuple(_)) => {
                let u = self.tuple_union(have);
                Some(1 + self.unify(e, u, env)?)
            }
            // A bool is an i64, 0 or 1: matched a little less well than one.
            (Term::Base(Base::I64), Term::Base(Base::Bool)) => Some(6),
            (Term::Base(a), Term::Base(b)) => (a == b).then_some(8),
            (Term::Nat(a), Term::Nat(b)) => (a == b).then_some(8),
            (Term::Ary(e), Term::Ary(f)) => Some(8 + self.unify(e, f, env)?),
            (Term::Ary(e), Term::Vec(_, f)) => Some(4 + self.unify(e, f, env)?),
            (Term::Vec(n, e), Term::Vec(m, f)) => {
                Some(8 + self.unify(n, m, env)? + self.unify(e, f, env)?)
            }
            (Term::Map(k, v), Term::Map(k2, v2)) => {
                Some(8 + self.unify(k, k2, env)? + self.unify(v, v2, env)?)
            }
            _ => None,
        }
    }

    /// Gives the inputs the types their patterns name, so literals convert.
    /// An untyped value pattern names no type: its input keeps its own.
    fn settle(&mut self, sig: &Sig, env: &Env) -> Result<()> {
        let base = self.stack.len() - sig.ins.len();
        for (k, &p) in sig.ins.iter().enumerate() {
            let t = self.types.subst(p, env);
            let have = self.stack[base + k].ty;
            // A vec given where an array is wanted keeps its length, which
            // the body may use: it is an array, more precisely known.
            if (sig.value(k) && self.types.is_literal(t)) || (have != t && self.forgets(t, have)) {
                continue;
            }
            self.annotate(base + k, t)?;
        }
        Ok(())
    }

    // ---- Families ----

    /// Applies a family (TYPES.md 2.6, 2.8), evaluated where it is applied,
    /// on these judgments. A family applied again to the same types inside
    /// its own application is recursive: the outer application starts over
    /// as a call to an instance, a word of its own for those types, and the
    /// inner one, compiled inside that instance, calls it.
    fn family(&mut self, name: &Rc<str>) -> Result<()> {
        let arity = self.arity(name);
        let reach = arity.min(self.stack.len() - self.floor);
        let types: Vec<Type> = self.stack[self.stack.len() - reach..]
            .iter()
            .map(|j| j.ty)
            .collect();
        if let Some(i) = self
            .frames
            .iter()
            .rposition(|f| f.name == *name && f.ins == types)
        {
            if i + 1 == self.frames.len() {
                return self.recur(arity);
            }
            return Err(self.err(
                Kind::Limit,
                format!(
                    "`{name}` is applied inside an instance it calls: \
                     mutual recursion is not built yet"
                ),
            ));
        }
        let key = (name.clone(), types);
        if let Some(i) = self.active.iter().position(|k| *k == key) {
            return Err(self.err(Kind::Again(i), name.to_string()));
        }
        let at = self.pos;
        let index = self.active.len();
        let saved = (
            self.stack.clone(),
            self.code.len(),
            self.floor,
            self.literals.len(),
            self.effects,
        );
        self.active.push(key);
        let r = self.resolve(name, arity, at, false);
        self.active.truncate(index);
        self.pos = at;
        match r {
            Err(e) if e.kind == Kind::Again(index) => {
                self.stack = saved.0;
                self.code.truncate(saved.1);
                self.floor = saved.2;
                self.literals.truncate(saved.3);
                self.effects = saved.4;
                self.call_instance(name, arity)
            }
            r => r,
        }
    }

    /// How many values a family takes: the most any signature names, or what
    /// its definition takes.
    fn arity(&self, name: &Rc<str>) -> usize {
        self.words[name]
            .clauses
            .iter()
            .filter_map(|c| c.sig.as_ref().map(|s| s.ins.len()))
            .max()
            .or_else(|| self.effect_of(name).map(|e| e.0))
            .unwrap_or(0)
    }

    /// Chooses among a family's clauses and applies the choice. Clauses are
    /// chosen in two phases (TYPES.md 2.15): by types now, keeping those
    /// whose inputs match; then, if some have guards, by their guards at run
    /// time, in the order they were defined, the best clause without guards
    /// last. `top` says this is an instance's own family, whose recursive
    /// alternatives may wait for its results' types.
    fn resolve(&mut self, name: &Rc<str>, arity: usize, at: Pos, top: bool) -> Result<()> {
        let clauses = self.words[name].clauses.clone();
        let mut cands = self.candidates(&clauses);
        if let Some(i) = self.union_input(arity)
            && self.must_split(&clauses, &cands, i)
        {
            return self.split(name, i, arity);
        }
        if cands.is_empty() {
            let fewest = clauses
                .iter()
                .map(|c| c.sig.as_ref().map_or(0, |s| s.ins.len()))
                .min()
                .unwrap_or(0);
            self.need(fewest)?;
            return Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` for {}", self.top_types(arity.max(1))),
            ));
        }
        if cands.iter().any(|c| c.1.guarded()) {
            // Literals settle by the best match, and the clauses match again.
            let (_, c, env) = primary(&cands).expect("not empty");
            self.settle_literals(&c, &env)?;
            if !top {
                // Its key is now the settled types, as an inner application's
                // will be.
                let reach = arity.min(self.stack.len() - self.floor);
                let types = self.stack[self.stack.len() - reach..]
                    .iter()
                    .map(|j| j.ty)
                    .collect();
                self.active.last_mut().expect("this application").1 = types;
            }
            cands = self.candidates(&clauses);
            return self.chain(name, &cands, arity, at, top);
        }
        match self.best(name, &cands, arity)? {
            Some((c, env)) => self.apply_clause(name, &c, env, at),
            None => unreachable!("candidates"),
        }
    }

    /// The clauses whose inputs match the values on top, in the order they
    /// were defined, with their scores and bindings.
    fn candidates(&mut self, clauses: &[Rc<Clause>]) -> Vec<Candidate> {
        clauses
            .iter()
            .filter_map(|c| {
                let m = match &c.sig {
                    None => Some((0, Env::default())),
                    Some(s) => self.match_sig(s),
                };
                m.map(|(score, env)| (score, c.clone(), env))
            })
            .collect()
    }

    /// The best of the clauses given, if any: the highest score; a tie is an
    /// error.
    fn best(
        &self,
        name: &str,
        cands: &[Candidate],
        arity: usize,
    ) -> Result<Option<(Rc<Clause>, Env)>> {
        let Some(top) = cands.iter().map(|c| c.0).max() else {
            return Ok(None);
        };
        let mut best = cands.iter().filter(|c| c.0 == top);
        let (_, c, env) = best.next().expect("the maximum");
        if best.next().is_some() {
            return Err(self.err(
                Kind::Mismatch,
                format!("clauses of `{name}` tie for {}", self.top_types(arity)),
            ));
        }
        Ok(Some((c.clone(), env.clone())))
    }

    /// Converts the literals among a clause's inputs to the types its
    /// signature gives them.
    fn settle_literals(&mut self, c: &Clause, env: &Env) -> Result<()> {
        let Some(sig) = &c.sig else { return Ok(()) };
        let base = self.stack.len() - sig.ins.len();
        for (k, &p) in sig.ins.iter().enumerate() {
            let t = self.types.subst(p, env);
            if self.types.is_literal(self.stack[base + k].ty)
                && !self.types.is_pattern(t)
                && !(sig.value(k) && self.types.is_literal(t))
            {
                self.annotate(base + k, t)?;
            }
        }
        Ok(())
    }

    /// Evaluates a clause of a family, noting the family in its errors.
    fn apply_clause(&mut self, name: &str, c: &Rc<Clause>, env: Env, at: Pos) -> Result<()> {
        let outer = self.site;
        if c.core && outer.is_none() {
            self.site = Some(at);
        }
        let r = self.clause(c, env);
        self.site = outer;
        self.pos = at;
        r.map_err(|e| match c.core {
            true => self.blame(c, e, at),
            false => e.within(name, at),
        })
    }

    /// The choice at run time: for each guarded clause, in order, a test of
    /// its guards and its body; then the best clause without guards, or no
    /// word. Each alternative starts from the same judgments, in code of its
    /// own; where their results' types differ, the result has their union
    /// (TYPES.md 2.9, 3.6). A guard on known
    /// values is decided now, so an alternative may be dropped, or taken for
    /// certain.
    ///
    /// In an instance's own family (`top`), an alternative that reaches the
    /// recursion before its results' types are known waits: once others
    /// have finished, their results type the recursion, a ghost (TYPES.md
    /// 2.7), and it is compiled again.
    fn chain(
        &mut self,
        name: &Rc<str>,
        cands: &[Candidate],
        arity: usize,
        at: Pos,
        top: bool,
    ) -> Result<()> {
        let n = cands
            .iter()
            .filter_map(|c| c.1.sig.as_ref().map(|s| s.ins.len()))
            .max()
            .unwrap_or(0);
        self.need(n)?;
        let snapshot = self.stack.clone();
        let base = snapshot.len() - n;
        // The alternatives in the order they are laid out, and those waiting.
        let mut order: Vec<(Candidate, bool)> = Vec::new();
        let mut alts: Vec<Option<Alt>> = Vec::new();
        let mut waiting = Vec::new();
        let mut certain = false;
        for cand in cands.iter().filter(|c| c.1.guarded()) {
            match self.alternative(name, cand, true, &snapshot, at, top)? {
                Outcome::Dropped => continue,
                Outcome::Taken(alt, sure) => {
                    alts.push(Some(alt));
                    certain = sure;
                }
                Outcome::Waits(sure) => {
                    waiting.push(alts.len());
                    alts.push(None);
                    certain = sure;
                }
            }
            order.push((cand.clone(), true));
            if certain {
                break;
            }
        }
        if !certain {
            let plain: Vec<Candidate> = cands.iter().filter(|c| !c.1.guarded()).cloned().collect();
            self.stack = snapshot.clone();
            if let Some((c, env)) = self.best(name, &plain, arity)? {
                let cand = (0, c, env);
                match self.alternative(name, &cand, false, &snapshot, at, top)? {
                    Outcome::Taken(alt, _) => alts.push(Some(alt)),
                    Outcome::Waits(_) => {
                        waiting.push(alts.len());
                        alts.push(None);
                    }
                    Outcome::Dropped => unreachable!("no guards"),
                }
                order.push((cand, false));
            } else if alts.is_empty() {
                // Said where the family was applied, not where a guard was.
                self.pos = at;
                return Err(self.err(
                    Kind::NoWord,
                    format!(
                        "no word `{name}` for {}: no clause's guard holds",
                        self.top_types(arity)
                    ),
                ));
            } else {
                alts.push(Some(Alt {
                    code: vec![Ins::Op(Op::Lit(1)), Ins::Op(Op::Prim(P::Trap))],
                    stack: None,
                    skip: None,
                }));
            }
        }
        if !waiting.is_empty() {
            let done: Vec<&[Jdg]> = alts
                .iter()
                .flatten()
                .filter_map(|a| a.stack.as_deref())
                .collect();
            if done.is_empty() {
                return Err(Error::new(Kind::Ghost, Some(at), name.to_string()));
            }
            let ghost = self.ghost(&done, base);
            self.frames.last_mut().expect("an instance").ghost = Some(ghost);
            for i in waiting {
                let (cand, guarded) = order[i].clone();
                match self.alternative(name, &cand, guarded, &snapshot, at, top)? {
                    Outcome::Taken(alt, _) => alts[i] = Some(alt),
                    _ => return Err(Error::new(Kind::Ghost, Some(at), name.to_string())),
                }
            }
        }
        // A union its clauses' outputs name is meant, and not warned of.
        let declared = order.iter().all(|(c, _)| {
            c.1.sig
                .as_ref()
                .and_then(|s| s.outs.as_ref())
                .is_some_and(|o| o.iter().any(|&t| self.is_union(t)))
        });
        self.merge(
            alts.into_iter().flatten().collect(),
            &snapshot,
            base,
            !declared,
        )
    }

    /// One alternative of a choice, compiled from the saved judgments into
    /// code of its own: its guards' test, if it has guards, and its body.
    fn alternative(
        &mut self,
        name: &Rc<str>,
        cand: &Candidate,
        guarded: bool,
        snapshot: &[Jdg],
        at: Pos,
        top: bool,
    ) -> Result<Outcome> {
        let (_, c, env) = cand;
        self.stack = snapshot.to_vec();
        let outer = std::mem::take(&mut self.code);
        let skip = self.label();
        let test = if guarded {
            match self.test(c.sig.as_ref().expect("guarded"), skip) {
                Ok(t) => t,
                Err(e) => {
                    self.code = outer;
                    return Err(e.within(name, at));
                }
            }
        } else {
            Some(true)
        };
        if test == Some(false) {
            self.code = outer;
            return Ok(Outcome::Dropped);
        }
        if guarded && test.is_none() {
            self.pin(c.sig.as_ref().expect("guarded"));
        }
        let r = self.apply_clause(name, c, env.clone(), at);
        let code = std::mem::replace(&mut self.code, outer);
        let sure = guarded && test == Some(true);
        match r {
            Err(e) if top && e.kind == Kind::Ghost => Ok(Outcome::Waits(sure)),
            Err(e) => Err(e),
            Ok(()) => Ok(Outcome::Taken(
                Alt {
                    code,
                    stack: Some(self.stack.clone()),
                    skip: test.is_none().then_some(skip),
                },
                sure,
            )),
        }
    }

    /// The types of a recursion's results, from the alternatives that have
    /// finished. A literal result takes an input's type it can become, as
    /// `fact`'s 1 becomes the type of what it multiplies, or its default; if
    /// the guess is wrong, the instance is compiled again with the types it
    /// found.
    fn ghost(&mut self, done: &[&[Jdg]], base: usize) -> Vec<Type> {
        let ins = self.frames.last().expect("an instance").ins.clone();
        let slots = done[0].len() - base;
        (0..slots)
            .map(|k| {
                let tys: Vec<Type> = done
                    .iter()
                    .filter_map(|s| s.get(base + k).map(|j| j.ty))
                    .collect();
                if let Some(&lit) = tys.iter().find(|&&t| self.types.is_literal(t))
                    && tys.iter().all(|&t| self.types.is_literal(t))
                    && let Some(&t) = ins.iter().find(|&&t| self.accepts(t, lit))
                {
                    return t;
                }
                self.join(&tys).unwrap_or(tys[0])
            })
            .collect()
    }

    /// Joins a family's alternatives. One taken for certain is just its code
    /// and judgments. Otherwise every alternative's results take the types
    /// they share and become values at run time, each in its own code, and
    /// the alternatives are laid out with their branches.
    ///
    /// Results whose types differ have their union (TYPES.md 2.9, 3.6): each
    /// alternative's value is tagged with its type, and `warn` says to note
    /// that each use of it will branch.
    fn merge(&mut self, alts: Vec<Alt>, snapshot: &[Jdg], base: usize, warn: bool) -> Result<()> {
        if let [
            Alt {
                skip: None,
                stack: Some(_),
                ..
            },
        ] = &alts[..]
        {
            let alt = alts.into_iter().next().expect("one");
            self.code.extend(alt.code);
            self.stack = alt.stack.expect("returns");
            return Ok(());
        }
        // What each alternative leaves above the values it cannot reach.
        let mut lists: Vec<Vec<Type>> = Vec::new();
        let mut runs = false;
        for alt in &alts {
            let Some(stack) = &alt.stack else { continue };
            let same_below = stack.len() >= base
                && stack[..base]
                    .iter()
                    .zip(&snapshot[..base])
                    .all(|(a, b)| a.ty == b.ty && a.val == b.val);
            if !same_below {
                return Err(self.err(
                    Kind::Mismatch,
                    "the clauses chosen between at run time take different values",
                ));
            }
            runs |= stack[base..].iter().any(|j| j.val == Val::Many);
            lists.push(stack[base..].iter().map(|j| j.ty).collect());
        }
        let varying = runs || lists.windows(2).any(|w| w[0].len() != w[1].len());
        let mut below: Vec<Jdg> = snapshot[..base].to_vec();
        // Inside an array literal, alternatives may leave different numbers
        // of elements: together they are a run, whose count the gather finds.
        let collect = match (varying, self.collecting(base)) {
            (false, _) => None,
            (true, Some(start)) => {
                let all: Vec<Type> = lists.concat();
                let Some(t) = self.join(&all) else {
                    return Err(self.differ(&all));
                };
                // The literal's elements below the run become values at run
                // time first, since nothing can be placed beneath a run.
                self.stack = snapshot.to_vec();
                for i in start..base {
                    self.annotate(i, t)?;
                    self.materialize(i)?;
                }
                below = self.stack[..base].to_vec();
                Some(t)
            }
            (true, None) => {
                return Err(self.err(
                    Kind::Mismatch,
                    "the clauses chosen between at run time leave different numbers of values",
                ));
            }
        };
        let mut joined = Vec::new();
        if collect.is_none() {
            for k in 0..lists.first().map_or(0, Vec::len) {
                let tys: Vec<Type> = lists.iter().map(|l| l[k]).collect();
                let t = self.join_union(&tys);
                if warn && self.is_union(t) && tys.iter().any(|&x| x != t) {
                    self.warn(
                        Level::Informative,
                        format!(
                            "this leaves a value of `{}`, its type chosen at run time, \
                             so each use of it branches",
                            self.types.name(t)
                        ),
                    );
                }
                joined.push(t);
            }
        }
        let end = self.label();
        let last = alts.len() - 1;
        for (k, alt) in alts.into_iter().enumerate() {
            let outer = std::mem::replace(&mut self.code, alt.code);
            let r = match alt.stack {
                Some(mut stack) => {
                    stack.splice(..base, below.iter().cloned());
                    self.stack = stack;
                    match collect {
                        Some(t) => self.settle_run(base, t),
                        None => self.settle_results(base, &joined),
                    }
                }
                None => Ok(()),
            };
            let code = std::mem::replace(&mut self.code, outer);
            r?;
            self.code.extend(code);
            if k != last {
                self.code.push(Ins::Jump(end));
            }
            if let Some(skip) = alt.skip {
                self.code.push(Ins::Label(skip));
            }
        }
        self.code.push(Ins::Label(end));
        self.stack = below;
        match collect {
            Some(t) => self.push(t, Val::Many),
            None => {
                for t in joined {
                    self.push(t, Val::Run);
                }
            }
        }
        Ok(())
    }

    fn differ(&self, tys: &[Type]) -> Error {
        let all: Vec<_> = tys.iter().map(|&t| self.types.name(t)).collect();
        self.err(
            Kind::Mismatch,
            format!(
                "the clauses chosen between at run time leave different types: {}",
                all.join(", ")
            ),
        )
    }

    /// Gives every value an alternative leaves the element type of the run
    /// they join, and makes them values at run time.
    fn settle_run(&mut self, base: usize, t: Type) -> Result<()> {
        for i in base..self.stack.len() {
            self.annotate(i, t)?;
            self.materialize(i)?;
        }
        Ok(())
    }

    /// Where the innermost literal starts, if it is an array literal that
    /// the values from `base` up are elements of: a run may be collected
    /// there.
    fn collecting(&self, base: usize) -> Option<usize> {
        let l = self.literals.last()?;
        (l.bracket == b'(' && l.start <= base).then_some(l.start)
    }

    // ---- Instances and recursion ----

    /// Calls a family's instance for the types of its inputs. The inputs
    /// settle first: literals take the types of the best match, or their
    /// defaults. Inside that very instance, the call is to itself.
    fn call_instance(&mut self, name: &Rc<str>, arity: usize) -> Result<()> {
        self.need(arity)?;
        let clauses = self.words[name].clauses.clone();
        let cands = self.candidates(&clauses);
        if let Some((_, c, env)) = primary(&cands) {
            self.settle_literals(&c, &env)?;
        }
        let base = self.stack.len() - arity;
        for i in base..self.stack.len() {
            match self.stack[i].ty {
                INT_LIT => self.annotate(i, I64)?,
                DEC_LIT => self.annotate(i, F64)?,
                t if self.types.compile_time_only(t) => {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "`{name}` applies itself, so it is compiled as a word of its own, \
                             and its inputs must be values at run time, not {}",
                            self.describe(&self.stack[i])
                        ),
                    ));
                }
                _ => {}
            }
        }
        let ins: Vec<Type> = self.stack[base..].iter().map(|j| j.ty).collect();
        if let Some(i) = self
            .frames
            .iter()
            .rposition(|f| f.name == *name && f.ins == ins)
        {
            if i + 1 == self.frames.len() {
                return self.recur(arity);
            }
            return Err(self.err(
                Kind::Limit,
                format!("`{name}` is applied inside an instance it calls: mutual recursion is not built yet"),
            ));
        }
        let at = self.pos;
        let (cid, outs, effects) = self.instance(name, ins, arity, at)?;
        self.effects |= effects;
        self.pos = at;
        self.materialize_top(arity)?;
        self.emit(Op::Call(cid));
        self.stack.truncate(base);
        for t in outs {
            self.push(t, Val::Run);
        }
        Ok(())
    }

    /// A call inside an instance to itself. Its results have the ghost's
    /// types, if they are known yet.
    fn recur(&mut self, arity: usize) -> Result<()> {
        self.materialize_top(arity)?;
        let f = self.frames.last_mut().expect("an instance");
        let Some(ghost) = f.ghost.clone() else {
            return Err(Error::new(Kind::Ghost, Some(self.pos), f.name.to_string()));
        };
        f.used = true;
        self.emit(Op::Recur);
        self.stack.truncate(self.stack.len() - arity);
        for t in ghost {
            self.push(t, Val::Run);
        }
        Ok(())
    }

    /// Compiles a family for input types as a word of its own, once: the
    /// family resolved on values at run time of those types, guards and all.
    /// Its results' types, for its calls to itself, are those its signature
    /// promises, or else those the alternatives that finish first leave; if
    /// the code it compiles leaves others, it is compiled again with them.
    fn instance(
        &mut self,
        name: &Rc<str>,
        ins: Vec<Type>,
        arity: usize,
        at: Pos,
    ) -> Result<(Cid, Vec<Type>, Effects)> {
        let key = (name.clone(), ins.clone());
        if let Some(r) = self.instances.get(&key) {
            return Ok(r.clone());
        }
        if self.frames.len() >= 16 {
            return Err(self.err(Kind::Limit, "instances nested more than 16 deep"));
        }
        let run: Vec<Jdg> = ins.iter().map(|&ty| Jdg { ty, val: Val::Run }).collect();
        let saved = (
            std::mem::replace(&mut self.stack, run.clone()),
            std::mem::take(&mut self.code),
            std::mem::replace(&mut self.floor, 0),
            std::mem::take(&mut self.literals),
            std::mem::take(&mut self.active),
            std::mem::replace(&mut self.depth, 0),
            std::mem::replace(&mut self.effects, 0),
        );
        let ghost = self.promised(name);
        self.frames.push(Frame {
            name: name.clone(),
            ins: ins.clone(),
            ghost,
            used: false,
        });
        let names: Vec<_> = ins.iter().map(|&t| self.types.name(t)).collect();
        let names = names.join(" ");
        let mut tries = 0;
        let r = loop {
            self.stack = run.clone();
            self.code.clear();
            self.floor = 0;
            self.literals.clear();
            self.active.clear();
            self.effects = 0;
            let r = self.resolve(name, arity, at, true).and_then(|()| {
                for i in 0..self.stack.len() {
                    self.materialize(i)?;
                }
                Ok(self.stack.iter().map(|j| j.ty).collect::<Vec<_>>())
            });
            match r {
                Ok(outs) => {
                    let f = self.frames.last_mut().expect("this instance");
                    if f.used && f.ghost.as_ref() != Some(&outs) {
                        if tries == 3 {
                            break Err(Error::new(
                                Kind::Mismatch,
                                Some(at),
                                format!(
                                    "the results of `{name}` on {names} do not settle: \
                                     give its signature outputs after `--`"
                                ),
                            ));
                        }
                        tries += 1;
                        f.ghost = Some(outs);
                        f.used = false;
                        continue;
                    }
                    break Ok(outs);
                }
                Err(e) if e.kind == Kind::Ghost => {
                    break Err(Error::new(
                        Kind::Mismatch,
                        Some(at),
                        format!(
                            "`{name}` applies itself on {names}, and none of its clauses for \
                             them finishes without doing so, so its results have no types: \
                             give its signature outputs after `--`"
                        ),
                    ));
                }
                Err(e) => break Err(e),
            }
        };
        let code = std::mem::take(&mut self.code);
        let effects = self.effects;
        self.frames.pop();
        (
            self.stack,
            self.code,
            self.floor,
            self.literals,
            self.active,
            self.depth,
            self.effects,
        ) = saved;
        let outs = r?;
        let (_, cid) = self.seal(code);
        self.instances.insert(key, (cid, outs.clone(), effects));
        Ok((cid, outs, effects))
    }

    /// The outputs a family's signature promises for the values on top, if
    /// the clause that matches best says.
    fn promised(&mut self, name: &Rc<str>) -> Option<Vec<Type>> {
        let clauses = self.words[name].clauses.clone();
        let cands = self.candidates(&clauses);
        let (_, c, env) = primary(&cands)?;
        let outs = c.sig.as_ref()?.outs.clone()?;
        let outs: Vec<Type> = outs.iter().map(|&t| self.types.subst(t, &env)).collect();
        outs.iter()
            .all(|&t| !self.types.is_pattern(t))
            .then_some(outs)
    }

    /// Seals code as the machine takes it: labels become positions, string
    /// literals data objects, a call followed only by a return a tail call,
    /// and a return ends it. The blobs are kept for the session to publish.
    pub fn seal(&mut self, code: Vec<Ins>) -> (Vec<Op>, Cid) {
        let mut at = HashMap::new();
        let mut n = 0u32;
        for ins in &code {
            match ins {
                Ins::Label(l) => {
                    at.insert(*l, n);
                }
                Ins::Str(_) => n += 2,
                _ => n += 1,
            }
        }
        let mut ops = Vec::new();
        for ins in code {
            match ins {
                Ins::Op(op) => ops.push(op),
                Ins::Str(s) => {
                    let blob = Blob::Data(s.as_bytes().to_vec());
                    ops.push(Op::Data(blob.cid()));
                    ops.push(Op::Prim(P::Text));
                    self.blobs.push(blob);
                }
                Ins::Label(_) => {}
                Ins::Jump(l) => ops.push(Op::Branch(at[&l])),
                Ins::JumpZero(l) => ops.push(Op::ZeroBranch(at[&l])),
            }
        }
        ops.push(Op::Return);
        for i in 0..ops.len() {
            if returns(&ops, i + 1) {
                match &ops[i] {
                    Op::Call(c) => ops[i] = Op::Tail(*c),
                    Op::Recur => ops[i] = Op::TailRecur,
                    _ => {}
                }
            }
        }
        let blob = Blob::Code(crate::code::encode(&ops));
        let cid = blob.cid();
        self.blobs.push(blob);
        (ops, cid)
    }

    /// Gives an alternative's results the types the alternatives share, and
    /// makes them values at run time.
    fn settle_results(&mut self, base: usize, joined: &[Type]) -> Result<()> {
        for (i, &t) in joined.iter().enumerate() {
            if self.is_union(t) {
                self.inject(base + i, t)?;
            } else {
                self.annotate(base + i, t)?;
            }
        }
        self.materialize_top(joined.len())
    }

    // ---- Unions ----

    fn is_union(&self, t: Type) -> bool {
        matches!(self.types.term(t), Term::Or(..))
    }

    /// The deepest of the values a word of this arity takes whose type is a
    /// union, if any.
    fn union_input(&self, arity: usize) -> Option<usize> {
        let reach = arity.min(self.stack.len() - self.floor);
        (self.stack.len() - reach..self.stack.len()).find(|&i| self.is_union(self.stack[i].ty))
    }

    /// Whether a family is applied to each type of the union at `i` apart:
    /// when no clause takes the union whole, or some type of it has a
    /// better clause of its own, as a string has `>string`'s.
    fn must_split(&mut self, clauses: &[Rc<Clause>], whole: &[Candidate], i: usize) -> bool {
        let Some((_, best, _)) = primary(whole) else {
            return true;
        };
        let u = self.stack[i].ty;
        let mut apart = false;
        for m in self.types.members(u) {
            self.stack[i].ty = m;
            let cands = self.candidates(clauses);
            apart = !primary(&cands).is_some_and(|c| Rc::ptr_eq(&c.1, &best));
            if apart {
                break;
            }
        }
        self.stack[i].ty = u;
        apart
    }

    /// Applies a word to each type of the union at `i`, in code of its own,
    /// chosen by the value's tag at run time, the last with no test; the
    /// results are merged (TYPES.md 3.6). Inside each, the value is untagged:
    /// it has that type.
    fn split(&mut self, name: &Rc<str>, i: usize, arity: usize) -> Result<()> {
        let snapshot = self.stack.clone();
        let base = snapshot.len() - arity.min(snapshot.len() - self.floor);
        let u = snapshot[i].ty;
        let members = self.types.members(u);
        let at = self.pos;
        let mut alts = Vec::new();
        for (k, &m) in members.iter().enumerate() {
            self.stack = snapshot.clone();
            let outer = std::mem::take(&mut self.code);
            let skip = (k + 1 < members.len()).then(|| self.label());
            let r = self.split_arm(name, i, m, skip);
            self.pos = at;
            let code = std::mem::replace(&mut self.code, outer);
            r.map_err(|mut e| {
                e.trace.push(format!(
                    "where the value of `{}` is {}",
                    self.types.name(u),
                    self.types.name(m)
                ));
                e
            })?;
            alts.push(Alt {
                code,
                stack: Some(self.stack.clone()),
                skip,
            });
        }
        self.merge(alts, &snapshot, base, false)
    }

    fn split_arm(&mut self, name: &Rc<str>, i: usize, m: Type, skip: Option<u32>) -> Result<()> {
        if let Some(skip) = skip {
            self.copy(i)?;
            self.stack.pop();
            self.emit(Op::Prim(P::UnionTag));
            self.emit(Op::Lit(m as u64));
            self.emit(Op::Prim(P::Eq));
            self.code.push(Ins::JumpZero(skip));
        }
        self.in_place(i, &[Op::Prim(P::UnionValue)], m)?;
        self.apply_word(name)
    }

    /// Makes the value at `i` a value of the union `u`, which has its type:
    /// tagged with that type, unless it is a union already, whose tags are
    /// its types' own, so it is one of `u`'s as it is.
    fn inject(&mut self, i: usize, u: Type) -> Result<()> {
        let have = self.stack[i].ty;
        if have == u {
            return Ok(());
        }
        if self.is_union(have) {
            self.stack[i].ty = u;
            return Ok(());
        }
        // A literal takes its default type.
        self.materialize(i)?;
        let t = self.stack[i].ty;
        let Some(m) = self
            .types
            .members(u)
            .into_iter()
            .find(|&m| m == t || self.forgets(m, t))
        else {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "expected {}, found {}",
                    self.types.name(u),
                    self.types.name(t)
                ),
            ));
        };
        self.in_place(i, &[Op::Lit(m as u64), Op::Prim(P::UnionMake)], u)
    }

    /// Emits `ops` on the value at `i` where it lies, which leaves a value
    /// of type `ty` there: brought to the top, if it is not there, and put
    /// back.
    fn in_place(&mut self, i: usize, ops: &[Op], ty: Type) -> Result<()> {
        self.materialize(i)?;
        let n = self.stack.len() - i;
        let up: Vec<usize> = (1..n).chain([0]).collect();
        self.permute(i, &up);
        for op in ops {
            self.emit(op.clone());
        }
        self.stack.last_mut().expect("brought up").ty = ty;
        let down: Vec<usize> = std::iter::once(n - 1).chain(0..n - 1).collect();
        self.permute(i, &down);
        Ok(())
    }

    /// The union of a tuple's types.
    fn tuple_union(&mut self, t: Type) -> Type {
        let es = self.types.elements(t).expect("a tuple").to_vec();
        es.into_iter()
            .reduce(|a, b| self.types.union(a, b))
            .expect("a tuple has types")
    }

    /// Where an array is wanted, a tuple at `i` forgets its positions: its
    /// elements, each tagged with its type, gathered into an array of their
    /// union (TYPES.md 2.5). Whether it did.
    fn forget_tuple(&mut self, i: usize, t: Type) -> Result<bool> {
        let have = self.stack[i].ty;
        let (Term::Ary(e), Some(es)) = (self.types.term(t), self.types.elements(have)) else {
            return Ok(false);
        };
        let es = es.to_vec();
        let u = self.tuple_union(have);
        let fits = e == u
            || (self.is_union(e)
                && self
                    .types
                    .members(u)
                    .iter()
                    .all(|m| self.types.members(e).contains(m)));
        if !fits {
            return Ok(false);
        }
        let mut ops = vec![Op::Prim(P::ScratchPush), Op::Prim(P::Mark)];
        for (k, &x) in es.iter().enumerate() {
            ops.extend([
                Op::Lit(0),
                Op::Prim(P::ScratchAt),
                Op::Lit(k as u64),
                Op::Prim(P::VecAt),
            ]);
            if !self.is_union(x) {
                ops.extend([Op::Lit(x as u64), Op::Prim(P::UnionMake)]);
            }
        }
        ops.extend([
            Op::Prim(P::Gather),
            Op::Prim(P::ScratchPop),
            Op::Prim(P::Drop),
        ]);
        self.in_place(i, &ops, t)?;
        self.warn(
            Level::Informative,
            format!(
                "this makes an array of `{}` from a tuple: each element is tagged, \
                 and each use of one branches",
                self.types.name(e)
            ),
        );
        Ok(true)
    }

    /// The type values of these types share, as `join` finds it, or else
    /// their union, literals taking their defaults (TYPES.md 2.9).
    fn join_union(&mut self, tys: &[Type]) -> Type {
        if let Some(t) = self.join(tys)
            && tys.iter().all(|&x| {
                x == t || self.forgets(t, x) || (self.types.is_literal(x) && self.accepts(t, x))
            })
        {
            return t;
        }
        let mut u: Option<Type> = None;
        for &x in tys {
            let x = if self.types.is_literal(x) {
                Self::default_of(x)
            } else {
                x
            };
            u = Some(match u {
                None => x,
                Some(y) => self.types.union(y, x),
            });
        }
        u.expect("a type")
    }

    fn warn(&mut self, level: Level, msg: String) {
        if self.core {
            return;
        }
        let w = Warning {
            level,
            pos: Some(self.site.unwrap_or(self.pos)),
            msg,
        };
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// Tests a clause's guards on copies of its inputs: `Some(true)` if they
    /// hold now, `Some(false)` if one fails now, and `None` if some are
    /// decided at run time, each branching to `skip` when it fails.
    fn test(&mut self, sig: &Sig, skip: u32) -> Result<Option<bool>> {
        let base = self.stack.len() - sig.ins.len();
        let mut certain = true;
        for g in &sig.guards {
            let depth = self.stack.len();
            for k in g.slot..g.slot + g.arity {
                self.copy(base + k)?;
            }
            let floor = std::mem::replace(&mut self.floor, depth);
            let outer = std::mem::replace(&mut self.effects, 0);
            self.pos = g.pos;
            let r = match &g.test {
                Test::Quote(q) => self.enter().and_then(|()| {
                    let r = self.run(q);
                    self.depth -= 1;
                    r
                }),
                // A value pattern is the input `eq?` the value.
                Test::Equals(v) => {
                    self.stack.push(v.clone());
                    self.apply_word(&"eq?".into())
                }
            };
            let found = std::mem::replace(&mut self.effects, outer);
            self.effects |= found;
            self.floor = floor;
            r?;
            self.pos = g.pos;
            // A guard may read, which makes it a choice at run time, but
            // must not write (Thomas, 2026-10-08).
            if found & prims::WRITES != 0 {
                return Err(self.err(
                    Kind::Effect,
                    format!(
                        "{} {}, and a guard must not write",
                        g.name(),
                        prims::describe_effects(found)
                    ),
                ));
            }
            if self.stack.len() != depth + 1 {
                return Err(self.err(Kind::Mismatch, format!("{} must leave one flag", g.name())));
            }
            // A guard leaves a bool (Thomas, 2026-10-09), or a literal 0 or 1:
            // a count is not a truth.
            let flag = self.stack.pop().expect("one");
            let crisp = flag.ty == BOOL || flag.ty == INT_LIT;
            match flag.val {
                Val::Int(0) if crisp => return Ok(Some(false)),
                Val::Int(1) if crisp => {}
                Val::Run if flag.ty == BOOL => {
                    self.code.push(Ins::JumpZero(skip));
                    certain = false;
                }
                _ => {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("{} leaves {}, not a bool", g.name(), self.describe(&flag)),
                    ));
                }
            }
        }
        Ok(certain.then_some(true))
    }

    /// Reorders the judgments from `base`: output k is input `order[k]`.
    /// Known values move for nothing; the values at run time among them,
    /// if their order changes, go to the scratch stack, the deepest ending
    /// on top, and are copied back in the new order.
    fn permute(&mut self, base: usize, order: &[usize]) {
        let present: Vec<usize> = (0..order.len())
            .filter(|&k| !self.stack[base + k].known())
            .collect();
        let after: Vec<usize> = order
            .iter()
            .copied()
            .filter(|k| present.contains(k))
            .collect();
        if after != present {
            for _ in 0..present.len() {
                self.emit(Op::Prim(P::ScratchPush));
            }
            for k in &after {
                let depth = present.iter().position(|p| p == k).expect("present");
                self.emit(Op::Lit(depth as u64));
                self.emit(Op::Prim(P::ScratchAt));
            }
            for _ in 0..present.len() {
                self.emit(Op::Prim(P::ScratchPop));
                self.emit(Op::Prim(P::Drop));
            }
        }
        let moved: Vec<Jdg> = order
            .iter()
            .map(|&k| self.stack[base + k].clone())
            .collect();
        self.stack.truncate(base);
        self.stack.extend(moved);
    }

    /// Inside a clause chosen by a value pattern tested at run time, its
    /// input is known to equal the value: pinned to it, as the input's type.
    fn pin(&mut self, sig: &Sig) {
        let base = self.stack.len() - sig.ins.len();
        for g in &sig.guards {
            let Test::Equals(v) = &g.test else { continue };
            let i = base + g.slot;
            if self.stack[i].val != Val::Run {
                continue;
            }
            let ty = self.stack[i].ty;
            self.stack.push(v.clone());
            let top = self.stack.len() - 1;
            let r = self.annotate(top, ty);
            let v = self.stack.pop().expect("pushed");
            if r.is_ok() && v.known() {
                self.stack[i].val = Val::Pinned(Box::new(v.val));
            }
        }
    }

    /// Pushes a copy of the judgment at `i`: a known value is copied as it
    /// is, and a value at run time by copying it on the machine's stack.
    fn copy(&mut self, i: usize) -> Result<()> {
        let j = self.stack[i].clone();
        if let Some(k) = j.known_copy() {
            self.stack.push(k);
            return Ok(());
        }
        let above = self.stack[i + 1..].iter().filter(|j| !j.known()).count();
        match above {
            0 => self.emit(Op::Prim(P::Dup)),
            1 => self.emit(Op::Prim(P::Over)),
            n => {
                self.emit(Op::Lit(n as u64));
                self.emit(Op::Prim(P::Pick));
            }
        }
        self.stack.push(j);
        Ok(())
    }

    // ---- Stack effects ----

    /// How many values a word takes and leaves, worked out from its
    /// definition: what a guard looks at. Clauses of a family share one
    /// effect, so the first whose effect is known gives it.
    fn effect_of(&self, name: &Rc<str>) -> Option<(usize, usize)> {
        self.word_effect(name, &mut Vec::new())
    }

    fn word_effect(&self, name: &Rc<str>, visiting: &mut Vec<Rc<str>>) -> Option<(usize, usize)> {
        let w = self.words.get(name)?;
        if let Some((p, sig)) = &w.prim {
            return Some(match p {
                Prim::Dup => (1, 2),
                Prim::Drop => (1, 0),
                Prim::Swap => (2, 2),
                Prim::Over => (2, 3),
                Prim::Rot => (3, 3),
                Prim::Swap2 => (4, 4),
                Prim::VecConcat | Prim::Map | Prim::Compose => (2, 1),
                Prim::Each | Prim::EachRight => (2, 0),
                Prim::Within => (2, 1),
                Prim::AryInsert | Prim::Zip | Prim::Table => (3, 1),
                Prim::AryRemove => (2, 1),
                Prim::VecSpread | Prim::TupleSpread => return None,
                Prim::Range | Prim::Reverse => (1, 1),
                Prim::Def => (2, 0),
                _ => (sig.ins.len(), sig.outs.as_ref().map_or(0, Vec::len)),
            });
        }
        // A type applied is the type, and `nil.` its value: one value left.
        if w.ty.is_some() {
            return Some((0, 1));
        }
        if let Some(c) = w.con {
            return Some(if c == Con::Ary { (1, 1) } else { (2, 1) });
        }
        if visiting.contains(name) {
            return None;
        }
        visiting.push(name.clone());
        let r = w
            .clauses
            .iter()
            .find_map(|c| self.clause_effect(c, visiting));
        visiting.pop();
        r
    }

    fn clause_effect(&self, c: &Clause, visiting: &mut Vec<Rc<str>>) -> Option<(usize, usize)> {
        match &c.sig {
            Some(Sig {
                ins,
                outs: Some(outs),
                ..
            }) => Some((ins.len(), outs.len())),
            Some(sig) => {
                let (i, o) = self.effect(&c.body, visiting)?;
                let n = sig.ins.len();
                Some(if i <= n { (n, n - i + o) } else { (i, o) })
            }
            None => self.effect(&c.body, visiting),
        }
    }

    /// The effect of tokens of the explicit form.
    fn effect(&self, toks: &[Token], visiting: &mut Vec<Rc<str>>) -> Option<(usize, usize)> {
        let mut ins = 0;
        let mut items: Vec<Item> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            match &toks[i].tok {
                Tok::Int(_) | Tok::Dec(..) | Tok::Str(_) => items.push(Item::Other),
                Tok::Template(pieces) => {
                    take(&mut items, &mut ins, template_pulls(pieces).len());
                    items.push(Item::Other);
                }
                Tok::Name(s) => items.push(Item::Name(s.clone())),
                Tok::Open(b'[') => {
                    let j = matching(toks, i);
                    items.push(Item::Quote(i + 1, j));
                    i = j;
                }
                Tok::Open(b'<') => {
                    let j = matching(toks, i);
                    let n = self.bracket_size(&toks[i + 1..j]);
                    take(&mut items, &mut ins, n);
                    items.extend((0..n).map(|_| Item::Other));
                    i = j;
                }
                Tok::Open(_) => {
                    let j = matching(toks, i);
                    if self.effect(&toks[i + 1..j], visiting)?.0 != 0 {
                        return None;
                    }
                    items.push(Item::Other);
                    i = j;
                }
                Tok::Apply => {
                    let (a, b) = match items.pop()? {
                        Item::Name(s) => self.word_effect(&s, visiting)?,
                        Item::Quote(from, to) => self.effect(&toks[from..to], visiting)?,
                        Item::Other => return None,
                    };
                    take(&mut items, &mut ins, a);
                    items.extend((0..b).map(|_| Item::Other));
                }
                _ => return None,
            }
            i += 1;
        }
        Some((ins, items.len()))
    }

    /// How many types a bracket in a body leaves: its code's values, each
    /// word applied taking and leaving what its effect says.
    fn bracket_size(&self, toks: &[Token]) -> usize {
        let mut n: isize = 0;
        let mut i = 0;
        while i < toks.len() {
            match &toks[i].tok {
                Tok::Int(_) | Tok::Dec(..) | Tok::Str(_) | Tok::Name(_) => n += 1,
                Tok::Apply => {
                    if let Some(Tok::Name(s)) = i.checked_sub(1).map(|k| &toks[k].tok)
                        && let Some((a, b)) = self.word_effect(s, &mut Vec::new())
                    {
                        n += b as isize - a as isize - 1;
                    }
                }
                Tok::Open(b'[') => i = matching(toks, i),
                Tok::Open(_) => {
                    i = matching(toks, i);
                    n += 1;
                }
                Tok::Dashes => break,
                _ => {}
            }
            i += 1;
        }
        n.max(0) as usize
    }

    /// An error from inside a core clause, as an error where it was applied.
    fn blame(&self, c: &Clause, mut e: Error, at: Pos) -> Error {
        if c.core && e.trace.is_empty() {
            e.pos = Some(at);
        }
        e
    }

    /// Evaluates a clause on the values on top. A signature settles its
    /// inputs, keeps its body to them, and checks the outputs it promises.
    fn clause(&mut self, c: &Clause, env: Env) -> Result<()> {
        self.enter()?;
        let r = self.clause_body(c, env);
        self.depth -= 1;
        r
    }

    fn clause_body(&mut self, c: &Clause, env: Env) -> Result<()> {
        let Some(sig) = &c.sig else {
            return self.run(&c.body);
        };
        self.settle(sig, &env)?;
        let base = self.stack.len() - sig.ins.len();
        let floor = self.floor;
        self.floor = base;
        let r = self.run(&c.body);
        self.floor = floor;
        r?;
        if let Some(outs) = &sig.outs {
            let left = self.stack.len() - base;
            if left != outs.len() {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "leaves {left} values, and its signature says {}",
                        outs.len()
                    ),
                ));
            }
            for (k, &o) in outs.iter().enumerate() {
                let t = self.types.subst(o, &env);
                self.annotate(base + k, t)?;
            }
        }
        Ok(())
    }

    /// Adds a clause to a family. A clause with the same inputs as one
    /// already there replaces it. A clause whose signature types all its
    /// inputs is compiled once now, on values of those types, so that its
    /// errors show where it is defined; if it fails, the family is left as
    /// it was.
    fn define(&mut self, name: &Rc<str>, clause: Clause) -> Result<()> {
        // Instances compiled before may apply the family changed.
        self.instances.clear();
        let ins = clause.sig.as_ref().map(|s| s.ins.clone());
        let c = Rc::new(clause);
        let context = c.sig.as_ref().map(Sig::context);
        let w = self.words.entry(name.clone()).or_default();
        let before = w.clauses.clone();
        match w
            .clauses
            .iter()
            .position(|old| old.sig.as_ref().map(Sig::context) == context)
        {
            Some(i) => w.clauses[i] = c.clone(),
            None => w.clauses.push(c.clone()),
        }
        let typed = ins.is_some_and(|ins| {
            ins.iter()
                .all(|&t| !self.types.is_pattern(t) && !self.types.compile_time_only(t))
        });
        if typed && let Err(e) = self.check(name, &c) {
            self.word(name).clauses = before;
            return Err(e);
        }
        Ok(())
    }

    /// Compiles a clause on values at run time of its signature's types, and
    /// throws the code away.
    fn check(&mut self, name: &Rc<str>, c: &Clause) -> Result<()> {
        let ins = &c.sig.as_ref().expect("typed").ins;
        let stack = ins.iter().map(|&ty| Jdg { ty, val: Val::Run }).collect();
        let saved = (
            std::mem::replace(&mut self.stack, stack),
            std::mem::take(&mut self.code),
            std::mem::replace(&mut self.floor, 0),
            std::mem::take(&mut self.literals),
            self.pos,
            self.effects,
        );
        let sig = c.sig.as_ref().expect("typed");
        let mut r = Ok(None);
        if !sig.guards.is_empty() {
            let skip = self.label();
            r = self.test(sig, skip);
            self.stack.truncate(ins.len());
        }
        let mut r = r.and_then(|_| self.clause(c, Env::default()));
        for i in 0..self.stack.len() {
            if r.is_ok() {
                r = self.materialize(i);
            }
        }
        (
            self.stack,
            self.code,
            self.floor,
            self.literals,
            self.pos,
            self.effects,
        ) = saved;
        r.map_err(|e| {
            let mut e = e;
            e.trace.push(format!("in `{name}`, defined at {}", c.at));
            e
        })
    }

    // ---- Types as values ----

    /// A bracket, `< … >`, is code, a comprehension of types (Thomas,
    /// 2026-10-09). Its words mean what they mean anywhere: a type applied is
    /// the type, `string.`; a constructor applied builds one, `a ary.`; a
    /// name left bare is a symbol. Its one rule of its own: a word of one
    /// letter is a type variable. What the code leaves is the signature, by
    /// what each value is: a type, inputs of that type; a value, a literal,
    /// a symbol or `true.`, inputs equal to it; a quotation, a guard, which
    /// tests the inputs before it. `--` divides the inputs from the outputs,
    /// which are types.
    pub fn bracket(&mut self, toks: &[Token]) -> Result<Sig> {
        let dashes = toks.iter().position(|t| t.tok == Tok::Dashes);
        let (inputs, outputs) = match dashes {
            Some(d) => (&toks[..d], Some(&toks[d + 1..])),
            None => (toks, None),
        };
        let mut ins = Vec::new();
        let mut equals = Vec::new();
        let mut guards = Vec::new();
        for (j, pos) in self.comprehend(inputs)? {
            match j.val {
                Val::Type(t) => ins.push(t),
                // `nil.`, the one value of its type, is that type.
                _ if j.ty == NIL => ins.push(NIL),
                Val::Quote(q) => guards.push(self.quote_guard(q, ins.len(), pos)?),
                _ => {
                    equals.push(Guard {
                        test: Test::Equals(j.clone()),
                        slot: ins.len(),
                        arity: 1,
                        pos,
                    });
                    ins.push(j.ty);
                }
            }
        }
        let outs = match outputs {
            None => None,
            Some(o) => {
                let mut outs = Vec::new();
                for (j, pos) in self.comprehend(o)? {
                    match j.val {
                        Val::Type(t) => outs.push(t),
                        _ if j.ty == NIL => outs.push(NIL),
                        _ => {
                            self.pos = pos;
                            return Err(self.err(
                                Kind::Mismatch,
                                format!(
                                    "an output is a type, and {} is not one",
                                    self.describe(&j)
                                ),
                            ));
                        }
                    }
                }
                Some(outs)
            }
        };
        equals.extend(guards);
        Ok(Sig {
            ins,
            guards: equals,
            outs,
        })
    }

    /// Runs a bracket's code, on a stack of its own, and gives what it
    /// leaves, each value with where it was written. It runs while
    /// compiling, so it may leave no code: its values are known.
    fn comprehend(&mut self, toks: &[Token]) -> Result<Vec<(Jdg, Pos)>> {
        let at = toks.first().map_or(self.pos, |t| t.pos);
        let saved = (
            std::mem::take(&mut self.stack),
            std::mem::take(&mut self.code),
            std::mem::replace(&mut self.floor, 0),
            std::mem::take(&mut self.literals),
            std::mem::replace(&mut self.effects, 0),
        );
        self.in_bracket += 1;
        let r = self.run(toks);
        self.in_bracket -= 1;
        let (stack, code, effects) = (
            std::mem::take(&mut self.stack),
            std::mem::take(&mut self.code),
            self.effects,
        );
        (
            self.stack,
            self.code,
            self.floor,
            self.literals,
            self.effects,
        ) = saved;
        r?;
        if !code.is_empty() || effects != 0 || stack.iter().any(|j| !j.known()) {
            self.pos = at;
            return Err(self.err(
                Kind::Mismatch,
                "a bracket's code runs while compiling, so its values must be known then",
            ));
        }
        Ok(stack
            .into_iter()
            .map(|j| {
                let pos = match &j.val {
                    Val::Name(_, p) => *p,
                    Val::Quote(q) => q.first().map_or(at, |t| t.pos),
                    _ => at,
                };
                (j, pos)
            })
            .collect())
    }

    /// A quotation left in a bracket: a guard, testing the inputs before it,
    /// as many as it takes, and leaving one flag.
    fn quote_guard(&self, q: Rc<[Token]>, before: usize, pos: Pos) -> Result<Guard> {
        let text = source_text(&q);
        let Some((ins, outs)) = self.effect(&q, &mut Vec::new()) else {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!("`[ {text} ]` cannot be a guard: how many values it takes is not known"),
            ));
        };
        if outs != 1 {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!(
                    "`[ {text} ]` leaves {}, and a guard leaves one flag",
                    values(outs)
                ),
            ));
        }
        if ins > before {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!(
                    "the guard `[ {text} ]` looks at {}, and {} before it",
                    values(ins),
                    values(before)
                ),
            ));
        }
        Ok(Guard {
            test: Test::Quote(q),
            slot: before - ins,
            arity: ins,
            pos,
        })
    }

    /// Applies a constructor with `.`: `100 i64 vec .` is a type.
    fn construct(&mut self, c: Con) -> Result<()> {
        let t = match c {
            Con::Ary => {
                let e = self.pop_type()?;
                Term::Ary(e)
            }
            Con::Vec => {
                let e = self.pop_type()?;
                let j = self.pop()?;
                if let Val::Type(v) = j.val
                    && matches!(self.types.term(v), Term::Var(_))
                {
                    let t = self.types.intern(Term::Vec(v, e));
                    self.push(TYPE, Val::Type(t));
                    return Ok(());
                }
                let n = match j.val {
                    Val::Int(n) if n >= 0 && (j.ty == INT_LIT || j.ty == I64) => n as u64,
                    _ => {
                        return Err(self.err(
                            Kind::Mismatch,
                            format!(
                                "`vec` needs a length known at compile time, not {}",
                                self.describe(&j)
                            ),
                        ));
                    }
                };
                Term::Vec(self.types.intern(Term::Nat(n)), e)
            }
            Con::Map => {
                let v = self.pop_type()?;
                let k = self.pop_type()?;
                Term::Map(k, v)
            }
            Con::Or => {
                let b = self.pop_type()?;
                let a = self.pop_type()?;
                let t = self.types.union(a, b);
                self.push(TYPE, Val::Type(t));
                return Ok(());
            }
        };
        let t = self.types.intern(t);
        self.push(TYPE, Val::Type(t));
        Ok(())
    }

    /// `def` on a type: the name is a name for it, as `i64 f64 or. num def.`
    /// names a class, in a bracket or applied to a value.
    fn name_type(&mut self, name: &Rc<str>, t: Type) -> Result<()> {
        let w = self.word(name);
        if w.ty.is_some() || w.con.is_some() {
            return Err(self.err(Kind::Mismatch, format!("`{name}` names a type already")));
        }
        w.ty = Some(t);
        self.types.name_class(t, name);
        self.instances.clear();
        Ok(())
    }

    fn pop_type(&mut self) -> Result<Type> {
        let j = self.pop()?;
        match &j.val {
            Val::Type(t) => Ok(*t),
            _ if j.ty == NIL => Ok(NIL),
            Val::Name(n, _) if self.words.get(n).is_some_and(|w| w.ty.is_some()) => {
                Ok(self.words[n].ty.expect("checked"))
            }
            _ => Err(self.err(
                Kind::Mismatch,
                format!("expected a type, found {}", self.describe(&j)),
            )),
        }
    }

    // ---- Code ----

    /// Makes the judgment at `i` a value at run time, if it is not: its
    /// value is emitted, then moved under the values at run time above it,
    /// by `swap`, `rot rot`, or the scratch stack. A literal takes its
    /// default type.
    fn materialize(&mut self, i: usize) -> Result<()> {
        if !self.stack[i].known() {
            return Ok(());
        }
        match self.stack[i].ty {
            INT_LIT => self.annotate(i, I64)?,
            DEC_LIT => self.annotate(i, F64)?,
            _ => {}
        }
        let ins = match &self.stack[i].val {
            Val::Int(n) => Ins::Op(Op::Lit(*n as i64 as u64)),
            Val::Float(x) => Ins::Op(Op::Float(float_bits(*x))),
            Val::Str(s) => Ins::Str(s.clone()),
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "{} is left unapplied: it has no value at run time",
                        self.describe(&self.stack[i])
                    ),
                ));
            }
        };
        if self.stack[i + 1..].iter().any(|j| j.val == Val::Many) {
            return Err(self.err(
                Kind::Mismatch,
                "a value cannot be placed beneath a run of values in an array literal",
            ));
        }
        let above = self.stack[i + 1..].iter().filter(|j| !j.known()).count();
        match above {
            0 => self.code.push(ins),
            1 => {
                self.code.push(ins);
                self.emit(Op::Prim(P::Swap));
            }
            2 => {
                self.code.push(ins);
                self.emit(Op::Prim(P::Rot));
                self.emit(Op::Prim(P::Rot));
            }
            n => {
                for _ in 0..n {
                    self.emit(Op::Prim(P::ScratchPush));
                }
                self.code.push(ins);
                for _ in 0..n {
                    self.emit(Op::Prim(P::ScratchPop));
                }
            }
        }
        self.stack[i].val = Val::Run;
        Ok(())
    }

    fn materialize_top(&mut self, n: usize) -> Result<()> {
        let base = self.stack.len() - n;
        for i in base..self.stack.len() {
            self.materialize(i)?;
        }
        Ok(())
    }

    /// Emits operations that take the top `n` values and leave values of the
    /// types `outs`.
    fn emit_ops(&mut self, n: usize, ops: &[P], outs: &[Type]) -> Result<()> {
        self.materialize_top(n)?;
        for &op in ops {
            self.emit(Op::Prim(op));
        }
        self.stack.truncate(self.stack.len() - n);
        for &ty in outs {
            self.push(ty, Val::Run);
        }
        Ok(())
    }

    // ---- Primitives ----

    fn prim(&mut self, name: &str, p: Prim, sig: &Sig) -> Result<()> {
        let n = sig.ins.len();
        self.need(n)?;
        let Some((_, env)) = self.match_sig(sig) else {
            if let Some(i) = self.union_input(n) {
                return self.split(&name.into(), i, n);
            }
            let want: Vec<_> = sig.ins.iter().map(|&t| self.types.name(t)).collect();
            return Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `{name}` for {}: it takes {}",
                    self.top_types(n),
                    want.join(" ")
                ),
            ));
        };
        self.settle(sig, &env)?;
        let base = self.stack.len() - n;
        let len = |s: &mut Self, v: u8| match env.0[v as usize].map(|t| s.types.term(t)) {
            Some(Term::Nat(k)) => k,
            _ => unreachable!("a vec's length is bound"),
        };
        match p {
            Prim::Dup => {
                let j = self.stack[base].clone();
                match j.known_copy() {
                    Some(k) => self.stack.push(k),
                    None => {
                        self.emit(Op::Prim(P::Dup));
                        self.stack.push(j);
                    }
                }
            }
            Prim::Drop => {
                if !self.stack[base].known() {
                    self.emit(Op::Prim(P::Drop));
                }
                self.stack.pop();
            }
            Prim::Swap => {
                if !self.stack[base].known() && !self.stack[base + 1].known() {
                    self.emit(Op::Prim(P::Swap));
                }
                self.stack.swap(base, base + 1);
            }
            Prim::Over => {
                let j = self.stack[base].clone();
                match j.known_copy() {
                    Some(k) => self.stack.push(k),
                    None => {
                        let top_runs = !self.stack[base + 1].known();
                        self.emit(Op::Prim(if top_runs { P::Over } else { P::Dup }));
                        self.stack.push(j);
                    }
                }
            }
            Prim::Rot => {
                let runs: Vec<bool> = self.stack[base..].iter().map(|j| !j.known()).collect();
                match runs[..] {
                    [true, true, true] => self.emit(Op::Prim(P::Rot)),
                    [true, true, false] | [true, false, true] => self.emit(Op::Prim(P::Swap)),
                    _ => {}
                }
                let a = self.stack.remove(base);
                self.stack.push(a);
            }
            Prim::Swap2 => self.permute(base, &[2, 3, 0, 1]),
            Prim::VecLength => {
                let k = len(self, 13);
                if !self.stack[base].known() {
                    self.emit(Op::Prim(P::Drop));
                }
                self.stack.pop();
                self.push(I64, Val::Int(k.into()));
            }
            Prim::VecAt => {
                let k = len(self, 13);
                let e = env.0[0].expect("bound");
                self.at(p, base, e, Some(k))?;
            }
            Prim::AryAt => {
                let e = env.0[0].expect("bound");
                self.at(p, base, e, None)?;
            }
            Prim::StrAt => self.at(p, base, I64, None)?,
            Prim::ArySlice => {
                let e = env.0[0].expect("bound");
                let t = self.types.intern(Term::Ary(e));
                self.slice(p, base, t)?;
            }
            Prim::StrSlice => self.slice(p, base, STRING)?,
            Prim::VecConcat => {
                let k = len(self, 13) + len(self, 12);
                let e = env.0[0].expect("bound");
                let k = self.types.intern(Term::Nat(k));
                let t = self.types.intern(Term::Vec(k, e));
                self.emit_ops(2, &[P::Concat], &[t])?;
            }
            Prim::Map => self.map(name)?,
            Prim::Each => self.each(name, false)?,
            Prim::EachRight => self.each(name, true)?,
            Prim::Range => {
                let ty = match self.stack[base].val {
                    Val::Int(k) if k < 0 => {
                        return Err(self.err(Kind::Mismatch, format!("a range of {k} values")));
                    }
                    Val::Int(k) => {
                        let n = self.types.intern(Term::Nat(k as u64));
                        self.types.intern(Term::Vec(n, I64))
                    }
                    _ => self.types.intern(Term::Ary(I64)),
                };
                self.emit_ops(1, &[P::Range], &[ty])?;
            }
            Prim::Reverse => {
                let mut t = self.stack[base].ty;
                if let Some(es) = self.types.elements(t) {
                    let es = es.iter().rev().copied().collect();
                    t = self.types.tuple(es);
                } else if !matches!(self.types.term(t), Term::Ary(_) | Term::Vec(..)) {
                    return Err(self.err(
                        Kind::NoWord,
                        format!("no word `{name}` for {}", self.types.name(t)),
                    ));
                }
                self.emit_ops(1, &[P::Reverse], &[t])?;
            }
            Prim::Within => self.within(name)?,
            Prim::AryInsert => self.insert(name, base)?,
            Prim::Zip => self.zip(name)?,
            Prim::Table => self.table(name)?,
            Prim::AryRemove => self.remove(name, base)?,
            Prim::TupleLength => {
                let n = self
                    .types
                    .elements(self.stack[base].ty)
                    .expect("a tuple")
                    .len();
                if !self.stack[base].known() {
                    self.emit(Op::Prim(P::Drop));
                }
                self.stack.pop();
                self.push(I64, Val::Int(n as i128));
            }
            Prim::TupleAt => self.tuple_at(base)?,
            Prim::Parse => self.parse(name, base)?,
            Prim::To => self.to(name, base)?,
            // A file's text read while compiling, a known string, so what
            // follows folds on it: macros by staging. The text goes into the
            // code as data, so the code's identity follows the file.
            Prim::Embed => {
                let Val::Str(path) = self.stack[base].val.clone() else {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("`{name}` reads while compiling, so its file must be known then"),
                    ));
                };
                let text = std::fs::read_to_string(&*path)
                    .map_err(|e| self.err(Kind::Io, format!("cannot embed {path:?}: {e}")))?;
                self.stack.pop();
                self.push(STRING, Val::Str(text.into()));
            }
            // An empty array or map of a type: `string i64 map. empty.`.
            Prim::Empty => {
                let Val::Type(t) = self.stack[base].val else {
                    unreachable!("a type value")
                };
                let op = match self.types.term(t) {
                    Term::Ary(_) => P::Gather,
                    Term::Map(..) => P::MapGather,
                    _ => {
                        return Err(self.err(
                            Kind::NoWord,
                            format!("no word `{name}` for {}", self.types.name(t)),
                        ));
                    }
                };
                if self.types.is_pattern(t) {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "`{name}` needs a type, not the pattern {}",
                            self.types.name(t)
                        ),
                    ));
                }
                self.stack.pop();
                self.emit(Op::Prim(P::Mark));
                self.emit(Op::Prim(op));
                self.push(t, Val::Run);
            }
            Prim::TupleSpread => {
                let es = self
                    .types
                    .elements(self.stack[base].ty)
                    .expect("a tuple")
                    .to_vec();
                self.materialize(base)?;
                self.stack.pop();
                self.emit(Op::Prim(P::ScratchPush));
                for (k, &e) in es.iter().enumerate() {
                    self.emit(Op::Lit(0));
                    self.emit(Op::Prim(P::ScratchAt));
                    self.emit(Op::Lit(k as u64));
                    self.emit(Op::Prim(P::VecAt));
                    self.push(e, Val::Run);
                }
                self.emit(Op::Prim(P::ScratchPop));
                self.emit(Op::Prim(P::Drop));
            }
            Prim::VecSpread => {
                let n = len(self, 13);
                let e = env.0[0].expect("bound");
                self.materialize(base)?;
                self.stack.pop();
                self.emit(Op::Prim(P::ScratchPush));
                for i in 0..n {
                    self.emit(Op::Lit(0));
                    self.emit(Op::Prim(P::ScratchAt));
                    self.emit(Op::Lit(i));
                    self.emit(Op::Prim(P::VecAt));
                }
                self.emit(Op::Prim(P::ScratchPop));
                self.emit(Op::Prim(P::Drop));
                for _ in 0..n {
                    self.push(e, Val::Run);
                }
            }
            Prim::Compose => {
                let r = self.words_of(name)?;
                let q = self.words_of(name)?;
                let words: Vec<Token> = q.iter().chain(r.iter()).cloned().collect();
                self.push(QUOTE, Val::Quote(words.into()));
            }
            Prim::Def => {
                let name = self.stack.pop().expect("matched");
                let quote = self.stack.pop().expect("matched");
                let typed = match &quote.val {
                    Val::Type(t) => Some(*t),
                    Val::Name(n, _) => self.words.get(n).and_then(|w| w.ty),
                    _ => None,
                };
                if let (Val::Name(name, _), Some(t)) = (&name.val, typed) {
                    return self.name_type(name, t);
                }
                let (Val::Name(name, _), Val::Quote(q)) = (name.val, quote.val) else {
                    return Err(self.err(
                        Kind::Mismatch,
                        "`def` takes a quotation or a type, and a name",
                    ));
                };
                let at = q.first().map_or(self.pos, |t| t.pos);
                let (sig, body) = match q.first().map(|t| &t.tok) {
                    Some(Tok::Open(b'<')) => {
                        let j = matching(&q, 0);
                        let sig = self.bracket(&q[1..j])?;
                        (Some(sig), q[j + 1..].into())
                    }
                    _ => (None, q.clone()),
                };
                let core = self.core;
                self.define(
                    &name,
                    Clause {
                        sig,
                        body,
                        at,
                        core,
                    },
                )?;
            }
            _ => {
                let outs: Vec<Type> = sig
                    .outs
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&t| self.types.subst(t, &env))
                    .collect();
                let known = self.stack[base..]
                    .iter()
                    .all(|j| j.known() || matches!(j.val, Val::Pinned(_)));
                if known && prims::foldable(p) {
                    // A pinned input's cell goes: its value is known. They
                    // are the only cells among the inputs, so on top.
                    for j in &self.stack[base..] {
                        if matches!(j.val, Val::Pinned(_)) {
                            self.code.push(Ins::Op(Op::Prim(P::Drop)));
                        }
                    }
                    let args: Vec<Val> = self
                        .stack
                        .drain(base..)
                        .map(|j| match j.val {
                            Val::Pinned(v) => *v,
                            v => v,
                        })
                        .collect();
                    let vals = prims::fold(p, &args).map_err(|(k, m)| self.err(k, m))?;
                    for (val, ty) in vals.into_iter().zip(outs) {
                        self.push(ty, val);
                    }
                } else if prims::compile_time_only(p) {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("`{name}` needs values known at compile time"),
                    ));
                } else {
                    let ops = PRIMS.iter().find(|d| d.prim == p).expect("listed").ops;
                    self.emit_ops(n, ops, &outs)?;
                    self.effects |= prims::effects(p);
                }
            }
        }
        Ok(())
    }

    /// `map`: an array and a quotation. The quotation is compiled once, on
    /// a value of the element type at run time, as the body of a loop over
    /// the array; it may read the values below the element but must leave
    /// them as they were, and leave one value. The loop's state, the array,
    /// the new array and an index, is on the scratch stack. A vec maps to a
    /// vec as long.
    fn map(&mut self, name: &str) -> Result<()> {
        let q = self.words_of(name)?;
        let base = self.stack.len() - 1;
        if let Some(es) = self.types.elements(self.stack[base].ty) {
            let es = es.to_vec();
            return self.map_tuple(name, &q, es);
        }
        let (len, e) = match self.types.term(self.stack[base].ty) {
            Term::Ary(e) => (None, e),
            Term::Vec(n, e) => (Some(n), e),
            _ => {
                return Err(self.err(
                    Kind::NoWord,
                    format!("no word `{name}` for {} quote", self.top_types(1)),
                ));
            }
        };
        self.materialize(base)?;
        self.stack.pop();
        let (top, end) = (self.label(), self.label());
        for op in [P::ScratchPush, P::Mark, P::Gather, P::ScratchPush] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Label(top));
        self.emit(Op::Prim(P::ScratchPeek));
        self.emit(Op::Lit(2));
        for op in [P::ScratchAt, P::VecLen, P::ILt] {
            self.emit(Op::Prim(op));
        }
        self.code.push(Ins::JumpZero(end));
        self.emit(Op::Lit(2));
        for op in [P::ScratchAt, P::ScratchPeek, P::VecAt] {
            self.emit(Op::Prim(op));
        }
        let below = self.stack.clone();
        self.push(e, Val::Run);
        self.enter()?;
        let r = self.run(&q);
        self.depth -= 1;
        r?;
        let kept = self.stack.len() == below.len() + 1
            && self.stack[..below.len()]
                .iter()
                .zip(&below)
                .all(|(a, b)| a.ty == b.ty && a.val == b.val);
        if !kept {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`{name}`'s quotation must take its element and leave one value, \
                     keeping the values below"
                ),
            ));
        }
        self.materialize(below.len())?;
        let r = self.stack.pop().expect("one").ty;
        self.emit(Op::Lit(1));
        for op in [P::ScratchAt, P::Swap, P::VecPush, P::Drop, P::ScratchPop] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::IAdd));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Jump(top));
        self.code.push(Ins::Label(end));
        for op in [
            P::ScratchPop,
            P::Drop,
            P::ScratchPop,
            P::ScratchPop,
            P::Drop,
        ] {
            self.emit(Op::Prim(op));
        }
        let out = match len {
            Some(n) => Term::Vec(n, r),
            None => Term::Ary(r),
        };
        let out = self.types.intern(out);
        self.push(out, Val::Run);
        Ok(())
    }

    /// `each` and `each-right`: an array and a quotation, applied to each
    /// element in turn, from the first or from the last. The quotation takes
    /// its element and may change the values below it, as an accumulator, but
    /// leaves as many as there were, of the same types, since the next turn
    /// starts from them (the loop's invariant). The loop's state, the array
    /// and an index, is on the scratch stack.
    fn each(&mut self, name: &str, backward: bool) -> Result<()> {
        let q = self.words_of(name)?;
        let base = self.stack.len() - 1;
        if let Some(es) = self.types.elements(self.stack[base].ty) {
            let es = es.to_vec();
            return self.each_tuple(&q, es, backward);
        }
        let e = match self.types.term(self.stack[base].ty) {
            Term::Ary(e) | Term::Vec(_, e) => e,
            _ => {
                return Err(self.err(
                    Kind::NoWord,
                    format!("no word `{name}` for {} quote", self.top_types(1)),
                ));
            }
        };
        self.materialize(base)?;
        let collect = self.invariant(base, e, &q, name)?;
        self.stack.pop();
        let (top, end) = (self.label(), self.label());
        self.emit(Op::Prim(P::ScratchPush));
        if backward {
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Prim(P::VecLen));
        } else {
            self.emit(Op::Lit(0));
        }
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Label(top));
        if backward {
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchPeek));
            self.emit(Op::Prim(P::ILt));
            self.code.push(Ins::JumpZero(end));
            self.emit(Op::Prim(P::ScratchPop));
            self.emit(Op::Lit(1));
            self.emit(Op::Prim(P::ISub));
            self.emit(Op::Prim(P::ScratchPush));
        } else {
            self.emit(Op::Prim(P::ScratchPeek));
            self.emit(Op::Lit(1));
            for op in [P::ScratchAt, P::VecLen, P::ILt] {
                self.emit(Op::Prim(op));
            }
            self.code.push(Ins::JumpZero(end));
        }
        self.emit(Op::Lit(1));
        for op in [P::ScratchAt, P::ScratchPeek, P::VecAt] {
            self.emit(Op::Prim(op));
        }
        let below = self.stack.clone();
        self.push(e, Val::Run);
        self.enter()?;
        let r = self.run(&q);
        self.depth -= 1;
        r?;
        if let Some(t) = collect {
            // Collecting: the values it leaves above the loop's are elements,
            // left on the machine's stack for the literal to gather.
            if self.stack.len() < below.len() || self.stack[..below.len()] != below[..] {
                return Err(self.loop_shape(name));
            }
            self.settle_run(below.len(), t)?;
            self.stack.truncate(below.len());
        } else {
            // What the body leaves takes the loop's shape again.
            if self.stack.len() != below.len() {
                return Err(self.loop_shape(name));
            }
            for (i, b) in below.iter().enumerate() {
                if self.stack[i] != *b {
                    self.annotate(i, b.ty)?;
                    self.materialize(i)?;
                }
            }
            if self.stack != below {
                return Err(self.loop_shape(name));
            }
        }
        if !backward {
            self.emit(Op::Prim(P::ScratchPop));
            self.emit(Op::Lit(1));
            self.emit(Op::Prim(P::IAdd));
            self.emit(Op::Prim(P::ScratchPush));
        }
        self.code.push(Ins::Jump(top));
        self.code.push(Ins::Label(end));
        for op in [P::ScratchPop, P::Drop, P::ScratchPop, P::Drop] {
            self.emit(Op::Prim(op));
        }
        if let Some(t) = collect {
            self.push(t, Val::Many);
        }
        Ok(())
    }

    /// `map` on a tuple: the quotation is compiled for each position, on its
    /// type, so each result keeps its own: a tuple, or a vec when the
    /// results are one type. The tuple and the new array are on the scratch
    /// stack.
    fn map_tuple(&mut self, name: &str, q: &[Token], es: Vec<Type>) -> Result<()> {
        let base = self.stack.len() - 1;
        self.materialize(base)?;
        self.stack.pop();
        for op in [P::ScratchPush, P::Mark, P::Gather, P::ScratchPush] {
            self.emit(Op::Prim(op));
        }
        let mut outs = Vec::new();
        for (k, &e) in es.iter().enumerate() {
            self.emit(Op::Lit(1));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Lit(k as u64));
            self.emit(Op::Prim(P::VecAt));
            let below = self.stack.clone();
            self.push(e, Val::Run);
            self.enter()?;
            let r = self.run(q);
            self.depth -= 1;
            r?;
            let kept = self.stack.len() == below.len() + 1
                && self.stack[..below.len()]
                    .iter()
                    .zip(&below)
                    .all(|(a, b)| a.ty == b.ty && a.val == b.val);
            if !kept {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`{name}`'s quotation must take its element and leave one value, \
                         keeping the values below"
                    ),
                ));
            }
            self.materialize(below.len())?;
            outs.push(self.stack.pop().expect("one").ty);
            self.emit(Op::Lit(0));
            for op in [P::ScratchAt, P::Swap, P::VecPush, P::Drop] {
                self.emit(Op::Prim(op));
            }
        }
        for op in [P::ScratchPop, P::ScratchPop, P::Drop] {
            self.emit(Op::Prim(op));
        }
        let t = self.types.tuple(outs);
        self.push(t, Val::Run);
        Ok(())
    }

    /// `each` on a tuple, unrolled: the quotation is compiled for each
    /// position in turn, on its type, so the values it threads may change
    /// type from one to the next. The tuple is on the scratch stack.
    fn each_tuple(&mut self, q: &[Token], es: Vec<Type>, backward: bool) -> Result<()> {
        let base = self.stack.len() - 1;
        self.materialize(base)?;
        self.stack.pop();
        self.emit(Op::Prim(P::ScratchPush));
        let mut order: Vec<usize> = (0..es.len()).collect();
        if backward {
            order.reverse();
        }
        for k in order {
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Lit(k as u64));
            self.emit(Op::Prim(P::VecAt));
            self.push(es[k], Val::Run);
            self.enter()?;
            let r = self.run(q);
            self.depth -= 1;
            r?;
        }
        self.emit(Op::Prim(P::ScratchPop));
        self.emit(Op::Prim(P::Drop));
        Ok(())
    }

    /// `to`: the value below as the type on top, `2 i64.to.`, a literal
    /// converted by the type's clause for it, a value of another type by
    /// the type's clause for that, as `"data.csv" file.to.` makes a file;
    /// otherwise the value must have the type already, or forget into it.
    fn to(&mut self, name: &str, base: usize) -> Result<()> {
        let j = self.stack[base + 1].clone();
        let t = match &j.val {
            Val::Type(t) => *t,
            Val::Name(n, _) if self.words.get(n).is_some_and(|w| w.ty.is_some()) => {
                self.words[n].ty.expect("checked")
            }
            _ if j.ty == NIL => NIL,
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`{name}` converts to a type, and {} is not one",
                        self.describe(&j)
                    ),
                ));
            }
        };
        self.stack.pop();
        let have = self.stack[base].ty;
        if have != t
            && !self.types.is_literal(have)
            && let Some(c) = self.conversion(t, have)
        {
            let at = self.pos;
            return self.apply_clause(name, &c, Env::default(), at);
        }
        self.annotate(base, t)
    }

    /// `parse`: the value of a type that a string writes, `"42" i64 parse.`,
    /// or nil: `i64 nil or`, the value tagged with its type at run time. A
    /// known string is parsed now, and is the value or nil itself.
    fn parse(&mut self, name: &str, base: usize) -> Result<()> {
        let t = match &self.stack[base + 1].val {
            Val::Type(t) => Some(*t),
            Val::Name(n, _) => self.words.get(n).and_then(|w| w.ty),
            _ => None,
        };
        let (t, op) = match t {
            Some(I64) => (I64, P::ParseInt),
            Some(F64) => (F64, P::ParseFloat),
            _ => {
                return Err(self.err(
                    Kind::NoWord,
                    format!("no word `{name}` for {}", self.top_types(2)),
                ));
            }
        };
        if let Val::Str(s) = &self.stack[base].val {
            let s = s.trim();
            let v = if t == I64 {
                s.parse::<i64>().ok().map(|n| Val::Int(n.into()))
            } else {
                s.parse::<f64>().ok().map(Val::Float)
            };
            self.stack.truncate(base);
            match v {
                Some(v) => self.push(t, v),
                None => self.push(NIL, Val::Int(0)),
            }
            return Ok(());
        }
        self.stack.pop();
        self.materialize(base)?;
        self.stack.pop();
        let (fail, end) = (self.label(), self.label());
        self.emit(Op::Prim(op));
        self.code.push(Ins::JumpZero(fail));
        self.emit(Op::Lit(t as u64));
        self.emit(Op::Prim(P::UnionMake));
        self.code.push(Ins::Jump(end));
        self.code.push(Ins::Label(fail));
        self.emit(Op::Prim(P::Drop));
        self.emit(Op::Lit(0));
        self.emit(Op::Lit(NIL as u64));
        self.emit(Op::Prim(P::UnionMake));
        self.code.push(Ins::Label(end));
        let u = self.types.union(t, NIL);
        self.push(u, Val::Run);
        Ok(())
    }

    /// `at` on a tuple. With the index known, the element has its
    /// position's type; otherwise the tuple forgets its positions, and the
    /// element is of their union.
    fn tuple_at(&mut self, base: usize) -> Result<()> {
        let t = self.stack[base].ty;
        let es = self.types.elements(t).expect("a tuple").to_vec();
        if let Val::Int(k) = self.stack[base + 1].val {
            let i = self
                .known_element(base, k, Some(es.len() as u64))?
                .expect("the length is known");
            self.stack[base + 1].val = Val::Int(i as i128);
            return self.emit_ops(2, &[P::VecAt], &[es[i]]);
        }
        let u = self.tuple_union(t);
        let ary = self.types.intern(Term::Ary(u));
        self.annotate(base, ary)?;
        self.at(Prim::AryAt, base, u, None)
    }

    /// `within`: an array and a quotation, run with the array's last
    /// elements as its stack, as many as it takes, and what it leaves put in
    /// their place: Joy's `infra`, on the end of an array. `( 1 2 3 ) [ ~. ]
    /// within.` is `( 1 3 2 )`, `[ 4 ] within.` appends, `[ drop. ] within.`
    /// drops the last. Only those elements are loaded and the rest is kept by
    /// a slice, so it costs what the quotation does, not the array's length.
    fn within(&mut self, name: &str) -> Result<()> {
        let q = self.words_of(name)?;
        let base = self.stack.len() - 1;
        let (len, e) = match self.types.term(self.stack[base].ty) {
            Term::Ary(e) => (None, e),
            Term::Vec(n, e) => match self.types.term(n) {
                Term::Nat(n) => (Some(n), e),
                _ => (None, e),
            },
            _ => {
                return Err(self.err(
                    Kind::NoWord,
                    format!("no word `{name}` for {} quote", self.top_types(1)),
                ));
            }
        };
        let Some((k, _)) = self.effect(&q, &mut Vec::new()) else {
            return Err(self.err(
                Kind::Mismatch,
                format!("`{name}` needs to know how many values its quotation takes"),
            ));
        };
        if let Some(n) = len
            && (k as u64) > n
        {
            return Err(self.err(
                Kind::Mismatch,
                format!("the quotation takes {}, and the array has {n}", values(k)),
            ));
        }
        self.materialize(base)?;
        self.stack.pop();
        self.emit(Op::Prim(P::ScratchPush));
        // An array of any length is checked to have enough.
        if len.is_none() && k > 0 {
            let ok = self.label();
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Prim(P::VecLen));
            self.emit(Op::Lit(k as u64));
            self.emit(Op::Prim(P::ILt));
            self.code.push(Ins::JumpZero(ok));
            self.emit(Op::Lit(2));
            self.emit(Op::Prim(P::Trap));
            self.code.push(Ins::Label(ok));
        }
        // The last k elements, above a mark, are the quotation's stack.
        self.emit(Op::Prim(P::Mark));
        for i in 0..k {
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Lit(0));
            self.emit(Op::Prim(P::ScratchAt));
            self.emit(Op::Prim(P::VecLen));
            self.emit(Op::Lit((k - i) as u64));
            self.emit(Op::Prim(P::ISub));
            self.emit(Op::Prim(P::VecAt));
        }
        let depth = self.stack.len();
        let floor = std::mem::replace(&mut self.floor, depth);
        for _ in 0..k {
            self.push(e, Val::Run);
        }
        let r = self.enter().and_then(|()| {
            let r = self.run(&q);
            self.depth -= 1;
            r
        });
        self.floor = floor;
        r?;
        // What it leaves are the array's new last elements.
        let m = self.stack.len() - depth;
        for i in depth..self.stack.len() {
            self.annotate(i, e)?;
            self.materialize(i)?;
        }
        self.stack.truncate(depth);
        self.emit(Op::Prim(P::Gather));
        // The rest of the array, then the two joined.
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchAt));
        self.emit(Op::Lit(0));
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchAt));
        self.emit(Op::Prim(P::VecLen));
        self.emit(Op::Lit(k as u64));
        self.emit(Op::Prim(P::ISub));
        for op in [P::Slice, P::Swap, P::Concat, P::ScratchPop, P::Drop] {
            self.emit(Op::Prim(op));
        }
        let ty = match len {
            Some(n) => {
                let n = self.types.intern(Term::Nat(n - k as u64 + m as u64));
                Term::Vec(n, e)
            }
            None => Term::Ary(e),
        };
        let ty = self.types.intern(ty);
        self.push(ty, Val::Run);
        Ok(())
    }

    /// Folds a primitive on the values from `base`, if they are all known
    /// and it may be folded; says whether it did.
    fn fold_now(&mut self, p: Prim, base: usize, outs: &[Type]) -> Result<bool> {
        if !(prims::foldable(p) && self.stack[base..].iter().all(Jdg::known)) {
            return Ok(false);
        }
        let args: Vec<Val> = self.stack.drain(base..).map(|j| j.val).collect();
        let vals = prims::fold(p, &args).map_err(|(k, m)| self.err(k, m))?;
        for (val, &ty) in vals.into_iter().zip(outs) {
            self.push(ty, val);
        }
        Ok(true)
    }

    /// `at` on an array or a string. Elements count from 1, and from -1 at
    /// the end (Thomas, 2026-10-09). A known index is settled now, and
    /// checked against a length known now; one known only at run time is
    /// settled by `element_index`.
    fn at(&mut self, p: Prim, base: usize, out: Type, len: Option<u64>) -> Result<()> {
        if self.fold_now(p, base, &[out])? {
            return Ok(());
        }
        match self.stack[base + 1].val {
            Val::Int(k) => {
                let i = self.known_element(base, k, len)?;
                match i {
                    Some(i) => {
                        self.stack[base + 1].val = Val::Int(i as i128);
                        return self.emit_ops(2, &[P::VecAt], &[out]);
                    }
                    // From the end of an array of any length.
                    None => {
                        self.materialize(base)?;
                        self.emit(Op::Prim(P::Dup));
                        self.emit(Op::Prim(P::VecLen));
                        self.emit(Op::Lit(k as i64 as u64));
                        self.emit(Op::Prim(P::IAdd));
                        self.emit(Op::Prim(P::VecAt));
                    }
                }
            }
            _ => {
                self.materialize_top(2)?;
                self.element_index(1);
                self.emit(Op::Prim(P::VecAt));
            }
        }
        self.stack.truncate(base);
        self.push(out, Val::Run);
        Ok(())
    }

    /// A known element index, from 0, if it can be known now: always for
    /// one from the start, and from the end when the length is known.
    fn known_element(&self, base: usize, k: i128, len: Option<u64>) -> Result<Option<usize>> {
        let outside = || {
            self.err(
                Kind::Mismatch,
                format!(
                    "there is no element {k} in a {}{}",
                    self.types.name(self.stack[base].ty),
                    if k == 0 {
                        ": elements count from 1"
                    } else {
                        ""
                    }
                ),
            )
        };
        if k == 0 {
            return Err(outside());
        }
        match len {
            Some(n) => prims::element(k, n as i128).map(Some).ok_or_else(outside),
            None if k > 0 => Ok(Some((k - 1) as usize)),
            None => Ok(None),
        }
    }

    /// `slice` on an array or a string: elements `i` to `j`, both included,
    /// either counting from the end when negative, so `2 -2 slice` drops the
    /// first and the last.
    fn slice(&mut self, p: Prim, base: usize, out: Type) -> Result<()> {
        if self.fold_now(p, base, &[out])? {
            return Ok(());
        }
        let (i, j) = (&self.stack[base + 1].val, &self.stack[base + 2].val);
        if let (Val::Int(i), Val::Int(j)) = (i, j)
            && *i > 0
            && *j >= 0
        {
            let (i, j) = (*i, *j);
            self.stack[base + 1].val = Val::Int(i - 1);
            self.stack[base + 2].val = Val::Int(j);
            return self.emit_ops(3, &[P::Slice], &[out]);
        }
        if self.stack[base + 1].val == Val::Int(0) {
            return Err(self.err(
                Kind::Mismatch,
                "there is no element 0: elements count from 1",
            ));
        }
        self.materialize_top(3)?;
        self.emit(Op::Prim(P::ScratchPush));
        self.element_index(1);
        self.emit(Op::Prim(P::ScratchPop));
        self.gap_index(2);
        self.emit(Op::Prim(P::Slice));
        self.stack.truncate(base);
        self.push(out, Val::Run);
        Ok(())
    }

    /// Code that turns the element index on top into one from 0: k-1 for
    /// k from the start, n+k from the end, n the length of the array
    /// `depth` below it.
    fn element_index(&mut self, depth: u64) {
        let (start, done) = (self.label(), self.label());
        self.emit(Op::Prim(P::Dup));
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ILt));
        self.code.push(Ins::JumpZero(start));
        self.emit(Op::Lit(depth));
        self.emit(Op::Prim(P::Pick));
        self.emit(Op::Prim(P::VecLen));
        self.emit(Op::Prim(P::IAdd));
        self.code.push(Ins::Jump(done));
        self.code.push(Ins::Label(start));
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::ISub));
        self.code.push(Ins::Label(done));
    }

    /// Code that turns the gap index on top into one from 0: k for k from
    /// the start, n+k+1 from the end, the gap after element k.
    fn gap_index(&mut self, depth: u64) {
        let done = self.label();
        self.emit(Op::Prim(P::Dup));
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ILt));
        self.code.push(Ins::JumpZero(done));
        self.emit(Op::Lit(depth));
        self.emit(Op::Prim(P::Pick));
        self.emit(Op::Prim(P::VecLen));
        self.emit(Op::Prim(P::IAdd));
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::IAdd));
        self.code.push(Ins::Label(done));
    }

    /// The length a vec's type knows, and its element type, for an array.
    fn array_of(&self, t: Type) -> Option<(Option<u64>, Type)> {
        match self.types.term(t) {
            Term::Ary(e) => Some((None, e)),
            Term::Vec(n, e) => Some((
                match self.types.term(n) {
                    Term::Nat(n) => Some(n),
                    _ => None,
                },
                e,
            )),
            _ => None,
        }
    }

    /// Compiles a pair loop's body once, on an element of each array: it
    /// may read the values below but must leave them, and leave one value,
    /// a value at run time; its type is returned.
    fn pair_body(&mut self, name: &str, q: &[Token], a: Type, b: Type) -> Result<Type> {
        let below = self.stack.clone();
        self.push(a, Val::Run);
        self.push(b, Val::Run);
        self.enter()?;
        let r = self.run(q);
        self.depth -= 1;
        r?;
        let kept = self.stack.len() == below.len() + 1
            && self.stack[..below.len()]
                .iter()
                .zip(&below)
                .all(|(x, y)| x.ty == y.ty && x.val == y.val);
        if !kept {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`{name}`'s quotation must take its two elements and leave one value, \
                     keeping the values below"
                ),
            ));
        }
        self.materialize(below.len())?;
        Ok(self.stack.pop().expect("one").ty)
    }

    /// The two arrays on top, for a pair loop: each's known length and
    /// element type.
    fn two_arrays(&self, name: &str) -> Result<[(Option<u64>, Type); 2]> {
        let n = self.stack.len();
        match (
            self.array_of(self.stack[n - 2].ty),
            self.array_of(self.stack[n - 1].ty),
        ) {
            (Some(a), Some(b)) => Ok([a, b]),
            _ => Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` for {} quote", self.top_types(2)),
            )),
        }
    }

    /// `xs ys q zip`: q applied to each pair of elements, in step, and
    /// collected. The arrays must be as long as each other: checked at
    /// compile time when both lengths are known, at run time otherwise.
    /// Arithmetic on two arrays is `zip` (core/core.march).
    fn zip(&mut self, name: &str) -> Result<()> {
        let q = self.words_of(name)?;
        let [(la, ea), (lb, eb)] = self.two_arrays(name)?;
        if let (Some(a), Some(b)) = (la, lb)
            && a != b
        {
            return Err(self.err(
                Kind::Mismatch,
                format!("`{name}` pairs arrays of {a} and {b} elements: they must be as long"),
            ));
        }
        let base = self.stack.len() - 2;
        self.materialize_top(2)?;
        if la.is_none() || lb.is_none() {
            let (bad, ok) = (self.label(), self.label());
            for op in [P::Over, P::VecLen, P::Over, P::VecLen, P::Eq] {
                self.emit(Op::Prim(op));
            }
            self.code.push(Ins::JumpZero(bad));
            self.code.push(Ins::Jump(ok));
            self.code.push(Ins::Label(bad));
            self.emit(Op::Lit(3));
            self.emit(Op::Prim(P::Trap));
            self.code.push(Ins::Label(ok));
        }
        self.stack.truncate(base);
        // The scratch stack: ys, xs, the new array, the index.
        for op in [
            P::ScratchPush,
            P::ScratchPush,
            P::Mark,
            P::Gather,
            P::ScratchPush,
        ] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchPush));
        let (top, end) = (self.label(), self.label());
        self.code.push(Ins::Label(top));
        self.emit(Op::Prim(P::ScratchPeek));
        self.emit(Op::Lit(2));
        for op in [P::ScratchAt, P::VecLen, P::ILt] {
            self.emit(Op::Prim(op));
        }
        self.code.push(Ins::JumpZero(end));
        for depth in [2, 3] {
            self.emit(Op::Lit(depth));
            for op in [P::ScratchAt, P::ScratchPeek, P::VecAt] {
                self.emit(Op::Prim(op));
            }
        }
        let r = self.pair_body(name, &q, ea, eb)?;
        self.emit(Op::Lit(1));
        for op in [P::ScratchAt, P::Swap, P::VecPush, P::Drop, P::ScratchPop] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::IAdd));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Jump(top));
        self.code.push(Ins::Label(end));
        for op in [
            P::ScratchPop,
            P::Drop,
            P::ScratchPop,
            P::ScratchPop,
            P::Drop,
            P::ScratchPop,
            P::Drop,
        ] {
            self.emit(Op::Prim(op));
        }
        let out = match la.or(lb) {
            Some(n) => {
                let n = self.types.intern(Term::Nat(n));
                Term::Vec(n, r)
            }
            None => Term::Ary(r),
        };
        let out = self.types.intern(out);
        self.push(out, Val::Run);
        Ok(())
    }

    /// `xs ys q table`: q applied to every pair, an element of xs with an
    /// element of ys, as rows, one for each element of xs: the outer
    /// product, APL's `∘.`, Uiua's `⊞`.
    fn table(&mut self, name: &str) -> Result<()> {
        let q = self.words_of(name)?;
        let [(la, ea), (lb, eb)] = self.two_arrays(name)?;
        let base = self.stack.len() - 2;
        self.materialize_top(2)?;
        self.stack.truncate(base);
        // The scratch stack: ys, xs, the rows, i; in a row, the row and j.
        for op in [
            P::ScratchPush,
            P::ScratchPush,
            P::Mark,
            P::Gather,
            P::ScratchPush,
        ] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchPush));
        let (top_i, end_i, top_j, end_j) = (self.label(), self.label(), self.label(), self.label());
        self.code.push(Ins::Label(top_i));
        self.emit(Op::Prim(P::ScratchPeek));
        self.emit(Op::Lit(2));
        for op in [P::ScratchAt, P::VecLen, P::ILt] {
            self.emit(Op::Prim(op));
        }
        self.code.push(Ins::JumpZero(end_i));
        for op in [P::Mark, P::Gather, P::ScratchPush] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(0));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Label(top_j));
        self.emit(Op::Prim(P::ScratchPeek));
        self.emit(Op::Lit(5));
        for op in [P::ScratchAt, P::VecLen, P::ILt] {
            self.emit(Op::Prim(op));
        }
        self.code.push(Ins::JumpZero(end_j));
        // x is xs at i, y is ys at j.
        self.emit(Op::Lit(4));
        self.emit(Op::Prim(P::ScratchAt));
        self.emit(Op::Lit(2));
        self.emit(Op::Prim(P::ScratchAt));
        self.emit(Op::Prim(P::VecAt));
        self.emit(Op::Lit(5));
        for op in [P::ScratchAt, P::ScratchPeek, P::VecAt] {
            self.emit(Op::Prim(op));
        }
        let r = self.pair_body(name, &q, ea, eb)?;
        self.emit(Op::Lit(1));
        for op in [P::ScratchAt, P::Swap, P::VecPush, P::Drop, P::ScratchPop] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::IAdd));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Jump(top_j));
        self.code.push(Ins::Label(end_j));
        for op in [P::ScratchPop, P::Drop, P::ScratchPop] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(1));
        for op in [P::ScratchAt, P::Swap, P::VecPush, P::Drop, P::ScratchPop] {
            self.emit(Op::Prim(op));
        }
        self.emit(Op::Lit(1));
        self.emit(Op::Prim(P::IAdd));
        self.emit(Op::Prim(P::ScratchPush));
        self.code.push(Ins::Jump(top_i));
        self.code.push(Ins::Label(end_i));
        for op in [
            P::ScratchPop,
            P::Drop,
            P::ScratchPop,
            P::ScratchPop,
            P::Drop,
            P::ScratchPop,
            P::Drop,
        ] {
            self.emit(Op::Prim(op));
        }
        let mut sized = |n: Option<u64>, e: Type| -> Type {
            let t = match n {
                Some(n) => Term::Vec(self.types.intern(Term::Nat(n)), e),
                None => Term::Ary(e),
            };
            self.types.intern(t)
        };
        let row = sized(lb, r);
        let out = sized(la, row);
        self.push(out, Val::Run);
        Ok(())
    }

    /// `xs v k insert`: v at the gap after element k, so 0 prepends and -1
    /// appends (Thomas, 2026-10-09).
    fn insert(&mut self, name: &str, base: usize) -> Result<()> {
        let Some((len, e)) = self.array_of(self.stack[base].ty) else {
            return Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` for {}", self.top_types(3)),
            ));
        };
        self.annotate(base + 1, e)?;
        let runtime = match (self.stack[base + 2].val.clone(), len) {
            (Val::Int(k), Some(n)) => {
                let g = prims::end(k, n as i128).ok_or_else(|| {
                    self.err(
                        Kind::Mismatch,
                        format!(
                            "there is no gap {k} in a {}",
                            self.types.name(self.stack[base].ty)
                        ),
                    )
                })?;
                self.stack[base + 2].val = Val::Int(g as i128);
                false
            }
            (Val::Int(k), None) => k < 0,
            _ => true,
        };
        self.materialize_top(3)?;
        if runtime {
            self.gap_index(2);
        }
        self.emit(Op::Prim(P::VecInsert));
        let ty = match len {
            Some(n) => {
                let n = self.types.intern(Term::Nat(n + 1));
                Term::Vec(n, e)
            }
            None => Term::Ary(e),
        };
        let ty = self.types.intern(ty);
        self.stack.truncate(base);
        self.push(ty, Val::Run);
        Ok(())
    }

    /// `xs k remove`: without element k.
    fn remove(&mut self, name: &str, base: usize) -> Result<()> {
        let Some((len, e)) = self.array_of(self.stack[base].ty) else {
            return Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` for {}", self.top_types(2)),
            ));
        };
        let runtime = match self.stack[base + 1].val {
            Val::Int(k) => match self.known_element(base, k, len)? {
                Some(i) => {
                    self.stack[base + 1].val = Val::Int(i as i128);
                    false
                }
                None => true,
            },
            _ => true,
        };
        self.materialize_top(2)?;
        if runtime {
            self.element_index(1);
        }
        self.emit(Op::Prim(P::VecRemove));
        let ty = match len {
            Some(n) => {
                let n = self.types.intern(Term::Nat(n - 1));
                Term::Vec(n, e)
            }
            None => Term::Ary(e),
        };
        let ty = self.types.intern(ty);
        self.stack.truncate(base);
        self.push(ty, Val::Run);
        Ok(())
    }

    /// Takes the code a consumer applies: a quotation, or a word, which is
    /// applied as `[ word. ]` is.
    fn words_of(&mut self, name: &str) -> Result<Rc<[Token]>> {
        let j = self.stack.pop().expect("matched");
        match j.val {
            Val::Quote(q) => Ok(q),
            Val::Name(n, at) => Ok(vec![
                Token {
                    tok: Tok::Name(n),
                    pos: at,
                },
                Token {
                    tok: Tok::Apply,
                    pos: at,
                },
            ]
            .into()),
            _ => Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `{name}` for {}: it takes a quotation or a word",
                    self.describe(&j)
                ),
            )),
        }
    }

    fn loop_shape(&self, name: &str) -> Error {
        self.err(
            Kind::Mismatch,
            format!(
                "`{name}`'s quotation must take its element and leave the values \
                 below it, as many as there were, of the same types"
            ),
        )
    }

    /// Makes the values below a loop's array fit its invariant: the body is
    /// compiled to see what it leaves, and its code thrown away; a value it
    /// changes becomes, before the loop, a value at run time of the type it
    /// keeps, a literal taking the body's type; then again, until nothing
    /// changes.
    /// Inside an array literal, a body may leave values above the loop's, its
    /// elements: the loop collects them, as a run, of one type with the
    /// literal's elements before it, which become values at run time first.
    /// Then the type is returned.
    fn invariant(&mut self, base: usize, e: Type, q: &[Token], name: &str) -> Result<Option<Type>> {
        for _ in 0..4 {
            let below: Vec<Jdg> = self.stack[..base].to_vec();
            let saved = (
                std::mem::take(&mut self.code),
                self.stack.clone(),
                self.effects,
            );
            self.stack.truncate(base);
            self.push(e, Val::Run);
            let r = self.enter().and_then(|()| {
                let r = self.run(q);
                self.depth -= 1;
                r
            });
            let after = std::mem::replace(&mut self.stack, saved.1);
            self.code = saved.0;
            self.effects = saved.2;
            r?;
            if after.len() > below.len() && after[..below.len()] == below[..] {
                let Some(start) = self.collecting(base) else {
                    return Err(self.loop_shape(name));
                };
                let mut tys: Vec<Type> = self.stack[start..base].iter().map(|j| j.ty).collect();
                tys.extend(after[below.len()..].iter().map(|j| j.ty));
                let Some(t) = self.join(&tys) else {
                    let all: Vec<_> = tys.iter().map(|&t| self.types.name(t)).collect();
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("an array's elements differ in type: {}", all.join(", ")),
                    ));
                };
                let mut changed = false;
                for i in start..base {
                    if self.stack[i].ty != t || self.stack[i].known() {
                        self.annotate(i, t)?;
                        self.materialize(i)?;
                        changed = true;
                    }
                }
                if !changed {
                    return Ok(Some(t));
                }
                continue;
            }
            if after.len() != below.len() {
                return Err(self.loop_shape(name));
            }
            let mut changed = false;
            for i in 0..base {
                if after[i] == below[i] {
                    continue;
                }
                let Some(t) = self.join(&[below[i].ty, after[i].ty]) else {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "`{name}`'s quotation changes a value's type: {} becomes {}",
                            self.types.name(below[i].ty),
                            self.types.name(after[i].ty)
                        ),
                    ));
                };
                if self.stack[i].ty != t || self.stack[i].known() {
                    self.annotate(i, t)?;
                    self.materialize(i)?;
                    changed = true;
                } else if matches!(self.stack[i].val, Val::Pinned(_)) {
                    self.stack[i].val = Val::Run;
                    changed = true;
                }
            }
            if !changed {
                return Ok(None);
            }
        }
        Err(self.err(
            Kind::Mismatch,
            format!("the values `{name}`'s loop changes do not settle"),
        ))
    }

    // ---- Array and map literals ----

    /// The type values of the given types share: equal types, or arrays of
    /// one element type; literals take the others' type, or their default.
    fn join(&mut self, tys: &[Type]) -> Option<Type> {
        let mut t: Option<Type> = None;
        let typed: Vec<Type> = tys
            .iter()
            .copied()
            .filter(|&x| !self.types.is_literal(x))
            .collect();
        for x in typed {
            t = Some(match t {
                None => x,
                Some(y) if y == x => y,
                // One is the other, less precisely known: a bool is an i64.
                Some(y) if self.forgets(y, x) => y,
                Some(y) if self.forgets(x, y) => x,
                Some(y) => match (self.types.term(y), self.types.term(x)) {
                    (Term::Vec(_, e) | Term::Ary(e), Term::Vec(_, f) | Term::Ary(f)) if e == f => {
                        self.types.intern(Term::Ary(e))
                    }
                    _ => return None,
                },
            });
        }
        Some(t.unwrap_or(if tys.contains(&DEC_LIT) { F64 } else { I64 }))
    }

    fn close_literal(&mut self, b: u8) -> Result<()> {
        if self.in_bracket > 0 && b == b')' {
            return self.close_tuple_type();
        }
        let lit = self.literals.pop().expect("the reader checks nesting");
        let start = lit.start;
        let n = self.stack.len() - start;
        let what = if b == b')' { "an array" } else { "a map" };
        let runs = self.stack[start..].iter().any(|j| j.val == Val::Many);
        if n == 0 {
            return Err(self.err(
                Kind::Mismatch,
                format!("{what} with no elements has no type for them"),
            ));
        }
        if b == b'}' && (n % 2 == 1 || runs) {
            return Err(self.err(Kind::Mismatch, "a map needs a value for each key"));
        }
        let step = if b == b')' { 1 } else { 2 };
        let mut parts = Vec::new();
        // An array's elements of different types, at fixed positions, are a
        // tuple (TYPES.md 2.5); each literal among them takes its default.
        if b == b')' && !runs {
            let tys: Vec<Type> = self.stack[start..].iter().map(|j| j.ty).collect();
            let shared = self.join(&tys).filter(|&t| {
                tys.iter().all(|&x| {
                    x == t || self.forgets(t, x) || (self.types.is_literal(x) && self.accepts(t, x))
                })
            });
            if shared.is_none() {
                for i in start..self.stack.len() {
                    self.materialize(i)?;
                }
                let elems = self.stack[start..].iter().map(|j| j.ty).collect();
                parts.push(self.types.tuple(elems));
            }
        }
        let tuple = !parts.is_empty();
        for first in (0..step).filter(|_| !tuple) {
            let tys: Vec<Type> = (start + first..self.stack.len())
                .step_by(step)
                .map(|i| self.stack[i].ty)
                .collect();
            let Some(t) = self.join(&tys) else {
                let all: Vec<_> = tys.iter().map(|&t| self.types.name(t)).collect();
                return Err(self.err(
                    Kind::Mismatch,
                    format!("{what}'s elements differ in type: {}", all.join(", ")),
                ));
            };
            for i in (start + first..self.stack.len()).step_by(step) {
                self.annotate(i, t)?;
            }
            parts.push(t);
        }
        // With a run among them the count is known only at run time.
        let ty = if b == b'}' {
            self.types.intern(Term::Map(parts[0], parts[1]))
        } else if tuple {
            parts[0]
        } else if runs {
            self.types.intern(Term::Ary(parts[0]))
        } else {
            let n = self.types.intern(Term::Nat(n as u64));
            self.types.intern(Term::Vec(n, parts[0]))
        };
        let op = if b == b')' { P::Gather } else { P::MapGather };
        for i in start..self.stack.len() {
            self.materialize(i)?;
        }
        for _ in start..self.stack.len() {
            self.stack.pop();
        }
        self.emit(Op::Prim(op));
        self.push(ty, Val::Run);
        self.floor = lit.floor;
        // The values it pulled are its inputs: it takes them.
        for _ in 0..lit.pulls.len() {
            let i = self.stack.len() - 2;
            if !self.stack[i].known() {
                self.emit(Op::Prim(P::Swap));
                self.emit(Op::Prim(P::Drop));
            }
            self.stack.remove(i);
        }
        Ok(())
    }

    /// `( … )` in a bracket: the types in it, a tuple type.
    fn close_tuple_type(&mut self) -> Result<()> {
        let lit = self.literals.pop().expect("the reader checks nesting");
        let mut elems = Vec::new();
        for j in self.stack.drain(lit.start..).collect::<Vec<_>>() {
            match j.val {
                Val::Type(t) => elems.push(t),
                _ if j.ty == NIL => elems.push(NIL),
                _ => {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "a tuple type holds types, and {} is not one",
                            self.describe(&j)
                        ),
                    ));
                }
            }
        }
        self.floor = lit.floor;
        if elems.is_empty() {
            return Err(self.err(Kind::Mismatch, "a tuple type with no types"));
        }
        let t = self.types.tuple(elems);
        self.push(TYPE, Val::Type(t));
        Ok(())
    }

    /// `_.` in a literal: a copy of one of the values below it, its inputs,
    /// the first `_.` written taking the deepest. A value at run time is
    /// found from the literal's mark, so the copy is exact even above a run.
    fn pull(&mut self, pos: Pos) -> Result<()> {
        let Some((li, k)) = self
            .literals
            .iter()
            .enumerate()
            .rev()
            .find_map(|(li, l)| l.pulls.iter().position(|&p| p == pos).map(|k| (li, k)))
        else {
            return Err(self.err(
                Kind::Syntax,
                "`_` takes a value from below an array literal, and it is in none",
            ));
        };
        let lit = &self.literals[li];
        let (start, inputs, string) = (lit.start, lit.pulls.len(), lit.bracket == b'"');
        let i = start - inputs + k;
        // A string makes no mark: nothing above its inputs is a run, so a copy
        // finds them by counting.
        if string {
            return self.copy(i);
        }
        let j = self.stack[i].clone();
        if !j.known() {
            let below = self.stack[i + 1..start]
                .iter()
                .filter(|j| !j.known())
                .count();
            let marks = self.literals[li + 1..]
                .iter()
                .filter(|l| l.bracket != b'"')
                .count();
            self.emit(Op::Lit(below as u64));
            self.emit(Op::Lit(marks as u64));
            self.emit(Op::Prim(P::MarkPick));
        }
        self.stack.push(j);
        Ok(())
    }

    /// A string with holes: each hole's code runs, its value is written in
    /// by `>string`, and the pieces are joined by `concat`, so on known values
    /// the string is a constant. `_.` in a hole pulls an input from below the
    /// string, as in an array literal, and the string takes its inputs.
    fn template(&mut self, pieces: &[Piece], pos: Pos) -> Result<()> {
        let pulls = template_pulls(pieces);
        let n = pulls.len();
        self.need(n)?;
        self.literals.push(Literal {
            bracket: b'"',
            start: self.stack.len(),
            floor: self.floor,
            pulls,
        });
        let r = self.pieces(pieces, pos);
        let lit = self.literals.pop().expect("pushed");
        self.floor = lit.floor;
        r?;
        for _ in 0..n {
            let i = self.stack.len() - 2;
            if !self.stack[i].known() {
                self.emit(Op::Prim(P::Swap));
                self.emit(Op::Prim(P::Drop));
            }
            self.stack.remove(i);
        }
        Ok(())
    }

    fn pieces(&mut self, pieces: &[Piece], pos: Pos) -> Result<()> {
        for (k, piece) in pieces.iter().enumerate() {
            match piece {
                Piece::Text(t) => self.push(STRING, Val::Str(t.clone())),
                Piece::Hole(toks) => {
                    // A hole reaches only what it makes.
                    let depth = self.stack.len();
                    self.floor = depth;
                    self.run(toks)?;
                    self.pos = pos;
                    if self.stack.len() != depth + 1 {
                        return Err(self.err(
                            Kind::Mismatch,
                            format!(
                                "a hole leaves one value, to write in, and this one leaves {}",
                                values(self.stack.len() - depth)
                            ),
                        ));
                    }
                    self.apply_word(&">string".into())?;
                }
            }
            if k > 0 {
                self.floor = self.stack.len() - 2;
                self.apply_word(&"concat".into())?;
            }
        }
        Ok(())
    }
}
