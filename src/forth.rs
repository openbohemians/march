// Core FORTH interpreter

use std::collections::HashMap;
use crate::value::Value;
use crate::xt::XT;
use crate::word::Word;
use crate::input::InputBuffer;

pub struct Forth {
    pub data_stack: Vec<Value>,
    pub return_stack: Vec<Value>,  // Return stack for calls, loops, and temp storage
    pub namespace_stack: Vec<HashMap<String, Word>>, // Stack of namespace dictionaries
    pub namespace_names: Vec<String>, // Parallel stack of namespace names (for tracking)
    pub global_state: HashMap<String, Value>, // Immutable global state
    pub compiling: bool,           // Are we in compilation mode?
    pub current_def: Vec<XT>,      // Current word being compiled
    pub current_name: Option<String>, // Name of word being compiled
    pub in_quotation: bool,        // Are we compiling a quotation?
    pub quotation_depth: usize,    // Nesting depth of quotations
    pub current_quotation: Vec<XT>, // Current quotation being compiled
    pub lookup_namespace: Option<usize>, // Temporary namespace stack index for next word lookup
}

impl Forth {
    pub fn new() -> Self {
        let mut forth = Forth {
            data_stack: Vec::new(),
            return_stack: Vec::new(),
            namespace_stack: vec![HashMap::new()], // Start with root namespace
            namespace_names: vec![String::new()],  // Root namespace has empty name
            global_state: HashMap::new(),
            compiling: false,
            current_def: Vec::new(),
            current_name: None,
            in_quotation: false,
            quotation_depth: 0,
            current_quotation: Vec::new(),
            lookup_namespace: None,
        };

        // Bootstrap: Add primitive words to dictionary
        forth.add_word("+", Word::new(XT::Add));
        forth.add_word("-", Word::new(XT::Sub));
        forth.add_word("*", Word::new(XT::Mul));
        forth.add_word("/", Word::new(XT::Div));
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
        // Add word to the current namespace (top of stack)
        let current_ns = self.namespace_stack.len() - 1;
        self.namespace_stack[current_ns].insert(name.to_string(), word);
    }

    // Get current namespace index (top of stack)
    fn current_namespace_index(&self) -> usize {
        self.namespace_stack.len() - 1
    }

    // Check if a token is a namespace name that exists in the stack
    fn is_namespace(&self, token: &str) -> bool {
        self.namespace_names.iter().any(|name| name == token)
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
                    return self.namespace_stack[idx].get(word_name).cloned();
                }
                return None; // Qualified name but namespace not found
            }
            // Fall through if it's just "." or malformed
        }

        // Check if there's a temporary lookup namespace override
        if let Some(ns_idx) = self.lookup_namespace.take() {
            if let Some(word) = self.namespace_stack[ns_idx].get(name).cloned() {
                return Some(word);
            }
            // Fall through to regular lookup if not found
        }

        // Walk namespace stack backwards (most recent first)
        for ns in self.namespace_stack.iter().rev() {
            if let Some(word) = ns.get(name) {
                return Some(word.clone());
            }
        }

        None
    }

    pub fn eval_token_with_input(&mut self, token: &str, input: &mut InputBuffer) -> Result<(), String> {
        // Try to parse as string literal
        if token.starts_with('"') {
            let string_val = self.parse_string(token, input)?;
            let literal = XT::Literal(Value::String(string_val));
            if self.in_quotation {
                self.current_quotation.push(literal);
            } else if self.compiling {
                self.current_def.push(literal);
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
                self.current_quotation.push(word.xt);
            } else if self.compiling {
                // Compiling word: add to current definition
                self.current_def.push(word.xt);
            } else {
                // Interpreting: execute immediately
                self.execute(&word.xt, input)?;
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
}

// Native word implementations

fn native_colon(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // : word-name ...
    // Read the next token as the word name
    let name = input.next_token()?
        .ok_or("Expected word name after ':'")?;

    forth.compiling = true;
    forth.current_def.clear();
    forth.current_name = Some(name);

    Ok(())
}

fn native_colon_immediate(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // :: word-name ...
    // Same as : but the resulting word will be immediate
    let name = input.next_token()?
        .ok_or("Expected word name after '::'")?;

    forth.compiling = true;
    forth.current_def.clear();
    forth.current_name = Some(format!("__IMMEDIATE__{}", name)); // Mark as immediate

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
    let word = if is_immediate {
        Word::immediate(xt)
    } else {
        Word::new(xt)
    };

    // Add word to current namespace (top of stack)
    let current_ns = forth.namespace_stack.len() - 1;
    forth.namespace_stack[current_ns].insert(actual_name, word);

    forth.compiling = false;
    forth.current_def.clear();

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

fn native_signature(_forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // SIGNATURE. ... ;
    // For now, just a placeholder - we'll implement type signatures later
    // Just consume tokens until semicolon
    // TODO: Actually parse and store type signatures
    Err("SIGNATURE. not yet implemented".to_string())
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
    if let Some(_idx) = forth.find_namespace_index(&namespace) {
        // Namespace exists - for now just continue (could switch to it in future)
        return Ok(());
    }

    // Create new namespace
    forth.namespace_stack.push(HashMap::new());
    forth.namespace_names.push(namespace);
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
        // Clone the namespace and push it onto the stack
        let imported_ns = forth.namespace_stack[idx].clone();
        forth.namespace_stack.push(imported_ns);
        forth.namespace_names.push(format!("imported:{}", namespace));
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
        let current_ns = forth.namespace_stack.len() - 1;
        forth.namespace_stack[current_ns].insert(alias_name, word);
        Ok(())
    } else {
        Err(format!("Target word '{}' not found", target_name))
    }
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
