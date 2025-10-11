// REPL - Read-Eval-Print Loop

use crate::forth::Forth;
use crate::input::InputBuffer;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::BufReader;

pub fn run() {
    println!("March2 FORTH v0.2 - Bootstrap Edition");
    println!("Type 'bye' to exit\n");

    let mut forth = Forth::new();

    // Try to use rustyline for interactive editing
    if atty::is(atty::Stream::Stdin) {
        run_with_rustyline(&mut forth);
    } else {
        // Non-interactive (piped input), use simple buffer
        let mut input = InputBuffer::new_interactive();
        run_simple(&mut forth, &mut input);
    }
}

fn run_with_rustyline(forth: &mut Forth) {
    let mut rl = DefaultEditor::new().expect("Failed to create editor");

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                if !line.trim().is_empty() {
                    let _ = rl.add_history_entry(&line);
                }

                // Create input buffer from the line
                let cursor = std::io::Cursor::new(line.into_bytes());
                let reader = BufReader::new(cursor);
                let mut input = InputBuffer::new_from_reader(Box::new(reader));

                // Process all tokens in the line
                loop {
                    match input.next_token() {
                        Ok(Some(token)) => {
                            if token == "bye" || token == "quit" {
                                return;
                            }
                            if let Err(e) = forth.eval_token_with_input(&token, &mut input) {
                                eprintln!("Error: {}", e);
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            eprintln!("Input error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
}

fn run_simple(forth: &mut Forth, input: &mut InputBuffer) {
    loop {
        match input.next_token() {
            Ok(Some(token)) => {
                if token == "bye" || token == "quit" {
                    break;
                }
                if let Err(e) = forth.eval_token_with_input(&token, input) {
                    eprintln!("Error: {}", e);
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("Input error: {}", e);
                break;
            }
        }
    }
}
