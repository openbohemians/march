use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;
use im::Vector as ImVector;

// Linked list node for stack with structural sharing
#[derive(Debug, Clone)]
struct StackNode {
    value: Value,
    typ: Type,
    next: Option<Rc<StackNode>>,
}

// Stack head wrapper
#[derive(Debug, Clone)]
struct Stack {
    head: Option<Rc<StackNode>>,
    len: usize,
}

impl Stack {
    fn new() -> Self {
        Stack { head: None, len: 0 }
    }

    fn push(&mut self, value: Value, typ: Type) {
        let node = StackNode {
            value,
            typ,
            next: self.head.take(),
        };
        self.head = Some(Rc::new(node));
        self.len += 1;
    }

    fn pop(&mut self) -> Option<(Value, Type)> {
        self.head.take().map(|node| {
            self.head = node.next.clone();
            self.len -= 1;
            (node.value.clone(), node.typ.clone())
        })
    }

    fn peek(&self) -> Option<(&Value, &Type)> {
        self.head.as_ref().map(|node| (&node.value, &node.typ))
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn len(&self) -> usize {
        self.len
    }

    // Create a copy of the stack head for constraint evaluation
    // This shares the tail structure (COW-like)
    fn fork(&self) -> Self {
        Stack {
            head: self.head.clone(),
            len: self.len,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Type {
    I64,
    Quotation,
    String,
    Symbol,
    Array(Box<Type>),  // Homogeneous arrays of a single type
    Tuple(Vec<Type>),  // Heterogeneous tuples
    ArrayMarker,  // Marker for collection construction (matches Value::ArrayMarker)
    TypeVar(String),  // Type variable (e.g., 'a', 'b')
    Named(String),    // Named concrete type (e.g., 'string', 'int')
    // Future extensions:
    // Struct(String),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::I64 => write!(f, "i64"),
            Type::Quotation => write!(f, "quot"),
            Type::String => write!(f, "str"),
            Type::Symbol => write!(f, "sym"),
            Type::Array(elem_type) => write!(f, "[{}]", elem_type),
            Type::Tuple(types) => {
                write!(f, "(")?;
                for (i, t) in types.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            Type::ArrayMarker => write!(f, "{{"),
            Type::TypeVar(name) => write!(f, "{}", name),
            Type::Named(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone)]
enum Value {
    Number(i64),
    Quotation(Vec<Word>),
    String(String),
    Symbol(String),  // For (symbol) syntax if needed
    Array(Vec<Value>),  // Mutable array (for stack operations)
    PersistentArray(ImVector<Value>),  // Immutable array (for state storage)
    Tuple(Vec<Value>),  // Heterogeneous tuple of values
    ErrorValue(String),  // Error value with type name (e.g., "DivideByZero")
    ArrayMarker,  // Marker for runtime collection construction
}

impl Value {
    fn get_type(&self) -> Type {
        match self {
            Value::Number(_) => Type::I64,
            Value::Quotation(_) => Type::Quotation,
            Value::String(_) => Type::String,
            Value::Symbol(_) => Type::Symbol,
            Value::Array(elements) => {
                if elements.is_empty() {
                    // Empty array - use a generic placeholder
                    Type::Array(Box::new(Type::I64))
                } else {
                    Type::Array(Box::new(elements[0].get_type()))
                }
            }
            Value::PersistentArray(elements) => {
                if elements.is_empty() {
                    Type::Array(Box::new(Type::I64))
                } else {
                    Type::Array(Box::new(elements[0].get_type()))
                }
            }
            Value::Tuple(elements) => {
                Type::Tuple(elements.iter().map(|v| v.get_type()).collect())
            }
            Value::ErrorValue(type_name) => Type::Named(type_name.clone()),
            Value::ArrayMarker => panic!("ArrayMarker should not have a type - it's internal only"),
        }
    }
}

#[derive(Debug, Clone)]
enum Word {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,

    // Stack manipulation
    Dup,
    Drop,
    Swap,
    Over,
    Rot,

    // Comparison (return -1 for true, 0 for false)
    Eq,
    Lt,
    Gt,
    Lte,
    Gte,

    // Logic
    And,
    Or,
    Not,

    // I/O
    Dot,       // Print TOS
    DotTypes,  // Print type stack (.types)

    // String operations
    Concat,    // Concatenate two strings (++)
    StrLen,    // Get string length (str.len)

    // State operations
    Arrow(String),      // -> varname - Pop and store to state variable (immediate word)
    DoubleArrow(String), // => varname - Copy TOS and store to state variable (immediate word)

    // Collection operations
    ArrayLen,  // Get array length (array.len)
    ArrayGet,  // Get element at index (array.get)
    ArraySet,  // Set element at index (array.set)
    ArrayBegin,  // { - Push array marker
    ArrayEnd,    // } - Collect values above marker into collection
    ToMutable,   // Convert persistent array to mutable array (to-mutable)

    // Control flow
    Times,      // Execute quotation N times (#do)
    Call,       // Execute quotation once (eval)
    If,         // Execute one of two quotations based on condition (if)
    LoopIndex,  // i0
    Raise(String),  // Raise an error with given type name

    // Literals
    Number(i64),
    Quote(Vec<Word>), // Push quotation onto stack
    StringLit(String), // Push string onto stack
    SymbolLit(String), // Push symbol onto stack
    VarFetch(String), // Fetch value from state variable (auto-deref)

    // User-defined words
    UserDefined(Vec<Word>),        // Direct execution (inline)
    NamedWord(String),             // Runtime dispatch by name (for multi-methods)
    WildcardDispatch,              // [*] - Redispatch using current_word
}

#[derive(Debug, Clone)]
struct Signature {
    inputs: Vec<Type>,
    outputs: Vec<Type>,
    constraint: Option<Vec<Word>>,  // Optional ? guard expression
}

struct Forth {
    stack: Stack,  // Linked list stack with structural sharing
    dictionary: HashMap<String, Vec<(Signature, Word)>>,  // Multi-methods: name -> [(sig, impl)]
    immediate_words: std::collections::HashSet<String>,  // Words that execute at compile-time
    state: HashMap<String, (Type, Value)>,  // State variables with type and value
    state_tempmap: Option<HashMap<String, (Type, Value)>>,  // Temporary overlay for constraint evaluation
    type_hierarchy: HashMap<String, String>,  // Type name -> parent type name (for subtyping)
    namespaces: std::collections::HashSet<String>,  // Registered namespace names
    current_word: Option<String>,  // Currently executing word name (for error handling)
    current_signature: Option<Signature>,    // Active signature context
    saved_stack_head: Option<Stack>,  // Stack state at word entry (for error handling)
    compiling: bool,
    current_definition: Vec<Word>,
    loop_indices: Vec<i64>, // Stack of loop counters
    recursion_depth: usize,  // Track recursion depth
    max_recursion_depth: usize,  // Maximum allowed recursion depth
    iteration_count: usize,  // Track total iterations in loops
    max_iterations: usize,  // Maximum allowed iterations
    test_failed: bool,  // Track if any test has failed
}

impl Forth {
    fn new() -> Self {
        let mut forth = Forth {
            stack: Stack::new(),
            dictionary: HashMap::new(),
            immediate_words: std::collections::HashSet::new(),
            state: HashMap::new(),
            state_tempmap: None,
            type_hierarchy: HashMap::new(),
            namespaces: std::collections::HashSet::new(),
            current_word: None,
            current_signature: None,
            saved_stack_head: None,
            compiling: false,
            current_definition: Vec::new(),
            loop_indices: Vec::new(),
            recursion_depth: 0,
            max_recursion_depth: 1000,  // Prevent stack overflow
            iteration_count: 0,
            max_iterations: 1_000_000,  // Prevent infinite loops
            test_failed: false,
        };

        // Initialize built-in type hierarchy
        forth.init_builtin_types();
        // Initialize built-in operators in dictionary
        forth.init_builtin_operators();
        forth
    }

    fn init_builtin_types(&mut self) {
        // Error hierarchy
        self.type_hierarchy.insert("MathError".to_string(), "Error".to_string());
        self.type_hierarchy.insert("DivideByZero".to_string(), "MathError".to_string());
        // Can add more built-in errors later
    }

    fn init_builtin_operators(&mut self) {
        // Register built-in operators in the dictionary so they can be overridden
        // and participate in wildcard dispatch
        let sig_binary = Signature {
            inputs: vec![Type::I64, Type::I64],
            outputs: vec![Type::I64],
            constraint: None,
        };

        self.dictionary.insert("+".to_string(), vec![(sig_binary.clone(), Word::Add)]);
        self.dictionary.insert("-".to_string(), vec![(sig_binary.clone(), Word::Sub)]);
        self.dictionary.insert("*".to_string(), vec![(sig_binary.clone(), Word::Mul)]);
        self.dictionary.insert("/".to_string(), vec![(sig_binary.clone(), Word::Div)]);
    }

    // Check if 'subtype' is a subtype of 'supertype' (walks up the hierarchy)
    fn is_subtype(&self, subtype: &str, supertype: &str) -> bool {
        if subtype == supertype {
            return true;
        }

        // Walk up the hierarchy
        let mut current = subtype;
        let mut visited = std::collections::HashSet::new();

        while let Some(parent) = self.type_hierarchy.get(current) {
            if visited.contains(current) {
                // Cycle detection
                return false;
            }
            visited.insert(current.to_string());

            if parent == supertype {
                return true;
            }
            current = parent;
        }

        false
    }

    fn signature_specificity(sig: &Signature) -> usize {
        // Count concrete (non-type-variable) types in inputs
        sig.inputs.iter()
            .filter(|t| !matches!(t, Type::TypeVar(_)))
            .count()
    }

    fn type_matches(&self, actual: &Type, expected: &Type, bindings: &mut HashMap<String, Type>) -> bool {
        match (expected, actual) {
            // Type variable - check if already bound
            (Type::TypeVar(var), _) => {
                if let Some(bound_type) = bindings.get(var) {
                    // Already bound, check if it matches
                    bound_type == actual
                } else {
                    // Not bound yet, bind it
                    bindings.insert(var.clone(), actual.clone());
                    true
                }
            }
            // Exact match for built-in types
            (Type::I64, Type::I64) => true,
            (Type::String, Type::String) => true,
            (Type::Quotation, Type::Quotation) => true,
            (Type::Symbol, Type::Symbol) => true,
            // Named types - check subtype relationship
            (Type::Named(expected_name), Type::Named(actual_name)) => {
                // Exact match or subtype
                expected_name == actual_name || self.is_subtype(actual_name, expected_name)
            }
            // Also allow named type to match built-in if names match
            (Type::Named(name), Type::I64) if name == "i64" || name == "int" => true,
            (Type::Named(name), Type::String) if name == "string" || name == "str" => true,
            (Type::Named(name), Type::Quotation) if name == "quotation" || name == "quot" => true,
            (Type::Named(name), Type::Symbol) if name == "symbol" || name == "sym" => true,
            // Arrays - check element types match
            (Type::Array(exp_elem), Type::Array(act_elem)) => {
                self.type_matches(act_elem, exp_elem, bindings)
            }
            // Otherwise no match
            _ => false,
        }
    }

    // State access helpers that use tempmap overlay during constraint evaluation
    fn state_get(&self, name: &str) -> Option<&(Type, Value)> {
        // Check tempmap first if we're in constraint evaluation
        if let Some(ref tempmap) = self.state_tempmap {
            if let Some(entry) = tempmap.get(name) {
                return Some(entry);
            }
        }
        // Fall back to main state
        self.state.get(name)
    }

    fn state_set(&mut self, name: String, typ: Type, value: Value) {
        // Convert to persistent form for storage
        let persistent_value = self.to_persistent(value);

        if let Some(ref mut tempmap) = self.state_tempmap {
            // In constraint mode: write to tempmap
            tempmap.insert(name, (typ, persistent_value));
        } else {
            // Normal mode: write to main state
            self.state.insert(name, (typ, persistent_value));
        }
    }

    fn state_ensure_in_tempmap(&mut self, name: &str) {
        // Ensure variable exists in tempmap (lazy copy from main state)
        if let Some(ref mut tempmap) = self.state_tempmap {
            if !tempmap.contains_key(name) {
                if let Some(entry) = self.state.get(name) {
                    tempmap.insert(name.to_string(), entry.clone());
                }
            }
        }
    }

    fn check_signature(&mut self, name: &str, sig: &Signature) -> Result<(), String> {
        let mut bindings = HashMap::new();

        // Check inputs (from bottom up on stack)
        // Collect types from stack (excluding markers)
        let mut available_types = Vec::new();
        let mut current = self.stack.head.as_ref();
        while let Some(node) = current {
            if !matches!(node.typ, Type::ArrayMarker) {
                available_types.push(&node.typ);
            }
            current = node.next.as_ref();
        }
        // Reverse to get bottom-to-top order
        available_types.reverse();

        if available_types.len() < sig.inputs.len() {
            return Err(format!(
                "Not enough arguments for '{}': expected {}, got {}",
                name, sig.inputs.len(), available_types.len()
            ));
        }

        // Check each input type (inputs are in bottom-to-top order)
        let start_pos = available_types.len() - sig.inputs.len();
        for (i, expected_type) in sig.inputs.iter().enumerate() {
            let actual_type = available_types[start_pos + i];
            if !self.type_matches(actual_type, expected_type, &mut bindings) {
                return Err(format!(
                    "Type mismatch in '{}' argument {}: expected {}, got {}",
                    name, i + 1, expected_type, actual_type
                ));
            }
        }

        Ok(())
    }

    fn push(&mut self, val: Value) {
        let typ = val.get_type();
        self.stack.push(val, typ);
    }

    fn push_num(&mut self, n: i64) {
        self.stack.push(Value::Number(n), Type::I64);
    }

    fn push_quote(&mut self, words: Vec<Word>) {
        self.stack.push(Value::Quotation(words), Type::Quotation);
    }

    fn push_string(&mut self, s: String) {
        self.stack.push(Value::String(s), Type::String);
    }

    fn push_symbol(&mut self, s: String) {
        self.stack.push(Value::Symbol(s), Type::Symbol);
    }

    fn push_array(&mut self, elements: Vec<Value>) {
        let typ = if elements.is_empty() {
            Type::Array(Box::new(Type::I64)) // Default for empty arrays
        } else {
            Type::Array(Box::new(elements[0].get_type()))
        };
        self.stack.push(Value::Array(elements), typ);
    }

    fn push_tuple(&mut self, elements: Vec<Value>) {
        let types: Vec<Type> = elements.iter().map(|v| v.get_type()).collect();
        self.stack.push(Value::Tuple(elements), Type::Tuple(types));
    }

    fn push_persistent_array(&mut self, elements: ImVector<Value>) {
        let typ = if elements.is_empty() {
            Type::Array(Box::new(Type::I64))
        } else {
            Type::Array(Box::new(elements[0].get_type()))
        };
        self.stack.push(Value::PersistentArray(elements), typ);
    }

    // Convert value to persistent form for state storage
    fn to_persistent(&self, value: Value) -> Value {
        match value {
            Value::Array(vec) => {
                // Convert mutable array to persistent
                Value::PersistentArray(vec.into_iter().collect())
            }
            // Other types are already immutable or don't need conversion
            other => other,
        }
    }

    // Convert persistent array to mutable for stack operations
    fn to_mutable(&self, value: Value) -> Value {
        match value {
            Value::PersistentArray(imvec) => {
                // Convert persistent array to mutable
                Value::Array(imvec.iter().cloned().collect())
            }
            other => other,
        }
    }

    fn pop(&mut self) -> Result<Value, &'static str> {
        // Check if TOS is an array marker
        if let Some((value, _)) = self.stack.peek() {
            if matches!(value, Value::ArrayMarker) {
                // Don't pop the marker, it stays put
                // Need to pop from before the marker instead
                // This is complex with linked list - for now, error out
                return Err("Cannot pop with array marker on top - use } to close array");
            }
        }

        // Normal pop
        self.stack.pop()
            .map(|(value, _type)| value)
            .ok_or("Stack underflow")
    }

    fn pop_num(&mut self) -> Result<i64, String> {
        // Use the marker-aware pop
        match self.pop()? {
            Value::Number(n) => Ok(n),
            other => Err(format!("Type error: expected i64, got {}", other.get_type())),
        }
    }

    fn pop_string(&mut self) -> Result<String, String> {
        match self.pop()? {
            Value::String(s) => Ok(s),
            other => Err(format!("Type error: expected string, got {}", other.get_type())),
        }
    }

    fn pop_symbol(&mut self) -> Result<String, String> {
        match self.pop()? {
            Value::Symbol(s) => Ok(s),
            other => Err(format!("Type error: expected symbol, got {}", other.get_type())),
        }
    }

    fn pop_quote(&mut self) -> Result<Vec<Word>, String> {
        match self.pop()? {
            Value::Quotation(q) => Ok(q),
            other => Err(format!("Type error: expected quotation, got {}", other.get_type())),
        }
    }

    fn execute(&mut self, word: &Word) -> Result<(), String> {
        // Check recursion depth
        if self.recursion_depth >= self.max_recursion_depth {
            return Err(format!("Maximum recursion depth ({}) exceeded", self.max_recursion_depth));
        }

        match word {
            // Arithmetic
            Word::Add => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(a + b);
            }
            Word::Sub => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(a - b);
            }
            Word::Mul => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(a * b);
            }
            Word::Div => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                if b == 0 {
                    return Err("Division by zero".to_string());
                }
                self.push_num(a / b);
            }

            // Stack manipulation
            Word::Dup => {
                let a = self.pop()?;
                self.push(a.clone());
                self.push(a);
            }
            Word::Drop => {
                self.pop()?;
            }
            Word::Swap => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b);
                self.push(a);
            }
            Word::Over => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.clone());
                self.push(b);
                self.push(a);
            }
            Word::Rot => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b);
                self.push(c);
                self.push(a);
            }

            // Comparison (return -1 for true, 0 for false)
            Word::Eq => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(if a == b { -1 } else { 0 });
            }
            Word::Lt => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(if a < b { -1 } else { 0 });
            }
            Word::Gt => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(if a > b { -1 } else { 0 });
            }
            Word::Lte => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(if a <= b { -1 } else { 0 });
            }
            Word::Gte => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(if a >= b { -1 } else { 0 });
            }

            // Logic
            Word::And => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(a & b);
            }
            Word::Or => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.push_num(a | b);
            }
            Word::Not => {
                let a = self.pop_num()?;
                self.push_num(!a);
            }

            // I/O
            Word::Dot => {
                match self.pop()? {
                    Value::Number(n) => println!("{}", n),
                    Value::Quotation(_) => println!("[quotation]"),
                    Value::String(s) => println!("{}", s),
                    Value::Symbol(s) => println!("({})", s),
                    Value::Array(arr) => {
                        print!("{{");
                        for (i, elem) in arr.iter().enumerate() {
                            if i > 0 { print!(" "); }
                            match elem {
                                Value::Number(n) => print!("{}", n),
                                Value::String(s) => print!("\"{}\"", s),
                                _ => print!("..."),
                            }
                        }
                        println!("}}");
                    }
                    Value::PersistentArray(arr) => {
                        print!("{{");
                        for (i, elem) in arr.iter().enumerate() {
                            if i > 0 { print!(" "); }
                            match elem {
                                Value::Number(n) => print!("{}", n),
                                Value::String(s) => print!("\"{}\"", s),
                                _ => print!("..."),
                            }
                        }
                        println!("}}");
                    }
                    Value::Tuple(tup) => {
                        print!("(");
                        for (i, elem) in tup.iter().enumerate() {
                            if i > 0 { print!(", "); }
                            match elem {
                                Value::Number(n) => print!("{}", n),
                                Value::String(s) => print!("\"{}\"", s),
                                _ => print!("..."),
                            }
                        }
                        println!(")");
                    }
                    Value::ErrorValue(type_name) => {
                        println!("<Error: {}>", type_name);
                    }
                    Value::ArrayMarker => {
                        return Err("Cannot print array marker - unmatched {".to_string());
                    }
                }
            }
            Word::DotTypes => {
                print!("Types <{}> ", self.stack.len());
                // Iterate through stack and print types
                let mut current = self.stack.head.as_ref();
                let mut types = Vec::new();
                while let Some(node) = current {
                    types.push(&node.typ);
                    current = node.next.as_ref();
                }
                // Reverse to show bottom-to-top
                types.reverse();
                for typ in types {
                    print!("{} ", typ);
                }
                println!();
            }

            // String operations
            Word::Concat => {
                let b = self.pop_string()?;
                let a = self.pop_string()?;
                self.push_string(format!("{}{}", a, b));
            }
            Word::StrLen => {
                let s = self.pop_string()?;
                let len = s.len() as i64;
                self.push_num(len);
            }

            // State operations
            Word::Arrow(var_name) => {
                // -> varname - Pop value, store in variable (immediate word)
                // Stack: value ->
                let value = self.pop()?;

                // Type check: ensure value matches declared type
                if let Some((declared_type, _)) = self.state_get(var_name) {
                    let declared_type = declared_type.clone();
                    if value.get_type() != declared_type {
                        return Err(format!(
                            "Type mismatch: {} expects {}, got {}",
                            var_name, declared_type, value.get_type()
                        ));
                    }
                    self.state_set(var_name.clone(), declared_type, value);
                } else {
                    return Err(format!("Undefined state variable: {}", var_name));
                }
            }
            Word::DoubleArrow(var_name) => {
                // => varname - Copy TOS, store in variable (immediate word)
                // Stack: value => (value remains on stack)
                if self.stack.is_empty() {
                    return Err("Stack underflow: => requires a value on stack".to_string());
                }

                // Peek at top value (don't pop)
                let (value, _typ) = self.stack.peek().unwrap();
                let value = value.clone();

                // Type check: ensure value matches declared type
                if let Some((declared_type, _)) = self.state_get(var_name) {
                    let declared_type = declared_type.clone();
                    if value.get_type() != declared_type {
                        return Err(format!(
                            "Type mismatch: {} expects {}, got {}",
                            var_name, declared_type, value.get_type()
                        ));
                    }
                    self.state_set(var_name.clone(), declared_type, value);
                } else {
                    return Err(format!("Undefined state variable: {}", var_name));
                }
            }

            // Array operations
            Word::ArrayLen => {
                match self.pop()? {
                    Value::Array(arr) => {
                        let len = arr.len() as i64;
                        self.push_num(len);
                    }
                    Value::PersistentArray(arr) => {
                        let len = arr.len() as i64;
                        self.push_num(len);
                    }
                    other => {
                        return Err(format!("array.len expects array, got {}", other.get_type()));
                    }
                }
            }
            Word::ArrayGet => {
                // Stack: array index -- element
                let index = self.pop_num()?;
                match self.pop()? {
                    Value::Array(arr) => {
                        if index < 0 || index >= arr.len() as i64 {
                            return Err(format!("Array index out of bounds: {}", index));
                        }
                        self.push(arr[index as usize].clone());
                    }
                    Value::PersistentArray(arr) => {
                        if index < 0 || index >= arr.len() as i64 {
                            return Err(format!("Array index out of bounds: {}", index));
                        }
                        self.push(arr[index as usize].clone());
                    }
                    Value::Tuple(tup) => {
                        if index < 0 || index >= tup.len() as i64 {
                            return Err(format!("Tuple index out of bounds: {}", index));
                        }
                        self.push(tup[index as usize].clone());
                    }
                    other => {
                        return Err(format!("array.get expects array or tuple, got {}", other.get_type()));
                    }
                }
            }
            Word::ArraySet => {
                // Stack: value array index -- new_array
                let index = self.pop_num()?;
                match self.pop()? {
                    Value::Array(mut arr) => {
                        if index < 0 || index >= arr.len() as i64 {
                            return Err(format!("Array index out of bounds: {}", index));
                        }
                        let new_value = self.pop()?;
                        // Type check: new value must match array element type
                        let expected_type = arr[0].get_type();
                        if new_value.get_type() != expected_type {
                            return Err(format!(
                                "Type mismatch: array expects {}, got {}",
                                expected_type, new_value.get_type()
                            ));
                        }
                        arr[index as usize] = new_value;
                        self.push_array(arr);
                    }
                    Value::PersistentArray(arr) => {
                        if index < 0 || index >= arr.len() as i64 {
                            return Err(format!("Array index out of bounds: {}", index));
                        }
                        let new_value = self.pop()?;
                        // Type check: new value must match array element type
                        let expected_type = arr[0].get_type();
                        if new_value.get_type() != expected_type {
                            return Err(format!(
                                "Type mismatch: array expects {}, got {}",
                                expected_type, new_value.get_type()
                            ));
                        }
                        // Persistent array: use update() which creates new version with structural sharing
                        let new_arr = arr.update(index as usize, new_value);
                        self.push_persistent_array(new_arr);
                    }
                    other => {
                        return Err(format!("array.set expects array, got {}", other.get_type()));
                    }
                }
            }
            Word::ArrayBegin => {
                // Push marker onto stack
                self.stack.push(Value::ArrayMarker, Type::ArrayMarker);
            }
            Word::ArrayEnd => {
                // Collect all values above the marker into a collection
                let mut elements = Vec::new();

                loop {
                    match self.stack.pop() {
                        Some((Value::ArrayMarker, _)) => {
                            // Found the marker
                            break;
                        }
                        Some((val, _typ)) => {
                            elements.push(val);
                        }
                        None => {
                            return Err("Unmatched } without {".to_string());
                        }
                    }
                }

                // Reverse because we popped in reverse order
                elements.reverse();

                // Decide: Array (homogeneous) or Tuple (heterogeneous)?
                if elements.is_empty() {
                    // Empty collection -> empty array
                    self.push_array(elements);
                } else {
                    let first_type = elements[0].get_type();
                    let all_same = elements.iter().all(|v| v.get_type() == first_type);

                    if all_same {
                        // Homogeneous -> Array
                        self.push_array(elements);
                    } else {
                        // Heterogeneous -> Tuple
                        self.push_tuple(elements);
                    }
                }
            }
            Word::ToMutable => {
                // Convert persistent array to mutable array
                let value = self.pop()?;
                let converted = self.to_mutable(value);
                self.push(converted);
            }

            // Control flow
            Word::Times => {
                let count = self.pop_num()?;
                let quote = self.pop_quote()?;
                let abs_count = count.abs();

                // Check iteration limit
                if self.iteration_count + abs_count as usize > self.max_iterations {
                    return Err(format!("Maximum iteration limit ({}) exceeded", self.max_iterations));
                }

                self.recursion_depth += 1;
                for i in 0..abs_count {
                    self.iteration_count += 1;
                    let index = if count < 0 { -i } else { i };
                    self.loop_indices.push(index);

                    for w in &quote {
                        self.execute(w)?;
                    }

                    self.loop_indices.pop();
                }
                self.recursion_depth -= 1;
            }
            Word::Call => {
                let quote = self.pop_quote()?;
                self.recursion_depth += 1;
                for w in &quote {
                    self.execute(w)?;
                }
                self.recursion_depth -= 1;
            }
            Word::If => {
                // Stack: cond {true-branch} {false-branch} if
                let false_branch = self.pop_quote()?;
                let true_branch = self.pop_quote()?;
                let condition = self.pop_num()?;

                self.recursion_depth += 1;
                if condition != 0 {
                    // Non-zero = true
                    for w in &true_branch {
                        self.execute(w)?;
                    }
                } else {
                    // Zero = false
                    for w in &false_branch {
                        self.execute(w)?;
                    }
                }
                self.recursion_depth -= 1;
            }
            Word::LoopIndex => {
                if let Some(&index) = self.loop_indices.last() {
                    self.push_num(index);
                } else {
                    return Err("i0 used outside of loop".to_string());
                }
            }
            Word::Raise(error_type) => {
                // Recover original arguments from saved stack
                if let (Some(word_name), Some(signature), Some(saved_stack)) =
                    (self.current_word.clone(), self.current_signature.clone(), self.saved_stack_head.clone()) {

                    // Push error value first
                    self.push(Value::ErrorValue(error_type.clone()));

                    // Recover original args from saved stack
                    // Walk the saved stack and collect values matching the signature's input types
                    let mut args = Vec::new();
                    let mut current = saved_stack.head.as_ref();

                    for _ in 0..signature.inputs.len() {
                        if let Some(node) = current {
                            args.push((node.value.clone(), node.typ.clone()));
                            current = node.next.as_ref();
                        } else {
                            return Err(format!("Could not recover original args for error handling"));
                        }
                    }

                    // Push recovered args back onto stack (in reverse order since we collected top-down)
                    for (value, typ) in args.into_iter().rev() {
                        self.stack.push(value, typ);
                    }

                    // Re-dispatch current word with error + original args on stack
                    return self.eval_token(&word_name);
                } else {
                    return Err(format!("Error raised ({}) but no current word/signature to re-dispatch", error_type));
                }
            }

            // Literals
            Word::Number(n) => {
                self.push_num(*n);
            }
            Word::Quote(words) => {
                self.push_quote(words.clone());
            }
            Word::StringLit(s) => {
                self.push_string(s.clone());
            }
            Word::SymbolLit(s) => {
                self.push_symbol(s.clone());
            }
            Word::VarFetch(var_name) => {
                // Auto-fetch: lookup and push the value directly
                if let Some((_typ, value)) = self.state_get(var_name) {
                    self.push(value.clone());
                } else {
                    return Err(format!("Undefined state variable: {}", var_name));
                }
            }

            // User-defined words
            Word::UserDefined(words) => {
                self.recursion_depth += 1;
                for w in words {
                    self.execute(w)?;
                }
                self.recursion_depth -= 1;
            }
            Word::NamedWord(name) => {
                // Dispatch to multi-method
                self.eval_token(name)?;
            }
            Word::WildcardDispatch => {
                // [*] - Redispatch using the current operator name
                if let Some(current_op) = self.current_word.clone() {
                    self.eval_token(&current_op)?;
                } else {
                    return Err("[*] can only be used within a wildcard handler".to_string());
                }
            }
        }
        Ok(())
    }

    fn eval(&mut self, input: &str) -> Result<(), String> {
        let cleaned = self.strip_comments(input);
        let tokens = self.tokenize(&cleaned);
        // Reset iteration count for each new eval
        self.iteration_count = 0;
        self.eval_tokens(&tokens, false)
    }

    fn tokenize<'a>(&self, input: &'a str) -> Vec<&'a str> {
        let mut tokens = Vec::new();
        let mut start = 0;
        let mut in_string = false;
        let chars: Vec<char> = input.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '"' {
                if in_string {
                    // End of string - include the closing quote
                    let token = &input[start..=i];
                    tokens.push(token);
                    in_string = false;
                    start = i + 1;
                } else {
                    // Start of string
                    in_string = true;
                    start = i;
                }
            } else if !in_string && chars[i].is_whitespace() {
                // Found whitespace outside string
                if start < i {
                    let token = &input[start..i];
                    if !token.trim().is_empty() {
                        tokens.push(token);
                    }
                }
                start = i + 1;
            }
            i += 1;
        }

        // Handle last token
        if start < chars.len() {
            let token = &input[start..];
            if !token.trim().is_empty() {
                tokens.push(token);
            }
        }

        tokens
    }

    fn strip_comments(&self, input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '"' {
                // String literal: preserve everything until closing quote
                result.push(ch);
                while let Some(c) = chars.next() {
                    result.push(c);
                    if c == '"' {
                        break;
                    }
                    // Handle escaped quotes
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            result.push(escaped);
                        }
                    }
                }
            } else if ch == '-' && chars.peek() == Some(&'-') {
                // Line comment: skip until newline
                chars.next(); // consume second '-'
                while let Some(c) = chars.next() {
                    if c == '\n' {
                        result.push('\n'); // preserve newline
                        break;
                    }
                }
            } else if ch == '(' && chars.peek() == Some(&' ') {
                // Block comment: skip until ')'
                chars.next(); // consume space
                let mut depth = 1;
                while let Some(c) = chars.next() {
                    if c == '(' && chars.peek() == Some(&' ') {
                        chars.next();
                        depth += 1;
                    } else if c == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    fn eval_tokens(&mut self, tokens: &[&str], in_definition: bool) -> Result<(), String> {
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];

            // Handle signature: = type1 type2 -> type3 ;
            if token == "=" {
                i += 1;

                // Parse input types (until we hit ->)
                let mut inputs = Vec::new();
                while i < tokens.len() && tokens[i] != "->" && tokens[i] != ";" {
                    let typ = self.parse_type_or_var(tokens[i])?;
                    inputs.push(typ);
                    i += 1;
                }

                // Expect -> or ; (for no inputs)
                let mut outputs = Vec::new();
                if i < tokens.len() && tokens[i] == "->" {
                    i += 1;  // skip ->

                    // Parse output types (until ;)
                    while i < tokens.len() && tokens[i] != ";" {
                        let typ = self.parse_type_or_var(tokens[i])?;
                        outputs.push(typ);
                        i += 1;
                    }
                }

                // Expect semicolon
                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end signature".to_string());
                }

                // Set current signature context (no constraint yet, will be set by ? if present)
                self.current_signature = Some(Signature { inputs, outputs, constraint: None });
            }
            // Handle context constraints (guards)
            else if token == "?" {
                i += 1;

                // Must have a current signature
                if self.current_signature.is_none() {
                    return Err("? constraint requires a preceding = signature".to_string());
                }

                // Collect constraint expression until ;
                let mut constraint_tokens = Vec::new();
                while i < tokens.len() && tokens[i] != ";" {
                    constraint_tokens.push(tokens[i]);
                    i += 1;
                }

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end constraint".to_string());
                }

                // Parse constraint into words (handles -> and => specially)
                let constraint_words = self.parse_constraint_tokens(&constraint_tokens)?;

                // Add constraint to current signature
                if let Some(ref mut sig) = self.current_signature {
                    sig.constraint = Some(constraint_words);
                }
            }
            // Handle type declarations
            else if token == "%" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected type name after '%'".to_string());
                }
                let type_name = tokens[i].to_string();
                i += 1;

                if i >= tokens.len() || tokens[i] != "<" {
                    return Err("Expected '<' after type name".to_string());
                }
                i += 1;

                if i >= tokens.len() {
                    return Err("Expected parent type after '<'".to_string());
                }
                let parent_type = tokens[i].to_string();
                i += 1;

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end type declaration".to_string());
                }

                // Register type in hierarchy
                self.type_hierarchy.insert(type_name, parent_type);
            }
            // Handle namespace declarations: <> name ;
            else if token == "<>" || token == "◇" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected namespace name after '<>'".to_string());
                }
                let namespace_name = tokens[i].to_string();
                i += 1;

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end namespace declaration".to_string());
                }

                // Register namespace
                self.namespaces.insert(namespace_name);
            }
            // Handle test assertions: TEST. expression ;
            else if token == "TEST." || token.to_lowercase() == "test." {
                i += 1;

                // Collect tokens until semicolon
                let mut test_tokens = Vec::new();
                while i < tokens.len() && tokens[i] != ";" {
                    test_tokens.push(tokens[i]);
                    i += 1;
                }

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end test".to_string());
                }

                let test_expr = test_tokens.join(" ");

                // Execute test expression
                let test_words = self.parse_tokens(&test_tokens)?;
                for word in &test_words {
                    self.execute(word)?;
                }

                // Pop result and check
                if self.stack.is_empty() {
                    return Err("TEST. expression produced no value".to_string());
                }
                let result = self.pop_num()?;
                if result == 0 {
                    println!("✗ FAIL {}", test_expr);
                    self.test_failed = true;
                } else {
                    println!("✓ PASS {}", test_expr);
                }
            }
            // Handle state variable declarations
            else if token == "$" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '$'".to_string());
                }
                let name = tokens[i].to_string();
                i += 1;

                if i >= tokens.len() || tokens[i] != "=" {
                    return Err("Expected '=' after variable name".to_string());
                }
                i += 1;

                if i >= tokens.len() {
                    return Err("Expected initial value after '='".to_string());
                }

                // Collect tokens until semicolon for comptime evaluation
                let mut init_tokens = Vec::new();
                while i < tokens.len() && tokens[i] != ";" {
                    init_tokens.push(tokens[i]);
                    i += 1;
                }

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end state declaration".to_string());
                }

                // Evaluate initialization expression at compile time
                let init_words = self.parse_tokens(&init_tokens)?;
                for word in &init_words {
                    self.execute(word)?;
                }

                // Pop the resulting value from stack
                if self.stack.is_empty() {
                    return Err(format!("State initialization for '{}' produced no value", name));
                }
                let (initial_value, var_type) = self.stack.pop().unwrap();

                // Store state variable
                self.state.insert(name, (var_type, initial_value));
            }
            // Handle colon definitions (: for runtime, :: for immediate/compile-time)
            else if token == ":" || token == "::" {
                let is_immediate = token == "::";

                if self.compiling {
                    return Err("Already compiling".to_string());
                }
                i += 1;
                if i >= tokens.len() {
                    return Err(format!("Expected word name after '{}'", token));
                }
                let name = tokens[i].to_string();

                // Get signature (required for multi-methods)
                let signature = self.current_signature.clone()
                    .ok_or("Word definition requires a signature (use = ... ; before :)")?;

                // Add placeholder to dictionary for forward reference support
                // This allows the word to call itself recursively
                let placeholder = Word::UserDefined(Vec::new());
                let methods = self.dictionary.entry(name.clone()).or_insert_with(Vec::new);
                let placeholder_index = methods.len();
                methods.push((signature.clone(), placeholder));

                self.compiling = true;
                self.current_definition.clear();
                i += 1;

                // Collect words until semicolon
                while i < tokens.len() && tokens[i] != ";" {
                    i = self.compile_token_at(tokens, i)?;
                }

                if i >= tokens.len() || tokens[i] != ";" {
                    return Err("Expected ';' to end definition".to_string());
                }

                // Store actual definition
                let definition = Word::UserDefined(self.current_definition.clone());

                // Replace placeholder with actual definition
                let methods = self.dictionary.get_mut(&name).unwrap();

                // Calculate specificity and find proper sorted position
                let new_specificity = Self::signature_specificity(&signature);

                // Remove the placeholder
                methods.remove(placeholder_index);

                // Find insertion position (sorted by specificity desc, then append)
                let insert_pos = methods.iter()
                    .position(|(sig, _)| {
                        let existing_specificity = Self::signature_specificity(sig);
                        new_specificity > existing_specificity
                    })
                    .unwrap_or(methods.len());

                // Insert at sorted position
                methods.insert(insert_pos, (signature, definition));

                // Mark as immediate if using ::
                if is_immediate {
                    self.immediate_words.insert(name.clone());
                }

                self.compiling = false;
                self.current_definition.clear();
                self.current_signature = None;  // Clear signature after use
            }
            // Handle -> (immediate word that reads next token)
            else if token == "->" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '->'".to_string());
                }
                let var_name = tokens[i].to_string();

                if in_definition {
                    // Compile into definition
                    self.current_definition.push(Word::Arrow(var_name));
                } else {
                    // Execute immediately
                    self.execute(&Word::Arrow(var_name))?;
                }
            }
            // Handle => (immediate word that reads next token)
            else if token == "=>" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '=>'".to_string());
                }
                let var_name = tokens[i].to_string();

                if in_definition {
                    // Compile into definition
                    self.current_definition.push(Word::DoubleArrow(var_name));
                } else {
                    // Execute immediately
                    self.execute(&Word::DoubleArrow(var_name))?;
                }
            }
            // Handle raise (immediate word that reads next token)
            else if token == "raise" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected error type name after 'raise'".to_string());
                }
                let error_type = tokens[i].to_string();

                if in_definition {
                    // Compile into definition
                    self.current_definition.push(Word::Raise(error_type));
                } else {
                    // Execute immediately
                    self.execute(&Word::Raise(error_type))?;
                }
            } else if in_definition {
                // We shouldn't be here when inside a definition
                return Err("Unexpected token in definition".to_string());
            } else if token == "[" || token == "#[" {
                // Handle quotations in immediate mode
                i = self.eval_quotation_at(tokens, i)?;
            } else {
                // Normal execution (including { and })
                self.eval_token(token)?;
            }

            i += 1;
        }

        Ok(())
    }

    fn eval_quotation_at(&mut self, tokens: &[&str], start: usize) -> Result<usize, String> {
        let token = tokens[start];
        let is_sugar = token == "#[";

        // Note: { } are array literals, not nested in [ ] depth tracking
        let mut depth = 1;
        let mut i = start + 1;
        let mut quote_tokens = Vec::new();

        while i < tokens.len() && depth > 0 {
            if tokens[i] == "[" || tokens[i] == "#[" {
                depth += 1;
            } else if tokens[i] == "]" {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            quote_tokens.push(tokens[i]);
            i += 1;
        }

        if depth != 0 {
            return Err(format!("Unmatched {} without ]", token));
        }

        let body = self.parse_tokens(&quote_tokens)?;

        if is_sugar {
            // #[ ... ] => [ ... ] swap #do
            self.execute(&Word::Quote(body))?;
            self.execute(&Word::Swap)?;
            self.execute(&Word::Times)?;
        } else {
            // [ ... ] => just push quotation
            self.execute(&Word::Quote(body))?;
        }

        Ok(i)
    }

    fn compile_token_at(&mut self, tokens: &[&str], start: usize) -> Result<usize, String> {
        let token = tokens[start];

        // Handle -> (immediate word in definitions)
        if token == "->" {
            if start + 1 >= tokens.len() {
                return Err("Expected variable name after '->'".to_string());
            }
            let var_name = tokens[start + 1].to_string();
            self.current_definition.push(Word::Arrow(var_name));
            return Ok(start + 2);  // Skip both -> and varname
        }
        // Handle => (immediate word in definitions)
        else if token == "=>" {
            if start + 1 >= tokens.len() {
                return Err("Expected variable name after '=>'".to_string());
            }
            let var_name = tokens[start + 1].to_string();
            self.current_definition.push(Word::DoubleArrow(var_name));
            return Ok(start + 2);  // Skip both => and varname
        }
        // Handle raise (immediate word in definitions)
        else if token == "raise" {
            if start + 1 >= tokens.len() {
                return Err("Expected error type name after 'raise'".to_string());
            }
            let error_type = tokens[start + 1].to_string();
            self.current_definition.push(Word::Raise(error_type));
            return Ok(start + 2);  // Skip both raise and error type
        } else if token == "[" {
            // Collect quotation until ]
            // Note: { } are array literals, not nested in [ ] depth tracking
            let mut depth = 1;
            let mut i = start + 1;
            let mut quote_tokens = Vec::new();

            while i < tokens.len() && depth > 0 {
                if tokens[i] == "[" || tokens[i] == "#[" {
                    depth += 1;
                } else if tokens[i] == "]" {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                quote_tokens.push(tokens[i]);
                i += 1;
            }

            if depth != 0 {
                return Err("Unmatched [ without ]".to_string());
            }

            let body = self.parse_tokens(&quote_tokens)?;
            self.current_definition.push(Word::Quote(body));
            Ok(i + 1)  // i points at ], skip past it
        } else if token == "#[" {
            // Syntactic sugar: #[ ... ] => [ ... ] swap #do
            let mut depth = 1;
            let mut i = start + 1;
            let mut quote_tokens = Vec::new();

            while i < tokens.len() && depth > 0 {
                if tokens[i] == "#[" || tokens[i] == "[" {
                    depth += 1;
                } else if tokens[i] == "]" {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                quote_tokens.push(tokens[i]);
                i += 1;
            }

            if depth != 0 {
                return Err("Unmatched #[ without ]".to_string());
            }

            let body = self.parse_tokens(&quote_tokens)?;
            self.current_definition.push(Word::Quote(body));
            self.current_definition.push(Word::Swap);
            self.current_definition.push(Word::Times);
            Ok(i + 1)  // i points at ], skip past it
        } else {
            // Check if this is an immediate word
            if self.immediate_words.contains(token) {
                // Execute immediately (at compile-time) instead of compiling
                self.eval_token(token)?;
                Ok(start + 1)
            } else {
                // Normal compilation: add word to current definition
                let word = self.parse_token(token)?;
                self.current_definition.push(word);
                Ok(start + 1)  // Return next position, not current
            }
        }
    }

    fn eval_token(&mut self, token: &str) -> Result<(), String> {
        // Check if this is a multi-method word
        if let Some(methods) = self.dictionary.get(token).cloned() {
            // Set current word and save state for error handling
            let saved_word = self.current_word.clone();
            let saved_signature = self.current_signature.clone();
            let saved_stack_head = self.saved_stack_head.clone();

            self.current_word = Some(token.to_string());

            // Try each signature in order (most specific first)
            for (sig, word) in methods {
                // First check type signature
                if self.signature_matches(&sig).is_ok() {
                    // Then check constraint (if present)
                    if let Some(ref constraint) = sig.constraint {
                        if !self.eval_constraint(constraint)? {
                            // Constraint failed, try next variant
                            continue;
                        }
                    }
                    // Found a match! Save stack and signature before execution
                    self.current_signature = Some(sig.clone());
                    self.saved_stack_head = Some(self.stack.clone());

                    let result = self.execute(&word);

                    // Restore saved state
                    self.current_word = saved_word;
                    self.current_signature = saved_signature;
                    self.saved_stack_head = saved_stack_head;

                    return result;
                }
            }

            // No signature matched - try wildcard [*] as fallback
            self.current_word = saved_word.clone();

            // Check if there's a wildcard definition
            if let Some(wildcard_methods) = self.dictionary.get("[*]").cloned() {
                self.current_word = Some(token.to_string());

                for (sig, word) in wildcard_methods {
                    if self.signature_matches(&sig).is_ok() {
                        if let Some(ref constraint) = sig.constraint {
                            if !self.eval_constraint(constraint)? {
                                continue;
                            }
                        }

                        self.current_signature = Some(sig.clone());
                        self.saved_stack_head = Some(self.stack.clone());

                        let result = self.execute(&word);

                        self.current_word = saved_word.clone();
                        self.current_signature = saved_signature;
                        self.saved_stack_head = saved_stack_head;

                        return result;
                    }
                }

                self.current_word = saved_word.clone();
            }

            return Err(format!("No matching signature for '{}' with current stack types", token));
        }

        // Not in dictionary, try parsing as literal or built-in
        let word = self.parse_token(token)?;
        self.execute(&word)
    }

    fn eval_constraint(&mut self, constraint: &[Word]) -> Result<bool, String> {
        // Fork the stack (structural sharing via Rc)
        let saved_stack = self.stack.fork();

        // Enable tempmap overlay for lazy state copying
        self.state_tempmap = Some(HashMap::new());

        // Evaluate constraint expression
        let result = (|| -> Result<bool, String> {
            for word in constraint {
                self.execute(word)?;
            }

            // Pop boolean result
            if self.stack.is_empty() {
                return Err("Constraint expression must produce a value".to_string());
            }

            let result = self.pop_num()?;
            Ok(result != 0)
        })();

        // Always restore original state (even if constraint fails)
        self.stack = saved_stack;
        self.state_tempmap = None;  // Discard tempmap

        result
    }

    fn signature_matches(&self, sig: &Signature) -> Result<(), String> {
        let mut bindings = HashMap::new();

        // Check inputs - collect types from stack (excluding markers)
        let mut available_types = Vec::new();
        let mut current = self.stack.head.as_ref();
        while let Some(node) = current {
            if !matches!(node.typ, Type::ArrayMarker) {
                available_types.push(&node.typ);
            }
            current = node.next.as_ref();
        }
        // Reverse to get bottom-to-top order
        available_types.reverse();

        if available_types.len() < sig.inputs.len() {
            return Err("Not enough arguments".to_string());
        }

        let start_pos = available_types.len() - sig.inputs.len();
        for (i, expected_type) in sig.inputs.iter().enumerate() {
            let actual_type = available_types[start_pos + i];
            if !self.type_matches(actual_type, expected_type, &mut bindings) {
                return Err("Type mismatch".to_string());
            }
        }

        Ok(())
    }

    fn parse_tokens(&self, tokens: &[&str]) -> Result<Vec<Word>, String> {
        let mut words = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];

            // Handle namespace qualification: if token is a namespace and there's a next token,
            // combine them into a qualified name
            if self.namespaces.contains(token) && i + 1 < tokens.len() {
                let next_token = tokens[i + 1];
                let qualified_name = format!("{}.{}", token, next_token);
                let word = self.parse_token(&qualified_name)?;
                words.push(word);
                i += 2;  // Skip both tokens
                continue;
            }

            // Handle -> and => and raise in parsed context (e.g., quotations)
            if token == "->" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '->'".to_string());
                }
                let var_name = tokens[i].to_string();
                words.push(Word::Arrow(var_name));
                i += 1;
                continue;
            } else if token == "=>" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '=>'".to_string());
                }
                let var_name = tokens[i].to_string();
                words.push(Word::DoubleArrow(var_name));
                i += 1;
                continue;
            } else if token == "raise" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected error type name after 'raise'".to_string());
                }
                let error_type = tokens[i].to_string();
                words.push(Word::Raise(error_type));
                i += 1;
                continue;
            } else if token == "[" {
                // Collect quotation until ]
                // Note: { } are array literals, not nested in [ ] depth tracking
                let mut depth = 1;
                let mut j = i + 1;
                let mut quote_tokens = Vec::new();

                while j < tokens.len() && depth > 0 {
                    if tokens[j] == "[" || tokens[j] == "#[" {
                        depth += 1;
                    } else if tokens[j] == "]" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    quote_tokens.push(tokens[j]);
                    j += 1;
                }

                if depth != 0 {
                    return Err("Unmatched [ without ]".to_string());
                }

                // Parse the quotation recursively
                let body = self.parse_tokens(&quote_tokens)?;
                words.push(Word::Quote(body));
                i = j;
            } else if token == "#[" {
                // Syntactic sugar: #[ ... ] => [ ... ] swap #do
                // Collect quotation until ]
                // Note: { } are array literals, not nested in [ ] depth tracking
                let mut depth = 1;
                let mut j = i + 1;
                let mut quote_tokens = Vec::new();

                while j < tokens.len() && depth > 0 {
                    if tokens[j] == "#[" || tokens[j] == "[" {
                        depth += 1;
                    } else if tokens[j] == "]" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    quote_tokens.push(tokens[j]);
                    j += 1;
                }

                if depth != 0 {
                    return Err("Unmatched #[ without ]".to_string());
                }

                // Desugar to: [ ... ] swap #do
                let body = self.parse_tokens(&quote_tokens)?;
                words.push(Word::Quote(body));
                words.push(Word::Swap);
                words.push(Word::Times);
                i = j;
            } else {
                words.push(self.parse_token(token)?);
            }

            i += 1;
        }

        Ok(words)
    }

    fn parse_constraint_tokens(&self, tokens: &[&str]) -> Result<Vec<Word>, String> {
        let mut words = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];

            // Handle -> and => specially
            if token == "->" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '->'".to_string());
                }
                let var_name = tokens[i].to_string();
                words.push(Word::Arrow(var_name));
            } else if token == "=>" {
                i += 1;
                if i >= tokens.len() {
                    return Err("Expected variable name after '=>'".to_string());
                }
                let var_name = tokens[i].to_string();
                words.push(Word::DoubleArrow(var_name));
            } else {
                words.push(self.parse_token(token)?);
            }

            i += 1;
        }

        Ok(words)
    }

    fn parse_token(&self, token: &str) -> Result<Word, String> {
        match token {
            // Arithmetic - now go through dictionary dispatch
            "+" | "-" | "*" | "/" => Ok(Word::NamedWord(token.to_string())),

            // Stack manipulation
            "dup" => Ok(Word::Dup),
            "drop" => Ok(Word::Drop),
            "swap" => Ok(Word::Swap),
            "over" => Ok(Word::Over),
            "rot" => Ok(Word::Rot),

            // Comparison
            "eq" => Ok(Word::Eq),
            "lt" => Ok(Word::Lt),
            "gt" => Ok(Word::Gt),
            "lte" => Ok(Word::Lte),
            "gte" => Ok(Word::Gte),

            // Logic
            "and" => Ok(Word::And),
            "or" => Ok(Word::Or),
            "not" => Ok(Word::Not),

            // I/O
            "." => Ok(Word::Dot),
            ".types" => Ok(Word::DotTypes),

            // String operations
            "++" => Ok(Word::Concat),
            "str.len" => Ok(Word::StrLen),

            // Collection operations
            "array.len" => Ok(Word::ArrayLen),
            "array.get" => Ok(Word::ArrayGet),
            "array.set" => Ok(Word::ArraySet),
            "@" => Ok(Word::ArrayGet),  // Alias for array.get
            "!" => Ok(Word::ArraySet),  // Alias for array.set
            "{" => Ok(Word::ArrayBegin),
            "to-mutable" => Ok(Word::ToMutable),
            "}" => Ok(Word::ArrayEnd),

            // Control flow
            "#do" | "times" => Ok(Word::Times),
            "call" | "eval" => Ok(Word::Call),
            "if" => Ok(Word::If),
            "i0" => Ok(Word::LoopIndex),

            // Wildcard dispatch
            "[*]" => Ok(Word::WildcardDispatch),

            _ => {
                // Try to parse as symbol literal (variable reference)
                if token.starts_with('(') && token.ends_with(')') && token.len() > 2 {
                    let symbol_name = &token[1..token.len()-1];
                    // Check if it ends with ! for (symbol!) syntax
                    if symbol_name.ends_with('!') {
                        return Err("(symbol!) syntax not supported, use: value (symbol) !".to_string());
                    }
                    Ok(Word::SymbolLit(symbol_name.to_string()))
                }
                // Try to parse as string literal
                else if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
                    let content = &token[1..token.len()-1];
                    // Handle escape sequences
                    let unescaped = content
                        .replace("\\n", "\n")
                        .replace("\\t", "\t")
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\");
                    Ok(Word::StringLit(unescaped))
                }
                // Try to parse as number
                else if let Ok(n) = token.parse::<i64>() {
                    Ok(Word::Number(n))
                }
                // Check if it's a state variable - auto-fetch the value
                else if self.state.contains_key(token) {
                    Ok(Word::VarFetch(token.to_string()))
                }
                // Check if it's a defined word (including qualified names like math.div)
                else if self.dictionary.contains_key(token) {
                    // Word exists in dictionary - runtime dispatch
                    Ok(Word::NamedWord(token.to_string()))
                } else {
                    Err(format!("Unknown word: {}", token))
                }
            }
        }
    }

    fn parse_type(&self, type_name: &str) -> Result<Type, String> {
        match type_name {
            "i64" | "int" => Ok(Type::I64),
            "str" | "string" => Ok(Type::String),
            "quot" | "quotation" => Ok(Type::Quotation),
            "sym" | "symbol" => Ok(Type::Symbol),
            _ => Err(format!("Unknown type: {}", type_name)),
        }
    }

    fn parse_type_or_var(&self, name: &str) -> Result<Type, String> {
        // Handle array syntax: {type}
        if name.starts_with('{') && name.ends_with('}') && name.len() > 2 {
            let inner = &name[1..name.len()-1];
            let inner_type = self.parse_type_or_var(inner)?;
            return Ok(Type::Array(Box::new(inner_type)));
        }

        // Single lowercase letter = type variable
        if name.len() == 1 && name.chars().next().unwrap().is_lowercase() {
            return Ok(Type::TypeVar(name.to_string()));
        }

        // Try built-in types first
        match name {
            "i64" | "int" => Ok(Type::I64),
            "str" | "string" => Ok(Type::String),
            "quot" | "quotation" => Ok(Type::Quotation),
            "sym" | "symbol" => Ok(Type::Symbol),
            // Otherwise it's a named type
            _ => Ok(Type::Named(name.to_string())),
        }
    }

    fn parse_value_for_type(&self, token: &str, expected_type: &Type) -> Result<Value, String> {
        match expected_type {
            Type::I64 => {
                let n = token.parse::<i64>()
                    .map_err(|_| format!("Expected i64 value, got: {}", token))?;
                Ok(Value::Number(n))
            }
            Type::String => {
                if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
                    let content = &token[1..token.len()-1];
                    let unescaped = content
                        .replace("\\n", "\n")
                        .replace("\\t", "\t")
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\");
                    Ok(Value::String(unescaped))
                } else {
                    Err(format!("Expected string literal, got: {}", token))
                }
            }
            Type::Symbol => {
                if token.starts_with('(') && token.ends_with(')') {
                    let symbol = &token[1..token.len()-1];
                    Ok(Value::Symbol(symbol.to_string()))
                } else {
                    Err(format!("Expected symbol literal (name), got: {}", token))
                }
            }
            Type::Quotation => {
                Err("Quotation initial values not yet supported in state declarations".to_string())
            }
            Type::Array(_) => {
                Err("Array initial values not yet supported in state declarations".to_string())
            }
            Type::Tuple(_) => {
                Err("Tuple initial values not yet supported in state declarations".to_string())
            }
            Type::ArrayMarker => {
                Err("ArrayMarker is internal only, not a valid state variable type".to_string())
            }
            Type::TypeVar(_) => {
                Err("Type variables not allowed in state declarations".to_string())
            }
            Type::Named(_) => {
                Err("Named types not yet supported in state declarations".to_string())
            }
        }
    }

    fn show_stack(&self) {
        print!("<{}> ", self.stack.len());
        // Iterate through stack and print values
        let mut current = self.stack.head.as_ref();
        let mut values = Vec::new();
        while let Some(node) = current {
            values.push(&node.value);
            current = node.next.as_ref();
        }
        // Reverse to show bottom-to-top
        values.reverse();
        for val in values {
            match val {
                Value::Number(n) => print!("{} ", n),
                Value::Quotation(_) => print!("[...] "),
                Value::String(s) => print!("\"{}\" ", s),
                Value::Symbol(s) => print!("({}) ", s),
                Value::Array(_) => print!("{{...}} "),
                Value::PersistentArray(_) => print!("{{...}} "),
                Value::Tuple(_) => print!("(...) "),
                Value::ErrorValue(type_name) => print!("<Error:{}> ", type_name),
                Value::ArrayMarker => print!("{{ "),
            }
        }
        println!();
    }
}

fn main() {
    let mut forth = Forth::new();

    // Check if stdin is a terminal (interactive mode)
    let is_interactive = atty::is(atty::Stream::Stdin);

    if is_interactive {
        println!("March2 FORTH v0.1");
        println!("Type 'quit' to exit\n");
    }

    loop {
        if is_interactive {
            print!("> ");
            io::stdout().flush().unwrap();
        }

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap();

        // Check for EOF
        if bytes_read == 0 {
            break;
        }

        let input = input.trim();

        if input == "quit" || input == "exit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        match forth.eval(input) {
            Ok(_) => {
                if is_interactive {
                    forth.show_stack();
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Exit with error status if any test failed
    if forth.test_failed {
        std::process::exit(1);
    }
}
