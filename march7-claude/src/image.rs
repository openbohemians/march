use crate::code::{MAX_BYTES, MAX_OBJECTS, Reader};
use crate::{Blob, Cid, Error};
use std::collections::BTreeMap;

/// Generic blobs and two roots. The host does not interpret the data root.
#[derive(Clone, Debug)]
pub struct Image {
    pub entry: Cid,
    pub data: Cid,
    pub blobs: BTreeMap<Cid, Blob>,
}
impl Image {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut b = b"MARCH7\0\x01".to_vec();
        b.extend(self.entry);
        b.extend(self.data);
        b.extend((self.blobs.len() as u32).to_le_bytes());
        for (cid, blob) in &self.blobs {
            b.extend(cid);
            b.push(if matches!(blob, Blob::Code(_)) { 0 } else { 1 });
            b.extend((blob.bytes().len() as u32).to_le_bytes());
            b.extend(blob.bytes());
            if b.len() > MAX_BYTES {
                return Err(Error::Limit);
            }
        }
        Ok(b)
    }
    pub fn decode(b: &[u8]) -> Result<Self, Error> {
        if b.len() > MAX_BYTES {
            return Err(Error::Limit);
        }
        let mut r = Reader(b);
        if r.take(8)? != b"MARCH7\0\x01" {
            return Err(Error::InvalidCode);
        }
        let entry = r.array()?;
        let data = r.array()?;
        let n = r.u32()? as usize;
        if n > MAX_OBJECTS {
            return Err(Error::Limit);
        }
        let mut blobs = BTreeMap::new();
        for _ in 0..n {
            let cid = r.array()?;
            let kind = r.byte()?;
            let len = r.u32()? as usize;
            let bytes = r.take(len)?.to_vec();
            let blob = match kind {
                0 => Blob::Code(bytes),
                1 => Blob::Data(bytes),
                _ => return Err(Error::InvalidCode),
            };
            if blob.cid() != cid || blobs.insert(cid, blob).is_some() {
                return Err(Error::InvalidCode);
            }
        }
        if !r.0.is_empty() {
            return Err(Error::InvalidCode);
        }
        let image = Self { entry, data, blobs };
        image.validate()?;
        Ok(image)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.blobs.len() > MAX_OBJECTS {
            return Err(Error::Limit);
        }
        if !matches!(self.blobs.get(&self.entry), Some(Blob::Code(_)))
            || !matches!(self.blobs.get(&self.data), Some(Blob::Data(_)))
        {
            return Err(Error::InvalidCode);
        }
        let mut total = 0usize;
        for (cid, blob) in &self.blobs {
            total = total
                .checked_add(blob.bytes().len() + 37)
                .ok_or(Error::Limit)?;
            if total > MAX_BYTES || blob.cid() != *cid {
                return Err(Error::InvalidCode);
            }
            if let Blob::Code(b) = blob {
                for op in crate::code::decode(b)? {
                    match op {
                        crate::Op::Call(c) | crate::Op::Quote(c) | crate::Op::Tail(c) => {
                            if !matches!(self.blobs.get(&c), Some(Blob::Code(_))) {
                                return Err(Error::InvalidCode);
                            }
                        }
                        crate::Op::Data(c) => {
                            if !matches!(self.blobs.get(&c), Some(Blob::Data(_))) {
                                return Err(Error::InvalidCode);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}
