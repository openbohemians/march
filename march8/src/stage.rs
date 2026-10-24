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
use crate::machine::{self, Machine};
use crate::prims::{self, Effects, PRIMS, Prim};
use crate::read::{self, Tok, Token};
use crate::types::*;
use regex::{Regex, RegexSet};
use std::collections::{HashMap, HashSet};
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
    /// A ratio literal: its terms, as written or, once computed, in lowest
    /// terms.
    Ratio(Rc<[i128]>),
    Float(f64),
    Str(Rc<str>),
    /// A type, as a value.
    Type(Type),
    /// A name not yet applied, a symbol, and where it was written, where
    /// errors in applying it are reported. A symbol is global text
    /// (doc/design/SYMBOLIC-FORTH.md 16): code means what it means where it
    /// was written because its names are expanded to their paths there.
    Name(Rc<str>, Pos),
    /// A quotation.
    Quote(Rc<[Token]>),
    /// An array literal whose elements are all known: built when a value
    /// at run time is needed, so it can be read while compiling, `(
    /// "can't read it" no-file' ) fail`, and a clause that wants its
    /// elements as another type takes them so (TYPES.md 2.10).
    Seq(Rc<[Jdg]>),
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

/// What a guard tests: a quotation that leaves a flag, `[ positive? ]`, or,
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
            // A symbol by its name, not where it was written.
            (Test::Equals(a), Test::Equals(b)) => match (&a.val, &b.val) {
                (Val::Name(x, ..), Val::Name(y, ..)) => a.ty == b.ty && x == y,
                _ => a == b,
            },
            _ => false,
        }
    }
}

/// Whether a name is spelled as one that does not apply: a variable, one
/// letter.
fn unapplied(n: &str) -> bool {
    read::is_variable(n)
}

/// Whether a name is `var#`, which makes a variable of a letter, by its
/// path or not.
fn is_var(n: &str) -> bool {
    n == "var#" || n == "core.var#"
}

/// Tokens as they are written, for a message: a name and its application
/// are the name, `!` and `;` among them, a variable, `a var# .`, is its
/// letter, a quotation made data is its bracket, `[ … ] seq# !` being
/// `( … )`, a string is `"…"`, and a symbol is spelled as one.
fn source_text(toks: &[Token]) -> String {
    // Each bracket's spelling, by where it is, and the words that make one,
    // which it stands for; and where each string's quotation closes.
    let mut spelled: Vec<Option<&str>> = vec![None; toks.len()];
    let mut made = vec![false; toks.len()];
    let mut strings: Vec<Option<usize>> = vec![None; toks.len()];
    let mut open = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.tok {
            Tok::Open(_) => open.push(i),
            Tok::Close(_) => {
                let (o, c) = match (open.pop(), made_at(toks, i + 1)) {
                    (Some(o), Some("str#")) => {
                        strings[o] = Some(i);
                        continue;
                    }
                    (Some(o), Some("seq#")) => (o, ["(", ")"]),
                    (Some(o), Some("map#")) => (o, ["{", "}"]),
                    (Some(o), Some(_)) => (o, ["<", ">"]),
                    _ => continue,
                };
                (spelled[o], spelled[i]) = (Some(c[0]), Some(c[1]));
                made[i + 1..i + 4].fill(true);
            }
            _ => {}
        }
    }
    let mut out = String::new();
    let mut i = 0;
    while i < toks.len() {
        if made[i] {
            i += 1;
            continue;
        }
        if let Some(c) = strings[i] {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&string_text(&toks[i + 1..c]));
            i = c + 4;
            continue;
        }
        let word = match &toks[i].tok {
            Tok::Apply => ".".into(),
            Tok::Test => "?".into(),
            Tok::Name(s) if toks.get(i + 1).is_some_and(|t| t.tok == Tok::Test) => {
                i += 1;
                format!("{s}?")
            }
            // A variable, `a var# .`, is its letter.
            Tok::Name(s)
                if unapplied(s)
                    && matches!(toks.get(i + 1).map(|t| &t.tok), Some(Tok::Name(v)) if is_var(v))
                    && toks.get(i + 2).is_some_and(|t| t.tok == Tok::Apply) =>
            {
                i += 2;
                s.to_string()
            }
            Tok::Name(s) if toks.get(i + 1).is_some_and(|t| t.tok == Tok::Apply) => {
                i += 1;
                if unapplied(s) {
                    format!("{s}.")
                } else {
                    s.to_string()
                }
            }
            Tok::Name(s) => {
                let mut out = String::new();
                crate::show::symbol(s, &mut out);
                out
            }
            Tok::Int(n) => n.to_string(),
            Tok::Dec(d, s) => prims::show_dec(*d, *s),
            Tok::Ratio(t) => prims::show_ratio(t),
            Tok::Open(b) | Tok::Close(b) => spelled[i].map_or((*b as char).to_string(), Into::into),
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&word);
        i += 1;
    }
    out
}

/// A string as it is written, `"…"`, from its quotation's code: a symbol is
/// its text, and the code between them a hole, a letter among it,
/// `\[ a ]`.
fn string_text(toks: &[Token]) -> String {
    let text = |k: usize| {
        matches!(toks[k].tok, Tok::Name(_))
            && !toks.get(k + 1).is_some_and(|t| {
                matches!(&t.tok, Tok::Apply | Tok::Test)
                    || matches!(&t.tok, Tok::Name(v) if is_var(v))
            })
    };
    let mut out = String::from('"');
    let mut i = 0;
    while i < toks.len() {
        if text(i) {
            let Tok::Name(s) = &toks[i].tok else {
                unreachable!("text is a symbol")
            };
            for c in s.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n;"),
                    '\t' => out.push_str("\\t;"),
                    c => out.push(c),
                }
            }
            i += 1;
        } else {
            let mut j = i;
            while j < toks.len() && !(j > i && text(j)) {
                j = match toks[j].tok {
                    Tok::Open(_) => {
                        let c = matching(toks, j);
                        c + if made_at(toks, c + 1).is_some() { 4 } else { 1 }
                    }
                    _ => j + 1,
                };
            }
            out.push_str("\\[ ");
            out.push_str(&source_text(&toks[i..j]));
            out.push_str(" ]");
            i = j;
        }
    }
    out.push('"');
    out
}

impl Guard {
    /// The guard as a message names it.
    fn name(&self) -> String {
        match &self.test {
            Test::Quote(q) => format!("the guard `{}`", source_text(q)),
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
        Val::Ratio(t) => prims::show_ratio(t),
        Val::Float(x) => format!("{x:?}"),
        Val::Str(s) => format!("{s:?}"),
        Val::Name(s, ..) => {
            let mut out = String::new();
            crate::show::symbol(s, &mut out);
            out
        }
        Val::Seq(es) => {
            let es: Vec<String> = es.iter().map(value_text).collect();
            format!("( {} )", es.join(" "))
        }
        _ => "a value".into(),
    }
}

/// How many of these values have cells on the machine's stack.
fn cells(stack: &[Jdg]) -> usize {
    stack.iter().filter(|j| !j.known()).count()
}

/// An open array, map or string literal: its bracket, where its elements
/// start, the floor outside it, and where in the code its mark is. No
/// literal is sealed (doc/design/SYMBOLIC-FORTH.md 14), so `low` is the
/// deepest its code has reached, where its elements start once it closes,
/// and `cells` how many values below its start had cells at an array's or
/// a map's mark.
struct Literal {
    bracket: u8,
    start: usize,
    floor: usize,
    mark: usize,
    low: usize,
    cells: usize,
    /// How deep the stage is in words where its own code runs, so a `;`
    /// there makes one of its entries (TYPES.md 2.11).
    depth: u32,
}

impl Literal {
    /// Where its elements start: its start, or as deep as its code reached.
    fn from(&self) -> usize {
        self.start.min(self.low)
    }
}

/// The inputs of a bracket being applied (doc/design/SYMBOLIC-FORTH.md
/// 14): its letters, by their tokens' addresses, in the order they run, and
/// the places they name; the values at those places, taken when it was
/// applied; and the places of those known only at run time, which wait on
/// the scratch stack, the one used last at the bottom, as `dip`'s value
/// does. The first `given` letters were put on the stack as it was applied.
struct Inputs {
    owns: Vec<usize>,
    places: Vec<usize>,
    vals: Vec<Option<Jdg>>,
    slots: Vec<usize>,
    given: usize,
}

/// A letter applied by `var#`, `a var# .`, at `i`: its letter.
fn letter_at(toks: &[Token], i: usize) -> Option<u8> {
    match (&toks[i].tok, toks.get(i + 1), toks.get(i + 2)) {
        (Tok::Name(l), Some(v), Some(a))
            if read::is_variable(l)
                && matches!(&v.tok, Tok::Name(v) if is_var(v))
                && a.tok == Tok::Apply =>
        {
            Some(l.as_bytes()[0])
        }
        _ => None,
    }
}

/// The place a letter names, from the top (doc/design/SYMBOLIC-FORTH.md
/// 14): `a` the first, and `x`, `y` and `z` the first three, as math names
/// them, so `w` is the twenty-third.
fn place(l: u8) -> usize {
    match l {
        b'x'..=b'z' => (l - b'x') as usize + 1,
        l => (l - b'a') as usize + 1,
    }
}

/// A bracket's letters, in the order written: each one's token's address,
/// the place it names, and where it is. Those of a bracket inside it, a
/// string among them, are that bracket's.
fn letters_in(toks: &[Token]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let Tok::Open(_) = toks[i].tok {
            i = matching(toks, i);
        } else if let Some(l) = letter_at(toks, i) {
            out.push((&toks[i] as *const Token as usize, place(l), i));
            i += 2;
        }
        i += 1;
    }
    out
}

/// Code to run after other code, as composing does: as it is, or, if it
/// names its inputs, a quotation applied, `[ … ] .`, so that its letters
/// are its own (doc/design/SYMBOLIC-FORTH.md 14).
fn applied(code: &[Token], at: Pos) -> Vec<Token> {
    if letters_in(code).is_empty() {
        return code.to_vec();
    }
    let tok = |tok| Token { tok, pos: at };
    std::iter::once(tok(Tok::Open(b'[')))
        .chain(code.iter().cloned())
        .chain([tok(Tok::Close(b']')), tok(Tok::Apply)])
        .collect()
}

/// How many inputs a bracket takes for its letters: the place of the
/// deepest it names.
fn inputs_of(toks: &[Token]) -> usize {
    letters_in(toks).iter().map(|u| u.1).max().unwrap_or(0)
}

/// The ordinal of a place, for a message.
fn ordinal(n: usize) -> String {
    match n {
        1 => "first".into(),
        2 => "second".into(),
        3 => "third".into(),
        n if n % 10 == 1 && n != 11 => format!("{n}st"),
        n if n % 10 == 2 && n != 12 => format!("{n}nd"),
        n if n % 10 == 3 && n != 13 => format!("{n}rd"),
        n => format!("{n}th"),
    }
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
        self.pattern(k).is_some()
    }
    /// The value input `k` must equal, if it is a value pattern.
    fn pattern(&self, k: usize) -> Option<&Jdg> {
        self.guards.iter().find_map(|g| match &g.test {
            Test::Equals(j) if g.slot == k => Some(j),
            _ => None,
        })
    }
}

impl Clause {
    fn guarded(&self) -> bool {
        self.sig.as_ref().is_some_and(|s| !s.guards.is_empty())
    }
    /// Whether it takes a trap last, as a handler does
    /// (doc/design/SYMBOLIC-FORTH.md 29).
    fn handles(&self) -> bool {
        self.sig
            .as_ref()
            .is_some_and(|s| s.ins.last() == Some(&TRAP))
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
    /// A bracket, a value: converted by, it types as many values as it has
    /// slots.
    Bracket(usize, usize),
    /// A type, given by its name.
    Type,
    /// An integer, a count for a power or a width.
    Int(i128),
    /// Code whose effect is known: a power or a width, made.
    Code(usize, usize),
    Other,
}

/// What an error making a `have` a `t` adds: a float becomes an integer
/// only by a word that says how it rounds (core/core.march), and a
/// quotation is text only of what its code leaves, by its kind
/// (doc/design/SYMBOLIC-FORTH.md 15).
fn narrowing(t: Type, have: Type) -> &'static str {
    if matches!(t, I64 | INT_LIT) && matches!(have, F64 | DEC_LIT | RATIO_LIT) {
        ": say how it rounds, `round`, `floor`, `ceil` or `trunc`"
    } else if t == STRING && have == QUOTE {
        ": `[ … ] str# !` makes text of what its code leaves, as `\"…\"` is"
    } else {
        ""
    }
}

/// What each value a clause takes costs its score, so that of clauses of
/// different lengths the shortest wins, before how well each matches.
const REACH: i32 = 1 << 12;

/// The most values a family's signatures take.
fn longest(clauses: &[Rc<Clause>]) -> usize {
    clauses
        .iter()
        .filter(|c| !c.handles())
        .filter_map(|c| c.sig.as_ref().map(|s| s.ins.len()))
        .max()
        .unwrap_or(0)
}

/// How many values a clause takes: its signature's inputs, or, without one,
/// as many as the family's longest.
fn length(c: &Clause, longest: usize) -> usize {
    c.sig.as_ref().map_or(longest, |s| s.ins.len())
}

/// The candidate that matches best, the first defined among equals.
fn primary(cands: &[Candidate]) -> Option<Candidate> {
    let top = cands.iter().map(|c| c.0).max()?;
    cands.iter().find(|c| c.0 == top).cloned()
}

/// Shuffles across a value set aside, by `dip`, that the machine does in
/// one operation: `[ dup ] dip swap` is `over`, `[ swap ] dip swap` `rot`.
const FUSED: &[([P; 4], P)] = &[
    ([P::ScratchPush, P::Dup, P::ScratchPop, P::Swap], P::Over),
    ([P::ScratchPush, P::Swap, P::ScratchPop, P::Swap], P::Rot),
];

fn fuse(code: Vec<Ins>) -> Vec<Ins> {
    let prim = |i: &Ins| match i {
        Ins::Op(Op::Prim(p)) => Some(*p),
        _ => None,
    };
    let mut out: Vec<Ins> = Vec::with_capacity(code.len());
    for ins in code {
        out.push(ins);
        let n = out.len();
        // A value set aside and taken straight back, as a bracket's
        // argument used first is, never moved.
        if n >= 2
            && prim(&out[n - 2]) == Some(P::ScratchPush)
            && prim(&out[n - 1]) == Some(P::ScratchPop)
        {
            out.truncate(n - 2);
            continue;
        }
        if n < 4 {
            continue;
        }
        let tail: Vec<_> = out[n - 4..].iter().map(prim).collect();
        if let Some((_, op)) = FUSED
            .iter()
            .find(|(seq, _)| seq.iter().zip(&tail).all(|(a, b)| Some(*a) == *b))
        {
            out.truncate(n - 4);
            out.push(Ins::Op(Op::Prim(*op)));
        }
    }
    out
}

/// The rolls that put values in the order `after`, from `before`, both
/// from the bottom: from the first out of place, each in turn brought up
/// to the top, above those placed already. A roll is how many values it
/// turns, the one brought up the deepest.
fn rolls(before: &[usize], after: &[usize]) -> Vec<usize> {
    let mut now = before.to_vec();
    let placed = now.iter().zip(after).take_while(|(a, b)| a == b).count();
    let mut out = Vec::new();
    for x in &after[placed..] {
        let at = now.iter().position(|v| v == x).expect("a permutation");
        out.push(now.len() - at);
        now.remove(at);
        now.push(*x);
    }
    out
}

/// How many operations a roll of `w` values takes (`Stage::roll`).
fn roll_cost(w: usize) -> usize {
    match w {
        0 | 1 => 0,
        2 | 3 => 1,
        w => 3 * (w - 3) + 1,
    }
}

/// Whether a name is a path in a namespace, `io.file.read`: names joined by
/// dots, the last of which may be a pattern, `logs./log_([0-9]+)/`.
fn is_path(name: &str) -> bool {
    let path = match pattern_at(name) {
        Some(0) => return false,
        Some(p) => &name[..p - 1],
        None if name.contains('.') => name,
        None => return false,
    };
    path.split('.').all(|p| !p.is_empty())
}

/// Where a rule's pattern starts in its name: `/…/`, after the path of its
/// namespace if it has one (doc/design/SYMBOLIC-FORTH.md 25).
fn pattern_at(name: &str) -> Option<usize> {
    let p = name.find('/')?;
    (name.len() - p > 2 && name.ends_with('/') && (p == 0 || name[..p].ends_with('.'))).then_some(p)
}

/// A name's namespace and its last part, `io.file` and `read`: a pattern's
/// dots are its own, so `logs./log.(.+)/` is `logs` and `/log.(.+)/`.
fn split_path(name: &str) -> (&str, &str) {
    match pattern_at(name) {
        Some(0) => ("", name),
        Some(p) => (&name[..p - 1], &name[p..]),
        None => name.rsplit_once('.').unwrap_or(("", name)),
    }
}

/// A pattern as an error names it, by its namespace.
fn whose(space: &str, key: &str) -> String {
    if space.is_empty() {
        format!("the root's `{key}`")
    } else {
        format!("`{space}`'s `{key}`")
    }
}

/// Whether a value is code, a quotation or a word, which a width does not
/// widen.
fn is_code(j: &Jdg) -> bool {
    match &j.val {
        Val::Name(..) => true,
        Val::Quote(q) => !signature(q),
        _ => false,
    }
}

/// Whether two values are the same code, written once.
fn same_code(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Quote(x), Val::Quote(y)) => Rc::ptr_eq(x, y),
        (Val::Name(x, ..), Val::Name(y, ..)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// The effect of code's n-th power, by its kind (doc/design/SYMBOLIC-FORTH.md
/// 21), if it has one.
fn power_effect((k, m): (usize, usize), n: usize) -> Option<(usize, usize)> {
    match (k, m) {
        // The 0th power of a value or an operation is an identity, a value.
        (0, 1) | (2.., 1) if n == 0 => Some((0, 1)),
        (0, 1) => Some((0, 1)),
        (2.., 1) => (n - 1).is_multiple_of(k - 1).then_some((n, 1)),
        _ => Some((0..n).fold((0, 0), |(a, b), _| {
            if k <= b {
                (a, b - k + m)
            } else {
                (a + k - b, m)
            }
        })),
    }
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
    /// Its signature as written, each value a type or a bracket: none, one,
    /// or two, the inputs then the outputs.
    pub written: Rc<[Jdg]>,
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
    /// Defined by the program, not the core vocabulary alone.
    user: bool,
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
    /// The inputs of the brackets being applied, the innermost last.
    inputs: Vec<Inputs>,
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
    /// Set while compiling code that may not run, a choice's alternative
    /// or a loop's body: there `fail` fails only when the program runs.
    unsure: u32,
    /// Set while compiling the body of a word with a handler for its
    /// inputs: a failure there is caught as the program runs, so it fails
    /// then (doc/design/SYMBOLIC-FORTH.md 29).
    handled: u32,
    /// Set for the dispatch among a family's handlers, which `resolve`
    /// takes: it chooses among those alone, and none of them is chosen
    /// otherwise.
    catching: bool,
    /// The families whose handlers are being compiled, with their inputs.
    catches: Vec<(Rc<str>, Vec<Type>)>,
    /// The machine primitives are folded on, and `[ … ] !` runs on, made
    /// when first needed: it holds all the code sealed.
    machine: Option<Machine>,
    /// What each quotation `[ … ] !` ran left, by its code's identity, so
    /// each runs once.
    ran: HashMap<Cid, Rc<[Jdg]>>,
    /// What code run while compiling wrote, for the session to show.
    pub output: Vec<u8>,
    /// Set while a word is applied wide (doc/design/SYMBOLIC-FORTH.md 22).
    wide: Option<Wide>,
    /// Where the name a rule rewrites was applied, a namespace, empty for
    /// the root, which `name#` reads (doc/design/SYMBOLIC-FORTH.md 25).
    applied_in: Rc<str>,
    /// The names a source defines, known while it is expanded, so that a
    /// namespace's own later words count wherever they are written (16).
    pending: HashSet<Rc<str>>,
    /// The namespaces that hold words, by their paths.
    spaces: HashSet<Rc<str>>,
    /// Set while a typed clause is checked as it is defined.
    checking: u32,
    /// Each namespace's import and export: the paths they name.
    imports: HashMap<Rc<str>, Rc<[Rc<str>]>>,
    exports: HashMap<Rc<str>, Rc<[Rc<str>]>>,
    /// Each namespace's rules, by their patterns, and the core's under
    /// `CORE_RULES`.
    patterns: HashMap<Rc<str>, Patterns>,
}

/// Where the core's rules are kept among the namespaces' (doc/design/
/// SYMBOLIC-FORTH.md 25): a key no namespace has, since a namespace's
/// parts are names.
const CORE_RULES: &str = ".";

/// The core's namespace.
const CORE: &str = "core";

/// A rule that rewrites a name: where its patterns are kept, a namespace or
/// `CORE_RULES`, and its place among them.
type Rule = (Rc<str>, usize);

/// A namespace's patterns (doc/design/SYMBOLIC-FORTH.md 25): as one set, to
/// match a word against all at once, and each one.
struct Patterns {
    set: RegexSet,
    each: Vec<Pattern>,
}

/// A pattern, as written, `/log_([0-9]+)/`, and as matched, the whole word,
/// and the word it names, its clauses a family.
struct Pattern {
    key: Rc<str>,
    re: Regex,
    word: Rc<str>,
}

/// A width being applied: how many values make a group, and the code
/// handed to the word widened, which runs as written.
struct Wide {
    n: usize,
    outside: Vec<Val>,
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a quotation is a bracket alone, as a bracket in code is pushed.
fn signature(q: &[Token]) -> bool {
    q.first().is_some_and(|t| t.tok == Tok::Open(b'<')) && matching(q, 0) + 1 == q.len()
}

/// A bracket in a bracket that is no one type, `< a ary < a b > >`, would
/// be the type of a quotation, an input or output that is code.
fn quotation_type(q: &[Token], pos: Pos) -> Error {
    Error::new(
        Kind::Limit,
        Some(pos),
        format!(
            "`{}` in a bracket would type a quotation, which is not built yet",
            source_text(q)
        ),
    )
}

/// The kind a quotation closed at `j - 1` is made by `!` if it is, `[ … ]
/// seq# !`, `map# !`, `sig# !` or `str# !`, as High Stack's brackets and
/// strings are read: its name, `seq#`.
fn made_at(toks: &[Token], j: usize) -> Option<&str> {
    match toks
        .get(j..j + 3)
        .map(|t| [&t[0].tok, &t[1].tok, &t[2].tok])
    {
        Some([Tok::Name(k), Tok::Name(b), Tok::Apply])
            if matches!(&**k, "seq#" | "map#" | "sig#" | "str#") && &**b == "!" =>
        {
            Some(k)
        }
        _ => None,
    }
}

/// A body's checks (doc/design/SYMBOLIC-FORTH.md 6.32), each a bracket made
/// a signature and applied, `[ i64 . ] sig# ! .`: the one it begins with,
/// its inputs, and, if it has that, the one it ends with, what it leaves,
/// each as the code that makes it; and the body between them.
type Checks = (
    Option<std::ops::Range<usize>>,
    Option<std::ops::Range<usize>>,
    std::ops::Range<usize>,
);

fn checks(q: &[Token]) -> Checks {
    // Where a check that begins at `i` ends.
    let check = |i: usize| -> Option<usize> {
        let Tok::Open(_) = q.get(i)?.tok else {
            return None;
        };
        let j = matching(q, i);
        let applied = q.get(j + 4).is_some_and(|t| t.tok == Tok::Apply);
        (made_at(q, j + 1) == Some("sig#") && applied).then_some(j + 5)
    };
    let Some(start) = check(0) else {
        return (None, None, 0..q.len());
    };
    let mut i = start;
    while i < q.len() {
        if let Tok::Open(_) = q[i].tok {
            if check(i) == Some(q.len()) {
                return (Some(0..start - 1), Some(i..q.len() - 1), start..i);
            }
            i = matching(q, i);
        }
        i += 1;
    }
    (Some(0..start - 1), None, start..q.len())
}

/// Where a definition that starts at `i` ends, at its `;`: a name, then
/// its signature and body, brackets, or one value, then `;`.
fn definition(toks: &[Token], i: usize) -> Option<usize> {
    let Tok::Name(_) = toks[i].tok else {
        return None;
    };
    let mut j = i + 1;
    while toks.get(j).is_some_and(|t| matches!(t.tok, Tok::Open(_))) {
        j = matching(toks, j) + 1;
        if made_at(toks, j).is_some() {
            j += 3;
        }
    }
    let value = |t: &Token| {
        matches!(
            t.tok,
            Tok::Name(_) | Tok::Int(_) | Tok::Dec(..) | Tok::Ratio(_)
        )
    };
    if j == i + 1 && toks.get(j).is_some_and(value) {
        j += 1;
    }
    let semicolon = j > i + 1
        && toks.get(j).is_some_and(|t| t.tok == Tok::Name(";".into()))
        && toks.get(j + 1).is_some_and(|t| t.tok == Tok::Apply);
    semicolon.then_some(j)
}

/// The words a source defines, by their names.
fn definitions(toks: &[Token]) -> impl Iterator<Item = Rc<str>> + '_ {
    (0..toks.len()).filter_map(move |i| {
        let Tok::Name(n) = &toks[i].tok else {
            return None;
        };
        let applied = toks
            .get(i + 1)
            .is_some_and(|t| matches!(t.tok, Tok::Apply | Tok::Test));
        if applied || !names(n) {
            return None;
        }
        definition(toks, i)?;
        let key = split_path(n).1;
        (pattern_at(key) != Some(0) && !matches!(key, "import#" | "export#")).then(|| n.clone())
    })
}

/// Whether a name applied names a word: not a meta-op, `!` or `;`, nor a
/// variable.
fn names(n: &str) -> bool {
    !matches!(n, "!" | ";") && !read::is_variable(n)
}

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

/// A core word's path, `core.dup`.
fn core_path(name: &str) -> Rc<str> {
    format!("{CORE}.{name}").into()
}

/// Whether a word found is the core's of that name.
fn is_core_word(found: &str, name: &str) -> bool {
    found.strip_prefix(CORE).and_then(|p| p.strip_prefix('.')) == Some(name)
}

/// Steps code run while compiling may take, `[ … ] !`'s.
const FUEL: u64 = 10_000_000;

/// A value the machine left, of a type, as the stage knows one: a number
/// by its cell, a string or a symbol by its text, and an array or a tuple by
/// its elements. Others it does not know yet.
fn known_of(m: &Machine, types: &Types, c: u64, t: Type) -> Option<Val> {
    let elements = |es: Vec<Type>| -> Option<Val> {
        let seq = m.sequence(c).ok()?;
        let es = es.into_iter().cycle().take(seq.len());
        let vals = seq
            .iter()
            .zip(es)
            .map(|(&x, ty)| {
                Some(Jdg {
                    ty,
                    val: known_of(m, types, x, ty)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Val::Seq(vals.into()))
    };
    Some(match types.term(t) {
        Term::Base(Base::I64 | Base::Money | Base::Bool | Base::Nil) => Val::Int((c as i64).into()),
        Term::Base(Base::F64) => Val::Float(f64::from_bits(c)),
        Term::Base(Base::String) => Val::Str(m.text(c).ok()?.to_text()?.into()),
        Term::Base(Base::Symbol) => Val::Name(m.text(c).ok()?.to_text()?.into(), Pos::default()),
        Term::Ary(e) | Term::Vec(_, e) => elements(vec![e])?,
        Term::Tuple(_) => elements(types.elements(t)?.to_vec())?,
        _ => return None,
    })
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
            inputs: Vec::new(),
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
            unsure: 0,
            handled: 0,
            catching: false,
            catches: Vec::new(),
            machine: None,
            ran: HashMap::new(),
            output: Vec::new(),
            wide: None,
            applied_in: "".into(),
            pending: HashSet::new(),
            spaces: HashSet::new(),
            checking: 0,
            imports: HashMap::new(),
            exports: HashMap::new(),
            patterns: HashMap::new(),
        };
        // The core is the namespace `core`, which every namespace finds
        // last, as though imported (doc/design/SYMBOLIC-FORTH.md 16).
        for (i, (_, name)) in Base::ALL.iter().enumerate() {
            let name = core_path(name);
            s.word(&name).ty = Some(i as Type);
            s.type_names.insert(i as Type, name);
        }
        for (con, name) in [
            (Con::Ary, "ary"),
            (Con::Vec, "vec"),
            (Con::Map, "map"),
            (Con::Or, "or"),
            (Con::Trap, "trap"),
            (Con::Own, "own"),
        ] {
            s.word(&core_path(name)).con = Some(con);
        }
        let atom = s.types.intern(Term::Atom);
        s.word(&core_path("atom")).ty = Some(atom);
        let tuple = s.types.intern(Term::AnyTuple);
        s.word(&core_path("tuple")).ty = Some(tuple);
        let record = s.types.intern(Term::AnyRecord);
        s.word(&core_path("record")).ty = Some(record);
        let none = crate::symbols::Symbols::parse("").expect("an empty table");
        for p in PRIMS {
            let toks = crate::read::read(p.sig, &none).expect("a primitive's signature reads");
            // Its inputs, and its outputs, each a check, `[ … ] sig# ! .`
            // (doc/design/SYMBOLIC-FORTH.md 6.32).
            let end = matching(&toks, 0);
            let mut sig = s
                .bracket(&toks[1..end])
                .expect("a primitive's signature is well formed");
            let outs = end + 5;
            if outs < toks.len() {
                sig.outs = Some(
                    s.outputs(&toks[outs + 1..matching(&toks, outs)])
                        .expect("a primitive's outputs are types"),
                );
            }
            s.word(&core_path(p.name)).prim = Some((p.prim, Rc::new(sig)));
        }
        s
    }

    fn word(&mut self, name: &Rc<str>) -> &mut Word {
        if !self.words.contains_key(name) {
            let mut space = split_path(name).0;
            while !space.is_empty() {
                self.spaces.insert(space.into());
                space = space.rfind('.').map_or("", |i| &space[..i]);
            }
        }
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
        self.inputs.clear();
        self.active.clear();
        self.frames.clear();
        self.effects = 0;
        self.steps = 0;
        self.depth = 0;
        self.applied_in = "".into();
    }

    /// Ends a compilation: every value left is made a value at run time, and
    /// their types are returned.
    pub fn finish(&mut self) -> Result<Vec<Type>> {
        for i in 0..self.stack.len() {
            let t = self.stack[i].ty;
            if self.types.is_record_lit(t) {
                let m = self.lit_map(t);
                self.annotate(i, m)?;
            }
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

    /// Checks that `n` values are within reach, and notes that the code
    /// reached them, so a literal knows how deep its code reached.
    fn need(&mut self, n: usize) -> Result<()> {
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
        self.reached(self.stack.len() - n);
        Ok(())
    }

    /// Notes that the code reached the value at `at`, for the innermost
    /// literal open.
    fn reached(&mut self, at: usize) {
        if let Some(l) = self.literals.last_mut() {
            l.low = l.low.min(at);
        }
    }

    fn pop(&mut self) -> Result<Jdg> {
        self.need(1)?;
        Ok(self.stack.pop().expect("checked"))
    }

    /// Evaluates code, its names expanded where it was written.
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
                Tok::Ratio(t) => self.push(RATIO_LIT, Val::Ratio(t.clone())),
                // A letter in code, `a var# .`, is an input by its place, of
                // the bracket it is written in (doc/design/SYMBOLIC-FORTH.md
                // 14); in a signature, run over types, it is a type.
                Tok::Name(s) if self.in_bracket == 0 && letter_at(toks, i).is_some() => {
                    self.letter(t as *const Token as usize, s.as_bytes()[0] as char)?;
                    i += 2;
                }
                Tok::Name(s) => self.push(SYMBOL, Val::Name(s.clone(), t.pos)),
                Tok::Apply => self.apply()?,
                Tok::Test => self.apply_test()?,
                Tok::Open(b'[') => {
                    let j = matching(toks, i);
                    let q: Rc<[Token]> = toks[i + 1..j].into();
                    self.push(QUOTE, Val::Quote(q));
                    i = j;
                }
                // The reader writes High Stack's brackets as Low Stack's
                // one, and the stage keeps its own `<` inside a signature's
                // value only.
                Tok::Open(_) | Tok::Close(_) => {
                    return Err(self.err(
                        Kind::Syntax,
                        "Low Stack's one bracket is `[ … ]`, made data by `!`",
                    ));
                }
            }
            i += 1;
        }
        Ok(())
    }

    /// `.`: applies the value on top.
    fn apply(&mut self) -> Result<()> {
        let f = self.pop()?;
        if self.wide.is_some() && is_code(&f) {
            return self.wide_apply(f);
        }
        match f.val {
            Val::Name(n, at) => {
                self.pos = at;
                self.apply_name(&n, at)
            }
            // A type applied checks that the values on top have it, where it
            // is applied (doc/design/SYMBOLIC-FORTH.md 6.32): `< i64 >.`, a
            // body's first check being its signature.
            Val::Type(t) => {
                let t = self.named(t)?;
                self.annotate_top(&Sig {
                    ins: vec![t],
                    guards: Vec::new(),
                    outs: None,
                })
            }
            Val::Quote(q) if signature(&q) => {
                let sig = self.bracket(&q[1..q.len() - 1])?;
                self.annotate_top(&sig)
            }
            Val::Quote(q) => self.run_code(&q),
            // Which word a name applies is chosen while compiling.
            Val::Run if f.ty == SYMBOL => Err(self.err(
                Kind::Mismatch,
                "a symbol known only at run time cannot be applied: \
                 the word a name applies is chosen while compiling",
            )),
            _ => Err(self.err(
                Kind::Mismatch,
                format!("{} cannot be applied", self.describe(&f)),
            )),
        }
    }

    /// `convert`, written `!` (Thomas, 2026-10-13): the value below given the
    /// type on top. A type converts it, `2 i64!`, `"42" i64!`, `x 3 i64 vec
    /// !`, by the clauses under its name; a class or a type with variables
    /// checks it; a signature, a bracket of more than one type, gives the
    /// values below its types, `x y < i64 f64 > !`.
    fn convert_by(&mut self) -> Result<()> {
        let f = self.pop()?;
        let named = match &f.val {
            Val::Name(n, _) => self
                .words
                .get(&self.resolve_quiet("", n))
                .and_then(|w| w.ty),
            _ => None,
        };
        let t = match f.val {
            Val::Type(t) => self.named(t)?,
            Val::Name(..) if named.is_some() => named.expect("checked"),
            Val::Quote(q) if signature(&q) => {
                let sig = self.bracket(&q[1..q.len() - 1])?;
                return self.annotate_top(&sig);
            }
            _ if f.ty == NIL => NIL,
            // A quotation on top is run while compiling, and its values are
            // made data (doc/design/SYMBOLIC-FORTH.md 6.32).
            Val::Quote(q) => return self.now(q),
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`!` structures by a type, and {} is not one",
                        self.describe(&f)
                    ),
                ));
            }
        };
        self.need(1)?;
        if matches!(t, SEQ_LIT | MAP_LIT | SIG_LIT | STR_LIT) {
            let at = match f.val {
                Val::Name(_, at) => at,
                _ => self.pos,
            };
            return self.made(t, at);
        }
        if self.types.is_pattern(t) {
            let sig = Sig {
                ins: vec![t],
                guards: Vec::new(),
                outs: None,
            };
            self.annotate_top(&sig)
        } else {
            self.convert(self.stack.len() - 1, t)
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
            // The source's own word, which it applies wherever it is
            // written, before it is defined too (doc/design/SYMBOLIC-FORTH.md
            // 16).
            if self.pending.contains(name) {
                let core = match self.words.contains_key(&core_path(name)) {
                    true => format!("; the core's is `{CORE}.{name}`"),
                    false => String::new(),
                };
                // A clause checked as it is defined waits for it, as for a
                // value known only later.
                let kind = match self.checking {
                    0 => Kind::NoWord,
                    _ => Kind::Unknown,
                };
                return Err(self.err(
                    kind,
                    format!(
                        "no word `{name}` yet: the source defines its own `{name}`, and \
                         applies it here, before it is defined{core}"
                    ),
                ));
            }
            if let Some((space, last)) = name.rsplit_once('.')
                && is_path(name)
            {
                let prefix = format!("{space}.");
                let msg = if self.words.keys().any(|k| k.starts_with(&prefix)) {
                    // A name written there, found nowhere, was taken as the
                    // namespace's own: the program's root is not the core.
                    let root = match self.words.get(last) {
                        Some(w) if w.user => format!(
                            ", its import, or the core: the root's `{last}` is the program's, \
                             which a namespace imports to use"
                        ),
                        _ => String::new(),
                    };
                    format!("no word `{last}` in `{space}`{root}")
                } else {
                    format!(
                        "no word `{name}`: there is no namespace `{space}`, and `{}` applies \
                         each in turn",
                        name.split('.').collect::<Vec<_>>().join(" ")
                    )
                };
                return Err(self.err(Kind::NoWord, msg));
            }
            let why = self.typo(name);
            return Err(self.err(Kind::NoWord, format!("no word `{name}`{why}")));
        };
        // A word that builds types and does something else too, `map` and
        // `or`, is a family: its constructor is the clause that takes types,
        // chosen as any clause is, by what it takes.
        if let Some(c) = w.con {
            let alone = w.prim.is_none() && w.clauses.is_empty();
            if alone || self.constructs(c) {
                return self.construct(c);
            }
        }
        let w = &self.words[name];
        if let Some((p, sig)) = w.prim.clone() {
            self.prim(name, p, &sig)
        } else if w.ty == Some(NIL) {
            // A type with one value is that value, `nil`, which stands for
            // its type where a type is wanted.
            self.push(NIL, Val::Int(0));
            Ok(())
        } else if let Some(t) = w.ty {
            // A type's name is a word that gives the type, as any word that
            // builds a value gives it (Thomas, 2026-10-12): types are
            // structure, made by words. `!` structures by the type, `2 i64!`.
            self.push(TYPE, Val::Type(t));
            Ok(())
        } else if !w.clauses.is_empty() {
            self.family(name)
        } else {
            Err(self.err(Kind::NoWord, format!("no word `{name}`")))
        }
    }

    // ---- Namespaces (doc/design/SYMBOLIC-FORTH.md 16, 17) ----

    /// The word a name applies from the root: a path is itself; a bare
    /// name is the root's own, its import's, or the core's.
    fn lookup(&mut self, name: &Rc<str>) -> Result<Rc<str>> {
        self.find("", name)
    }

    /// The word a name applies, written in a namespace: a path is itself; a
    /// bare name is looked up in the namespace, then among its import, then
    /// in the core, and found in two of the first is an error.
    fn find(&self, ns: &str, name: &Rc<str>) -> Result<Rc<str>> {
        if is_path(name) {
            return Ok(name.clone());
        }
        let local = self.local(ns, name);
        let imported = self.imported(ns, name);
        match (local, &imported[..]) {
            (Some(l), []) => Ok(l),
            (None, [i]) => Ok(i.clone()),
            (None, []) if self.in_core(name).is_some() => Ok(core_path(name)),
            (None, []) if ns.is_empty() || !self.words.get(name).is_some_and(|w| w.user) => {
                Ok(name.clone())
            }
            (None, []) => Err(self.err(
                Kind::NoWord,
                format!("no word `{name}` in `{ns}`, its import, or the core"),
            )),
            (l, _) => {
                let all: Vec<String> = l
                    .iter()
                    .map(|p| format!("`{p}`, defined here"))
                    .chain(imported.iter().map(|p| format!("`{p}`, imported")))
                    .collect();
                Err(self.err(
                    Kind::NoWord,
                    format!(
                        "`{name}` names both {}: a path says which",
                        all.join(", and ")
                    ),
                ))
            }
        }
    }

    /// The word a name applies from a namespace, as `lookup` finds it, or
    /// the name itself where that would be an error.
    fn resolve_quiet(&self, ns: &str, name: &Rc<str>) -> Rc<str> {
        if is_path(name) {
            return name.clone();
        }
        let imported = self.imported(ns, name);
        match (self.local(ns, name), &imported[..]) {
            (Some(l), []) => l,
            (None, [i]) => i.clone(),
            (None, []) => self.in_core(name).unwrap_or_else(|| name.clone()),
            _ => name.clone(),
        }
    }

    /// The shortest name that applies a word from the root: its last part,
    /// if the root finds the word by it, as it finds the core's, else its
    /// path. A symbol is global text, so a name taken out of code, which
    /// applied from the stack is found from the root, keeps what it meant.
    fn short(&self, path: &Rc<str>) -> Rc<str> {
        if !is_path(path) {
            return path.clone();
        }
        let last: Rc<str> = split_path(path).1.into();
        if self.resolve_quiet("", &last) == *path {
            last
        } else {
            path.clone()
        }
    }

    /// A message with the core's words named as the root finds them, `dup`
    /// for `core.dup`, but not a name a definition says, `core.unit:`.
    fn shorten(&self, msg: &str) -> String {
        let prefix = format!("{CORE}.");
        let mut out = String::with_capacity(msg.len());
        let mut at = 0;
        while let Some(k) = msg[at..].find(&prefix) {
            let i = at + k;
            let end = msg[i..]
                .find(|c: char| c.is_whitespace() || c == '`')
                .map_or(msg.len(), |e| i + e);
            let starts = msg[..i]
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace() || "`'\"([<,".contains(c));
            out.push_str(&msg[at..i]);
            let chunk = &msg[i..end];
            let mut name = chunk;
            loop {
                if !starts || chunk.ends_with(':') {
                    out.push_str(chunk);
                    break;
                }
                if self.words.contains_key(name) {
                    out.push_str(&self.short(&name.into()));
                    out.push_str(&chunk[name.len()..]);
                    break;
                }
                // A name, less what follows it in a message.
                match name.char_indices().next_back() {
                    Some((j, c)) if "',.);".contains(c) => name = &name[..j],
                    _ => {
                        out.push_str(chunk);
                        break;
                    }
                }
            }
            at = end;
        }
        out.push_str(&msg[at..]);
        out
    }

    /// An error with the core's words named as the root finds them.
    fn shorten_error(&self, mut e: Error) -> Error {
        e.msg = self.shorten(&e.msg);
        for t in &mut e.trace {
            *t = self.shorten(t);
        }
        e
    }

    /// Runs a source: its imports first, wherever they are written, since an
    /// import holds for all of its namespace (doc/design/SYMBOLIC-FORTH.md
    /// 17); and its patterns known first, as they hold for all of theirs, so
    /// no word reads one way before its pattern and another after (25). Then
    /// it is expanded, each name it applies made its path where it was
    /// written, its own later words counting (16), and run.
    pub fn run_source(&mut self, toks: &[Token]) -> Result<()> {
        self.pending = definitions(toks).map(|n| self.defined(&n)).collect();
        // Its messages name the core's words as the root finds them, its
        // own words counting.
        let r = self.run_expanded(toks).map_err(|e| self.shorten_error(e));
        self.pending.clear();
        r
    }

    fn run_expanded(&mut self, toks: &[Token]) -> Result<()> {
        let mut imports = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            match &toks[i].tok {
                Tok::Name(n)
                    if matches!(split_path(n).1, "import#" | "export#")
                        || pattern_at(split_path(n).1) == Some(0) =>
                {
                    let mut j = i + 1;
                    while toks.get(j).is_some_and(|t| matches!(t.tok, Tok::Open(_))) {
                        j = matching(toks, j) + 1;
                    }
                    let defined = j > i + 1
                        && toks.get(j).is_some_and(|t| t.tok == Tok::Name(";".into()))
                        && toks.get(j + 1).is_some_and(|t| t.tok == Tok::Apply);
                    if !defined {
                        i += 1;
                    } else if matches!(split_path(n).1, "import#" | "export#") {
                        imports.push(i..j + 2);
                        i = j + 2;
                    } else {
                        self.pos = toks[i].pos;
                        let n = self.defined(n);
                        let (space, key) = split_path(&n);
                        let space: Rc<str> = space.into();
                        self.pattern(&space, key)?;
                        i = j + 2;
                    }
                }
                Tok::Open(_) => i = matching(toks, i) + 1,
                _ => i += 1,
            }
        }
        for r in &imports {
            self.run(&toks[r.clone()])?;
        }
        // Expanding renames names, so the tokens keep their places.
        let toks = self.expand(toks, self.home())?;
        let mut at = 0;
        for r in imports {
            self.run(&toks[at..r.start])?;
            at = r.end;
        }
        self.run(&toks[at..])
    }

    // ---- Expansion (doc/design/SYMBOLIC-FORTH.md 16) ----

    /// Code as it means where it was written, in `ns`: each name it applies
    /// made its path, so that it means the same wherever it runs. A symbol
    /// is global text, and is left as it is; so is a meta-op, which is no
    /// name. A definition's signature and body are written in the namespace
    /// of the word it defines.
    fn expand(&mut self, toks: &[Token], ns: &str) -> Result<Vec<Token>> {
        let mut out = Vec::with_capacity(toks.len());
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            let applied = matches!(
                toks.get(i + 1),
                Some(Token {
                    tok: Tok::Apply | Tok::Test,
                    ..
                })
            );
            if let (Tok::Name(n), false, Some(end)) = (&t.tok, applied, definition(toks, i)) {
                out.push(t.clone());
                // One in another's body, an entry in braces, is in its
                // namespace, `ns`, as the source's own are in `home`.
                let n: Rc<str> = match ns {
                    "" => n.clone(),
                    _ if is_path(n) => n.clone(),
                    _ => format!("{ns}.{n}").into(),
                };
                out.extend(self.expand(&toks[i + 1..end], split_path(&n).0)?);
                i = end;
                continue;
            }
            match &t.tok {
                Tok::Name(n) if applied && names(n) => {
                    self.pos = t.pos;
                    out.push(Token {
                        tok: Tok::Name(self.path_of(ns, n)?),
                        pos: t.pos,
                    });
                }
                _ => out.push(t.clone()),
            }
            i += 1;
        }
        Ok(out)
    }

    /// The path of a name applied where it is written, in `ns`: a rule of
    /// the namespace's that matches it applies there, so its path is in
    /// `ns`; a path is itself; a bare name is the namespace's own, its
    /// import's, or the core's; and one found nowhere is the namespace's
    /// own, defined later or never.
    fn path_of(&mut self, ns: &str, name: &Rc<str>) -> Result<Rc<str>> {
        let here = || -> Rc<str> {
            match ns {
                "" => name.clone(),
                _ => format!("{ns}.{name}").into(),
            }
        };
        if self.rule_match(ns, name)?.is_some() {
            return Ok(here());
        }
        if is_path(name) {
            self.reach(ns, name);
            return Ok(name.clone());
        }
        let local = self.local(ns, name);
        let imported = self.imported(ns, name);
        match (local, &imported[..]) {
            (Some(l), []) => Ok(l),
            (None, [i]) => Ok(i.clone()),
            (None, []) if self.in_core(name).is_some() => Ok(core_path(name)),
            (None, []) if ns.is_empty() => Ok(name.clone()),
            (None, []) => Ok(here()),
            _ => self.find(ns, name),
        }
    }

    /// The core's word of a name, which every namespace finds last, as
    /// though it imported `core.*` (doc/design/SYMBOLIC-FORTH.md 16).
    fn in_core(&self, name: &str) -> Option<Rc<str>> {
        let path = core_path(name);
        (self.words.contains_key(&path) || self.pending.contains(&path)).then_some(path)
    }

    /// Where a definition's bare name is defined: the root, or, while the
    /// core is read, the core.
    fn home(&self) -> &'static str {
        if self.core { CORE } else { "" }
    }

    /// The word a definition's name defines: a path is itself, and a bare
    /// name is the home's.
    fn defined(&self, name: &Rc<str>) -> Rc<str> {
        match self.home() {
            "" => name.clone(),
            _ if is_path(name) => name.clone(),
            home => format!("{home}.{name}").into(),
        }
    }

    /// A namespace's own word of that name: in the root, one the program
    /// defined, as a core word is no import's rival.
    fn local(&self, ns: &str, name: &str) -> Option<Rc<str>> {
        if ns.is_empty() {
            if let Some(n) = self.pending.get(name) {
                return Some(n.clone());
            }
            let w = self.words.get_key_value(name)?;
            return w.1.user.then(|| w.0.clone());
        }
        let path: Rc<str> = format!("{ns}.{name}").into();
        (self.words.contains_key(&path) || self.pending.contains(&path)).then_some(path)
    }

    /// What a namespace's import makes of a bare name: a path whose last
    /// part it is, and of `math.*`, `math`'s exported word of that name.
    fn imported(&self, ns: &str, name: &str) -> Vec<Rc<str>> {
        let Some(paths) = self.imports.get(ns) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for p in paths.iter() {
            let path: Rc<str> = match p.strip_suffix(".*") {
                Some(space) if self.exported(space, name) => format!("{space}.{name}").into(),
                Some(_) => continue,
                None if p.rsplit('.').next() == Some(name) => p.clone(),
                None => continue,
            };
            let known = self.words.contains_key(&path) || self.pending.contains(&path);
            if known && !found.contains(&path) {
                found.push(path);
            }
        }
        found
    }

    /// Whether a namespace exports a name: all of them, with no `export#`.
    fn exported(&self, space: &str, name: &str) -> bool {
        self.exports
            .get(space)
            .is_none_or(|e| e.iter().any(|n| &**n == name))
    }

    /// Warns of a path reached from outside its namespace that it does not
    /// export: it compiles, a word being private but not locked out, and a
    /// namespace's children see its unexported words.
    fn reach(&mut self, from: &str, path: &str) {
        let (space, name) = split_path(path);
        if space.is_empty() {
            return;
        }
        let inside = from == space || from.starts_with(&format!("{space}."));
        if !inside && !self.exported(space, name) {
            self.warn(
                Level::Severe,
                format!("`{path}` is reached from outside `{space}`, which does not export it"),
            );
        }
    }

    /// A namespace's import or export set: the paths a quotation names.
    fn set_names(&mut self, space: &Rc<str>, key: &str, q: &[Token]) -> Result<()> {
        let mut names: Vec<Rc<str>> = Vec::new();
        for t in q {
            match &t.tok {
                Tok::Name(n) => names.push(n.clone()),
                Tok::Apply => {}
                _ => {
                    return Err(self.err(
                        Kind::Syntax,
                        format!("`{key}` takes the names it lists, `[| math.* io.file.read |]`"),
                    ));
                }
            }
        }
        let table = if key == "import#" {
            &mut self.imports
        } else {
            &mut self.exports
        };
        if table.contains_key(space) {
            let shown = if space.is_empty() {
                "the root".to_string()
            } else {
                format!("`{space}`")
            };
            return Err(self.err(
                Kind::Syntax,
                format!("{shown} has its `{key}` already: a namespace has one"),
            ));
        }
        table.insert(space.clone(), names.into());
        Ok(())
    }

    /// A rule's pattern, known to its namespace, or the core's rules
    /// (doc/design/SYMBOLIC-FORTH.md 25): a regular expression between
    /// slashes, `/log_([0-9]+)/`, which matches a whole name, and each of
    /// whose groups captures in every match, so the rewrite's effect is
    /// fixed. How many groups it has.
    fn pattern(&mut self, space: &Rc<str>, key: &str) -> Result<usize> {
        let word: Rc<str> = if space.is_empty() {
            key.into()
        } else {
            format!("{space}.{key}").into()
        };
        // The core's rules are tried after everything else (25).
        let rules: Rc<str> = if &**space == CORE {
            CORE_RULES.into()
        } else {
            space.clone()
        };
        if let Some(r) = self.patterns.get(&rules)
            && let Some(p) = r.each.iter().find(|p| p.word == word)
        {
            return Ok(p.re.captures_len() - 1);
        }
        if !space.is_empty() && space.split('.').any(str::is_empty) {
            return Err(self.err(
                Kind::Syntax,
                format!("`{word}` has an empty part: a path's parts are names"),
            ));
        }
        let pattern = key[1..key.len() - 1].replace("\\/", "/");
        let re = Regex::new(&format!("^(?:{pattern})$")).map_err(|e| {
            let e = e.to_string();
            let why = e
                .lines()
                .find_map(|l| l.strip_prefix("error: "))
                .unwrap_or(&e);
            self.err(Kind::Syntax, format!("`{key}` is no pattern: {why}"))
        })?;
        if re.static_captures_len() != Some(re.captures_len()) {
            return Err(self.err(
                Kind::Syntax,
                format!(
                    "`{key}` has a group that may not match: each group captures in every \
                     match, so the rewrite's effect is fixed"
                ),
            ));
        }
        let groups = re.captures_len() - 1;
        let r = self.patterns.entry(rules).or_insert_with(|| Patterns {
            set: RegexSet::empty(),
            each: Vec::new(),
        });
        r.each.push(Pattern {
            key: key.into(),
            re,
            word,
        });
        r.set = RegexSet::new(r.each.iter().map(|p| p.re.as_str())).expect("each compiles");
        Ok(groups)
    }

    /// The namespaces whose rules hold in `ns`, and whether each is `ns`
    /// itself: its own, as a word is applied in its own namespace, and those
    /// its import names by `*`.
    fn ruling<'a>(&'a self, ns: &'a str) -> impl Iterator<Item = (&'a Rc<str>, bool)> + 'a {
        let own = self.patterns.get_key_value(ns).map(|(k, _)| (k, true));
        let paths = self.imports.get(ns).map_or(&[][..], |p| &p[..]);
        own.into_iter().chain(paths.iter().filter_map(move |p| {
            let space = p.strip_suffix(".*")?;
            let (k, _) = self.patterns.get_key_value(space)?;
            (space != ns).then_some((k, false))
        }))
    }

    /// The rule that rewrites a name applied in a namespace
    /// (doc/design/SYMBOLIC-FORTH.md 25): one of the namespace's own, or of
    /// one its import names by `*` and that exports it, at most one, since a
    /// name two match, in one namespace or in two, is an error. So is a name
    /// that is also defined here or imported, as two imports are.
    fn rule(&self, ns: &str, name: &str) -> Result<Option<Rule>> {
        let Some((space, k)) = self.rule_match(ns, name)? else {
            return Ok(None);
        };
        let word = &self.patterns[&space].each[k].word;
        if self.words.get(word).is_none_or(|w| w.clauses.is_empty()) {
            let pattern = whose(&space, &self.patterns[&space].each[k].key);
            return Err(self.err(
                Kind::NoWord,
                format!(
                    "`{name}` matches {pattern}, whose code is not defined yet: a rule \
                     holds for all of its namespace, once its code is defined"
                ),
            ));
        }
        Ok(Some((space, k)))
    }

    /// The rule that matches a name written in a namespace, as `rule` finds
    /// it, whether or not its code is defined yet.
    fn rule_match(&self, ns: &str, name: &str) -> Result<Option<Rule>> {
        let mut found: Vec<Rule> = Vec::new();
        for (space, own) in self.ruling(ns) {
            let r = &self.patterns[space];
            if !r.set.is_match(name) || found.iter().any(|(s, _)| s == space) {
                continue;
            }
            found.extend(
                r.set
                    .matches(name)
                    .into_iter()
                    .filter(|&k| own || self.exported(space, &r.each[k].key))
                    .map(|k| (space.clone(), k)),
            );
        }
        let whose_each = |found: &[Rule]| -> Vec<String> {
            found
                .iter()
                .map(|(space, k)| whose(space, &self.patterns[space].each[*k].key))
                .collect()
        };
        if let [_, _, ..] = &found[..] {
            return Err(self.err(
                Kind::Syntax,
                format!(
                    "`{name}` matches both {}: a name has one meaning",
                    whose_each(&found).join(", and ")
                ),
            ));
        }
        let Some((space, k)) = found.pop() else {
            return Ok(None);
        };
        let pattern = whose_each(&[(space.clone(), k)]).remove(0);
        let named = match (self.local(ns, name), self.imported(ns, name).pop()) {
            (Some(_), _) => Some("defined here".to_string()),
            (None, Some(path)) => Some(format!("imported, `{path}`")),
            (None, None) => None,
        };
        if let Some(named) = named {
            return Err(self.err(
                Kind::Syntax,
                format!("`{name}` is {named}, and matches {pattern}: a name has one meaning"),
            ));
        }
        Ok(Some((space, k)))
    }

    /// The core's rule that rewrites a name, if one does: tried where nothing
    /// else is found, as the core's names are found last (25).
    fn core_rule(&self, name: &str) -> Option<Rule> {
        let r = self.patterns.get(CORE_RULES)?;
        let k = r.set.matches(name).into_iter().next()?;
        Some((CORE_RULES.into(), k))
    }

    /// What a rule's groups capture of a name, in order.
    fn captures(&self, (rules, k): &Rule, name: &str) -> Vec<Rc<str>> {
        let p = &self.patterns[rules].each[*k];
        let caps = p.re.captures(name).expect("it matched");
        caps.iter()
            .skip(1)
            .map(|c| c.expect("each group captures").as_str().into())
            .collect()
    }

    /// A name rewritten by a rule (25): what its groups capture, each a
    /// symbol, then the rule's code, whose names mean what they mean where
    /// the rule is defined. `log_2` is `'2'` and the code. A capture is
    /// global text, as any symbol is; the code reads it as a name where the
    /// name was applied, `site`, by `name#`.
    fn rewrite(&mut self, rule: &Rule, name: &str, at: Pos, site: Rc<str>) -> Result<()> {
        for c in self.captures(rule, name) {
            self.push(SYMBOL, Val::Name(c, at));
        }
        let word = self.patterns[&rule.0].each[rule.1].word.clone();
        self.enter()?;
        let saved = std::mem::replace(&mut self.applied_in, site);
        let r = self.apply_word(&word);
        self.applied_in = saved;
        self.depth -= 1;
        r
    }

    /// `name#`: a symbol as a name written where the name a rule rewrites
    /// was applied, its path (doc/design/SYMBOLIC-FORTH.md 16, 25), so a
    /// rule may apply what it captured as a word there: `dbl^2` in `math`
    /// squares `math.dbl`.
    fn name_of(&mut self) -> Result<()> {
        let j = self.stack.pop().expect("matched");
        let Val::Name(n, at) = j.val else {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`name#` takes a symbol, and {} is not one",
                    self.describe(&j)
                ),
            ));
        };
        let site = self.applied_in.clone();
        let path = self.path_of(&site, &n)?;
        self.push(SYMBOL, Val::Name(self.short(&path), at));
        Ok(())
    }

    /// `a var# .`: a variable of a one-letter name, which its bracket fills
    /// (doc/design/SYMBOLIC-FORTH.md 7): in a signature a type, filled by
    /// matching, `< a a >.`. A letter is read as one, and a letter's
    /// symbol, `a'`, is a value to match, as any symbol is. In code a letter
    /// is an input by its place (6.14), which `run` gives where the letter
    /// is written, so here it is one made otherwise.
    fn var(&mut self) -> Result<()> {
        let j = self.stack.pop().expect("matched");
        let n = match &j.val {
            Val::Name(n, _) if read::is_variable(n) => n.clone(),
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`var#` takes a letter, `a` to `z`, and {} is not one",
                        self.describe(&j)
                    ),
                ));
            }
        };
        if self.in_bracket == 0 {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "a letter is an input of the bracket it is written in, `{n} var# .`, \
                     and this `{n}` was not written before `var#`"
                ),
            ));
        }
        let v = self.types.intern(Term::Var(n.as_bytes()[0] - b'a'));
        self.push(TYPE, Val::Type(v));
        Ok(())
    }

    /// Where a name was applied, by its path: the longest part before a dot
    /// that is a namespace, and the rest, which a rule there may match; the
    /// root for a bare name.
    fn site_of(&self, name: &Rc<str>) -> (Rc<str>, Rc<str>) {
        let mut end = name.len();
        while let Some(i) = name[..end].rfind('.') {
            let space = &name[..i];
            if self.is_space(space) {
                return (space.into(), name[i + 1..].into());
            }
            end = i;
        }
        ("".into(), name.clone())
    }

    fn is_space(&self, s: &str) -> bool {
        self.spaces.contains(s)
            || self.patterns.contains_key(s)
            || self.imports.contains_key(s)
            || self.exports.contains_key(s)
    }

    /// The effect of a name applied in a namespace: a rule's rewrite, else
    /// the word it names, else the core's rule's rewrite.
    fn name_effect(
        &self,
        ns: &str,
        name: &Rc<str>,
        visiting: &mut Vec<Rc<str>>,
    ) -> Option<(usize, usize)> {
        let (site, key) = match ns {
            "" => self.site_of(name),
            _ => (ns.into(), name.clone()),
        };
        if let Some(rule) = self.rule(&site, &key).ok()? {
            return self.rewrite_effect(&site, &rule, &key, visiting);
        }
        let found = self.resolve_quiet(ns, name);
        if self.words.contains_key(&found) {
            return self.word_effect(&found, visiting);
        }
        let rule = self.core_rule(&key)?;
        self.rewrite_effect(&site, &rule, &key, visiting)
    }

    /// The effect of a rule's rewrite of a name: its captures, each found as
    /// a name of `ns` would be, then its code, in the rule's namespace.
    fn rewrite_effect(
        &self,
        ns: &str,
        rule: &Rule,
        name: &str,
        visiting: &mut Vec<Rc<str>>,
    ) -> Option<(usize, usize)> {
        let word = &self.patterns[&rule.0].each[rule.1].word;
        if visiting.contains(word) {
            return None;
        }
        let body = self.words.get(word)?.clauses.first()?.body.clone();
        let mut toks: Vec<Token> = self
            .captures(rule, name)
            .into_iter()
            .map(|c| Token {
                tok: Tok::Name(self.resolve_quiet(ns, &c)),
                pos: Pos::default(),
            })
            .collect();
        toks.extend(body.iter().cloned());
        visiting.push(word.clone());
        let r = self.effect_in(&toks, "", visiting);
        visiting.pop();
        r
    }

    /// Applies a name, a path or the root's: a rule of the namespace where
    /// it was applied that matches it, else the word lookup finds, else a
    /// rule of the core's (doc/design/SYMBOLIC-FORTH.md 25). A name a rule
    /// matches is expanded to a path in the namespace it was written in, so
    /// the path says where.
    fn apply_name(&mut self, name: &Rc<str>, at: Pos) -> Result<()> {
        let (site, key) = self.site_of(name);
        if let Some(rule) = self.rule(&site, &key)? {
            return self.rewrite(&rule, &key, at, site);
        }
        if !self.words.contains_key(&self.resolve_quiet("", name))
            && let Some(rule) = self.core_rule(&key)
        {
            return self.rewrite(&rule, &key, at, site);
        }
        let word = self.lookup(name)?;
        self.apply_word(&word)
    }

    /// `;` (doc/design/SYMBOLIC-FORTH.md 18): binds the name below to the
    /// value on top, a clause's body or a type. A body's first check is the
    /// clause's signature, its inputs, and its last what it leaves (6.32):
    /// `sq: dup * ;`, `fib: < n >. … ;`, `sq: < i64 >. dup * < i64 >. ;` and
    /// `num: < i64 f64 or > ;`.
    fn semicolon(&mut self) -> Result<()> {
        let usage = "`;` takes a name, then a quotation, a type or a symbol";
        let mut value = self.stack.pop().expect("matched");
        let (name, named_at) = match self.pop() {
            Ok(Jdg {
                val: Val::Name(n, at),
                ..
            }) => (n, at),
            // Until 2026-10-24 a signature came before its body.
            Ok(Jdg {
                val: Val::Type(_), ..
            }) => {
                return Err(self.err(
                    Kind::Syntax,
                    "a signature is a check, first in its body: `sq: < i64 >. dup * ;`, \
                     `sq [ [ i64 . ] sig# ! . dup . * . ] ;`",
                ));
            }
            Ok(Jdg {
                val: Val::Quote(q), ..
            }) if signature(&q) => {
                return Err(self.err(
                    Kind::Syntax,
                    "a signature is a check, first in its body: `sq: < i64 >. dup * ;`, \
                     `sq [ [ i64 . ] sig# ! . dup . * . ] ;`",
                ));
            }
            _ => return Err(self.err(Kind::Mismatch, usage)),
        };
        // In braces, `;` makes an entry of the map around it, so a record
        // type's fields are its entries, `{ x: < f64 > ; }` (TYPES.md 2.11).
        if self
            .literals
            .last()
            .is_some_and(|l| l.bracket == b'{' && l.depth == self.depth)
        {
            return self.entry(name, named_at, value);
        }
        // A body's first check is the clause's signature, its inputs, and its
        // last, after that, what it leaves (doc/design/SYMBOLIC-FORTH.md
        // 6.32), each made here; a body that is one type is that type, which
        // `;` names, `num: < i64 f64 or … > ;`.
        let mut sigs = Vec::new();
        if let Val::Quote(q) = &value.val
            && !signature(q)
        {
            let q = q.clone();
            let (ins, outs, body) = checks(&q);
            for r in [ins, outs].into_iter().flatten() {
                self.run(&q[r])?;
                sigs.push(self.pop()?);
            }
            if !sigs.is_empty() {
                value.val = Val::Quote(q[body].into());
            } else if let Some(t) = self.type_body(&q)? {
                value = t;
            } else if matches!(q.first().map(|t| &t.tok), Some(Tok::Open(_)))
                && made_at(&q, matching(&q, 0) + 1) == Some("map#")
                && matching(&q, 0) + 4 == q.len()
                && q.iter().any(|t| t.tok == Tok::Name(self.defined(&name)))
            {
                // A map naming the type it defines would be a type made of
                // itself, which is to come, not code that applies itself.
                self.pos = named_at;
                return Err(self.err(
                    Kind::Limit,
                    format!(
                        "`{name}` is named in its own definition, a type made of itself, \
                         which is to come"
                    ),
                ));
            }
        }
        // The meta-ops are not names: the reader knows them, nothing looks
        // them up, and so nothing defines them (doc/design/SYMBOLIC-FORTH.md
        // 16).
        if matches!(&*name, "." | "?" | "!" | ";" | "[" | "]") {
            return Err(self.err(
                Kind::Syntax,
                format!("`{name}` is a meta-op, which is no name, and is not defined"),
            ));
        }
        // A symbol is global, so a name is defined where it says: a path in
        // its namespace, a bare name in the root, or while the core is read,
        // in the core.
        let name = self.defined(&name);
        let (space, key) = split_path(&name);
        let space: Rc<str> = space.into();
        if matches!(key, "import#" | "export#") {
            return match &value.val {
                // A body of one bracket lists the names, `import#: [| math.*
                // |] ;`, as `import# [ … ] ;` does.
                Val::Quote(q)
                    if sigs.is_empty()
                        && matches!(q.first().map(|t| &t.tok), Some(Tok::Open(_)))
                        && matching(q, 0) + 1 == q.len() =>
                {
                    self.set_names(&space, key, &q[1..q.len() - 1])
                }
                Val::Quote(q) if sigs.is_empty() => self.set_names(&space, key, q),
                _ => Err(self.err(
                    Kind::Syntax,
                    format!("`{key}` takes the names it lists, `[| math.* io.file.read |]`"),
                )),
            };
        }
        // A pattern defines a rule (doc/design/SYMBOLIC-FORTH.md 25): code
        // put where a name it matches is applied, after the captures. A rule
        // is a spelling, not a word, so it has no signature and one meaning.
        let rule = pattern_at(key) == Some(0);
        if rule {
            self.pattern(&space, key)?;
            if !sigs.is_empty() {
                return Err(self.err(
                    Kind::Syntax,
                    format!(
                        "`{key}` is a rule, code put where a name it matches stands, and has \
                         no signature: `{key}: … ;`"
                    ),
                ));
            }
            if self.words.get(&name).is_some_and(|w| !w.clauses.is_empty()) {
                return Err(self.err(
                    Kind::Syntax,
                    format!("`{key}` has its code already: a rule has one meaning"),
                ));
            }
            if !matches!(&value.val, Val::Quote(q) if !signature(q)) {
                return Err(self.err(
                    Kind::Mismatch,
                    format!("`{key}` is a rule, and takes its code, a quotation"),
                ));
            }
        } else {
            self.definable(&name)?;
        }
        if !self.core {
            self.word(&name).user = true;
        }
        let q = match value.val {
            Val::Quote(q) if !signature(&q) => q,
            Val::Type(t) if sigs.is_empty() => {
                return self.name_type(&name, t);
            }
            Val::Name(n, ..)
                if sigs.is_empty()
                    && self
                        .words
                        .get(&self.resolve_quiet("", &n))
                        .is_some_and(|w| w.ty.is_some()) =>
            {
                let t = self.words[&self.resolve_quiet("", &n)].ty.expect("a type");
                return self.name_type(&name, t);
            }
            // A symbol is data, which `;` binds as it does code: the word
            // leaves it, `Human: Human' ;` (doc/design/SYMBOLIC-FORTH.md 18,
            // 27).
            Val::Name(n, pos) if value.ty == SYMBOL => Rc::from([Token {
                tok: Tok::Name(n),
                pos,
            }]),
            _ => return Err(self.err(Kind::Mismatch, usage)),
        };
        let at = named_at;
        let sig = match &sigs[..] {
            [] => None,
            [s] => Some(self.sig_of(s)?),
            [ins, outs] => {
                let ins = self.sig_of(ins)?;
                let outs = match &outs.val {
                    Val::Type(t) => vec![*t],
                    Val::Quote(q) => self.outputs(&q.clone()[1..q.len() - 1])?,
                    _ => unreachable!("a signature"),
                };
                Some(Sig {
                    outs: Some(outs),
                    ..ins
                })
            }
            _ => unreachable!("a body's first check and its last"),
        };
        let core = self.core;
        self.define(
            &name,
            Clause {
                sig,
                written: sigs.into(),
                body: q,
                at,
                core,
            },
        )
    }

    /// `;` in braces: an entry of the map literal around it, its key the
    /// name and its value what the body names, a type for now.
    fn entry(&mut self, key: Rc<str>, at: Pos, value: Jdg) -> Result<()> {
        let t = match &value.val {
            Val::Quote(q) if !signature(q) => self.type_body(&q.clone())?,
            _ => None,
        };
        let Some(t) = t else {
            return Err(self.err(
                Kind::Limit,
                "`;` in braces makes an entry of a type for now, a record type's field, \
                 `{ x: < f64 > ; }`; a value is written `{ x: 1 }`",
            ));
        };
        self.push(SYMBOL, Val::Name(key, at));
        self.stack.push(t);
        Ok(())
    }

    /// A body that is one type, a type's name, `temp: f64 ;`, or a bracket
    /// of one, `num: < i64 f64 or … > ;`: that type, made as the code before
    /// `;` made it until bodies were bare (doc/design/SYMBOLIC-FORTH.md
    /// 6.32), for `;` to name.
    fn type_body(&mut self, q: &[Token]) -> Result<Option<Jdg>> {
        match q {
            [
                Token {
                    tok: Tok::Name(n), ..
                },
                Token {
                    tok: Tok::Apply, ..
                },
            ] if self
                .words
                .get(&self.resolve_quiet("", n))
                .is_some_and(|w| w.ty.is_some()) =>
            {
                self.run(q)?;
                self.pop().map(Some)
            }
            [
                Token {
                    tok: Tok::Open(_), ..
                },
                ..,
            ] if made_at(q, matching(q, 0) + 1) == Some("sig#")
                && matching(q, 0) + 4 == q.len() =>
            {
                self.run(q)?;
                let j = self.pop()?;
                Ok(matches!(j.val, Val::Type(_)).then_some(j))
            }
            // A body that takes nothing may leave a type, made now: a map of
            // types, a record type, `point: { x: < f64 > ; y: < f64 > ; } ;`,
            // or a type of its own, `celsius: f64 own ;` (TYPES.md 2.11). If
            // it leaves anything else, or fails here, it is code.
            [
                Token {
                    tok: Tok::Open(_), ..
                },
                ..,
            ] if made_at(q, matching(q, 0) + 1) == Some("map#")
                && matching(q, 0) + 4 == q.len() =>
            {
                self.made_now(q)
            }
            // Code run while compiling, `[ … ] !`, runs where its word is
            // applied, not here.
            [_, ..]
                if self.effect(q, &mut Vec::new()) == Some((0, 1))
                    && !q.windows(2).any(|w| {
                        matches!(w[0].tok, Tok::Close(_))
                            && matches!(&w[1].tok, Tok::Name(n) if &**n == "!")
                    }) =>
            {
                self.made_now(q)
            }
            _ => Ok(None),
        }
    }

    /// The type a body leaves, run now, or none, and nothing done, if it
    /// leaves anything else or fails here.
    fn made_now(&mut self, q: &[Token]) -> Result<Option<Jdg>> {
        let saved = (
            self.stack.clone(),
            self.code.len(),
            self.literals.len(),
            self.floor,
            self.inputs.len(),
            self.effects,
            self.warnings.len(),
        );
        let r = self.run(q);
        if r.is_ok()
            && self.stack.len() == saved.0.len() + 1
            && self.code.len() == saved.1
            && matches!(self.stack.last().map(|j| &j.val), Some(Val::Type(_)))
        {
            return self.pop().map(Some);
        }
        self.stack = saved.0;
        self.code.truncate(saved.1);
        self.literals.truncate(saved.2);
        self.floor = saved.3;
        self.inputs.truncate(saved.4);
        self.effects = saved.5;
        self.warnings.truncate(saved.6);
        Ok(None)
    }

    /// A signature from its value: a type, its one input, or a bracket.
    fn sig_of(&mut self, j: &Jdg) -> Result<Sig> {
        match &j.val {
            Val::Type(t) => Ok(Sig {
                ins: vec![*t],
                guards: Vec::new(),
                outs: None,
            }),
            Val::Quote(q) => self.bracket(&q[1..q.len() - 1]),
            _ => unreachable!("a signature"),
        }
    }

    /// Refuses a name no word may have.
    fn definable(&self, path: &str) -> Result<()> {
        if path.contains('.') && !is_path(path) {
            return Err(self.err(
                Kind::Syntax,
                format!("`{path}` has an empty part: a path's parts are names"),
            ));
        }
        let n = path.rsplit('.').next().expect("a part");
        if read::is_variable(path) {
            return Err(self.err(
                Kind::Syntax,
                format!("`{n}` is a variable, one letter, and names no word"),
            ));
        }
        if let Some(stem) = n.strip_suffix('?').filter(|s| !s.is_empty()) {
            return Err(self.err(
                Kind::Syntax,
                format!(
                    "`{n}` is `{stem}` tested, `{stem} ?`, and names no word: name it `{stem}`"
                ),
            ));
        }
        // A subscript is part of a name, an index (22), but a power is
        // read in lookup, and no name ends in one (24).
        let (_, _, sup) = crate::symbols::scripts(n);
        if let Some(base) = n
            .strip_suffix(sup)
            .and_then(|b| b.strip_suffix('^'))
            .filter(|b| !sup.is_empty() && !b.is_empty())
        {
            return Err(self.err(
                Kind::Syntax,
                format!(
                    "`{n}` is a spelling, `{base}` to the power {sup}, and names no word; \
                     a meaning of its own is a clause of `power#`"
                ),
            ));
        }
        Ok(())
    }

    /// `?`: applies the value on top as a test, which leaves one bool
    /// (doc/design/SYMBOLIC-FORTH.md 20). In a signature, whose code runs
    /// over types, the test is held as a guard on the clause's inputs, and
    /// `?` applies it when the guard runs.
    fn apply_test(&mut self) -> Result<()> {
        let f = self.pop()?;
        if !is_code(&f) {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "{} is no test: `?` applies a word or a quotation",
                    self.describe(&f)
                ),
            ));
        }
        if self.in_bracket > 0 {
            let at = self.pos;
            let tok = |tok| Token { tok, pos: at };
            let mut q = match &f.val {
                Val::Name(n, p) => vec![Token {
                    tok: Tok::Name(n.clone()),
                    pos: *p,
                }],
                Val::Quote(c) => {
                    let mut v = vec![tok(Tok::Open(b'['))];
                    v.extend(c.iter().cloned());
                    v.push(tok(Tok::Close(b']')));
                    v
                }
                _ => unreachable!("code"),
            };
            q.push(tok(Tok::Test));
            // Held, the test it is: a bool to come, of the inputs.
            self.push(BOOL, Val::Quote(q.into()));
            return Ok(());
        }
        let shown = match &f.val {
            Val::Name(n, ..) => format!("`{n}?`"),
            _ => "a quotation tested".into(),
        };
        let outs = self.code_effect(&f).map(|(_, o)| o);
        self.stack.push(f);
        self.apply()?;
        let top = self.stack.last().filter(|_| self.stack.len() > self.floor);
        let left = match (outs, top) {
            (Some(1) | None, Some(j)) if j.ty == BOOL => return Ok(()),
            (Some(o), _) if o != 1 => values(o),
            (_, Some(j)) => self.describe(j),
            (_, None) => "none".into(),
        };
        Err(self.err(
            Kind::Mismatch,
            format!("{shown} tests, so it leaves one bool, and it leaves {left}"),
        ))
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
            Val::Ratio(t) => format!("{} (ratio#)", prims::show_ratio(t)),
            Val::Float(x) => format!("{x:?} (f64)"),
            Val::Str(s) => format!("{s:?}"),
            Val::Type(t) => format!("the type `{}`", self.types.name(*t)),
            Val::Name(n, ..) => format!("the symbol `{n}`"),
            Val::Quote(_) => "a quotation".into(),
            Val::Seq(_) => format!("{} ({})", value_text(j), self.types.name(j.ty)),
            Val::Many => format!("a run of {}", self.types.name(j.ty)),
            Val::Pinned(v) => self.describe(&Jdg {
                ty: j.ty,
                val: (**v).clone(),
            }),
        }
    }

    /// The types of the top `n` values, for a message, a symbol by its name:
    /// `door` for `Locked Open`.
    fn top_types(&self, n: usize) -> String {
        let n = n.min(self.stack.len() - self.floor);
        if n == 0 {
            return "no values".into();
        }
        let top = &self.stack[self.stack.len() - n..];
        top.iter()
            .map(|j| match &j.val {
                Val::Name(s, ..) if j.ty == SYMBOL => {
                    let mut out = String::new();
                    crate::show::symbol(s, &mut out);
                    out
                }
                _ => self.types.name(j.ty),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A note for a capitalized name that a word is spelled like: `Dup`,
    /// written for `dup`.
    fn typo(&self, n: &str) -> String {
        let mut cs = n.chars();
        let Some(c) = cs.next().filter(|c| c.is_uppercase()) else {
            return String::new();
        };
        let word: String = c.to_lowercase().chain(cs).collect();
        if self
            .words
            .contains_key(&self.resolve_quiet("", &word.as_str().into()))
        {
            format!("; the word is `{word}`")
        } else {
            String::new()
        }
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
            || self
                .types
                .own_parts(t)
                .is_some_and(|(_, r)| r == lit || self.accepts(r, lit))
    }

    /// The type a literal takes when nothing says (TYPES.md 2.10).
    fn default_of(lit: Type) -> Type {
        if matches!(lit, INT_LIT | UNIT_LIT) {
            I64
        } else {
            F64
        }
    }

    /// Gives the judgment at `i` the type `t`: it has it already; or it is a
    /// vec and `t` an array of the same elements, which forgets the length;
    /// or it is a literal, converted by `t`'s clause for it.
    fn annotate(&mut self, i: usize, t: Type) -> Result<()> {
        let have = self.stack[i].ty;
        if t == TYPE && have == NIL && self.stack[i].known() {
            self.stack[i] = Jdg {
                ty: TYPE,
                val: Val::Type(NIL),
            };
            return Ok(());
        }
        // A class is checked, and the value keeps its type.
        let fits = |s: &mut Self| s.match_slot(t, have, &mut Env::default()).is_some();
        if have == t || (self.types.is_class(t) && fits(self)) {
            return Ok(());
        }
        // A literal becomes a type of its own as its representation, where
        // no conversion of its own says how, `price 3 +`, a map literal too.
        if let Some((_, r)) = self.types.own_parts(t)
            && (self.types.is_literal(have) || self.types.is_record_lit(have))
            && self.conversion(t, have).is_none()
        {
            self.annotate(i, r)?;
            self.stack[i].ty = t;
            return Ok(());
        }
        if self.types.is_record_lit(have) && !self.types.is_pattern(t) {
            return self.annotate_record(i, t);
        }
        if self.annotate_seq(i, t)? {
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
            // A symbol is its name, interned, at run time as a string is, so
            // only a known one changes.
            if t == STRING
                && let Val::Name(n, ..) = &self.stack[i].val
            {
                self.stack[i].val = Val::Str(n.clone());
            }
            return Ok(());
        }
        if self.types.is_literal(have) {
            let Some(c) = self.conversion(t, have) else {
                return Err(self.err(
                    Kind::Literal,
                    format!(
                        "{} cannot become {}{}",
                        self.describe(&self.stack[i]),
                        self.types.name(t),
                        narrowing(t, have)
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
                "expected {}, found {}{}",
                self.types.name(t),
                self.describe(&self.stack[i]),
                narrowing(t, have)
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
                // A type of its own is its representation, forgotten.
                (_, Term::Own(_, r)) => self.forgets(t, r),
                // A symbol is text, its name, a string once taken as one
                // (doc/design/SYMBOLIC-FORTH.md 28).
                (Term::Base(Base::String), Term::Base(Base::Symbol)) => true,
                _ => false,
            }
    }

    /// A bracket in a body: it types the values on top.
    fn annotate_top(&mut self, sig: &Sig) -> Result<()> {
        if !sig.guards.is_empty() {
            return Err(self.err(
                Kind::Syntax,
                "guards and value patterns belong in a signature, a body's first check, \
                 `max: < a a gt? >. drop ;`",
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

    /// A literal matches its own type best, then another literal kind, then
    /// its default type, then any type it converts to; anything else must
    /// unify.
    fn match_slot(&mut self, p: Type, have: Type, env: &mut Env) -> Option<i32> {
        if let Term::Or(..) = self.types.term(p) {
            return self.match_union(p, have, env);
        }
        // A type with one value is that value, `nil`, which stands for its
        // type where a type is wanted, `i64 nil or`.
        if p == TYPE && have == NIL {
            return Some(4);
        }
        if self.types.is_literal(have) {
            return match self.types.term(p) {
                Term::Var(v) => self.bind(v, have, env),
                Term::Atom => Some(2),
                _ if p == have => Some(8),
                // Another literal kind keeps it a literal, exact, so `1 3
                // /*` is the ratio 1:3: a literal takes a type when one is
                // needed (TYPES.md 2.10). Then its default, then any other.
                _ if self.accepts(p, have) => Some(if self.types.is_literal(p) {
                    6
                } else if p == Self::default_of(have) {
                    4
                } else {
                    2
                }),
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
            // bool: `3 true max` is 3.
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
            // A type of its own is only itself where it is wanted, and where
            // its representation is, it is that, matched a little less well
            // (TYPES.md 2.11), as a bool is an i64.
            (Term::Own(..), _) => (p == have).then_some(8),
            (_, Term::Own(_, r)) => Some((self.unify(p, r, env)? - 2).max(0)),
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
            // cost: an array of the union of its types (TYPES.md 2.5), or of
            // a type each element is, `( "a" b' )` strings.
            (Term::Ary(e), Term::Tuple(_)) => {
                let u = self.tuple_union(have);
                if let Some(s) = self.unify(e, u, env) {
                    return Some(1 + s);
                }
                let es = self.types.elements(have).expect("a tuple").to_vec();
                let mut worst = i32::MAX;
                for x in es {
                    worst = worst.min(self.unify(e, x, env)?);
                }
                Some(1 + worst)
            }
            // A bool is an i64, 0 or 1: matched a little less well than one.
            (Term::Base(Base::I64), Term::Base(Base::Bool)) => Some(6),
            // A symbol is a string where one is taken, matched a little less
            // well than one (doc/design/SYMBOLIC-FORTH.md 28).
            (Term::Base(Base::String), Term::Base(Base::Symbol)) => Some(6),
            (Term::Base(a), Term::Base(b)) => (a == b).then_some(8),
            (Term::Nat(a), Term::Nat(b)) => (a == b).then_some(8),
            (Term::Trap, Term::Trap) => Some(8),
            (Term::Ary(e), Term::Ary(f)) => Some(8 + self.unify(e, f, env)?),
            (Term::Ary(e), Term::Vec(_, f)) => Some(4 + self.unify(e, f, env)?),
            (Term::Vec(n, e), Term::Vec(m, f)) => {
                Some(8 + self.unify(n, m, env)? + self.unify(e, f, env)?)
            }
            (Term::Map(k, v), Term::Map(k2, v2)) => {
                Some(8 + self.unify(k, k2, env)? + self.unify(v, v2, env)?)
            }
            (Term::Record(_), Term::Record(_)) => {
                let (ps, hs) = (self.types.fields(p)?, self.types.fields(have)?);
                if ps.len() != hs.len() || ps.iter().zip(hs.iter()).any(|(a, b)| a.0 != b.0) {
                    return None;
                }
                let mut s = 8;
                for (a, b) in ps.iter().zip(hs.iter()) {
                    s += self.unify(a.1, b.1, env)?;
                }
                Some(s)
            }
            // A map literal undecided is the record of its keys it can be,
            // field by field, a literal as it would be alone (TYPES.md 2.11).
            (Term::Record(_), Term::RecordLit(_)) => {
                let (ps, hs) = (self.types.fields(p)?, self.types.fields(have)?);
                if ps.len() != hs.len() || ps.iter().zip(hs.iter()).any(|(a, b)| a.0 != b.0) {
                    return None;
                }
                let mut s = 4;
                for (a, b) in ps.iter().zip(hs.iter()) {
                    s += self.match_slot(a.1, b.1, env)?;
                }
                Some(s)
            }
            (Term::AnyRecord, Term::Record(_) | Term::RecordLit(_)) => Some(4),
            // And the map it is with nothing to take it, matched less well.
            (Term::Map(..), Term::RecordLit(_)) => {
                let m = self.lit_map(have);
                self.unify(p, m, env).map(|_| 2)
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
            let mut have = self.stack[base + k].ty;
            // A type of its own forgets itself where its representation's
            // word is applied (TYPES.md 2.11), `price sqrt` a decimal.
            while let Some((_, r)) = self.types.own_parts(have)
                && have != t
            {
                have = r;
                self.stack[base + k].ty = r;
            }
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
    /// on these judgments, handled if it has handlers for them.
    fn family(&mut self, name: &Rc<str>) -> Result<()> {
        if self.has_handlers(name) {
            return self.handled(name);
        }
        self.apply_family(name)
    }

    /// Whether a family has handlers for the values on top: clauses that
    /// take no trap take them, and clauses for them and a trap on top
    /// (doc/design/SYMBOLIC-FORTH.md 29). A clause that takes a trap is
    /// otherwise chosen as any is, as a word applied to a trap.
    fn has_handlers(&mut self, name: &Rc<str>) -> bool {
        if !self.words[name].clauses.iter().any(|c| c.handles()) {
            return false;
        }
        let handlers = self.clauses(name, true);
        let plain = self.clauses(name, false);
        if self.candidates(&plain).is_empty() {
            return false;
        }
        self.push(TRAP, Val::Run);
        let found = !self.candidates(&handlers).is_empty();
        self.stack.pop();
        found
    }

    /// Applies a family whose handlers take the values on top, handled
    /// (doc/design/SYMBOLIC-FORTH.md 29): the machine keeps the inputs that
    /// are values at run time, and when anything inside fails, they come
    /// back, the trap goes on top, and the handlers choose for them as any
    /// clauses do, a trap none of them takes going on outward. Either way
    /// the word leaves what it leaves, its results joined as a choice's.
    /// Inside, a failure is caught as the program runs, so it fails then.
    fn handled(&mut self, name: &Rc<str>) -> Result<()> {
        let arity = self.arity(name);
        let at = self.pos;
        let snapshot = self.stack.clone();
        let base = snapshot.len() - arity;
        let key = (
            name.clone(),
            snapshot[base..].iter().map(|j| j.ty).collect(),
        );
        if snapshot[base..].iter().any(|j| j.val == Val::Many) || self.catches.contains(&key) {
            return Err(self.err(
                Kind::Limit,
                format!(
                    "`{name}` is handled here, and inside its own handler for the same \
                     inputs, or on a run of values: that is not built yet"
                ),
            ));
        }
        let kept = snapshot[base..].iter().filter(|j| !j.known()).count();
        // The handlers, given the inputs and the trap.
        self.push(TRAP, Val::Run);
        let handlers = self.clauses(name, true);
        for (_, c, _) in self.candidates(&handlers) {
            let n = c.sig.as_ref().map_or(0, |s| s.ins.len());
            if n != arity + 1 {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "a handler of `{name}` takes its inputs and a trap, {}, \
                         and the one defined at {} takes {}",
                        values(arity + 1),
                        c.at,
                        values(n)
                    ),
                ));
            }
        }
        // A handler runs only if something fails.
        let outer = std::mem::take(&mut self.code);
        self.catches.push(key);
        self.catching = true;
        self.unsure += 1;
        let r = self.resolve(name, arity + 1, at, false);
        self.unsure -= 1;
        self.catching = false;
        self.catches.pop();
        let code = std::mem::replace(&mut self.code, outer);
        let caught = match r {
            Err(e) if e.kind == Kind::Fails => None,
            Err(e) => return Err(e),
            Ok(()) => Some(self.stack.clone()),
        };
        if let Some(st) = &caught
            && st[base.min(st.len())..]
                .iter()
                .any(|j| self.types.holds(j.ty, TRAP))
        {
            self.pos = at;
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "a handler of `{name}` leaves its trap: a trap lives only while it is handled"
                ),
            ));
        }
        // The word itself.
        self.stack = snapshot.clone();
        let outer = std::mem::take(&mut self.code);
        self.handled += 1;
        let r = self.apply_family(name);
        self.handled -= 1;
        let mut body = std::mem::replace(&mut self.code, outer);
        let done = match r {
            Err(e) if e.kind == Kind::Fails => None,
            Err(e) => return Err(e),
            Ok(()) => {
                body.push(Ins::Op(Op::Prim(P::Unhandle)));
                Some(self.stack.clone())
            }
        };
        // `handle` goes on past itself, to the body, or, given the trap, to
        // the handlers.
        let start = self.label();
        self.emit(Op::Lit(kept as u64));
        self.emit(Op::Prim(P::Handle));
        self.code.push(Ins::JumpZero(start));
        let alts = vec![
            Alt {
                code,
                stack: caught,
                skip: Some(start),
            },
            Alt {
                code: body,
                stack: done,
                skip: None,
            },
        ];
        let r = self.merge(alts, &snapshot, base, true);
        self.pos = at;
        self.fails_here(r, name)
    }

    /// Applies a family on these judgments. A family applied again to the
    /// same types inside its own application is recursive: the outer
    /// application starts over as a call to an instance, a word of its own
    /// for those types, and the inner one, compiled inside that instance,
    /// calls it.
    fn apply_family(&mut self, name: &Rc<str>) -> Result<()> {
        let arity = self.arity(name);
        let reach = arity.min(self.stack.len() - self.floor);
        let types: Vec<Type> = self.stack[self.stack.len() - reach..]
            .iter()
            .map(|j| j.ty)
            .collect();
        // A value that is only known while compiling, as a quotation, has no
        // cell for an instance to take, so a family applied to one again is
        // unfolded, its guards decided by the values, until one finishes, as
        // March walks a quotation: depth bounds it.
        let unfolds = types
            .iter()
            .any(|&t| self.types.compile_time_only(t) && !self.types.is_literal(t));
        if let Some(i) = self
            .frames
            .iter()
            .rposition(|f| f.name == *name && f.ins == types)
            .filter(|_| !unfolds)
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
        if let Some(i) = self.active.iter().position(|k| *k == key)
            && !unfolds
        {
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
            self.inputs.len(),
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
                self.inputs.truncate(saved.5);
                self.call_instance(name, arity)
            }
            r => r,
        }
    }

    /// A family's clauses that take a trap last, or those that do not.
    fn clauses(&self, name: &str, handlers: bool) -> Vec<Rc<Clause>> {
        self.words[name]
            .clauses
            .iter()
            .filter(|c| c.handles() == handlers)
            .cloned()
            .collect()
    }

    /// How many values an application of a family takes: the most that the
    /// clauses which may be chosen for the values on top take, as a choice
    /// at run time between them leaves those deeper than a shorter one
    /// takes as they were (TYPES.md 3.13); if none may be, the most any
    /// signature names; and without signatures, what its definition takes.
    fn arity(&mut self, name: &Rc<str>) -> usize {
        let clauses = self.words[name].clauses.clone();
        if clauses.iter().all(|c| c.sig.is_none()) {
            return self.effect_of(name).map_or(0, |e| e.0);
        }
        let longest = longest(&clauses);
        self.candidates(&clauses)
            .iter()
            .map(|c| length(&c.1, longest))
            .max()
            .unwrap_or(longest)
    }

    /// Chooses among a family's clauses and applies the choice. Clauses are
    /// chosen in two phases (TYPES.md 2.15): by types now, keeping those
    /// whose inputs match; then, if some have guards, by their guards at run
    /// time, in the order they were defined, the best clause without guards
    /// last. `top` says this is an instance's own family, whose recursive
    /// alternatives may wait for its results' types.
    fn resolve(&mut self, name: &Rc<str>, arity: usize, at: Pos, top: bool) -> Result<()> {
        let catching = std::mem::take(&mut self.catching);
        let clauses = match catching {
            true => self.clauses(name, true),
            false => self.words[name].clauses.clone(),
        };
        let mut cands = self.candidates(&clauses);
        if let Some(i) = self.union_input(arity)
            && !catching
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
            // A handlers' dispatch is no application of its own.
            if !top && !catching {
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
            return self.chain(name, &cands, arity, at, top, catching);
        }
        match self.best(name, &cands, arity)? {
            Some((c, env)) => self.apply_clause(name, &c, env, at),
            None => unreachable!("candidates"),
        }
    }

    /// The clauses whose inputs match the values on top and which may be
    /// chosen, with their scores and bindings, the shortest first and in the
    /// order they were defined among equals.
    ///
    /// When clauses of different lengths match, the shortest wins (Thomas,
    /// TYPES.md 3.13): a word reaches as little of the stack as it can, so a
    /// longer clause never changes what a call to a shorter one did. So a
    /// clause scores less the more values it takes, then by how well it
    /// matches; and one longer than the shortest that matches without
    /// guards, which always holds, is never chosen.
    fn candidates(&mut self, clauses: &[Rc<Clause>]) -> Vec<Candidate> {
        let longest = longest(clauses);
        let mut cands: Vec<(usize, Candidate)> = clauses
            .iter()
            .filter_map(|c| {
                let m = match &c.sig {
                    None => Some((0, Env::default())),
                    Some(s) => self.match_sig(s),
                };
                let n = length(c, longest);
                m.map(|(score, env)| (n, (score - REACH * n as i32, c.clone(), env)))
            })
            .collect();
        let bound = cands
            .iter()
            .filter(|(_, c)| !c.1.guarded())
            .map(|&(n, _)| n)
            .min();
        cands.retain(|&(n, _)| bound.is_none_or(|b| n <= b));
        cands.sort_by_key(|&(n, _)| n);
        cands.into_iter().map(|(_, c)| c).collect()
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
            let have = self.stack[base + k].ty;
            if (self.types.is_literal(have) || self.types.is_record_lit(have))
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
        catching: bool,
    ) -> Result<()> {
        let longest = longest(&self.words[name].clauses);
        let n = cands
            .iter()
            .map(|c| length(&c.1, longest))
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
            let first = order.is_empty();
            match self.alternative(name, cand, true, first, &snapshot, at, top)? {
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
                let first = order.is_empty();
                match self.alternative(name, &cand, false, first, &snapshot, at, top)? {
                    Outcome::Taken(alt, _) => alts.push(Some(alt)),
                    Outcome::Waits(_) => {
                        waiting.push(alts.len());
                        alts.push(None);
                    }
                    Outcome::Dropped => unreachable!("no guards"),
                }
                order.push((cand, false));
            } else if catching {
                // A trap no handler takes goes on outward, as it is.
                let mut code: Vec<Ins> = Self::rethrow().into_iter().map(Ins::Op).collect();
                code.push(Ins::Op(Op::Prim(P::Trap)));
                alts.push(Some(Alt {
                    code,
                    stack: None,
                    skip: None,
                }));
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
                // As the program runs, it fails as it would now.
                let msg = self.shorten(&format!(
                    "no word `{name}` for {}: no clause's guard holds",
                    self.top_types(arity)
                ));
                alts.push(Some(Alt {
                    code: vec![
                        Ins::Str(msg.into()),
                        Ins::Str("no-word".into()),
                        Ins::Op(Op::Prim(P::Trap)),
                    ],
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
                match self.alternative(name, &cand, guarded, i == 0, &snapshot, at, top)? {
                    Outcome::Taken(alt, _) => alts[i] = Some(alt),
                    _ => return Err(Error::new(Kind::Ghost, Some(at), name.to_string())),
                }
            }
        }
        // Clauses of different lengths chosen between at run time must leave
        // the stack as deep (TYPES.md 3.13).
        let lens: Vec<usize> = order.iter().map(|(c, _)| length(&c.1, longest)).collect();
        let depths: Vec<usize> = alts
            .iter()
            .flatten()
            .filter_map(|a| a.stack.as_ref().map(Vec::len))
            .collect();
        if lens.windows(2).any(|w| w[0] != w[1])
            && depths.windows(2).any(|w| w[0] != w[1])
            && self.collecting(base).is_none()
        {
            let mut takes: Vec<usize> = lens.clone();
            takes.sort_unstable();
            takes.dedup();
            let takes: Vec<String> = takes.iter().map(usize::to_string).collect();
            self.pos = at;
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "clauses of `{name}` taking {} values are chosen between at run time, \
                     and leave the stack at different depths: whichever is chosen must \
                     leave it as deep",
                    takes.join(" and ")
                ),
            ));
        }
        // A union its clauses' outputs name is meant, and not warned of.
        let declared = order.iter().all(|(c, _)| {
            c.1.sig
                .as_ref()
                .and_then(|s| s.outs.as_ref())
                .is_some_and(|o| o.iter().any(|&t| self.is_union(t)))
        });
        let r = self.merge(
            alts.into_iter().flatten().collect(),
            &snapshot,
            base,
            !declared,
        );
        self.pos = at;
        self.fails_here(r, name)
    }

    /// A choice every alternative of which fails never returns. Where it
    /// may not run, nothing after it is reached, as after `fail`; where it
    /// surely runs, it is sure to fail, by a kind known only as the program
    /// runs, an error.
    fn fails_here(&self, r: Result<()>, name: &str) -> Result<()> {
        match r {
            Err(e)
                if e.kind == Kind::Fails
                    && self.unsure == 0
                    && self.handled == 0
                    && self.checking == 0 =>
            {
                Err(self.err(
                    Kind::Unknown,
                    format!(
                        "`{name}` fails here whichever clause is chosen, and code sure to \
                         fail fails while compiling, but which kind is known only as the \
                         program runs"
                    ),
                ))
            }
            r => r,
        }
    }

    /// One alternative of a choice, compiled from the saved judgments into
    /// code of its own: its guards' test, if it has guards, and its body.
    /// Its body surely runs if it is the first and its guards hold now.
    #[allow(clippy::too_many_arguments)]
    fn alternative(
        &mut self,
        name: &Rc<str>,
        cand: &Candidate,
        guarded: bool,
        first: bool,
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
        let unsure = !(first && test == Some(true));
        self.unsure += unsure as u32;
        let r = self.apply_clause(name, c, env.clone(), at);
        self.unsure -= unsure as u32;
        let code = std::mem::replace(&mut self.code, outer);
        let sure = guarded && test == Some(true);
        // One that fails ends in its trap, and leaves nothing.
        let stack = match r {
            Err(e) if top && e.kind == Kind::Ghost => return Ok(Outcome::Waits(sure)),
            Err(e) if e.kind == Kind::Fails => None,
            Err(e) => return Err(e),
            Ok(()) => Some(self.stack.clone()),
        };
        Ok(Outcome::Taken(
            Alt {
                code,
                stack,
                skip: test.is_none().then_some(skip),
            },
            sure,
        ))
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
        if lists.is_empty() {
            return Err(self.err(Kind::Fails, "fails"));
        }
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
        (l.bracket == b'(' && l.from() <= base).then_some(l.from())
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
                t if self.types.is_literal(t) => self.annotate(i, Self::default_of(t))?,
                t if self.types.is_record_lit(t) => {
                    let m = self.lit_map(t);
                    self.annotate(i, m)?
                }
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
            // Its code is its own, whether or not where it is applied runs.
            std::mem::replace(&mut self.unsure, 0),
            std::mem::replace(&mut self.checking, 0),
            std::mem::replace(&mut self.handled, 0),
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
                                     give it its outputs, a check last in its body, `< i64 >.`"
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
                             give it its outputs, a check last in its body, `< i64 >.`"
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
            self.unsure,
            self.checking,
            self.handled,
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

    /// Seals code as the machine takes it: shuffles across a value set aside
    /// become the one operation they come to, labels positions, string
    /// literals data objects, a call followed only by a return a tail call,
    /// and a return ends it. The blobs are kept for the session to publish.
    pub fn seal(&mut self, code: Vec<Ins>) -> (Vec<Op>, Cid) {
        let code = fuse(code);
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
                    self.keep(blob);
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
        self.keep(blob);
        (ops, cid)
    }

    /// Keeps a blob for the session to publish, and gives it to the stage's
    /// machine, where `[ … ] !` runs code. One it cannot take, past a limit,
    /// fails there when the code that needs it runs.
    fn keep(&mut self, blob: Blob) {
        let m = self.machine.get_or_insert_with(Machine::empty);
        let _ = m.publish(blob.clone());
        self.blobs.push(blob);
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
            // Each arm runs for its own type alone.
            self.unsure += 1;
            let r = self.split_arm(name, i, m, skip);
            self.unsure -= 1;
            self.pos = at;
            let code = std::mem::replace(&mut self.code, outer);
            // One that fails ends in its trap, and leaves nothing.
            let stack = match r {
                Err(e) if e.kind == Kind::Fails => None,
                r => {
                    r.map_err(|mut e| {
                        e.trace.push(format!(
                            "where the value of `{}` is {}",
                            self.types.name(u),
                            self.types.name(m)
                        ));
                        e
                    })?;
                    Some(self.stack.clone())
                }
            };
            alts.push(Alt { code, stack, skip });
        }
        let r = self.merge(alts, &snapshot, base, false);
        self.fails_here(r, name)
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
        self.in_place_ins(i, ops.iter().cloned().map(Ins::Op).collect(), ty)
    }

    /// As `in_place`, with code that may hold string literals.
    fn in_place_ins(&mut self, i: usize, ins: Vec<Ins>, ty: Type) -> Result<()> {
        self.materialize(i)?;
        let n = self.stack.len() - i;
        let up: Vec<usize> = (1..n).chain([0]).collect();
        self.permute(i, &up);
        self.code.extend(ins);
        self.stack.last_mut().expect("brought up").ty = ty;
        let down: Vec<usize> = std::iter::once(n - 1).chain(0..n - 1).collect();
        self.permute(i, &down);
        Ok(())
    }

    /// A known array at `i` taken as an array or a tuple of other types:
    /// each element converted as it would be alone, so it stays known, `(
    /// "a" b' )` two strings where a `string ary` is wanted (TYPES.md
    /// 2.10). Whether it was; not where an element would need code, or a
    /// tag, as a union's does, or does not convert, which is said of the
    /// whole.
    fn annotate_seq(&mut self, i: usize, t: Type) -> Result<bool> {
        let Val::Seq(es) = &self.stack[i].val else {
            return Ok(false);
        };
        let es = es.clone();
        let want: Vec<Type> = match self.types.term(t) {
            Term::Ary(e) => vec![e; es.len()],
            Term::Vec(n, e) if self.types.term(n) == Term::Nat(es.len() as u64) => {
                vec![e; es.len()]
            }
            Term::Tuple(_) => match self.types.elements(t) {
                Some(ps) if ps.len() == es.len() => ps.to_vec(),
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if want.iter().any(|&w| self.is_union(w)) {
            return Ok(false);
        }
        let code = self.code.len();
        let mut out = Vec::with_capacity(es.len());
        for (e, &w) in es.iter().zip(&want) {
            self.stack.push(e.clone());
            let r = self.annotate(self.stack.len() - 1, w);
            let j = self.stack.pop().expect("pushed");
            if r.is_err() || !j.known() || self.code.len() != code {
                self.code.truncate(code);
                return Ok(false);
            }
            out.push(j);
        }
        self.stack[i] = Jdg {
            ty: t,
            val: Val::Seq(out.into()),
        };
        Ok(true)
    }

    /// The map an undecided map literal is with nothing to take it: of its
    /// symbols and of the type its values share.
    fn lit_map(&mut self, t: Type) -> Type {
        let tys: Vec<Type> = self
            .types
            .fields(t)
            .expect("a record")
            .iter()
            .map(|f| f.1)
            .collect();
        let v = self.join(&tys).expect("its values are alike");
        self.types.intern(Term::Map(SYMBOL, v))
    }

    /// An undecided map literal at `i` given a type (TYPES.md 2.11): a record
    /// of its keys, each value converted as it would be alone; or else the
    /// map it is, its values of the type a map wanted has, made as the
    /// program runs from the values it holds in the order of its keys.
    fn annotate_record(&mut self, i: usize, t: Type) -> Result<()> {
        let have = self.stack[i].ty;
        let fields = self.types.fields(have).expect("a record");
        let wrong = |s: &Self| {
            s.err(
                Kind::Mismatch,
                format!(
                    "expected {}, found {}",
                    s.types.name(t),
                    s.describe(&s.stack[i])
                ),
            )
        };
        if let Term::Record(_) = self.types.term(t) {
            let want = self.types.fields(t).expect("a record");
            if want.len() != fields.len() || want.iter().zip(fields.iter()).any(|(a, b)| a.0 != b.0)
            {
                return Err(wrong(self));
            }
            let types: Vec<Type> = want.iter().map(|f| f.1).collect();
            return self.give_fields(i, &types, t).map_err(|_| wrong(self));
        }
        let v = match self.types.term(t) {
            Term::Map(_, v) => v,
            _ => self
                .join(&fields.iter().map(|f| f.1).collect::<Vec<_>>())
                .expect("alike"),
        };
        let held: Vec<(Rc<str>, Type)> = fields.iter().map(|(k, _)| (k.clone(), v)).collect();
        let record = self.types.record(held, false);
        self.give_fields(i, &vec![v; fields.len()], record)
            .map_err(|_| wrong(self))?;
        let mut ins = vec![
            Ins::Op(Op::Prim(P::ScratchPush)),
            Ins::Op(Op::Prim(P::Mark)),
        ];
        for (k, (key, _)) in fields.iter().enumerate() {
            ins.push(Ins::Str(key.clone()));
            ins.extend(
                [
                    Op::Lit(0),
                    Op::Prim(P::ScratchAt),
                    Op::Lit(k as u64),
                    Op::Prim(P::VecAt),
                ]
                .map(Ins::Op),
            );
        }
        ins.extend(
            [
                Op::Prim(P::MapGather),
                Op::Prim(P::ScratchPop),
                Op::Prim(P::Drop),
            ]
            .map(Ins::Op),
        );
        let m = self.types.intern(Term::Map(SYMBOL, v));
        self.in_place_ins(i, ins, m)?;
        if m != t {
            self.annotate(i, t)?;
        }
        Ok(())
    }

    /// The record at `i`, known or at run time, given fields of these types,
    /// in order, and so the type `t`: a known value converted as it would be
    /// alone, and one at run time only to a type it is already.
    fn give_fields(&mut self, i: usize, types: &[Type], t: Type) -> Result<()> {
        match self.stack[i].val.clone() {
            Val::Seq(es) => {
                let mut out = Vec::with_capacity(es.len());
                for (e, &w) in es.iter().zip(types) {
                    self.stack.push(e.clone());
                    let r = self.annotate(self.stack.len() - 1, w);
                    let j = self.stack.pop().expect("pushed");
                    r?;
                    out.push(j);
                }
                self.stack[i] = Jdg {
                    ty: t,
                    val: Val::Seq(out.into()),
                };
            }
            _ => {
                let have = self.types.fields(self.stack[i].ty).expect("a record");
                if have
                    .iter()
                    .zip(types)
                    .any(|(h, &w)| h.1 != w && !self.forgets(w, h.1))
                {
                    return Err(self.err(Kind::Mismatch, "fields of other types"));
                }
                self.stack[i].ty = t;
            }
        }
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
        // Each element of the type wanted, a symbol among strings, the
        // cells are the array's already.
        if !self.is_union(e) && es.iter().all(|&x| x == e || self.forgets(e, x)) {
            self.stack[i].ty = t;
            return Ok(true);
        }
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
            msg: self.shorten(&msg),
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
                    let r = self.run_quote(q);
                    self.depth -= 1;
                    r
                }),
                // A trap's pattern is its kind (doc/design/SYMBOLIC-FORTH.md
                // 29).
                Test::Equals(v) if v.ty == TRAP => {
                    self.trap_part(1);
                    self.stack.push(Jdg {
                        ty: SYMBOL,
                        val: v.val.clone(),
                    });
                    self.apply_word(&core_path("eq"))
                }
                // A value pattern is the input `eq` the value.
                Test::Equals(v) => {
                    self.stack.push(v.clone());
                    self.apply_word(&core_path("eq"))
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
    /// if their order changes, are rolled into place one by one, or, if
    /// that is longer, go to the scratch stack, the deepest ending on top,
    /// and are copied back in the new order.
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
            // By rolling each value into place, from the first out of
            // place, or through the scratch stack, whichever is shorter.
            let rolls = rolls(&present, &after);
            if rolls.iter().map(|&w| roll_cost(w)).sum::<usize>() < 5 * present.len() {
                for w in rolls {
                    self.roll(w);
                }
            } else {
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
            // A trap's value is not its kind.
            if self.stack[i].val != Val::Run || v.ty == TRAP {
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
    /// definition: what a guard looks at. Clauses of a family of one length
    /// share one effect, so the first whose effect is known gives it. Of
    /// clauses of different lengths, which is chosen depends on the values,
    /// so they have one only if they agree in what they add to the stack:
    /// the longest's inputs, and as many more or fewer outputs.
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
                // Its effect is its quotation's and one more, which `effect`
                // reads where the quotation is written.
                Prim::Dip => return None,
                Prim::QuotePower => (2, 1),
                Prim::Width => return None,
                // It never returns, so it leaves whatever is wanted.
                Prim::Fail => return None,
                Prim::QuoteLength => (1, 1),
                Prim::QuoteAt | Prim::QuoteConcat => (2, 1),
                Prim::QuoteSlice | Prim::QuoteInsert => (3, 1),
                Prim::NameOf => (1, 1),
                Prim::Clauses => (2, 1),
                Prim::Effect => (1, 2),
                Prim::Var => (1, 1),
                Prim::VecConcat | Prim::Map | Prim::Compose => (2, 1),
                Prim::Each | Prim::EachRight => (2, 0),
                Prim::Within => (2, 1),
                Prim::AryInsert | Prim::Zip | Prim::Table => (3, 1),
                Prim::AryRemove => (2, 1),
                Prim::VecSpread | Prim::TupleSpread => return None,
                Prim::Range | Prim::Reverse => (1, 1),
                Prim::Def => return None,
                Prim::Convert => (2, 1),
                _ => (sig.ins.len(), sig.outs.as_ref().map_or(0, Vec::len)),
            });
        }
        // `nil` applied is its value, and another type's name the type.
        if w.ty.is_some() {
            return Some((0, 1));
        }
        if let Some(c) = w.con {
            return Some(if matches!(c, Con::Ary | Con::Trap | Con::Own) {
                (1, 1)
            } else {
                (2, 1)
            });
        }
        if visiting.contains(name) {
            return None;
        }
        visiting.push(name.clone());
        // A handler leaves what its word leaves, given a trap more.
        let mut clauses: Vec<&Rc<Clause>> = w.clauses.iter().filter(|c| !c.handles()).collect();
        if clauses.is_empty() {
            clauses = w.clauses.iter().collect();
        }
        let longest = longest(&w.clauses);
        let r = if clauses.iter().all(|c| length(c, longest) == longest) {
            clauses.iter().find_map(|c| self.clause_effect(c, visiting))
        } else {
            let effects: Vec<(usize, usize)> = clauses
                .iter()
                .filter_map(|c| self.clause_effect(c, visiting))
                .collect();
            let net = |e: &(usize, usize)| e.1 as isize - e.0 as isize;
            match effects.split_first() {
                Some((first, rest)) if rest.iter().all(|e| net(e) == net(first)) => {
                    let ins = effects.iter().map(|e| e.0).max().expect("one");
                    Some((ins, (ins as isize + net(first)) as usize))
                }
                _ => None,
            }
        };
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
        self.effect_in(toks, "", visiting)
    }

    /// The effect of code written in a namespace.
    fn effect_in(
        &self,
        toks: &[Token],
        ns: &str,
        visiting: &mut Vec<Rc<str>>,
    ) -> Option<(usize, usize)> {
        let mut ins = 0;
        let mut items: Vec<Item> = Vec::new();
        self.effect_on(toks, 0..toks.len(), ns, visiting, &mut items, &mut ins)?;
        // Its inputs are taken first, so they are beneath those its code
        // reaches.
        Some((ins + inputs_of(toks), items.len()))
    }

    /// What code in `toks[span]` does to the items it finds, counting in
    /// `ins` those it takes from beneath them.
    fn effect_on(
        &self,
        toks: &[Token],
        span: std::ops::Range<usize>,
        ns: &str,
        visiting: &mut Vec<Rc<str>>,
        items: &mut Vec<Item>,
        ins: &mut usize,
    ) -> Option<()> {
        let mut i = span.start;
        while i < span.end {
            match &toks[i].tok {
                Tok::Int(n) => items.push(Item::Int(*n)),
                Tok::Dec(..) | Tok::Ratio(_) => items.push(Item::Other),
                // A letter, an input taken before the code ran.
                Tok::Name(_) if letter_at(toks, i).is_some() => {
                    items.push(Item::Other);
                    i += 2;
                }
                Tok::Name(s) => items.push(Item::Name(s.clone())),
                Tok::Open(_) => {
                    let j = matching(toks, i);
                    match made_at(toks, j + 1) {
                        None => items.push(Item::Quote(i + 1, j)),
                        Some("sig#") => items.push(Item::Bracket(i + 1, j)),
                        // A sequence, a map or a string takes what its
                        // code takes, its inputs among them.
                        Some(_) => {
                            let (k, _) = self.effect_in(&toks[i + 1..j], ns, visiting)?;
                            take(items, ins, k);
                            items.push(Item::Other);
                        }
                    }
                    i = j + if made_at(toks, j + 1).is_some() { 3 } else { 0 };
                }
                Tok::Apply | Tok::Test => {
                    let top = match items.pop()? {
                        Item::Name(s) => Item::Name(self.resolve_quiet(ns, &s)),
                        top => top,
                    };
                    let (a, b) = match top {
                        Item::Name(s) if self.words.get(&s).is_some_and(|w| w.ty.is_some()) => {
                            items.push(Item::Type);
                            i += 1;
                            continue;
                        }
                        // A symbol of digits given a type, as a rule's
                        // capture is, `2 int# !`, is a count.
                        Item::Name(s)
                            if is_core_word(&s, "!") && self.count_of(items).is_some() =>
                        {
                            let n = self.count_of(items).expect("checked");
                            items.truncate(items.len() - 2);
                            items.push(Item::Int(n));
                            i += 1;
                            continue;
                        }
                        // `!` types as many values as the type on top has
                        // slots, and takes the type too.
                        // A quotation on top runs while compiling, and leaves
                        // what its code leaves (6.32).
                        Item::Name(s) if is_core_word(&s, "!") => match items.last() {
                            Some(Item::Bracket(from, to)) => {
                                let n = self.bracket_size(&toks[*from..*to]);
                                (n + 1, n)
                            }
                            Some(&Item::Quote(from, to)) => {
                                let (a, b) = self.effect_in(&toks[from..to], ns, visiting)?;
                                (a + 1, b)
                            }
                            _ => (2, 1),
                        },
                        // A quotation's power, of the code and the count
                        // before it, as `[ dup + ]^3` is spelled.
                        Item::Name(s) if is_core_word(&s, "power#") => {
                            let (Some(Item::Int(n)), Some(code)) = (items.pop(), items.pop())
                            else {
                                return None;
                            };
                            let e = match code {
                                Item::Quote(from, to) => {
                                    self.effect_in(&toks[from..to], ns, visiting)?
                                }
                                Item::Name(q) => self.name_effect(ns, &q, visiting)?,
                                Item::Code(a, b) => (a, b),
                                _ => return None,
                            };
                            let (a, b) = power_effect(e, usize::try_from(n).ok()?)?;
                            items.push(Item::Code(a, b));
                            i += 1;
                            continue;
                        }
                        // A word's width, of the word and the count before
                        // it, as the core's rule spells `dup_2`: n times what
                        // the word does, and `dip`'s sets aside n values.
                        Item::Name(s) if is_core_word(&s, "width#") => {
                            let (Some(Item::Int(n)), Some(Item::Name(w))) =
                                (items.pop(), items.pop())
                            else {
                                return None;
                            };
                            let n = usize::try_from(n).ok()?;
                            if self.is_dip(&w) {
                                let (a, b) =
                                    self.handed_effect(items.pop()?, toks, ns, visiting)?;
                                (a + n, b + n)
                            } else {
                                let (a, b) = self.name_effect(ns, &w, visiting)?;
                                (a * n, b * n)
                            }
                        }
                        // A symbol read as a name is still that name, as
                        // far as its effect goes: a rule's capture, which
                        // is found where the name was applied.
                        Item::Name(s)
                            if is_core_word(&s, "name#")
                                && matches!(items.last(), Some(Item::Name(_))) =>
                        {
                            i += 1;
                            continue;
                        }
                        // `x q dip` runs q's code on what is beneath x, and
                        // puts x back.
                        Item::Name(s)
                            if self.is_dip(&s) && matches!(items.last(), Some(Item::Quote(..))) =>
                        {
                            let Some(Item::Quote(from, to)) = items.pop() else {
                                unreachable!("checked")
                            };
                            let x = items.pop().unwrap_or_else(|| {
                                *ins += 1;
                                Item::Other
                            });
                            self.effect_on(toks, from..to, ns, visiting, items, ins)?;
                            items.push(x);
                            i += 1;
                            continue;
                        }
                        // `x q dip` takes and leaves one more than q.
                        Item::Name(s) if self.is_dip(&s) => {
                            let (a, b) = self.handed_effect(items.pop()?, toks, ns, visiting)?;
                            (a + 1, b + 1)
                        }
                        Item::Name(s) => self.name_effect(ns, &s, visiting)?,
                        Item::Type => return None,
                        Item::Quote(from, to) => self.effect_in(&toks[from..to], ns, visiting)?,
                        Item::Code(a, b) => (a, b),
                        Item::Bracket(..) | Item::Int(_) | Item::Other => return None,
                    };
                    take(items, ins, a);
                    items.extend((0..b).map(|_| Item::Other));
                }
                _ => return None,
            }
            i += 1;
        }
        Some(())
    }

    /// The effect of code handed to a word, as `dip` is handed it.
    fn handed_effect(
        &self,
        code: Item,
        toks: &[Token],
        ns: &str,
        visiting: &mut Vec<Rc<str>>,
    ) -> Option<(usize, usize)> {
        match code {
            Item::Quote(from, to) => self.effect_in(&toks[from..to], ns, visiting),
            Item::Name(q) => self.name_effect(ns, &q, visiting),
            Item::Code(a, b) => Some((a, b)),
            _ => None,
        }
    }

    /// The count a symbol of digits makes, given a type on top: a rule's
    /// capture, `2 int# !`.
    fn count_of(&self, items: &[Item]) -> Option<i128> {
        let (Item::Name(d), top) = (items.get(items.len().checked_sub(2)?)?, items.last()?) else {
            return None;
        };
        let typed = match top {
            Item::Type => true,
            Item::Name(t) => self
                .words
                .get(&self.resolve_quiet("", t))
                .is_some_and(|w| w.ty.is_some()),
            _ => false,
        };
        match read::number(d, Pos::default()) {
            Ok(Some(Tok::Int(n))) if typed => Some(n),
            _ => None,
        }
    }

    /// How many values a bracket in a body types when `!` structures by it:
    /// its code's values, each word applied taking and leaving what its
    /// effect says.
    fn bracket_size(&self, toks: &[Token]) -> usize {
        let mut n: isize = 0;
        let mut i = 0;
        while i < toks.len() {
            match &toks[i].tok {
                Tok::Int(_) | Tok::Dec(..) | Tok::Ratio(_) | Tok::Name(_) => n += 1,
                Tok::Apply | Tok::Test => {
                    if let Some(Tok::Name(s)) = i.checked_sub(1).map(|k| &toks[k].tok)
                        && let Some((a, b)) = self.word_effect(s, &mut Vec::new())
                    {
                        n += b as isize - a as isize - 1;
                    }
                }
                // A quotation is no slot, a guard's, and a tuple type is one.
                Tok::Open(_) => {
                    i = matching(toks, i);
                    if made_at(toks, i + 1).is_some() {
                        i += 3;
                        n += 1;
                    }
                }
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
            return self.run_quote(&c.body);
        };
        self.settle(sig, &env)?;
        let base = self.stack.len() - sig.ins.len();
        self.reached(base);
        let floor = self.floor;
        self.floor = base;
        let r = self.run_quote(&c.body);
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
    /// it was. One that needs its inputs known while compiling, as `vec`'s
    /// length, cannot be compiled on values at run time, and is checked where
    /// it is applied instead.
    fn define(&mut self, name: &Rc<str>, clause: Clause) -> Result<()> {
        // Instances compiled before may apply the family changed.
        self.instances.clear();
        let ins = clause.sig.as_ref().map(|s| s.ins.clone());
        let c = Rc::new(clause);
        let context = c.sig.as_ref().map(Sig::context);
        let w = self.word(name);
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
        if typed
            && let Err(e) = self.check(name, &c)
            && e.kind != Kind::Unknown
        {
            self.word(name).clauses = before;
            return Err(e);
        }
        if let Some(sig) = &c.sig {
            self.pos = c.at;
            self.shadows(name, sig);
        }
        Ok(())
    }

    /// Warns of clauses of different lengths that both may match: the
    /// shorter is chosen then (TYPES.md 3.13). A new shorter clause may
    /// change what applying the family did; a new longer one, under a
    /// shorter without guards, is not chosen where that one matches.
    fn shadows(&mut self, name: &Rc<str>, new: &Sig) {
        // A handler is chosen only for a trap, which no other clause is.
        if new.ins.last() == Some(&TRAP) {
            return;
        }
        let clauses = self.clauses(name, false);
        for old in &clauses {
            let Some(o) = &old.sig else { continue };
            let msg = if o.ins.len() > new.ins.len() && self.overlap(new, o) {
                format!(
                    "this clause of `{name}`, {}, takes fewer values than its clause {}, \
                     so it is chosen wherever both match",
                    self.sig_text(new),
                    self.sig_text(o)
                )
            } else if o.ins.len() < new.ins.len() && o.guards.is_empty() && self.overlap(o, new) {
                format!(
                    "this clause of `{name}`, {}, takes more values than its clause {}, \
                     which is chosen wherever both match",
                    self.sig_text(new),
                    self.sig_text(o)
                )
            } else {
                continue;
            };
            self.warn(Level::Informative, msg);
        }
    }

    /// Whether a value could match both a shorter signature and the same
    /// inputs of a longer one, its top ones: as far as types and value
    /// patterns tell, since a guard's test is not known.
    fn overlap(&mut self, short: &Sig, long: &Sig) -> bool {
        let off = long.ins.len() - short.ins.len();
        for k in 0..short.ins.len() {
            if let (Some(a), Some(b)) = (short.pattern(k), long.pattern(off + k))
                && Test::Equals(a.clone()) != Test::Equals(b.clone())
            {
                return false;
            }
            let (s, l) = (short.ins[k], long.ins[off + k]);
            let loose = |me: &Self, t| me.types.is_pattern(t) || me.types.is_literal(t);
            if s == l || loose(self, s) || loose(self, l) {
                continue;
            }
            if self.unify(s, l, &mut Env::default()).is_none()
                && self.unify(l, s, &mut Env::default()).is_none()
            {
                return false;
            }
        }
        true
    }

    /// A signature as a message writes it, its value patterns by value.
    fn sig_text(&self, sig: &Sig) -> String {
        let slots: Vec<String> = sig
            .ins
            .iter()
            .enumerate()
            .map(|(k, &t)| match sig.pattern(k) {
                Some(j) => value_text(j),
                None => self.types.name(t),
            })
            .collect();
        format!("`< {} >`", slots.join(" "))
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
        self.checking += 1;
        let mut r = r.and_then(|_| self.clause(c, Env::default()));
        self.checking -= 1;
        // Failing, it leaves nothing to check (doc/design/SYMBOLIC-FORTH.md
        // 29).
        let fails = matches!(&r, Err(e) if e.kind == Kind::Fails);
        if fails {
            r = Ok(());
        }
        for i in 0..self.stack.len() {
            if r.is_ok() && !fails {
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
    /// 2026-10-09), read as all code is: a type's name gives the type, and a
    /// constructor builds one, `a ary` (2026-10-12). A variable, one letter,
    /// is a type variable, and a word or a quotation tested, `positive?` or
    /// `[ 2 mod zero ] ?`, is a guard. What the code leaves is the
    /// signature, by what each value is: a type, inputs of that type; a
    /// value, a literal, a symbol, `true`, inputs equal to it; a test, a
    /// guard, which tests the inputs before it.
    /// A clause's outputs are a check last in its body, `< i64 >.` (6.32),
    /// which `outputs` reads.
    pub fn bracket(&mut self, toks: &[Token]) -> Result<Sig> {
        let mut ins = Vec::new();
        let mut equals = Vec::new();
        let mut guards = Vec::new();
        for (j, pos) in self.comprehend(toks)? {
            match j.val {
                Val::Type(t) => ins.push(t),
                // A trap of one kind, `no-file' trap`, a value to match.
                Val::Name(..) if j.ty == TRAP => {
                    equals.push(Guard {
                        test: Test::Equals(j.clone()),
                        slot: ins.len(),
                        arity: 1,
                        pos,
                    });
                    ins.push(TRAP);
                }
                // A name left is a symbol, `< string Camel >`: a type's name
                // applies, and gives the type (Thomas, 2026-10-12).
                Val::Name(n, at) => {
                    equals.push(Guard {
                        test: Test::Equals(Jdg {
                            ty: SYMBOL,
                            val: Val::Name(n, at),
                        }),
                        slot: ins.len(),
                        arity: 1,
                        pos,
                    });
                    ins.push(SYMBOL);
                }
                // `nil`, the one value of its type, is that type.
                _ if j.ty == NIL => ins.push(NIL),
                Val::Quote(q) if j.ty == BOOL => {
                    guards.push(self.quote_guard(q, ins.len(), pos)?)
                }
                Val::Quote(q) if signature(&q) => return Err(quotation_type(&q, pos)),
                // Each role has its mark (doc/design/SYMBOLIC-FORTH.md 2): a
                // test is tested.
                Val::Quote(q) => {
                    let text = source_text(&q);
                    return Err(self.at_pos(
                        pos,
                        Kind::Syntax,
                        format!("`[ {text} ]` is left bare, and a guard is a test: `[ {text} ] ?`"),
                    ));
                }
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
        equals.extend(guards);
        for &t in &ins {
            self.named(t)?;
        }
        Ok(Sig {
            ins,
            guards: equals,
            outs: None,
        })
    }

    /// A clause's outputs, the code of its second bracket: types.
    fn outputs(&mut self, toks: &[Token]) -> Result<Vec<Type>> {
        let mut outs = Vec::new();
        for (j, pos) in self.comprehend(toks)? {
            match j.val {
                Val::Type(t) => outs.push(t),
                _ if j.ty == NIL => outs.push(NIL),
                Val::Quote(q) if signature(&q) => return Err(quotation_type(&q, pos)),
                _ => {
                    self.pos = pos;
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("an output is a type, and {} is not one", self.describe(&j)),
                    ));
                }
            }
        }
        Ok(outs)
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
                Kind::Unknown,
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

    /// A test held in a bracket: a guard, testing the inputs before it,
    /// as many as it takes, and leaving one flag.
    fn quote_guard(&self, q: Rc<[Token]>, before: usize, pos: Pos) -> Result<Guard> {
        let text = source_text(&q);
        let Some((ins, outs)) = self.effect(&q, &mut Vec::new()) else {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!("`{text}` cannot be a guard: how many values it takes is not known"),
            ));
        };
        if outs != 1 {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!(
                    "`{text}` leaves {}, and a guard leaves one flag",
                    values(outs)
                ),
            ));
        }
        if ins > before {
            return Err(Error::new(
                Kind::Mismatch,
                Some(pos),
                format!(
                    "the guard `{text}` looks at {}, and {} before it",
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

    /// Applies a constructor: `100 i64 vec` is a type.
    /// Whether the values on top are what a constructor takes, as its
    /// signature says: `< type type >. < type >.` for `map` and `or`, `nil`
    /// standing for its type.
    fn constructs(&mut self, c: Con) -> bool {
        let ins = match c {
            Con::Ary | Con::Own => vec![TYPE],
            Con::Map | Con::Or => vec![TYPE, TYPE],
            // A length or a variable, then a type: `vec` builds types alone.
            Con::Vec => return true,
            // A kind or a variable, and nothing else is one: `trap` alone.
            Con::Trap => return true,
        };
        let sig = Sig {
            ins,
            guards: Vec::new(),
            outs: Some(vec![TYPE]),
        };
        self.match_sig(&sig).is_some()
    }

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
                            Kind::Unknown,
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
            // A type of its own over a representation, named by the `;` that
            // binds it (TYPES.md 2.11), `celsius: f64 own ;`.
            Con::Own => {
                let r = self.pop_type()?;
                let t = self.types.own("", r);
                self.push(TYPE, Val::Type(t));
                return Ok(());
            }
            // A trap's kind is in its type (doc/design/SYMBOLIC-FORTH.md 29):
            // a letter takes any, `k trap`, as `a ary` is any array, and a
            // kind is a value of the type, a trap of that kind, to match.
            Con::Trap => {
                let j = self.pop()?;
                match j.val {
                    Val::Type(v) if matches!(self.types.term(v), Term::Var(_)) => {
                        self.push(TYPE, Val::Type(TRAP))
                    }
                    Val::Name(k, at) if j.ty == SYMBOL => self.push(TRAP, Val::Name(k, at)),
                    _ => {
                        return Err(self.err(
                            Kind::Mismatch,
                            format!(
                                "`trap` takes a kind, `no-file'`, or a letter for any, `k`, \
                                 and {} is neither",
                                self.describe(&j)
                            ),
                        ));
                    }
                }
                return Ok(());
            }
        };
        let t = self.types.intern(t);
        self.push(TYPE, Val::Type(t));
        Ok(())
    }

    /// `;` on a type: the name is a name for it, as `num: < i64 f64 or > ;`
    /// names a class, in a bracket or applied to a value.
    fn name_type(&mut self, name: &Rc<str>, t: Type) -> Result<()> {
        // A type of its own takes the name of the path that binds it, and
        // one named already is an alias, `temp: celsius ;` (TYPES.md 2.11).
        let t = match self.types.own_parts(t) {
            Some((n, r)) if n.is_empty() => {
                let t = self.types.own(name, self.named(r)?);
                self.type_names.insert(t, name.clone());
                t
            }
            _ => self.named(t)?,
        };
        let w = self.word(name);
        if w.ty.is_some() || w.con.is_some() {
            return Err(self.err(Kind::Mismatch, format!("`{name}` names a type already")));
        }
        w.ty = Some(t);
        let shown = name.strip_prefix(&format!("{CORE}.")).unwrap_or(name);
        self.types.name_class(t, shown);
        self.instances.clear();
        Ok(())
    }

    /// A type, if each type of its own in it is named: one is named where
    /// it is defined, by `;` (TYPES.md 2.11).
    fn named(&self, t: Type) -> Result<Type> {
        match self.types.unnamed(t) {
            true => Err(self.err(
                Kind::Mismatch,
                format!(
                    "{} is a type of its own, named where it is defined: `celsius: f64 own ;`",
                    self.types.name(t)
                ),
            )),
            false => Ok(t),
        }
    }

    fn pop_type(&mut self) -> Result<Type> {
        let j = self.pop()?;
        match &j.val {
            Val::Type(t) => self.named(*t),
            _ if j.ty == NIL => Ok(NIL),
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
        let t = self.stack[i].ty;
        if self.types.is_literal(t) {
            self.annotate(i, Self::default_of(t))?;
        }
        // A map literal undecided is a map, with nothing to take it.
        if self.types.is_record_lit(t) {
            let m = self.lit_map(t);
            return self.annotate(i, m);
        }
        let j = self.stack[i].clone();
        let ins = self.code_of(&j)?;
        if self.stack[i + 1..].iter().any(|j| j.val == Val::Many) {
            return Err(self.err(
                Kind::Mismatch,
                "a value cannot be placed beneath a run of values in an array literal",
            ));
        }
        let above = self.stack[i + 1..].iter().filter(|j| !j.known()).count();
        match above {
            0 => self.code.extend(ins),
            1 => {
                self.code.extend(ins);
                self.emit(Op::Prim(P::Swap));
            }
            2 => {
                self.code.extend(ins);
                self.emit(Op::Prim(P::Rot));
                self.emit(Op::Prim(P::Rot));
            }
            n => {
                for _ in 0..n {
                    self.emit(Op::Prim(P::ScratchPush));
                }
                self.code.extend(ins);
                for _ in 0..n {
                    self.emit(Op::Prim(P::ScratchPop));
                }
            }
        }
        self.stack[i].val = Val::Run;
        Ok(())
    }

    /// The code that leaves a known value: a known array's, its elements
    /// gathered.
    fn code_of(&self, j: &Jdg) -> Result<Vec<Ins>> {
        Ok(match &j.val {
            Val::Int(n) => vec![Ins::Op(Op::Lit(*n as i64 as u64))],
            Val::Float(x) => vec![Ins::Op(Op::Float(float_bits(*x)))],
            // A symbol is its name, an interned string, so symbols compare
            // by identity.
            Val::Str(s) | Val::Name(s, ..) => vec![Ins::Str(s.clone())],
            Val::Seq(es) => {
                let mut ins = vec![Ins::Op(Op::Prim(P::Mark))];
                for e in es.iter() {
                    ins.extend(self.code_of(e)?);
                }
                ins.push(Ins::Op(Op::Prim(P::Gather)));
                ins
            }
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "{} is left unapplied: it has no value at run time",
                        self.describe(j)
                    ),
                ));
            }
        })
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
        // Its inputs are the type on top and as many values as it types, of
        // any type, a literal kept a literal until it converts.
        if p == Prim::Convert {
            return self.convert_by();
        }
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
            Prim::Dip => self.dip(name)?,
            Prim::QuotePower => self.quote_power()?,
            Prim::Width => self.width()?,
            Prim::Fail => return Err(self.fail()),
            Prim::TrapKind => self.trap_part(1),
            Prim::TrapWhy => self.trap_part(0),
            Prim::QuoteLength
            | Prim::QuoteAt
            | Prim::QuoteSlice
            | Prim::QuoteConcat
            | Prim::QuoteInsert => self.quote_data(p, base)?,
            Prim::NameOf => self.name_of()?,
            Prim::Clauses => self.clauses_of()?,
            Prim::Effect => self.effect_counts()?,
            Prim::Var => self.var()?,
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
            Prim::RecordAt => self.record_at(base)?,
            Prim::StrToI64 => self.parse(I64, base)?,
            Prim::StrToF64 => self.parse(F64, base)?,
            // An empty array or map of a type: `string i64 map empty`.
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
                let mut words = applied(&q, self.pos);
                words.extend(applied(&r, self.pos));
                self.push(QUOTE, Val::Quote(words.into()));
            }
            Prim::Def => self.semicolon()?,
            _ => {
                let outs: Vec<Type> = sig
                    .outs
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|&t| self.types.subst(t, &env))
                    .collect();
                if self.fold(p, base, &outs)? {
                } else if prims::compile_time_only(p) {
                    return Err(self.err(
                        Kind::Unknown,
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

    /// Folds a primitive on the values from `base`, if they are all known,
    /// and says whether it did. A literal has no value at run time, and is
    /// folded by `prims::fold`; any other value by running the primitive's
    /// operations on the stage's own machine, so that a primitive has one
    /// meaning, compiled or folded: running while compiling is running, as
    /// FORTH's immediate words are. Not a primitive with an effect, whose
    /// reads are facts of the run and whose writes must happen then, nor
    /// one whose results have no known value here, as an array has none.
    fn fold(&mut self, p: Prim, base: usize, outs: &[Type]) -> Result<bool> {
        let known = self.stack[base..]
            .iter()
            .all(|j| j.known() || matches!(j.val, Val::Pinned(_)));
        let held = |t: &Type| matches!(*t, I64 | F64 | MONEY | BOOL | STRING | SYMBOL);
        let literal = prims::compile_time_only(p);
        if !known || prims::effects(p) != 0 || !(literal || outs.iter().all(held)) {
            return Ok(false);
        }
        let args: Vec<Jdg> = self.stack[base..]
            .iter()
            .map(|j| match &j.val {
                Val::Pinned(v) => Jdg {
                    ty: j.ty,
                    val: (**v).clone(),
                },
                _ => j.clone(),
            })
            .collect();
        let vals = if literal {
            let vals: Vec<Val> = args.iter().map(|j| j.val.clone()).collect();
            prims::fold(p, &vals).map_err(|(k, m)| self.err(k, m))?
        } else {
            let def = PRIMS.iter().find(|d| d.prim == p).expect("listed");
            match self.run_now(def.ops, &args, outs) {
                Ok(vals) => vals,
                // Where it is handled, it fails as the program runs.
                Err(e) if self.handled > 0 && e.trap().is_some() => return Ok(false),
                Err(e) => {
                    let ins: Vec<String> = args.iter().map(value_text).collect();
                    let outs: Vec<String> = outs.iter().map(|&t| self.types.name(t)).collect();
                    let kind = match e {
                        machine::Error::Arithmetic => Kind::Arithmetic,
                        machine::Error::Limit => Kind::Limit,
                        _ => Kind::Mismatch,
                    };
                    let what = match &outs[..] {
                        [t] => format!("no {t} result"),
                        _ => "no result".into(),
                    };
                    return Err(self.err(
                        kind,
                        format!("`{}` on {} has {what}", def.name, ins.join(" and ")),
                    ));
                }
            }
        };
        // A pinned input's cell goes: its value is known. They are the only
        // cells among the inputs, so on top.
        for j in &self.stack[base..] {
            if matches!(j.val, Val::Pinned(_)) {
                self.code.push(Ins::Op(Op::Prim(P::Drop)));
            }
        }
        self.stack.truncate(base);
        for (val, &ty) in vals.into_iter().zip(outs) {
            self.push(ty, val);
        }
        Ok(true)
    }

    /// Runs operations on known values on the stage's machine, and reads
    /// back their results, of the given types: a number is its cell, and a
    /// string or a symbol is text, its name.
    fn run_now(
        &mut self,
        ops: &[P],
        args: &[Jdg],
        outs: &[Type],
    ) -> std::result::Result<Vec<Val>, machine::Error> {
        let m = self.machine.get_or_insert_with(Machine::empty);
        m.stack.clear();
        // A known array is gathered there, as its code would.
        fn cell(m: &mut Machine, j: &Jdg) -> std::result::Result<u64, machine::Error> {
            Ok(match &j.val {
                Val::Int(n) => *n as i64 as u64,
                Val::Float(x) => float_bits(*x),
                Val::Str(s) | Val::Name(s, ..) => m.text_cell(s)?,
                Val::Seq(es) => {
                    let cells = es
                        .iter()
                        .map(|e| cell(m, e))
                        .collect::<std::result::Result<_, _>>()?;
                    m.vector_cell(cells)?
                }
                _ => unreachable!("a known value with a cell"),
            })
        }
        for j in args {
            let c = cell(m, j)?;
            m.stack.push(c);
        }
        m.apply(ops)?;
        let cells = std::mem::take(&mut m.stack);
        assert_eq!(cells.len(), outs.len(), "a primitive leaves its results");
        let mut vals = Vec::with_capacity(outs.len());
        for (&c, &t) in cells.iter().zip(outs) {
            vals.push(match t {
                F64 => Val::Float(f64::from_bits(c)),
                STRING | SYMBOL => {
                    let s: Rc<str> = m.text(c)?.to_text().ok_or(machine::Error::Memory)?.into();
                    if t == STRING {
                        Val::Str(s)
                    } else {
                        Val::Name(s, Pos::default())
                    }
                }
                _ => Val::Int((c as i64).into()),
            });
        }
        Ok(vals)
    }

    /// `[ … ] !`: runs a quotation while compiling, and leaves its values,
    /// known (doc/design/SYMBOLIC-FORTH.md 6.32), as FORTH's `[ … ] LITERAL`
    /// does. It takes nothing from below, known only as the program runs.
    /// Its effects run, a file read as Rust's `include_str!` reads one, and
    /// what it read goes into the code as data, so the code's identity
    /// follows it; each quotation runs once, kept by its code's identity. A
    /// value it leaves that is known as its code is compiled stays as it is,
    /// a quotation or a type too, so `[ … ] ! .` applies code made while
    /// compiling.
    fn now(&mut self, q: Rc<[Token]>) -> Result<()> {
        let at = self.pos;
        if let Some((ins, _)) = self.effect(&q, &mut Vec::new())
            && ins > 0
        {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`[ … ] !` takes nothing from below, known only as the program runs, \
                     and this quotation takes {}",
                    values(ins)
                ),
            ));
        }
        let saved = (
            std::mem::take(&mut self.stack),
            std::mem::take(&mut self.code),
            std::mem::replace(&mut self.floor, 0),
            std::mem::take(&mut self.literals),
            std::mem::take(&mut self.inputs),
            std::mem::take(&mut self.active),
            std::mem::take(&mut self.frames),
            std::mem::replace(&mut self.effects, 0),
            // Its code runs now, whether or not where it is applied runs.
            std::mem::replace(&mut self.unsure, 0),
            std::mem::replace(&mut self.checking, 0),
            std::mem::replace(&mut self.handled, 0),
        );
        let r = self.run_code(&q).and_then(|()| {
            let reach = self
                .stack
                .iter()
                .any(|j| matches!(j.val, Val::Many | Val::Pinned(_)));
            match reach {
                true => Err(self.err(Kind::Mismatch, "`[ … ] !` leaves values it cannot give")),
                false => Ok(()),
            }
        });
        let left = std::mem::take(&mut self.stack);
        let code = std::mem::take(&mut self.code);
        (
            self.stack,
            self.code,
            self.floor,
            self.literals,
            self.inputs,
            self.active,
            self.frames,
            self.effects,
            self.unsure,
            self.checking,
            self.handled,
        ) = saved;
        r?;
        self.pos = at;
        // With no code, all it leaves is known already.
        if code.is_empty() {
            self.stack.extend(left);
            return Ok(());
        }
        let (_, cid) = self.seal(code);
        let vals = match self.ran.get(&cid) {
            Some(vals) => vals.clone(),
            None => {
                let vals: Rc<[Jdg]> = self.ran_now(cid, left)?.into();
                self.ran.insert(cid, vals.clone());
                vals
            }
        };
        self.stack.extend(vals.iter().cloned());
        Ok(())
    }

    /// Runs code sealed while compiling, on the stage's machine, and reads
    /// back what it left: the values known already as they were, and those
    /// it made by their types.
    fn ran_now(&mut self, cid: Cid, left: Vec<Jdg>) -> Result<Vec<Jdg>> {
        let m = self.machine.get_or_insert_with(Machine::empty);
        m.stack.clear();
        let r = m.link(cid).and_then(|xt| m.run(xt, FUEL));
        self.output.extend(m.take_output());
        if let Err(e) = r {
            let mut e: Error = e.into();
            e.kind = match e.kind {
                Kind::Run(machine::Error::Trap(kind, _)) => Kind::Fail(kind),
                Kind::Run(machine::Error::Io(_)) => Kind::Io,
                Kind::Run(machine::Error::Arithmetic) => Kind::Arithmetic,
                Kind::Run(machine::Error::Fuel) => Kind::Limit,
                k => k,
            };
            e.msg = format!("{}, in `[ … ] !`, run while compiling", e.msg);
            e.pos = Some(self.pos);
            return Err(e);
        }
        let m = self.machine.as_ref().expect("made");
        let cells = m.stack.clone();
        assert_eq!(
            cells.len(),
            left.iter().filter(|j| !j.known()).count(),
            "the stage's judgments match the machine's stack"
        );
        let mut cells = cells.into_iter();
        let mut vals = Vec::with_capacity(left.len());
        for j in left {
            if j.known() {
                vals.push(j);
                continue;
            }
            let c = cells.next().expect("counted");
            let Some(val) = known_of(m, &self.types, c, j.ty) else {
                return Err(self.err(
                    Kind::Limit,
                    format!(
                        "`[ … ] !` leaves {}, which march8 cannot yet read back while \
                         compiling: numbers, strings, symbols, and arrays and tuples of them",
                        self.types.name(j.ty)
                    ),
                ));
            };
            vals.push(Jdg { ty: j.ty, val });
        }
        Ok(vals)
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
        // A loop's body runs only if the array has elements.
        self.unsure += 1;
        let r = self.run_quote(&q);
        self.unsure -= 1;
        self.depth -= 1;
        self.loop_fails(r, name)?;
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
        // A loop's body runs only if the array has elements.
        self.unsure += 1;
        let r = self.run_quote(&q);
        self.unsure -= 1;
        self.depth -= 1;
        self.loop_fails(r, name)?;
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

    /// `x q dip`: q applied with x set aside, out of its reach, and x put
    /// back on top. It is FORTH's `>r q r>` in one word, so nothing can be
    /// left aside.
    fn dip(&mut self, name: &str) -> Result<()> {
        let q = self.words_of(name)?;
        self.aside(1, |s| s.run_code(&q))
    }

    /// Sets the top `m` values aside while `f` compiles, those at run time
    /// on the scratch stack, and puts them back. A value known at compile
    /// time has no cell to set aside, and if `f` leaves no code, none moves.
    fn aside(&mut self, m: usize, f: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        self.need(m)?;
        let set = self.stack.split_off(self.stack.len() - m);
        let cells = set.iter().filter(|j| !j.known()).count();
        for _ in 0..cells {
            self.emit(Op::Prim(P::ScratchPush));
        }
        let start = self.code.len();
        f(self)?;
        if self.code.len() == start {
            self.code.truncate(start - cells);
        } else {
            for _ in 0..cells {
                self.emit(Op::Prim(P::ScratchPop));
            }
        }
        self.stack.extend(set);
        Ok(())
    }

    fn run_code(&mut self, q: &[Token]) -> Result<()> {
        self.enter()?;
        let r = self.run_quote(q);
        self.depth -= 1;
        r
    }

    /// Applies a bracket's code, its letters taking its inputs first
    /// (doc/design/SYMBOLIC-FORTH.md 14).
    fn run_quote(&mut self, q: &[Token]) -> Result<()> {
        let uses = letters_in(q);
        if uses.is_empty() {
            return self.run(q);
        }
        self.take_inputs(q, uses, true)?;
        let n = self.inputs.len();
        let r = self.run(q);
        self.inputs.truncate(n - 1);
        r
    }

    /// Takes a bracket's inputs as it is applied (doc/design/SYMBOLIC-FORTH.md
    /// 14): as many as its deepest letter names, from the top, those no
    /// letter names dropped, so its code may reach below them. Those its
    /// letters name once its code has begun wait, those known only at run
    /// time on the scratch stack, the one used last at the bottom, so that
    /// letters in the stack's order give the plain word. If `given`, the
    /// letters its code begins with are given now, the inputs copied and
    /// moved into their places; a literal's are not, its elements being
    /// what its code leaves above its inputs.
    fn take_inputs(
        &mut self,
        q: &[Token],
        uses: Vec<(usize, usize, usize)>,
        given: bool,
    ) -> Result<()> {
        let k = uses.iter().map(|u| u.1).max().unwrap_or(0);
        let have = self.stack.len() - self.floor;
        if have < k {
            let i = uses.iter().find(|u| u.1 == k).expect("the deepest").2;
            let Tok::Name(l) = &q[i].tok else {
                unreachable!("a letter")
            };
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`{l}`, at {}, is the {} input from the top, and {} in reach",
                    q[i].pos,
                    ordinal(k),
                    match have {
                        1 => "1 is".to_string(),
                        h => format!("{h} are"),
                    }
                ),
            ));
        }
        self.need(k)?;
        let base = self.stack.len() - k;
        let places: Vec<usize> = uses.iter().map(|u| u.1).collect();
        let first = if given {
            uses.iter()
                .enumerate()
                .take_while(|(n, u)| u.2 == 3 * n)
                .count()
        } else {
            0
        };
        let last = |p: usize| places.iter().rposition(|&q| q == p);
        let mut wait: Vec<usize> = Vec::new();
        for &p in &places[first..] {
            if !wait.contains(&p) {
                wait.push(p);
            }
        }
        wait.sort_by_key(|&p| last(p));
        let order: Vec<usize> = places[..first]
            .iter()
            .chain(&wait)
            .map(|&p| k - p)
            .collect();
        self.shuffle(base, &order)?;
        let mut vals = vec![None; k + 1];
        let mut slots = Vec::new();
        for &p in wait.iter().rev() {
            let j = self.stack.pop().expect("shuffled");
            if !j.known() {
                self.emit(Op::Prim(P::ScratchPush));
                slots.push(p);
            }
            vals[p] = Some(j);
        }
        self.inputs.push(Inputs {
            owns: uses.iter().map(|u| u.0).collect(),
            places,
            vals,
            slots,
            given: first,
        });
        Ok(())
    }

    /// A letter, `a var# .`: the input at its place, of the bracket it is
    /// written in (doc/design/SYMBOLIC-FORTH.md 14). One known only at run
    /// time is taken from the scratch stack where it is used last, if it is
    /// on top, and copied before; those on top used for the last time then
    /// go.
    fn letter(&mut self, at: usize, l: char) -> Result<()> {
        let Some(f) = self.inputs.iter().rposition(|f| f.owns.contains(&at)) else {
            return Err(self.err(
                Kind::Syntax,
                format!(
                    "`{l}` is an input of the bracket it is written in, and this one is in \
                     none; the symbol is `{l}'`"
                ),
            ));
        };
        let f = &mut self.inputs[f];
        let u = f.owns.iter().position(|&a| a == at).expect("owned");
        if u < f.given {
            return Ok(());
        }
        let p = f.places[u];
        let j = f.vals[p].clone().expect("taken");
        let mut ops = Vec::new();
        if !j.known() {
            let rest = &f.places[u + 1..];
            match f.slots.iter().rev().position(|&s| s == p).expect("waiting") {
                0 if !rest.contains(&p) => {
                    ops.push(Op::Prim(P::ScratchPop));
                    f.slots.pop();
                }
                0 => ops.push(Op::Prim(P::ScratchPeek)),
                d => ops.extend([Op::Lit(d as u64), Op::Prim(P::ScratchAt)]),
            }
            while f.slots.last().is_some_and(|s| !rest.contains(s)) {
                ops.extend([Op::Prim(P::ScratchPop), Op::Prim(P::Drop)]);
                f.slots.pop();
            }
        }
        for op in ops {
            self.emit(op);
        }
        self.stack.push(j);
        Ok(())
    }

    /// Puts in place of the values from `base` those `order` names by their
    /// places from `base`, the first the deepest: one named twice is copied,
    /// and one named nowhere dropped. Those `order` begins with that are in
    /// order already stay, as `b a` in `b a c`, and the rest are brought up
    /// in turn, each moved where it is named last and copied before. A known
    /// value moves for nothing.
    fn shuffle(&mut self, base: usize, order: &[usize]) -> Result<()> {
        let k = self.stack.len() - base;
        let mut at: Vec<Option<usize>> = (0..k).map(|v| Some(base + v)).collect();
        for v in (0..k).rev().filter(|v| !order.contains(v)) {
            if !self.lift(&mut at, v).known() {
                self.emit(Op::Prim(P::Drop));
            }
        }
        let kept = 1 + order.windows(2).take_while(|w| w[0] < w[1]).count();
        let kept = kept.min(order.len());
        for (n, &v) in order.iter().enumerate().skip(kept) {
            if !order[..kept].contains(&v) && !order[n + 1..].contains(&v) {
                let j = self.lift(&mut at, v);
                self.stack.push(j);
            } else {
                self.copy(at[v].expect("not moved"))?;
            }
        }
        Ok(())
    }

    /// Takes value `v` of those `at` follows from where it is, its cell, if
    /// it has one, brought to the top.
    fn lift(&mut self, at: &mut [Option<usize>], v: usize) -> Jdg {
        let i = at[v].take().expect("not moved");
        for a in at.iter_mut().flatten() {
            if *a > i {
                *a -= 1;
            }
        }
        let j = self.stack.remove(i);
        if !j.known() {
            let above = cells(&self.stack[i..]);
            self.roll(above + 1);
        }
        j
    }

    /// A word followed through other names for it, words whose one clause
    /// applies another, as `<>` does `dip` and `=` `dup`.
    fn alias_of(&self, name: &Rc<str>) -> Rc<str> {
        let mut name = name.clone();
        for _ in 0..DEPTH {
            let Some(w) = self.words.get(&name).filter(|w| w.prim.is_none()) else {
                break;
            };
            match &w.clauses[..] {
                [c] if c.sig.is_none() => match &c.body[..] {
                    [
                        Token {
                            tok: Tok::Name(n), ..
                        },
                        Token {
                            tok: Tok::Apply, ..
                        },
                    ] => name = n.clone(),
                    _ => break,
                },
                _ => break,
            }
        }
        name
    }

    fn is_dip(&self, name: &Rc<str>) -> bool {
        let name = self.alias_of(name);
        matches!(
            self.words.get(&name).and_then(|w| w.prim.as_ref()),
            Some((Prim::Dip, _))
        )
    }

    // ---- Powers and widths (doc/design/SYMBOLIC-FORTH.md 21, 22) ----

    /// The count a power takes, written in the source.
    fn count(&mut self, word: &str, least: i128) -> Result<usize> {
        let j = self.stack.pop().expect("matched");
        match j.val {
            Val::Int(k) if k >= least && (j.ty == INT_LIT || j.ty == I64) => {
                usize::try_from(k).map_err(|_| self.err(Kind::Limit, format!("`{word}` of {k}")))
            }
            _ => Err(self.err(
                Kind::Mismatch,
                format!(
                    "`{word}` takes a count, at least {least}, written in the source, \
                     and is given {}",
                    self.describe(&j)
                ),
            )),
        }
    }

    /// How a word or a quotation reads in a message.
    fn shown(j: &Jdg) -> String {
        match &j.val {
            Val::Name(n, ..) => format!("`{n}`"),
            _ => "the quotation".into(),
        }
    }

    /// `x fail`: fails, by a kind, `no-file' fail`, or a message and a kind,
    /// `( "can't read it" no-file' ) fail` (doc/design/SYMBOLIC-FORTH.md 29).
    /// It never returns: while compiling, it is an error where the failing
    /// code was applied, of the kind it names, the machine's own kinds by
    /// their names, `no-word'`; the program's, its own. Where the code may
    /// not run, in a choice made at run time or a loop's body, it fails only
    /// if it runs, a trap, and its kind and message may be known only then.
    /// Either way nothing after it is reached, which `Kind::Fails` says.
    fn fail(&mut self) -> Error {
        let x = self.stack.pop().expect("matched");
        let taken = match &x.val {
            Val::Name(k, _) => Some((None, k.clone())),
            Val::Seq(es) => match &es[..] {
                [
                    Jdg {
                        val: Val::Str(m), ..
                    },
                    Jdg {
                        val: Val::Name(k, _),
                        ..
                    },
                ] => Some((Some(m.clone()), k.clone())),
                _ => None,
            },
            _ => None,
        };
        let runs = !x.known()
            && (x.ty == SYMBOL
                || x.ty == TRAP
                || self.types.elements(x.ty) == Some(&[STRING, SYMBOL][..]));
        if taken.is_none() && !runs {
            return self.err(
                Kind::Mismatch,
                format!(
                    "`fail` takes a kind, `no-file'`, a message and a kind, \
                     `( \"can't read it\" no-file' )`, or a trap, and {} is none of them",
                    self.describe(&x)
                ),
            );
        }
        if self.unsure > 0 || self.handled > 0 {
            match taken {
                Some((why, kind)) => {
                    self.code.push(Ins::Str(why.unwrap_or_else(|| "".into())));
                    self.code.push(Ins::Str(kind));
                }
                None if x.ty == SYMBOL => {
                    self.code.push(Ins::Str("".into()));
                    self.emit(Op::Prim(P::Swap));
                }
                None => {
                    for op in Self::rethrow() {
                        self.emit(op);
                    }
                }
            }
            self.emit(Op::Prim(P::Trap));
            return self.err(Kind::Fails, "fails");
        }
        // Where the clause is checked, it leaves whatever it must from here.
        if self.checking > 0 {
            return self.err(Kind::Fails, "fails");
        }
        let Some((msg, kind)) = taken else {
            return self.err(
                Kind::Unknown,
                format!(
                    "this `fail` is sure to be reached, and code sure to fail fails while \
                     compiling, but its kind, {}, is known only as the program runs",
                    self.describe(&x)
                ),
            );
        };
        let msg = msg.map_or_else(|| format!("fails: `{kind}`"), |m| m.to_string());
        let kind = match &*kind {
            "syntax" => Kind::Syntax,
            "unfinished" => Kind::Unfinished,
            "no-word" => Kind::NoWord,
            "mismatch" => Kind::Mismatch,
            "literal" => Kind::Literal,
            "unknown" => Kind::Unknown,
            "arithmetic" => Kind::Arithmetic,
            "limit" => Kind::Limit,
            "effect" => Kind::Effect,
            "io" => Kind::Io,
            _ => Kind::Fail(kind.to_string()),
        };
        self.err(kind, msg)
    }

    /// The operations that leave a trap's message and its kind, the trap
    /// on top, for `trap` to fail with them again.
    fn rethrow() -> [Op; 6] {
        [
            Op::Prim(P::Dup),
            Op::Lit(0),
            Op::Prim(P::VecAt),
            Op::Prim(P::Swap),
            Op::Lit(1),
            Op::Prim(P::VecAt),
        ]
    }

    /// A trap's message, 0, or its kind, 1, in place of the trap on top.
    fn trap_part(&mut self, i: usize) {
        let j = self.stack.pop().expect("a trap");
        let ty = [STRING, SYMBOL][i];
        match j.val {
            Val::Seq(es) => self.stack.push(es[i].clone()),
            _ => {
                self.emit(Op::Lit(i as u64));
                self.emit(Op::Prim(P::VecAt));
                self.push(ty, Val::Run);
            }
        }
    }

    /// Fails by `kind`, with `why`, as the program runs: a trap.
    fn trap(&mut self, kind: &str, why: &str) {
        self.code.push(Ins::Str(self.shorten(why).into()));
        self.code.push(Ins::Str(kind.into()));
        self.emit(Op::Prim(P::Trap));
    }

    // ---- A quotation as data (doc/design/SYMBOLIC-FORTH.md 6) ----

    /// `length`, `at`, `slice`, `concat` and `insert` on a quotation, while
    /// compiling: its elements are what Low Stack holds, each a symbol or a
    /// quotation. Running one splices its elements in, where each symbol
    /// lands on the stack and each meta-op acts; taken apart, nothing runs,
    /// so a meta-op is held as the symbol of its name, `'.'`, and put back,
    /// it acts again.
    fn quote_data(&mut self, p: Prim, base: usize) -> Result<()> {
        let Val::Quote(q) = self.stack[base].val.clone() else {
            return Err(self.err(Kind::Unknown, "a quotation is taken apart while compiling"));
        };
        let elems = self.elements(&q)?;
        let n = elems.len() as i128;
        let int = |s: &Self, k: usize| match s.stack[base + k].val {
            Val::Int(i) => Ok(i),
            _ => Err(s.err(
                Kind::Unknown,
                "a position in a quotation must be known while compiling",
            )),
        };
        let out = match p {
            Prim::QuoteLength => Jdg {
                ty: I64,
                val: Val::Int(n),
            },
            Prim::QuoteAt => {
                let k = int(self, 1)?;
                let Some(i) = prims::element(k, n) else {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("there is no element {k} in a quotation of {n}"),
                    ));
                };
                self.element_value(&elems[i])
            }
            Prim::QuoteSlice => {
                let (i, j) = (int(self, 1)?, int(self, 2)?);
                if i == 0 {
                    return Err(self.err(
                        Kind::Mismatch,
                        "there is no element 0: elements count from 1",
                    ));
                }
                // The gap before element i, and the gap after element j.
                let a = if i > 0 { i - 1 } else { n + i };
                let (a, b) = match (usize::try_from(a), prims::end(j, n)) {
                    (Ok(a), Some(b)) if a <= b => (a, b),
                    _ => {
                        return Err(self.err(
                            Kind::Mismatch,
                            format!("{i} to {j} is outside a quotation of {n}"),
                        ));
                    }
                };
                self.joined(&elems[a..b])?
            }
            Prim::QuoteConcat => {
                let Val::Quote(r) = self.stack[base + 1].val.clone() else {
                    unreachable!("matched a quotation")
                };
                let mut all = elems;
                all.extend(self.elements(&r)?);
                self.joined(&all)?
            }
            Prim::QuoteInsert => {
                let k = int(self, 2)?;
                let Some(g) = prims::end(k, n) else {
                    return Err(self.err(
                        Kind::Mismatch,
                        format!("there is no gap {k} in a quotation of {n}"),
                    ));
                };
                let v = self.stack[base + 1].clone();
                let mut all = elems;
                all.insert(g, self.element_of(&v)?);
                self.joined(&all)?
            }
            _ => unreachable!("{p:?} takes no quotation apart"),
        };
        self.stack.truncate(base);
        self.stack.push(out);
        Ok(())
    }

    /// A quotation's elements, as Low Stack holds them, each a run of its
    /// tokens: a name is a symbol, a number three, `10 int# !`, a meta-op
    /// one, `.`, `?`, `!` or `;`, and a quotation inside one, whole, a
    /// string's among them, `[ … ] str# !` (section 7). Low Stack has no
    /// other bracket, so a quotation holding a signature the stage keeps,
    /// `< … >`, is not taken apart.
    fn elements(&self, q: &[Token]) -> Result<Vec<Vec<Token>>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < q.len() {
            let t = &q[i];
            let applied = q.get(i + 1).is_some_and(|t| t.tok == Tok::Apply);
            let sym = |s: String| Token {
                tok: Tok::Name(s.into()),
                pos: t.pos,
            };
            let lit = |n: String, kind: &str| {
                let bang = [
                    sym("!".into()),
                    Token {
                        tok: Tok::Apply,
                        pos: t.pos,
                    },
                ];
                vec![vec![sym(n)], vec![sym(kind.into())], bang.to_vec()]
            };
            match &t.tok {
                Tok::Name(m) if matches!(&**m, "!" | ";") && applied => {
                    out.push(q[i..i + 2].to_vec());
                    i += 1;
                }
                Tok::Name(m) if matches!(&**m, "." | "?" | "!" | ";") => {
                    return Err(self.at_pos(
                        t.pos,
                        Kind::Mismatch,
                        format!(
                            "a quotation holding the symbol `{m}` is not taken apart: \
                             a meta-op taken out is the symbol of its name, so the two would \
                             be one"
                        ),
                    ));
                }
                Tok::Name(_) | Tok::Apply | Tok::Test => out.push(vec![t.clone()]),
                Tok::Int(k) => out.extend(lit(k.to_string(), "int#")),
                Tok::Dec(d, s) => out.extend(lit(prims::show_dec(*d, *s), "dec#")),
                Tok::Ratio(r) => out.extend(lit(prims::show_ratio(r), "ratio#")),
                Tok::Open(b'[') => {
                    let mut depth = 0;
                    let end = (i..q.len())
                        .find(|&j| {
                            match q[j].tok {
                                Tok::Open(_) => depth += 1,
                                Tok::Close(_) => depth -= 1,
                                _ => {}
                            }
                            depth == 0
                        })
                        .expect("a quotation's brackets close");
                    out.push(q[i..=end].to_vec());
                    i = end;
                }
                tok => {
                    let what = read::lower(&[Token {
                        tok: tok.clone(),
                        pos: t.pos,
                    }]);
                    return Err(self.at_pos(
                        t.pos,
                        Kind::Mismatch,
                        format!(
                            "a quotation is taken apart as Low Stack holds it, and Low Stack \
                             has no `{what}` (doc/design/SYMBOLIC-FORTH.md 7)"
                        ),
                    ));
                }
            }
            i += 1;
        }
        Ok(out)
    }

    fn at_pos(&self, pos: Pos, kind: Kind, msg: String) -> Error {
        Error::new(kind, Some(pos), msg)
    }

    /// An element as a value: a symbol, global text, or a quotation. A name
    /// expanded where it was written is its path, and taken out, the
    /// shortest name the root finds it by, `swap'` for `core.swap`.
    fn element_value(&self, e: &[Token]) -> Jdg {
        let at = e[0].pos;
        let sym = |s: &Rc<str>| Jdg {
            ty: SYMBOL,
            val: Val::Name(self.short(s), at),
        };
        match e {
            [
                Token {
                    tok: Tok::Apply, ..
                },
            ] => sym(&".".into()),
            [Token { tok: Tok::Test, .. }] => sym(&"?".into()),
            [
                Token {
                    tok: Tok::Name(m), ..
                },
                ..,
            ] => sym(m),
            [_, inner @ .., _] => Jdg {
                ty: QUOTE,
                val: Val::Quote(inner.into()),
            },
            _ => unreachable!("an element is a symbol or a quotation"),
        }
    }

    /// A value as an element.
    fn element_of(&self, j: &Jdg) -> Result<Vec<Token>> {
        // Put in by the core's code, it is where the program applied that.
        let pos = self.site.unwrap_or(self.pos);
        let tok = |tok| Token { tok, pos };
        Ok(match &j.val {
            Val::Name(s, _) => match &**s {
                "." => vec![tok(Tok::Apply)],
                "?" => vec![tok(Tok::Test)],
                "!" | ";" => vec![tok(Tok::Name(s.clone())), tok(Tok::Apply)],
                _ => vec![tok(Tok::Name(s.clone()))],
            },
            Val::Quote(q) => {
                let mut e = vec![tok(Tok::Open(b'['))];
                e.extend(q.iter().cloned());
                e.push(tok(Tok::Close(b']')));
                e
            }
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "a quotation holds symbols and quotations, and {} is neither",
                        self.describe(j)
                    ),
                ));
            }
        })
    }

    /// A quotation of elements, its numbers read back, `10 int# !` as `10`.
    /// Its names were expanded where they were written, so it means what
    /// its pieces meant, wherever they came from (16).
    fn joined(&mut self, elems: &[Vec<Token>]) -> Result<Jdg> {
        let mut toks = Vec::new();
        for t in elems.iter().flatten() {
            toks.push(t.clone());
            read::fold(&mut toks)?;
        }
        Ok(Jdg {
            ty: QUOTE,
            val: Val::Quote(toks.into()),
        })
    }

    // ---- A word's clauses as data (doc/design/SYMBOLIC-FORTH.md 6) ----

    /// `w ins clauses#`: the clauses of the word w that dispatch would
    /// choose for inputs of the types ins gives, a type, or code that leaves
    /// them, `< i64 i64 >` or `[ i64 i64 ]`, or for values, `[ i64+' ]`, in
    /// the order it would try them: those with guards first, as they were
    /// defined, then the best without. A value pattern another value is
    /// given for is not chosen.
    /// Each is a quotation of what `;` takes after the name, each bracket as
    /// the code written in it: the inputs, the outputs if it gives them, and
    /// the body, `[ [ a a gt ? ] [ a ] [ drop ] ]`. A primitive has none, its
    /// code being the machine's. Dispatch is the stage's, so it is not
    /// written twice.
    fn clauses_of(&mut self) -> Result<()> {
        let ins = self.stack.pop().expect("matched");
        let w = self.stack.pop().expect("matched");
        let (types, values) = match &ins.val {
            Val::Type(t) => (vec![*t], Vec::new()),
            Val::Quote(q) => {
                let inner = if signature(q) { &q[1..q.len() - 1] } else { q };
                let sig = self.bracket(inner)?;
                let values = sig.guards.iter().all(|g| matches!(g.test, Test::Equals(_)));
                if sig.outs.is_some() || !values {
                    return Err(self.err(
                        Kind::Mismatch,
                        "`clauses#` takes its inputs alone, their types or values, \
                         `< i64 i64 >`, with no guard or outputs",
                    ));
                }
                (sig.ins, sig.guards)
            }
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`clauses#` takes its inputs' types, a type or code that leaves them, \
                         `< i64 i64 >`, and {} is neither",
                        self.describe(&ins)
                    ),
                ));
            }
        };
        let Val::Name(n, _) = &w.val else {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`clauses#` takes a word's symbol, and {} is not one",
                    self.describe(&w)
                ),
            ));
        };
        let name = self.resolve_quiet("", n);
        let Some(word) = self.words.get(&name) else {
            return Err(self.err(Kind::NoWord, format!("no word `{n}`")));
        };
        let clauses = word.clauses.clone();
        // Dispatch's choice, for values of those types alone.
        let base = self.stack.len();
        let floor = std::mem::replace(&mut self.floor, base);
        for &ty in &types {
            self.stack.push(Jdg { ty, val: Val::Run });
        }
        let cands = self.candidates(&clauses);
        let plain: Vec<Candidate> = cands.iter().filter(|c| !c.1.guarded()).cloned().collect();
        let best = self.best(n, &plain, types.len());
        self.stack.truncate(base);
        self.floor = floor;
        let given = |c: &Clause| {
            let sig = c.sig.as_ref().expect("guarded");
            let shift = types.len() as isize - sig.ins.len() as isize;
            sig.guards.iter().all(|g| {
                let at = g.slot as isize + shift;
                values
                    .iter()
                    .find(|v| v.slot as isize == at)
                    .is_none_or(|v| !matches!(g.test, Test::Equals(_)) || v.test == g.test)
            })
        };
        let mut chosen: Vec<Rc<Clause>> = cands
            .iter()
            .filter(|c| c.1.guarded() && given(&c.1))
            .map(|c| c.1.clone())
            .collect();
        chosen.extend(best?.map(|(c, _)| c));
        let at = self.site.unwrap_or(self.pos);
        let tok = |tok| Token { tok, pos: at };
        let mut out = Vec::new();
        for c in chosen {
            let start = out.len();
            out.push(tok(Tok::Open(b'[')));
            for j in c.written.iter() {
                out.push(tok(Tok::Open(b'[')));
                out.extend(self.written(j)?);
                out.push(tok(Tok::Close(b']')));
            }
            out.push(tok(Tok::Open(b'[')));
            out.extend(c.body.iter().cloned());
            out.extend([tok(Tok::Close(b']')), tok(Tok::Close(b']'))]);
            // The core's own source is not the program's, so its code is
            // where it was taken.
            if c.core {
                for t in &mut out[start..] {
                    t.pos = at;
                }
            }
        }
        self.push(QUOTE, Val::Quote(out.into()));
        Ok(())
    }

    /// A bracket of a clause's signature as written, its code. A type given
    /// alone is written by its name, `i64`.
    fn written(&mut self, j: &Jdg) -> Result<Vec<Token>> {
        Ok(match &j.val {
            Val::Quote(q) => q[1..q.len() - 1].to_vec(),
            Val::Type(t) => {
                let none = crate::symbols::Symbols::parse("").expect("an empty table");
                let toks = read::read(&self.types.name(*t), &none)?;
                self.expand(&toks, "")?
            }
            _ => unreachable!("a signature is a type or a bracket"),
        })
    }

    /// `w effect#`: how many values a word or a quotation takes and leaves,
    /// `max' effect#` being 2 and 1, as the stage knows while compiling.
    fn effect_counts(&mut self) -> Result<()> {
        let w = self.stack.pop().expect("matched");
        if !is_code(&w) {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "`effect#` takes a word's symbol or a quotation, and {} is neither",
                    self.describe(&w)
                ),
            ));
        }
        if let Val::Name(n, _) = &w.val
            && !self.words.contains_key(&self.resolve_quiet("", n))
            && self.core_rule(n).is_none()
        {
            return Err(self.err(Kind::NoWord, format!("no word `{n}`")));
        }
        let Some((ins, outs)) = self.code_effect(&w) else {
            return Err(self.err(
                Kind::Mismatch,
                format!(
                    "{} has no stack effect known while compiling",
                    Self::shown(&w)
                ),
            ));
        };
        for k in [ins, outs] {
            self.push(I64, Val::Int(k as i128));
        }
        Ok(())
    }

    /// The effect of a word or a quotation, if it is known.
    fn code_effect(&self, j: &Jdg) -> Option<(usize, usize)> {
        match &j.val {
            Val::Name(n, _) => self.name_effect("", n, &mut Vec::new()),
            Val::Quote(q) => self.effect(q, &mut Vec::new()),
            _ => None,
        }
    }

    /// `w n quote-power`: w's n-th power, a quotation, as mathematics takes
    /// it for w's kind, which its stack effect gives: a value's n copies
    /// multiplied, `pi^2` being π²; an operation's n operands, `+^4` being
    /// `+ + +`; and any other word applied n times, `sqrt^2` a fourth root.
    fn quote_power(&mut self) -> Result<()> {
        let n = self.count("power#", 0)?;
        let w = self.stack.pop().expect("matched");
        let code = self.code_tokens(&w, "power#")?;
        let shown = Self::shown(&w);
        let Some((k, m)) = self.code_effect(&w) else {
            return Err(self.err(
                Kind::Mismatch,
                format!("{shown} has no power: its stack effect is not known while compiling"),
            ));
        };
        let (times, products) = match (k, m) {
            // x⁰ is the identity of x's operation, a value whose type comes
            // where it is used, as a literal's does: `w' unit# !`. A value's
            // power is its copies multiplied, so its operation is `⋅`.
            (0, 1) | (2, 1) if n == 0 => (0, 0),
            (3.., 1) if n == 0 => {
                return Err(self.err(
                    Kind::NoWord,
                    format!(
                        "the power 0 of {shown} is the identity of its operation, and an \
                         identity is an operation's of two values; {shown} takes {k}"
                    ),
                ));
            }
            (0, 1) => (n, n - 1),
            (2.., 1) if (n - 1).is_multiple_of(k - 1) => ((n - 1) / (k - 1), 0),
            (2.., 1) => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "{shown} takes {k} values, so a power of it takes a multiple of {} \
                         and one more",
                        k - 1
                    ),
                ));
            }
            _ => (n, 0),
        };
        let at = self.pos;
        let mut q: Vec<Token> = Vec::new();
        for _ in 0..times {
            q.extend(applied(&code, at));
        }
        for _ in 0..products {
            q.push(Token {
                tok: Tok::Name(core_path("*")),
                pos: at,
            });
            q.push(Token {
                tok: Tok::Apply,
                pos: at,
            });
        }
        if n == 0 && m == 1 && (k == 0 || k == 2) {
            let op: Vec<Tok> = match &w.val {
                _ if k == 0 => vec![Tok::Name(core_path("*"))],
                Val::Name(name, ..) => vec![Tok::Name(name.clone())],
                _ => std::iter::once(Tok::Open(b'['))
                    .chain(code.iter().map(|t| t.tok.clone()))
                    .chain([Tok::Close(b']')])
                    .collect(),
            };
            let unit = [
                Tok::Name(core_path("unit#")),
                Tok::Name("!".into()),
                Tok::Apply,
            ];
            q.extend(op.into_iter().chain(unit).map(|tok| Token { tok, pos: at }));
        }
        self.push(QUOTE, Val::Quote(q.into()));
        Ok(())
    }

    /// `w n width#`: w applied n wide, as the core's rule spells `dup_2`
    /// (doc/design/SYMBOLIC-FORTH.md 22). Each value w moves is a group of n,
    /// but the code handed to it, on the stack now, runs once, as written, on
    /// a group's values. A word has a width if it moves values only, or is
    /// handed code; any other `w_n` is an index no family has defined.
    fn width(&mut self) -> Result<()> {
        let n = self.count("width#", 1)?;
        let w = self.stack.pop().expect("matched");
        let Val::Name(name, at) = &w.val else {
            return Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `width#` for {}: it takes a word",
                    self.describe(&w)
                ),
            ));
        };
        self.pos = *at;
        let found = match self.lookup(name) {
            Ok(found) => Some(found),
            Err(e) if e.kind == Kind::NoWord => None,
            Err(e) => return Err(e),
        };
        let outside = self
            .stack
            .iter()
            .filter(|j| is_code(j))
            .map(|j| j.val.clone())
            .collect();
        let saved = self.wide.replace(Wide { n, outside });
        let r = match &found {
            Some(found) => self.widens(found),
            None => Ok(false),
        }
        .and_then(|widens| match &found {
            Some(found) if widens && n == 1 => self.unwide(|s| s.apply_word(found)),
            Some(found) if widens => self.wide_word(found),
            _ => Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `{name}_{n}`: a subscript is an index, which a word's family \
                     gives a meaning, and the core gives one only to a word that moves values \
                     or is handed code, a width, as `dup_2`"
                ),
            )),
        });
        self.wide = saved;
        r
    }

    /// Whether a word has a width, while widening: it moves values only, as
    /// the stack words do and words made of them, or it is handed code, as
    /// the combinators are.
    fn widens(&self, name: &Rc<str>) -> Result<bool> {
        let name = self.alias_of(name);
        if self.moves_only(&name, &mut Vec::new()) {
            return Ok(true);
        }
        let Some(w) = self.words.get(&name) else {
            return Ok(false);
        };
        if w.ty.is_some() || w.con.is_some() || !self.stack[self.floor..].iter().any(is_code) {
            return Ok(false);
        }
        match self.effect_of(&name) {
            Some((k, _)) => self.code_among(k),
            None => Ok(true),
        }
    }

    fn unwide(&mut self, f: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        let saved = self.wide.take();
        let r = f(self);
        self.wide = saved;
        r
    }

    /// `.` while widening: code handed to the word runs as written, and
    /// the word's own is widened.
    fn wide_apply(&mut self, f: Jdg) -> Result<()> {
        let outside = self
            .wide
            .as_ref()
            .is_some_and(|w| w.outside.iter().any(|o| same_code(o, &f.val)));
        let code = self.code_tokens(&f, ".")?;
        if outside {
            return self.unwide(|s| s.run_code(&code));
        }
        match &f.val {
            Val::Name(name, at) => {
                self.pos = *at;
                let name = self.lookup(name)?;
                self.wide_word(&name)
            }
            _ => self.run_code(&code),
        }
    }

    /// A word widened. `dup`, `drop`, `swap` and `dip` move groups as they
    /// move values, and so does a word made only of them, or one handed
    /// code: its body is widened in turn.
    fn wide_word(&mut self, name: &Rc<str>) -> Result<()> {
        let name = self.alias_of(name);
        let Some(w) = self.words.get(&name) else {
            return self.apply_word(&name);
        };
        match w.prim.as_ref().map(|(p, _)| *p) {
            Some(Prim::Dup) => return self.wide_dup(),
            Some(Prim::Drop) => return self.wide_drop(),
            Some(Prim::Swap) => return self.wide_swap(),
            Some(Prim::Dip) => return self.wide_dip(),
            _ => {}
        }
        if w.ty.is_some() || w.con.is_some() {
            return self.unwide(|s| s.apply_word(&name));
        }
        let (prim, clauses) = (w.prim.is_some(), w.clauses.clone());
        if self.widens(&name)? {
            // The clause chosen as a family chooses one, its body widened.
            let chosen = if prim {
                None
            } else {
                primary(&self.candidates(&clauses))
            };
            let Some((_, clause, _)) = chosen else {
                return Err(self.err(
                    Kind::NoWord,
                    format!("no width for `{name}`: it is handed code, and has no body to widen"),
                ));
            };
            return self.run_code(&clause.body);
        }
        Err(self.err(
            Kind::NoWord,
            format!("`{name}` has no width: it neither moves values nor is handed code"),
        ))
    }

    /// The sizes of the top `k` values while widening, from the top: code
    /// is one value, and anything else a group of n.
    fn items(&self, k: usize) -> Result<Vec<usize>> {
        let n = self.wide.as_ref().expect("widening").n;
        let mut at = self.stack.len();
        let mut sizes = Vec::with_capacity(k);
        for _ in 0..k {
            let size = if at > self.floor && is_code(&self.stack[at - 1]) {
                1
            } else {
                n
            };
            if at < self.floor + size
                || self.stack[at - size..at]
                    .iter()
                    .any(|j| j.val == Val::Many || (size > 1 && is_code(j)))
            {
                return Err(self.err(
                    Kind::Mismatch,
                    format!("a width of {n} needs groups of {n} values here"),
                ));
            }
            at -= size;
            sizes.push(size);
        }
        Ok(sizes)
    }

    fn code_among(&self, k: usize) -> Result<bool> {
        Ok(self.items(k)?.contains(&1))
    }

    fn wide_dup(&mut self) -> Result<()> {
        let s = self.items(1)?[0];
        for _ in 0..s {
            self.copy(self.stack.len() - s)?;
        }
        Ok(())
    }

    fn wide_drop(&mut self) -> Result<()> {
        let s = self.items(1)?[0];
        for _ in 0..s {
            if !self.stack.pop().expect("in reach").known() {
                self.emit(Op::Prim(P::Drop));
            }
        }
        Ok(())
    }

    fn wide_swap(&mut self) -> Result<()> {
        let sizes = self.items(2)?;
        let (a, b) = (sizes[0], sizes[1]);
        let base = self.stack.len() - a - b;
        if self.stack[base..].iter().all(|j| !j.known()) {
            // Each value of the lower, rolled up past the upper.
            for _ in 0..b {
                self.roll(a + b);
            }
            let lower: Vec<Jdg> = self.stack.drain(base..base + b).collect();
            self.stack.extend(lower);
        } else {
            let order: Vec<usize> = (b..a + b).chain(0..b).collect();
            self.permute(base, &order);
        }
        Ok(())
    }

    /// Emits code that brings the cell `w - 1` below the top up to the top:
    /// `swap` for two, `rot` for three, and for more `[ roll ] dip swap`.
    fn roll(&mut self, w: usize) {
        match w {
            0 | 1 => {}
            2 => self.emit(Op::Prim(P::Swap)),
            3 => self.emit(Op::Prim(P::Rot)),
            _ => {
                self.emit(Op::Prim(P::ScratchPush));
                self.roll(w - 1);
                self.emit(Op::Prim(P::ScratchPop));
                self.emit(Op::Prim(P::Swap));
            }
        }
    }

    fn wide_dip(&mut self) -> Result<()> {
        let f = self.pop()?;
        if !is_code(&f) {
            return Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `dip` for {}: it takes a quotation or a word",
                    self.describe(&f)
                ),
            ));
        }
        let s = self.items(1)?[0];
        self.aside(s, |st| st.wide_apply(f))
    }

    /// Whether a word only moves values: `dup`, `drop`, `swap` and `dip`,
    /// and words of one clause made only of them, of quotations of them and
    /// of `.`, as `over`, `keep` and `bi` are. Widened, such a word moves
    /// groups as it moves values.
    fn moves_only(&self, name: &Rc<str>, seen: &mut Vec<Rc<str>>) -> bool {
        let name = self.alias_of(name);
        let Some(w) = self.words.get(&name) else {
            return false;
        };
        if let Some((p, _)) = &w.prim {
            return matches!(p, Prim::Dup | Prim::Drop | Prim::Swap | Prim::Dip);
        }
        let [c] = &w.clauses[..] else {
            return false;
        };
        if seen.contains(&name) {
            return false;
        }
        let body = c.body.clone();
        seen.push(name);
        let r = self.moves_only_tokens(&body, seen);
        seen.pop();
        r
    }

    fn moves_only_tokens(&self, toks: &[Token], seen: &mut Vec<Rc<str>>) -> bool {
        toks.iter().all(|t| match &t.tok {
            Tok::Name(n) => self.moves_only(n, seen),
            Tok::Apply | Tok::Open(b'[') | Tok::Close(b']') => true,
            _ => false,
        })
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
            let r = self.run_quote(q);
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
            let r = self.run_quote(q);
            self.depth -= 1;
            r?;
        }
        self.emit(Op::Prim(P::ScratchPop));
        self.emit(Op::Prim(P::Drop));
        Ok(())
    }

    /// The value at `i` as type `t`: a literal by the type's clause for it, a
    /// value of another type by the type's clause for that, as a type's own
    /// clauses say how to make one; otherwise it must have the type already,
    /// or forget into it.
    fn convert(&mut self, i: usize, t: Type) -> Result<()> {
        // An operation, by its symbol or a quotation, is made its identity,
        // `+' unit# !`, whose type comes where it is used
        // (doc/design/SYMBOLIC-FORTH.md 21).
        if t == UNIT_LIT && matches!(self.stack[i].val, Val::Name(..) | Val::Quote(_)) {
            self.stack[i].ty = UNIT_LIT;
            return Ok(());
        }
        // Not yet typed, it is its operation, which `quote !` gives as
        // code, a word's symbol being the code that applies it, `[ + ]`, so
        // March can find which identity it is.
        if t == QUOTE && self.stack[i].ty == UNIT_LIT {
            if let Val::Name(n, at) = &self.stack[i].val {
                let tok = |tok| Token { tok, pos: *at };
                let q = vec![tok(Tok::Name(n.clone())), tok(Tok::Apply)];
                self.stack[i].val = Val::Quote(q.into());
            }
            self.stack[i].ty = QUOTE;
            return Ok(());
        }
        // A symbol that writes a number, as a pattern's capture `2` does,
        // is that number given a type (doc/design/SYMBOLIC-FORTH.md 7).
        if let Val::Name(n, at) = &self.stack[i].val
            && !matches!(t, SYMBOL | STRING)
            && let Ok(Some(tok)) = read::number(n, *at)
        {
            let (ty, val) = match tok {
                Tok::Int(n) => (INT_LIT, Val::Int(n)),
                Tok::Dec(d, s) => (DEC_LIT, Val::Dec(d, s)),
                Tok::Ratio(r) => (RATIO_LIT, Val::Ratio(r)),
                _ => unreachable!("a number"),
            };
            self.stack[i] = Jdg { ty, val };
            return self.convert(i, t);
        }
        let have = self.stack[i].ty;
        // One known only at run time is a number only once known, as a
        // capture is where the name its rule rewrites is applied.
        if have == SYMBOL && !matches!(t, SYMBOL | STRING) && !self.stack[i].known() {
            return Err(self.err(
                Kind::Unknown,
                format!(
                    "a symbol becomes {} only where it is known, while compiling",
                    self.types.name(t)
                ),
            ));
        }
        if have != t
            && !self.types.is_literal(have)
            && i + 1 == self.stack.len()
            && let Some(c) = self.conversion(t, have)
        {
            let at = self.pos;
            let name = self.types.name(t);
            return self.apply_clause(&name, &c, Env::default(), at);
        }
        // A type of its own is given by `!` to a value of its
        // representation, which checks nothing (TYPES.md 2.11).
        if let Some((_, r)) = self.types.own_parts(t)
            && !self.types.is_literal(have)
            && (have == r || self.forgets(r, have))
        {
            self.annotate(i, r)?;
            self.stack[i].ty = t;
            return Ok(());
        }
        self.annotate(i, t)
    }

    /// `string>i64` and `string>f64`, under the conversions `"42" i64!` and
    /// `"2.5" f64!`: the number a string writes, or nil, `i64 nil or`, the
    /// value tagged with its type at run time. Parsing is converting (Thomas,
    /// 2026-10-13). A known string is parsed now, and is the value or nil
    /// itself.
    fn parse(&mut self, t: Type, base: usize) -> Result<()> {
        let op = if t == I64 { P::ParseInt } else { P::ParseFloat };
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
            if self.known_at(base, i) {
                return Ok(());
            }
            self.stack[base + 1].val = Val::Int(i as i128);
            return self.emit_ops(2, &[P::VecAt], &[es[i]]);
        }
        let u = self.tuple_union(t);
        let ary = self.types.intern(Term::Ary(u));
        self.annotate(base, ary)?;
        self.at(Prim::AryAt, base, u, None)
    }

    /// `at` on a record and a key known while compiling: the field, of its
    /// own type, with no lookup as the program runs (TYPES.md 2.11). A field
    /// the record lacks is an error here, as a misspelled label is.
    fn record_at(&mut self, base: usize) -> Result<()> {
        let t = self.stack[base].ty;
        let fields = self.types.fields(t).expect("a record");
        let Val::Name(key, _) = &self.stack[base + 1].val else {
            return Err(self.err(
                Kind::Unknown,
                "a record's field is named while compiling, `p x' at`",
            ));
        };
        let Some(i) = fields.iter().position(|(k, _)| k == key) else {
            return Err(self.err(
                Kind::Mismatch,
                format!("{} has no field `{key}`", self.types.name(t)),
            ));
        };
        if self.known_at(base, i) {
            return Ok(());
        }
        self.stack[base + 1] = Jdg {
            ty: I64,
            val: Val::Int(i as i128),
        };
        self.emit_ops(2, &[P::VecAt], &[fields[i].1])
    }

    /// `within`: an array and a quotation, run with the array's last
    /// elements as its stack, as many as it takes, and what it leaves put in
    /// their place: Joy's `infra`, on the end of an array. `( 1 2 3 ) [ ~ ]
    /// within` is `( 1 3 2 )`, `[ 4 ] within` appends, `[ drop ] within`
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
            let why = format!("`{name}`: the array has fewer elements than its quotation takes");
            self.trap("mismatch", &why);
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
            let r = self.run_quote(&q);
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

    /// The length of a string known now, in characters.
    fn known_length(&self, base: usize) -> Option<u64> {
        match &self.stack[base].val {
            Val::Str(s) => Some(s.chars().count() as u64),
            _ => None,
        }
    }

    /// `at` on an array or a string. Elements count from 1, and from -1 at
    /// the end (Thomas, 2026-10-09). A known index is settled now, and
    /// checked against a length known now; one known only at run time is
    /// settled by `element_index`.
    fn at(&mut self, p: Prim, base: usize, out: Type, len: Option<u64>) -> Result<()> {
        let len = len.or(self.known_length(base));
        match self.stack[base + 1].val {
            Val::Int(k) => {
                let i = self.known_element(base, k, len)?;
                match i {
                    Some(i) if self.known_at(base, i) => return Ok(()),
                    Some(i) => {
                        self.stack[base + 1].val = Val::Int(i as i128);
                        if self.fold(p, base, &[out])? {
                            return Ok(());
                        }
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

    /// The element at i, from 0, of a known array at `base`, the index on
    /// it, in their place: whether the array was known.
    fn known_at(&mut self, base: usize, i: usize) -> bool {
        let Val::Seq(es) = &self.stack[base].val else {
            return false;
        };
        let e = es[i].clone();
        self.stack.truncate(base);
        self.stack.push(e);
        true
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
        let (i, j) = (&self.stack[base + 1].val, &self.stack[base + 2].val);
        if let (Some(n), Val::Int(i), Val::Int(j)) = (self.known_length(base), i, j) {
            let (i, j) = (*i, *j);
            let (n, what) = (n as i128, self.types.name(self.stack[base].ty));
            let (Some(a), Some(b)) = (prims::element(i, n), prims::end(j, n)) else {
                return Err(self.err(Kind::Mismatch, format!("{i} to {j} is outside the {what}")));
            };
            if a > b {
                return Err(self.err(Kind::Mismatch, format!("{i} to {j} is outside the {what}")));
            }
            self.stack[base + 1].val = Val::Int(a as i128);
            self.stack[base + 2].val = Val::Int(b as i128);
            if self.fold(p, base, &[out])? {
                return Ok(());
            }
            return self.emit_ops(3, &[P::Slice], &[out]);
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

    /// A loop's quotation that fails whenever it runs never finishes, so it
    /// has no effect to compile the loop by: an error, unless the clause it
    /// is in is being checked, which it passes.
    fn loop_fails(&self, r: Result<()>, name: &str) -> Result<()> {
        match r {
            Err(e) if e.kind == Kind::Fails && self.checking == 0 => Err(self.err(
                Kind::Mismatch,
                format!("the quotation `{name}` applies always fails, so it has no effect"),
            )),
            r => r,
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
        self.unsure += 1;
        let r = self.run_quote(q);
        self.unsure -= 1;
        self.depth -= 1;
        self.loop_fails(r, name)?;
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
            self.trap("mismatch", &format!("`{name}`: the arrays are not as long"));
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
    /// applied as `[ word ]` is.
    fn words_of(&mut self, name: &str) -> Result<Rc<[Token]>> {
        let j = self.stack.pop().expect("matched");
        self.code_tokens(&j, name)
    }

    /// The code a quotation or a word is, as `words_of` takes it.
    fn code_tokens(&mut self, j: &Jdg, name: &str) -> Result<Rc<[Token]>> {
        match &j.val {
            Val::Quote(q) => Ok(q.clone()),
            // A symbol's code applies it, as global text.
            Val::Name(n, at) => Ok(vec![
                Token {
                    tok: Tok::Name(n.clone()),
                    pos: *at,
                },
                Token {
                    tok: Tok::Apply,
                    pos: *at,
                },
            ]
            .into()),
            _ => Err(self.err(
                Kind::NoWord,
                format!(
                    "no word `{name}` for {}: it takes a quotation or a word",
                    self.describe(j)
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
            self.unsure += 1;
            let r = self.enter().and_then(|()| {
                let r = self.run_quote(q);
                self.depth -= 1;
                r
            });
            self.unsure -= 1;
            let after = std::mem::replace(&mut self.stack, saved.1);
            self.code = saved.0;
            self.effects = saved.2;
            self.loop_fails(r, name)?;
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
        // A map literal undecided shares what its map would.
        let tys: Vec<Type> = tys
            .iter()
            .map(|&x| match self.types.is_record_lit(x) {
                true => self.lit_map(x),
                false => x,
            })
            .collect();
        let tys = &tys[..];
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
        Some(t.unwrap_or(if tys.iter().all(|&x| x == INT_LIT) {
            I64
        } else {
            F64
        }))
    }

    /// Closes the array literal, `b` being `(`, or the map literal, `{`.
    fn close_literal(&mut self, open: u8) -> Result<()> {
        let b = closer(open);
        let lit = self.literals.pop().expect("pushed");
        // Its elements are what its code left, from as deep as it reached,
        // so it is in reach of the literal around it as deep.
        let start = lit.from();
        self.reached(start);
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
        // Keys that are symbols make a record, or may (TYPES.md 2.11).
        if b == b'}'
            && (start..self.stack.len())
                .step_by(2)
                .all(|i| matches!(self.stack[i].val, Val::Name(..)))
        {
            return self.close_record(lit, start);
        }
        let step = if b == b')' { 1 } else { 2 };
        let mut parts = Vec::new();
        // An array's elements of different types, at fixed positions, are a
        // tuple (TYPES.md 2.5); each literal among them takes its default. A
        // symbol beside a string is one of them: it is text where a string
        // is taken (doc/design/SYMBOLIC-FORTH.md 28), and nothing takes it
        // here, so `( "can't read it" no-file' )` is a string and a symbol.
        if b == b')' && !runs {
            // A map literal undecided among them is the map it would be,
            // which an array or a tuple holds.
            for i in start..self.stack.len() {
                let t = self.stack[i].ty;
                if self.types.is_record_lit(t) {
                    let m = self.lit_map(t);
                    self.annotate(i, m)?;
                }
            }
            let tys: Vec<Type> = self.stack[start..].iter().map(|j| j.ty).collect();
            let text = tys.contains(&SYMBOL) && tys.iter().any(|&x| x != SYMBOL);
            let shared = self.join(&tys).filter(|&t| {
                !text
                    && tys.iter().all(|&x| {
                        x == t
                            || self.forgets(t, x)
                            || (self.types.is_literal(x) && self.accepts(t, x))
                    })
            });
            if shared.is_none() {
                for i in start..self.stack.len() {
                    let t = self.stack[i].ty;
                    if self.types.is_literal(t) {
                        self.annotate(i, Self::default_of(t))?;
                    }
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
        self.floor = lit.floor;
        self.inputs.pop();
        // Its elements all known, and no code since its mark, it is known.
        let known = self.stack[start..].iter().all(Jdg::known);
        if b == b')' && !runs && known && self.code.len() == lit.mark + 1 {
            self.code.pop();
            let es: Rc<[Jdg]> = self.stack.drain(start..).collect();
            self.push(ty, Val::Seq(es));
            return Ok(());
        }
        let op = if b == b')' { P::Gather } else { P::MapGather };
        for i in start..self.stack.len() {
            self.materialize(i)?;
        }
        for _ in start..self.stack.len() {
            self.stack.pop();
        }
        // The gather starts below the mark by as many cells as the code
        // reached below its start.
        let reach = lit
            .cells
            .checked_sub(cells(&self.stack))
            .expect("nothing below where a literal's code reached changes");
        if reach > 0 {
            self.emit(Op::Lit(reach as u64));
            self.emit(Op::Prim(P::Reach));
        }
        self.emit(Op::Prim(op));
        self.push(ty, Val::Run);
        Ok(())
    }

    /// Closes a map literal whose keys are symbols (TYPES.md 2.11): of
    /// types, a record type, `{ x: < f64 > ; y: < f64 > ; }`; of values not
    /// alike, a record; of values alike, a map literal undecided, a record
    /// where one is wanted and else a map. A key written again replaces the
    /// one before (2.10). Its values are kept in the order of their keys, as
    /// a tuple's are by position.
    fn close_record(&mut self, lit: Literal, start: usize) -> Result<()> {
        // The last value of each key, by the place it was written, in the
        // order of their keys.
        let mut last: Vec<(Rc<str>, usize)> = Vec::new();
        for k in (start..self.stack.len()).step_by(2) {
            let Val::Name(key, _) = &self.stack[k].val else {
                unreachable!("a symbol")
            };
            last.retain(|(n, _)| n != key);
            last.push((key.clone(), k + 1));
        }
        last.sort_by(|a, b| a.0.cmp(&b.0));
        self.floor = lit.floor;
        self.inputs.pop();
        let none = self.code.len() == lit.mark + 1;
        let is_type = |j: &Jdg| matches!(j.val, Val::Type(_)) || (j.ty == NIL && j.known());
        if last.iter().all(|&(_, i)| is_type(&self.stack[i])) {
            if !none {
                return Err(self.err(
                    Kind::Unknown,
                    "a record type's fields are types, known while compiling",
                ));
            }
            self.code.pop();
            let fields = last
                .iter()
                .map(|(k, i)| match self.stack[*i].val {
                    Val::Type(t) => (k.clone(), t),
                    _ => (k.clone(), NIL),
                })
                .collect();
            self.stack.truncate(start);
            let t = self.types.record(fields, false);
            self.push(TYPE, Val::Type(t));
            return Ok(());
        }
        let tys: Vec<Type> = last.iter().map(|&(_, i)| self.stack[i].ty).collect();
        let text = tys.contains(&SYMBOL) && tys.iter().any(|&x| x != SYMBOL);
        let joined = self.join(&tys).filter(|&t| {
            !text
                && tys.iter().all(|&x| {
                    x == t || self.forgets(t, x) || (self.types.is_literal(x) && self.accepts(t, x))
                })
        });
        let alike = joined.is_some();
        // Its values all known, it is known, and an undecided one keeps its
        // literals for what takes it.
        if none && last.iter().all(|&(_, i)| self.stack[i].known()) {
            self.code.pop();
            let mut vals = Vec::with_capacity(last.len());
            for &(_, i) in &last {
                let t = self.stack[i].ty;
                if !alike && self.types.is_literal(t) {
                    self.annotate(i, Self::default_of(t))?;
                }
                vals.push(self.stack[i].clone());
            }
            let fields = last
                .iter()
                .zip(&vals)
                .map(|((k, _), j)| (k.clone(), j.ty))
                .collect();
            self.stack.truncate(start);
            let t = self.types.record(fields, alike);
            self.push(t, Val::Seq(vals.into()));
            return Ok(());
        }
        // Else its values at run time, in the order of their keys: the keys,
        // known, have no cells, and a value replaced is dropped.
        for k in (start + 1..self.stack.len()).step_by(2) {
            let t = self.stack[k].ty;
            if self.types.is_literal(t) {
                self.annotate(k, joined.unwrap_or(Self::default_of(t)))?;
            }
            self.materialize(k)?;
        }
        let vals: Vec<Jdg> = self.stack.drain(start..).skip(1).step_by(2).collect();
        let written = vals.len();
        self.stack.extend(vals);
        let kept: Vec<usize> = last.iter().map(|&(_, i)| (i - start - 1) / 2).collect();
        let order: Vec<usize> = kept
            .iter()
            .copied()
            .chain((0..written).filter(|w| !kept.contains(w)))
            .collect();
        self.permute(start, &order);
        for _ in kept.len()..written {
            self.emit(Op::Prim(P::Drop));
            self.stack.pop();
        }
        let fields = last
            .iter()
            .zip(&self.stack[start..])
            .map(|((k, _), j)| (k.clone(), j.ty))
            .collect();
        self.stack.truncate(start);
        let reach = lit
            .cells
            .checked_sub(cells(&self.stack))
            .expect("nothing below where a literal's code reached changes");
        if reach > 0 {
            self.emit(Op::Lit(reach as u64));
            self.emit(Op::Prim(P::Reach));
        }
        self.emit(Op::Prim(P::Gather));
        let t = self.types.record(fields, alike);
        self.push(t, Val::Run);
        Ok(())
    }

    /// `( … )` in a bracket: the types in it, a tuple type.
    fn close_tuple_type(&mut self) -> Result<()> {
        let lit = self.literals.pop().expect("the reader checks nesting");
        self.reached(lit.from());
        let mut elems = Vec::new();
        for j in self.stack.drain(lit.from()..).collect::<Vec<_>>() {
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

    /// `[ … ] seq# !`, `map# !`, `sig# !` or `str# !`: the quotation below
    /// made a sequence, a map, a signature or text (doc/design/SYMBOLIC-FORTH.md
    /// 6.8, 6.10, 6.15), as High Stack's `( … )`, `{ … }`, `< … >` and `"…"`
    /// are read. The kind decides when and how its code runs (6.6). `at` is
    /// where the kind was written, where High Stack's bracket or string
    /// opened.
    fn made(&mut self, kind: Type, at: Pos) -> Result<()> {
        let j = self.pop()?;
        // A symbol is text as it is, `'ab' str# !`, as `10 int# !` is a
        // number (6.7): `string` is the type it becomes.
        if kind == STR_LIT && j.ty == SYMBOL {
            self.stack.push(j);
            return self.convert(self.stack.len() - 1, STRING);
        }
        let q = match j.val {
            Val::Quote(q) if !signature(&q) => q,
            _ if kind == STR_LIT => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`str#` makes text of a symbol or a quotation, and {} is neither",
                        self.describe(&j)
                    ),
                ));
            }
            _ => {
                return Err(self.err(
                    Kind::Mismatch,
                    format!(
                        "`{}` makes data of a quotation, and {} is not one",
                        self.types.name(kind),
                        self.describe(&j)
                    ),
                ));
            }
        };
        let closed = std::mem::replace(&mut self.pos, at);
        match kind {
            SIG_LIT => self.quote_sig(&q),
            SEQ_LIT if self.in_bracket > 0 => self.quote_tuple(&q, closed),
            SEQ_LIT => self.quote_literal(&q, b'(', closed),
            STR_LIT => {
                let r = self.quote_text(&q);
                self.pos = closed;
                r
            }
            _ => self.quote_literal(&q, b'{', closed),
        }
    }

    /// A sequence or a map of what the quotation's code leaves. It is
    /// applied where it stands, so its inputs are taken there
    /// (doc/design/SYMBOLIC-FORTH.md 14), and its code reaches what `.`'s
    /// would, its elements being all it leaves, from as deep as it reached.
    fn quote_literal(&mut self, q: &[Token], bracket: u8, closed: Pos) -> Result<()> {
        self.take_inputs(q, letters_in(q), false)?;
        let mark = self.code.len();
        self.emit(Op::Prim(P::Mark));
        let start = self.stack.len();
        self.literals.push(Literal {
            bracket,
            start,
            floor: self.floor,
            mark,
            low: start,
            cells: cells(&self.stack),
            depth: self.depth + 1,
        });
        self.enter()?;
        let r = self.run(q);
        self.depth -= 1;
        r?;
        self.pos = closed;
        self.close_literal(bracket)
    }

    /// A sequence in a signature, its code run over types: a tuple type,
    /// `< ( i64 string ) >`, of the types it leaves, from as deep as it
    /// reached.
    fn quote_tuple(&mut self, q: &[Token], closed: Pos) -> Result<()> {
        let start = self.stack.len();
        self.literals.push(Literal {
            bracket: b'(',
            start,
            floor: self.floor,
            mark: self.code.len(),
            low: start,
            cells: 0,
            depth: self.depth,
        });
        self.run(q)?;
        self.pos = closed;
        self.close_tuple_type()
    }

    /// A signature, its code run over types (TYPES.md 2.2): a type, or, of
    /// more than one, a signature, which applied gives the values below
    /// their types, `x y < i64 f64 > !`. Before a definition's body it is
    /// the clause's context, which `;` reads. The stage keeps it as its code
    /// between `<` and `>`.
    fn quote_sig(&mut self, q: &[Token]) -> Result<()> {
        let at = self.pos;
        let sig = self.bracket(q)?;
        self.pos = at;
        if let ([t], None, []) = (&sig.ins[..], &sig.outs, &sig.guards[..]) {
            self.push(TYPE, Val::Type(*t));
            return Ok(());
        }
        let tok = |tok| Token { tok, pos: at };
        let toks: Vec<Token> = std::iter::once(tok(Tok::Open(b'<')))
            .chain(q.iter().cloned())
            .chain(std::iter::once(tok(Tok::Close(b'>'))))
            .collect();
        self.push(QUOTE, Val::Quote(toks.into()));
        Ok(())
    }

    /// `[ … ] str# !`, as `"…"` is read
    /// (doc/design/SYMBOLIC-FORTH.md 15): its code runs where it stands,
    /// taking its inputs, and each value it leaves is written, in order,
    /// into one string, a string as its text, a symbol known as its name and
    /// any other value by `>string`. Its code reaches what `.`'s would, as a
    /// sequence's does (14), and the values it leaves are all it writes, from
    /// as deep as it reached.
    fn quote_text(&mut self, q: &[Token]) -> Result<()> {
        self.take_inputs(q, letters_in(q), false)?;
        let n = self.inputs.len();
        let start = self.stack.len();
        self.literals.push(Literal {
            bracket: b'"',
            start,
            floor: self.floor,
            mark: self.code.len(),
            low: start,
            cells: 0,
            depth: self.depth,
        });
        let r = self.enter().and_then(|()| {
            let r = self.text_code(q, start);
            self.depth -= 1;
            r
        });
        let lit = self.literals.pop().expect("pushed");
        self.inputs.truncate(n - 1);
        r?;
        let start = lit.from();
        self.reached(start);
        self.write_text(start)?;
        self.join_text(start)
    }

    /// Runs a string's code, and writes what it has left as soon as the code
    /// after reaches none of it, before a symbol or a letter: so a hole's
    /// value is written where it is made, and not brought up to be written
    /// once all have run. What `each` and its kin reach is not known for
    /// sure, so before code holding a quotation, nothing is written yet.
    fn text_code(&mut self, q: &[Token], start: usize) -> Result<()> {
        let mut written = start;
        let mut i = 0;
        while i < q.len() {
            let applied = |k: usize| {
                q.get(k + 1)
                    .is_some_and(|t| matches!(t.tok, Tok::Apply | Tok::Test))
            };
            let end = match &q[i].tok {
                Tok::Open(_) => matching(q, i) + 1,
                Tok::Name(_) if letter_at(q, i).is_some() => i + 3,
                _ => i + 1,
            };
            let next = matches!(&q[i].tok, Tok::Name(_) if !applied(i));
            if i > 0 && next && !q[i..].iter().any(|t| matches!(t.tok, Tok::Open(_))) {
                let (mut items, mut ins) = (Vec::new(), 0);
                let known = self
                    .effect_on(q, i..q.len(), "", &mut Vec::new(), &mut items, &mut ins)
                    .is_some();
                written = written.min(self.literals.last().expect("open").low);
                if known && ins == 0 {
                    self.write_text(written)?;
                    written = self.stack.len();
                }
            }
            self.run(&q[i..end])?;
            i = end;
        }
        Ok(())
    }

    /// Writes the values from `start` as text, each in its place: a symbol
    /// known as its name, and any other value not a string by `>string`.
    fn write_text(&mut self, start: usize) -> Result<()> {
        if self.stack[start..].iter().any(|j| j.val == Val::Many) {
            return Err(self.err(
                Kind::Mismatch,
                "a string writes the values its code leaves, and a run of them is gathered \
                 by `( … )` first",
            ));
        }
        for j in &mut self.stack[start..] {
            if let (SYMBOL, Val::Name(s, _)) = (j.ty, &j.val) {
                *j = Jdg {
                    ty: STRING,
                    val: Val::Str(s.clone()),
                };
            }
        }
        // Each from the first not a string in turn brought to the top and
        // written, so they end in their order.
        let Some(first) = self.stack[start..].iter().position(|j| j.ty != STRING) else {
            return Ok(());
        };
        let first = start + first;
        for _ in first..self.stack.len() {
            let j = self.stack.remove(first);
            if !j.known() {
                let above = cells(&self.stack[first..]);
                self.roll(above + 1);
            }
            let ty = j.ty;
            self.stack.push(j);
            if ty != STRING {
                let floor = std::mem::replace(&mut self.floor, self.stack.len() - 1);
                let r = self.apply_word(&core_path(">string"));
                self.floor = floor;
                r?;
            }
        }
        Ok(())
    }

    /// Joins the strings from `start` into one: those known join now, and
    /// the rest in one buffer at run time.
    fn join_text(&mut self, start: usize) -> Result<()> {
        let mut pieces: Vec<Jdg> = Vec::new();
        for j in self.stack.drain(start..) {
            match (pieces.last_mut(), &j.val) {
                (
                    Some(Jdg {
                        val: Val::Str(a), ..
                    }),
                    Val::Str(b),
                ) => *a = format!("{a}{b}").into(),
                _ => pieces.push(j),
            }
        }
        self.stack.extend(pieces);
        let n = self.stack.len() - start;
        if n == 0 {
            self.push(STRING, Val::Str("".into()));
        } else if n > 1 {
            for i in start..self.stack.len() {
                self.materialize(i)?;
            }
            self.emit(Op::Lit(n as u64));
            self.emit(Op::Prim(P::TextJoin));
            self.stack.truncate(start);
            self.push(STRING, Val::Run);
        }
        Ok(())
    }
}
