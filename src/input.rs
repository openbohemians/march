// Input buffer and token stream management
//
// FORTH uses an input buffer that can be refilled from various sources.
// The interpreter consumes tokens from this buffer one at a time.

use std::io::{self, BufRead, Write};

pub struct InputBuffer {
    buffer: String,
    position: usize,
    source: Box<dyn BufRead>,
    interactive: bool,
}

impl InputBuffer {
    pub fn new_interactive() -> Self {
        InputBuffer {
            buffer: String::new(),
            position: 0,
            source: Box::new(io::stdin().lock()),
            interactive: true,
        }
    }

    pub fn new_from_reader(reader: Box<dyn BufRead>) -> Self {
        InputBuffer {
            buffer: String::new(),
            position: 0,
            source: reader,
            interactive: false,
        }
    }

    // Get the next token from the input buffer
    pub fn next_token(&mut self) -> Result<Option<String>, String> {
        loop {
            // Skip whitespace
            while self.position < self.buffer.len()
                && self.buffer[self.position..].chars().next().unwrap().is_whitespace() {
                self.position += 1;
            }

            // If we have remaining content, extract a token
            if self.position < self.buffer.len() {
                let start = self.position;
                let rest = &self.buffer[start..];

                // Find the end of the token (next whitespace)
                if let Some(end_offset) = rest.find(|c: char| c.is_whitespace()) {
                    self.position = start + end_offset;
                    return Ok(Some(rest[..end_offset].to_string()));
                } else {
                    // Rest of buffer is the token
                    self.position = self.buffer.len();
                    return Ok(Some(rest.to_string()));
                }
            }

            // Buffer is exhausted, try to refill
            if !self.refill()? {
                // No more input
                return Ok(None);
            }
        }
    }

    // Refill the buffer from the input source
    // Returns false if EOF reached
    fn refill(&mut self) -> Result<bool, String> {
        self.buffer.clear();
        self.position = 0;

        if self.interactive {
            print!("> ");
            io::stdout().flush().unwrap();
        }

        match self.source.read_line(&mut self.buffer) {
            Ok(0) => Ok(false), // EOF
            Ok(_) => Ok(true),
            Err(e) => Err(format!("Input error: {}", e)),
        }
    }

    // Check if we're at the end of input
    pub fn is_eof(&self) -> bool {
        self.position >= self.buffer.len()
    }

    // Skip to end of current line (for line comments)
    pub fn skip_to_eol(&mut self) {
        self.position = self.buffer.len();
    }

    // Read a string literal from the buffer (after the opening ")
    // Handles escape sequences like \"
    pub fn read_string_literal(&mut self) -> Result<String, String> {
        let mut result = String::new();
        let mut escaped = false;

        loop {
            // Check if we need to refill the buffer
            if self.position >= self.buffer.len() {
                if !self.refill()? {
                    return Err("Unterminated string literal (EOF)".to_string());
                }
            }

            let ch = self.buffer[self.position..].chars().next()
                .ok_or("Unexpected end of buffer")?;

            self.position += ch.len_utf8();

            if escaped {
                // Handle escape sequences
                match ch {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    _ => {
                        // Unknown escape, just keep the character
                        result.push('\\');
                        result.push(ch);
                    }
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                // Found closing quote
                return Ok(result);
            } else {
                result.push(ch);
            }
        }
    }
}
