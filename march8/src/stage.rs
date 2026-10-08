//! The symbolic stack machine (docs/MACHINE.md): it runs the explicit form,
//! in which `.` applies, on a stack of judgments, and leaves code for the
//! machine behind.
//!
//! A judgment is a value's type, and its value when that is known at compile
//! time. A known value has no code until a value at run time is needed:
//! it is *materialized* then, so constants fold through every word, and the
//! code holds only what must happen at run time.

use crate::code::{Op, Primitive as P};
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
}

/// A signature: the input patterns, deepest first, and the outputs it
/// promises, if it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Sig {
    pub ins: Vec<Type>,
    pub outs: Option<Vec<Type>>,
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
    /// Families being applied, so that one applying itself is caught.
    active: Vec<Rc<str>>,
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

    /// Starts a compilation whose inputs are values on the machine's stack.
    pub fn begin(&mut self, inputs: &[Type]) {
        self.stack = inputs.iter().map(|&ty| Jdg { ty, val: Val::Run }).collect();
        self.code.clear();
        self.floor = 0;
        self.literals.clear();
        self.active.clear();
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
        if let Some((p, sig)) = w.prim.clone() {
            self.prim(name, p, &sig)
        } else if let Some(t) = w.ty {
            self.need(1)?;
            self.annotate(self.stack.len() - 1, t)
        } else if let Some(c) = w.con {
            self.construct(c)
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

    /// Applies a family: the clause whose inputs match best (TYPES.md 2.8),
    /// evaluated here, on these judgments (2.6).
    fn family(&mut self, name: &Rc<str>) -> Result<()> {
        if self.active.contains(name) {
            return Err(self.err(
                Kind::Limit,
                format!("`{name}` applies itself: recursion is not built yet"),
            ));
        }
        let clauses = self.words[name].clauses.clone();
        let mut best: Option<(i32, Rc<Clause>, Env)> = None;
        let mut tie = false;
        for c in &clauses {
            let m = match &c.sig {
                None => Some((0, Env::default())),
                Some(s) => self.match_sig(s),
            };
            let Some((score, env)) = m else { continue };
            match &best {
                Some((b, ..)) if score < *b => {}
                Some((b, ..)) if score == *b => tie = true,
                _ => {
                    best = Some((score, c.clone(), env));
                    tie = false;
                }
            }
        }
        let arity = clauses
            .iter()
            .filter_map(|c| c.sig.as_ref().map(|s| s.ins.len()))
            .max()
            .unwrap_or(1);
        let Some((_, c, env)) = best else {
            let fewest = clauses
                .iter()
                .map(|c| c.sig.as_ref().map_or(0, |s| s.ins.len()))
                .min()
                .unwrap_or(0);
            self.need(fewest)?;
            return Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` for {}", self.top_types(arity)),
            ));
        };
        if tie {
            return Err(self.err(
                Kind::Mismatch,
                format!("clauses of `{name}` tie for {}", self.top_types(arity)),
            ));
        }
        let at = self.pos;
        self.active.push(name.clone());
        let r = self.clause(&c, env);
        self.active.pop();
        self.pos = at;
        r.map_err(|e| match c.core {
            true => self.blame(&c, e, at),
            false => e.within(name, at),
        })
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
        let ins = clause.sig.as_ref().map(|s| s.ins.clone());
        let c = Rc::new(clause);
        let w = self.word(name);
        let before = w.clauses.clone();
        match w
            .clauses
            .iter()
            .position(|old| old.sig.as_ref().map(|s| &s.ins) == ins.as_ref())
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
        self.active.push(name.clone());
        let mut r = self.clause(c, Env::default());
        for i in 0..self.stack.len() {
            if r.is_ok() {
                r = self.materialize(i);
            }
        }
        self.active.pop();
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
                        return Err(
                            self.err(Kind::Limit, format!("`{s}`: guards are not built yet"))
                        );
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
                outs: None,
            },
            Some(d) => {
                let outs = items.split_off(d);
                Sig {
                    ins: items,
                    outs: Some(outs),
                }
            }
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
    /// value is emitted, under the values at run time above it. A literal
    /// takes its default type.
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
        for _ in 0..above {
            self.emit(Op::Prim(P::ScratchPush));
        }
        self.code.push(ins);
        for _ in 0..above {
            self.emit(Op::Prim(P::ScratchPop));
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
