use crate::code::{MAX_BYTES, MAX_OBJECTS, decode};
use crate::{Blob, Cid, Image, Op, Primitive};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidCode,
    InvalidToken,
    Stack,
    Memory,
    ReadOnly,
    Arithmetic,
    Fuel,
    Limit,
    User(u64),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug)]
enum Instruction {
    Return,
    Lit(u64),
    Prim(Primitive),
    Call(usize),
    Quote(usize),
    Branch(usize),
    ZeroBranch(usize),
    Recur,
    Data(u64, usize),
    Tail(usize),
}
struct Executable {
    cid: Cid,
    ops: Vec<Instruction>,
}
struct Region {
    bytes: Vec<u8>,
    cid: Option<Cid>,
    read_only: bool,
}
struct RegionSlot {
    generation: u32,
    region: Option<Region>,
}
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub steps: u64,
    pub peak_stack: usize,
    pub peak_control: usize,
    pub allocated_bytes: u64,
    pub peak_live_bytes: usize,
}

/// Region 1 is the virtual FORTH working memory, allocated at boot. It is not
/// a host pointer and is never serialized. Other handles identify a slot and
/// generation; released slots can be reused without reviving stale addresses.
pub struct Machine {
    pub stack: Vec<u64>,
    /// Per-invocation scratch stack. Each call frame owns the values it pushes;
    /// returning (or tail-calling) discards them, so a word can neither leave
    /// hidden outputs nor read its caller's scratch values.
    scratch: Vec<u64>,
    pub stats: Stats,
    pub(crate) blobs: BTreeMap<Cid, Blob>,
    words: Vec<Executable>,
    linked: HashMap<Cid, usize>,
    regions: Vec<RegionSlot>,
    free_regions: Vec<usize>,
    read_only: HashMap<Cid, u64>,
    live_bytes: usize,
    linked_ops: usize,
    blob_bytes: usize,
}
impl Machine {
    pub fn new(image: &Image) -> Result<Self, Error> {
        image.validate()?;
        let mut m = Self {
            stack: Vec::new(),
            scratch: Vec::new(),
            stats: Stats::default(),
            blobs: image.blobs.clone(),
            words: Vec::new(),
            linked: HashMap::new(),
            regions: vec![RegionSlot {
                generation: 0,
                region: None,
            }],
            free_regions: Vec::new(),
            read_only: HashMap::new(),
            live_bytes: 0,
            linked_ops: 0,
            blob_bytes: image.blobs.values().map(|b| b.bytes().len() + 37).sum(),
        };
        m.allocate(vec![0; 1024 * 1024], None)?;
        Ok(m)
    }
    pub fn empty() -> Self {
        let mut m = Self {
            stack: Vec::new(),
            scratch: Vec::new(),
            stats: Stats::default(),
            blobs: BTreeMap::new(),
            words: Vec::new(),
            linked: HashMap::new(),
            regions: vec![RegionSlot {
                generation: 0,
                region: None,
            }],
            free_regions: Vec::new(),
            read_only: HashMap::new(),
            live_bytes: 0,
            linked_ops: 0,
            blob_bytes: 0,
        };
        m.allocate(vec![0; 1024 * 1024], None)
            .expect("fixed workspace");
        m
    }
    fn allocate(&mut self, bytes: Vec<u8>, cid: Option<Cid>) -> Result<u64, Error> {
        let size = bytes.len();
        if size > MAX_BYTES || self.live_bytes.checked_add(size).ok_or(Error::Limit)? > MAX_BYTES {
            return Err(Error::Limit);
        }
        let slot = if let Some(slot) = self.free_regions.pop() {
            slot
        } else {
            if self.regions.len() >= MAX_OBJECTS {
                return Err(Error::Limit);
            }
            let slot = self.regions.len();
            self.regions.push(RegionSlot {
                generation: 0,
                region: None,
            });
            slot
        };
        let id = ((self.regions[slot].generation as u64) << 32) | slot as u64;
        self.regions[slot].region = Some(Region {
            bytes,
            cid,
            read_only: cid.is_some(),
        });
        self.live_bytes += size;
        self.stats.allocated_bytes += size as u64;
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(id)
    }
    pub fn input(&mut self, bytes: &[u8]) -> Result<u64, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::Limit);
        }
        // Immutable external input need not become a persistent CAS object.
        let id = self.allocate(bytes.to_vec(), None)?;
        let slot = self.region_slot(id)?;
        self.regions[slot]
            .region
            .as_mut()
            .ok_or(Error::Memory)?
            .read_only = true;
        Ok(id)
    }
    pub fn release(&mut self, r: u64) -> Result<(), Error> {
        if r == 1 {
            return Err(Error::Memory);
        }
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.take().ok_or(Error::Memory)?;
        if let Some(c) = region.cid {
            self.read_only.remove(&c);
        }
        self.live_bytes -= region.bytes.len();
        // Never wrap a generation. Retire an exhausted slot permanently.
        if let Some(next) = self.regions[slot].generation.checked_add(1) {
            self.regions[slot].generation = next;
            self.free_regions.push(slot);
        }
        Ok(())
    }
    fn region_slot(&self, r: u64) -> Result<usize, Error> {
        let index = (r & u32::MAX as u64) as usize;
        let slot = self.regions.get(index).ok_or(Error::Memory)?;
        if slot.generation != (r >> 32) as u32 || slot.region.is_none() {
            return Err(Error::Memory);
        }
        Ok(index)
    }
    fn region(&self, r: u64) -> Result<&Region, Error> {
        self.regions[self.region_slot(r)?]
            .region
            .as_ref()
            .ok_or(Error::Memory)
    }
    pub fn read(&self, r: u64, off: u64, n: usize) -> Result<&[u8], Error> {
        let start = usize::try_from(off).map_err(|_| Error::Memory)?;
        let end = start.checked_add(n).ok_or(Error::Memory)?;
        self.region(r)?.bytes.get(start..end).ok_or(Error::Memory)
    }
    pub fn write(&mut self, r: u64, off: u64, b: &[u8]) -> Result<(), Error> {
        let start = usize::try_from(off).map_err(|_| Error::Memory)?;
        let end = start.checked_add(b.len()).ok_or(Error::Memory)?;
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        if region.read_only {
            return Err(Error::ReadOnly);
        }
        region
            .bytes
            .get_mut(start..end)
            .ok_or(Error::Memory)?
            .copy_from_slice(b);
        Ok(())
    }
    pub fn publish(&mut self, blob: Blob) -> Result<Cid, Error> {
        if blob.bytes().len() > MAX_BYTES {
            return Err(Error::Limit);
        }
        let cid = blob.cid();
        if self.blobs.contains_key(&cid) {
            return Ok(cid);
        }
        if self.blobs.len() >= MAX_OBJECTS
            || self
                .blob_bytes
                .checked_add(blob.bytes().len() + 37)
                .ok_or(Error::Limit)?
                > MAX_BYTES
        {
            return Err(Error::Limit);
        }
        // Public insertion maintains a valid, dependency-closed code store.
        // In particular, a failed compiler attempt cannot poison image export.
        if let Blob::Code(bytes) = &blob {
            for op in decode(bytes)? {
                match op {
                    Op::Call(c) | Op::Quote(c) | Op::Tail(c) => {
                        if !matches!(self.blobs.get(&c), Some(Blob::Code(_))) {
                            return Err(Error::InvalidCode);
                        }
                    }
                    Op::Data(c) => {
                        if !matches!(self.blobs.get(&c), Some(Blob::Data(_))) {
                            return Err(Error::InvalidCode);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.blob_bytes += blob.bytes().len() + 37;
        self.blobs.insert(cid, blob);
        Ok(cid)
    }
    pub fn blob_region(&mut self, cid: Cid) -> Result<u64, Error> {
        if let Some(&r) = self.read_only.get(&cid) {
            return Ok(r);
        }
        let bytes = self
            .blobs
            .get(&cid)
            .ok_or(Error::InvalidCode)?
            .bytes()
            .to_vec();
        let r = self.allocate(bytes, Some(cid))?;
        self.read_only.insert(cid, r);
        Ok(r)
    }
    pub fn define(&mut self, ops: &[Op]) -> Result<u64, Error> {
        let cid = self.publish(Blob::Code(crate::code::encode(ops)))?;
        self.link(cid)
    }
    pub fn cid(&self, xt: u64) -> Result<Cid, Error> {
        Ok(self
            .words
            .get(self.token(xt)?)
            .ok_or(Error::InvalidToken)?
            .cid)
    }
    fn token(&self, xt: u64) -> Result<usize, Error> {
        let n = usize::try_from(xt).map_err(|_| Error::InvalidToken)?;
        if n == 0 || n > self.words.len() {
            return Err(Error::InvalidToken);
        }
        Ok(n - 1)
    }
    pub fn link(&mut self, cid: Cid) -> Result<u64, Error> {
        Ok(self.link_inner(cid, 0)? as u64 + 1)
    }
    fn link_inner(&mut self, cid: Cid, depth: usize) -> Result<usize, Error> {
        if let Some(&id) = self.linked.get(&cid) {
            return Ok(id);
        }
        if depth >= 256 || self.words.len() >= MAX_OBJECTS {
            return Err(Error::Limit);
        }
        let Blob::Code(bytes) = self.blobs.get(&cid).ok_or(Error::InvalidCode)? else {
            return Err(Error::InvalidCode);
        };
        let ops = decode(bytes)?;
        let mut out = Vec::with_capacity(ops.len());
        for op in ops {
            out.push(match op {
                Op::Return => Instruction::Return,
                Op::Lit(n) => Instruction::Lit(n),
                Op::Prim(p) => Instruction::Prim(p),
                Op::Call(c) => Instruction::Call(self.link_inner(c, depth + 1)?),
                Op::Quote(c) => Instruction::Quote(self.link_inner(c, depth + 1)?),
                Op::Tail(c) => Instruction::Tail(self.link_inner(c, depth + 1)?),
                Op::Branch(n) => Instruction::Branch(n as usize),
                Op::ZeroBranch(n) => Instruction::ZeroBranch(n as usize),
                Op::Recur => Instruction::Recur,
                Op::Data(c) => {
                    if !matches!(self.blobs.get(&c), Some(Blob::Data(_))) {
                        return Err(Error::InvalidCode);
                    }
                    let r = self.blob_region(c)?;
                    Instruction::Data(r, self.region(r)?.bytes.len())
                }
            });
        }
        if self.linked_ops + out.len() > 1_000_000 {
            return Err(Error::Limit);
        }
        self.linked_ops += out.len();
        let id = self.words.len();
        self.words.push(Executable { cid, ops: out });
        self.linked.insert(cid, id);
        Ok(id)
    }
    fn pop(&mut self) -> Result<u64, Error> {
        self.stack.pop().ok_or(Error::Stack)
    }
    fn push(&mut self, n: u64) -> Result<(), Error> {
        if self.stack.len() >= 65536 {
            return Err(Error::Stack);
        }
        self.stack.push(n);
        self.stats.peak_stack = self.stats.peak_stack.max(self.stack.len());
        Ok(())
    }
    fn address(&mut self) -> Result<(u64, u64), Error> {
        let o = self.pop()?;
        let r = self.pop()?;
        Ok((r, o))
    }
    fn slice(&mut self) -> Result<Vec<u8>, Error> {
        let n = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
        let (r, o) = self.address()?;
        Ok(self.read(r, o, n)?.to_vec())
    }
    pub fn run(&mut self, xt: u64, mut fuel: u64) -> Result<(), Error> {
        let mut word = self.token(xt)?;
        let mut ip = 0usize;
        // Each return frame also records its caller's scratch-frame base.
        let mut returns: Vec<(usize, usize, usize)> = Vec::new();
        let mut base = 0usize;
        self.scratch.clear();
        loop {
            if fuel == 0 {
                return Err(Error::Fuel);
            }
            fuel -= 1;
            self.stats.steps += 1;
            let op = *self.words[word].ops.get(ip).ok_or(Error::InvalidCode)?;
            ip += 1;
            match op {
                Instruction::Return => {
                    self.scratch.truncate(base);
                    match returns.pop() {
                        Some((w, i, b)) => {
                            word = w;
                            ip = i;
                            base = b;
                        }
                        None => return Ok(()),
                    }
                }
                Instruction::Lit(n) => self.push(n)?,
                Instruction::Quote(w) => self.push(w as u64 + 1)?,
                Instruction::Data(r, len) => {
                    self.region(r)?;
                    self.push(r)?;
                    self.push(0)?;
                    self.push(len as u64)?;
                }
                Instruction::Branch(n) => ip = n,
                Instruction::ZeroBranch(n) => {
                    if self.pop()? == 0 {
                        ip = n;
                    }
                }
                Instruction::Call(w) => {
                    if returns.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    returns.push((word, ip, base));
                    base = self.scratch.len();
                    word = w;
                    ip = 0;
                }
                Instruction::Recur => {
                    if returns.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    returns.push((word, ip, base));
                    base = self.scratch.len();
                    ip = 0;
                }
                Instruction::Tail(w) => {
                    // The current word is finished: its scratch values go.
                    self.scratch.truncate(base);
                    word = w;
                    ip = 0;
                }
                Instruction::Prim(Primitive::Execute) => {
                    let xt = self.pop()?;
                    let w = self.token(xt)?;
                    if returns.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    returns.push((word, ip, base));
                    base = self.scratch.len();
                    word = w;
                    ip = 0;
                }
                Instruction::Prim(Primitive::ScratchPush) => {
                    let v = self.pop()?;
                    if self.scratch.len() >= 65536 {
                        return Err(Error::Stack);
                    }
                    self.scratch.push(v);
                }
                Instruction::Prim(Primitive::ScratchPop) => {
                    if self.scratch.len() <= base {
                        return Err(Error::Stack);
                    }
                    let v = self.scratch.pop().ok_or(Error::Stack)?;
                    self.push(v)?;
                }
                Instruction::Prim(Primitive::ScratchPeek) => {
                    if self.scratch.len() <= base {
                        return Err(Error::Stack);
                    }
                    let v = *self.scratch.last().ok_or(Error::Stack)?;
                    self.push(v)?;
                }
                Instruction::Prim(p) => self.primitive(p)?,
            }
            self.stats.peak_control = self.stats.peak_control.max(returns.len());
        }
    }
    fn primitive(&mut self, p: Primitive) -> Result<(), Error> {
        use Primitive::*;
        match p {
            Dup => {
                let a = *self.stack.last().ok_or(Error::Stack)?;
                self.push(a)?;
            }
            Drop => {
                self.pop()?;
            }
            Swap => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(a)?;
            }
            Over => {
                let n = self.stack.len().checked_sub(2).ok_or(Error::Stack)?;
                self.push(self.stack[n])?;
            }
            Rot => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(c)?;
                self.push(a)?;
            }
            Add | Sub | Mul | Div | Mod | Eq | Lt | And | Or | Xor | Shl | Shr => {
                let b = self.pop()?;
                let a = self.pop()?;
                let n = match p {
                    Add => a.wrapping_add(b),
                    Sub => a.wrapping_sub(b),
                    Mul => a.wrapping_mul(b),
                    Div => a.checked_div(b).ok_or(Error::Arithmetic)?,
                    Mod => a.checked_rem(b).ok_or(Error::Arithmetic)?,
                    Eq => (a == b) as u64,
                    Lt => (a < b) as u64,
                    And => a & b,
                    Or => a | b,
                    Xor => a ^ b,
                    Shl | Shr => {
                        if b >= 64 {
                            return Err(Error::Arithmetic);
                        }
                        if p == Shl { a << b } else { a >> b }
                    }
                    _ => unreachable!(),
                };
                self.push(n)?;
            }
            Not => {
                let a = self.pop()?;
                self.push(!a)?;
            }
            Load8 | Load64 => {
                let (r, o) = self.address()?;
                let n = if p == Load8 { 1 } else { 8 };
                if n == 8 && o % 8 != 0 {
                    return Err(Error::Memory);
                }
                let b = self.read(r, o, n)?;
                let value = if n == 1 {
                    b[0] as u64
                } else {
                    u64::from_le_bytes(b.try_into().map_err(|_| Error::Memory)?)
                };
                self.push(value)?;
            }
            Store8 | Store64 => {
                let (r, o) = self.address()?;
                let value = self.pop()?;
                if p == Store64 && o % 8 != 0 {
                    return Err(Error::Memory);
                }
                self.write(
                    r,
                    o,
                    &value.to_le_bytes()[..if p == Store8 { 1 } else { 8 }],
                )?;
            }
            RegionNew => {
                let size = usize::try_from(self.pop()?).map_err(|_| Error::Limit)?;
                if size > MAX_BYTES || self.live_bytes + size > MAX_BYTES {
                    return Err(Error::Limit);
                }
                let r = self.allocate(vec![0; size], None)?;
                self.push(r)?;
            }
            RegionFree => {
                let r = self.pop()?;
                if self.region(r)?.read_only {
                    return Err(Error::ReadOnly);
                }
                self.release(r)?;
            }
            RegionSize => {
                let r = self.pop()?;
                self.push(self.region(r)?.bytes.len() as u64)?;
            }
            Seal => {
                let bytes = self.slice()?;
                let cid = self.publish(Blob::Code(bytes))?;
                let xt = self.link(cid)?;
                self.push(xt)?;
            }
            CodeCid => {
                let (r, o) = self.address()?;
                let xt = self.pop()?;
                let cid = self.cid(xt)?;
                self.write(r, o, &cid)?;
            }
            Resolve => {
                let (r, o) = self.address()?;
                let cid = self.read(r, o, 32)?.try_into().map_err(|_| Error::Memory)?;
                let xt = self.link(cid)?;
                self.push(xt)?;
            }
            Publish => {
                let bytes = self.slice()?;
                let cid = self.publish(Blob::Data(bytes))?;
                let r = self.blob_region(cid)?;
                self.push(r)?;
            }
            BlobCid => {
                let (r, o) = self.address()?;
                let blob = self.pop()?;
                let cid = self.region(blob)?.cid.ok_or(Error::InvalidCode)?;
                if !self.blobs.contains_key(&cid) {
                    return Err(Error::InvalidCode);
                }
                self.write(r, o, &cid)?;
            }
            BlobRead => {
                let (r, o) = self.address()?;
                let cid = self.read(r, o, 32)?.try_into().map_err(|_| Error::Memory)?;
                let blob = self.blob_region(cid)?;
                let n = self.region(blob)?.bytes.len();
                self.push(blob)?;
                self.push(0)?;
                self.push(n as u64)?;
            }
            Trap => return Err(Error::User(self.pop()?)),
            Execute | ScratchPush | ScratchPop | ScratchPeek => unreachable!(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_generation_retires_slot_instead_of_wrapping() {
        let mut m = Machine::empty();
        let old = m.input(b"x").unwrap();
        let slot = m.region_slot(old).unwrap();
        m.regions[slot].generation = u32::MAX;
        let last = (u64::from(u32::MAX) << 32) | slot as u64;
        assert_eq!(m.read(old, 0, 1), Err(Error::Memory));
        m.release(last).unwrap();
        let fresh = m.input(b"y").unwrap();
        assert_ne!(fresh as u32, old as u32);
        assert_eq!(m.read(old, 0, 1), Err(Error::Memory));
        assert_eq!(m.read(last, 0, 1), Err(Error::Memory));
        assert_eq!(m.release(1), Err(Error::Memory));
    }

    #[test]
    fn publication_validates_before_inserting_and_counts_only_new_bytes() {
        let mut m = Machine::empty();
        let data = Blob::Data(b"data".to_vec());
        let data_cid = m.publish(data.clone()).unwrap();
        m.publish(data).unwrap();
        let code = Blob::Code(crate::code::encode(&[Op::Return]));
        let code_cid = m.publish(code).unwrap();
        let before = m.blobs.clone();
        let bytes = m.blob_bytes;
        for bad in [
            Blob::Code(vec![255]),
            Blob::Code(crate::code::encode(&[Op::Call([9; 32]), Op::Return])),
            Blob::Code(crate::code::encode(&[Op::Quote([9; 32]), Op::Return])),
            Blob::Code(crate::code::encode(&[Op::Tail([9; 32])])),
            Blob::Code(crate::code::encode(&[Op::Data([9; 32]), Op::Return])),
            Blob::Code(crate::code::encode(&[Op::Call(data_cid), Op::Return])),
            Blob::Code(crate::code::encode(&[Op::Quote(data_cid), Op::Return])),
            Blob::Code(crate::code::encode(&[Op::Tail(data_cid)])),
            Blob::Code(crate::code::encode(&[Op::Data(code_cid), Op::Return])),
        ] {
            assert_eq!(m.publish(bad), Err(Error::InvalidCode));
            assert_eq!(m.blobs, before);
            assert_eq!(m.blob_bytes, bytes);
        }
        assert_eq!(bytes, m.blobs.values().map(|b| b.bytes().len() + 37).sum());
    }
}
