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
}

impl Base {
    pub const ALL: [(Base, &'static str); 9] = [
        (Base::I64, "i64"),
        (Base::F64, "f64"),
        (Base::Money, "money"),
        (Base::String, "string"),
        (Base::IntLit, "int#"),
        (Base::DecLit, "dec#"),
        (Base::Type, "type"),
        (Base::Symbol, "symbol"),
        (Base::Quote, "quote"),
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
}

/// The constructors a bracket or `.` can apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Con {
    Ary,
    Vec,
    Map,
}

pub struct Types {
    terms: Vec<Term>,
    index: HashMap<Term, Type>,
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
    pub fn has_vars(&self, t: Type) -> bool {
        match self.term(t) {
            Term::Var(_) | Term::Atom => true,
            Term::Base(_) | Term::Nat(_) => false,
            Term::Ary(e) => self.has_vars(e),
            Term::Vec(n, e) | Term::Map(n, e) => self.has_vars(n) || self.has_vars(e),
        }
    }
    /// The type with each bound variable replaced by its binding.
    pub fn subst(&mut self, t: Type, env: &Env) -> Type {
        match self.term(t) {
            Term::Var(v) => env.0[v as usize].unwrap_or(t),
            Term::Atom => t,
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
    /// The type as a bracket writes it: `3 i64 vec`, `string i64 ary map`.
    pub fn name(&self, t: Type) -> String {
        match self.term(t) {
            Term::Base(b) => Base::ALL.iter().find(|x| x.0 == b).unwrap().1.to_string(),
            Term::Nat(n) => n.to_string(),
            Term::Ary(e) => format!("{} ary", self.name(e)),
            Term::Vec(n, e) => format!("{} {} vec", self.name(n), self.name(e)),
            Term::Map(k, v) => format!("{} {} map", self.name(k), self.name(v)),
            Term::Var(v) => ((b'a' + v) as char).to_string(),
            Term::Atom => "atom".into(),
        }
    }
}

/// Bindings of the type variables `a` to `z`, made while matching.
#[derive(Clone, Debug, Default)]
pub struct Env(pub [Option<Type>; 26]);
