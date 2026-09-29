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
            Self::Code(_) => b"march7/code/v1\0".as_slice(),
            Self::Data(_) => b"march7/data/v1\0".as_slice(),
        });
        h.update(self.bytes());
        h.finalize().into()
    }
}

// Fixed semantic operations in the march7/code/v1 domain; numbers are wire IDs.
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
