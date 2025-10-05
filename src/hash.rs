use sha2::{Sha256, Digest};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WordHash([u8; 32]);  // Fixed 32-byte hash

impl WordHash {
    /// Check if this is a reserved hash (high bit set)
    pub fn is_reserved(&self) -> bool {
        self.0[0] & 0x80 != 0
    }

    /// Check if this is a content hash (high bit clear)
    pub fn is_content(&self) -> bool {
        self.0[0] & 0x80 == 0
    }

    /// Create a primitive hash from an ID
    pub fn primitive(id: u8) -> Self {
        let mut hash = [0u8; 32];
        hash[0] = 0x80;  // Set reserved bit
        hash[1] = id;    // Primitive ID
        WordHash(hash)
    }

    /// Create a state operation hash
    pub fn state_get() -> Self {
        let mut hash = [0u8; 32];
        hash[0] = 0x81;  // Reserved bit + state get category
        WordHash(hash)
    }

    pub fn state_set() -> Self {
        let mut hash = [0u8; 32];
        hash[0] = 0x82;  // Reserved bit + state set category
        WordHash(hash)
    }

    /// Create a literal value hash
    pub fn literal_i64(value: i64) -> Self {
        let mut hash = [0u8; 32];
        hash[0] = 0xA0;  // Reserved bit + literal category
        hash[1..9].copy_from_slice(&value.to_be_bytes());  // Store full i64
        WordHash(hash)
    }

    /// Generate content hash from a sequence of word hashes
    pub fn content_hash(definition: &[WordHash]) -> Self {
        let mut hasher = Sha256::new();

        for word_hash in definition {
            hasher.update(&word_hash.0);
        }

        let mut hash: [u8; 32] = hasher.finalize().into();
        // Ensure high bit is clear (content hash)
        if hash[0] & 0x80 != 0 {
            hash[0] &= 0x7F;  // Clear high bit
        }
        WordHash(hash)
    }

    /// Check if this is a primitive
    pub fn is_primitive(&self) -> bool {
        self.is_reserved() && self.0[0] == 0x80
    }

    /// Get primitive ID if this is a primitive
    pub fn primitive_id(&self) -> Option<u8> {
        if self.is_primitive() {
            Some(self.0[1])
        } else {
            None
        }
    }

    /// Get the literal i64 value if this is a literal
    pub fn get_literal_i64(&self) -> Option<i64> {
        if self.is_reserved() && self.0[0] == 0xA0 {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&self.0[1..9]);
            Some(i64::from_be_bytes(bytes))
        } else {
            None
        }
    }

    /// Check what kind of reserved hash this is
    pub fn reserved_category(&self) -> Option<u8> {
        if self.is_reserved() {
            Some(self.0[0])
        } else {
            None
        }
    }

    /// Get the raw bytes (for database storage, etc.)
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        WordHash(bytes)
    }
}

/// Primitive word IDs - now using u8
pub mod primitives {
    pub const DUP: u8 = 1;
    pub const DROP: u8 = 2;
    pub const SWAP: u8 = 3;
    pub const OVER: u8 = 4;
    pub const ROT: u8 = 5;

    pub const ADD: u8 = 10;
    pub const SUB: u8 = 11;
    pub const MUL: u8 = 12;
    pub const DIV: u8 = 13;

    pub const EQ: u8 = 20;
    pub const GT: u8 = 21;
    pub const LT: u8 = 22;
    pub const AND: u8 = 23;
    pub const OR: u8 = 24;

    pub const EXIT: u8 = 100;
}

/// Execution token for ITC execution
#[derive(Debug, Clone)]
pub enum ExecutionToken {
    /// Primitive operation
    Primitive(u8),

    /// User-defined word (sequence of hashes to execute)
    UserWord(Vec<WordHash>),

    /// Immediate value
    PushI64(i64),

    /// State variable reference
    StateRef(String),  // Will be optimized to ID later
}

/// Runtime execution context
pub struct Runtime {
    /// Maps word hashes to their execution tokens
    pub hash_to_xt: HashMap<WordHash, ExecutionToken>,

    /// Primitive function table for ITC execution
    primitive_table: Vec<fn(&mut crate::Interpreter) -> Result<(), crate::RuntimeError>>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Runtime {
            hash_to_xt: HashMap::new(),
            primitive_table: Vec::new(),
        };

        runtime.register_primitives();
        runtime
    }

    /// Register all built-in primitives
    fn register_primitives(&mut self) {
        use primitives::*;

        // Stack operations
        self.register_primitive(DUP, crate::Interpreter::dup);
        self.register_primitive(DROP, crate::Interpreter::drop);
        self.register_primitive(SWAP, crate::Interpreter::swap);

        // Arithmetic operations
        self.register_primitive(ADD, crate::Interpreter::add);
        self.register_primitive(SUB, crate::Interpreter::sub);
        self.register_primitive(MUL, crate::Interpreter::mul);
        self.register_primitive(DIV, crate::Interpreter::div);

        // Comparison operations
        self.register_primitive(EQ, crate::Interpreter::equals);
        self.register_primitive(GT, crate::Interpreter::greater_than);
        self.register_primitive(AND, crate::Interpreter::logical_and);
    }

    /// Register a primitive function
    fn register_primitive(&mut self, id: u8, func: fn(&mut crate::Interpreter) -> Result<(), crate::RuntimeError>) {
        let hash = WordHash::primitive(id);
        let xt = ExecutionToken::Primitive(id);

        self.hash_to_xt.insert(hash, xt);

        // Ensure primitive_table is large enough
        while self.primitive_table.len() <= id as usize {
            self.primitive_table.push(|_| Ok(()));  // Dummy function
        }

        self.primitive_table[id as usize] = func;
    }

    /// Define a user word from source
    pub fn define_word(&mut self, source: &str) -> Result<WordHash, crate::RuntimeError> {
        let definition = self.compile_source(source)?;
        let word_hash = WordHash::content_hash(&definition);

        self.hash_to_xt.insert(word_hash.clone(), ExecutionToken::UserWord(definition));

        Ok(word_hash)
    }

    /// Compile source code to sequence of word hashes
    fn compile_source(&self, source: &str) -> Result<Vec<WordHash>, crate::RuntimeError> {
        let mut definition = Vec::new();

        for token in source.split_whitespace() {
            let word_hash = self.resolve_token(token)?;
            definition.push(word_hash);
        }

        Ok(definition)
    }

    /// Resolve a token to a word hash
    fn resolve_token(&self, token: &str) -> Result<WordHash, crate::RuntimeError> {
        use primitives::*;

        // Try to parse as number
        if let Ok(_num) = token.parse::<i64>() {
            // For now, return a special primitive that pushes the number
            // TODO: Handle immediate values properly
            return Ok(WordHash::primitive(0)); // Placeholder
        }

        // Map token to hash
        let word_hash = match token {
            "dup" => WordHash::primitive(DUP),
            "drop" => WordHash::primitive(DROP),
            "swap" => WordHash::primitive(SWAP),
            "over" => WordHash::primitive(OVER),
            "rot" => WordHash::primitive(ROT),
            "+" => WordHash::primitive(ADD),
            "-" => WordHash::primitive(SUB),
            "*" => WordHash::primitive(MUL),
            "/" => WordHash::primitive(DIV),
            "=" => WordHash::primitive(EQ),
            ">" => WordHash::primitive(GT),
            "<" => WordHash::primitive(LT),
            "&" => WordHash::primitive(AND),
            "|" => WordHash::primitive(OR),
            "@" => WordHash::state_get(),
            "!" => WordHash::state_set(),
            ";" => WordHash::primitive(EXIT),
            _ => return Err(crate::RuntimeError::ParseError),
        };

        Ok(word_hash)
    }

    /// Execute a word by its hash
    pub fn execute_word_hash(&self, interpreter: &mut crate::Interpreter, word_hash: &WordHash) -> Result<(), crate::RuntimeError> {
        let xt = self.hash_to_xt.get(word_hash)
            .ok_or(crate::RuntimeError::ParseError)?;

        match xt {
            ExecutionToken::Primitive(id) => {
                let func = self.primitive_table.get(*id as usize)
                    .ok_or(crate::RuntimeError::ParseError)?;
                func(interpreter)
            }
            ExecutionToken::UserWord(definition) => {
                for hash in definition {
                    self.execute_word_hash(interpreter, hash)?;
                }
                Ok(())
            }
            ExecutionToken::PushI64(value) => {
                interpreter.push(crate::Value::I64(*value), crate::ConcreteType::I64);
                Ok(())
            }
            ExecutionToken::StateRef(_name) => {
                // TODO: Implement state variable access
                Err(crate::RuntimeError::ParseError)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_hash() {
        let dup_hash = WordHash::primitive(primitives::DUP);
        assert!(dup_hash.is_primitive());
        assert_eq!(dup_hash.primitive_id(), Some(primitives::DUP));
    }

    #[test]
    fn test_content_hash() {
        let dup_hash = WordHash::primitive(primitives::DUP);
        let mul_hash = WordHash::primitive(primitives::MUL);

        let square_hash = WordHash::content_hash(&[dup_hash, mul_hash]);
        assert!(!square_hash.is_primitive());
        assert_eq!(square_hash.primitive_id(), None);
    }

    #[test]
    fn test_deterministic_hashing() {
        let dup_hash = WordHash::primitive(primitives::DUP);
        let mul_hash = WordHash::primitive(primitives::MUL);

        let hash1 = WordHash::content_hash(&[dup_hash.clone(), mul_hash.clone()]);
        let hash2 = WordHash::content_hash(&[dup_hash, mul_hash]);

        assert_eq!(hash1, hash2);
    }
}