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
    TailRecur,
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
    /// An array (docs/ARRAYS.md); its bytes are empty, so byte access fails.
    array: Option<Array>,
    /// A string (docs/STRINGS.md): UTF-8 text, measured in characters and
    /// lines; its bytes are empty too.
    text: Option<Text>,
    /// A map (docs/MAPS.md) from cells to cells.
    map: Option<Map>,
    /// Bytes counted against the live limit: the byte length, or 8 per
    /// element of an array.
    charged: usize,
}

/// A string: UTF-8 text in a content-defined sequence, whose branches count
/// characters and lines.
pub type Text = merkle_champ::Sequence<merkle_champ::sequence::TextByte>;

/// A map: a persistent CHAMP map from cells to cells. Strings are interned,
/// so a string key is a cell like any other.
pub type Map = merkle_champ::ChampMap<u64, u64>;

/// The checker's type of a string (docs/STRINGS.md).
const STRING: u8 = 253;
/// Map types run from 231 (unknown keys and values) to 245, by key and
/// value kind, and 246 is the empty map (docs/MAPS.md).
const MAP: u8 = 231;
const EMPTY_MAP: u8 = 246;

/// The type of an array's elements, for a type byte as the checker writes it
/// (docs/ARRAYS.md): 3, 4 and 5 are arrays of i64, f64 and unknown elements,
/// each rank adds 3, 247 holds strings, 252 has mixed elements and 254 is
/// empty.
fn element_type(t: u8) -> u8 {
    match t {
        3 => 1,
        4 => 2,
        6..=230 => t - 3,
        247 => STRING,
        _ => 0,
    }
}

/// An array: a persistent, content-defined sequence of cells, and the cells
/// appended to it since (`vector-push`, which builds in place), which join
/// the sequence when the array is next needed whole.
struct Array {
    seq: merkle_champ::Sequence<u64>,
    tail: Vec<u64>,
}

impl Array {
    fn len(&self) -> usize {
        self.seq.len() + self.tail.len()
    }
    fn get(&self, i: usize) -> Option<u64> {
        match i.checked_sub(self.seq.len()) {
            None => self.seq.get(i).copied(),
            Some(j) => self.tail.get(j).copied(),
        }
    }
    /// The whole array as one sequence.
    fn whole(&self) -> merkle_champ::Sequence<u64> {
        if self.tail.is_empty() {
            self.seq.clone()
        } else {
            self.seq.concat(&self.tail.iter().copied().collect())
        }
    }
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

/// The working region's handle.
const WORKING: u64 = 1;

/// Region 1 is the virtual FORTH working memory, allocated at boot. It is not
/// a host pointer and is never serialized. Other handles identify a slot and
/// generation; released slots can be reused without reviving stale addresses.
pub struct Machine {
    pub stack: Vec<u64>,
    /// Per-invocation scratch stack. Each call frame owns the values it pushes;
    /// returning (or tail-calling) discards them, so a word can neither leave
    /// hidden outputs nor read its caller's scratch values.
    scratch: Vec<u64>,
    /// Depths marked by `mark` for array literals, per call frame in the same
    /// way, apart from the scratch stack so that a literal inside a loop body
    /// does not hide the loop's index.
    marks: Vec<u64>,
    /// Strings by the identity of their text, so equal text is one handle.
    strings: HashMap<merkle_champ::Identity, u64>,
    /// Strings made by `text` from read-only bytes, by where the bytes are:
    /// they cannot change, and making one chunks and hashes all of them, so
    /// a literal in a loop is made once, not on every pass.
    data_texts: HashMap<(u64, u64, usize), u64>,
    /// Text written by `write`, for the host to show.
    output: Vec<u8>,
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
            marks: Vec::new(),
            strings: HashMap::new(),
            data_texts: HashMap::new(),
            output: Vec::new(),
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
            marks: Vec::new(),
            strings: HashMap::new(),
            data_texts: HashMap::new(),
            output: Vec::new(),
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
            array: None,
            text: None,
            map: None,
            charged: size,
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
        if let Some(t) = &region.text {
            self.strings.remove(&t.identity());
        }
        self.live_bytes -= region.charged;
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
    /// The handle of a string with this text: an existing one, since strings
    /// are interned, so equal text is one handle and strings compare and hash
    /// as cells; otherwise a new region, charged a byte per byte.
    fn allocate_text(&mut self, t: Text) -> Result<u64, Error> {
        let id = t.identity();
        if let Some(&r) = self.strings.get(&id) {
            return Ok(r);
        }
        let r = self.allocate_new_text(t)?;
        self.strings.insert(id, r);
        Ok(r)
    }
    fn allocate_new_text(&mut self, t: Text) -> Result<u64, Error> {
        let size = t.len();
        if size > MAX_BYTES || self.live_bytes.checked_add(size).ok_or(Error::Limit)? > MAX_BYTES {
            return Err(Error::Limit);
        }
        let r = self.allocate(Vec::new(), None)?;
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        region.text = Some(t);
        region.charged = size;
        self.live_bytes += size;
        self.stats.allocated_bytes += size as u64;
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(r)
    }
    /// The text a string handle holds.
    pub fn text(&self, r: u64) -> Result<&Text, Error> {
        self.region(r)?.text.as_ref().ok_or(Error::Memory)
    }
    /// Writes a value of checker type `t` as March writes it: integers
    /// signed, floats with a point, arrays as `( … )`, maps as `{ … }` sorted
    /// by key, strings as literals that read back (docs/STRINGS.md), except a
    /// string at the top when `quote` is false, which is its text as it is.
    /// Collections show their first `limit` elements, if one is given. A
    /// value whose type is unknown is written as an integer.
    pub fn format_value(&self, v: u64, t: u8, quote: bool, limit: Option<usize>, out: &mut String) {
        let shown = limit.unwrap_or(usize::MAX);
        if t == STRING
            && let Some(s) = self.text(v).ok().and_then(|t| t.to_text())
        {
            if !quote {
                out.push_str(&s);
                return;
            }
            out.push('"');
            for c in s.chars() {
                match c {
                    '\\' | '"' => {
                        out.push('\\');
                        out.push(c);
                    }
                    '\n' => out.push_str("\\n;"),
                    '\t' => out.push_str("\\t;"),
                    '\r' => out.push_str("\\r;"),
                    c if c.is_control() => {
                        out.push_str(&format!("\\#{};", u32::from(c)));
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
            return;
        }
        if (MAP..=EMPTY_MAP).contains(&t)
            && let Ok(map) = self.map(v)
        {
            // Keys and values are shown by the kinds the type gives them.
            let (k, val) = ((t - MAP) / 5, (t - MAP) % 5);
            let key_type = [0, 1, STRING][usize::from(k.min(2))];
            let value_type = [0, 1, 2, STRING, 0][usize::from(val)];
            let mut entries: Vec<(String, u64, u64)> = map
                .iter()
                .map(|(&key, &value)| {
                    let mut s = String::new();
                    self.format_value(key, key_type, true, limit, &mut s);
                    (s, key, value)
                })
                .collect();
            if key_type == 1 {
                entries.sort_by_key(|e| e.1 as i64);
            } else {
                entries.sort();
            }
            out.push('{');
            for (key, _, value) in entries.iter().take(shown) {
                out.push(' ');
                out.push_str(key);
                out.push(' ');
                self.format_value(*value, value_type, true, limit, out);
            }
            if entries.len() > shown {
                out.push_str(&format!(" … {} more", entries.len() - shown));
            }
            out.push_str(" }");
            return;
        }
        if t == 2 {
            out.push_str(&format!("{:?}", f64::from_bits(v)));
            return;
        }
        if ((3..=230).contains(&t) || t == 247 || t == 252 || t == 254)
            && let Ok(seq) = self.sequence(v)
        {
            out.push('(');
            for &x in seq.iter().take(shown) {
                out.push(' ');
                self.format_value(x, element_type(t), true, limit, out);
            }
            if seq.len() > shown {
                out.push_str(&format!(" … {} more", seq.len() - shown));
            }
            out.push_str(" )");
            return;
        }
        out.push_str(&(v as i64).to_string());
    }
    /// The text written by `write` (primitive 69) since it was last taken.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }
    /// A new region holding a map, charged 16 bytes per entry.
    fn allocate_map(&mut self, m: Map) -> Result<u64, Error> {
        let size = m.len().checked_mul(16).ok_or(Error::Limit)?;
        if size > MAX_BYTES || self.live_bytes.checked_add(size).ok_or(Error::Limit)? > MAX_BYTES {
            return Err(Error::Limit);
        }
        let r = self.allocate(Vec::new(), None)?;
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        region.map = Some(m);
        region.charged = size;
        self.live_bytes += size;
        self.stats.allocated_bytes += size as u64;
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(r)
    }
    /// The map a map handle holds.
    pub fn map(&self, r: u64) -> Result<&Map, Error> {
        self.region(r)?.map.as_ref().ok_or(Error::Memory)
    }
    /// A new region holding an array, charged 8 bytes per element.
    fn allocate_vector(&mut self, v: merkle_champ::Sequence<u64>) -> Result<u64, Error> {
        let size = v.len().checked_mul(8).ok_or(Error::Limit)?;
        if size > MAX_BYTES || self.live_bytes.checked_add(size).ok_or(Error::Limit)? > MAX_BYTES {
            return Err(Error::Limit);
        }
        let r = self.allocate(Vec::new(), None)?;
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        region.array = Some(Array {
            seq: v,
            tail: Vec::new(),
        });
        region.charged = size;
        self.live_bytes += size;
        self.stats.allocated_bytes += size as u64;
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(r)
    }
    fn array(&self, r: u64) -> Result<&Array, Error> {
        self.region(r)?.array.as_ref().ok_or(Error::Memory)
    }
    /// The elements an array handle holds, as one sequence.
    pub fn sequence(&self, r: u64) -> Result<merkle_champ::Sequence<u64>, Error> {
        Ok(self.array(r)?.whole())
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
    /// A published blob.
    pub fn blob(&self, cid: &Cid) -> Option<&Blob> {
        self.blobs.get(cid)
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
                Op::Lit(n) | Op::Float(n) => Instruction::Lit(n),
                Op::Prim(p) => Instruction::Prim(p),
                Op::Call(c) => Instruction::Call(self.link_inner(c, depth + 1)?),
                Op::Quote(c) => Instruction::Quote(self.link_inner(c, depth + 1)?),
                Op::Tail(c) => Instruction::Tail(self.link_inner(c, depth + 1)?),
                Op::Branch(n) => Instruction::Branch(n as usize),
                Op::ZeroBranch(n) => Instruction::ZeroBranch(n as usize),
                Op::Recur => Instruction::Recur,
                Op::TailRecur => Instruction::TailRecur,
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
        // Each return frame also records its caller's scratch and mark bases.
        let mut returns: Vec<(usize, usize, usize, usize)> = Vec::new();
        let mut base = 0usize;
        let mut mbase = 0usize;
        self.scratch.clear();
        self.marks.clear();
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
                    self.marks.truncate(mbase);
                    match returns.pop() {
                        Some((w, i, b, m)) => {
                            word = w;
                            ip = i;
                            base = b;
                            mbase = m;
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
                    returns.push((word, ip, base, mbase));
                    base = self.scratch.len();
                    mbase = self.marks.len();
                    word = w;
                    ip = 0;
                }
                Instruction::Recur => {
                    if returns.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    returns.push((word, ip, base, mbase));
                    base = self.scratch.len();
                    mbase = self.marks.len();
                    ip = 0;
                }
                Instruction::TailRecur => {
                    // As a tail call to itself: its scratch values and marks go.
                    self.scratch.truncate(base);
                    self.marks.truncate(mbase);
                    ip = 0;
                }
                Instruction::Tail(w) => {
                    // The current word is finished: its scratch values and marks go.
                    self.scratch.truncate(base);
                    self.marks.truncate(mbase);
                    word = w;
                    ip = 0;
                }
                Instruction::Prim(Primitive::Execute) => {
                    let xt = self.pop()?;
                    let w = self.token(xt)?;
                    if returns.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    returns.push((word, ip, base, mbase));
                    base = self.scratch.len();
                    mbase = self.marks.len();
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
                Instruction::Prim(Primitive::ScratchAt) => {
                    let n = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                    let i = self.scratch.len().checked_sub(n + 1).ok_or(Error::Stack)?;
                    if i < base {
                        return Err(Error::Stack);
                    }
                    let v = self.scratch[i];
                    self.push(v)?;
                }
                Instruction::Prim(Primitive::MarkPick) => {
                    let m = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                    let j = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                    let i = self.marks.len().checked_sub(m + 1).ok_or(Error::Stack)?;
                    if i < mbase {
                        return Err(Error::Stack);
                    }
                    let depth = self.marks[i] as usize;
                    let at = depth.checked_sub(j + 1).ok_or(Error::Stack)?;
                    let v = *self.stack.get(at).ok_or(Error::Stack)?;
                    self.push(v)?;
                }
                Instruction::Prim(Primitive::Mark) => {
                    if self.marks.len() >= 65536 {
                        return Err(Error::Stack);
                    }
                    self.marks.push(self.stack.len() as u64);
                }
                Instruction::Prim(Primitive::MapGather) => {
                    if self.marks.len() <= mbase {
                        return Err(Error::Stack);
                    }
                    let mark = self.marks.pop().ok_or(Error::Stack)? as usize;
                    if mark > self.stack.len() || !(self.stack.len() - mark).is_multiple_of(2) {
                        return Err(Error::Stack);
                    }
                    let cells = self.stack.split_off(mark);
                    let mut m = Map::new();
                    for pair in cells.chunks_exact(2) {
                        m.insert(pair[0], pair[1]);
                    }
                    let r = self.allocate_map(m)?;
                    self.push(r)?;
                }
                Instruction::Prim(Primitive::Gather) => {
                    if self.marks.len() <= mbase {
                        return Err(Error::Stack);
                    }
                    let mark = self.marks.pop().ok_or(Error::Stack)? as usize;
                    if mark > self.stack.len() {
                        return Err(Error::Stack);
                    }
                    let cells = self.stack.split_off(mark);
                    let r = self.allocate_vector(cells.into_iter().collect())?;
                    self.push(r)?;
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
            FAdd | FSub | FMul | FDiv | FEq | FLt => {
                let b = f64::from_bits(self.pop()?);
                let a = f64::from_bits(self.pop()?);
                let n = match p {
                    FAdd => float_bits(a + b),
                    FSub => float_bits(a - b),
                    FMul => float_bits(a * b),
                    FDiv => float_bits(a / b),
                    FEq => (a == b) as u64,
                    FLt => (a < b) as u64,
                    _ => unreachable!(),
                };
                self.push(n)?;
            }
            IDivMod => {
                let b = self.pop()? as i64;
                let a = self.pop()? as i64;
                let (q, r) = floored(a, b).ok_or(Error::Arithmetic)?;
                self.push(q as u64)?;
                self.push(r as u64)?;
            }
            FFloor | FCeil | FRound => {
                let a = f64::from_bits(self.pop()?);
                let r = match p {
                    FFloor => a.floor(),
                    FCeil => a.ceil(),
                    _ => a.round(),
                };
                self.push(float_bits(r))?;
            }
            FSqrt => {
                let a = f64::from_bits(self.pop()?);
                self.push(float_bits(a.sqrt()))?;
            }
            FPow => {
                let b = f64::from_bits(self.pop()?);
                let a = f64::from_bits(self.pop()?);
                self.push(float_bits(a.powf(b)))?;
            }
            IPow => {
                let b = self.pop()? as i64;
                let a = self.pop()? as i64;
                let n = u32::try_from(b)
                    .ok()
                    .and_then(|b| a.checked_pow(b))
                    .ok_or(Error::Arithmetic)?;
                self.push(n as u64)?;
            }
            IToF => {
                let a = self.pop()? as i64;
                self.push(float_bits(a as f64))?;
            }
            FToI => {
                let a = f64::from_bits(self.pop()?);
                // Truncate toward zero. NaN, infinities and values outside
                // i64 are errors rather than Rust's saturating conversion.
                if !(a > -9_223_372_036_854_777_856.0 && a < 9_223_372_036_854_775_808.0) {
                    return Err(Error::Arithmetic);
                }
                self.push(a as i64 as u64)?;
            }
            Load8 | Load64 | WorkLoad => {
                let (r, o) = if p == WorkLoad {
                    (WORKING, self.pop()?)
                } else {
                    self.address()?
                };
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
            Store8 | Store64 | WorkStore => {
                let (r, o) = if p == WorkStore {
                    (WORKING, self.pop()?)
                } else {
                    self.address()?
                };
                let value = self.pop()?;
                if p != Store8 && o % 8 != 0 {
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
            // On a string, length counts characters and `at` gives one; on a
            // map, length counts entries and `at` looks a key up.
            VecLen => {
                let r = self.pop()?;
                let n = if let Ok(t) = self.text(r) {
                    t.chars() as usize
                } else if let Ok(m) = self.map(r) {
                    m.len()
                } else {
                    self.array(r)?.len()
                };
                self.push(n as u64)?;
            }
            VecAt => {
                let i = self.pop()?;
                let r = self.pop()?;
                let x = if let Ok(t) = self.text(r) {
                    u64::from(t.char_at(i).ok_or(Error::Memory)?)
                } else if let Ok(m) = self.map(r) {
                    *m.get(&i).ok_or(Error::Memory)?
                } else {
                    let i = usize::try_from(i).map_err(|_| Error::Memory)?;
                    self.array(r)?.get(i).ok_or(Error::Memory)?
                };
                self.push(x)?;
            }
            VecPush => {
                let x = self.pop()?;
                let r = self.pop()?;
                if self.live_bytes + 8 > MAX_BYTES {
                    return Err(Error::Limit);
                }
                let slot = self.region_slot(r)?;
                let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
                region.array.as_mut().ok_or(Error::Memory)?.tail.push(x);
                region.charged += 8;
                self.live_bytes += 8;
                self.push(r)?;
            }
            VecSet => {
                let x = self.pop()?;
                let i = self.pop()?;
                let r = self.pop()?;
                let i = usize::try_from(i).map_err(|_| Error::Memory)?;
                let a = self.array(r)?;
                if i >= a.len() {
                    return Err(Error::Memory);
                }
                let v = a.whole().update(i, x);
                let r = self.allocate_vector(v)?;
                self.push(r)?;
            }
            Text => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let (r, o) = self.address()?;
                let fixed = self
                    .region_slot(r)
                    .ok()
                    .and_then(|s| self.regions[s].region.as_ref())
                    .is_some_and(|g| g.read_only);
                let made = self.data_texts.get(&(r, o, n)).copied();
                if fixed && let Some(t) = made.filter(|&t| self.text(t).is_ok()) {
                    self.push(t)?;
                } else {
                    let s = std::str::from_utf8(self.read(r, o, n)?).map_err(|_| Error::Memory)?;
                    let t = merkle_champ::Sequence::text(s);
                    let t = self.allocate_text(t)?;
                    if fixed {
                        self.data_texts.insert((r, o, n), t);
                    }
                    self.push(t)?;
                }
            }
            Concat => {
                let b = self.pop()?;
                let a = self.pop()?;
                let r = match (self.text(a), self.text(b)) {
                    (Ok(x), Ok(y)) => {
                        let t = x.concat(y);
                        self.allocate_text(t)?
                    }
                    _ => {
                        let v = self.array(a)?.whole().concat(&self.array(b)?.whole());
                        self.allocate_vector(v)?
                    }
                };
                self.push(r)?;
            }
            Slice => {
                let j = self.pop()?;
                let i = self.pop()?;
                let a = self.pop()?;
                if i > j {
                    return Err(Error::Memory);
                }
                let r = match self.text(a) {
                    Ok(t) => {
                        let t = t.char_slice(i..j).ok_or(Error::Memory)?;
                        self.allocate_text(t)?
                    }
                    Err(_) => {
                        let whole = self.array(a)?.whole();
                        let (i, j) = (i as usize, j as usize);
                        if j > whole.len() {
                            return Err(Error::Memory);
                        }
                        self.allocate_vector(whole.slice(i..j))?
                    }
                };
                self.push(r)?;
            }
            // Equal contents, by identity, for two strings or two arrays;
            // for anything else, equal cells.
            Same => {
                let b = self.pop()?;
                let a = self.pop()?;
                let same = match (self.text(a), self.text(b)) {
                    (Ok(x), Ok(y)) => x.identity() == y.identity(),
                    _ => match (self.array(a), self.array(b)) {
                        (Ok(x), Ok(y)) => x.whole().identity() == y.whole().identity(),
                        _ => match (self.map(a), self.map(b)) {
                            (Ok(x), Ok(y)) => x.identity() == y.identity(),
                            _ => a == b,
                        },
                    },
                };
                self.push(same as u64)?;
            }
            // A new version with key k set to v, for a map; with element k
            // replaced, for an array.
            Put => {
                let v = self.pop()?;
                let k = self.pop()?;
                let m = self.pop()?;
                let r = if let Ok(map) = self.map(m) {
                    let map = map.update(k, v);
                    self.allocate_map(map)?
                } else {
                    let a = self.array(m)?;
                    let i = usize::try_from(k).map_err(|_| Error::Memory)?;
                    if i >= a.len() {
                        return Err(Error::Memory);
                    }
                    let s = a.whole().update(i, v);
                    self.allocate_vector(s)?
                };
                self.push(r)?;
            }
            Has => {
                let k = self.pop()?;
                let m = self.pop()?;
                let has = self.map(m)?.contains_key(&k);
                self.push(has as u64)?;
            }
            Remove => {
                let k = self.pop()?;
                let m = self.pop()?;
                let map = self.map(m)?.without(&k);
                let r = self.allocate_map(map)?;
                self.push(r)?;
            }
            Keys | Values => {
                let m = self.pop()?;
                let map = self.map(m)?;
                let cells: merkle_champ::Sequence<u64> = if p == Keys {
                    map.iter().map(|(k, _)| *k).collect()
                } else {
                    map.iter().map(|(_, v)| *v).collect()
                };
                let r = self.allocate_vector(cells)?;
                self.push(r)?;
            }
            // ( r o end b -- k ) The first offset from o before end whose
            // byte is b, is above b, or is at or below b; end if none.
            ByteFind | BytePast | ByteUpto => {
                let b = self.pop()? as u8;
                let end = self.pop()?;
                let o = self.pop()?;
                let r = self.pop()?;
                let n = end.checked_sub(o).ok_or(Error::Memory)?;
                let n = usize::try_from(n).map_err(|_| Error::Memory)?;
                let bytes = self.read(r, o, n)?;
                let i = match p {
                    ByteFind => bytes.iter().position(|&x| x == b),
                    BytePast => bytes.iter().position(|&x| x > b),
                    _ => bytes.iter().position(|&x| x <= b),
                };
                self.push(o + i.unwrap_or(n) as u64)?;
            }
            // ( r o n -- h ) FNV-1a over the span, unmixed.
            ByteHash => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let (r, o) = self.address()?;
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for &b in self.read(r, o, n)? {
                    h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                }
                self.push(h)?;
            }
            // ( r1 o1 r2 o2 n -- flag )
            BytesEq => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let (r2, o2) = self.address()?;
                let (r1, o1) = self.address()?;
                let same = self.read(r1, o1, n)? == self.read(r2, o2, n)?;
                self.push(same as u64)?;
            }
            // ( r o n -- value status ) The span's decimal digits as a number:
            // status 1, or 0 if the span is empty or not all digits, or 2 if
            // the digits do not fit 64 bits.
            Decimal => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let (r, o) = self.address()?;
                let bytes = self.read(r, o, n)?;
                let (value, status) = if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
                    (0, 0)
                } else {
                    bytes
                        .iter()
                        .try_fold(0u64, |v, &b| {
                            v.checked_mul(10)?.checked_add(u64::from(b - b'0'))
                        })
                        .map_or((0, 2), |v| (v, 1))
                };
                self.push(value)?;
                self.push(status)?;
            }
            // ( x t -- s ) A value of checker type t as text: a string as
            // itself, anything else as March writes it.
            TextOf => {
                let t = self.pop()? as u8;
                let x = self.pop()?;
                let mut s = String::new();
                self.format_value(x, t, false, None, &mut s);
                let r = self.allocate_text(merkle_champ::Sequence::text(&s))?;
                self.push(r)?;
            }
            Write => {
                let r = self.pop()?;
                let t = self.text(r)?;
                let bytes: Vec<u8> = t.iter().map(|b| b.0).collect();
                self.output.extend(bytes);
            }
            // ( a b -- n ) -1, 0 or 1 as a is before, equal to or after b, by
            // code point.
            Compare => {
                let b = self.pop()?;
                let a = self.pop()?;
                let order = if a == b {
                    std::cmp::Ordering::Equal
                } else {
                    self.text(a)?
                        .iter()
                        .map(|x| x.0)
                        .cmp(self.text(b)?.iter().map(|x| x.0))
                };
                self.push(order as i64 as u64)?;
            }
            // ( s t -- i ) The index, in characters, of t's first occurrence
            // in s, or -1.
            Search => {
                let t = self.pop()?;
                let s = self.pop()?;
                let hay = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let needle = self.text(t)?.to_text().ok_or(Error::Memory)?;
                let i = hay
                    .find(&needle)
                    .map_or(u64::MAX, |b| hay[..b].chars().count() as u64);
                self.push(i)?;
            }
            // ( s -- n ok ) ( s -- f ok ) A string's number: a decimal integer
            // in i64's range, or a float; ok is 0 if it is not one.
            TextInt | TextFloat => {
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let parsed = if p == TextInt {
                    text.parse::<i64>().ok().map(|n| n as u64)
                } else {
                    text.parse::<f64>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map(float_bits)
                };
                self.push(parsed.unwrap_or(0))?;
                self.push(parsed.is_some() as u64)?;
            }
            // ( … n -- … x ) The cell n below the top, n counted after
            // popping it: 0 pick is dup, 1 pick is over.
            Pick => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                let i = self.stack.len().checked_sub(n + 1).ok_or(Error::Stack)?;
                self.push(self.stack[i])?;
            }
            IAdd | ISub | IMul | IDiv | IMod | ILt => {
                let b = self.pop()? as i64;
                let a = self.pop()? as i64;
                let n = match p {
                    IAdd => a.checked_add(b),
                    ISub => a.checked_sub(b),
                    IMul => a.checked_mul(b),
                    IDiv => a.checked_div(b),
                    IMod => a.checked_rem(b),
                    _ => Some((a < b) as i64),
                };
                self.push(n.ok_or(Error::Arithmetic)? as u64)?;
            }
            Range => {
                let n = self.pop()? as i64;
                if n < 0 {
                    return Err(Error::Arithmetic);
                }
                let size = (n as usize).checked_mul(8).ok_or(Error::Limit)?;
                if size > MAX_BYTES {
                    return Err(Error::Limit);
                }
                let r = self.allocate_vector((1..=n as u64).collect())?;
                self.push(r)?;
            }
            SortInts | SortFloats | SortTexts => {
                let a = self.pop()?;
                let mut cells: Vec<u64> = self.array(a)?.whole().iter().copied().collect();
                match p {
                    SortInts => cells.sort_by_key(|&x| x as i64),
                    SortFloats => {
                        cells.sort_by(|x, y| f64::from_bits(*x).total_cmp(&f64::from_bits(*y)))
                    }
                    _ => {
                        let mut texts = Vec::with_capacity(cells.len());
                        for &c in &cells {
                            texts.push((self.text(c)?.to_text().ok_or(Error::Memory)?, c));
                        }
                        texts.sort();
                        cells = texts.into_iter().map(|t| t.1).collect();
                    }
                }
                let r = self.allocate_vector(cells.into_iter().collect())?;
                self.push(r)?;
            }
            VecInsert => {
                let g = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let x = self.pop()?;
                let a = self.pop()?;
                let whole = self.array(a)?.whole();
                if g > whole.len() {
                    return Err(Error::Memory);
                }
                let one: merkle_champ::Sequence<u64> = std::iter::once(x).collect();
                let v = whole
                    .slice(0..g)
                    .concat(&one)
                    .concat(&whole.slice(g..whole.len()));
                let r = self.allocate_vector(v)?;
                self.push(r)?;
            }
            VecRemove => {
                let i = usize::try_from(self.pop()?).map_err(|_| Error::Memory)?;
                let a = self.pop()?;
                let whole = self.array(a)?.whole();
                if i >= whole.len() {
                    return Err(Error::Memory);
                }
                let v = whole.slice(0..i).concat(&whole.slice(i + 1..whole.len()));
                let r = self.allocate_vector(v)?;
                self.push(r)?;
            }
            GradeInts | GradeFloats | GradeTexts | GradeDownInts | GradeDownFloats
            | GradeDownTexts => {
                let a = self.pop()?;
                let cells: Vec<u64> = self.array(a)?.whole().iter().copied().collect();
                let mut order: Vec<usize> = (0..cells.len()).collect();
                // Stable either way: equal elements keep their order.
                let down = matches!(p, GradeDownInts | GradeDownFloats | GradeDownTexts);
                let way = |o: std::cmp::Ordering| if down { o.reverse() } else { o };
                match p {
                    GradeInts | GradeDownInts => {
                        order.sort_by(|&i, &j| way((cells[i] as i64).cmp(&(cells[j] as i64))))
                    }
                    GradeFloats | GradeDownFloats => order.sort_by(|&i, &j| {
                        way(f64::from_bits(cells[i]).total_cmp(&f64::from_bits(cells[j])))
                    }),
                    _ => {
                        let mut texts = Vec::with_capacity(cells.len());
                        for &c in &cells {
                            texts.push(self.text(c)?.to_text().ok_or(Error::Memory)?);
                        }
                        order.sort_by(|&i, &j| way(texts[i].cmp(&texts[j])));
                    }
                }
                let r = self.allocate_vector(order.into_iter().map(|i| i as u64 + 1).collect())?;
                self.push(r)?;
            }
            Split | Lines | Words => {
                let sep = if p == Split { Some(self.pop()?) } else { None };
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let parts: Vec<String> = match sep {
                    Some(sep) => {
                        let sep = self.text(sep)?.to_text().ok_or(Error::Memory)?;
                        if sep.is_empty() {
                            text.chars().map(String::from).collect()
                        } else {
                            text.split(sep.as_str()).map(String::from).collect()
                        }
                    }
                    None if p == Lines => text.lines().map(String::from).collect(),
                    None => text.split_whitespace().map(String::from).collect(),
                };
                let mut cells = Vec::with_capacity(parts.len());
                for part in parts {
                    cells.push(self.allocate_text(merkle_champ::Sequence::text(&part))?);
                }
                let r = self.allocate_vector(cells.into_iter().collect())?;
                self.push(r)?;
            }
            Lower | Upper => {
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let out = if p == Lower {
                    text.to_lowercase()
                } else {
                    text.to_uppercase()
                };
                let r = self.allocate_text(merkle_champ::Sequence::text(&out))?;
                self.push(r)?;
            }
            ParseInt | ParseFloat => {
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let t = text.trim();
                let parsed = if p == ParseInt {
                    t.parse::<i64>().ok().map(|n| n as u64)
                } else {
                    t.parse::<f64>().ok().map(f64::to_bits)
                };
                let (x, ok) = parsed.map_or((0, 0), |x| (x, 1));
                self.push(x)?;
                self.push(ok)?;
            }
            Keep => {
                let m = self.pop()?;
                let a = self.pop()?;
                let mask = self.array(m)?.whole();
                let whole = self.array(a)?.whole();
                if mask.len() != whole.len() {
                    return Err(Error::User(4));
                }
                let kept: merkle_champ::Sequence<u64> = whole
                    .iter()
                    .zip(mask.iter())
                    .filter(|(_, f)| **f != 0)
                    .map(|(&x, _)| x)
                    .collect();
                let r = self.allocate_vector(kept)?;
                self.push(r)?;
            }
            UnionMake => {
                let t = self.pop()?;
                let x = self.pop()?;
                let r = self.allocate_vector([t, x].into_iter().collect())?;
                self.push(r)?;
            }
            UnionTag | UnionValue => {
                let u = self.pop()?;
                let k = if p == UnionTag { 0 } else { 1 };
                let x = self.array(u)?.get(k).ok_or(Error::Memory)?;
                self.push(x)?;
            }
            Reverse => {
                let a = self.pop()?;
                let whole = self.array(a)?.whole();
                let mut cells: Vec<u64> = whole.iter().copied().collect();
                cells.reverse();
                let r = self.allocate_vector(cells.into_iter().collect())?;
                self.push(r)?;
            }
            IntText | FloatText | MoneyText | StringShow => {
                let x = self.pop()?;
                let s = match p {
                    IntText => (x as i64).to_string(),
                    FloatText => format!("{:?}", f64::from_bits(x)),
                    MoneyText => crate::prims::show_dec((x as i64).into(), 2),
                    _ => {
                        let text = self.text(x)?.to_text().ok_or(Error::Memory)?;
                        let mut out = String::new();
                        crate::show::quoted(&text, &mut out);
                        out
                    }
                };
                let r = self.allocate_text(merkle_champ::Sequence::text(&s))?;
                self.push(r)?;
            }
            Execute | ScratchPush | ScratchPop | ScratchPeek | ScratchAt | MarkPick | Mark
            | Gather | MapGather => {
                unreachable!()
            }
        }
        Ok(())
    }
}

/// Floored division: the quotient rounded down and the remainder with the
/// divisor's sign; none on division by zero or overflow.
pub fn floored(a: i64, b: i64) -> Option<(i64, i64)> {
    let (mut q, mut r) = (a.checked_div(b)?, a.checked_rem(b)?);
    if r != 0 && (r < 0) != (b < 0) {
        q -= 1;
        r += b;
    }
    Some((q, r))
}

/// The bits of `x`, with every NaN mapped to one canonical pattern.
fn float_bits(x: f64) -> u64 {
    if x.is_nan() {
        f64::NAN.to_bits()
    } else {
        x.to_bits()
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
