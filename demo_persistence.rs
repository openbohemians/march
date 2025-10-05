use march::*;

fn main() {
    println!("=== March Persistence Demo ===");

    let db_path = "demo.db";

    // Session 1: Create some words
    println!("\n--- Session 1: Defining words ---");
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        interp.execute_word(": square dup * ;").unwrap();
        interp.execute_word(": cube dup square * ;").unwrap();
        interp.execute_word(": double dup + ;").unwrap();

        println!("Defined: square, cube, double");

        // Test them
        interp.execute_word("5").unwrap();
        interp.execute_word("square").unwrap();
        println!("5 square = {}", match interp.pop().unwrap().0 {
            Value::I64(n) => n.to_string(),
            _ => "?".to_string(),
        });
    }

    // Session 2: Use existing words, define new ones
    println!("\n--- Session 2: Using persisted words ---");
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Use words from session 1
        interp.execute_word("3").unwrap();
        interp.execute_word("cube").unwrap();
        println!("3 cube = {}", match interp.pop().unwrap().0 {
            Value::I64(n) => n.to_string(),
            _ => "?".to_string(),
        });

        // Define new words using old words
        interp.execute_word(": sixth_power cube cube ;").unwrap();
        println!("Defined: sixth_power (using cube)");

        interp.execute_word("2").unwrap();
        interp.execute_word("sixth_power").unwrap();
        println!("2 sixth_power = {}", match interp.pop().unwrap().0 {
            Value::I64(n) => n.to_string(),
            _ => "?".to_string(),
        });
    }

    // Session 3: Show all words persist
    println!("\n--- Session 3: Listing all persisted words ---");
    {
        let interp = Interpreter::with_database(db_path).unwrap();

        let words = interp.db.list_words(None).unwrap();
        println!("Total words in database: {}", words.len());

        for word in &words {
            if let Ok(Some(body)) = interp.db.lookup_word(word, None) {
                println!("  {} : {}", word, body);
            }
        }
    }

    println!("\n=== Database file '{}' contains all words! ===", db_path);
    println!("You can now run 'cargo run' to start a REPL with these words loaded.");
}