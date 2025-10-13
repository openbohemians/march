// Serializable representations for database storage
// Uses MessagePack for efficient, cross-platform binary format

use serde::{Serialize, Deserialize};
use crate::cid::CID;
use crate::value::{Value, Type};

/// Serializable representation of Value
/// This can be converted to/from bytes using MessagePack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializableValue {
    Number(i64),
    String(String),
    Type(SerializableType),
    /// Quotation stored as its CID (the quotation is compiled separately)
    Quotation(CID),
    Array(Vec<SerializableValue>),
    Map(Vec<(String, SerializableValue)>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializableType {
    I64,
    String,
    Type,
    Quotation,
    Array,
    Map,
    MutableArray,
    MutableMap,
}

impl From<Type> for SerializableType {
    fn from(t: Type) -> Self {
        match t {
            Type::I64 => SerializableType::I64,
            Type::String => SerializableType::String,
            Type::Type => SerializableType::Type,
            Type::Quotation => SerializableType::Quotation,
            Type::Array => SerializableType::Array,
            Type::Map => SerializableType::Map,
            Type::MutableArray => SerializableType::MutableArray,
            Type::MutableMap => SerializableType::MutableMap,
        }
    }
}

impl From<SerializableType> for Type {
    fn from(st: SerializableType) -> Self {
        match st {
            SerializableType::I64 => Type::I64,
            SerializableType::String => Type::String,
            SerializableType::Type => Type::Type,
            SerializableType::Quotation => Type::Quotation,
            SerializableType::Array => Type::Array,
            SerializableType::Map => Type::Map,
            SerializableType::MutableArray => Type::MutableArray,
            SerializableType::MutableMap => Type::MutableMap,
        }
    }
}

impl SerializableValue {
    /// Serialize to MessagePack bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        rmp_serde::to_vec(self)
            .map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize from MessagePack bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| format!("Deserialization error: {}", e))
    }

    /// Create a CID for this value by serializing and hashing
    pub fn to_cid(&self) -> Result<CID, String> {
        let bytes = self.to_bytes()?;
        Ok(CID::from_literal(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_number() {
        let val = SerializableValue::Number(42);
        let bytes = val.to_bytes().unwrap();
        let restored = SerializableValue::from_bytes(&bytes).unwrap();
        match restored {
            SerializableValue::Number(n) => assert_eq!(n, 42),
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_serialize_string() {
        let val = SerializableValue::String("hello".to_string());
        let bytes = val.to_bytes().unwrap();
        let restored = SerializableValue::from_bytes(&bytes).unwrap();
        match restored {
            SerializableValue::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_value_to_cid() {
        let val1 = SerializableValue::Number(42);
        let val2 = SerializableValue::Number(42);
        let cid1 = val1.to_cid().unwrap();
        let cid2 = val2.to_cid().unwrap();
        // Same value should produce same CID
        assert_eq!(cid1, cid2);
        assert!(cid1.is_literal());
    }
}
