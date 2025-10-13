// REPL - Read-Eval-Print Loop

use crate::forth::Forth;
use crate::input::InputBuffer;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::BufReader;
use std::fs;
use std::path::Path;

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

/// Run all test files in the tests/ directory
/// Returns exit code: 0 if all tests pass, 1 if any fail
pub fn run_tests() -> i32 {
    println!("March2 FORTH Test Runner\n");

    let test_dir = Path::new("tests");
    if !test_dir.exists() {
        eprintln!("Error: tests/ directory not found");
        return 1;
    }

    // Find all .fth test files
    let test_files: Vec<_> = match fs::read_dir(test_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "fth")
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect(),
        Err(e) => {
            eprintln!("Error reading tests/ directory: {}", e);
            return 1;
        }
    };

    if test_files.is_empty() {
        println!("No test files found in tests/");
        return 0;
    }

    let mut files_with_errors = Vec::new();

    // Run each test file
    for test_file in &test_files {
        let file_name = test_file.file_name().unwrap().to_string_lossy();
        println!("\n=== Running {} ===", file_name);

        // Create fresh interpreter for each file
        let mut forth = Forth::new();

        // Read and execute the file - TEST. words print their own results
        match fs::read_to_string(test_file) {
            Ok(content) => {
                let cursor = std::io::Cursor::new(content.into_bytes());
                let reader = BufReader::new(cursor);
                let mut input = InputBuffer::new_from_reader(Box::new(reader));

                // Run the file - TEST. words will print PASS/FAIL as they execute
                run_simple(&mut forth, &mut input);
            }
            Err(e) => {
                eprintln!("✗ Error reading file: {}", e);
                files_with_errors.push((file_name.to_string(), e.to_string()));
            }
        }
    }

    // Print summary
    println!("\n{}", "=".repeat(50));
    if !files_with_errors.is_empty() {
        println!("Files with errors: {}", files_with_errors.len());
        for (name, err) in &files_with_errors {
            println!("  ✗ {}: {}", name, err);
        }
        println!("{}", "=".repeat(50));
        1
    } else {
        println!("All test files completed");
        println!("{}", "=".repeat(50));
        0
    }
}
