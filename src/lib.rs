use std::collections::HashMap;
use std::io::{self, Write};
use num_bigint::BigInt;
use num_rational::BigRational;

pub mod database;
pub mod state;
pub mod hash;

// Thunk system for lazy evaluation and context signatures
#[derive(Debug, Clone, PartialEq)]
pub enum ThunkType {
    General,           // General lazy evaluation - can return anything
    ContextSignature,  // Context signatures - must return [types...] boolean
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThunkDefinition {
    pub body_source: String,
    pub thunk_type: ThunkType,
}

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
    // Type system types
    TypeOf(AbstractType),                           // Type-of-type
    ConstrainedType(AbstractType, Vec<Constraint>), // Constrained type
    // Thunk type
    Thunk,                                          // Lazy evaluation thunk
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I64(i64),
    BigInt(BigInt),
    BigRational(BigRational),
    String(String),
    Array(Vec<Value>),
    Ptr(usize),
    // Type-of-types for constraint building
    TypeOf(AbstractType),                           // Type-of-type (Int', String', etc)
    ConstrainedType(AbstractType, Vec<Constraint>), // Constrained type
    // Thunk for lazy evaluation
    Thunk(ThunkDefinition),                         // Lazy evaluation thunk
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    GreaterThan(Value),
    GreaterThanOrEqual(Value),
    LessThan(Value),
    LessThanOrEqual(Value),
    OneOf(Vec<Value>),
    // Future: custom constraint words
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorType {
    pub name: String,
    pub parent: Option<String>,  // For hierarchy: extends parent
}

pub struct ErrorRegistry {
    errors: HashMap<String, ErrorType>,
}

impl ErrorRegistry {
    pub fn new() -> Self {
        let mut registry = ErrorRegistry {
            errors: HashMap::new(),
        };

        // Add built-in error types
        registry.register_builtin_errors();
        registry
    }

    fn register_builtin_errors(&mut self) {
        // Root error type
        self.register_error("Error", None);

        // Common error categories
        self.register_error("MathError", Some("Error"));
        self.register_error("DivisionByZero", Some("MathError"));
        self.register_error("Overflow", Some("MathError"));

        self.register_error("NetworkError", Some("Error"));
        self.register_error("NetworkTimeout", Some("NetworkError"));
        self.register_error("ConnectionLost", Some("NetworkError"));
    }

    pub fn register_error(&mut self, name: &str, parent: Option<&str>) {
        let error_type = ErrorType {
            name: name.to_string(),
            parent: parent.map(|p| p.to_string()),
        };
        self.errors.insert(name.to_string(), error_type);
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.errors.contains_key(name)
    }

    pub fn get_parent(&self, name: &str) -> Option<String> {
        self.errors.get(name).and_then(|e| e.parent.clone())
    }

    pub fn is_subtype_of(&self, child: &str, parent: &str) -> bool {
        if child == parent {
            return true;
        }

        if let Some(child_parent) = self.get_parent(child) {
            self.is_subtype_of(&child_parent, parent)
        } else {
            false
        }
    }
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
    // Future: Heap-based memory management
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            // Future: Initialize heap and address management
        }
    }
}

pub struct Interpreter {
    pub value_stack: Vec<Value>,
    type_stack: Vec<ConcreteType>,
    // memory: Memory,  // Future: Heap-based memory management
    pub db: database::Database,
    pub state: state::ProgramState,
    current_context: Option<String>,
    current_executing_word: Option<String>,  // Track current word for re-dispatch
    pub runtime: hash::Runtime,  // Hash-based execution engine
    execution_mode: ExecutionMode,  // Multi-modal execution
    pub type_analysis_stack: Vec<ConcreteType>,  // For type checking mode
    pub errors: ErrorRegistry,  // Error type system
}

impl Interpreter {
    pub fn new() -> Result<Self, RuntimeError> {
        let mut interp = Interpreter {
            value_stack: Vec::new(),
            type_stack: Vec::new(),
            // memory: Memory::new(),  // Future: Heap-based memory management
            db: database::Database::in_memory()?,
            state: state::ProgramState::new(),
            current_context: None,
            current_executing_word: None,
            runtime: hash::Runtime::new(),
            execution_mode: ExecutionMode::Runtime,
            type_analysis_stack: Vec::new(),
            errors: ErrorRegistry::new(),
        };
        interp.initialize_standard_operators()?;
        Ok(interp)
    }

    pub fn with_database(db_path: &str) -> Result<Self, RuntimeError> {
        let mut interp = Interpreter {
            value_stack: Vec::new(),
            type_stack: Vec::new(),
            // memory: Memory::new(),  // Future: Heap-based memory management
            db: database::Database::new(db_path)?,
            state: state::ProgramState::new(),
            current_context: None,
            current_executing_word: None,
            runtime: hash::Runtime::new(),
            execution_mode: ExecutionMode::Runtime,
            type_analysis_stack: Vec::new(),
            errors: ErrorRegistry::new(),
        };
        interp.initialize_standard_operators()?;
        Ok(interp)
    }

    /// Initialize standard operators as extensible March words
    fn initialize_standard_operators(&mut self) -> Result<(), RuntimeError> {
        use crate::hash::primitives::*;

        // For now, define operators without contexts (default behavior)
        // Future: These will become context-aware for type dispatch

        // Create primitive hashes (these are already registered in runtime)
        let add_hash = crate::hash::WordHash::primitive(ADD);
        let sub_hash = crate::hash::WordHash::primitive(SUB);
        let mul_hash = crate::hash::WordHash::primitive(MUL);
        let div_hash = crate::hash::WordHash::primitive(DIV);

        // Alias operators to primitive hashes (making them extensible)
        self.db.alias_word("+", None, &add_hash)?;
        self.db.alias_word("-", None, &sub_hash)?;
        self.db.alias_word("*", None, &mul_hash)?;
        self.db.alias_word("/", None, &div_hash)?;

        Ok(())
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

    /// Force a thunk - execute its body and return the result
    pub fn force_thunk(&mut self, thunk: &ThunkDefinition) -> Result<Value, RuntimeError> {
        // Save current stack state to isolate thunk execution
        let saved_stack_size = self.value_stack.len();

        // Execute thunk body in current environment
        self.execute_body(&thunk.body_source)?;

        // For general thunks, return the top of stack (if any)
        match thunk.thunk_type {
            ThunkType::General => {
                if self.value_stack.len() > saved_stack_size {
                    let (result, _) = self.pop()?;
                    Ok(result)
                } else {
                    // Thunk didn't produce a value
                    Ok(Value::I64(0)) // Default/unit value
                }
            }
            ThunkType::ContextSignature => {
                // Context signature thunks must return [types...] boolean
                // For now, return a simple success indicator
                // TODO: Implement proper type signature extraction
                Ok(Value::I64(1)) // Success for now
            }
        }
    }

    /// Check if a value is a thunk and force it if needed
    pub fn force_if_thunk(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Thunk(thunk_def) => self.force_thunk(&thunk_def),
            other => Ok(other),
        }
    }

    /// Force the thunk on top of the stack
    pub fn force_top_thunk(&mut self) -> Result<(), RuntimeError> {
        let (value, _) = self.pop()?;

        match value {
            Value::Thunk(thunk_def) => {
                let result = self.force_thunk(&thunk_def)?;
                let result_type = self.infer_concrete_type_from_value(&result);
                self.push(result, result_type);
                println!("Forced thunk, result on stack");
                Ok(())
            }
            _ => {
                // Not a thunk, just put it back
                let value_type = self.infer_concrete_type_from_value(&value);
                self.push(value, value_type);
                println!("Value is not a thunk");
                Ok(())
            }
        }
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

        // Handle state declarations with $
        if input.trim().starts_with('$') {
            return self.parse_state_declaration(input);
        }

        // Handle context declarations with ?
        if input.trim().starts_with('?') {
            return self.parse_context_declaration(input);
        }

        // Handle error declarations with Error:
        if input.trim().starts_with("Error:") {
            return self.parse_error_declaration(input);
        }

        // Handle thunk creation with { }
        if input.trim().starts_with('{') && input.trim().ends_with('}') {
            return self.parse_thunk_creation(input);
        }

        // Handle array creation with [ ]
        if input.trim().starts_with('[') && input.trim().ends_with(']') {
            return self.parse_array_creation(input);
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
            "dup" => self.dup(),
            "swap" => self.swap(),
            "drop" => self.drop(),
            ".s" => { self.print_stack(); Ok(()) },
            "words" => { self.list_words(); Ok(()) },
            ".state" => { self.print_state(); Ok(()) },
            "&" => self.logical_and(),
            "=" => self.equals(),
            ">" => self.greater_than_or_constraint(),
            "<" => self.less_than_or_constraint(),
            // Boolean literals
            "true" => {
                self.push(Value::I64(-1), ConcreteType::I64);
                Ok(())
            },
            "false" => {
                self.push(Value::I64(0), ConcreteType::I64);
                Ok(())
            },
            // Type-of-type words
            "Int" => self.push_int_type(),
            "String" => self.push_string_type(),
            "Rational" => self.push_rational_type(),
            // Control flow
            "if" => self.if_condition(),
            "then" => Ok(()),  // No-op for now
            // Error handling
            "raise" => self.raise_error(),
            // Thunk operations
            "force" => self.force_top_thunk(),
            _ => {
                // Try revolutionary type-based dispatch first!
                if let Some(context) = self.find_matching_context(input)? {
                    if let Some(word_hash) = self.db.find_word_hash(input, Some(&context))? {
                        // Track current word for error re-dispatch
                        self.current_executing_word = Some(input.to_string());

                        println!("🚀 Type dispatch: {} with context: {:?}", input, context);

                        // Use runtime execution
                        let result = self.execute_word_hash_runtime(&word_hash);

                        // Clear tracking after execution
                        self.current_executing_word = None;

                        return result;
                    }
                }

                // Fall back to legacy lookups
                if let Ok(n) = input.parse::<i64>() {
                    // Try to parse as number
                    self.push(Value::I64(n), ConcreteType::I64);
                    Ok(())
                } else if self.errors.is_registered(input) {
                    // Push registered error names as strings
                    self.push(Value::String(input.to_string()), ConcreteType::String);
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

        // Compile the word body to hash sequence with user-word resolution
        let definition = self.compile_word_body(&body)?;
        let word_hash = hash::WordHash::content_hash(&definition);

        // Store the execution token in runtime
        self.runtime.hash_to_xt.insert(word_hash.clone(), hash::ExecutionToken::UserWord(definition.clone()));

        // Get context after mutable operations
        let context = self.current_context.as_deref();

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

            // Check for array literal start
            if word == "[" {
                // Find matching ]
                let mut j = i + 1;
                let mut bracket_count = 1;
                while j < words.len() && bracket_count > 0 {
                    if words[j] == "[" {
                        bracket_count += 1;
                    } else if words[j] == "]" {
                        bracket_count -= 1;
                    }
                    j += 1;
                }

                if bracket_count == 0 {
                    // Found complete array literal
                    let array_tokens = &words[i..j];
                    let array_literal = array_tokens.join(" ");
                    self.parse_array_creation(&array_literal)?;
                    i = j; // Skip to after the ]
                    continue;
                } else {
                    return Err(RuntimeError::ParseError); // Unmatched [
                }
            }

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
        // Parse: "$ page_number Int 0 > 100 <"
        let input = input.trim();

        // Remove "$" prefix
        let declaration = input.strip_prefix('$').unwrap().trim();

        // Split by whitespace
        let tokens: Vec<&str> = declaration.split_whitespace().collect();
        if tokens.len() < 2 {
            println!("Invalid state declaration. Use: $ name Type [constraints...]");
            return Err(RuntimeError::ParseError);
        }

        let name = tokens[0];
        let type_name = tokens[1];

        // Parse base type
        let abstract_type = match type_name {
            "Integer" | "Int" => AbstractType::Int,
            "Rational" => AbstractType::Rational,
            "String" => AbstractType::String,
            _ => {
                println!("Unknown type: {}", type_name);
                return Err(RuntimeError::ParseError);
            }
        };

        // Parse constraints from remaining tokens
        let constraints = self.parse_constraints(&tokens[2..])?;

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

        let constraint_count = constraints.len();
        self.state.declare_variable(name, abstract_type, constraints, initial_value)?;
        println!("Declared state variable: {} with {} constraints", name, constraint_count);
        Ok(())
    }

    fn parse_error_declaration(&mut self, input: &str) -> Result<(), RuntimeError> {
        // Parse: "Error: DivisionByZero extends MathError ;"
        let input = input.trim();

        // Remove "Error:" prefix
        let declaration = input.strip_prefix("Error:").unwrap().trim();

        // Check for trailing semicolon
        let declaration = if declaration.ends_with(';') {
            declaration.strip_suffix(';').unwrap().trim()
        } else {
            declaration
        };

        // Split by whitespace
        let tokens: Vec<&str> = declaration.split_whitespace().collect();
        if tokens.is_empty() {
            println!("Invalid error declaration. Use: Error: ErrorName [extends ParentError] ;");
            return Err(RuntimeError::ParseError);
        }

        let error_name = tokens[0];
        let parent = if tokens.len() >= 3 && tokens[1] == "extends" {
            Some(tokens[2])
        } else {
            None
        };

        // Register the error type
        self.errors.register_error(error_name, parent);

        if let Some(parent_name) = parent {
            println!("Declared error: {} extends {}", error_name, parent_name);
        } else {
            println!("Declared error: {}", error_name);
        }

        Ok(())
    }

    /// Parse constraint expressions from tokens
    /// Example: ["0", ">", "total_pages", "@", "<"]
    /// Becomes: [GreaterThan(0), LessThan(dynamic_ref(total_pages))]
    fn parse_constraints(&self, tokens: &[&str]) -> Result<Vec<state::Constraint>, RuntimeError> {
        let mut constraints = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            // Look for constraint patterns
            if i + 1 < tokens.len() {
                let value_str = tokens[i];
                let operator = tokens[i + 1];

                // Parse the value (could be literal or variable reference)
                let value = if let Ok(num) = value_str.parse::<i64>() {
                    Value::I64(num)
                } else {
                    // For now, treat as literal 0 - TODO: handle variable references
                    println!("Warning: Variable references in constraints not yet implemented: {}", value_str);
                    Value::I64(0)
                };

                // Parse the operator and create constraint
                match operator {
                    ">" => {
                        constraints.push(state::Constraint::GreaterThan(value));
                        i += 2;
                    }
                    ">=" => {
                        constraints.push(state::Constraint::GreaterThanOrEqual(value));
                        i += 2;
                    }
                    "<" => {
                        constraints.push(state::Constraint::LessThan(value));
                        i += 2;
                    }
                    "<=" => {
                        constraints.push(state::Constraint::LessThanOrEqual(value));
                        i += 2;
                    }
                    _ => {
                        // Unknown operator, skip this token
                        i += 1;
                    }
                }
            } else {
                // Single token, skip
                i += 1;
            }
        }

        Ok(constraints)
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
            Value::TypeOf(abstract_type) => ConcreteType::TypeOf(abstract_type.clone()),
            Value::ConstrainedType(abstract_type, constraints) => {
                ConcreteType::ConstrainedType(abstract_type.clone(), constraints.clone())
            }
            Value::Thunk(_) => ConcreteType::Thunk,
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
        // Check if it's a primitive - handle directly for common math operations
        if word_hash.is_primitive() {
            if let Some(prim_id) = word_hash.primitive_id() {
                use crate::hash::primitives::*;
                match prim_id {
                    ADD => return self.add(),
                    SUB => return self.sub(),
                    MUL => return self.mul(),
                    DIV => return self.div(),
                    DUP => return self.dup(),
                    DROP => return self.drop(),
                    SWAP => return self.swap(),
                    EQ => return self.equals(),
                    GT => return self.greater_than(),
                    AND => return self.logical_and(),
                    _ => {} // Fall through for other primitives
                }
            }
        }

        // Fall back to database lookup for user words
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

        // Handle context setting with thunks
        if input == "?" {
            return self.set_context_from_thunk();
        }

        // Parse "? default"
        if input == "? default" {
            self.current_context = None;
            println!("Set context to default");
            return Ok(());
        }

        // Legacy support for simple context names
        if let Some(condition) = input.strip_prefix("? ") {
            let condition = condition.trim();
            if condition.is_empty() {
                println!("Invalid context declaration. Use: ? (with thunk on stack) or ? default");
                return Err(RuntimeError::ParseError);
            }
            self.current_context = Some(condition.to_string());
            println!("Set context to: {}", condition);
            return Ok(());
        }

        println!("Invalid context syntax. Use: ? (with thunk on stack) or ? default");
        Err(RuntimeError::ParseError)
    }

    /// Get current stack types as a signature string for type dispatch
    fn get_current_stack_signature(&self, depth: usize) -> String {
        let mut types = Vec::new();
        let available = std::cmp::min(depth, self.type_stack.len());

        // Get types from top of stack
        for i in 0..available {
            let stack_index = self.type_stack.len() - 1 - i;
            let concrete_type = &self.type_stack[stack_index];
            let type_name = match concrete_type {
                ConcreteType::I64 => "I64",
                ConcreteType::String => "String",
                ConcreteType::BigRational => "Rational",
                ConcreteType::Array { .. } => "Array",
                ConcreteType::Thunk => "Thunk",
                _ => "Unknown", // Handle other types
            };
            types.push(type_name);
        }

        types.join(" ")
    }

    /// Find the best matching context signature for current stack types
    fn find_matching_context(&mut self, word_name: &str) -> Result<Option<String>, RuntimeError> {
        // Get current stack types (we'll start with 3 elements for typical binary operations)
        let current_types = self.get_current_stack_signature(3);

        println!("🔍 Looking for {} with stack types: [{}]", word_name, current_types);

        // For now, try exact match against known context signatures
        // This is a simplified implementation - in the future we could query the database
        // for all contexts and do sophisticated matching

        // Try current context first (if it matches our stack)
        if let Some(context) = &self.current_context {
            println!("   Trying current context: {}", context);
            if current_types.starts_with(context) || context.contains(&current_types) {
                if let Some(_) = self.db.find_word_hash(word_name, Some(context))? {
                    println!("   ✅ Found match in current context: {}", context);
                    return Ok(Some(context.clone()));
                }
            }
        }

        // Try to find a context that matches our stack types exactly
        // For now, we'll try common patterns
        let possible_contexts = vec![
            format!("{} {}", current_types, current_types.split_whitespace().last().unwrap_or("I64")), // Binary op pattern
            current_types.clone(), // Exact match
            "I64 I64 I64".to_string(), // Common I64 binary pattern
            "String String String".to_string(), // Common String binary pattern
        ];

        for candidate_context in possible_contexts {
            println!("   Trying candidate context: {}", candidate_context);
            if let Some(_) = self.db.find_word_hash(word_name, Some(&candidate_context))? {
                println!("   ✅ Found match with context: {}", candidate_context);
                return Ok(Some(candidate_context));
            }
        }

        // Try default context
        println!("   Trying default context");
        if let Some(_) = self.db.find_word_hash(word_name, None)? {
            println!("   ✅ Found in default context");
            return Ok(None);
        }

        println!("   ❌ No matching context found");
        Ok(None)
    }

    /// Set context signature from thunk on stack
    /// Expects thunk that returns: [ types... ] boolean
    fn set_context_from_thunk(&mut self) -> Result<(), RuntimeError> {
        // Check if there's a thunk on the stack
        if self.value_stack.is_empty() {
            println!("Error: ? requires a thunk on the stack");
            return Err(RuntimeError::ParseError);
        }

        // Get the top value (should be a thunk)
        let (top_value, _) = self.pop()?;
        match top_value {
            Value::Thunk(thunk) => {
                // Execute thunk body directly (don't use force_thunk which pops the result)
                println!("Forcing context signature thunk...");
                self.execute_body(&thunk.body_source)?;

                // Expect boolean on top, array below
                if self.value_stack.len() < 2 {
                    println!("Error: Context thunk must return [types...] boolean");
                    return Err(RuntimeError::ParseError);
                }

                let (boolean_val, _) = self.pop()?;
                let (array_val, _) = self.pop()?;

                // Validate we got the right types
                let condition = match boolean_val {
                    Value::I64(n) => n != 0, // FORTH-style: -1 is true, 0 is false
                    _ => {
                        println!("Error: Expected boolean from context thunk");
                        return Err(RuntimeError::ParseError);
                    }
                };

                let type_signature = match array_val {
                    Value::Array(types) => {
                        // Convert array elements to type signature string
                        let mut sig_parts = Vec::new();
                        let mut found_separator = false;

                        for element in types {
                            match element {
                                Value::String(s) if s == "--" => {
                                    found_separator = true;
                                    sig_parts.push("--".to_string());
                                }
                                Value::TypeOf(abstract_type) => {
                                    let type_name = match abstract_type {
                                        crate::AbstractType::Int => "I64",
                                        crate::AbstractType::String => "String",
                                        crate::AbstractType::Rational => "Rational",
                                        crate::AbstractType::Array(_) => "Array",
                                        crate::AbstractType::Ptr(_) => "Ptr",
                                    };
                                    sig_parts.push(type_name.to_string());
                                }
                                Value::String(s) => {
                                    sig_parts.push(s);
                                }
                                _ => {
                                    println!("Warning: Unexpected element in type signature array");
                                }
                            }
                        }
                        sig_parts.join(" ")
                    }
                    _ => {
                        println!("Error: Expected array from context thunk");
                        return Err(RuntimeError::ParseError);
                    }
                };

                if condition {
                    self.current_context = Some(type_signature.clone());
                    println!("✅ Set context signature: [ {} ]", type_signature);
                } else {
                    println!("❌ Context condition failed: [ {} ]", type_signature);
                }

                Ok(())
            }
            _ => {
                println!("Error: ? requires a thunk on the stack, got {:?}", top_value);
                Err(RuntimeError::ParseError)
            }
        }
    }

    /// Parse thunk creation: { body }
    fn parse_thunk_creation(&mut self, input: &str) -> Result<(), RuntimeError> {
        let input = input.trim();

        // Extract body between { and }
        if input.len() < 2 {
            println!("Invalid thunk syntax. Use: {{ body }}");
            return Err(RuntimeError::ParseError);
        }

        let body = &input[1..input.len()-1].trim();

        // Create thunk definition
        let thunk = ThunkDefinition {
            body_source: body.to_string(),
            thunk_type: ThunkType::General,
        };

        // Push thunk onto stack
        self.push(Value::Thunk(thunk), ConcreteType::Thunk);
        println!("Created thunk: {{{}}}", body);

        Ok(())
    }

    /// Parse array creation: [ item item item ]
    fn parse_array_creation(&mut self, input: &str) -> Result<(), RuntimeError> {
        let input = input.trim();

        // Extract content between [ and ]
        if input.len() < 2 {
            println!("Invalid array syntax. Use: [ item item ... ]");
            return Err(RuntimeError::ParseError);
        }

        let content = &input[1..input.len()-1].trim();

        // Handle empty array
        if content.is_empty() {
            let empty_array = Value::Array(Vec::new());
            self.push(empty_array, ConcreteType::Array {
                element_type: Box::new(ConcreteType::I64), // Default element type
                size: 0
            });
            println!("Created empty array: []");
            return Ok(());
        }

        // Parse array elements
        let mut array_elements = Vec::new();
        let tokens: Vec<&str> = content.split_whitespace().collect();
        let token_count = tokens.len();

        for token in tokens {
            // Try to parse as number first
            if let Ok(n) = token.parse::<i64>() {
                array_elements.push(Value::I64(n));
            } else {
                // Try to parse as other types or references
                match token {
                    "I64" => array_elements.push(Value::TypeOf(crate::AbstractType::Int)),
                    "String" => array_elements.push(Value::TypeOf(crate::AbstractType::String)),
                    "Rational" => array_elements.push(Value::TypeOf(crate::AbstractType::Rational)),
                    _ => {
                        // Treat as string literal for now
                        array_elements.push(Value::String(token.to_string()));
                    }
                }
            }
        }

        // Create array value
        let array = Value::Array(array_elements);
        let array_type = ConcreteType::Array {
            element_type: Box::new(ConcreteType::I64), // TODO: Better type inference
            size: token_count
        };

        self.push(array, array_type);
        println!("Created array: [{}]", content);

        Ok(())
    }

    /// Parse context signature declaration: ? [ I64 I64 -- I64 ] condition ;
    fn parse_context_signature_declaration(&mut self, input: &str) -> Result<(), RuntimeError> {
        let input = input.trim();

        // Extract signature between [ and ]
        let start = input.find('[').ok_or(RuntimeError::ParseError)?;
        let end = input.find(']').ok_or(RuntimeError::ParseError)?;

        if start >= end {
            println!("Invalid signature syntax. Use: ? [ input-types -- output-types ]");
            return Err(RuntimeError::ParseError);
        }

        let signature_str = &input[start+1..end].trim();
        let after_bracket = &input[end+1..].trim();

        // Parse optional condition after ] (but not the semicolon)
        let condition = if after_bracket.is_empty() || *after_bracket == ";" {
            None
        } else {
            // Remove trailing semicolon if present
            let cond = after_bracket.strip_suffix(';').unwrap_or(after_bracket);
            if cond.trim().is_empty() {
                None
            } else {
                Some(cond.trim().to_string())
            }
        };

        // Create context signature thunk
        let thunk_body = if let Some(cond) = &condition {
            format!("{} [ {} ]", cond, signature_str)
        } else {
            format!("[ {} ]", signature_str)
        };

        let context_thunk = ThunkDefinition {
            body_source: thunk_body,
            thunk_type: ThunkType::ContextSignature,
        };

        // Store as current context (for now, we'll use the signature as context name)
        let context_name = format!("sig:{}", signature_str);
        self.current_context = Some(context_name.clone());

        println!("Created context signature: [ {} ]", signature_str);
        if let Some(cond) = condition {
            println!("  with condition: {}", cond);
        }

        Ok(())
    }

    #[allow(dead_code)]  // Future: Advanced context resolution
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

    #[allow(dead_code)]  // Future: Advanced context conditions
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

    /// Smart > operator: comparison for values, constraint building for types
    pub fn greater_than_or_constraint(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (&a, &b) {
            // Type-of-type + value = constrained type
            (Value::TypeOf(base_type), value) => {
                let constraint = Constraint::GreaterThan(value.clone());
                let constraints = vec![constraint.clone()];
                let constrained = Value::ConstrainedType(base_type.clone(), constraints.clone());
                self.push(constrained, ConcreteType::ConstrainedType(base_type.clone(), constraints));
                Ok(())
            }
            // Constrained type + value = add another constraint
            (Value::ConstrainedType(base_type, constraints), value) => {
                let mut new_constraints = constraints.clone();
                new_constraints.push(Constraint::GreaterThan(value.clone()));
                let constrained = Value::ConstrainedType(base_type.clone(), new_constraints.clone());
                self.push(constrained, ConcreteType::ConstrainedType(base_type.clone(), new_constraints));
                Ok(())
            }
            // Regular values = comparison
            _ => {
                self.push(a, a_type);
                self.push(b, b_type);
                self.greater_than()
            }
        }
    }

    /// Smart < operator: comparison for values, constraint building for types
    pub fn less_than_or_constraint(&mut self) -> Result<(), RuntimeError> {
        let (b, b_type) = self.pop()?;
        let (a, a_type) = self.pop()?;

        match (&a, &b) {
            // Type-of-type + value = constrained type
            (Value::TypeOf(base_type), value) => {
                let constraint = Constraint::LessThan(value.clone());
                let constraints = vec![constraint.clone()];
                let constrained = Value::ConstrainedType(base_type.clone(), constraints.clone());
                self.push(constrained, ConcreteType::ConstrainedType(base_type.clone(), constraints));
                Ok(())
            }
            // Constrained type + value = add another constraint
            (Value::ConstrainedType(base_type, constraints), value) => {
                let mut new_constraints = constraints.clone();
                new_constraints.push(Constraint::LessThan(value.clone()));
                let constrained = Value::ConstrainedType(base_type.clone(), new_constraints.clone());
                self.push(constrained, ConcreteType::ConstrainedType(base_type.clone(), new_constraints));
                Ok(())
            }
            // Regular values = comparison (would need less_than implementation)
            _ => {
                self.push(a, a_type);
                self.push(b, b_type);
                // For now, return error since we don't have less_than implemented
                Err(RuntimeError::ParseError)
            }
        }
    }

    /// Push Int type-of-type onto stack
    pub fn push_int_type(&mut self) -> Result<(), RuntimeError> {
        self.push(Value::TypeOf(AbstractType::Int), ConcreteType::TypeOf(AbstractType::Int));
        Ok(())
    }

    /// Push String type-of-type onto stack
    pub fn push_string_type(&mut self) -> Result<(), RuntimeError> {
        self.push(Value::TypeOf(AbstractType::String), ConcreteType::TypeOf(AbstractType::String));
        Ok(())
    }

    /// Push Rational type-of-type onto stack
    pub fn push_rational_type(&mut self) -> Result<(), RuntimeError> {
        self.push(Value::TypeOf(AbstractType::Rational), ConcreteType::TypeOf(AbstractType::Rational));
        Ok(())
    }

    /// REVOLUTIONARY: Raise error with automatic context dispatch
    /// Usage: ErrorName raise
    /// This changes context and re-dispatches the calling word!
    pub fn raise_error(&mut self) -> Result<(), RuntimeError> {
        // Pop error name from stack
        let (error_value, _) = self.pop()?;

        let error_name = match error_value {
            Value::String(name) => name,
            _ => {
                println!("Error: raise expects error name on stack");
                return Err(RuntimeError::ParseError);
            }
        };

        // Verify error is registered
        if !self.errors.is_registered(&error_name) {
            println!("Error: unknown error type '{}'", error_name);
            return Err(RuntimeError::ParseError);
        }

        // Change context to error context
        self.current_context = Some(error_name.clone());
        println!("Raised error: {} (context changed)", error_name);

        // THE MAGIC: Re-dispatch current word in new context
        // If we're currently executing 'foo' and raise 'DivisionByZero',
        // look for 'foo' defined in DivisionByZero context and call it
        if let Some(current_word) = self.current_executing_word.clone() {
            println!("  Looking for '{}' in context '{}'...", current_word, error_name);

            // Try to find word in error context
            if let Ok(Some(_)) = self.db.lookup_word(&current_word, Some(&error_name)) {
                println!("  Found! Re-dispatching '{}' in error context", current_word);

                // Execute the context-specific version
                let result = self.execute_word(&current_word);

                // Clear current context after handling
                self.current_context = None;

                return result;
            } else {
                println!("  No error handler found for '{}' in context '{}'", current_word, error_name);
                // Clear context and continue normally
                self.current_context = None;
            }
        }

        Ok(())
    }

    fn if_condition(&mut self) -> Result<(), RuntimeError> {
        // For simple demo - if TOS is non-zero, continue; if zero, skip next word
        let (condition, _) = self.pop()?;

        match condition {
            Value::I64(0) => {
                // Condition is false - for demo purposes, just continue
                // In a full implementation, this would skip to 'else' or 'then'
                Ok(())
            }
            _ => {
                // Condition is true - continue execution
                Ok(())
            }
        }
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