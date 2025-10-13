// March2 FORTH - Bootstrap Architecture
//
// Building a proper FORTH from the ground up:
// - Tiny core in Rust
// - Everything else is words (including : and ;)
// - Bootstrap from minimal primitives

mod value;
mod xt;
mod word;
mod forth;
mod input;
mod repl;
mod cid;
mod serializable;
mod database;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let command = &args[1];
        match command.as_str() {
            "test" => {
                std::process::exit(repl::run_tests());
            }
            "repl" => {
                repl::run();
            }
            _ => {
                eprintln!("Unknown command: {}", command);
                eprintln!("Usage: march2 [test|repl]");
                eprintln!("  test - Run all test files in tests/ directory");
                eprintln!("  repl - Start interactive REPL (default)");
                std::process::exit(1);
            }
        }
    } else {
        // Default to REPL
        repl::run();
    }
}
