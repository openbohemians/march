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
    /// A failure, a program's or the core's, by its kind, with its message,
    /// empty if it has none (doc/design/SYMBOLIC-FORTH.md 29).
    Trap(String, String),
    /// Input or output failed, and why.
    Io(String),
}
/// A trap for what is not there, an element or a key.
fn missing(why: String) -> Error {
    Error::Trap("mismatch".into(), why)
}

fn beyond(len: usize) -> String {
    format!("the array of {len} has no such element")
}

/// What a version of a sequence adds, made from another by an edit with
/// `seams` seams, as merkle-champ makes one: at each seam, the leaves it
/// chunks again, a leaf being about a kilobyte, and a branch for each level
/// above them, of about 32 children. The rest it shares. Never more than
/// `whole`, what it would be charged on its own (Thomas, 2026-10-25).
fn edited(seams: usize, height: usize, whole: usize) -> usize {
    const LEAF: usize = 1024;
    const BRANCH: usize = 32 * 8;
    whole.min(seams.saturating_mul(LEAF + BRANCH * (height + 1)))
}

impl Error {
    /// A failure as a trap, its kind and its message: a program's, or the
    /// machine's, overflow or a file not read. Not the fuel limit, the
    /// session's, nor a fault of the machine's.
    pub fn trap(&self) -> Option<(String, String)> {
        match self {
            Error::Trap(kind, why) => Some((kind.clone(), why.clone())),
            Error::Arithmetic => Some((
                "arithmetic".into(),
                "arithmetic overflow, or division by zero".into(),
            )),
            Error::Io(why) => Some(("io".into(), why.clone())),
            _ => None,
        }
    }
}

/// Where a run goes on: the word, the next operation, and the bases of its
/// frame's scratch values and marks.
#[derive(Clone, Copy)]
struct Resume {
    word: usize,
    ip: usize,
    base: usize,
    mbase: usize,
}

/// A handler, `handle`: where the run goes on when a trap comes, how deep
/// each stack was, and the cells it keeps.
struct Handler {
    at: Resume,
    returns: usize,
    scratch: usize,
    marks: usize,
    depth: usize,
    inputs: Vec<u64>,
    /// Which of the inputs are counted values, held while it waits.
    held: u64,
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
    /// How many hold it: cells on the stacks, the containers it is in, the
    /// handlers that keep it. A value, a string, an array or a map, is
    /// freed when none is left (doc/design/REPRESENTATIONS.md 4); raw bytes
    /// only by `region-free`.
    holders: u32,
    /// Which of its cells are counted values, as `holds` sets them: for an
    /// array, bit i its element i, and every bit every element; for a map,
    /// 1 its keys and 2 its values.
    holds: u64,
}

impl Region {
    /// Whether it is a value, counted, rather than raw bytes.
    fn is_value(&self) -> bool {
        self.array.is_some() || self.text.is_some() || self.map.is_some()
    }
}

/// Whether element i of a container is a counted value, by its mask.
fn held(holds: u64, i: usize) -> bool {
    holds == u64::MAX || (i < 64 && holds >> i & 1 == 1)
}

/// The mask of elements taken from another's, `from` giving, for each new
/// element, the old one it is, or none for one put in that is not counted.
fn remask(holds: u64, from: impl Iterator<Item = Option<usize>>) -> u64 {
    if holds == 0 || holds == u64::MAX {
        return holds;
    }
    from.enumerate()
        .take(64)
        .filter(|(_, o)| o.is_some_and(|o| held(holds, o)))
        .fold(0, |m, (i, _)| m | 1 << i)
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
            holders: 1,
            holds: 0,
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
    /// One more holder of `r`, if it is a value: a copy of it, or a
    /// container or a handler that keeps it.
    fn hold(&mut self, r: u64) -> Result<(), Error> {
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        if region.is_value() {
            region.holders = region.holders.checked_add(1).ok_or(Error::Limit)?;
        }
        Ok(())
    }
    /// One holder fewer of `r`, if it is a value, as `drop` has: with none
    /// left it is freed, and each value it holds has one holder fewer.
    fn unhold(&mut self, r: u64) -> Result<(), Error> {
        let mut work = Vec::new();
        let mut next = Some(r);
        while let Some(r) = next.take().or_else(|| work.pop()) {
            let slot = self.region_slot(r)?;
            let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
            if !region.is_value() {
                continue;
            }
            region.holders -= 1;
            if region.holders > 0 {
                continue;
            }
            let holds = region.holds;
            if holds != 0 {
                if let Some(a) = &region.array {
                    for (i, &x) in a.seq.iter().chain(&a.tail).enumerate() {
                        if held(holds, i) {
                            work.push(x);
                        }
                    }
                } else if let Some(m) = &region.map {
                    for (&k, &v) in m.iter() {
                        if holds & 1 != 0 {
                            work.push(k);
                        }
                        if holds & 2 != 0 {
                            work.push(v);
                        }
                    }
                }
            }
            self.release(r)?;
        }
        Ok(())
    }
    /// Whether `r` has no holder but the one about to let it go, so that
    /// what is made of it may take its place.
    fn alone(&self, r: u64) -> Result<bool, Error> {
        Ok(self.region(r)?.holders == 1)
    }
    /// The mask of `r`'s counted cells.
    fn holds_of(&self, r: u64) -> Result<u64, Error> {
        Ok(self.region(r)?.holds)
    }
    /// Holds once more each counted element of an array, but those at the
    /// positions `skip` gives, which are put in rather than shared.
    fn hold_elements(&mut self, r: u64, skip: &[usize]) -> Result<(), Error> {
        let holds = self.holds_of(r)?;
        if holds == 0 {
            return Ok(());
        }
        let cells: Vec<u64> = {
            let a = self.array(r)?;
            a.seq
                .iter()
                .chain(&a.tail)
                .enumerate()
                .filter(|&(i, _)| held(holds, i) && !skip.contains(&i))
                .map(|(_, &x)| x)
                .collect()
        };
        for x in cells {
            self.hold(x)?;
        }
        Ok(())
    }
    /// Holds once more each counted key and value of a map, but the key and
    /// the value `skip` gives, which are put in rather than shared.
    fn hold_entries(&mut self, r: u64, skip: Option<u64>) -> Result<(), Error> {
        let holds = self.holds_of(r)?;
        if holds == 0 {
            return Ok(());
        }
        let mut cells = Vec::new();
        for (&k, &v) in self.map(r)?.iter() {
            if Some(k) == skip {
                continue;
            }
            if holds & 1 != 0 {
                cells.push(k);
            }
            if holds & 2 != 0 {
                cells.push(v);
            }
        }
        for x in cells {
            self.hold(x)?;
        }
        Ok(())
    }
    /// Sets which of a new container's cells are counted values.
    fn set_holds(&mut self, r: u64, holds: u64) -> Result<(), Error> {
        let slot = self.region_slot(r)?;
        self.regions[slot]
            .region
            .as_mut()
            .ok_or(Error::Memory)?
            .holds = holds;
        Ok(())
    }
    /// A new array, `r`'s elements as `from` picks them, sharing with `r`:
    /// each counted element held once more, and `r` held once less.
    fn derived(&mut self, r: u64, cells: Vec<u64>, from: Vec<usize>) -> Result<u64, Error> {
        let holds = self.holds_of(r)?;
        let n = self.allocate_vector(cells.into_iter().collect())?;
        self.set_holds(n, remask(holds, from.into_iter().map(Some)))?;
        self.hold_elements(n, &[])?;
        self.unhold(r)?;
        Ok(n)
    }
    /// Replaces an array alone in its region by an edit of it, charged by
    /// how its length changed.
    fn edit_in_place(
        &mut self,
        r: u64,
        seq: merkle_champ::Sequence<u64>,
        holds: u64,
    ) -> Result<(), Error> {
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        let a = region.array.as_mut().ok_or(Error::Memory)?;
        let was = a.len() * 8;
        let now = seq.len() * 8;
        *a = Array {
            seq,
            tail: Vec::new(),
        };
        region.holds = holds;
        region.charged = (region.charged + now).saturating_sub(was);
        self.live_bytes = (self.live_bytes + now).saturating_sub(was);
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(())
    }
    /// An array with element `i` replaced by `x`: in place if nothing else
    /// holds it, the old element let go of; else a new version sharing the
    /// rest, each counted element of it held once more.
    fn set_element(&mut self, r: u64, i: usize, x: u64) -> Result<u64, Error> {
        let old = self.array(r)?.get(i).ok_or(Error::Memory)?;
        let v = self.whole(r)?.update(i, x);
        let holds = self.holds_of(r)?;
        if self.alone(r)? {
            self.edit_in_place(r, v, holds)?;
            if held(holds, i) {
                self.unhold(old)?;
            }
            return Ok(r);
        }
        let n = self.allocate_version(v, 1)?;
        self.set_holds(n, holds)?;
        self.hold_elements(n, &[i])?;
        self.unhold(r)?;
        Ok(n)
    }
    /// Two arrays joined: in the first's place if nothing else holds it,
    /// else a new version sharing it; the second's elements shared, or taken
    /// over if nothing else holds it.
    fn concat(&mut self, a: u64, b: u64) -> Result<u64, Error> {
        let (x, y) = (self.whole(a)?, self.whole(b)?);
        let (ha, hb) = (self.holds_of(a)?, self.holds_of(b)?);
        let (la, lb) = (x.len(), y.len());
        let mask = if lb == 0 {
            ha
        } else if la == 0 {
            hb
        } else if ha == hb && (ha == 0 || ha == u64::MAX) {
            ha
        } else {
            (0..la + lb)
                .take(64)
                .filter(|&k| {
                    if k < la {
                        held(ha, k)
                    } else {
                        held(hb, k - la)
                    }
                })
                .fold(0, |m, k| m | 1 << k)
        };
        let v = x.concat(&y);
        let r = if a != b && self.alone(a)? {
            self.edit_in_place(a, v, mask)?;
            a
        } else {
            let r = self.allocate_version(v, 1)?;
            self.set_holds(r, mask)?;
            self.hold_elements(a, &[])?;
            self.unhold(a)?;
            r
        };
        if a != b && self.alone(b)? {
            // Its region goes; its elements are the new one's now.
            self.release(b)?;
        } else {
            self.hold_elements(b, &[])?;
            self.unhold(b)?;
        }
        Ok(r)
    }
    /// Replaces a map alone in its region by an edit of it, charged 16
    /// bytes an entry by how many it has.
    fn map_in_place(&mut self, m: u64, map: Map) -> Result<(), Error> {
        let slot = self.region_slot(m)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        let was = region.map.as_ref().ok_or(Error::Memory)?.len() * 16;
        let now = map.len() * 16;
        region.map = Some(map);
        region.charged = (region.charged + now).saturating_sub(was);
        self.live_bytes = (self.live_bytes + now).saturating_sub(was);
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
        Ok(())
    }
    /// A map with key `k` set to `v`: in place if nothing else holds it,
    /// the value replaced let go of, and the key given too if it was there,
    /// the map holding it already; else a new version sharing the rest.
    fn put(&mut self, m: u64, k: u64, v: u64) -> Result<u64, Error> {
        let holds = self.holds_of(m)?;
        let map = self.map(m)?;
        let old = map.get(&k).copied();
        let new = map.update(k, v);
        if self.alone(m)? {
            self.map_in_place(m, new)?;
            if let Some(old) = old {
                if holds & 2 != 0 {
                    self.unhold(old)?;
                }
                if holds & 1 != 0 {
                    self.unhold(k)?;
                }
            }
            return Ok(m);
        }
        let r = self.allocate_map_version(new)?;
        self.set_holds(r, holds)?;
        self.hold_entries(r, Some(k))?;
        self.unhold(m)?;
        Ok(r)
    }
    /// A map without key `k`: in place if nothing else holds it, the entry
    /// let go of; else a new version sharing the rest. The key given is let
    /// go of either way.
    fn remove(&mut self, m: u64, k: u64) -> Result<u64, Error> {
        let holds = self.holds_of(m)?;
        let map = self.map(m)?;
        let old = map.get(&k).copied();
        let new = map.without(&k);
        let r = if self.alone(m)? {
            self.map_in_place(m, new)?;
            if let Some(old) = old {
                if holds & 2 != 0 {
                    self.unhold(old)?;
                }
                if holds & 1 != 0 {
                    self.unhold(k)?;
                }
            }
            m
        } else {
            let r = self.allocate_map_version(new)?;
            self.set_holds(r, holds)?;
            self.hold_entries(r, None)?;
            self.unhold(m)?;
            r
        };
        if holds & 1 != 0 {
            self.unhold(k)?;
        }
        Ok(r)
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
            self.hold(r)?;
            return Ok(r);
        }
        let r = self.allocate_new_text(t)?;
        self.strings.insert(id, r);
        Ok(r)
    }
    /// The handle of a string made from another by an edit with `seams`
    /// seams, a new one charged what it adds (`edited`).
    fn allocate_text_version(&mut self, t: Text, seams: usize) -> Result<u64, Error> {
        let id = t.identity();
        if let Some(&r) = self.strings.get(&id) {
            self.hold(r)?;
            return Ok(r);
        }
        let size = edited(seams, t.height(), t.len());
        let r = self.allocate_text_charged(t, size)?;
        self.strings.insert(id, r);
        Ok(r)
    }
    fn allocate_new_text(&mut self, t: Text) -> Result<u64, Error> {
        let size = t.len();
        self.allocate_text_charged(t, size)
    }
    fn allocate_text_charged(&mut self, t: Text, size: usize) -> Result<u64, Error> {
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

    /// The handle of a string with this text.
    pub fn text_cell(&mut self, s: &str) -> Result<u64, Error> {
        self.allocate_text(merkle_champ::Sequence::text(s))
    }

    /// The handle of an array of these cells, as `Gather` makes one.
    pub fn vector_cell(&mut self, cells: Vec<u64>) -> Result<u64, Error> {
        self.allocate_vector(cells.into_iter().collect())
    }

    /// One more holder of each of these values, as the session's copy of
    /// the stack is.
    pub fn hold_all(&mut self, cells: &[u64]) -> Result<(), Error> {
        for &c in cells {
            self.hold(c)?;
        }
        Ok(())
    }
    /// One holder fewer of each of these values.
    pub fn release_all(&mut self, cells: &[u64]) -> Result<(), Error> {
        for &c in cells {
            self.unhold(c)?;
        }
        Ok(())
    }
    /// What is held now: bytes and regions.
    pub fn live(&self) -> (usize, usize) {
        (
            self.live_bytes,
            self.regions.iter().filter(|s| s.region.is_some()).count(),
        )
    }

    /// Runs primitives on the stack, outside any word: what the stage does
    /// to fold a primitive on known values, so that it has one meaning,
    /// compiled or folded. Only primitives on values: no control, scratch
    /// stack or marks.
    pub fn apply(&mut self, ops: &[Primitive]) -> Result<(), Error> {
        for &p in ops {
            self.primitive(p)?;
        }
        Ok(())
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
        self.allocate_map_charged(m, size)
    }
    /// A new version of a map, a key set or removed, charged what it adds:
    /// the nodes on the key's path, one for each level, each of at most 32
    /// entries; the rest it shares.
    fn allocate_map_version(&mut self, m: Map) -> Result<u64, Error> {
        let mut levels = 1;
        let mut n = m.len();
        while n > 32 {
            n /= 32;
            levels += 1;
        }
        let size = m.len().saturating_mul(16).min(levels * 32 * 16);
        self.allocate_map_charged(m, size)
    }
    fn allocate_map_charged(&mut self, m: Map, size: usize) -> Result<u64, Error> {
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
        self.allocate_array(v, size)
    }
    /// A new version of an array, made from another by an edit with `seams`
    /// seams, charged what it adds (`edited`).
    fn allocate_version(
        &mut self,
        v: merkle_champ::Sequence<u64>,
        seams: usize,
    ) -> Result<u64, Error> {
        let size = edited(seams, v.height(), v.len().saturating_mul(8));
        self.allocate_array(v, size)
    }
    fn allocate_array(
        &mut self,
        v: merkle_champ::Sequence<u64>,
        size: usize,
    ) -> Result<u64, Error> {
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
    /// The elements an array holds, as one sequence, its tail joined to it
    /// for good: however often it is read whole, a tail is joined once.
    fn whole(&mut self, r: u64) -> Result<merkle_champ::Sequence<u64>, Error> {
        let slot = self.region_slot(r)?;
        let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
        let a = region.array.as_mut().ok_or(Error::Memory)?;
        if !a.tail.is_empty() {
            a.seq = a.whole();
            a.tail = Vec::new();
        }
        Ok(a.seq.clone())
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
        let mut at = Resume {
            word: self.token(xt)?,
            ip: 0,
            base: 0,
            mbase: 0,
        };
        let mut returns = Vec::new();
        let mut handlers: Vec<Handler> = Vec::new();
        self.scratch.clear();
        self.marks.clear();
        // A trap goes to the innermost handler, if there is one, and the
        // run goes on there (doc/design/SYMBOLIC-FORTH.md 29).
        loop {
            let e = match self.execute(at, &mut returns, &mut handlers, &mut fuel) {
                Ok(()) => return Ok(()),
                Err(e) => e,
            };
            let (Some((kind, why)), Some(h)) = (e.trap(), handlers.pop()) else {
                return Err(e);
            };
            if self.stack.len() < h.depth {
                return Err(Error::Stack);
            }
            // What the trap throws away is not freed: which of its cells
            // are values is known only while compiling, and a table of it
            // beside the code is to come. The inputs come back, held.
            returns.truncate(h.returns);
            self.scratch.truncate(h.scratch);
            self.marks.truncate(h.marks);
            self.stack.truncate(h.depth);
            self.stack.extend(h.inputs);
            let why = self.text_cell(&why)?;
            let kind = self.text_cell(&kind)?;
            let trap = self.vector_cell(vec![why, kind])?;
            self.set_holds(trap, u64::MAX)?;
            self.push(trap)?;
            self.push(1)?;
            at = h.at;
        }
    }

    /// Runs from where `at` says until the outermost word returns.
    fn execute(
        &mut self,
        at: Resume,
        returns: &mut Vec<(usize, usize, usize, usize)>,
        handlers: &mut Vec<Handler>,
        fuel: &mut u64,
    ) -> Result<(), Error> {
        let Resume {
            mut word,
            mut ip,
            mut base,
            mut mbase,
        } = at;
        loop {
            if *fuel == 0 {
                return Err(Error::Fuel);
            }
            *fuel -= 1;
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
                Instruction::Prim(Primitive::Handle) => {
                    let m = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                    let mask = self.pop()?;
                    let depth = self.stack.len().checked_sub(m).ok_or(Error::Stack)?;
                    if handlers.len() >= 16384 {
                        return Err(Error::Stack);
                    }
                    for i in 0..m {
                        if held(mask, i) {
                            self.hold(self.stack[depth + i])?;
                        }
                    }
                    handlers.push(Handler {
                        at: Resume {
                            word,
                            ip,
                            base,
                            mbase,
                        },
                        returns: returns.len(),
                        scratch: self.scratch.len(),
                        marks: self.marks.len(),
                        depth,
                        inputs: self.stack[depth..].to_vec(),
                        held: mask,
                    });
                    self.push(0)?;
                }
                Instruction::Prim(Primitive::Unhandle) => {
                    let h = handlers.pop().ok_or(Error::Stack)?;
                    for (i, &x) in h.inputs.iter().enumerate() {
                        if held(h.held, i) {
                            self.unhold(x)?;
                        }
                    }
                }
                Instruction::Prim(Primitive::Mark) => {
                    if self.marks.len() >= 65536 {
                        return Err(Error::Stack);
                    }
                    self.marks.push(self.stack.len() as u64);
                }
                Instruction::Prim(Primitive::Reach) => {
                    let n = self.pop()?;
                    if self.marks.len() <= mbase {
                        return Err(Error::Stack);
                    }
                    let mark = self.marks.last_mut().ok_or(Error::Stack)?;
                    *mark = mark.checked_sub(n).ok_or(Error::Stack)?;
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
            // Raw bytes freed; a value let go of, as `release` does.
            RegionFree => {
                let r = self.pop()?;
                let region = self.region(r)?;
                if region.is_value() {
                    self.unhold(r)?;
                } else if region.read_only {
                    return Err(Error::ReadOnly);
                } else {
                    self.release(r)?;
                }
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
            // The kind on top, its message beneath, each text.
            Trap => {
                let kind = self.pop()?;
                let why = self.pop()?;
                let text = |r| self.text(r)?.to_text().ok_or(Error::Memory);
                let e = Error::Trap(text(kind)?, text(why)?);
                self.unhold(kind)?;
                self.unhold(why)?;
                return Err(e);
            }
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
                self.unhold(r)?;
                self.push(n as u64)?;
            }
            // What is not there is a trap of `mismatch'`, as it is while
            // compiling, so a handler can take it. The index is the
            // machine's, from 0, not the one `at` was given.
            VecAt => {
                let i = self.pop()?;
                let r = self.pop()?;
                let holds = self.holds_of(r)?;
                let (x, counted) = if let Ok(t) = self.text(r) {
                    let c = t
                        .char_at(i)
                        .ok_or_else(|| missing("the string has no such character".into()))?;
                    (u64::from(c), false)
                } else if let Ok(m) = self.map(r) {
                    let x = *m
                        .get(&i)
                        .ok_or_else(|| missing("the map has no such key".into()))?;
                    if holds & 1 != 0 {
                        self.unhold(i)?;
                    }
                    (x, holds & 2 != 0)
                } else {
                    let a = self.array(r)?;
                    let k = usize::try_from(i).ok().filter(|&k| k < a.len());
                    let Some(k) = k else {
                        return Err(missing(beyond(a.len())));
                    };
                    (a.get(k).ok_or(Error::Memory)?, held(holds, k))
                };
                if counted {
                    self.hold(x)?;
                }
                self.unhold(r)?;
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
                let a = self.array(r)?;
                let Some(i) = usize::try_from(i).ok().filter(|&k| k < a.len()) else {
                    return Err(missing(beyond(a.len())));
                };
                let r = self.set_element(r, i, x)?;
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
                    self.hold(t)?;
                    self.push(t)?;
                } else {
                    let s = std::str::from_utf8(self.read(r, o, n)?).map_err(|_| Error::Memory)?;
                    let t = merkle_champ::Sequence::text(s);
                    let t = self.allocate_text(t)?;
                    // The cache holds it too, so a literal is never freed.
                    if fixed {
                        self.hold(t)?;
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
                        let r = self.allocate_text_version(t, 1)?;
                        self.unhold(a)?;
                        self.unhold(b)?;
                        r
                    }
                    _ => self.concat(a, b)?,
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
                        let r = self.allocate_text_version(t, 2)?;
                        self.unhold(a)?;
                        r
                    }
                    Err(_) => {
                        let whole = self.whole(a)?;
                        let (i, j) = (i as usize, j as usize);
                        if j > whole.len() {
                            return Err(Error::Memory);
                        }
                        let holds = self.holds_of(a)?;
                        let mask = remask(holds, (i..j).map(Some));
                        if self.alone(a)? {
                            let cut: Vec<u64> = (0..whole.len())
                                .filter(|&k| (k < i || k >= j) && held(holds, k))
                                .filter_map(|k| whole.get(k).copied())
                                .collect();
                            self.edit_in_place(a, whole.slice(i..j), mask)?;
                            for x in cut {
                                self.unhold(x)?;
                            }
                            a
                        } else {
                            let r = self.allocate_version(whole.slice(i..j), 2)?;
                            self.set_holds(r, mask)?;
                            self.hold_elements(r, &[])?;
                            self.unhold(a)?;
                            r
                        }
                    }
                };
                self.push(r)?;
            }
            // Equal contents, by identity, for two strings or two arrays;
            // for anything else, equal cells.
            Same => {
                let b = self.pop()?;
                let a = self.pop()?;
                let texts = self.text(a).is_ok() && self.text(b).is_ok();
                let arrays = self.array(a).is_ok() && self.array(b).is_ok();
                let same = if texts {
                    // Interned, so equal text is one handle.
                    a == b
                } else if arrays {
                    self.whole(a)?.identity() == self.whole(b)?.identity()
                } else {
                    match (self.map(a), self.map(b)) {
                        (Ok(x), Ok(y)) => x.identity() == y.identity(),
                        _ => a == b,
                    }
                };
                for r in [a, b] {
                    if self.region(r).is_ok_and(Region::is_value) {
                        self.unhold(r)?;
                    }
                }
                self.push(same as u64)?;
            }
            // A new version with key k set to v, for a map; with element k
            // replaced, for an array.
            Put => {
                let v = self.pop()?;
                let k = self.pop()?;
                let m = self.pop()?;
                let r = if self.map(m).is_ok() {
                    self.put(m, k, v)?
                } else {
                    let a = self.array(m)?;
                    let i = usize::try_from(k).map_err(|_| Error::Memory)?;
                    if i >= a.len() {
                        return Err(Error::Memory);
                    }
                    self.set_element(m, i, v)?
                };
                self.push(r)?;
            }
            Has => {
                let k = self.pop()?;
                let m = self.pop()?;
                let has = self.map(m)?.contains_key(&k);
                if self.holds_of(m)? & 1 != 0 {
                    self.unhold(k)?;
                }
                self.unhold(m)?;
                self.push(has as u64)?;
            }
            Remove => {
                let k = self.pop()?;
                let m = self.pop()?;
                let r = self.remove(m, k)?;
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
                let holds = self.holds_of(m)?;
                let counted = holds & if p == Keys { 1 } else { 2 } != 0;
                let r = self.allocate_vector(cells)?;
                if counted {
                    self.set_holds(r, u64::MAX)?;
                    self.hold_elements(r, &[])?;
                }
                self.unhold(m)?;
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
                let value = t == STRING
                    || (MAP..=EMPTY_MAP).contains(&t)
                    || (3..=230).contains(&t)
                    || matches!(t, 247 | 252 | 254);
                if value && self.region(x).is_ok_and(Region::is_value) {
                    self.unhold(x)?;
                }
                self.push(r)?;
            }
            Write => {
                let r = self.pop()?;
                let t = self.text(r)?;
                let bytes: Vec<u8> = t.iter().map(|b| b.0).collect();
                self.output.extend(bytes);
                self.unhold(r)?;
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
                self.unhold(a)?;
                self.unhold(b)?;
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
                self.unhold(s)?;
                self.unhold(t)?;
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
                self.unhold(s)?;
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
                let mut cells: Vec<u64> = self.whole(a)?.iter().copied().collect();
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
                // Each of its elements is somewhere in the sorted one, so a
                // mask that is not every element or none is lost.
                let r = self.allocate_vector(cells.into_iter().collect())?;
                if self.holds_of(a)? == u64::MAX {
                    self.set_holds(r, u64::MAX)?;
                    self.hold_elements(r, &[])?;
                }
                self.unhold(a)?;
                self.push(r)?;
            }
            VecInsert => {
                let g = self.pop()?;
                let x = self.pop()?;
                let a = self.pop()?;
                // An append to an array nothing else holds, its elements
                // counted alike, goes onto its tail, in place.
                let holds = self.holds_of(a)?;
                if usize::try_from(g).is_ok_and(|g| g == self.array(a).map_or(0, Array::len))
                    && (holds == 0 || holds == u64::MAX)
                    && self.alone(a)?
                {
                    if self.live_bytes + 8 > MAX_BYTES {
                        return Err(Error::Limit);
                    }
                    let slot = self.region_slot(a)?;
                    let region = self.regions[slot].region.as_mut().ok_or(Error::Memory)?;
                    region.array.as_mut().ok_or(Error::Memory)?.tail.push(x);
                    region.charged += 8;
                    self.live_bytes += 8;
                    self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(self.live_bytes);
                    return self.push(a);
                }
                let whole = self.whole(a)?;
                let Some(g) = usize::try_from(g).ok().filter(|&g| g <= whole.len()) else {
                    return Err(missing(format!(
                        "the array of {} has no such gap",
                        whole.len()
                    )));
                };
                let holds = self.holds_of(a)?;
                let len = whole.len();
                let mask = remask(
                    holds,
                    (0..=len).map(|k| match k.cmp(&g) {
                        std::cmp::Ordering::Less => Some(k),
                        std::cmp::Ordering::Equal => None,
                        std::cmp::Ordering::Greater => Some(k - 1),
                    }),
                );
                let v = whole.insert(g, x);
                let r = if self.alone(a)? {
                    self.edit_in_place(a, v, mask)?;
                    a
                } else {
                    let r = self.allocate_version(v, 1)?;
                    self.set_holds(r, mask)?;
                    self.hold_elements(r, &[g])?;
                    self.unhold(a)?;
                    r
                };
                self.push(r)?;
            }
            VecRemove => {
                let i = self.pop()?;
                let a = self.pop()?;
                let whole = self.whole(a)?;
                let Some(i) = usize::try_from(i).ok().filter(|&i| i < whole.len()) else {
                    return Err(missing(beyond(whole.len())));
                };
                let holds = self.holds_of(a)?;
                let old = whole.get(i).copied().ok_or(Error::Memory)?;
                let mask = remask(
                    holds,
                    (0..whole.len() - 1).map(|k| Some(k + (k >= i) as usize)),
                );
                let v = whole.remove(i);
                let r = if self.alone(a)? {
                    self.edit_in_place(a, v, mask)?;
                    if held(holds, i) {
                        self.unhold(old)?;
                    }
                    a
                } else {
                    let r = self.allocate_version(v, 1)?;
                    self.set_holds(r, mask)?;
                    self.hold_elements(r, &[])?;
                    self.unhold(a)?;
                    r
                };
                self.push(r)?;
            }
            GradeInts | GradeFloats | GradeTexts | GradeDownInts | GradeDownFloats
            | GradeDownTexts => {
                let a = self.pop()?;
                let cells: Vec<u64> = self.whole(a)?.iter().copied().collect();
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
                self.unhold(a)?;
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
                self.set_holds(r, u64::MAX)?;
                self.unhold(s)?;
                if let Some(sep) = sep {
                    self.unhold(sep)?;
                }
                self.push(r)?;
            }
            Snake | Kebab | Camel | Pascal => {
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let style = match p {
                    Snake => crate::prims::Style::Snake,
                    Kebab => crate::prims::Style::Kebab,
                    Camel => crate::prims::Style::Camel,
                    _ => crate::prims::Style::Pascal,
                };
                let out = crate::prims::restyle(&text, style);
                let r = self.allocate_text(merkle_champ::Sequence::text(&out))?;
                self.unhold(s)?;
                self.push(r)?;
            }
            TextJoin => {
                let n = usize::try_from(self.pop()?).map_err(|_| Error::Stack)?;
                let at = self.stack.len().checked_sub(n).ok_or(Error::Stack)?;
                let mut out = String::new();
                for &t in &self.stack[at..] {
                    out.push_str(&self.text(t)?.to_text().ok_or(Error::Memory)?);
                }
                let r = self.allocate_text(merkle_champ::Sequence::text(&out))?;
                for t in self.stack.split_off(at) {
                    self.unhold(t)?;
                }
                self.push(r)?;
            }
            SymShow => {
                let s = self.pop()?;
                let text = self.text(s)?.to_text().ok_or(Error::Memory)?;
                let mut out = String::new();
                crate::show::symbol(&text, &mut out);
                let r = self.allocate_text(merkle_champ::Sequence::text(&out))?;
                self.unhold(s)?;
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
                self.unhold(s)?;
                self.push(r)?;
            }
            ReadFile => {
                let r = self.pop()?;
                let path = self.text(r)?.to_text().ok_or(Error::Memory)?;
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| Error::Io(format!("cannot read {path:?}: {e}")))?;
                let t = self.allocate_text(merkle_champ::Sequence::text(&text))?;
                self.unhold(r)?;
                self.push(t)?;
            }
            ReadStdin => {
                let text = std::io::read_to_string(std::io::stdin())
                    .map_err(|e| Error::Io(format!("cannot read standard input: {e}")))?;
                let t = self.allocate_text(merkle_champ::Sequence::text(&text))?;
                self.push(t)?;
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
                self.unhold(s)?;
                self.push(x)?;
                self.push(ok)?;
            }
            Keep => {
                let m = self.pop()?;
                let a = self.pop()?;
                let mask = self.whole(m)?;
                let whole = self.whole(a)?;
                if mask.len() != whole.len() {
                    return Err(Error::Trap(
                        "mismatch".into(),
                        "`keep`: the array and its mask are not as long".into(),
                    ));
                }
                let from: Vec<usize> = mask
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| **f != 0)
                    .map(|(k, _)| k)
                    .collect();
                let kept: Vec<u64> = from.iter().filter_map(|&k| whole.get(k).copied()).collect();
                let r = self.derived(a, kept, from)?;
                self.unhold(m)?;
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
                if held(self.holds_of(u)?, k) {
                    self.hold(x)?;
                }
                self.unhold(u)?;
                self.push(x)?;
            }
            Reverse => {
                let a = self.pop()?;
                let whole = self.whole(a)?;
                let mut cells: Vec<u64> = whole.iter().copied().collect();
                cells.reverse();
                let n = cells.len();
                let r = self.derived(a, cells, (0..n).rev().collect())?;
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
                if p == StringShow {
                    self.unhold(x)?;
                }
                self.push(r)?;
            }
            Hold => {
                let x = *self.stack.last().ok_or(Error::Stack)?;
                self.hold(x)?;
            }
            Release => {
                let x = self.pop()?;
                self.unhold(x)?;
            }
            Holds => {
                let h = self.pop()?;
                let c = *self.stack.last().ok_or(Error::Stack)?;
                self.set_holds(c, h)?;
            }
            Execute | ScratchPush | ScratchPop | ScratchPeek | ScratchAt | Mark | Gather
            | MapGather | Reach | Handle | Unhandle => {
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
