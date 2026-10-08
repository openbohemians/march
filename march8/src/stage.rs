//! The symbolic stack machine (docs/MACHINE.md): it runs the explicit form,
//! in which `.` applies, on a stack of judgments, and leaves code for the
//! machine behind.
//!
//! A judgment is a value's type, and its value when that is known at compile
//! time. A known value has no code until a value at run time is needed:
//! it is *materialized* then, so constants fold through every word, and the
//! code holds only what must happen at run time.

use crate::code::{Blob, Cid, Op, Primitive as P};
use crate::error::{Error, Kind, Pos, Result};
use crate::prims::{self, PRIMS, Prim};
use crate::read::{Tok, Token};
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
    /// A name not yet applied.
    Name(Rc<str>),
    /// A quotation.
    Quote(Rc<[Token]>),
}

#[derive(Clone, Debug)]
pub struct Jdg {
    pub ty: Type,
    pub val: Val,
}

impl Jdg {
    pub fn known(&self) -> bool {
        self.val != Val::Run
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
    pub name: Rc<str>,
    pub slot: usize,
    pub arity: usize,
    pub pos: Pos,
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
        (&self.name, self.slot, self.arity) == (&o.name, o.slot, o.arity)
    }
}

impl Sig {
    /// The context a clause is chosen by: its inputs and guards.
    fn context(&self) -> (&[Type], &[Guard]) {
        (&self.ins, &self.guards)
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
    /// Open array and map literals: the bracket, where they start, and the
    /// floor outside.
    literals: Vec<(u8, usize, usize)>,
    /// Families being applied, with the types they were applied to, so that
    /// one applying itself to the same types is caught.
    active: Vec<Key>,
    labels: u32,
    /// Instances being compiled, innermost last.
    frames: Vec<Frame>,
    /// Instances compiled: their code's identity and their results' types.
    instances: HashMap<Key, (Cid, Vec<Type>)>,
    /// Code and data made while compiling, in the order made, so a callee
    /// comes before its callers; the session publishes them.
    pub blobs: Vec<Blob>,
    pos: Pos,
    steps: u64,
    depth: u32,
    /// Set while the core vocabulary is read.
    pub core: bool,
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
        };
        for (i, (_, name)) in Base::ALL.iter().enumerate() {
            let name: Rc<str> = (*name).into();
            s.word(&name).ty = Some(i as Type);
            s.type_names.insert(i as Type, name);
        }
        for (con, name) in [(Con::Ary, "ary"), (Con::Vec, "vec"), (Con::Map, "map")] {
            s.word(&name.into()).con = Some(con);
        }
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
                Tok::Name(s) => self.push(SYMBOL, Val::Name(s.clone())),
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
                Tok::Open(b) => {
                    self.emit(Op::Prim(P::Mark));
                    self.literals.push((*b, self.stack.len(), self.floor));
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
            Val::Name(n) => self.apply_word(&n),
            Val::Type(t) => {
                self.need(1)?;
                self.annotate(self.stack.len() - 1, t)
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
        let typeish = self.stack.len() > self.floor
            && match &self.stack[self.stack.len() - 1].val {
                Val::Type(_) => true,
                Val::Name(n) => self.words.get(n).is_some_and(|w| w.ty.is_some()),
                _ => false,
            };
        if let Some(c) = w.con
            && (typeish || w.prim.is_none())
        {
            self.construct(c)
        } else if let Some((p, sig)) = w.prim.clone() {
            self.prim(name, p, &sig)
        } else if let Some(t) = w.ty {
            self.need(1)?;
            self.annotate(self.stack.len() - 1, t)
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
            Val::Int(n) => format!("{n} ({})", self.types.name(j.ty)),
            Val::Dec(d, s) => format!("{} (dec#)", prims::show_dec(*d, *s)),
            Val::Float(x) => format!("{x:?} (f64)"),
            Val::Str(s) => format!("{s:?}"),
            Val::Type(t) => format!("the type `{}`", self.types.name(*t)),
            Val::Name(n) => format!("the word `{n}`"),
            Val::Quote(_) => "a quotation".into(),
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
        if have == t || self.types.has_vars(t) {
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
                _ => false,
            }
    }

    /// A bracket in a body: it types the values on top.
    fn annotate_top(&mut self, sig: &Sig) -> Result<()> {
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
            score += self.match_slot(sig.ins[k], have, &mut env)?;
        }
        Some((score, env))
    }

    /// A literal matches its own type best, then its default type, then any
    /// type it converts to; anything else must unify.
    fn match_slot(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        if self.types.is_literal(have) {
            return match self.types.term(p) {
                Term::Var(v) => self.bind(v, have, env),
                _ if p == have => Some(4),
                _ if self.accepts(p, have) => Some(if p == Self::default_of(have) { 2 } else { 1 }),
                _ => None,
            };
        }
        self.unify(p, have, env)
    }

    fn bind(&mut self, v: u8, have: Type, env: &mut Env) -> Option<i32> {
        let v = v as usize;
        match env.0[v] {
            None => {
                env.0[v] = Some(have);
                Some(0)
            }
            Some(b) if b == have => Some(0),
            // A variable bound to a literal takes a type the literal becomes.
            Some(b) if self.types.is_literal(b) && self.accepts(have, b) => {
                env.0[v] = Some(have);
                Some(0)
            }
            Some(b) if self.types.is_literal(have) && self.accepts(b, have) => Some(0),
            _ => None,
        }
    }

    /// Scores 4 for each part of the pattern that is not a variable, and 2
    /// where a vec stands for an array.
    fn unify(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        match (self.types.term(p), self.types.term(have)) {
            (Term::Var(v), _) => self.bind(v, have, env),
            (Term::Base(a), Term::Base(b)) => (a == b).then_some(4),
            (Term::Nat(a), Term::Nat(b)) => (a == b).then_some(4),
            (Term::Ary(e), Term::Ary(f)) => Some(4 + self.unify(e, f, env)?),
            (Term::Ary(e), Term::Vec(_, f)) => Some(2 + self.unify(e, f, env)?),
            (Term::Vec(n, e), Term::Vec(m, f)) => {
                Some(4 + self.unify(n, m, env)? + self.unify(e, f, env)?)
            }
            (Term::Map(k, v), Term::Map(k2, v2)) => {
                Some(4 + self.unify(k, k2, env)? + self.unify(v, v2, env)?)
            }
            _ => None,
        }
    }

    /// Gives the inputs the types their patterns name, so literals convert.
    fn settle(&mut self, sig: &Sig, env: &Env) -> Result<()> {
        let base = self.stack.len() - sig.ins.len();
        for (k, &p) in sig.ins.iter().enumerate() {
            let t = self.types.subst(p, env);
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
            if self.types.is_literal(self.stack[base + k].ty) && !self.types.has_vars(t) {
                self.annotate(base + k, t)?;
            }
        }
        Ok(())
    }

    /// Evaluates a clause of a family, noting the family in its errors.
    fn apply_clause(&mut self, name: &str, c: &Rc<Clause>, env: Env, at: Pos) -> Result<()> {
        let r = self.clause(c, env);
        self.pos = at;
        r.map_err(|e| match c.core {
            true => self.blame(c, e, at),
            false => e.within(name, at),
        })
    }

    /// The choice at run time: for each guarded clause, in order, a test of
    /// its guards and its body; then the best clause without guards, or no
    /// word. Each alternative starts from the same judgments, in code of its
    /// own, and all must leave the same types (TYPES.md 2.9). A guard on known
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
        self.merge(alts.into_iter().flatten().collect(), &snapshot, base)
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
    fn merge(&mut self, alts: Vec<Alt>, snapshot: &[Jdg], base: usize) -> Result<()> {
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
        let below = &snapshot[..base];
        let mut results: Option<Vec<Vec<Type>>> = None;
        for alt in &alts {
            let Some(stack) = &alt.stack else { continue };
            let same_below = stack.len() >= base
                && stack[..base]
                    .iter()
                    .zip(below)
                    .all(|(a, b)| a.ty == b.ty && a.val == b.val);
            if !same_below {
                return Err(self.err(
                    Kind::Mismatch,
                    "the clauses chosen between at run time take different values",
                ));
            }
            let tys: Vec<Type> = stack[base..].iter().map(|j| j.ty).collect();
            let r = results.get_or_insert_with(|| vec![Vec::new(); tys.len()]);
            if r.len() != tys.len() {
                return Err(self.err(
                    Kind::Mismatch,
                    "the clauses chosen between at run time leave different numbers of values",
                ));
            }
            for (slot, t) in r.iter_mut().zip(tys) {
                slot.push(t);
            }
        }
        let mut joined = Vec::new();
        for tys in results.unwrap_or_default() {
            let Some(t) = self.join(&tys) else {
                let all: Vec<_> = tys.iter().map(|&t| self.types.name(t)).collect();
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "the clauses chosen between at run time leave different types: {}",
                        all.join(", ")
                    ),
                ));
            };
            joined.push(t);
        }
        let end = self.label();
        let last = alts.len() - 1;
        for (k, alt) in alts.into_iter().enumerate() {
            let outer = std::mem::replace(&mut self.code, alt.code);
            let r = match alt.stack {
                Some(stack) => {
                    self.stack = stack;
                    self.settle_results(base, &joined)
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
        self.stack = below.to_vec();
        for t in joined {
            self.push(t, Val::Run);
        }
        Ok(())
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
        let (cid, outs) = self.instance(name, ins, arity, at)?;
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
    ) -> Result<(Cid, Vec<Type>)> {
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
        self.frames.pop();
        (
            self.stack,
            self.code,
            self.floor,
            self.literals,
            self.active,
            self.depth,
        ) = saved;
        let outs = r?;
        let (_, cid) = self.seal(code);
        self.instances.insert(key, (cid, outs.clone()));
        Ok((cid, outs))
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
            .all(|&t| !self.types.has_vars(t))
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
            self.annotate(base + i, t)?;
        }
        self.materialize_top(joined.len())
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
            self.pos = g.pos;
            let r = self.apply_word(&g.name);
            self.floor = floor;
            r?;
            self.pos = g.pos;
            if self.stack.len() != depth + 1 {
                return Err(self.err(
                    Kind::Mismatch,
                    format!("the guard `{}` must leave one flag", g.name),
                ));
            }
            let flag = self.stack.pop().expect("one");
            match flag.val {
                Val::Int(0) => return Ok(Some(false)),
                Val::Int(_) => {}
                Val::Run if flag.ty == I64 => {
                    self.code.push(Ins::JumpZero(skip));
                    certain = false;
                }
                _ => {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "the guard `{}` leaves {}, not a flag",
                            g.name,
                            self.describe(&flag)
                        ),
                    ));
                }
            }
        }
        Ok(certain.then_some(true))
    }

    /// Pushes a copy of the judgment at `i`: a known value is copied as it
    /// is, and a value at run time by copying it on the machine's stack.
    fn copy(&mut self, i: usize) -> Result<()> {
        let j = self.stack[i].clone();
        if !j.known() {
            let above = self.stack[i + 1..].iter().filter(|j| !j.known()).count();
            match above {
                0 => self.emit(Op::Prim(P::Dup)),
                1 => self.emit(Op::Prim(P::Over)),
                n => {
                    self.emit(Op::Lit(n as u64));
                    self.emit(Op::Prim(P::Pick));
                }
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
                Prim::VecConcat | Prim::Map => (2, 1),
                Prim::Def => (2, 0),
                _ => (sig.ins.len(), sig.outs.as_ref().map_or(0, Vec::len)),
            });
        }
        if w.ty.is_some() {
            return Some((1, 1));
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

    /// How many types a bracket in a body leaves.
    fn bracket_size(&self, toks: &[Token]) -> usize {
        let mut n: usize = 0;
        for t in toks {
            match &t.tok {
                Tok::Int(_) => n += 1,
                Tok::Name(s) => match self.words.get(s) {
                    Some(w) if w.ty.is_some() => n += 1,
                    Some(w) if w.con == Some(Con::Ary) => {}
                    Some(w) if w.con.is_some() => n = n.saturating_sub(1),
                    None => n += 1,
                    Some(_) => {}
                },
                _ => {}
            }
        }
        n
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
                .all(|&t| !self.types.has_vars(t) && !self.types.compile_time_only(t))
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
        (self.stack, self.code, self.floor, self.literals, self.pos) = saved;
        r.map_err(|e| {
            let mut e = e;
            e.trace.push(format!("in `{name}`, defined at {}", c.at));
            e
        })
    }

    // ---- Types as values ----

    /// A type expression in `< >`: a type's name pushes the type, a number a
    /// length, a single letter a variable, and a constructor builds from what
    /// is below it. `--` divides inputs from outputs.
    pub fn bracket(&mut self, toks: &[Token]) -> Result<Sig> {
        let mut items: Vec<Type> = Vec::new();
        let mut guards = Vec::new();
        let mut dashes = None;
        for t in toks {
            self.pos = t.pos;
            match &t.tok {
                Tok::Int(n) => {
                    let n = u64::try_from(*n)
                        .map_err(|_| self.err(Kind::Mismatch, "a length cannot be negative"))?;
                    items.push(self.types.intern(Term::Nat(n)));
                }
                Tok::Dashes if dashes.is_none() => dashes = Some(items.len()),
                Tok::Name(s) => {
                    let w = self.words.get(s);
                    if let Some(t) = w.and_then(|w| w.ty) {
                        items.push(t);
                    } else if let Some(c) = w.and_then(|w| w.con) {
                        self.build(c, &mut items)?;
                    } else if w.is_none() && s.len() == 1 && s.as_bytes()[0].is_ascii_lowercase() {
                        items.push(self.types.intern(Term::Var(s.as_bytes()[0] - b'a')));
                    } else if w.is_some() {
                        guards.push(self.guard(s, &items, dashes.is_some(), t.pos)?);
                    } else {
                        return Err(self.err(Kind::NoWord, format!("no type `{s}`")));
                    }
                }
                _ => {
                    return Err(
                        self.err(Kind::Syntax, "a bracket holds types, lengths and one `--`")
                    );
                }
            }
        }
        for &t in &items {
            if let Term::Nat(n) = self.types.term(t) {
                return Err(self.err(
                    Kind::Mismatch,
                    format!("a bracket leaves types, and {n} is a length"),
                ));
            }
        }
        Ok(match dashes {
            None => Sig {
                ins: items,
                guards,
                outs: None,
            },
            Some(d) => {
                let outs = items.split_off(d);
                Sig {
                    ins: items,
                    guards,
                    outs: Some(outs),
                }
            }
        })
    }

    /// A guard in a bracket: a word that looks at the inputs before it and
    /// leaves one flag.
    fn guard(&self, name: &Rc<str>, items: &[Type], outputs: bool, pos: Pos) -> Result<Guard> {
        if outputs {
            return Err(self.err(
                Kind::Syntax,
                format!("the guard `{name}` belongs among the inputs, before `--`"),
            ));
        }
        let Some((ins, outs)) = self.effect_of(name) else {
            return Err(self.err(
                Kind::Mismatch,
                format!("`{name}` cannot be a guard: how many values it takes is not known"),
            ));
        };
        if outs != 1 {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`{name}` leaves {}, and a guard leaves one flag",
                    values(outs)
                ),
            ));
        }
        if ins > items.len()
            || items
                .iter()
                .any(|&t| matches!(self.types.term(t), Term::Nat(_)))
        {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "the guard `{name}` looks at {}, and {} before it",
                    values(ins),
                    values(items.len())
                ),
            ));
        }
        Ok(Guard {
            name: name.clone(),
            slot: items.len() - ins,
            arity: ins,
            pos,
        })
    }

    /// Applies a constructor inside a bracket.
    fn build(&mut self, c: Con, items: &mut Vec<Type>) -> Result<()> {
        let missing = |s: &Self| {
            s.err(
                Kind::Mismatch,
                format!("`{c:?}` needs more below it").to_lowercase(),
            )
        };
        let ty = |s: &mut Self, items: &mut Vec<Type>| -> Result<Type> {
            let t = items.pop().ok_or_else(|| missing(s))?;
            if let Term::Nat(n) = s.types.term(t) {
                return Err(s.err(Kind::Mismatch, format!("{n} is a length, not a type")));
            }
            Ok(t)
        };
        let t = match c {
            Con::Ary => {
                let e = ty(self, items)?;
                Term::Ary(e)
            }
            Con::Vec => {
                let e = ty(self, items)?;
                let n = items.pop().ok_or_else(|| missing(self))?;
                if !matches!(self.types.term(n), Term::Nat(_) | Term::Var(_)) {
                    return Err(self.err(Kind::Mismatch, "`vec` needs a length below its type"));
                }
                Term::Vec(n, e)
            }
            Con::Map => {
                let v = ty(self, items)?;
                let k = ty(self, items)?;
                Term::Map(k, v)
            }
        };
        items.push(self.types.intern(t));
        Ok(())
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
        };
        let t = self.types.intern(t);
        self.push(TYPE, Val::Type(t));
        Ok(())
    }

    fn pop_type(&mut self) -> Result<Type> {
        let j = self.pop()?;
        match &j.val {
            Val::Type(t) => Ok(*t),
            Val::Name(n) if self.words.get(n).is_some_and(|w| w.ty.is_some()) => {
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
                if !j.known() {
                    self.emit(Op::Prim(P::Dup));
                }
                self.stack.push(j);
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
                if !j.known() {
                    let top_runs = !self.stack[base + 1].known();
                    self.emit(Op::Prim(if top_runs { P::Over } else { P::Dup }));
                }
                self.stack.push(j);
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
                if let Val::Int(i) = self.stack[base + 1].val
                    && (i < 0 || i >= k as i128)
                {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!(
                            "index {i} is outside a {}",
                            self.types.name(self.stack[base].ty)
                        ),
                    ));
                }
                let e = env.0[0].expect("bound");
                self.emit_ops(2, &[P::VecAt], &[e])?;
            }
            Prim::VecConcat => {
                let k = len(self, 13) + len(self, 12);
                let e = env.0[0].expect("bound");
                let k = self.types.intern(Term::Nat(k));
                let t = self.types.intern(Term::Vec(k, e));
                self.emit_ops(2, &[P::Concat], &[t])?;
            }
            Prim::Map => self.map(name)?,
            Prim::Def => {
                let name = self.stack.pop().expect("matched");
                let quote = self.stack.pop().expect("matched");
                let (Val::Name(name), Val::Quote(q)) = (name.val, quote.val) else {
                    return Err(self.err(Kind::Mismatch, "`def` takes a quotation and a name"));
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
                let known = self.stack[base..].iter().all(Jdg::known);
                if known && prims::foldable(p) {
                    let args: Vec<Val> = self.stack.drain(base..).map(|j| j.val).collect();
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
        let quote = self.stack.pop().expect("matched");
        let Val::Quote(q) = quote.val else {
            unreachable!("a quotation's value is its words")
        };
        let base = self.stack.len() - 1;
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
        let (_, start, floor) = self.literals.pop().expect("the reader checks nesting");
        let n = self.stack.len() - start;
        let what = if b == b')' { "an array" } else { "a map" };
        if n == 0 {
            return Err(self.err(
                Kind::Mismatch,
                format!("{what} with no elements has no type for them"),
            ));
        }
        if b == b'}' && n % 2 == 1 {
            return Err(self.err(Kind::Mismatch, "a map needs a value for each key"));
        }
        let step = if b == b')' { 1 } else { 2 };
        let mut parts = Vec::new();
        for first in 0..step {
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
        let ty = if b == b')' {
            let n = self.types.intern(Term::Nat(n as u64));
            self.types.intern(Term::Vec(n, parts[0]))
        } else {
            self.types.intern(Term::Map(parts[0], parts[1]))
        };
        let op = if b == b')' { P::Gather } else { P::MapGather };
        self.emit_ops(n, &[op], &[ty])?;
        self.floor = floor;
        Ok(())
    }
}
