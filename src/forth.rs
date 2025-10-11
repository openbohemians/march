// Core FORTH interpreter

use std::collections::HashMap;
use crate::value::Value;
use crate::xt::XT;
use crate::word::Word;
use crate::input::InputBuffer;

pub struct Forth {
    pub data_stack: Vec<Value>,
    pub return_stack: Vec<Value>,  // Return stack for calls, loops, and temp storage
    pub dictionary: HashMap<String, Word>,
    pub compiling: bool,           // Are we in compilation mode?
    pub current_def: Vec<XT>,      // Current word being compiled
    pub current_name: Option<String>, // Name of word being compiled
    pub in_quotation: bool,        // Are we compiling a quotation?
    pub quotation_depth: usize,    // Nesting depth of quotations
    pub current_quotation: Vec<XT>, // Current quotation being compiled
}

impl Forth {
    pub fn new() -> Self {
        let mut forth = Forth {
            data_stack: Vec::new(),
            return_stack: Vec::new(),
            dictionary: HashMap::new(),
            compiling: false,
            current_def: Vec::new(),
            current_name: None,
            in_quotation: false,
            quotation_depth: 0,
            current_quotation: Vec::new(),
        };

        // Bootstrap: Add primitive words to dictionary
        forth.add_word("+", Word::new(XT::Add));
        forth.add_word("-", Word::new(XT::Sub));
        forth.add_word("*", Word::new(XT::Mul));
        forth.add_word("/", Word::new(XT::Div));
        forth.add_word("dup", Word::new(XT::Dup));
        forth.add_word("drop", Word::new(XT::Drop));
        forth.add_word("swap", Word::new(XT::Swap));
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

        // Quotation delimiters (immediate words)
        forth.add_word("(", Word::immediate(XT::Native(native_lparen)));
        forth.add_word(")", Word::immediate(XT::Native(native_rparen)));

        forth
    }

    fn add_word(&mut self, name: &str, word: Word) {
        self.dictionary.insert(name.to_string(), word);
    }

    pub fn eval_token_with_input(&mut self, token: &str, input: &mut InputBuffer) -> Result<(), String> {
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

        // Look up in dictionary
        if let Some(word) = self.dictionary.get(token).cloned() {
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
            XT::Dot => {
                let val = self.pop()?;
                match val {
                    Value::Number(n) => println!("{}", n),
                    Value::Quotation(_) => println!("<quotation>"),
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
            Value::Quotation(_) => Err("Expected number, got quotation".to_string()),
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

    forth.dictionary.insert(actual_name, word);

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
