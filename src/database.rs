use rusqlite::{Connection, Result, params};
use std::path::Path;
use crate::hash::WordHash;

#[derive(Debug, Clone)]
pub struct WordDefinition {
    pub body_source: String,
    pub body_bytecode: Option<Vec<WordHash>>,  // Compiled hash sequence
    pub context_condition: Option<String>,
    pub context_hash: Option<WordHash>,       // Context condition as hash
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.create_tables()?;
        Ok(db)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.create_tables()?;
        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        // Content-addressed word storage
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS words (
                content_hash BLOB(32) PRIMARY KEY,    -- WordHash as primary key
                body_source TEXT NOT NULL,            -- Original source code
                body_bytecode BLOB,                   -- Compiled hash sequence
                context_condition TEXT,               -- Context predicate source (if any)
                context_hash BLOB(32),                -- Context predicate hash (if any)
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Name aliases for content hashes (local naming)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS word_names (
                name TEXT NOT NULL,
                namespace TEXT,
                content_hash BLOB(32) NOT NULL REFERENCES words(content_hash),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (name, namespace)
            )",
            [],
        )?;

        // Create indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_word_names_hash
             ON word_names(content_hash)",
            [],
        )?;

        Ok(())
    }

    /// Store a word by its content hash (universal storage)
    pub fn store_word(&self, word_hash: &WordHash, body_source: &str, body_bytecode: Option<&[WordHash]>, context_condition: Option<&str>, context_hash: Option<&WordHash>) -> Result<()> {
        // Serialize bytecode if provided
        let bytecode_blob = if let Some(bytecode) = body_bytecode {
            let mut blob = Vec::new();
            for hash in bytecode {
                blob.extend_from_slice(hash.as_bytes());
            }
            Some(blob)
        } else {
            None
        };

        // Insert or replace word (content-addressed, so always the same)
        self.conn.execute(
            "INSERT OR REPLACE INTO words
             (content_hash, body_source, body_bytecode, context_condition, context_hash, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
            params![
                word_hash.as_bytes().as_slice(),
                body_source,
                bytecode_blob,
                context_condition,
                context_hash.map(|h| h.as_bytes().as_slice())
            ],
        )?;

        Ok(())
    }

    /// Create a name alias for a content hash (local naming)
    pub fn alias_word(&self, name: &str, namespace: Option<&str>, content_hash: &WordHash) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO word_names (name, namespace, content_hash) VALUES (?1, ?2, ?3)",
            params![name, namespace, content_hash.as_bytes().as_slice()],
        )?;
        Ok(())
    }

    /// Define a word with both content storage and local naming
    pub fn define_word(&self, name: &str, namespace: Option<&str>, body: &str) -> Result<WordHash> {
        self.define_word_with_context(name, namespace, body, None)
    }

    pub fn define_word_with_context(&self, name: &str, namespace: Option<&str>, body: &str, context: Option<&str>) -> Result<WordHash> {
        // Create a simple content hash based on the source for now
        // This ensures different bodies get different hashes
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        if let Some(ctx) = context {
            hasher.update(b"CTX:");
            hasher.update(ctx.as_bytes());
        }

        let hash_bytes: [u8; 32] = hasher.finalize().into();
        let word_hash = WordHash::from_bytes(hash_bytes);

        // Store the word content
        self.store_word(&word_hash, body, None, context, None)?;

        // Create local name alias
        self.alias_word(name, namespace, &word_hash)?;

        Ok(word_hash)
    }

    /// Find content hash by name alias
    pub fn find_word_hash(&self, name: &str, namespace: Option<&str>) -> Result<Option<WordHash>> {
        let mut stmt = self.conn.prepare(
            "SELECT content_hash FROM word_names WHERE name = ?1 AND namespace IS ?2"
        )?;

        let mut rows = stmt.query_map(params![name, namespace], |row| {
            let bytes: Vec<u8> = row.get(0)?;
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(&bytes);
            Ok(WordHash::from_bytes(hash_bytes))
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Get word by content hash
    pub fn get_word(&self, content_hash: &WordHash) -> Result<Option<WordDefinition>> {
        let mut stmt = self.conn.prepare(
            "SELECT body_source, body_bytecode, context_condition, context_hash
             FROM words WHERE content_hash = ?1"
        )?;

        let mut rows = stmt.query_map(params![content_hash.as_bytes().as_slice()], |row| {
            let body_source: String = row.get(0)?;
            let bytecode_blob: Option<Vec<u8>> = row.get(1)?;
            let context_condition: Option<String> = row.get(2)?;
            let context_hash_bytes: Option<Vec<u8>> = row.get(3)?;

            // Deserialize bytecode
            let body_bytecode = if let Some(blob) = bytecode_blob {
                let mut bytecode = Vec::new();
                for chunk in blob.chunks(32) {
                    if chunk.len() == 32 {
                        let mut hash_bytes = [0u8; 32];
                        hash_bytes.copy_from_slice(chunk);
                        bytecode.push(WordHash::from_bytes(hash_bytes));
                    }
                }
                Some(bytecode)
            } else {
                None
            };

            // Deserialize context hash
            let context_hash = if let Some(bytes) = context_hash_bytes {
                if bytes.len() == 32 {
                    let mut hash_bytes = [0u8; 32];
                    hash_bytes.copy_from_slice(&bytes);
                    Some(WordHash::from_bytes(hash_bytes))
                } else {
                    None
                }
            } else {
                None
            };

            Ok(WordDefinition {
                body_source,
                body_bytecode,
                context_condition,
                context_hash,
            })
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Look up word by name (returns source for now - will return hash later)
    pub fn lookup_word(&self, name: &str, namespace: Option<&str>) -> Result<Option<String>> {
        if let Some(hash) = self.find_word_hash(name, namespace)? {
            if let Some(definition) = self.get_word(&hash)? {
                Ok(Some(definition.body_source))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn list_words(&self, namespace: Option<&str>) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM word_names WHERE namespace IS ?1 ORDER BY name"
        )?;

        let rows = stmt.query_map(params![namespace], |row| {
            Ok(row.get(0)?)
        })?;

        let mut words = Vec::new();
        for row in rows {
            words.push(row?);
        }
        Ok(words)
    }

    pub fn word_count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM words")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    /// Get all contextual definitions for a word name
    pub fn get_all_definitions(&self, name: &str, namespace: Option<&str>) -> Result<Vec<WordDefinition>> {
        // For now, just return the single definition we find
        // TODO: Implement proper contextual lookup with multiple definitions per name
        if let Some(hash) = self.find_word_hash(name, namespace)? {
            if let Some(definition) = self.get_word(&hash)? {
                Ok(vec![definition])
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::in_memory().unwrap();
        assert_eq!(db.word_count().unwrap(), 0);
    }

    #[test]
    fn test_word_definition() {
        let db = Database::in_memory().unwrap();

        // Define a word
        let _word_hash = db.define_word("square", None, "dup *").unwrap();
        assert_eq!(db.word_count().unwrap(), 1);

        // Look it up
        let body = db.lookup_word("square", None).unwrap();
        assert_eq!(body, Some("dup *".to_string()));

        // "Redefine" it with different content - creates new hash in content-addressed system
        let hash2 = db.define_word("square", None, "dup dup * *").unwrap();

        // The name alias should now point to the new hash
        let found_hash = db.find_word_hash("square", None).unwrap().unwrap();
        assert_eq!(found_hash, hash2);

        let body = db.lookup_word("square", None).unwrap();
        assert_eq!(body, Some("dup dup * *".to_string()));

        // Should now have 2 words (different content hashes) but name points to latest
        assert_eq!(db.word_count().unwrap(), 2);
    }

    #[test]
    fn test_namespaces() {
        let db = Database::in_memory().unwrap();

        // Content-addressed system: same content = same hash, regardless of namespace
        // This is actually a feature - universal code sharing!
        db.define_word("test", None, "shared version").unwrap();
        db.define_word("test", Some("math"), "shared version").unwrap();

        // Both should find the shared version (content-addressed)
        assert_eq!(
            db.lookup_word("test", None).unwrap(),
            Some("shared version".to_string())
        );
        assert_eq!(
            db.lookup_word("test", Some("math")).unwrap(),
            Some("shared version".to_string())
        );

        // Should have 1 unique word (same content hash)
        assert_eq!(db.word_count().unwrap(), 1);
    }

    #[test]
    fn test_contextual_definitions() {
        let db = Database::in_memory().unwrap();

        // In content-addressed system, each unique body gets its own hash
        // Contextual dispatch will be handled at a higher level
        db.define_word_with_context("update", None, "advance-simulation", Some("running?")).unwrap();
        db.define_word_with_context("update", None, "do-nothing", Some("paused?")).unwrap();
        db.define_word_with_context("update", None, "default-update", None).unwrap();

        // Should have 3 unique content hashes (different bodies)
        assert_eq!(db.word_count().unwrap(), 3);

        // For now, just verify we can retrieve one definition
        // TODO: Implement proper contextual resolution
        let definitions = db.get_all_definitions("update", None).unwrap();
        assert_eq!(definitions.len(), 1);  // Currently returns just the last one found

        // Should have one of the update definitions
        let valid_bodies = ["advance-simulation", "do-nothing", "default-update"];
        assert!(valid_bodies.contains(&definitions[0].body_source.as_str()));
    }

    #[test]
    fn test_contextual_redefinition() {
        let db = Database::in_memory().unwrap();

        // Define a contextual word
        db.define_word_with_context("process", None, "fast-process", Some("speed-ok?")).unwrap();

        // "Redefine" with different content - creates new hash in content-addressed system
        db.define_word_with_context("process", None, "very-fast-process", Some("speed-ok?")).unwrap();

        // Should have 2 unique content hashes now
        assert_eq!(db.word_count().unwrap(), 2);

        // Name alias points to the latest definition
        let definitions = db.get_all_definitions("process", None).unwrap();
        assert_eq!(definitions.len(), 1);  // Only returns latest alias
        assert_eq!(definitions[0].body_source, "very-fast-process");
    }
}