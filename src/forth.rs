// Core FORTH interpreter

use std::collections::HashMap;
use crate::value::{Value, Type};
use crate::xt::XT;
use crate::word::{Word, Signature};
use crate::input::InputBuffer;
use crate::cid::CID;
use crate::serializable::SerializableValue;

pub struct Forth {
    pub data_stack: Vec<Value>,
    pub return_stack: Vec<Value>,  // Return stack for calls, loops, and temp storage
    pub namespaces: Vec<HashMap<String, Word>>, // All namespaces (indexed)
    pub namespace_names: Vec<String>, // Names for each namespace (parallel to namespaces)
    pub namespace_stack: Vec<usize>, // Stack of indices into namespaces
    pub global_state: HashMap<String, Value>, // Immutable global state
    pub compiling: bool,           // Are we in compilation mode?
    pub current_def: Vec<XT>,      // Current word being compiled (XTs for execution)
    pub current_def_cids: Vec<CID>, // Current word being compiled (CIDs for storage)
    pub current_name: Option<String>, // Name of word being compiled
    pub in_quotation: bool,        // Are we compiling a quotation?
    pub quotation_depth: usize,    // Nesting depth of quotations
    pub current_quotation: Vec<XT>, // Current quotation being compiled
    pub lookup_namespace: Option<usize>, // Temporary namespace index for next word lookup
    pub current_signature: Option<Signature>, // Current type signature for new definitions
    pub type_stack: Vec<Type>,     // Compile-time type stack for type checking
}

impl Forth {
    pub fn new() -> Self {
        let mut forth = Forth {
            data_stack: Vec::new(),
            return_stack: Vec::new(),
            namespaces: vec![HashMap::new()], // Root namespace at index 0
            namespace_names: vec![String::new()],  // Root namespace has empty name
            namespace_stack: vec![0], // Stack starts with root namespace index
            global_state: HashMap::new(),
            compiling: false,
            current_def: Vec::new(),
            current_def_cids: Vec::new(),
            current_name: None,
            in_quotation: false,
            quotation_depth: 0,
            current_quotation: Vec::new(),
            lookup_namespace: None,
            current_signature: None,
            type_stack: Vec::new(),
        };

        // Bootstrap: Add primitive words to dictionary
        // Arithmetic ops (i64 i64 -> i64)
        let binary_i64_sig = Signature::new(vec![Type::I64, Type::I64], vec![Type::I64]);
        let mut add_word = Word::new(XT::Add);
        add_word.signature = Some(binary_i64_sig.clone());
        forth.add_word("+", add_word);

        let mut sub_word = Word::new(XT::Sub);
        sub_word.signature = Some(binary_i64_sig.clone());
        forth.add_word("-", sub_word);

        let mut mul_word = Word::new(XT::Mul);
        mul_word.signature = Some(binary_i64_sig.clone());
        forth.add_word("*", mul_word);

        let mut div_word = Word::new(XT::Div);
        div_word.signature = Some(binary_i64_sig);
        forth.add_word("/", div_word);

        // Stack ops (TODO: add signatures for these)
        forth.add_word("dup", Word::new(XT::Dup));
        forth.add_word("drop", Word::new(XT::Drop));
        forth.add_word("swap", Word::new(XT::Swap));
        forth.add_word("over", Word::new(XT::Over));
        forth.add_word("rot", Word::new(XT::Rot));
        forth.add_word(".", Word::new(XT::Dot));

        // Comparison operators
        forth.add_word("lt?", Word::new(XT::Lt));
        forth.add_word("gt?", Word::new(XT::Gt));
        forth.add_word("lte?", Word::new(XT::Lte));
        forth.add_word("gte?", Word::new(XT::Gte));
        forth.add_word("eq?", Word::new(XT::Eq));
        forth.add_word("neq?", Word::new(XT::Neq));

        // Return stack operations
        forth.add_word(">r", Word::new(XT::ToR));
        forth.add_word("r>", Word::new(XT::FromR));
        forth.add_word("r@", Word::new(XT::RFetch));

        // Quotation operations
        forth.add_word("call", Word::new(XT::Call));

        // Conditional operations
        forth.add_word("if", Word::new(XT::If));
        forth.add_word("iff", Word::new(XT::Iff));

        // Add immediate words for definitions
        forth.add_word(":", Word::immediate(XT::Native(native_colon)));
        forth.add_word("::", Word::immediate(XT::Native(native_colon_immediate)));
        forth.add_word(";", Word::immediate(XT::Native(native_semicolon)));

        // Add long-form aliases (for serialization/database format)
        forth.add_word("DEFINE.", Word::immediate(XT::Native(native_colon)));
        forth.add_word("SIGNATURE.", Word::immediate(XT::Native(native_signature)));
        forth.add_word("CONTEXT.", Word::immediate(XT::Native(native_context)));

        // Namespace support
        forth.add_word("NAMESPACE.", Word::immediate(XT::Native(native_namespace)));
        forth.add_word("IMPORT.", Word::immediate(XT::Native(native_import)));
        forth.add_word("ALIAS.", Word::immediate(XT::Native(native_alias)));

        // State/Variables
        forth.add_word("VARIABLE.", Word::immediate(XT::Native(native_variable)));
        forth.add_word("->", Word::new(XT::Native(native_store)));
        forth.add_word("mutable", Word::new(XT::Native(native_mutable)));
        forth.add_word("immutable", Word::new(XT::Native(native_immutable)));

        // Type operations
        forth.add_word("type", Word::new(XT::Type));

        // Type constants (push Type values onto stack)
        forth.add_word("i64", Word::new(XT::Literal(Value::Type(crate::value::Type::I64))));
        forth.add_word("string", Word::new(XT::Literal(Value::Type(crate::value::Type::String))));
        forth.add_word("quotation", Word::new(XT::Literal(Value::Type(crate::value::Type::Quotation))));
        forth.add_word("array", Word::new(XT::Literal(Value::Type(crate::value::Type::Array))));
        forth.add_word("map", Word::new(XT::Literal(Value::Type(crate::value::Type::Map))));

        // Generic type operations
        forth.add_word("?", Word::new(XT::Native(native_type_check)));
        forth.add_word("!", Word::new(XT::Native(native_type_cast)));

        // Type signatures
        forth.add_word("SIGNATURE.", Word::immediate(XT::Native(native_signature)));
        forth.add_word("sig", Word::new(XT::Native(native_sig)));

        // Testing support
        forth.add_word("TEST.", Word::immediate(XT::Native(native_test)));

        // Comments
        forth.add_word("--", Word::immediate(XT::Native(native_line_comment)));

        // Quotation delimiters (immediate words)
        forth.add_word("(", Word::immediate(XT::Native(native_lparen)));
        forth.add_word(")", Word::immediate(XT::Native(native_rparen)));

        forth
    }

    fn add_word(&mut self, name: &str, word: Word) {
        // Add word to the current namespace (top of namespace stack)
        let current_ns_idx = *self.namespace_stack.last().unwrap();
        self.namespaces[current_ns_idx].insert(name.to_string(), word);
    }

    // Find namespace index by name
    fn find_namespace_index(&self, name: &str) -> Option<usize> {
        self.namespace_names.iter().position(|ns| ns == name)
    }

    // Look up a word, checking namespace context
    fn lookup_word(&mut self, name: &str) -> Option<Word> {
        // If name contains dots (and has content before/after), it's a fully qualified name
        // Split it and navigate to the right namespace
        if name.contains('.') && name.len() > 1 {
            let parts: Vec<&str> = name.rsplitn(2, '.').collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                let ns_name = parts[1];
                let word_name = parts[0];
                if let Some(idx) = self.find_namespace_index(ns_name) {
                    return self.namespaces[idx].get(word_name).cloned();
                }
                return None; // Qualified name but namespace not found
            }
            // Fall through if it's just "." or malformed
        }

        // Check if there's a temporary lookup namespace override
        if let Some(ns_idx) = self.lookup_namespace.take() {
            if let Some(word) = self.namespaces[ns_idx].get(name).cloned() {
                return Some(word);
            }
            // Fall through to regular lookup if not found
        }

        // Walk namespace stack backwards (most recent first)
        for &ns_idx in self.namespace_stack.iter().rev() {
            if let Some(word) = self.namespaces[ns_idx].get(name) {
                return Some(word.clone());
            }
        }

        None
    }

    pub fn eval_token_with_input(&mut self, token: &str, input: &mut InputBuffer) -> Result<(), String> {
        // Try to parse as string literal
        if token.starts_with('"') {
            let string_val = self.parse_string(token, input)?;
            let literal = XT::Literal(Value::String(string_val.clone()));
            if self.in_quotation {
                self.current_quotation.push(literal);
            } else if self.compiling {
                self.current_def.push(literal);
                // Generate CID for the literal
                let ser_val = SerializableValue::String(string_val);
                let cid = ser_val.to_cid().map_err(|e| format!("Failed to create CID for string literal: {}", e))?;
                self.current_def_cids.push(cid);
                // Track type on type stack during compilation
                self.type_stack.push(Type::String);
            } else {
                self.data_stack.push(match literal {
                    XT::Literal(v) => v,
                    _ => unreachable!(),
                });
            }
            return Ok(());
        }

        // Try to parse as number
        if let Ok(n) = token.parse::<i64>() {
            let literal = XT::Literal(Value::Number(n));
            if self.in_quotation {
                self.current_quotation.push(literal);
            } else if self.compiling {
                self.current_def.push(literal);
                // Generate CID for the literal
                let ser_val = SerializableValue::Number(n);
                let cid = ser_val.to_cid().map_err(|e| format!("Failed to create CID for number literal: {}", e))?;
                self.current_def_cids.push(cid);
                // Track type on type stack during compilation
                self.type_stack.push(Type::I64);
            } else {
                self.data_stack.push(Value::Number(n));
            }
            return Ok(());
        }

        // Check if token is a namespace reference
        if let Some(ns_idx) = self.find_namespace_index(token) {
            // Set lookup namespace index for next word
            self.lookup_namespace = Some(ns_idx);
            return Ok(());
        }

        // Look up word in dictionary (with namespace resolution)
        if let Some(word) = self.lookup_word(token) {
            if word.immediate {
                // Immediate words always execute
                self.execute(&word.xt, input)?;
            } else if self.in_quotation {
                // Inside quotation: add to quotation
                self.current_quotation.push(word.xt.clone());
            } else if self.compiling {
                // Compiling word: add to current definition
                self.current_def.push(word.xt.clone());

                // Add word's CID to compilation
                if let Some(cid) = &word.cid {
                    self.current_def_cids.push(*cid);
                } else {
                    return Err(format!("Word '{}' has no CID - cannot compile", token));
                }

                // Type check if word has a signature
                if let Some(sig) = &word.signature {
                    self.check_and_update_types(sig, token)?;
                }
            } else {
                // Interpreting: execute immediately
                self.execute(&word.xt, input)?;
            }
            return Ok(());
        }

        // Check if it's a variable in global state
        if let Some(value) = self.global_state.get(token) {
            // Push variable value onto stack
            if self.in_quotation {
                self.current_quotation.push(XT::Literal(value.clone()));
            } else if self.compiling {
                self.current_def.push(XT::Literal(value.clone()));
            } else {
                self.data_stack.push(value.clone());
            }
            return Ok(());
        }

        Err(format!("Unknown word: {}", token))
    }

    fn parse_string(&mut self, first_token: &str, input: &mut InputBuffer) -> Result<String, String> {
        // Parse a string literal starting with "
        // first_token is like `"Hello` or `"Hello"` or just `"`

        let after_quote = &first_token[1..]; // Remove opening "

        // Check if the closing quote is already in this token (simple case)
        if let Some(pos) = after_quote.find('"') {
            // Entire string in one token like `"Hello"`
            return Ok(after_quote[..pos].to_string());
        }

        // String continues past this token
        // Start with what we have, then read character-by-character from buffer
        let mut result = after_quote.to_string();

        // Read the rest character-by-character (handles escapes and preserves all chars)
        let rest = input.read_string_literal()?;

        // Combine them - read_string_literal() preserves all characters including spaces
        result.push_str(&rest);

        Ok(result)
    }

    fn execute(&mut self, xt: &XT, input: &mut InputBuffer) -> Result<(), String> {
        match xt {
            XT::Add => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(a + b));
            }
            XT::Sub => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(a - b));
            }
            XT::Mul => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(a * b));
            }
            XT::Div => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                if b == 0 {
                    return Err("Division by zero".to_string());
                }
                self.data_stack.push(Value::Number(a / b));
            }
            XT::Dup => {
                let a = self.pop()?;
                self.data_stack.push(a.clone());
                self.data_stack.push(a);
            }
            XT::Drop => {
                self.pop()?;
            }
            XT::Swap => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.data_stack.push(b);
                self.data_stack.push(a);
            }
            XT::Over => {
                // ( a b -- a b a )
                let b = self.pop()?;
                let a = self.pop()?;
                self.data_stack.push(a.clone());
                self.data_stack.push(b);
                self.data_stack.push(a);
            }
            XT::Rot => {
                // ( a b c -- b c a )
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.data_stack.push(b);
                self.data_stack.push(c);
                self.data_stack.push(a);
            }
            XT::Dot => {
                let val = self.pop()?;
                match val {
                    Value::Number(n) => println!("{}", n),
                    Value::Quotation(_) => println!("<quotation>"),
                    Value::String(s) => println!("{}", s),
                    Value::Type(t) => println!("{}", t.name()),
                    Value::Array(_) => println!("<array>"),
                    Value::Map(_) => println!("<map>"),
                    Value::MutableArray(_) => println!("<mutable-array>"),
                    Value::MutableMap(_) => println!("<mutable-map>"),
                }
            }
            XT::Lt => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a < b { 1 } else { 0 }));
            }
            XT::Gt => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a > b { 1 } else { 0 }));
            }
            XT::Lte => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a <= b { 1 } else { 0 }));
            }
            XT::Gte => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a >= b { 1 } else { 0 }));
            }
            XT::Eq => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a == b { 1 } else { 0 }));
            }
            XT::Neq => {
                let b = self.pop_num()?;
                let a = self.pop_num()?;
                self.data_stack.push(Value::Number(if a != b { 1 } else { 0 }));
            }
            XT::ToR => {
                // >r - Move top of data stack to return stack
                let val = self.pop()?;
                self.return_stack.push(val);
            }
            XT::FromR => {
                // r> - Move top of return stack to data stack
                let val = self.return_stack.pop()
                    .ok_or("Return stack underflow")?;
                self.data_stack.push(val);
            }
            XT::RFetch => {
                // r@ - Copy top of return stack to data stack (peek)
                let val = self.return_stack.last()
                    .ok_or("Return stack empty")?;
                self.data_stack.push(val.clone());
            }
            XT::Call => {
                // Execute a quotation from the stack
                let val = self.pop()?;
                match val {
                    Value::Quotation(xts) => {
                        for xt in &xts {
                            self.execute(xt, input)?;
                        }
                    }
                    _ => return Err("call requires a quotation".to_string()),
                }
            }
            XT::If => {
                // ( cond true-quot false-quot -- )
                // Pop false branch
                let false_branch = match self.pop()? {
                    Value::Quotation(xts) => xts,
                    _ => return Err("if requires quotations".to_string()),
                };
                // Pop true branch
                let true_branch = match self.pop()? {
                    Value::Quotation(xts) => xts,
                    _ => return Err("if requires quotations".to_string()),
                };
                // Pop condition
                let cond = self.pop_num()?;

                // Execute appropriate branch
                let branch = if cond != 0 { &true_branch } else { &false_branch };
                for xt in branch {
                    self.execute(xt, input)?;
                }
            }
            XT::Iff => {
                // ( cond quot -- )
                // Pop quotation
                let quot = match self.pop()? {
                    Value::Quotation(xts) => xts,
                    _ => return Err("iff requires a quotation".to_string()),
                };
                // Pop condition
                let cond = self.pop_num()?;

                // Execute quotation only if condition is true (non-zero)
                if cond != 0 {
                    for xt in &quot {
                        self.execute(xt, input)?;
                    }
                }
            }
            XT::Type => {
                // ( value -- type-string )
                let value = self.pop()?;
                let type_name = value.type_name();
                self.data_stack.push(Value::String(type_name.to_string()));
            }
            XT::Compiled(words) => {
                for word_xt in words {
                    self.execute(word_xt, input)?;
                }
            }
            XT::Literal(val) => {
                self.data_stack.push(val.clone());
            }
            XT::Native(func) => {
                func(self, input)?;
            }
        }
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Value, String> {
        self.data_stack.pop().ok_or_else(|| "Stack underflow".to_string())
    }

    pub fn pop_num(&mut self) -> Result<i64, String> {
        match self.pop()? {
            Value::Number(n) => Ok(n),
            _ => Err("Expected number".to_string()),
        }
    }

    // Check type stack against signature and update it
    fn check_and_update_types(&mut self, sig: &Signature, word_name: &str) -> Result<(), String> {
        // Check we have enough types on the stack
        if self.type_stack.len() < sig.inputs.len() {
            return Err(format!(
                "Type error in '{}': expected {} inputs but type stack only has {}",
                word_name,
                sig.inputs.len(),
                self.type_stack.len()
            ));
        }

        // Check the types match (from the end of the stack)
        let stack_len = self.type_stack.len();
        for (i, expected_type) in sig.inputs.iter().enumerate() {
            let stack_idx = stack_len - sig.inputs.len() + i;
            let actual_type = &self.type_stack[stack_idx];
            if actual_type != expected_type {
                return Err(format!(
                    "Type error in '{}': expected {} at position {} but got {}",
                    word_name,
                    expected_type.name(),
                    i,
                    actual_type.name()
                ));
            }
        }

        // Pop the input types
        for _ in 0..sig.inputs.len() {
            self.type_stack.pop();
        }

        // Push the output types
        for output_type in &sig.outputs {
            self.type_stack.push(output_type.clone());
        }

        Ok(())
    }
}

// Native word implementations

fn native_colon(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // : word-name ...
    // Read the next token as the word name
    let name = input.next_token()?
        .ok_or("Expected word name after ':'")?;

    forth.compiling = true;
    forth.current_def.clear();
    forth.current_def_cids.clear();
    forth.current_name = Some(name);
    forth.type_stack.clear();

    // If there's a current signature, initialize type stack with its inputs
    if let Some(sig) = &forth.current_signature {
        for input_type in &sig.inputs {
            forth.type_stack.push(input_type.clone());
        }
    }

    Ok(())
}

fn native_colon_immediate(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // :: word-name ...
    // Same as : but the resulting word will be immediate
    let name = input.next_token()?
        .ok_or("Expected word name after '::'")?;

    forth.compiling = true;
    forth.current_def.clear();
    forth.current_def_cids.clear();
    forth.current_name = Some(format!("__IMMEDIATE__{}", name)); // Mark as immediate
    forth.type_stack.clear();

    // If there's a current signature, initialize type stack with its inputs
    if let Some(sig) = &forth.current_signature {
        for input_type in &sig.inputs {
            forth.type_stack.push(input_type.clone());
        }
    }

    Ok(())
}

fn native_semicolon(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    if !forth.compiling {
        return Err("Not in compilation mode".to_string());
    }

    let name = forth.current_name.take()
        .ok_or("No word name set")?;

    // Check if this is an immediate word definition
    let (is_immediate, actual_name) = if let Some(stripped) = name.strip_prefix("__IMMEDIATE__") {
        (true, stripped.to_string())
    } else {
        (false, name)
    };

    // Create the compiled word
    let xt = XT::Compiled(forth.current_def.clone());

    // Compute CID for the word from its CID sequence
    let word_cid = if !forth.current_def_cids.is_empty() {
        Some(CID::from_sequence(&forth.current_def_cids))
    } else {
        None
    };

    let mut word = if is_immediate {
        let mut w = Word::immediate(xt);
        w.cid = word_cid;
        w
    } else {
        let mut w = Word::new(xt);
        w.cid = word_cid;
        w
    };

    // Attach current signature if one exists and verify type stack
    if let Some(sig) = forth.current_signature.clone() {
        // Verify the type stack matches the signature's outputs
        if forth.type_stack.len() != sig.outputs.len() {
            return Err(format!(
                "Type error in '{}': signature expects {} outputs but type stack has {}",
                actual_name,
                sig.outputs.len(),
                forth.type_stack.len()
            ));
        }
        for (i, expected_type) in sig.outputs.iter().enumerate() {
            if &forth.type_stack[i] != expected_type {
                return Err(format!(
                    "Type error in '{}': output {} should be {} but got {}",
                    actual_name,
                    i,
                    expected_type.name(),
                    forth.type_stack[i].name()
                ));
            }
        }
        word.signature = Some(sig);
    }

    // Add word to current namespace (top of stack)
    let current_ns_idx = *forth.namespace_stack.last().unwrap();
    forth.namespaces[current_ns_idx].insert(actual_name, word);

    forth.compiling = false;
    forth.current_def.clear();
    forth.current_def_cids.clear();
    forth.type_stack.clear();  // Clear type stack

    Ok(())
}

fn native_lparen(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // ( starts a quotation
    if forth.in_quotation {
        // Nested quotation
        forth.quotation_depth += 1;
    } else {
        // Start new quotation
        forth.in_quotation = true;
        forth.quotation_depth = 1;
        forth.current_quotation.clear();
    }
    Ok(())
}

fn native_rparen(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // ) ends a quotation
    if !forth.in_quotation {
        return Err("Unexpected ')' without matching '('".to_string());
    }

    forth.quotation_depth -= 1;

    if forth.quotation_depth == 0 {
        // End of outermost quotation
        forth.in_quotation = false;

        // Create the quotation value
        let quot = Value::Quotation(forth.current_quotation.clone());

        if forth.compiling {
            // Compiling: add quotation as literal to current definition
            forth.current_def.push(XT::Literal(quot));
        } else {
            // Interpreting: push quotation onto data stack
            forth.data_stack.push(quot);
        }

        forth.current_quotation.clear();
    }

    Ok(())
}

fn native_context(_forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // CONTEXT. ... ;
    // For now, just a placeholder - we'll implement contexts later
    // Just consume tokens until semicolon
    // TODO: Actually parse and store contexts
    Err("CONTEXT. not yet implemented".to_string())
}

fn native_namespace(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // NAMESPACE. name ;
    // Pushes a new namespace onto the stack (or switches to existing)

    let namespace = input.next_token()?
        .ok_or("Expected namespace name after 'NAMESPACE.'")?;

    // Check for terminating semicolon
    let terminator = input.next_token()?
        .ok_or("Expected ';' after namespace name")?;

    if terminator != ";" {
        return Err(format!("Expected ';' after namespace name, got '{}'", terminator));
    }

    // Check if this namespace already exists
    if let Some(idx) = forth.find_namespace_index(&namespace) {
        // Namespace exists - push its index onto the stack
        forth.namespace_stack.push(idx);
        return Ok(());
    }

    // Create new namespace
    let new_idx = forth.namespaces.len();
    forth.namespaces.push(HashMap::new());
    forth.namespace_names.push(namespace);
    forth.namespace_stack.push(new_idx);
    Ok(())
}

fn native_import(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // IMPORT. namespace ;
    // Adds an existing namespace to the lookup stack

    let namespace = input.next_token()?
        .ok_or("Expected namespace name after 'IMPORT.'")?;

    // Check for terminating semicolon
    let terminator = input.next_token()?
        .ok_or("Expected ';' after namespace name")?;

    if terminator != ";" {
        return Err(format!("Expected ';' after namespace name, got '{}'", terminator));
    }

    // Find the namespace
    if let Some(idx) = forth.find_namespace_index(&namespace) {
        // Push the namespace index onto the stack for lookup
        forth.namespace_stack.push(idx);
        Ok(())
    } else {
        Err(format!("Namespace '{}' not found", namespace))
    }
}

fn native_alias(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // ALIAS. new-name existing-word ;
    // Creates an alias for an existing word

    let alias_name = input.next_token()?
        .ok_or("Expected alias name after 'ALIAS.'")?;

    let target_name = input.next_token()?
        .ok_or("Expected target word name")?;

    // Check for terminating semicolon
    let terminator = input.next_token()?
        .ok_or("Expected ';' after target name")?;

    if terminator != ";" {
        return Err(format!("Expected ';' after target name, got '{}'", terminator));
    }

    // Look up the target word
    if let Some(word) = forth.lookup_word(&target_name) {
        // Add alias to current namespace
        let current_ns_idx = *forth.namespace_stack.last().unwrap();
        forth.namespaces[current_ns_idx].insert(alias_name, word);
        Ok(())
    } else {
        Err(format!("Target word '{}' not found", target_name))
    }
}

fn native_variable(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // VARIABLE. name ... ;
    // Creates a variable in global state with initial value computed from code

    let var_name = input.next_token()?
        .ok_or("Expected variable name after 'VARIABLE.'")?;

    // Save current stack state
    let saved_stack = forth.data_stack.clone();

    // Peek at next token to see if it's an optional '='
    let first_token = input.next_token()?
        .ok_or("Expected expression or ';' after variable name")?;

    // Skip optional '=' sign
    let first_real_token = if first_token == "=" {
        input.next_token()?
            .ok_or("Expected expression after '='")?
    } else {
        first_token
    };

    // If first token is semicolon, error - need a value
    if first_real_token == ";" {
        return Err("VARIABLE. requires a value expression".to_string());
    }

    // Execute the first token
    forth.eval_token_with_input(&first_real_token, input)?;

    // Execute remaining tokens until we hit semicolon
    loop {
        let token = input.next_token()?
            .ok_or("Expected ';' to end VARIABLE.")?;

        if token == ";" {
            break;
        }

        // Execute the token to compute the value
        forth.eval_token_with_input(&token, input)?;
    }

    // Pop the computed value from stack
    let initial_value = forth.pop()
        .map_err(|_| "VARIABLE. requires a value on the stack".to_string())?;

    // Restore stack state (variable definition doesn't affect caller's stack)
    forth.data_stack = saved_stack;

    // Store in global state (always as immutable)
    forth.global_state.insert(var_name, initial_value);

    Ok(())
}

fn native_store(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // value -> varname
    // Stores TOS into variable (converting to immutable if needed)

    let var_name = input.next_token()?
        .ok_or("Expected variable name after '->'")?;

    // Pop value from stack
    let mut value = forth.pop()?;

    // Convert mutable to immutable if needed
    value = match value {
        Value::MutableArray(vec) => Value::Array(vec.into_iter().collect()),
        Value::MutableMap(map) => Value::Map(map.into_iter().collect()),
        v => v, // Already immutable or doesn't need conversion
    };

    // Store in global state
    forth.global_state.insert(var_name, value);

    Ok(())
}

fn native_mutable(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // Converts immutable collection to mutable
    let value = forth.pop()?;

    let mutable_value = match value {
        Value::Array(vec) => Value::MutableArray(vec.into_iter().collect()),
        Value::Map(map) => Value::MutableMap(map.into_iter().collect()),
        v => v, // Already mutable or doesn't need conversion (Number, String, Quotation)
    };

    forth.data_stack.push(mutable_value);
    Ok(())
}

fn native_immutable(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // Converts mutable collection to immutable
    let value = forth.pop()?;

    let immutable_value = match value {
        Value::MutableArray(vec) => Value::Array(vec.into_iter().collect()),
        Value::MutableMap(map) => Value::Map(map.into_iter().collect()),
        v => v, // Already immutable or doesn't need conversion (Number, String, Quotation)
    };

    forth.data_stack.push(immutable_value);
    Ok(())
}

// Generic type check: ( value type -- bool )
fn native_type_check(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    let expected_type = match forth.pop()? {
        Value::Type(t) => t,
        _ => return Err("? requires a type as second argument".to_string()),
    };
    let value = forth.pop()?;
    let result = if value.get_type() == expected_type { 1 } else { 0 };
    forth.data_stack.push(Value::Number(result));
    Ok(())
}

// Generic type cast: ( value type -- converted-value )
fn native_type_cast(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    let target_type = match forth.pop()? {
        Value::Type(t) => t,
        _ => return Err("! requires a type as second argument".to_string()),
    };
    let value = forth.pop()?;

    // If already the correct type, return as-is
    if value.get_type() == target_type {
        forth.data_stack.push(value);
        return Ok(());
    }

    // Type conversion logic
    let result = match (value, target_type) {
        // Convert to i64
        (Value::String(s), crate::value::Type::I64) => {
            let n = s.parse::<i64>()
                .map_err(|_| format!("Cannot convert string '{}' to i64", s))?;
            Value::Number(n)
        }
        // Convert to string
        (Value::Number(n), crate::value::Type::String) => Value::String(n.to_string()),

        // Add more conversions as needed
        (val, target) => {
            return Err(format!("Cannot convert {} to {}", val.type_name(), target.name()));
        }
    };

    forth.data_stack.push(result);
    Ok(())
}

// SIGNATURE. type1 type2 -> type3 type4 ;
fn native_signature(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut seen_arrow = false;

    loop {
        let token = input.next_token()?
            .ok_or("Expected types or ';' in SIGNATURE.")?;

        if token == ";" {
            break;
        }

        if token == "->" {
            if seen_arrow {
                return Err("Multiple '->' in SIGNATURE.".to_string());
            }
            seen_arrow = true;
            continue;
        }

        // Token should be a type name - look it up as a word
        if let Some(word) = forth.lookup_word(&token) {
            // Execute to get the type value
            if let XT::Literal(Value::Type(t)) = word.xt {
                if seen_arrow {
                    outputs.push(t);
                } else {
                    inputs.push(t);
                }
            } else {
                return Err(format!("'{}' is not a type", token));
            }
        } else {
            return Err(format!("Unknown type: {}", token));
        }
    }

    // Store the signature
    forth.current_signature = Some(Signature::new(inputs, outputs));
    Ok(())
}

// sig - prints the signature of the next word
fn native_sig(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    let word_name = input.next_token()?
        .ok_or("Expected word name after 'sig'")?;

    if let Some(word) = forth.lookup_word(&word_name) {
        if let Some(sig) = &word.signature {
            // Print signature
            print!("(");
            for (i, input_type) in sig.inputs.iter().enumerate() {
                if i > 0 { print!(" "); }
                print!("{}", input_type.name());
            }
            print!(" -> ");
            for (i, output_type) in sig.outputs.iter().enumerate() {
                if i > 0 { print!(" "); }
                print!("{}", output_type.name());
            }
            println!(")");
        } else {
            println!("<no signature>");
        }
    } else {
        return Err(format!("Unknown word: {}", word_name));
    }
    Ok(())
}

fn native_test(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // TEST. name ... ;
    // Reads test name, executes code until semicolon,
    // and checks if result is truthy (0 = fail, non-zero = pass)

    // Read the test name
    let test_name = input.next_token()?
        .ok_or("Expected test name after 'TEST.'")?;

    // Save current stack state
    let saved_stack = forth.data_stack.clone();

    // Execute tokens until we hit semicolon
    loop {
        let token = input.next_token()?
            .ok_or("Expected ';' to end TEST.")?;

        if token == ";" {
            break;
        }

        // Use the normal evaluation path which handles quotations, immediates, etc.
        forth.eval_token_with_input(&token, input)?;
    }

    // Check the result (0 = false/fail, non-zero = true/pass)
    let result = if let Some(Value::Number(n)) = forth.data_stack.pop() {
        n != 0
    } else {
        false
    };

    // Restore stack state
    forth.data_stack = saved_stack;

    // Print result
    if result {
        println!("✓ PASS: {}", test_name);
    } else {
        println!("✗ FAIL: {}", test_name);
    }

    Ok(())
}

fn native_line_comment(_forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // -- comment to end of line
    // Just skip the rest of the current line
    input.skip_to_eol();
    Ok(())
}
