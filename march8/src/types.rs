//! Types as data (doc/design/TYPES.md 2.2): each type is a term, a
//! constructor with its arguments, kept in a table where equal terms are one
//! entry. So a type is a small number, and equal types are equal numbers.

use std::collections::HashMap;
use std::rc::Rc;

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
    /// The type of a name not yet applied, a symbol: `sym`. At run time a
    /// symbol is its name, an interned string.
    Symbol,
    /// The type of a quotation, `[ … ]`.
    Quote,
    /// The type with one value, `nil`, which means nothing is there, as in
    /// `i64 nil or` (doc/design/TYPES.md 3.6).
    Nil,
    /// Truth: 0 and 1, Iverson's brackets, so a subset of i64 (Thomas,
    /// 2026-10-09). Shown as `false` and `true`.
    Bool,
    /// A ratio literal, `1:2`: its terms as written until its type is
    /// decided, exact while it is a literal.
    RatioLit,
    /// An operation's identity, `+' unit# !`, `+^0`: its type is decided
    /// where it is used, as a literal's is (doc/design/SYMBOLIC-FORTH.md 21).
    UnitLit,
    /// The kinds `!` makes of a quotation (doc/design/SYMBOLIC-FORTH.md
    /// 6.8, 6.10, 6.15): a sequence of what its code leaves, `[ … ] seq# !`,
    /// which `( … )` writes; a map of it paired, `map# !`, `{ … }`; a
    /// signature, `sig# !`, `< … >`, its code run over types; and text of
    /// it, `str# !`, `"…"`. No value has one.
    SeqLit,
    MapLit,
    SigLit,
    StrLit,
}

impl Base {
    pub const ALL: [(Base, &'static str); 17] = [
        (Base::I64, "i64"),
        (Base::F64, "f64"),
        (Base::Money, "money"),
        (Base::String, "string"),
        (Base::IntLit, "int#"),
        (Base::DecLit, "dec#"),
        (Base::Type, "type"),
        (Base::Symbol, "sym"),
        (Base::Quote, "quote"),
        (Base::Nil, "nil"),
        (Base::Bool, "bool"),
        (Base::RatioLit, "ratio#"),
        (Base::UnitLit, "unit#"),
        (Base::SeqLit, "seq#"),
        (Base::MapLit, "map#"),
        (Base::SigLit, "sig#"),
        (Base::StrLit, "str#"),
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
pub const BOOL: Type = 10;
pub const RATIO_LIT: Type = 11;
pub const UNIT_LIT: Type = 12;
pub const SEQ_LIT: Type = 13;
pub const MAP_LIT: Type = 14;
pub const SIG_LIT: Type = 15;
pub const STR_LIT: Type = 16;
/// Every trap's type, interned after the base types.
pub const TRAP: Type = 17;

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
    /// Fixed keys with a type each, `{ x: f64 y: string }` (TYPES.md 2.11):
    /// a map of types, as a tuple is a sequence of them, its fields in the
    /// order of their keys. At run time its values in that order, as a
    /// tuple's are.
    Record(u32),
    /// A map literal of symbols whose values are alike, undecided as a
    /// number is (TYPES.md 2.10, 2.11): a record where a record is wanted, a
    /// map where a map is, and with nothing to take it, a map. Held as a
    /// record of its values' own types until then.
    RecordLit(u32),
    /// In a pattern, any record: a class, as `tuple` is.
    AnyRecord,
    /// A type of its own (TYPES.md 2.11): a representation and the name of
    /// the path `;` binds it to, `celsius: f64 own ;`, so no other type is
    /// the same. A subset of its representation, its values the same at run
    /// time. Its name is empty until `;` binds it.
    Own(u32, Type),
    /// A failure being handled, `k trap` (doc/design/SYMBOLIC-FORTH.md 29):
    /// its kind is known only when it fails, so every trap has this type,
    /// and a pattern of one kind, `no-file' trap`, is tested as the program
    /// runs, as a union's member is. At run time, a vector of its message
    /// and its kind.
    Trap,
}

/// The constructors a bracket or `.` can apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Con {
    Ary,
    Vec,
    Map,
    Or,
    Trap,
    Own,
}

pub struct Types {
    terms: Vec<Term>,
    index: HashMap<Term, Type>,
    /// The names `;` gave classes, which messages use: no value has a
    /// class's type, so the name never hides a value's own.
    classes: HashMap<Type, String>,
    /// Tuples' types by position, each row once.
    rows: Vec<Vec<Type>>,
    row_index: HashMap<Vec<Type>, u32>,
    /// Records' fields, each a key and a type, in the order of their keys,
    /// each set once.
    fields: Vec<Fields>,
    field_index: HashMap<Fields, u32>,
    /// The names of types of their own, each once, the first empty.
    names: Vec<Rc<str>>,
    name_index: HashMap<Rc<str>, u32>,
}

/// A record's fields: its keys, in order, each with its type.
pub type Fields = Rc<[(Rc<str>, Type)]>;

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
            fields: Vec::new(),
            field_index: HashMap::new(),
            names: Vec::new(),
            name_index: HashMap::new(),
        };
        for (b, _) in Base::ALL {
            t.intern(Term::Base(b));
        }
        t.intern(Term::Trap);
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
        matches!(t, INT_LIT | DEC_LIT | RATIO_LIT | UNIT_LIT)
    }
    /// Whether a value of this type exists only at compile time.
    pub fn compile_time_only(&self, t: Type) -> bool {
        matches!(t, INT_LIT | DEC_LIT | RATIO_LIT | UNIT_LIT | TYPE | QUOTE)
    }
    /// Whether `t` is a pattern rather than one type: it has a variable or a
    /// class, `atom` or a union.
    pub fn is_pattern(&self, t: Type) -> bool {
        match self.term(t) {
            Term::Var(_) | Term::Atom | Term::Or(..) | Term::AnyTuple | Term::AnyRecord => true,
            Term::Base(_) | Term::Nat(_) | Term::Trap | Term::Own(..) => false,
            Term::Tuple(r) => self.rows[r as usize].iter().any(|&e| self.is_pattern(e)),
            Term::Record(r) | Term::RecordLit(r) => self.fields[r as usize]
                .iter()
                .any(|&(_, e)| self.is_pattern(e)),
            Term::Ary(e) => self.is_pattern(e),
            Term::Vec(n, e) | Term::Map(n, e) => self.is_pattern(n) || self.is_pattern(e),
        }
    }
    /// Whether a value of type `t` may hold one of type `x`: it is `x`, or
    /// an array, a map, a tuple or a union of something that may.
    pub fn holds(&self, t: Type, x: Type) -> bool {
        t == x
            || match self.term(t) {
                Term::Ary(e) | Term::Vec(_, e) => self.holds(e, x),
                Term::Map(k, v) | Term::Or(k, v) => self.holds(k, x) || self.holds(v, x),
                Term::Tuple(r) => self.rows[r as usize].iter().any(|&e| self.holds(e, x)),
                Term::Record(r) | Term::RecordLit(r) => self.fields[r as usize]
                    .iter()
                    .any(|&(_, e)| self.holds(e, x)),
                _ => false,
            }
    }
    /// Whether `t` is a class: it matches types without binding them.
    pub fn is_class(&self, t: Type) -> bool {
        matches!(
            self.term(t),
            Term::Atom | Term::Or(..) | Term::AnyTuple | Term::AnyRecord
        )
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
    /// Whether a value of `t` is counted, holding memory as the program
    /// runs: a string, a symbol, an array, a tuple, a record, a map, a union
    /// or a trap, which a copy holds once more and a drop once less
    /// (doc/design/REPRESENTATIONS.md 4). Numbers, truth and quotations are
    /// cells.
    pub fn counted(&self, t: Type) -> bool {
        match self.term(t) {
            Term::Base(b) => matches!(b, Base::String | Base::Symbol),
            Term::Ary(_)
            | Term::Vec(..)
            | Term::Map(..)
            | Term::Or(..)
            | Term::Tuple(_)
            | Term::Record(_)
            | Term::RecordLit(_)
            | Term::Trap => true,
            Term::Own(_, r) => self.counted(r),
            Term::Nat(_) | Term::Var(_) | Term::Atom | Term::AnyTuple | Term::AnyRecord => false,
        }
    }
    /// Which cells of a container of type `t` are counted values, as the
    /// machine's `holds` takes them: bit i its element i, every bit every
    /// element; for a map, 1 its keys and 2 its values. A union's depends on
    /// its member, so it is set where one is made.
    pub fn held_cells(&self, t: Type) -> u64 {
        let bits = |ts: &mut dyn Iterator<Item = Type>| {
            let ts: Vec<bool> = ts.map(|e| self.counted(e)).collect();
            if ts.iter().all(|&c| c) {
                u64::MAX
            } else {
                ts.iter()
                    .take(64)
                    .enumerate()
                    .filter(|(_, c)| **c)
                    .fold(0, |m, (i, _)| m | 1 << i)
            }
        };
        match self.term(t) {
            Term::Ary(e) | Term::Vec(_, e) => {
                if self.counted(e) {
                    u64::MAX
                } else {
                    0
                }
            }
            Term::Map(k, v) => u64::from(self.counted(k)) | u64::from(self.counted(v)) << 1,
            Term::Tuple(r) => bits(&mut self.rows[r as usize].iter().copied()),
            Term::Record(r) | Term::RecordLit(r) => {
                bits(&mut self.fields[r as usize].iter().map(|&(_, e)| e))
            }
            Term::Trap => u64::MAX,
            Term::Own(_, r) => self.held_cells(r),
            _ => 0,
        }
    }
    pub fn elements(&self, t: Type) -> Option<&[Type]> {
        match self.term(t) {
            Term::Tuple(r) => Some(&self.rows[r as usize]),
            _ => None,
        }
    }
    /// The record of these fields, each key once, or the map literal
    /// undecided, `lit`: put in the order of their keys.
    pub fn record(&mut self, mut fields: Vec<(Rc<str>, Type)>, lit: bool) -> Type {
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        let fields: Fields = fields.into();
        let r = match self.field_index.get(&fields) {
            Some(&r) => r,
            None => {
                let r = self.fields.len() as u32;
                self.fields.push(fields.clone());
                self.field_index.insert(fields, r);
                r
            }
        };
        self.intern(if lit {
            Term::RecordLit(r)
        } else {
            Term::Record(r)
        })
    }
    /// A record's fields, or an undecided map literal's, in the order of
    /// their keys.
    pub fn fields(&self, t: Type) -> Option<Fields> {
        match self.term(t) {
            Term::Record(r) | Term::RecordLit(r) => Some(self.fields[r as usize].clone()),
            _ => None,
        }
    }
    pub fn is_record_lit(&self, t: Type) -> bool {
        matches!(self.term(t), Term::RecordLit(_))
    }
    /// The type of its own over `repr` with this name, empty while no `;`
    /// has bound it.
    pub fn own(&mut self, name: &str, repr: Type) -> Type {
        let n = match self.name_index.get(name) {
            Some(&n) => n,
            None => {
                let n = self.names.len() as u32;
                self.names.push(name.into());
                self.name_index.insert(name.into(), n);
                n
            }
        };
        self.intern(Term::Own(n, repr))
    }
    /// A type of its own's name and its representation.
    pub fn own_parts(&self, t: Type) -> Option<(Rc<str>, Type)> {
        match self.term(t) {
            Term::Own(n, r) => Some((self.names[n as usize].clone(), r)),
            _ => None,
        }
    }
    /// Whether a type is, or holds, a type of its own no `;` has named.
    pub fn unnamed(&self, t: Type) -> bool {
        match self.term(t) {
            Term::Own(n, r) => self.names[n as usize].is_empty() || self.unnamed(r),
            Term::Ary(e) => self.unnamed(e),
            Term::Vec(_, e) => self.unnamed(e),
            Term::Map(k, v) | Term::Or(k, v) => self.unnamed(k) || self.unnamed(v),
            Term::Tuple(r) => self.rows[r as usize].iter().any(|&e| self.unnamed(e)),
            Term::Record(r) | Term::RecordLit(r) => self.fields[r as usize]
                .iter()
                .any(|&(_, e)| self.unnamed(e)),
            _ => false,
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
            Term::Atom | Term::AnyTuple | Term::AnyRecord => t,
            Term::Record(r) | Term::RecordLit(r) => {
                let lit = matches!(self.term(t), Term::RecordLit(_));
                let fs = self.fields[r as usize].clone();
                let fs = fs
                    .iter()
                    .map(|(k, e)| (k.clone(), self.subst(*e, env)))
                    .collect();
                self.record(fs, lit)
            }
            Term::Tuple(r) => {
                let es = self.rows[r as usize].clone();
                let es = es.into_iter().map(|e| self.subst(e, env)).collect();
                self.tuple(es)
            }
            Term::Or(a, b) => {
                let (a, b) = (self.subst(a, env), self.subst(b, env));
                self.union(a, b)
            }
            Term::Base(_) | Term::Nat(_) | Term::Trap | Term::Own(..) => t,
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
    /// The type as it is written, in a message or a bracket, where it reads
    /// back: `3 i64 vec`, `string i64 ary map`.
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
            Term::Record(r) | Term::RecordLit(r) => {
                let fs: Vec<String> = self.fields[r as usize]
                    .iter()
                    .map(|(k, e)| format!("{k}: {}", self.name(*e)))
                    .collect();
                format!("{{ {} }}", fs.join(" "))
            }
            Term::AnyRecord => "record".into(),
            // Its name is its path, shown as the root finds it, the core's
            // without `core.`.
            Term::Own(n, r) => match &*self.names[n as usize] {
                "" => format!("{} own", self.name(r)),
                n => n.strip_prefix("core.").unwrap_or(n).to_string(),
            },
            Term::Trap => "k trap".into(),
        }
    }
}

/// Bindings of the type variables `a` to `z`, made while matching.
#[derive(Clone, Debug, Default)]
pub struct Env(pub [Option<Type>; 26]);
