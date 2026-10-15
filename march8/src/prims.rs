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
    RatioAdd,
    RatioSub,
    RatioMul,
    RatioDiv,
    IntToRatio,
    DecToRatio,
    RatioToF64,
    RatioToMoney,
    RatioFloor,
    RatioCeil,
    RatioRound,
    RatioTrunc,
    IntToI64,
    IntToF64,
    DecToF64,
    IntToMoney,
    IntToBool,
    DecToMoney,
    // Numbers.
    I64Add,
    I64Sub,
    I64Mul,
    I64Quot,
    I64Rem,
    I64DivMod,
    IntDivMod,
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
    I64ToF64,
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
    MapHas,
    MapPut,
    Empty,
    TupleLength,
    GradeInts,
    GradeFloats,
    GradeMoney,
    GradeTexts,
    GradeDownInts,
    GradeDownFloats,
    GradeDownMoney,
    GradeDownTexts,
    AryKeep,
    StrSplit,
    StrLines,
    StrWords,
    StrLower,
    StrUpper,
    StrToI64,
    StrToF64,
    Convert,
    FileRead,
    StdinRead,
    Embed,
    SymEq,
    StrToSym,
    SymToStr,
    SymShow,
    StrSnake,
    StrKebab,
    StrCamel,
    StrPascal,
    TupleAt,
    TupleSpread,
    MapKeys,
    MapValues,
    SortInts,
    SortFloats,
    SortMoney,
    SortTexts,
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
    /// `xs q within`: q run on the array's last elements as its stack.
    Within,
    /// A vec's elements, each a value on the stack.
    VecSpread,
    /// `xs v k ary-insert`: v at the gap after element k.
    AryInsert,
    /// `xs k ary-remove`: without element k.
    AryRemove,
    /// `xs ys q zip`: q on each pair of elements, in step.
    Zip,
    /// `xs ys q table`: q on every pair, as rows.
    Table,
    FSqrt,
    FPow,
    FFloor,
    FCeil,
    FRound,
    FTrunc,
    DecFloor,
    DecCeil,
    DecRound,
    DecTrunc,
    IPow,
    IntPow,
    // Stack words, which move judgments as the code moves values.
    Dup,
    Drop,
    Swap,
    /// `x q dip`: q applied with x set aside, and x put back on top.
    Dip,
    /// `a b c d -- c d a b`: FORTH's 2SWAP.
    Swap2,
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
    d(Prim::IntToBool, "int#>bool", "< int# -- bool >", &[]),
    d(Prim::DecToMoney, "dec#>money", "< dec# -- money >", &[]),
    d(
        Prim::RatioAdd,
        "ratio#+",
        "< ratio# ratio# -- ratio# >",
        &[],
    ),
    d(
        Prim::RatioSub,
        "ratio#-",
        "< ratio# ratio# -- ratio# >",
        &[],
    ),
    d(
        Prim::RatioMul,
        "ratio#*",
        "< ratio# ratio# -- ratio# >",
        &[],
    ),
    d(
        Prim::RatioDiv,
        "ratio#/",
        "< ratio# ratio# -- ratio# >",
        &[],
    ),
    d(Prim::IntToRatio, "int#>ratio#", "< int# -- ratio# >", &[]),
    d(Prim::DecToRatio, "dec#>ratio#", "< dec# -- ratio# >", &[]),
    d(Prim::RatioToF64, "ratio#>f64", "< ratio# -- f64 >", &[]),
    d(
        Prim::RatioToMoney,
        "ratio#>money",
        "< ratio# -- money >",
        &[],
    ),
    d(Prim::RatioFloor, "ratio#-floor", "< ratio# -- int# >", &[]),
    d(Prim::RatioCeil, "ratio#-ceil", "< ratio# -- int# >", &[]),
    d(Prim::RatioRound, "ratio#-round", "< ratio# -- int# >", &[]),
    d(Prim::RatioTrunc, "ratio#-trunc", "< ratio# -- int# >", &[]),
    d(Prim::I64Add, "i64+", "< i64 i64 -- i64 >", &[P::IAdd]),
    d(Prim::I64Sub, "i64-", "< i64 i64 -- i64 >", &[P::ISub]),
    d(Prim::I64Mul, "i64*", "< i64 i64 -- i64 >", &[P::IMul]),
    d(Prim::I64Quot, "i64-quot", "< i64 i64 -- i64 >", &[P::IDiv]),
    d(Prim::I64Rem, "i64-rem", "< i64 i64 -- i64 >", &[P::IMod]),
    d(
        Prim::I64DivMod,
        "i64-divmod",
        "< i64 i64 -- i64 i64 >",
        &[P::IDivMod],
    ),
    d(
        Prim::IntDivMod,
        "int#-divmod",
        "< int# int# -- int# int# >",
        &[],
    ),
    d(Prim::I64Lt, "i64lt?", "< i64 i64 -- bool >", &[P::ILt]),
    d(Prim::I64Eq, "i64eq?", "< i64 i64 -- bool >", &[P::Eq]),
    d(Prim::F64Add, "f64+", "< f64 f64 -- f64 >", &[P::FAdd]),
    d(Prim::F64Sub, "f64-", "< f64 f64 -- f64 >", &[P::FSub]),
    d(Prim::F64Mul, "f64*", "< f64 f64 -- f64 >", &[P::FMul]),
    d(Prim::F64Div, "f64/", "< f64 f64 -- f64 >", &[P::FDiv]),
    d(Prim::F64Lt, "f64lt?", "< f64 f64 -- bool >", &[P::FLt]),
    d(Prim::F64Eq, "f64eq?", "< f64 f64 -- bool >", &[P::FEq]),
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
        "< money money -- bool >",
        &[P::ILt],
    ),
    d(
        Prim::MoneyEq,
        "money-eq?",
        "< money money -- bool >",
        &[P::Eq],
    ),
    d(
        Prim::I64Text,
        "i64>text",
        "< i64 -- string >",
        &[P::IntText],
    ),
    d(Prim::I64ToF64, "i64>f64", "< i64 -- f64 >", &[P::IToF]),
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
        "< string string -- bool >",
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
    d(Prim::MapHas, "map-has?", "< k v map k -- bool >", &[P::Has]),
    d(
        Prim::MapPut,
        "map-put",
        "< k v map k v -- k v map >",
        &[P::Put],
    ),
    d(Prim::Empty, "empty", "< type >", &[]),
    d(
        Prim::MapKeys,
        "map-keys",
        "< k v map -- k ary >",
        &[P::Keys],
    ),
    d(
        Prim::MapValues,
        "map-values",
        "< k v map -- v ary >",
        &[P::Values],
    ),
    d(
        Prim::SortInts,
        "i64-sort",
        "< i64 ary -- i64 ary >",
        &[P::SortInts],
    ),
    d(
        Prim::SortFloats,
        "f64-sort",
        "< f64 ary -- f64 ary >",
        &[P::SortFloats],
    ),
    d(
        Prim::SortMoney,
        "money-sort",
        "< money ary -- money ary >",
        &[P::SortInts],
    ),
    d(
        Prim::SortTexts,
        "string-sort",
        "< string ary -- string ary >",
        &[P::SortTexts],
    ),
    d(
        Prim::GradeInts,
        "i64-order",
        "< i64 ary -- i64 ary >",
        &[P::GradeInts],
    ),
    d(
        Prim::GradeFloats,
        "f64-order",
        "< f64 ary -- i64 ary >",
        &[P::GradeFloats],
    ),
    d(
        Prim::GradeMoney,
        "money-order",
        "< money ary -- i64 ary >",
        &[P::GradeInts],
    ),
    d(
        Prim::GradeTexts,
        "string-order",
        "< string ary -- i64 ary >",
        &[P::GradeTexts],
    ),
    d(
        Prim::GradeDownInts,
        "i64-dorder",
        "< i64 ary -- i64 ary >",
        &[P::GradeDownInts],
    ),
    d(
        Prim::GradeDownFloats,
        "f64-dorder",
        "< f64 ary -- i64 ary >",
        &[P::GradeDownFloats],
    ),
    d(
        Prim::GradeDownMoney,
        "money-dorder",
        "< money ary -- i64 ary >",
        &[P::GradeDownInts],
    ),
    d(
        Prim::GradeDownTexts,
        "string-dorder",
        "< string ary -- i64 ary >",
        &[P::GradeDownTexts],
    ),
    d(
        Prim::AryKeep,
        "ary-keep",
        "< a ary bool ary -- a ary >",
        &[P::Keep],
    ),
    d(
        Prim::StrSplit,
        "string-split",
        "< string string -- string ary >",
        &[P::Split],
    ),
    d(
        Prim::StrLines,
        "string-lines",
        "< string -- string ary >",
        &[P::Lines],
    ),
    d(
        Prim::StrWords,
        "string-words",
        "< string -- string ary >",
        &[P::Words],
    ),
    d(
        Prim::StrLower,
        "string-lower",
        "< string -- string >",
        &[P::Lower],
    ),
    d(
        Prim::StrUpper,
        "string-upper",
        "< string -- string >",
        &[P::Upper],
    ),
    // Under the conversions `"42" i64!` and `"2.5" f64!` (core.march).
    d(
        Prim::StrToI64,
        "string>i64",
        "< string -- i64 nil or >",
        &[],
    ),
    d(
        Prim::StrToF64,
        "string>f64",
        "< string -- f64 nil or >",
        &[],
    ),
    d(
        Prim::FileRead,
        "file-read",
        "< string -- string >",
        &[P::ReadFile],
    ),
    d(
        Prim::StdinRead,
        "stdin-read",
        "< -- string >",
        &[P::ReadStdin],
    ),
    // A symbol at run time is its name, an interned string, so symbols
    // compare by the machine's `eq` and convert to and from strings as they
    // are.
    d(Prim::SymEq, "sym-eq?", "< sym sym -- bool >", &[P::Eq]),
    d(Prim::StrToSym, "string>sym", "< string -- sym >", &[]),
    d(Prim::SymToStr, "sym>string", "< sym -- string >", &[]),
    d(
        Prim::SymShow,
        "sym-show",
        "< sym -- string >",
        &[P::SymShow],
    ),
    d(
        Prim::StrSnake,
        "string-snake",
        "< string -- string >",
        &[P::Snake],
    ),
    d(
        Prim::StrKebab,
        "string-kebab",
        "< string -- string >",
        &[P::Kebab],
    ),
    d(
        Prim::StrCamel,
        "string-camel",
        "< string -- string >",
        &[P::Camel],
    ),
    d(
        Prim::StrPascal,
        "string-pascal",
        "< string -- string >",
        &[P::Pascal],
    ),
    d(Prim::Embed, "file-embed", "< string -- string >", &[]),
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
    d(Prim::Within, "within", "< b c >", &[]),
    d(Prim::VecSpread, "vec-spread", "< n a vec >", &[]),
    d(Prim::TupleLength, "tuple-length", "< tuple -- i64 >", &[]),
    d(Prim::TupleAt, "tuple-at", "< tuple i64 >", &[]),
    d(Prim::TupleSpread, "tuple-spread", "< tuple >", &[]),
    d(Prim::AryInsert, "ary-insert", "< b c i64 >", &[]),
    d(Prim::AryRemove, "ary-remove", "< b i64 >", &[]),
    d(Prim::Zip, "zip", "< b c d >", &[]),
    d(Prim::Table, "table", "< b c d >", &[]),
    d(Prim::Dup, "dup", "< a -- a a >", &[]),
    d(Prim::Drop, "drop", "< a -- >", &[]),
    d(Prim::Swap, "swap", "< a b -- b a >", &[]),
    d(Prim::Dip, "dip", "< b c >", &[]),
    d(Prim::Swap2, "swap2", "< a b c d -- c d a b >", &[]),
    d(Prim::FSqrt, "f64-sqrt", "< f64 -- f64 >", &[P::FSqrt]),
    d(
        Prim::FFloor,
        "f64-floor",
        "< f64 -- i64 >",
        &[P::FFloor, P::FToI],
    ),
    d(
        Prim::FCeil,
        "f64-ceil",
        "< f64 -- i64 >",
        &[P::FCeil, P::FToI],
    ),
    d(
        Prim::FRound,
        "f64-round",
        "< f64 -- i64 >",
        &[P::FRound, P::FToI],
    ),
    d(Prim::FTrunc, "f64-trunc", "< f64 -- i64 >", &[P::FToI]),
    d(Prim::DecFloor, "dec#-floor", "< dec# -- int# >", &[]),
    d(Prim::DecCeil, "dec#-ceil", "< dec# -- int# >", &[]),
    d(Prim::DecRound, "dec#-round", "< dec# -- int# >", &[]),
    d(Prim::DecTrunc, "dec#-trunc", "< dec# -- int# >", &[]),
    d(Prim::FPow, "f64-pow", "< f64 f64 -- f64 >", &[P::FPow]),
    d(Prim::IPow, "i64-pow", "< i64 i64 -- i64 >", &[P::IPow]),
    d(Prim::IntPow, "int#-pow", "< int# int# -- int# >", &[]),
    d(Prim::Def, "def", "< sym a -- >", &[]),
    // `!`: the value below given the type on top (stage.rs, `convert_by`).
    d(Prim::Convert, "convert", "< a b -- a >", &[]),
];

/// Whether the stage handles the primitive itself, rather than by its
/// signature and operations.
pub fn custom(p: Prim) -> bool {
    use Prim::*;
    matches!(
        p,
        VecLength
            | VecAt
            | AryAt
            | StrAt
            | ArySlice
            | StrSlice
            | VecConcat
            | Map
            | Each
            | EachRight
            | Range
            | Reverse
            | Compose
            | Within
            | VecSpread
            | AryInsert
            | AryRemove
            | Zip
            | Table
            | Dup
            | Drop
            | Swap
            | Dip
            | Swap2
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
            | IntPow
            | IntDivMod
            | DecFloor
            | DecCeil
            | DecRound
            | DecTrunc
            | DecAdd
            | DecSub
            | DecMul
            | IntToDec
            | IntToI64
            | IntToF64
            | DecToF64
            | IntToMoney
            | IntToBool
            | DecToMoney
            | RatioAdd
            | RatioSub
            | RatioMul
            | RatioDiv
            | IntToRatio
            | DecToRatio
            | RatioToF64
            | RatioToMoney
            | RatioFloor
            | RatioCeil
            | RatioRound
            | RatioTrunc
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
        Prim::FileRead | Prim::StdinRead => IO_READ,
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
fn terms(v: &Val) -> &[i128] {
    match v {
        Val::Ratio(t) => t,
        _ => unreachable!("a ratio"),
    }
}

/// A ratio's number, in lowest terms with a positive second: a ratio of
/// two terms, the second not zero.
fn fraction(v: &Val) -> Result<(i128, i128), (Kind, String)> {
    let t = terms(v);
    let &[n, d] = t else {
        return Err((
            Kind::Literal,
            format!(
                "{} is not a number: a number is a ratio of two terms",
                show_ratio(t)
            ),
        ));
    };
    if d == 0 {
        return Err((
            Kind::Arithmetic,
            format!("{} divides by zero", show_ratio(t)),
        ));
    }
    Ok(lowest(n, d))
}

/// n:d in lowest terms, the second positive.
fn lowest(n: i128, d: i128) -> (i128, i128) {
    let (mut a, mut b) = (n.unsigned_abs(), d.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let g = (a as i128) * d.signum();
    (n / g, d / g)
}

/// n/d, d positive, as the nearest f64, ties to even: rounded once, from
/// the exact quotient. None for terms too large to divide so.
fn nearest(n: i128, d: i128) -> Option<f64> {
    if n == 0 {
        return Some(0.0);
    }
    let bits = |x: u128| 128 - x.leading_zeros() as i32;
    let (a, b) = (n.unsigned_abs(), d.unsigned_abs());
    // Scaled by 2^k, the quotient has 55 or 56 bits: the 53 an f64 keeps,
    // and those it is rounded by.
    let k = 55 + bits(b) - bits(a);
    let (a, b) = if k >= 0 {
        (bits(a) + k <= 128).then(|| (a << k, b))?
    } else {
        (bits(b) - k <= 128).then(|| (a, b << -k))?
    };
    let (q, r) = (a / b, a % b);
    let s = bits(q) - 53;
    let mut m = q >> s;
    let (rest, half) = (q & ((1 << s) - 1), 1 << (s - 1));
    if rest > half || (rest == half && (r != 0 || m & 1 == 1)) {
        m += 1;
    }
    // m is at most 2^53, and the power of two is a normal f64: exact.
    let x = m as f64 * f64::from_bits(((s - k + 1023) as u64) << 52);
    Some(if n < 0 { -x } else { x })
}

fn overflow(what: &str) -> (Kind, String) {
    (Kind::Arithmetic, format!("{what} overflows"))
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
        IntPow => {
            let e = u32::try_from(int(&a[1])).map_err(|_| {
                (
                    Kind::Arithmetic,
                    "a negative power of an integer".to_string(),
                )
            })?;
            lit(int(&a[0]).checked_pow(e), "an integer literal")
        }
        DecFloor | DecCeil | DecRound | DecTrunc => {
            let (d, s) = dec(&a[0]);
            let unit = pow10(s).ok_or_else(|| overflow("a decimal literal"))?;
            let (q, r) = (d / unit, d % unit);
            let n = match p {
                DecFloor => d.div_euclid(unit),
                DecCeil => -(-d).div_euclid(unit),
                DecTrunc => q,
                _ if 2 * r.abs() >= unit => q + d.signum(),
                _ => q,
            };
            Ok(vec![Val::Int(n)])
        }
        IntToDec => Ok(vec![Val::Dec(int(&a[0]), 0)]),
        RatioAdd | RatioSub | RatioMul | RatioDiv => {
            let ((x, y), (z, w)) = (fraction(&a[0])?, fraction(&a[1])?);
            if p == RatioDiv && z == 0 {
                return Err((Kind::Arithmetic, "division by zero".into()));
            }
            let r = match p {
                RatioAdd => x
                    .checked_mul(w)
                    .zip(z.checked_mul(y))
                    .and_then(|(a, b)| a.checked_add(b).zip(y.checked_mul(w))),
                RatioSub => x
                    .checked_mul(w)
                    .zip(z.checked_mul(y))
                    .and_then(|(a, b)| a.checked_sub(b).zip(y.checked_mul(w))),
                RatioMul => x.checked_mul(z).zip(y.checked_mul(w)),
                _ => x.checked_mul(w).zip(y.checked_mul(z)),
            };
            let (n, d) = r.ok_or_else(|| overflow("a ratio literal"))?;
            let (n, d) = lowest(n, d);
            Ok(vec![Val::Ratio([n, d].into())])
        }
        IntToRatio => Ok(vec![Val::Ratio([int(&a[0]), 1].into())]),
        DecToRatio => {
            let (d, s) = dec(&a[0]);
            let unit = pow10(s).ok_or_else(|| overflow("a decimal literal"))?;
            let (n, d) = lowest(d, unit);
            Ok(vec![Val::Ratio([n, d].into())])
        }
        RatioToF64 => {
            let (n, d) = fraction(&a[0])?;
            let x = nearest(n, d).ok_or_else(|| {
                (
                    Kind::Literal,
                    format!("{n}:{d} is too large to become an f64"),
                )
            })?;
            Ok(vec![Val::Float(x)])
        }
        RatioToMoney => {
            let (n, d) = fraction(&a[0])?;
            let cents = n.checked_mul(100).ok_or_else(|| overflow("money"))?;
            if cents % d != 0 {
                return Err((
                    Kind::Literal,
                    format!("{n}:{d} is not a whole number of cents, so it is not exact money"),
                ));
            }
            match i64::try_from(cents / d) {
                Ok(c) => Ok(vec![Val::Int(c.into())]),
                Err(_) => Err((Kind::Literal, format!("{n}:{d} is too large for money"))),
            }
        }
        RatioFloor | RatioCeil | RatioRound | RatioTrunc => {
            let (n, d) = fraction(&a[0])?;
            let (q, r) = (n / d, n % d);
            let k = match p {
                RatioFloor => n.div_euclid(d),
                RatioCeil => -(-n).div_euclid(d),
                RatioTrunc => q,
                _ if 2 * r.abs() >= d => q + n.signum(),
                _ => q,
            };
            Ok(vec![Val::Int(k)])
        }
        IntToI64 => {
            let n = int(&a[0]);
            match i64::try_from(n) {
                Ok(_) => Ok(vec![Val::Int(n)]),
                Err(_) => Err((Kind::Literal, format!("{n} is too large for an i64"))),
            }
        }
        IntToF64 => Ok(vec![Val::Float(int(&a[0]) as f64)]),
        IntToBool => match int(&a[0]) {
            n @ (0 | 1) => Ok(vec![Val::Int(n)]),
            n => Err((Kind::Literal, format!("{n} is not a bool: 0 or 1"))),
        },
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
        IntDivMod => {
            let (x, y) = (int(&a[0]), int(&a[1]));
            if y == 0 {
                return Err((Kind::Arithmetic, "division by zero".into()));
            }
            let (mut q, mut r) = (x / y, x % y);
            if r != 0 && (r < 0) != (y < 0) {
                q -= 1;
                r += y;
            }
            Ok(vec![Val::Int(q), Val::Int(r)])
        }
        _ => unreachable!("{p:?} is not a literal's"),
    }
}

/// Where element `k` of `n` is, from 0, if there is one: elements count
/// from 1, and from -1 at the end (Thomas, 2026-10-09); there is no element 0.
pub fn element(k: i128, n: i128) -> Option<usize> {
    let i = match k {
        0 => return None,
        k if k > 0 => k - 1,
        k => n + k,
    };
    usize::try_from(i).ok().filter(|&i| (i as i128) < n)
}

/// Where the gap after element `k` of `n` is, from 0, the gap before the
/// first: gap 0 is before the first element and -1 after the last. A slice's
/// end is the gap after its last element.
pub fn end(k: i128, n: i128) -> Option<usize> {
    let g = if k < 0 { n + k + 1 } else { k };
    usize::try_from(g).ok().filter(|&g| (g as i128) <= n)
}

/// A decimal as it is written.
/// A ratio as it is written: its terms between colons, `1:2`, `-1:3`.
pub fn show_ratio(t: &[i128]) -> String {
    let terms: Vec<String> = t.iter().map(i128::to_string).collect();
    terms.join(":")
}

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

/// A case style for `case`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Snake,
    Kebab,
    Camel,
    Pascal,
}

/// A string's words, for a case style: split at what is not a letter or a
/// digit, and where a capital starts a word (`helloWorld`, `HTTPServer`),
/// in lower case.
fn case_words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if c.is_uppercase() && !word.is_empty() {
            let before = chars[i - 1];
            let after_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if before.is_lowercase()
                || before.is_numeric()
                || (before.is_uppercase() && after_lower)
            {
                words.push(std::mem::take(&mut word));
            }
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words.into_iter().map(|w| w.to_lowercase()).collect()
}

fn capitalized(w: &str) -> String {
    let mut cs = w.chars();
    cs.next()
        .map(|c| c.to_uppercase().chain(cs).collect())
        .unwrap_or_default()
}

/// A string in a case style: `hello_world`, `hello-world`, `helloWorld`,
/// `HelloWorld`.
pub fn restyle(s: &str, style: Style) -> String {
    let words = case_words(s);
    match style {
        Style::Snake => words.join("_"),
        Style::Kebab => words.join("-"),
        Style::Camel => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { w.clone() } else { capitalized(w) })
            .collect(),
        Style::Pascal => words.iter().map(|w| capitalized(w)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ratio_is_the_nearest_f64() {
        // Two small integers are floats exactly, and an IEEE division of
        // exact values is rounded once, to the nearest: the same rounding.
        for n in -300i128..=300 {
            for d in 1i128..=300 {
                assert_eq!(nearest(n, d), Some(n as f64 / d as f64), "{n}:{d}");
            }
        }
        let big = 9_007_199_254_740_993; // 2^53 + 1, no f64: a tie, to even
        assert_eq!(nearest(big, 1), Some(9_007_199_254_740_992.0));
        assert_eq!(nearest(1, 1 << 72), Some(2f64.powi(-72)));
        assert_eq!(nearest(1, 1 << 73), None);
        assert_eq!(lowest(6, -4), (-3, 2));
        assert_eq!(lowest(0, 5), (0, 1));
    }
}
