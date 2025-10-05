use std::collections::HashMap;
use std::io::{self, Write};
use num_bigint::BigInt;
use num_rational::BigRational;

pub mod database;
pub mod state;
pub mod hash;

#[derive(Debug, Clone, PartialEq)]
pub enum AbstractType {
    Int,
    Rational,
    String,
    Array(Box<AbstractType>),
    Ptr(Box<AbstractType>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConcreteType {
    I8, I16, I32, I64, I128,
    F32, F64,
    BigInt,
    BigRational,
    String,
    Array { element_type: Box<ConcreteType>, size: usize },
    Ptr { target_type: Box<ConcreteType> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I64(i64),
    BigInt(BigInt),
    BigRational(BigRational),
    String(String),
    Array(Vec<Value>),
    Ptr(usize),
}

#[derive(Debug)]
pub enum RuntimeError {
    StackUnderflow,
    TypeMismatch,
    NotAPointer,
    DatabaseError(rusqlite::Error),
    ParseError,
    TypeError(TypeError),
}

#[derive(Debug)]
pub enum TypeError {
    StackUnderflow,
    Mismatch(ConcreteType, ConcreteType),
    InvalidOperation(String),
}

#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Runtime,           // Normal execution
    TypeChecker,       // Static type analysis
    Optimizer,         // Code optimization
    Documentation,     // Doc generation
    Profiler,          // Performance analysis
    Debugger,          // Step-through debugging
}

#[derive(Debug)]
pub enum ExecutionResult {
    Runtime(()),                    // Normal execution completed
    TypeCheck(TypeSignature),       // Type analysis result
    Optimized(Vec<u8>),            // Optimized bytecode
    Documentation(String),          // Generated documentation
    Profile(PerformanceStats),      // Performance data
    Debug(DebugInfo),              // Debug information
}

#[derive(Debug)]
pub struct TypeSignature {
    pub inputs: Vec<ConcreteType>,
    pub outputs: Vec<ConcreteType>,
}

#[derive(Debug)]
pub struct PerformanceStats {
    pub cycles: u64,
    pub memory_usage: usize,
}

#[derive(Debug)]
pub struct DebugInfo {
    pub step_count: u64,
    pub breakpoints: Vec<hash::WordHash>,
}

impl From<rusqlite::Error> for RuntimeError {
    fn from(err: rusqlite::Error) -> Self {
        RuntimeError::DatabaseError(err)
    }
}

pub struct Memory {
    heap: Vec<Value>,
    addresses: HashMap<usize, (ConcreteType, usize)>,
    next_addr: usize,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            heap: Vec::new(),
            addresses: HashMap::new(),
            next_addr: 1000,
        }
    }
}

pub struct Interpreter {
    value_stack: Vec<Value>,
    type_stack: Vec<ConcreteType>,
    memory: Memory,
    pub db: database::Database,
    pub state: state::ProgramState,
    current_context: Option<String>,
    pub runtime: hash::Runtime,  // Hash-based execution engine
    execution_mode: ExecutionMode,  // Multi-modal execution
    pub type_analysis_stack: Vec<ConcreteType>,  // For type checking mode
}

impl Interpreter {
    pub fn new() -> Result<Self, RuntimeError> {
        Ok(Interpreter {
            value_stack: Vec::new(),
            type_stack: Vec::new(),
            memory: Memory::new(),
            db: database::Database::in_memory()?,
            state: state::ProgramState::new(),
            current_context: None,
            runtime: hash::Runtime::new(),
            execution_mode: ExecutionMode::Runtime,
            type_analysis_stack: Vec::new(),
        })
    }

    pub fn with_database(db_path: &str) -> Result<Self, RuntimeError> {
        Ok(Interpreter {
            value_stack: Vec::new(),
            type_stack: Vec::new(),
            memory: Memory::new(),
            db: database::Database::new(db_path)?,
            state: state::ProgramState::new(),
            current_context: None,
            runtime: hash::Runtime::new(),
            execution_mode: ExecutionMode::Runtime,
            type_analysis_stack: Vec::new(),
        })
    }

    /// Set the execution mode for multi-modal execution
    pub fn set_execution_mode(&mut self, mode: ExecutionMode) {
        // Reset type analysis stack when switching to type checker mode
        if matches!(mode, ExecutionMode::TypeChecker) {
            self.type_analysis_stack.clear();
        }

        self.execution_mode = mode;
    }

    /// Get the current execution mode
    pub fn execution_mode(&self) -> &ExecutionMode {
        &self.execution_mode
    }

    pub fn push(&mut self, value: Value, concrete_type: ConcreteType) {
        self.value_stack.push(value);
        self.type_stack.push(concrete_type);
    }

    pub fn pop(&mut self) -> Result<(Value, ConcreteType), RuntimeError> {
        match (self.value_stack.pop(), self.type_stack.pop()) {
            (Some(val), Some(typ)) => Ok((val, typ)),
            _ => Err(RuntimeError::StackUnderflow),
        }
    }

    pub fn stack_size(&self) -> usize {
        self.value_stack.len()
    }

    pub fn dup(&mut self) -> Result<(), RuntimeError> {
        let (val, typ) = self.pop()?;
        self.push(val.clone(), typ.clone());
        self.push(val, typ);
        Ok(())
    }

    pub fn swap(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;
        self.push(b, b_type);
        self.push(a, a_type);
        Ok(())
    }

    pub fn drop(&mut self) -> Result<(), RuntimeError> {
        let _ = self.pop()?;
        Ok(())
    }

    pub fn add(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (a, b, a_type, b_type) {
            (Value::I64(x), Value::I64(y), ConcreteType::I64, ConcreteType::I64) => {
                self.push(Value::I64(x + y), ConcreteType::I64);
            }
            (Value::BigInt(x), Value::BigInt(y), ConcreteType::BigInt, ConcreteType::BigInt) => {
                self.push(Value::BigInt(x + y), ConcreteType::BigInt);
            }
            (Value::BigRational(x), Value::BigRational(y), ConcreteType::BigRational, ConcreteType::BigRational) => {
                self.push(Value::BigRational(x + y), ConcreteType::BigRational);
            }
            _ => return Err(RuntimeError::TypeMismatch),
        }
        Ok(())
    }

    pub fn sub(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (a, b, a_type, b_type) {
            (Value::I64(x), Value::I64(y), ConcreteType::I64, ConcreteType::I64) => {
                self.push(Value::I64(x - y), ConcreteType::I64);
            }
            (Value::BigInt(x), Value::BigInt(y), ConcreteType::BigInt, ConcreteType::BigInt) => {
                self.push(Value::BigInt(x - y), ConcreteType::BigInt);
            }
            (Value::BigRational(x), Value::BigRational(y), ConcreteType::BigRational, ConcreteType::BigRational) => {
                self.push(Value::BigRational(x - y), ConcreteType::BigRational);
            }
            _ => return Err(RuntimeError::TypeMismatch),
        }
        Ok(())
    }

    pub fn mul(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (a, b, a_type, b_type) {
            (Value::I64(x), Value::I64(y), ConcreteType::I64, ConcreteType::I64) => {
                self.push(Value::I64(x * y), ConcreteType::I64);
            }
            (Value::BigInt(x), Value::BigInt(y), ConcreteType::BigInt, ConcreteType::BigInt) => {
                self.push(Value::BigInt(x * y), ConcreteType::BigInt);
            }
            (Value::BigRational(x), Value::BigRational(y), ConcreteType::BigRational, ConcreteType::BigRational) => {
                self.push(Value::BigRational(x * y), ConcreteType::BigRational);
            }
            _ => return Err(RuntimeError::TypeMismatch),
        }
        Ok(())
    }

    pub fn div(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (a, b, a_type, b_type) {
            (Value::I64(x), Value::I64(y), ConcreteType::I64, ConcreteType::I64) => {
                if y == 0 {
                    return Err(RuntimeError::TypeMismatch);
                }
                self.push(Value::I64(x / y), ConcreteType::I64);
            }
            (Value::BigInt(x), Value::BigInt(y), ConcreteType::BigInt, ConcreteType::BigInt) => {
                if y == BigInt::from(0) {
                    return Err(RuntimeError::TypeMismatch);
                }
                self.push(Value::BigInt(x / y), ConcreteType::BigInt);
            }
            (Value::BigRational(x), Value::BigRational(y), ConcreteType::BigRational, ConcreteType::BigRational) => {
                if y == BigRational::from(BigInt::from(0)) {
                    return Err(RuntimeError::TypeMismatch);
                }
                self.push(Value::BigRational(x / y), ConcreteType::BigRational);
            }
            _ => return Err(RuntimeError::TypeMismatch),
        }
        Ok(())
    }

    pub fn print_stack(&self) {
        println!("Stack (top to bottom):");
        for (i, (val, typ)) in self.value_stack.iter().zip(self.type_stack.iter()).enumerate().rev() {
            println!("  {}: {:?} : {:?}", i, val, typ);
        }
        if self.value_stack.is_empty() {
            println!("  (empty)");
        }
    }

    pub fn execute_word(&mut self, input: &str) -> Result<(), RuntimeError> {
        // Handle word definitions
        if input.trim().starts_with(':') {
            return self.parse_definition(input);
        }

        // Handle state declarations
        if input.trim().starts_with("State:") {
            return self.parse_state_declaration(input);
        }

        // Handle context declarations
        if input.trim().starts_with("Context") {
            return self.parse_context_declaration(input);
        }

        // Handle state access operations
        if input.ends_with('@') {
            return self.state_get(&input[..input.len()-1]);
        }
        if input.ends_with('!') {
            return self.state_set(&input[..input.len()-1]);
        }

        // Handle single word execution
        match input {
            "+" => self.add(),
            "-" => self.sub(),
            "*" => self.mul(),
            "/" => self.div(),
            "dup" => self.dup(),
            "swap" => self.swap(),
            "drop" => self.drop(),
            ".s" => { self.print_stack(); Ok(()) },
            "words" => { self.list_words(); Ok(()) },
            ".state" => { self.print_state(); Ok(()) },
            "&" => self.logical_and(),
            "=" => self.equals(),
            ">" => self.greater_than(),
            _ => {
                // Try hash-based lookup first
                if let Some(word_hash) = self.db.find_word_hash(input, None)? {
                    // Use runtime execution for legacy compatibility
                    self.execute_word_hash_runtime(&word_hash)
                } else if let Ok(n) = input.parse::<i64>() {
                    // Try to parse as number
                    self.push(Value::I64(n), ConcreteType::I64);
                    Ok(())
                } else {
                    // Check if it's a state variable access
                    if let Ok(value) = self.state.get_variable(input) {
                        let value = value.clone();
                        let concrete_type = self.infer_concrete_type_from_value(&value);
                        self.push(value, concrete_type);
                        Ok(())
                    } else {
                        println!("Unknown word: {}", input);
                        Err(RuntimeError::ParseError)
                    }
                }
            }
        }
    }

    fn parse_definition(&mut self, input: &str) -> Result<(), RuntimeError> {
        let input = input.trim();

        // Split by whitespace and check for proper syntax
        let parts: Vec<&str> = input.split_whitespace().collect();

        if parts.len() < 3 || parts[0] != ":" || parts.last() != Some(&";") {
            println!("Invalid definition syntax. Use: : word_name body ;");
            return Err(RuntimeError::ParseError);
        }

        let name = parts[1];
        let body = parts[2..parts.len()-1].join(" ");

        if body.is_empty() {
            println!("Word definition cannot be empty");
            return Err(RuntimeError::ParseError);
        }

        // Use current context if set
        let context = self.current_context.as_deref();

        // Compile the word body to hash sequence with user-word resolution
        let definition = self.compile_word_body(&body)?;
        let word_hash = hash::WordHash::content_hash(&definition);

        // Store the execution token in runtime
        self.runtime.hash_to_xt.insert(word_hash.clone(), hash::ExecutionToken::UserWord(definition.clone()));

        // Store in database with content addressing
        self.db.store_word(&word_hash, &body, Some(&definition), context, None)?;

        // Create local name alias
        self.db.alias_word(name, None, &word_hash)?;

        if let Some(ctx) = &self.current_context {
            println!("Defined word: {} (context: {})", name, ctx);
        } else {
            println!("Defined word: {}", name);
        }
        Ok(())
    }

    /// Compile word body to sequence of hashes with user-word resolution
    fn compile_word_body(&mut self, body: &str) -> Result<Vec<hash::WordHash>, RuntimeError> {
        let mut definition = Vec::new();

        for token in body.split_whitespace() {
            let word_hash = self.resolve_token_to_hash(token)?;

            // Handle literal value registration
            if let Some(num) = word_hash.get_literal_i64() {
                self.runtime.hash_to_xt.insert(word_hash.clone(), hash::ExecutionToken::PushI64(num));
            }

            definition.push(word_hash);
        }

        Ok(definition)
    }

    /// Resolve a token to a word hash (primitives + user-defined words)
    fn resolve_token_to_hash(&self, token: &str) -> Result<hash::WordHash, RuntimeError> {
        use hash::primitives::*;

        // First try to find user-defined word
        if let Ok(Some(user_hash)) = self.db.find_word_hash(token, None) {
            return Ok(user_hash);
        }

        // Try to parse as number
        if let Ok(num) = token.parse::<i64>() {
            return Ok(hash::WordHash::literal_i64(num));
        }

        // Map primitive tokens to hashes
        let word_hash = match token {
            "dup" => hash::WordHash::primitive(DUP),
            "drop" => hash::WordHash::primitive(DROP),
            "swap" => hash::WordHash::primitive(SWAP),
            "over" => hash::WordHash::primitive(OVER),
            "rot" => hash::WordHash::primitive(ROT),
            "+" => hash::WordHash::primitive(ADD),
            "-" => hash::WordHash::primitive(SUB),
            "*" => hash::WordHash::primitive(MUL),
            "/" => hash::WordHash::primitive(DIV),
            "=" => hash::WordHash::primitive(EQ),
            ">" => hash::WordHash::primitive(GT),
            "<" => hash::WordHash::primitive(LT),
            "&" => hash::WordHash::primitive(AND),
            "|" => hash::WordHash::primitive(OR),
            "@" => hash::WordHash::state_get(),
            "!" => hash::WordHash::state_set(),
            ";" => hash::WordHash::primitive(EXIT),
            _ => return Err(RuntimeError::ParseError),
        };

        Ok(word_hash)
    }

    fn execute_body(&mut self, body: &str) -> Result<(), RuntimeError> {
        let words: Vec<&str> = body.split_whitespace().collect();
        let mut i = 0;

        while i < words.len() {
            let word = words[i];

            // Look ahead for state operations
            if i + 1 < words.len() {
                let next = words[i + 1];
                if next == "@" {
                    // Variable get
                    self.state_get(word)?;
                    i += 2; // Skip both words
                    continue;
                } else if next == "!" {
                    // Variable set
                    self.state_set(word)?;
                    i += 2; // Skip both words
                    continue;
                }
            }

            // Regular word execution
            self.execute_word(word)?;
            i += 1;
        }
        Ok(())
    }

    fn list_words(&self) {
        match self.db.list_words(None) {
            Ok(words) => {
                if words.is_empty() {
                    println!("No words defined.");
                } else {
                    println!("Defined words:");
                    for word in words {
                        if let Ok(Some(body)) = self.db.lookup_word(&word, None) {
                            println!("  {} : {}", word, body);
                        }
                    }
                }
            }
            Err(e) => println!("Error listing words: {:?}", e),
        }
    }

    fn print_state(&self) {
        let variables = self.state.list_variables();
        if variables.is_empty() {
            println!("No state variables declared.");
        } else {
            println!("State variables:");
            for var in variables {
                println!("  {} : {:?} = {:?}", var.name, var.abstract_type, var.value);
            }
        }
    }

    fn parse_state_declaration(&mut self, input: &str) -> Result<(), RuntimeError> {
        // Parse: "State: velocity : Integer where velocity >= 0"
        let input = input.trim();

        // Remove "State:" prefix
        let declaration = input.strip_prefix("State:").unwrap().trim();

        // Split on the first ":"
        let parts: Vec<&str> = declaration.splitn(2, ':').collect();
        if parts.len() != 2 {
            println!("Invalid state declaration. Use: State: name : Type");
            return Err(RuntimeError::ParseError);
        }

        let name = parts[0].trim();
        let type_part = parts[1].trim();

        // For now, simple type parsing (no constraints yet)
        let abstract_type = match type_part {
            "Integer" | "Int" => AbstractType::Int,
            "Rational" => AbstractType::Rational,
            "String" => AbstractType::String,
            _ => {
                println!("Unknown type: {}", type_part);
                return Err(RuntimeError::ParseError);
            }
        };

        // Default initial values
        let initial_value = match abstract_type {
            AbstractType::Int => Value::I64(0),
            AbstractType::Rational => Value::BigRational(num_rational::BigRational::from(num_bigint::BigInt::from(0))),
            AbstractType::String => Value::String(String::new()),
            _ => {
                println!("Cannot create initial value for type: {:?}", abstract_type);
                return Err(RuntimeError::ParseError);
            }
        };

        self.state.declare_variable(name, abstract_type, vec![], initial_value)?;
        println!("Declared state variable: {}", name);
        Ok(())
    }

    fn state_get(&mut self, name: &str) -> Result<(), RuntimeError> {
        let value = self.state.get_variable(name)?.clone();
        let concrete_type = self.infer_concrete_type_from_value(&value);
        self.push(value, concrete_type);
        Ok(())
    }

    fn state_set(&mut self, name: &str) -> Result<(), RuntimeError> {
        let (value, _) = self.pop()?;
        self.state.set_variable(name, value)?;
        Ok(())
    }

    fn infer_concrete_type_from_value(&self, value: &Value) -> ConcreteType {
        match value {
            Value::I64(_) => ConcreteType::I64,
            Value::BigInt(_) => ConcreteType::BigInt,
            Value::BigRational(_) => ConcreteType::BigRational,
            Value::String(_) => ConcreteType::String,
            Value::Array(_) => ConcreteType::Array {
                element_type: Box::new(ConcreteType::I64),
                size: 0
            }, // TODO: Better array type inference
            Value::Ptr(_) => ConcreteType::Ptr {
                target_type: Box::new(ConcreteType::I64)
            }, // TODO: Better pointer type inference
        }
    }

    /// Execute a word by its content hash (multi-modal)
    pub fn execute_word_hash(&mut self, word_hash: &hash::WordHash) -> Result<ExecutionResult, RuntimeError> {
        match &self.execution_mode {
            ExecutionMode::Runtime => {
                self.execute_runtime(word_hash)?;
                Ok(ExecutionResult::Runtime(()))
            }
            ExecutionMode::TypeChecker => {
                let sig = self.execute_type_check(word_hash)?;
                Ok(ExecutionResult::TypeCheck(sig))
            }
            ExecutionMode::Optimizer => {
                let bytecode = self.execute_optimize(word_hash)?;
                Ok(ExecutionResult::Optimized(bytecode))
            }
            ExecutionMode::Documentation => {
                let docs = self.execute_documentation(word_hash)?;
                Ok(ExecutionResult::Documentation(docs))
            }
            ExecutionMode::Profiler => {
                let stats = self.execute_profile(word_hash)?;
                Ok(ExecutionResult::Profile(stats))
            }
            ExecutionMode::Debugger => {
                let debug_info = self.execute_debug(word_hash)?;
                Ok(ExecutionResult::Debug(debug_info))
            }
        }
    }

    /// Legacy execute for compatibility (runtime mode only)
    pub fn execute_word_hash_runtime(&mut self, word_hash: &hash::WordHash) -> Result<(), RuntimeError> {
        self.execute_runtime(word_hash)
    }

    /// Runtime execution mode
    fn execute_runtime(&mut self, word_hash: &hash::WordHash) -> Result<(), RuntimeError> {
        // Fall back to legacy execution for now
        if let Some(definition) = self.db.get_word(word_hash)? {
            self.execute_body(&definition.body_source)
        } else {
            Err(RuntimeError::ParseError)
        }
    }

    /// Type checking execution mode
    fn execute_type_check(&mut self, word_hash: &hash::WordHash) -> Result<TypeSignature, RuntimeError> {

        // Check if it's a primitive with known type signature
        if word_hash.is_primitive() {
            let prim_id = word_hash.primitive_id().unwrap();
            return self.check_primitive_type(prim_id);
        }

        // For user words, analyze their definition
        if let Some(definition) = self.db.get_word(word_hash)? {
            self.analyze_word_type(&definition.body_source)
        } else {
            Err(RuntimeError::ParseError)
        }
    }

    /// Check type signature of primitive operations
    fn check_primitive_type(&mut self, prim_id: u8) -> Result<TypeSignature, RuntimeError> {
        use hash::primitives::*;

        match prim_id {
            DUP => {
                // dup: ( T -- T T )
                if self.type_analysis_stack.is_empty() {
                    return Err(RuntimeError::TypeError(TypeError::StackUnderflow));
                }
                let top_type = self.type_analysis_stack.last().unwrap().clone();
                self.type_analysis_stack.push(top_type.clone());
                Ok(TypeSignature {
                    inputs: vec![top_type.clone()],
                    outputs: vec![top_type.clone(), top_type],
                })
            }
            DROP => {
                // drop: ( T -- )
                if self.type_analysis_stack.is_empty() {
                    return Err(RuntimeError::TypeError(TypeError::StackUnderflow));
                }
                let dropped_type = self.type_analysis_stack.pop().unwrap();
                Ok(TypeSignature {
                    inputs: vec![dropped_type],
                    outputs: vec![],
                })
            }
            SWAP => {
                // swap: ( T1 T2 -- T2 T1 )
                if self.type_analysis_stack.len() < 2 {
                    return Err(RuntimeError::TypeError(TypeError::StackUnderflow));
                }
                let t2 = self.type_analysis_stack.pop().unwrap();
                let t1 = self.type_analysis_stack.pop().unwrap();
                self.type_analysis_stack.push(t2.clone());
                self.type_analysis_stack.push(t1.clone());
                Ok(TypeSignature {
                    inputs: vec![t1.clone(), t2.clone()],
                    outputs: vec![t2, t1],
                })
            }
            ADD | SUB | MUL | DIV => {
                // arithmetic: ( T T -- T ) where T is numeric
                if self.type_analysis_stack.len() < 2 {
                    return Err(RuntimeError::TypeError(TypeError::StackUnderflow));
                }
                let t2 = self.type_analysis_stack.pop().unwrap();
                let t1 = self.type_analysis_stack.pop().unwrap();

                if t1 != t2 || !self.is_numeric_type(&t1) {
                    return Err(RuntimeError::TypeError(TypeError::Mismatch(t1, t2)));
                }

                self.type_analysis_stack.push(t1.clone());
                Ok(TypeSignature {
                    inputs: vec![t1.clone(), t1.clone()],
                    outputs: vec![t1],
                })
            }
            EQ | GT | LT => {
                // comparison: ( T T -- Boolean ) -> simplified to ( T T -- I64 )
                if self.type_analysis_stack.len() < 2 {
                    return Err(RuntimeError::TypeError(TypeError::StackUnderflow));
                }
                let t2 = self.type_analysis_stack.pop().unwrap();
                let t1 = self.type_analysis_stack.pop().unwrap();

                if t1 != t2 {
                    return Err(RuntimeError::TypeError(TypeError::Mismatch(t1, t2)));
                }

                self.type_analysis_stack.push(ConcreteType::I64); // Boolean result
                Ok(TypeSignature {
                    inputs: vec![t1, t2],
                    outputs: vec![ConcreteType::I64],
                })
            }
            _ => {
                Err(RuntimeError::TypeError(TypeError::InvalidOperation(
                    format!("Unknown primitive: {}", prim_id)
                )))
            }
        }
    }

    /// Check if a type is numeric
    fn is_numeric_type(&self, concrete_type: &ConcreteType) -> bool {
        matches!(concrete_type,
            ConcreteType::I8 | ConcreteType::I16 | ConcreteType::I32 |
            ConcreteType::I64 | ConcreteType::I128 | ConcreteType::F32 |
            ConcreteType::F64 | ConcreteType::BigInt | ConcreteType::BigRational
        )
    }

    /// Analyze type signature of a word body (simplified for now)
    fn analyze_word_type(&mut self, body: &str) -> Result<TypeSignature, RuntimeError> {
        let initial_stack = self.type_analysis_stack.clone();

        // Simplified analysis: assume well-typed for now
        // TODO: Implement full type inference

        let tokens: Vec<&str> = body.split_whitespace().collect();
        for token in tokens {
            match token {
                "dup" => { self.check_primitive_type(hash::primitives::DUP)?; }
                "*" => { self.check_primitive_type(hash::primitives::MUL)?; }
                "+" => { self.check_primitive_type(hash::primitives::ADD)?; }
                "-" => { self.check_primitive_type(hash::primitives::SUB)?; }
                "/" => { self.check_primitive_type(hash::primitives::DIV)?; }
                _ => {
                    // Try to parse as number
                    if let Ok(_num) = token.parse::<i64>() {
                        self.type_analysis_stack.push(ConcreteType::I64);
                    }
                    // TODO: Handle user-defined words
                }
            }
        }

        let final_stack = self.type_analysis_stack.clone();
        Ok(TypeSignature {
            inputs: initial_stack,
            outputs: final_stack,
        })
    }

    /// Placeholder implementations for other execution modes
    fn execute_optimize(&mut self, _word_hash: &hash::WordHash) -> Result<Vec<u8>, RuntimeError> {
        // TODO: Implement optimization
        Ok(vec![0x90, 0x90]) // NOP instructions as placeholder
    }

    fn execute_documentation(&mut self, word_hash: &hash::WordHash) -> Result<String, RuntimeError> {
        if let Some(definition) = self.db.get_word(word_hash)? {
            Ok(format!("Documentation for word: {}", definition.body_source))
        } else {
            Ok("No documentation available".to_string())
        }
    }

    fn execute_profile(&mut self, _word_hash: &hash::WordHash) -> Result<PerformanceStats, RuntimeError> {
        // TODO: Implement profiling
        Ok(PerformanceStats {
            cycles: 42,
            memory_usage: 1024,
        })
    }

    fn execute_debug(&mut self, _word_hash: &hash::WordHash) -> Result<DebugInfo, RuntimeError> {
        // TODO: Implement debugging
        Ok(DebugInfo {
            step_count: 1,
            breakpoints: vec![],
        })
    }

    fn parse_context_declaration(&mut self, input: &str) -> Result<(), RuntimeError> {
        let input = input.trim();

        // Parse "Context condition-word?" or "Context ( default )"
        if input == "Context ( default )" {
            self.current_context = None;
            println!("Set context to default");
        } else if let Some(condition) = input.strip_prefix("Context ") {
            let condition = condition.trim();
            if condition.is_empty() {
                println!("Invalid context declaration. Use: Context condition-word?");
                return Err(RuntimeError::ParseError);
            }
            self.current_context = Some(condition.to_string());
            println!("Set context to: {}", condition);
        } else {
            println!("Invalid context syntax. Use: Context condition-word? or Context ( default )");
            return Err(RuntimeError::ParseError);
        }

        Ok(())
    }

    fn resolve_word_with_context(&mut self, name: &str) -> Result<Option<String>, RuntimeError> {
        let definitions = self.db.get_all_definitions(name, None)?;

        for def in definitions {
            if let Some(context_word) = &def.context_condition {
                // Evaluate the context condition
                if self.evaluate_context_condition(context_word)? {
                    return Ok(Some(def.body_source));
                }
            } else {
                // Default definition (no context) - return if no context-specific match found
                return Ok(Some(def.body_source));
            }
        }

        Ok(None)
    }

    fn evaluate_context_condition(&mut self, context_word: &str) -> Result<bool, RuntimeError> {
        // Save current stack state
        let saved_stack_size = self.value_stack.len();

        // Execute the context word
        let result = self.execute_word(context_word);

        // Check if the word executed successfully and left a value
        if result.is_ok() && self.value_stack.len() > saved_stack_size {
            let (value, _) = self.pop()?;
            // FORTH-style boolean: 0 = false, non-zero = true
            match value {
                Value::I64(n) => Ok(n != 0),
                _ => Ok(true), // Non-integer values are considered true
            }
        } else {
            // If context word fails or leaves no value, context is false
            Ok(false)
        }
    }

    pub fn logical_and(&mut self) -> Result<(), RuntimeError> {
        let (b, _) = self.pop()?;
        let (a, _) = self.pop()?;

        let a_bool = match a {
            Value::I64(n) => n != 0,
            _ => true,
        };
        let b_bool = match b {
            Value::I64(n) => n != 0,
            _ => true,
        };

        let result = if a_bool && b_bool { -1 } else { 0 }; // FORTH-style boolean
        self.push(Value::I64(result), ConcreteType::I64);
        Ok(())
    }

    pub fn equals(&mut self) -> Result<(), RuntimeError> {
        let (b, _) = self.pop()?;
        let (a, _) = self.pop()?;

        let result = if a == b { -1 } else { 0 }; // FORTH-style boolean
        self.push(Value::I64(result), ConcreteType::I64);
        Ok(())
    }

    pub fn greater_than(&mut self) -> Result<(), RuntimeError> {
        let (b, _) = self.pop()?;
        let (a, _) = self.pop()?;

        let result = match (a, b) {
            (Value::I64(x), Value::I64(y)) => if x > y { -1 } else { 0 },
            _ => 0, // Type mismatch defaults to false
        };
        self.push(Value::I64(result), ConcreteType::I64);
        Ok(())
    }
}

pub fn repl() {
    repl_with_database("march.db")
}

pub fn repl_with_database(db_path: &str) {
    println!("March REPL v0.1.0");
    println!("Database: {}", db_path);
    println!("Commands: numbers, +, -, *, /, dup, swap, drop, .s (show stack), : word body ;, quit");

    let mut interp = match Interpreter::with_database(db_path) {
        Ok(interp) => interp,
        Err(e) => {
            println!("Failed to initialize interpreter: {:?}", e);
            return;
        }
    };

    loop {
        print!("march> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();
        if input == "quit" || input == "exit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        // Handle full line for definitions, individual words for everything else
        if input.trim().starts_with(':') {
            if let Err(e) = interp.execute_word(input) {
                println!("Error: {:?}", e);
            }
        } else {
            for word in input.split_whitespace() {
                if let Err(e) = interp.execute_word(word) {
                    println!("Error: {:?}", e);
                    break;
                }
            }
        }
    }
}