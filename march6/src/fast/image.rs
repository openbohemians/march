//! Versioned code/dictionary images, not snapshots of live execution state.
//! Dependencies are addressed by CID and precede their users. A bounded,
//! iterative walk omits unreachable historical definitions and all caches.

use super::definition::{self, Definition, Item};
use super::{Cid, Error, Literal, Program, WordId, put, text_bytes};
use std::collections::HashSet;

const MAGIC: &[u8; 8] = b"MARCHF05";
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_WORDS: usize = 100_000;
const MAX_OPS: usize = 1_000_000;

fn invalid(message: &str) -> Error {
    Error::Image(message.into())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        if count > self.remaining() {
            return Err(invalid("truncated image"));
        }
        let start = self.at;
        self.at += count;
        Ok(&self.bytes[start..self.at])
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn number(&mut self) -> Result<usize, Error> {
        usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
            .map_err(|_| invalid("integer exceeds platform range"))
    }
    fn count(&mut self, maximum: usize, minimum_bytes: usize) -> Result<usize, Error> {
        let n = self.number()?;
        if n > maximum || n > self.remaining() / minimum_bytes {
            return Err(invalid("image count or size limit"));
        }
        Ok(n)
    }
    fn cid(&mut self) -> Result<Cid, Error> {
        Ok(Cid(self.take(32)?.try_into().unwrap()))
    }
    fn word(&mut self, program: &Program) -> Result<WordId, Error> {
        program
            .identities
            .get(&self.cid()?)
            .copied()
            .ok_or_else(|| invalid("unknown or forward word CID"))
    }
    fn string(&mut self) -> Result<String, Error> {
        let n = self.count(MAX_BYTES, 1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| invalid("invalid UTF-8"))
    }
    fn literal(&mut self, program: &mut Program) -> Result<Literal, Error> {
        Ok(match self.byte()? {
            4 => Literal::Text(program.intern_text(&self.string()?)?),
            0 => Literal::Int(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            1 => Literal::Bool(match self.byte()? {
                0 => false,
                1 => true,
                _ => return Err(invalid("invalid Boolean")),
            }),
            2 => Literal::Unit,
            3 => Literal::Quote(self.word(program)?),
            _ => return Err(invalid("unknown literal tag")),
        })
    }
    fn definition(&mut self, program: &mut Program) -> Result<Definition, Error> {
        Ok(match self.byte()? {
            0 => Definition::Primitive(self.string()?),
            1 => Definition::Kernel(super::stream::Primitive::decode(self.byte()?)?),
            2 => {
                let n = self.count(MAX_OPS, 1)?;
                let items = (0..n)
                    .map(|_| {
                        Ok(match self.byte()? {
                            6 => Item::Tuple(self.number()?),
                            7 => Item::Untuple(self.number()?),
                            0 => Item::Word(self.word(program)?),
                            1 => Item::Literal(self.literal(program)?),
                            2 => Item::Context(self.string()?),
                            3 => Item::StaticCall,
                            4 => Item::Apply {
                                inputs: self.number()?,
                                outputs: self.number()?,
                            },
                            5 => Item::Recur {
                                inputs: self.number()?,
                                outputs: self.number()?,
                            },
                            _ => return Err(invalid("unknown definition item")),
                        })
                    })
                    .collect::<Result<_, Error>>()?;
                Definition::Sequence(items)
            }
            3 => {
                let n = self.count(MAX_WORDS, 64)?;
                Definition::Family(
                    (0..n)
                        .map(|_| Ok((self.word(program)?, self.word(program)?)))
                        .collect::<Result<_, Error>>()?,
                )
            }
            _ => return Err(invalid("unknown definition kind")),
        })
    }
}

impl Program {
    fn image_dependencies(&self, id: WordId) -> Result<Vec<WordId>, Error> {
        let definition = self
            .definition(id)?
            .ok_or_else(|| invalid("graph-only evaluator fixtures cannot be saved"))?;
        if self.encode_definition(definition)?.len() > MAX_BYTES {
            return Err(invalid("definition size limit"));
        }
        let mut deps = definition.dependencies();
        deps.sort_unstable_by_key(|&id| self.words[id].cid);
        deps.dedup();
        Ok(deps)
    }

    fn image_order(&self, entry: WordId) -> Result<Vec<WordId>, Error> {
        self.word(entry)?;
        let mut roots: Vec<_> = self.names.values().copied().chain([entry]).collect();
        roots.sort_unstable_by_key(|&id| self.words[id].cid);
        roots.dedup();
        let mut todo: Vec<_> = roots.into_iter().rev().map(|id| (id, false)).collect();
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        let mut operations = 0usize;
        while let Some((id, finished)) = todo.pop() {
            if seen.contains(&id) {
                continue;
            }
            if finished {
                seen.insert(id);
                order.push(id);
                operations = operations
                    .checked_add(self.word(id)?.ops.len())
                    .ok_or_else(|| invalid("operation limit"))?;
                if order.len() > MAX_WORDS || operations > MAX_OPS {
                    return Err(invalid("image word or aggregate operation limit"));
                }
            } else {
                todo.push((id, true));
                todo.extend(
                    self.image_dependencies(id)?
                        .into_iter()
                        .rev()
                        .map(|id| (id, false)),
                );
            }
        }
        Ok(order)
    }

    /// Save reachable code and the dictionary. Runtime context, demand cells,
    /// memoized results, and compiled execution plans are deliberately excluded.
    pub fn to_image(&self, entry: WordId) -> Result<Vec<u8>, Error> {
        let order = self.image_order(entry)?;
        if self.names.len() > MAX_WORDS {
            return Err(invalid("dictionary size limit"));
        }
        let mut out = MAGIC.to_vec();
        put(&mut out, order.len());
        for id in order {
            let word = self.word(id)?;
            let bytes = self.encode_definition(
                word.definition
                    .as_ref()
                    .ok_or_else(|| invalid("graph-only evaluator fixtures cannot be saved"))?,
            )?;
            if out.len().saturating_add(40).saturating_add(bytes.len()) > MAX_BYTES {
                return Err(invalid("image size limit"));
            }
            out.extend_from_slice(&word.cid.0);
            put(&mut out, bytes.len());
            out.extend_from_slice(&bytes);
        }
        put(&mut out, self.names.len());
        for (name, &id) in &self.names {
            if out.len().saturating_add(40).saturating_add(name.len()) > MAX_BYTES {
                return Err(invalid("image size limit"));
            }
            text_bytes(&mut out, name);
            out.extend_from_slice(&self.cid(id)?.0);
        }
        {
            put(&mut out, self.immediate.len());
            for name in &self.immediate {
                if out.len().saturating_add(8).saturating_add(name.len()) > MAX_BYTES {
                    return Err(invalid("image size limit"));
                }
                text_bytes(&mut out, name);
            }
        }
        out.extend_from_slice(&self.cid(entry)?.0);
        if out.len() > MAX_BYTES {
            return Err(invalid("image size limit"));
        }
        Ok(out)
    }

    pub fn image_cid(&self, entry: WordId) -> Result<Cid, Error> {
        Ok(Cid::digest(b"march-fast-image-v1", &self.to_image(entry)?))
    }

    /// Load bounded, validated code; this does not execute it. All word CIDs are
    /// recomputed, and dependencies may refer only to earlier image records.
    pub fn from_image(bytes: &[u8]) -> Result<(Self, WordId), Error> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("image size limit"));
        }
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(invalid("image magic/version: only MARCHF05 is supported"));
        }
        let count = reader.count(MAX_WORDS, 40)?;
        let mut program = Self::new();
        let mut operations = 0usize;
        for _ in 0..count {
            let expected = reader.cid()?;
            if program.identities.contains_key(&expected) {
                return Err(invalid("duplicate word CID"));
            }
            let length = reader.count(MAX_BYTES, 1)?;
            let canonical = reader.take(length)?;
            if Cid::digest(definition::DOMAIN, canonical) != expected {
                return Err(invalid("word CID mismatch"));
            }
            let mut word = Reader::new(canonical);
            let definition = word.definition(&mut program)?;
            if word.remaining() != 0 {
                return Err(invalid("trailing definition bytes"));
            }
            let id = program.add_definition(definition)?;
            operations = operations
                .checked_add(program.word(id)?.ops.len())
                .ok_or_else(|| invalid("operation limit"))?;
            if operations > MAX_OPS {
                return Err(invalid("aggregate operation limit"));
            }
            if program.cid(id)? != expected {
                return Err(invalid("noncanonical definition"));
            }
        }
        let count = reader.count(MAX_WORDS, 40)?;
        let mut previous = None;
        for _ in 0..count {
            let name = reader.string()?;
            if previous.as_ref().is_some_and(|p| p >= &name) {
                return Err(invalid("duplicate or unordered dictionary name"));
            }
            let id = reader.word(&program)?;
            program.bind(&name, id)?;
            previous = Some(name);
        }
        {
            let count = reader.count(MAX_WORDS, 8)?;
            let mut previous = None;
            for _ in 0..count {
                let name = reader.string()?;
                if previous.as_ref().is_some_and(|p| p >= &name) {
                    return Err(invalid("duplicate or unordered immediate name"));
                }
                program.mark_immediate(&name)?;
                previous = Some(name);
            }
        }
        let entry = reader.word(&program)?;
        if reader.remaining() != 0 {
            return Err(invalid("trailing image bytes"));
        }
        // This also rejects extraneous unreachable records and alternative
        // dependency orderings: one code/dictionary image has one encoding.
        if program.to_image(entry)? != bytes {
            return Err(invalid("noncanonical image ordering or unreachable word"));
        }
        Ok((program, entry))
    }
}
