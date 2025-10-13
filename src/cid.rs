use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

/// Content Identifier - 32 bytes (256 bits)
///
/// Format:
/// - Bit 7 of byte 0: 0 = content-addressed code (CID), 1 = special
/// - Bits 6-5 of byte 0 (when bit 7 = 1): type tag
///   - 00 (0x80): primitive operation
///   - 01 (0xA0): literal data
///   - 10, 11: reserved
///
/// Examples:
/// - Content-addressed: 0x1A3F... (bit 7 = 0)
/// - Primitive Add:     0x8000 0001 00... (tag=0x80, id=1)
/// - Literal "hello":   0xA0XX XXXX XX... (tag=0xA0, hash with tag)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CID([u8; 32]);

// Type tags
pub const TAG_CID: u8 = 0x00;         // 0b0000_0000 - content-addressed code
pub const TAG_PRIMITIVE: u8 = 0x80;   // 0b1000_0000 - primitive operation
pub const TAG_LITERAL: u8 = 0xA0;     // 0b1010_0000 - literal data

impl CID {
    /// Create a CID for a primitive operation with assigned ID
    pub fn primitive(id: u16) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = TAG_PRIMITIVE;
        bytes[1..3].copy_from_slice(&id.to_be_bytes());
        CID(bytes)
    }

    /// Create a CID for literal data by hashing it
    pub fn from_literal(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hash);
        // Set the literal tag while preserving lower bits
        bytes[0] = (bytes[0] & 0x1F) | TAG_LITERAL;
        CID(bytes)
    }

    /// Create a CID by hashing a sequence of CIDs (for word definitions)
    pub fn from_sequence(cids: &[CID]) -> Self {
        let mut hasher = Sha256::new();
        for cid in cids {
            hasher.update(&cid.0);
        }
        let hash = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hash);
        // Ensure bit 7 is 0 for content-addressed code
        bytes[0] &= 0x7F;
        CID(bytes)
    }

    /// Check if this is a content-addressed CID (code)
    pub fn is_cid(&self) -> bool {
        self.0[0] & 0x80 == 0
    }

    /// Check if this is a primitive operation
    pub fn is_primitive(&self) -> bool {
        self.0[0] & 0xE0 == TAG_PRIMITIVE
    }

    /// Check if this is a literal value
    pub fn is_literal(&self) -> bool {
        self.0[0] & 0xE0 == TAG_LITERAL
    }

    /// Get the primitive ID (if this is a primitive)
    pub fn primitive_id(&self) -> Option<u16> {
        if self.is_primitive() {
            let id = u16::from_be_bytes([self.0[1], self.0[2]]);
            Some(id)
        } else {
            None
        }
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        CID(bytes)
    }
}

impl std::fmt::Display for CID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_primitive() {
            write!(f, "CID:PRIM:{}", self.primitive_id().unwrap())
        } else if self.is_literal() {
            write!(f, "CID:LIT:{}", hex::encode(&self.0[..8]))
        } else {
            write!(f, "CID:{}", hex::encode(&self.0[..8]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_cid() {
        let cid = CID::primitive(42);
        assert!(cid.is_primitive());
        assert!(!cid.is_literal());
        assert!(!cid.is_cid());
        assert_eq!(cid.primitive_id(), Some(42));
    }

    #[test]
    fn test_literal_cid() {
        let data = b"hello world";
        let cid = CID::from_literal(data);
        assert!(cid.is_literal());
        assert!(!cid.is_primitive());
        assert!(!cid.is_cid());
    }

    #[test]
    fn test_sequence_cid() {
        let cid1 = CID::primitive(1);
        let cid2 = CID::primitive(2);
        let seq_cid = CID::from_sequence(&[cid1, cid2]);
        assert!(seq_cid.is_cid());
        assert!(!seq_cid.is_primitive());
        assert!(!seq_cid.is_literal());
    }

    #[test]
    fn test_same_sequence_same_cid() {
        let cid1 = CID::primitive(1);
        let cid2 = CID::primitive(2);
        let seq1 = CID::from_sequence(&[cid1, cid2]);
        let seq2 = CID::from_sequence(&[cid1, cid2]);
        assert_eq!(seq1, seq2);
    }
}
