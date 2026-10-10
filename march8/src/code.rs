use crate::machine::Error;
use sha2::{Digest, Sha256};

pub type Cid = [u8; 32];
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_OBJECTS: usize = 100_000;
pub const MAX_WORD_OPS: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    Code(Vec<u8>),
    Data(Vec<u8>),
}
impl Blob {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Code(b) | Self::Data(b) => b,
        }
    }
    pub fn cid(&self) -> Cid {
        let mut h = Sha256::new();
        h.update(match self {
            Self::Code(_) => b"march8/code/v1\0".as_slice(),
            Self::Data(_) => b"march8/data/v1\0".as_slice(),
        });
        h.update(self.bytes());
        h.finalize().into()
    }
}

// Fixed semantic operations in the march8/code/v1 domain; numbers are wire IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Primitive {
    Dup = 0,
    Drop = 1,
    Swap = 2,
    Over = 3,
    Rot = 4,
    Add = 5,
    Sub = 6,
    Mul = 7,
    Div = 8,
    Mod = 9,
    Eq = 10,
    Lt = 11,
    And = 12,
    Or = 13,
    Xor = 14,
    Not = 15,
    Shl = 16,
    Shr = 17,
    Load8 = 18,
    Store8 = 19,
    Load64 = 20,
    Store64 = 21,
    RegionNew = 22,
    RegionFree = 23,
    RegionSize = 24,
    Execute = 25,
    Seal = 26,
    CodeCid = 27,
    Resolve = 28,
    Publish = 29,
    BlobCid = 30,
    BlobRead = 31,
    Trap = 32,
    ScratchPush = 33,
    ScratchPop = 34,
    ScratchPeek = 35,
    // IEEE-754 binary64 on the cell's bits. Results that are NaN are
    // canonicalized to one bit pattern so equal computations give equal bytes.
    FAdd = 36,
    FSub = 37,
    FMul = 38,
    FDiv = 39,
    FEq = 40,
    FLt = 41,
    IToF = 42,
    FToI = 43,
    // Load64 and Store64 in the working region, handle 1, without the region
    // operand: March's `get` and `put`, which the compiler runs constantly.
    WorkLoad = 44,
    WorkStore = 45,
    // Array literals (docs/ARRAYS.md): `mark` pushes the data stack's depth
    // onto the mark stack, kept per call frame like the scratch stack; `gather`
    // pops it and moves every cell above that depth into a new vector.
    Mark = 46,
    Gather = 47,
    // Persistent vectors of cells (docs/ARRAYS.md), held in regions:
    // length, element, append in place (for building), and a new version
    // with one element replaced.
    VecLen = 48,
    VecAt = 49,
    VecPush = 50,
    VecSet = 51,
    // Strings (docs/STRINGS.md): text from UTF-8 bytes in a region; and, for
    // arrays and strings alike, joining, slicing (by element or character)
    // and equality of contents.
    Text = 52,
    Concat = 53,
    Slice = 54,
    Same = 55,
    // Maps (docs/MAPS.md): persistent CHAMP maps of cells, gathered from the
    // key and value pairs above a mark like an array literal; a new version
    // with a key set (or an array element replaced), membership, a new
    // version without a key, and the keys and values as arrays.
    MapGather = 56,
    Put = 57,
    Has = 58,
    Remove = 59,
    Keys = 60,
    Values = 61,
    // Byte spans (docs/REBUILD.md), so reading source needs no byte loops in
    // March: the first byte equal to, above, or at or below a given byte,
    // in a region from an offset to an end (or the end); FNV-1a over a span;
    // two spans equal; and a span's decimal digits as a number.
    ByteFind = 62,
    BytePast = 63,
    ByteUpto = 64,
    ByteHash = 65,
    BytesEq = 66,
    Decimal = 67,
    // Strings, second slice (docs/STRINGS.md): a value as text by its checker
    // type, writing text out, ordering two strings, searching one in
    // another, and reading an integer or a float from a string.
    TextOf = 68,
    Write = 69,
    Compare = 70,
    Search = 71,
    TextInt = 72,
    TextFloat = 73,
    // March8 (docs/MACHINE.md): a copy of the cell n below the top; checked
    // signed integer arithmetic, which traps on overflow and on division by
    // zero; signed order; and numbers as text.
    Pick = 74,
    IAdd = 75,
    ISub = 76,
    IMul = 77,
    IDiv = 78,
    IMod = 79,
    ILt = 80,
    IntText = 81,
    FloatText = 82,
    // ( n -- x ) A copy of the scratch cell n below the top, in this frame:
    // a loop's state, read as FORTH reads its indices.
    ScratchAt = 83,
    // ( n -- a ) The array 1 to n; ( a -- a ) an array in reverse.
    Range = 84,
    Reverse = 85,
    // ( j m -- x ) A copy of the cell j below the mark m from the innermost,
    // in this frame: what an array literal pulls from below it with `_`.
    MarkPick = 86,
    // Money as text, from its cents: `21.09`; a string as a literal that
    // reads back, quoted and escaped.
    MoneyText = 87,
    StringShow = 88,
    // ( a -- a ) An array sorted: of integers by value, of floats by value
    // (IEEE total order), of strings by text, code point by code point.
    SortInts = 89,
    SortFloats = 90,
    SortTexts = 91,
    // ( x -- √x ), ( x y -- xʸ ) for floats; ( x y -- xʸ ) for integers,
    // checked, y at least 0.
    FSqrt = 92,
    FPow = 93,
    IPow = 94,
    // ( a b -- q r ) Floored division: q rounded down, r with the sign of b,
    // a = q·b + r; traps on division by zero and on overflow.
    IDivMod = 95,
    // ( x -- y ) A float rounded to a whole float: down, up, or half away
    // from zero. `f>i` then makes it an integer, checked.
    FFloor = 96,
    FCeil = 97,
    FRound = 98,
    // ( a x g -- a ) x inserted at the gap g, from 0, before the first
    // element; ( a i -- a ) the element at i, from 0, removed.
    VecInsert = 99,
    VecRemove = 100,
    // ( x t -- u ) A value of a union type (doc/design/TYPES.md 3.6): x and
    // t, the tag of x's type, as one cell; ( u -- t ) its tag; ( u -- x )
    // its value.
    UnionMake = 101,
    UnionTag = 102,
    UnionValue = 103,
    // ( a -- p ) The positions, from 1, that would put an array in order:
    // its grade, stable, as APL's `⍋`, by integer, float or text.
    GradeInts = 104,
    GradeFloats = 105,
    GradeTexts = 106,
    // ( a m -- a ) The elements whose mask cell is not 0: APL's compress.
    Keep = 107,
    // ( a -- p ) The grade down, stable: the positions, from 1, that would
    // put an array in descending order, as APL's `⍒`.
    GradeDownInts = 108,
    GradeDownFloats = 109,
    GradeDownTexts = 110,
    // ( s sep -- a ) A string's parts between separators, or its characters
    // for an empty one; ( s -- a ) its lines, or its words, the parts between
    // runs of white space.
    Split = 111,
    Lines = 112,
    Words = 113,
    // ( s -- s ) A string in lower case, or upper.
    Lower = 114,
    Upper = 115,
    // ( s -- x 1 ) The integer or the float a string writes, white space
    // around it allowed; ( s -- 0 0 ) if it writes none.
    ParseInt = 116,
    ParseFloat = 117,
}
impl Primitive {
    pub const ALL: &'static [(Self, &'static str)] = &[
        (Self::Dup, "dup"),
        (Self::Drop, "drop"),
        (Self::Swap, "swap"),
        (Self::Over, "over"),
        (Self::Rot, "rot"),
        (Self::Add, "add"),
        (Self::Sub, "sub"),
        (Self::Mul, "mul"),
        (Self::Div, "div"),
        (Self::Mod, "mod"),
        (Self::Eq, "eq"),
        (Self::Lt, "lt"),
        (Self::And, "and"),
        (Self::Or, "or"),
        (Self::Xor, "xor"),
        (Self::Not, "not"),
        (Self::Shl, "shl"),
        (Self::Shr, "shr"),
        (Self::Load8, "load8"),
        (Self::Store8, "store8"),
        (Self::Load64, "load64"),
        (Self::Store64, "store64"),
        (Self::RegionNew, "region-new"),
        (Self::RegionFree, "region-free"),
        (Self::RegionSize, "region-size"),
        (Self::Execute, "execute"),
        (Self::Seal, "seal"),
        (Self::CodeCid, "code-cid"),
        (Self::Resolve, "resolve"),
        (Self::Publish, "publish"),
        (Self::BlobCid, "blob-cid"),
        (Self::BlobRead, "blob-read"),
        (Self::Trap, "trap"),
        (Self::ScratchPush, "scratch-push"),
        (Self::ScratchPop, "scratch-pop"),
        (Self::ScratchPeek, "scratch-peek"),
        (Self::FAdd, "f+"),
        (Self::FSub, "f-"),
        (Self::FMul, "f*"),
        (Self::FDiv, "f/"),
        (Self::FEq, "feq?"),
        (Self::FLt, "flt?"),
        (Self::IToF, "i>f"),
        (Self::FToI, "f>i"),
        (Self::WorkLoad, "work-load"),
        (Self::WorkStore, "work-store"),
        (Self::Mark, "mark"),
        (Self::Gather, "gather"),
        (Self::VecLen, "vector-length"),
        (Self::VecAt, "vector-at"),
        (Self::VecPush, "vector-push"),
        (Self::VecSet, "vector-set"),
        (Self::Text, "text"),
        (Self::Concat, "concat"),
        (Self::Slice, "slice"),
        (Self::Same, "same?"),
        (Self::MapGather, "map-gather"),
        (Self::Put, "put"),
        (Self::Has, "has?"),
        (Self::Remove, "remove"),
        (Self::Keys, "keys"),
        (Self::Values, "values"),
        (Self::ByteFind, "byte-find"),
        (Self::BytePast, "byte-past"),
        (Self::ByteUpto, "byte-upto"),
        (Self::ByteHash, "byte-hash"),
        (Self::BytesEq, "bytes-eq?"),
        (Self::Decimal, "decimal"),
        (Self::TextOf, "text-of"),
        (Self::Write, "write"),
        (Self::Compare, "compare"),
        (Self::Search, "search"),
        (Self::TextInt, "text>integer"),
        (Self::TextFloat, "text>float"),
        (Self::Pick, "pick"),
        (Self::IAdd, "i64+"),
        (Self::ISub, "i64-"),
        (Self::IMul, "i64*"),
        (Self::IDiv, "i64-quot"),
        (Self::IMod, "i64-rem"),
        (Self::ILt, "i64lt?"),
        (Self::IntText, "i64>text"),
        (Self::FloatText, "f64>text"),
        (Self::ScratchAt, "scratch-at"),
        (Self::Range, "range"),
        (Self::Reverse, "reverse"),
        (Self::MarkPick, "mark-pick"),
        (Self::MoneyText, "money>text"),
        (Self::StringShow, "string-show"),
        (Self::SortInts, "sort-ints"),
        (Self::SortFloats, "sort-floats"),
        (Self::SortTexts, "sort-texts"),
        (Self::FSqrt, "f64-sqrt"),
        (Self::FPow, "f64-pow"),
        (Self::IPow, "i64-pow"),
        (Self::IDivMod, "i64-divmod"),
        (Self::FFloor, "f64-floor"),
        (Self::FCeil, "f64-ceil"),
        (Self::FRound, "f64-round"),
        (Self::VecInsert, "vector-insert"),
        (Self::VecRemove, "vector-remove"),
        (Self::UnionMake, "union-make"),
        (Self::UnionTag, "union-tag"),
        (Self::UnionValue, "union-value"),
        (Self::GradeInts, "grade-ints"),
        (Self::GradeFloats, "grade-floats"),
        (Self::GradeTexts, "grade-texts"),
        (Self::Keep, "vector-keep"),
        (Self::GradeDownInts, "grade-down-ints"),
        (Self::GradeDownFloats, "grade-down-floats"),
        (Self::GradeDownTexts, "grade-down-texts"),
        (Self::Split, "split"),
        (Self::Lines, "lines"),
        (Self::Words, "words"),
        (Self::Lower, "lower"),
        (Self::Upper, "upper"),
        (Self::ParseInt, "parse-int"),
        (Self::ParseFloat, "parse-float"),
    ];
    pub fn decode(n: u8) -> Result<Self, Error> {
        Self::ALL
            .iter()
            .find(|(primitive, _)| *primitive as u8 == n)
            .map(|p| p.0)
            .ok_or(Error::InvalidCode)
    }
    pub fn named(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|p| p.1 == s).map(|p| p.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Return,
    Lit(u64),
    Prim(Primitive),
    Call(Cid),
    Quote(Cid),
    Branch(u32),
    ZeroBranch(u32),
    Recur,
    Data(Cid),
    Tail(Cid),
    /// A literal whose bits are an IEEE-754 binary64. It runs exactly like
    /// `Lit`; the distinct opcode keeps the literal's type in canonical code,
    /// where the checker reads it.
    Float(u64),
    /// `Recur` in tail position: the current word starts again in its own
    /// frame, so a loop written as recursion runs in constant space.
    TailRecur,
}
impl Op {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Self::Return => out.push(0),
            Self::Lit(n) => {
                out.push(1);
                out.extend(n.to_le_bytes());
            }
            Self::Prim(p) => {
                out.push(2);
                out.push(*p as u8);
            }
            Self::Call(c) | Self::Quote(c) | Self::Data(c) | Self::Tail(c) => {
                out.push(match self {
                    Self::Call(_) => 3,
                    Self::Quote(_) => 4,
                    Self::Data(_) => 8,
                    _ => 9,
                });
                out.extend(c);
            }
            Self::Branch(n) | Self::ZeroBranch(n) => {
                out.push(if matches!(self, Self::Branch(_)) {
                    5
                } else {
                    6
                });
                out.extend(n.to_le_bytes());
            }
            Self::Recur => out.push(7),
            Self::TailRecur => out.push(11),
            Self::Float(n) => {
                out.push(10);
                out.extend(n.to_le_bytes());
            }
        }
    }
}
pub fn encode(ops: &[Op]) -> Vec<u8> {
    let mut b = Vec::new();
    for op in ops {
        op.encode(&mut b);
    }
    b
}
pub fn decode(bytes: &[u8]) -> Result<Vec<Op>, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    let mut r = Reader(bytes);
    let mut ops = Vec::new();
    while !r.0.is_empty() {
        if ops.len() >= MAX_WORD_OPS {
            return Err(Error::Limit);
        }
        ops.push(match r.byte()? {
            0 => Op::Return,
            1 => Op::Lit(r.u64()?),
            2 => Op::Prim(Primitive::decode(r.byte()?)?),
            3 => Op::Call(r.array()?),
            4 => Op::Quote(r.array()?),
            5 => Op::Branch(r.u32()?),
            6 => Op::ZeroBranch(r.u32()?),
            7 => Op::Recur,
            8 => Op::Data(r.array()?),
            9 => Op::Tail(r.array()?),
            10 => Op::Float(r.u64()?),
            11 => Op::TailRecur,
            _ => return Err(Error::InvalidCode),
        });
    }
    if ops.is_empty() {
        return Err(Error::InvalidCode);
    }
    for op in &ops {
        if let Op::Branch(n) | Op::ZeroBranch(n) = op
            && *n as usize >= ops.len()
        {
            return Err(Error::InvalidCode);
        }
    }
    Ok(ops)
}
pub(crate) struct Reader<'a>(pub &'a [u8]);
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if n > self.0.len() {
            return Err(Error::InvalidCode);
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?.try_into().map_err(|_| Error::InvalidCode)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
