use sha2::{Digest, Sha256};
use std::fmt;

/// A content identifier for canonical March objects.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cid(pub [u8; 32]);

impl Cid {
    pub fn digest(domain: &[u8], canonical: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update((domain.len() as u64).to_be_bytes());
        hash.update(domain);
        hash.update((canonical.len() as u64).to_be_bytes());
        hash.update(canonical);
        Self(hash.finalize().into())
    }

    pub fn short(self) -> String {
        self.to_string()[..12].to_owned()
    }
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cid({})", self.short())
    }
}

pub(crate) fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    put_u64(out, value.len() as u64);
    out.extend_from_slice(value);
}

pub(crate) fn put_str(out: &mut Vec<u8>, value: &str) {
    put_bytes(out, value.as_bytes());
}

pub(crate) fn put_cid(out: &mut Vec<u8>, value: Cid) {
    out.extend_from_slice(&value.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_separate_otherwise_identical_objects() {
        assert_ne!(Cid::digest(b"node", b"x"), Cid::digest(b"context", b"x"));
    }
}
