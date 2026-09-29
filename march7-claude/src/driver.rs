use crate::{Blob, Error, Image, Machine};

/// Adapter to the seed's explicit boot protocol, not a source compiler.
/// Boot returns three execution tokens: evaluate, recover, export metadata.
pub struct Driver {
    pub machine: Machine,
    entry: crate::Cid,
    eval: u64,
    recover: u64,
    export: u64,
    pub fuel: u64,
}
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
        if machine.stack.len() != 3 {
            return Err(Error::Stack);
        }
        let export = machine.stack.pop().ok_or(Error::Stack)?;
        let recover = machine.stack.pop().ok_or(Error::Stack)?;
        let eval = machine.stack.pop().ok_or(Error::Stack)?;
        machine.cid(export)?;
        machine.cid(recover)?;
        machine.cid(eval)?;
        Ok(Self {
            machine,
            entry: image.entry,
            eval,
            recover,
            export,
            fuel: 10_000_000,
        })
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
