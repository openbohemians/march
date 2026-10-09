//! The primitives the stage starts with: each a name, a signature and the
//! machine operations it compiles to, or none for those that exist only at
//! compile time. Families such as `+` are clauses over these, written in
//! March (core/core.march).

use crate::code::Primitive as P;
use crate::error::Kind;
use crate::stage::Val;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    // Literals, exact, at compile time.
    IntAdd,
    IntSub,
    IntMul,
    DecAdd,
    DecSub,
    DecMul,
    IntToDec,
    IntToI64,
    IntToF64,
    DecToF64,
    IntToMoney,
    DecToMoney,
    // Numbers.
    I64Add,
    I64Sub,
    I64Mul,
    I64Div,
    I64Mod,
    I64Lt,
    I64Eq,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Lt,
    F64Eq,
    MoneyAdd,
    MoneySub,
    MoneyLt,
    MoneyEq,
    // Text.
    I64Text,
    F64Text,
    MoneyText,
    StrShow,
    Write,
    StrConcat,
    StrLength,
    StrAt,
    StrEq,
    // Containers.
    AryLength,
    AryAt,
    AryConcat,
    MapLength,
    MapAt,
    VecLength,
    VecAt,
    VecConcat,
    /// `ary quote map`: the quotation applied to each element, in a loop.
    Map,
    /// `ary quote each`: the quotation applied to each element in turn, from
    /// the first; `each-right` from the last.
    Each,
    EachRight,
    /// `n range`: the array 0 to n-1.
    Range,
    Reverse,
    ArySlice,
    StrSlice,
    /// `q r compose`: one quotation, q's words then r's.
    Compose,
    // Stack words, which move judgments as the code moves values.
    Dup,
    Drop,
    Swap,
    Over,
    Rot,
    // Compile time.
    Def,
}

pub struct PrimDef {
    pub prim: Prim,
    pub name: &'static str,
    pub sig: &'static str,
    pub ops: &'static [P],
}

const fn d(prim: Prim, name: &'static str, sig: &'static str, ops: &'static [P]) -> PrimDef {
    PrimDef {
        prim,
        name,
        sig,
        ops,
    }
}

pub const PRIMS: &[PrimDef] = &[
    d(Prim::IntAdd, "int#+", "< int# int# -- int# >", &[]),
    d(Prim::IntSub, "int#-", "< int# int# -- int# >", &[]),
    d(Prim::IntMul, "int#*", "< int# int# -- int# >", &[]),
    d(Prim::DecAdd, "dec#+", "< dec# dec# -- dec# >", &[]),
    d(Prim::DecSub, "dec#-", "< dec# dec# -- dec# >", &[]),
    d(Prim::DecMul, "dec#*", "< dec# dec# -- dec# >", &[]),
    d(Prim::IntToDec, "int#>dec#", "< int# -- dec# >", &[]),
    d(Prim::IntToI64, "int#>i64", "< int# -- i64 >", &[]),
    d(Prim::IntToF64, "int#>f64", "< int# -- f64 >", &[]),
    d(Prim::DecToF64, "dec#>f64", "< dec# -- f64 >", &[]),
    d(Prim::IntToMoney, "int#>money", "< int# -- money >", &[]),
    d(Prim::DecToMoney, "dec#>money", "< dec# -- money >", &[]),
    d(Prim::I64Add, "i64+", "< i64 i64 -- i64 >", &[P::IAdd]),
    d(Prim::I64Sub, "i64-", "< i64 i64 -- i64 >", &[P::ISub]),
    d(Prim::I64Mul, "i64*", "< i64 i64 -- i64 >", &[P::IMul]),
    d(Prim::I64Div, "i64/", "< i64 i64 -- i64 >", &[P::IDiv]),
    d(Prim::I64Mod, "i64mod", "< i64 i64 -- i64 >", &[P::IMod]),
    d(Prim::I64Lt, "i64lt?", "< i64 i64 -- i64 >", &[P::ILt]),
    d(Prim::I64Eq, "i64eq?", "< i64 i64 -- i64 >", &[P::Eq]),
    d(Prim::F64Add, "f64+", "< f64 f64 -- f64 >", &[P::FAdd]),
    d(Prim::F64Sub, "f64-", "< f64 f64 -- f64 >", &[P::FSub]),
    d(Prim::F64Mul, "f64*", "< f64 f64 -- f64 >", &[P::FMul]),
    d(Prim::F64Div, "f64/", "< f64 f64 -- f64 >", &[P::FDiv]),
    d(Prim::F64Lt, "f64lt?", "< f64 f64 -- i64 >", &[P::FLt]),
    d(Prim::F64Eq, "f64eq?", "< f64 f64 -- i64 >", &[P::FEq]),
    d(
        Prim::MoneyAdd,
        "money+",
        "< money money -- money >",
        &[P::IAdd],
    ),
    d(
        Prim::MoneySub,
        "money-",
        "< money money -- money >",
        &[P::ISub],
    ),
    d(
        Prim::MoneyLt,
        "money-lt?",
        "< money money -- i64 >",
        &[P::ILt],
    ),
    d(
        Prim::MoneyEq,
        "money-eq?",
        "< money money -- i64 >",
        &[P::Eq],
    ),
    d(
        Prim::I64Text,
        "i64>text",
        "< i64 -- string >",
        &[P::IntText],
    ),
    d(
        Prim::F64Text,
        "f64>text",
        "< f64 -- string >",
        &[P::FloatText],
    ),
    d(
        Prim::MoneyText,
        "money>text",
        "< money -- string >",
        &[P::MoneyText],
    ),
    d(
        Prim::StrShow,
        "string-show",
        "< string -- string >",
        &[P::StringShow],
    ),
    d(Prim::Write, "write", "< string -- >", &[P::Write]),
    d(
        Prim::StrConcat,
        "string-concat",
        "< string string -- string >",
        &[P::Concat],
    ),
    d(
        Prim::StrLength,
        "string-length",
        "< string -- i64 >",
        &[P::VecLen],
    ),
    d(
        Prim::StrAt,
        "string-at",
        "< string i64 -- i64 >",
        &[P::VecAt],
    ),
    d(
        Prim::StrEq,
        "string-eq?",
        "< string string -- i64 >",
        &[P::Eq],
    ),
    d(
        Prim::AryLength,
        "ary-length",
        "< a ary -- i64 >",
        &[P::VecLen],
    ),
    d(Prim::AryAt, "ary-at", "< a ary i64 -- a >", &[P::VecAt]),
    d(
        Prim::AryConcat,
        "ary-concat",
        "< a ary a ary -- a ary >",
        &[P::Concat],
    ),
    d(
        Prim::MapLength,
        "map-length",
        "< k v map -- i64 >",
        &[P::VecLen],
    ),
    d(Prim::MapAt, "map-at", "< k v map k -- v >", &[P::VecAt]),
    d(Prim::VecLength, "vec-length", "< n a vec -- i64 >", &[]),
    d(Prim::VecAt, "vec-at", "< n a vec i64 -- a >", &[P::VecAt]),
    d(
        Prim::VecConcat,
        "vec-concat",
        "< n a vec m a vec >",
        &[P::Concat],
    ),
    d(Prim::Map, "map", "< b c >", &[]),
    d(Prim::Each, "each", "< b c >", &[]),
    d(Prim::EachRight, "each-right", "< b c >", &[]),
    d(Prim::Range, "range", "< i64 >", &[P::Range]),
    d(Prim::Reverse, "reverse", "< b >", &[P::Reverse]),
    d(
        Prim::ArySlice,
        "ary-slice",
        "< a ary i64 i64 -- a ary >",
        &[P::Slice],
    ),
    d(
        Prim::StrSlice,
        "string-slice",
        "< string i64 i64 -- string >",
        &[P::Slice],
    ),
    d(Prim::Compose, "compose", "< b c >", &[]),
    d(Prim::Dup, "dup", "< a -- a a >", &[]),
    d(Prim::Drop, "drop", "< a -- >", &[]),
    d(Prim::Swap, "swap", "< a b -- b a >", &[]),
    d(Prim::Over, "over", "< a b -- a b a >", &[]),
    d(Prim::Rot, "rot", "< a b c -- b c a >", &[]),
    d(Prim::Def, "def", "< quote symbol -- >", &[]),
];

/// Whether the stage handles the primitive itself, rather than by its
/// signature and operations.
pub fn custom(p: Prim) -> bool {
    use Prim::*;
    matches!(
        p,
        VecLength
            | VecAt
            | VecConcat
            | Map
            | Each
            | EachRight
            | Range
            | Reverse
            | Compose
            | Dup
            | Drop
            | Swap
            | Over
            | Rot
            | Def
    )
}

/// Whether the primitive exists only at compile time: its inputs must be
/// known.
pub fn compile_time_only(p: Prim) -> bool {
    use Prim::*;
    matches!(
        p,
        IntAdd
            | IntSub
            | IntMul
            | DecAdd
            | DecSub
            | DecMul
            | IntToDec
            | IntToI64
            | IntToF64
            | DecToF64
            | IntToMoney
            | DecToMoney
    )
}

/// What a primitive does besides leaving its results: for each domain,
/// whether it reads and whether it writes, as a set of bits (march5's effect
/// rows). Only `io` has any so far.
pub type Effects = u32;
pub const IO_READ: Effects = 1;
pub const IO_WRITE: Effects = 2;
/// Every effect that writes.
pub const WRITES: Effects = IO_WRITE;

pub fn effects(p: Prim) -> Effects {
    match p {
        Prim::Write => IO_WRITE,
        _ => 0,
    }
}

/// Effects as a message says them: "writes io".
pub fn describe_effects(e: Effects) -> String {
    let mut parts = Vec::new();
    if e & IO_READ != 0 {
        parts.push("reads io");
    }
    if e & IO_WRITE != 0 {
        parts.push("writes io");
    }
    parts.join(" and ")
}

/// Whether applying the primitive to known values may be done at compile
/// time: never one with an effect, whose reads are facts of the run and
/// whose writes must happen then.
pub fn foldable(p: Prim) -> bool {
    effects(p) == 0
        && !matches!(
            p,
            Prim::AryLength
                | Prim::ArySlice
                | Prim::AryAt
                | Prim::AryConcat
                | Prim::MapLength
                | Prim::MapAt
        )
}

type Folded = Result<Vec<Val>, (Kind, String)>;

fn int(v: &Val) -> i128 {
    match v {
        Val::Int(n) => *n,
        _ => unreachable!("an integer"),
    }
}
fn dec(v: &Val) -> (i128, u32) {
    match v {
        Val::Dec(d, s) => (*d, *s),
        _ => unreachable!("a decimal"),
    }
}
fn float(v: &Val) -> f64 {
    match v {
        Val::Float(x) => *x,
        _ => unreachable!("a float"),
    }
}
fn text(v: &Val) -> &str {
    match v {
        Val::Str(s) => s,
        _ => unreachable!("a string"),
    }
}

fn overflow(what: &str) -> (Kind, String) {
    (Kind::Arithmetic, format!("{what} overflows"))
}

/// An i64 result, checked.
fn i64_of(n: Option<i64>, what: &str) -> Folded {
    n.map(|n| vec![Val::Int(n.into())])
        .ok_or_else(|| overflow(what))
}

fn pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

/// Two decimals with the same number of places.
fn align(a: (i128, u32), b: (i128, u32)) -> Option<(i128, i128, u32)> {
    let s = a.1.max(b.1);
    Some((
        a.0.checked_mul(pow10(s - a.1)?)?,
        b.0.checked_mul(pow10(s - b.1)?)?,
        s,
    ))
}

/// Applies a primitive to known values, at compile time.
pub fn fold(p: Prim, a: &[Val]) -> Folded {
    use Prim::*;
    let lit =
        |n: Option<i128>, what: &str| n.map(|n| vec![Val::Int(n)]).ok_or_else(|| overflow(what));
    let decimal = |r: Option<(i128, u32)>| {
        r.map(|(d, s)| vec![Val::Dec(d, s)])
            .ok_or_else(|| overflow("a decimal literal"))
    };
    let small = |n: i128| i64::try_from(n).expect("an i64 value");
    let flag = |b: bool| Ok(vec![Val::Int(b as i128)]);
    match p {
        IntAdd => lit(int(&a[0]).checked_add(int(&a[1])), "an integer literal"),
        IntSub => lit(int(&a[0]).checked_sub(int(&a[1])), "an integer literal"),
        IntMul => lit(int(&a[0]).checked_mul(int(&a[1])), "an integer literal"),
        DecAdd | DecSub => decimal(align(dec(&a[0]), dec(&a[1])).and_then(|(x, y, s)| {
            let r = if p == DecAdd {
                x.checked_add(y)
            } else {
                x.checked_sub(y)
            };
            Some((r?, s))
        })),
        DecMul => {
            let ((x, s), (y, t)) = (dec(&a[0]), dec(&a[1]));
            decimal(x.checked_mul(y).zip(s.checked_add(t)))
        }
        IntToDec => Ok(vec![Val::Dec(int(&a[0]), 0)]),
        IntToI64 => {
            let n = int(&a[0]);
            match i64::try_from(n) {
                Ok(_) => Ok(vec![Val::Int(n)]),
                Err(_) => Err((Kind::Literal, format!("{n} is too large for an i64"))),
            }
        }
        IntToF64 => Ok(vec![Val::Float(int(&a[0]) as f64)]),
        DecToF64 => {
            let (d, s) = dec(&a[0]);
            let x: f64 = format!("{d}e-{s}")
                .parse()
                .expect("a decimal reads as a float");
            Ok(vec![Val::Float(x)])
        }
        IntToMoney => match int(&a[0])
            .checked_mul(100)
            .filter(|c| i64::try_from(*c).is_ok())
        {
            Some(c) => Ok(vec![Val::Int(c)]),
            None => Err((
                Kind::Literal,
                format!("{} is too large for money", int(&a[0])),
            )),
        },
        DecToMoney => {
            let (mut d, mut s) = dec(&a[0]);
            while s > 0 && d % 10 == 0 {
                d /= 10;
                s -= 1;
            }
            if s > 2 {
                return Err((
                    Kind::Literal,
                    format!(
                        "{} has more than two places, so it is not exact money",
                        show_dec(d, s)
                    ),
                ));
            }
            match d
                .checked_mul(pow10(2 - s).unwrap())
                .filter(|c| i64::try_from(*c).is_ok())
            {
                Some(c) => Ok(vec![Val::Int(c)]),
                None => Err((
                    Kind::Literal,
                    format!("{} is too large for money", show_dec(d, s)),
                )),
            }
        }
        I64Add | MoneyAdd => i64_of(
            small(int(&a[0])).checked_add(small(int(&a[1]))),
            "i64 addition",
        ),
        I64Sub | MoneySub => i64_of(
            small(int(&a[0])).checked_sub(small(int(&a[1]))),
            "i64 subtraction",
        ),
        I64Mul => i64_of(
            small(int(&a[0])).checked_mul(small(int(&a[1]))),
            "i64 multiplication",
        ),
        I64Div | I64Mod => {
            let (x, y) = (small(int(&a[0])), small(int(&a[1])));
            if y == 0 {
                return Err((Kind::Arithmetic, "division by zero".into()));
            }
            let r = if p == I64Div {
                x.checked_div(y)
            } else {
                x.checked_rem(y)
            };
            i64_of(r, "i64 division")
        }
        I64Lt | MoneyLt => flag(int(&a[0]) < int(&a[1])),
        I64Eq | MoneyEq => flag(int(&a[0]) == int(&a[1])),
        F64Add => Ok(vec![Val::Float(float(&a[0]) + float(&a[1]))]),
        F64Sub => Ok(vec![Val::Float(float(&a[0]) - float(&a[1]))]),
        F64Mul => Ok(vec![Val::Float(float(&a[0]) * float(&a[1]))]),
        F64Div => Ok(vec![Val::Float(float(&a[0]) / float(&a[1]))]),
        F64Lt => flag(float(&a[0]) < float(&a[1])),
        F64Eq => flag(float(&a[0]) == float(&a[1])),
        I64Text => Ok(vec![Val::Str(int(&a[0]).to_string().into())]),
        F64Text => Ok(vec![Val::Str(format!("{:?}", float(&a[0])).into())]),
        MoneyText => Ok(vec![Val::Str(show_dec(int(&a[0]), 2).into())]),
        StrShow => {
            let mut out = String::new();
            crate::show::quoted(text(&a[0]), &mut out);
            Ok(vec![Val::Str(out.into())])
        }
        StrConcat => Ok(vec![Val::Str(
            format!("{}{}", text(&a[0]), text(&a[1])).into(),
        )]),
        StrLength => Ok(vec![Val::Int(text(&a[0]).chars().count() as i128)]),
        StrAt => {
            let i = int(&a[1]);
            usize::try_from(i)
                .ok()
                .and_then(|i| text(&a[0]).chars().nth(i))
                .map(|c| vec![Val::Int(u32::from(c).into())])
                .ok_or_else(|| (Kind::Mismatch, format!("index {i} is outside the string")))
        }
        StrEq => flag(text(&a[0]) == text(&a[1])),
        StrSlice => {
            let (i, j) = (int(&a[1]), int(&a[2]));
            let chars: Vec<char> = text(&a[0]).chars().collect();
            match (usize::try_from(i), usize::try_from(j)) {
                (Ok(i), Ok(j)) if i <= j && j <= chars.len() => Ok(vec![Val::Str(
                    chars[i..j].iter().collect::<String>().into(),
                )]),
                _ => Err((Kind::Mismatch, format!("{i} to {j} is outside the string"))),
            }
        }
        _ => unreachable!("{p:?} is not folded"),
    }
}

/// A decimal as it is written.
pub fn show_dec(d: i128, s: u32) -> String {
    if s == 0 {
        return d.to_string();
    }
    let digits = d.unsigned_abs().to_string();
    let s = s as usize;
    let padded = format!("{digits:0>width$}", width = s + 1);
    let (whole, frac) = padded.split_at(padded.len() - s);
    format!("{}{whole}.{frac}", if d < 0 { "-" } else { "" })
}
