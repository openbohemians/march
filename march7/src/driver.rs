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
