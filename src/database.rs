// Database layer for persistent CID storage
// Implements SQLite-backed storage for content-addressed code

use rusqlite::{Connection, Result as SqlResult, params};
use crate::cid::CID;
use crate::word::{Word, Signature};
use crate::value::Type;
use std::path::Path;

/// Database schema:
///
/// cids table:
///   - cid: BLOB PRIMARY KEY (32-byte CID)
///   - content_type: TEXT ("primitive" | "literal" | "sequence")
///   - data: BLOB (MessagePack serialized data)
///
/// words table:
///   - namespace: TEXT
///   - name: TEXT
///   - cid: BLOB (foreign key to cids.cid)
///   - signature: TEXT (optional, serialized signature)
///   - immediate: INTEGER (boolean)
///   - PRIMARY KEY (namespace, name)

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Primitive,
    Literal,
    Sequence,
}

impl ContentType {
    fn to_str(&self) -> &'static str {
        match self {
            ContentType::Primitive => "primitive",
            ContentType::Literal => "literal",
            ContentType::Sequence => "sequence",
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "primitive" => Ok(ContentType::Primitive),
            "literal" => Ok(ContentType::Literal),
            "sequence" => Ok(ContentType::Sequence),
            _ => Err(format!("Unknown content type: {}", s)),
        }
    }
}

impl Database {
    /// Create a new database connection
    /// If path is ":memory:", creates an in-memory database (useful for testing)
    pub fn new<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.create_schema()?;
        Ok(db)
    }

    /// Create the database schema if it doesn't exist
    fn create_schema(&self) -> SqlResult<()> {
        // Create CIDs table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS cids (
                cid BLOB PRIMARY KEY,
                content_type TEXT NOT NULL,
                data BLOB NOT NULL
            )",
            [],
        )?;

        // Create Words table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS words (
                namespace TEXT NOT NULL,
                name TEXT NOT NULL,
                cid BLOB NOT NULL,
                signature TEXT,
                immediate INTEGER NOT NULL,
                PRIMARY KEY (namespace, name),
                FOREIGN KEY (cid) REFERENCES cids(cid)
            )",
            [],
        )?;

        // Create index on CID for faster lookups
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_words_cid ON words(cid)",
            [],
        )?;

        Ok(())
    }

    /// Store a CID with its content
    pub fn store_cid(&self, cid: &CID, content_type: ContentType, data: &[u8]) -> Result<(), String> {
        self.conn.execute(
            "INSERT OR REPLACE INTO cids (cid, content_type, data) VALUES (?1, ?2, ?3)",
            params![cid.as_bytes(), content_type.to_str(), data],
        ).map_err(|e| format!("Failed to store CID: {}", e))?;
        Ok(())
    }

    /// Retrieve CID data
    pub fn get_cid(&self, cid: &CID) -> Result<Option<(ContentType, Vec<u8>)>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT content_type, data FROM cids WHERE cid = ?1"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let result = stmt.query_row(params![cid.as_bytes()], |row| {
            let type_str: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((type_str, data))
        });

        match result {
            Ok((type_str, data)) => {
                let content_type = ContentType::from_str(&type_str)?;
                Ok(Some((content_type, data)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to retrieve CID: {}", e)),
        }
    }

    /// Check if a CID exists in the database
    pub fn has_cid(&self, cid: &CID) -> Result<bool, String> {
        let mut stmt = self.conn.prepare(
            "SELECT 1 FROM cids WHERE cid = ?1"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let exists = stmt.exists(params![cid.as_bytes()]).map_err(|e| format!("Query failed: {}", e))?;
        Ok(exists)
    }

    /// Store a word definition
    pub fn store_word(&self, namespace: &str, name: &str, word: &Word) -> Result<(), String> {
        // First, ensure the CID exists
        if let Some(ref cid) = word.cid {
            if !self.has_cid(cid)? {
                return Err(format!("CID not found in database: {}", cid));
            }
        } else {
            return Err("Cannot store word without CID".to_string());
        }

        // Serialize signature if present
        let sig_str = word.signature.as_ref().map(|sig| serialize_signature(sig));

        self.conn.execute(
            "INSERT OR REPLACE INTO words (namespace, name, cid, signature, immediate) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                namespace,
                name,
                word.cid.as_ref().unwrap().as_bytes(),
                sig_str,
                word.immediate as i32
            ],
        ).map_err(|e| format!("Failed to store word: {}", e))?;

        Ok(())
    }

    /// Retrieve a word definition (returns CID and metadata, not the full Word)
    pub fn get_word(&self, namespace: &str, name: &str) -> Result<Option<(CID, Option<Signature>, bool)>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT cid, signature, immediate FROM words WHERE namespace = ?1 AND name = ?2"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let result = stmt.query_row(params![namespace, name], |row| {
            let cid_bytes: Vec<u8> = row.get(0)?;
            let sig_str: Option<String> = row.get(1)?;
            let immediate: i32 = row.get(2)?;
            Ok((cid_bytes, sig_str, immediate))
        });

        match result {
            Ok((cid_bytes, sig_str, immediate)) => {
                let cid = CID::from_bytes(cid_bytes.try_into().map_err(|_| "Invalid CID size")?);
                let signature = sig_str.and_then(|s| deserialize_signature(&s).ok());
                Ok(Some((cid, signature, immediate != 0)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to retrieve word: {}", e)),
        }
    }

    /// List all words in a namespace
    pub fn list_words(&self, namespace: &str) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM words WHERE namespace = ?1 ORDER BY name"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let names = stmt.query_map(params![namespace], |row| {
            row.get(0)
        }).map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for name in names {
            result.push(name.map_err(|e| format!("Failed to read name: {}", e))?);
        }
        Ok(result)
    }

    /// List all namespaces
    pub fn list_namespaces(&self) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT namespace FROM words ORDER BY namespace"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let namespaces = stmt.query_map([], |row| {
            row.get(0)
        }).map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for ns in namespaces {
            result.push(ns.map_err(|e| format!("Failed to read namespace: {}", e))?);
        }
        Ok(result)
    }

    /// Delete a word
    pub fn delete_word(&self, namespace: &str, name: &str) -> Result<(), String> {
        self.conn.execute(
            "DELETE FROM words WHERE namespace = ?1 AND name = ?2",
            params![namespace, name],
        ).map_err(|e| format!("Failed to delete word: {}", e))?;
        Ok(())
    }

    /// Get statistics about the database
    pub fn stats(&self) -> Result<(usize, usize), String> {
        let cid_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM cids",
            [],
            |row| row.get(0)
        ).map_err(|e| format!("Failed to count CIDs: {}", e))?;

        let word_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM words",
            [],
            |row| row.get(0)
        ).map_err(|e| format!("Failed to count words: {}", e))?;

        Ok((cid_count, word_count))
    }
}

/// Serialize a signature to string format
/// Format: "type1 type2 -> type3 type4"
fn serialize_signature(sig: &Signature) -> String {
    let inputs: Vec<String> = sig.inputs.iter().map(|t| t.to_string()).collect();
    let outputs: Vec<String> = sig.outputs.iter().map(|t| t.to_string()).collect();
    format!("{} -> {}", inputs.join(" "), outputs.join(" "))
}

/// Deserialize a signature from string format
fn deserialize_signature(s: &str) -> Result<Signature, String> {
    let parts: Vec<&str> = s.split(" -> ").collect();
    if parts.len() != 2 {
        return Err(format!("Invalid signature format: {}", s));
    }

    let inputs: Result<Vec<Type>, String> = parts[0]
        .split_whitespace()
        .map(|t| Type::from_str(t))
        .collect();

    let outputs: Result<Vec<Type>, String> = parts[1]
        .split_whitespace()
        .map(|t| Type::from_str(t))
        .collect();

    Ok(Signature {
        inputs: inputs?,
        outputs: outputs?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::new(":memory:").unwrap();
        let (cid_count, word_count) = db.stats().unwrap();
        assert_eq!(cid_count, 0);
        assert_eq!(word_count, 0);
    }

    #[test]
    fn test_store_and_retrieve_cid() {
        let db = Database::new(":memory:").unwrap();
        let cid = CID::from_bytes([1u8; 32]);
        let data = b"test data";

        db.store_cid(&cid, ContentType::Literal, data).unwrap();

        let retrieved = db.get_cid(&cid).unwrap();
        assert!(retrieved.is_some());
        let (content_type, retrieved_data) = retrieved.unwrap();
        assert_eq!(retrieved_data, data);
    }

    #[test]
    fn test_signature_serialization() {
        let sig = Signature {
            inputs: vec![Type::I64, Type::I64],
            outputs: vec![Type::I64],
        };

        let serialized = serialize_signature(&sig);
        assert_eq!(serialized, "core.i64 core.i64 -> core.i64");

        let deserialized = deserialize_signature(&serialized).unwrap();
        assert_eq!(deserialized, sig);
    }
}
