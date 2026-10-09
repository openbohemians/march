//! Types as data (doc/design/TYPES.md 2.2): each type is a term, a
//! constructor with its arguments, kept in a table where equal terms are one
//! entry. So a type is a small number, and equal types are equal numbers.

use std::collections::HashMap;

pub type Type = u32;

/// The types the machine knows from the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Base {
    I64,
    F64,
    /// Exact cents, in an i64 (TYPES.md 2.11). A placeholder until March
    /// has precise numbers and newtypes.
    Money,
    String,
    /// An integer literal whose type is not decided yet (TYPES.md 2.10).
    IntLit,
    /// A decimal literal: exact digits until its type is decided.
    DecLit,
    /// The type of a type: `i64` unapplied is a value of this type.
    Type,
    /// The type of a name not yet applied.
    Symbol,
    /// The type of a quotation, `[ … ]`.
    Quote,
    /// The type with one value, `nil`, which means nothing is there, as in
    /// `i64 nil or` (doc/design/TYPES.md 3.6).
    Nil,
}

impl Base {
    pub const ALL: [(Base, &'static str); 10] = [
        (Base::I64, "i64"),
        (Base::F64, "f64"),
        (Base::Money, "money"),
        (Base::String, "string"),
        (Base::IntLit, "int#"),
        (Base::DecLit, "dec#"),
        (Base::Type, "type"),
        (Base::Symbol, "symbol"),
        (Base::Quote, "quote"),
        (Base::Nil, "nil"),
    ];
}

/// The numbers of the base types, interned first in this order.
pub const I64: Type = 0;
pub const F64: Type = 1;
pub const MONEY: Type = 2;
pub const STRING: Type = 3;
pub const INT_LIT: Type = 4;
pub const DEC_LIT: Type = 5;
pub const TYPE: Type = 6;
pub const SYMBOL: Type = 7;
pub const QUOTE: Type = 8;
pub const NIL: Type = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Term {
    Base(Base),
    /// A length, in a vec's type.
    Nat(u64),
    /// An array of any length: `i64 ary`.
    Ary(Type),
    /// An array of a length known at compile time: `3 i64 vec`, a length and
    /// an element type.
    Vec(Type, Type),
    /// `string i64 map`: keys and values.
    Map(Type, Type),
    /// A type variable in a pattern, `a` to `z`, numbered 0 to 25.
    Var(u8),
    /// In a pattern, any type that is not a container: a number, a string,
    /// a literal (Thomas, 2026-10-09). A class of types, not a type: no value
    /// has it, and it binds nothing.
    Atom,
    /// In a pattern, a union, `i64 f64 or`: any of its members
    /// (doc/design/TYPES.md 3.6). Kept canonical, its members sorted, without
    /// repeats and nested to the right, so equal unions are one type.
    Or(Type, Type),
    /// Fixed positions with a type each, `( i64 string )` (TYPES.md 2.5): a
    /// row of the table's rows. Positions of one type are a vec instead.
    Tuple(u32),
    /// In a pattern, any tuple: a class, as `atom` is.
    AnyTuple,
}

/// The constructors a bracket or `.` can apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Con {
    Ary,
    Vec,
    Map,
    Or,
}

pub struct Types {
    terms: Vec<Term>,
    index: HashMap<Term, Type>,
    /// The names `def` gave classes, which messages use: no value has a
    /// class's type, so the name never hides a value's own.
    classes: HashMap<Type, String>,
    /// Tuples' types by position, each row once.
    rows: Vec<Vec<Type>>,
    row_index: HashMap<Vec<Type>, u32>,
}

impl Default for Types {
    fn default() -> Self {
        Self::new()
    }
}

impl Types {
    pub fn new() -> Self {
        let mut t = Self {
            terms: Vec::new(),
            index: HashMap::new(),
            classes: HashMap::new(),
            rows: Vec::new(),
            row_index: HashMap::new(),
        };
        for (b, _) in Base::ALL {
            t.intern(Term::Base(b));
        }
        t
    }
    pub fn intern(&mut self, term: Term) -> Type {
        if let Some(&t) = self.index.get(&term) {
            return t;
        }
        let t = self.terms.len() as Type;
        self.terms.push(term);
        self.index.insert(term, t);
        t
    }
    pub fn term(&self, t: Type) -> Term {
        self.terms[t as usize]
    }
    pub fn is_literal(&self, t: Type) -> bool {
        t == INT_LIT || t == DEC_LIT
    }
    /// Whether a value of this type exists only at compile time.
    pub fn compile_time_only(&self, t: Type) -> bool {
        matches!(t, INT_LIT | DEC_LIT | TYPE | SYMBOL | QUOTE)
    }
    /// Whether `t` is a pattern rather than one type: it has a variable or a
    /// class, `atom` or a union.
    pub fn is_pattern(&self, t: Type) -> bool {
        match self.term(t) {
            Term::Var(_) | Term::Atom | Term::Or(..) | Term::AnyTuple => true,
            Term::Base(_) | Term::Nat(_) => false,
            Term::Tuple(r) => self.rows[r as usize].iter().any(|&e| self.is_pattern(e)),
            Term::Ary(e) => self.is_pattern(e),
            Term::Vec(n, e) | Term::Map(n, e) => self.is_pattern(n) || self.is_pattern(e),
        }
    }
    /// Whether `t` is a class: it matches types without binding them.
    pub fn is_class(&self, t: Type) -> bool {
        matches!(self.term(t), Term::Atom | Term::Or(..) | Term::AnyTuple)
    }
    /// The type of fixed positions of these types: a vec when they are all
    /// one type, a tuple otherwise.
    pub fn tuple(&mut self, elems: Vec<Type>) -> Type {
        if let Some(&e) = elems.first()
            && elems.iter().all(|&t| t == e)
        {
            let n = self.intern(Term::Nat(elems.len() as u64));
            return self.intern(Term::Vec(n, e));
        }
        let r = match self.row_index.get(&elems) {
            Some(&r) => r,
            None => {
                let r = self.rows.len() as u32;
                self.rows.push(elems.clone());
                self.row_index.insert(elems, r);
                r
            }
        };
        self.intern(Term::Tuple(r))
    }
    /// A tuple's types, by position.
    pub fn elements(&self, t: Type) -> Option<&[Type]> {
        match self.term(t) {
            Term::Tuple(r) => Some(&self.rows[r as usize]),
            _ => None,
        }
    }
    /// The union of two types: their members together, sorted, without
    /// repeats. A union of one member is that type.
    pub fn union(&mut self, a: Type, b: Type) -> Type {
        let mut ms = self.members(a);
        ms.extend(self.members(b));
        ms.sort_unstable();
        ms.dedup();
        let mut t = ms.pop().expect("a member");
        while let Some(m) = ms.pop() {
            t = self.intern(Term::Or(m, t));
        }
        t
    }
    /// A union's members, or the type itself.
    pub fn members(&self, mut t: Type) -> Vec<Type> {
        let mut ms = Vec::new();
        while let Term::Or(a, b) = self.term(t) {
            ms.push(a);
            t = b;
        }
        ms.push(t);
        ms
    }
    /// The type with each bound variable replaced by its binding.
    pub fn subst(&mut self, t: Type, env: &Env) -> Type {
        match self.term(t) {
            Term::Var(v) => env.0[v as usize].unwrap_or(t),
            Term::Atom | Term::AnyTuple => t,
            Term::Tuple(r) => {
                let es = self.rows[r as usize].clone();
                let es = es.into_iter().map(|e| self.subst(e, env)).collect();
                self.tuple(es)
            }
            Term::Or(a, b) => {
                let (a, b) = (self.subst(a, env), self.subst(b, env));
                self.union(a, b)
            }
            Term::Base(_) | Term::Nat(_) => t,
            Term::Ary(e) => {
                let e = self.subst(e, env);
                self.intern(Term::Ary(e))
            }
            Term::Vec(n, e) => {
                let (n, e) = (self.subst(n, env), self.subst(e, env));
                self.intern(Term::Vec(n, e))
            }
            Term::Map(k, v) => {
                let (k, v) = (self.subst(k, env), self.subst(v, env));
                self.intern(Term::Map(k, v))
            }
        }
    }
    /// Names a class, for messages.
    pub fn name_class(&mut self, t: Type, name: &str) {
        if self.is_class(t) {
            self.classes.entry(t).or_insert_with(|| name.to_string());
        }
    }
    /// The type as a bracket writes it: `3 i64 vec`, `string i64 ary map`.
    pub fn name(&self, t: Type) -> String {
        if let Some(n) = self.classes.get(&t) {
            return n.clone();
        }
        match self.term(t) {
            Term::Base(b) => Base::ALL.iter().find(|x| x.0 == b).unwrap().1.to_string(),
            Term::Nat(n) => n.to_string(),
            Term::Ary(e) => format!("{} ary", self.name(e)),
            Term::Vec(n, e) => format!("{} {} vec", self.name(n), self.name(e)),
            Term::Map(k, v) => format!("{} {} map", self.name(k), self.name(v)),
            Term::Var(v) => ((b'a' + v) as char).to_string(),
            Term::Atom => "atom".into(),
            Term::Or(a, b) => format!("{} {} or", self.name(a), self.name(b)),
            Term::Tuple(r) => {
                let es: Vec<String> = self.rows[r as usize]
                    .iter()
                    .map(|&e| self.name(e))
                    .collect();
                format!("( {} )", es.join(" "))
            }
            Term::AnyTuple => "tuple".into(),
        }
    }
}

/// Bindings of the type variables `a` to `z`, made while matching.
#[derive(Clone, Debug, Default)]
pub struct Env(pub [Option<Type>; 26]);
