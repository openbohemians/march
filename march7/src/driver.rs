use crate::{Blob, Error, Image, Machine};

/// Adapter to the seed's explicit boot protocol, not a source compiler.
/// Boot returns three execution tokens, evaluate, recover and export
/// metadata, and a rebuilt system a fourth, describe, which reports the types
/// March knows for the values on the stack.
pub struct Driver {
    pub machine: Machine,
    entry: crate::Cid,
    eval: u64,
    recover: u64,
    export: u64,
    describe: Option<u64>,
    pub fuel: u64,
}

/// Elements shown of an array, before the count of the rest.
const SHOWN: usize = 16;

impl Driver {
    pub fn boot(image: &Image) -> Result<Self, Error> {
        let mut machine = Machine::new(image)?;
        let entry = machine.link(image.entry)?;
        let data = machine.blob_region(image.data)?;
        let n = image
            .blobs
            .get(&image.data)
            .ok_or(Error::InvalidCode)?
            .bytes()
            .len();
        machine.stack.extend([data, 0, n as u64]);
        machine.run(entry, 10_000_000)?;
        let describe = match machine.stack.len() {
            4 => Some(machine.stack.pop().ok_or(Error::Stack)?),
            3 => None,
            _ => return Err(Error::Stack),
        };
        let export = machine.stack.pop().ok_or(Error::Stack)?;
        let recover = machine.stack.pop().ok_or(Error::Stack)?;
        let eval = machine.stack.pop().ok_or(Error::Stack)?;
        machine.cid(export)?;
        machine.cid(recover)?;
        machine.cid(eval)?;
        if let Some(d) = describe {
            machine.cid(d)?;
        }
        Ok(Self {
            machine,
            entry: image.entry,
            eval,
            recover,
            export,
            describe,
            fuel: 10_000_000,
        })
    }
    /// The types March knows for the values on the stack, a byte each (docs/
    /// CHECKER.md), the deepest first: 0 where it knows none, and everywhere
    /// for a system without `describe`.
    pub fn types(&mut self) -> Result<Vec<u8>, Error> {
        let n = self.machine.stack.len();
        let mut types = vec![0; n];
        let Some(describe) = self.describe else {
            return Ok(types);
        };
        let before = self.machine.stack.clone();
        let result = (|| {
            self.machine.run(describe, self.fuel)?;
            if self.machine.stack.len() != before.len() + 3 {
                return Err(Error::Stack);
            }
            let k = self.machine.stack.pop().ok_or(Error::Stack)? as usize;
            let off = self.machine.stack.pop().ok_or(Error::Stack)?;
            let r = self.machine.stack.pop().ok_or(Error::Stack)?;
            Ok(self.machine.read(r, off, k)?.to_vec())
        })();
        self.machine.stack = before;
        // March's type stack covers the top of the data stack.
        let known = result?;
        let k = known.len().min(n);
        types[n - k..].copy_from_slice(&known[known.len() - k..]);
        Ok(types)
    }
    /// The stack as March source writes values, after its depth: integers
    /// signed, floats with a point, arrays as `( … )`, strings as `"…"`.
    /// Values whose types March does not know are shown as integers.
    pub fn show(&mut self) -> String {
        let n = self.machine.stack.len();
        let types = self.types().unwrap_or_else(|_| vec![0; n]);
        let mut out = format!("<{n}>");
        for (&v, t) in self.machine.stack.iter().zip(types) {
            out.push(' ');
            self.show_value(v, t, &mut out);
        }
        out
    }
    fn show_value(&self, v: u64, t: u8, out: &mut String) {
        self.machine.format_value(v, t, true, Some(SHOWN), out);
    }
    pub fn evaluate(&mut self, source: &str) -> Result<(), Error> {
        let before = self.machine.stack.clone();
        let input = self.machine.input(source.as_bytes())?;
        self.machine.stack.extend([input, source.len() as u64]);
        let result = self.machine.run(self.eval, self.fuel);
        let mut recovery = Ok(());
        if result.is_err() {
            // The host owns no dictionary policy. March's recovery word abandons
            // only its unfinished definition. External effects are not undone.
            self.machine.stack = before;
            recovery = self.machine.run(self.recover, 100_000);
        }
        self.machine.release(input)?;
        recovery?;
        result
    }
    /// Export a fresh system image whose entry is the code token `xt`, with an
    /// empty data root. It contains exactly the blobs reachable from the entry
    /// through code operands; the host never interprets March data blobs. The
    /// token comes from March (for example `' boot` left on the stack), so the
    /// host performs no name lookup.
    pub fn system_image(&self, xt: u64) -> Result<Image, Error> {
        let entry = self.machine.cid(xt)?;
        let mut blobs = std::collections::BTreeMap::new();
        let mut todo = vec![entry];
        while let Some(cid) = todo.pop() {
            if blobs.contains_key(&cid) {
                continue;
            }
            let blob = self
                .machine
                .blobs
                .get(&cid)
                .ok_or(Error::InvalidCode)?
                .clone();
            if let Blob::Code(bytes) = &blob {
                for op in crate::code::decode(bytes)? {
                    if let crate::Op::Call(c)
                    | crate::Op::Quote(c)
                    | crate::Op::Tail(c)
                    | crate::Op::Data(c) = op
                    {
                        todo.push(c);
                    }
                }
            }
            blobs.insert(cid, blob);
        }
        let empty = Blob::Data(Vec::new());
        let data = empty.cid();
        blobs.insert(data, empty);
        let image = Image { entry, data, blobs };
        image.validate()?;
        Ok(image)
    }
    pub fn snapshot(&mut self) -> Result<Image, Error> {
        let before = self.machine.stack.clone();
        let result = (|| {
            self.machine.run(self.export, self.fuel)?;
            if self.machine.stack.len() != before.len() + 3 {
                return Err(Error::Stack);
            }
            let n = self.machine.stack.pop().ok_or(Error::Stack)? as usize;
            let off = self.machine.stack.pop().ok_or(Error::Stack)?;
            let r = self.machine.stack.pop().ok_or(Error::Stack)?;
            let b = self.machine.read(r, off, n)?.to_vec();
            self.machine.release(r)?;
            let data = self.machine.publish(Blob::Data(b))?;
            Ok(Image {
                entry: self.entry,
                data,
                blobs: self.machine.blobs.clone(),
            })
        })();
        self.machine.stack = before;
        result
    }
}
