use march::*;
use std::fs;

#[test]
fn test_database_persistence() {
    let db_path = "test_persistence.db";

    // Clean up any existing test database
    let _ = fs::remove_file(db_path);

    // Session 1: Define some words
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Define basic words
        interp.execute_word(": double dup + ;").unwrap();
        interp.execute_word(": square dup * ;").unwrap();
        interp.execute_word(": cube dup square * ;").unwrap();

        // Test they work
        interp.execute_word("3").unwrap();
        interp.execute_word("cube").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(27));
    } // Interpreter drops, database connection closes

    // Session 2: New interpreter, same database - words should persist
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Test that previously defined words still work
        interp.execute_word("5").unwrap();
        interp.execute_word("square").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(25));

        interp.execute_word("4").unwrap();
        interp.execute_word("double").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(8));

        // Define a new word that uses existing words
        interp.execute_word(": fourth_power square square ;").unwrap();

        interp.execute_word("2").unwrap();
        interp.execute_word("fourth_power").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(16));
    }

    // Session 3: Verify all words persist
    {
        let interp = Interpreter::with_database(db_path).unwrap();

        let words = interp.db.list_words(None).unwrap();
        assert_eq!(words.len(), 4);
        assert!(words.contains(&"double".to_string()));
        assert!(words.contains(&"square".to_string()));
        assert!(words.contains(&"cube".to_string()));
        assert!(words.contains(&"fourth_power".to_string()));

        // Test that fourth_power definition includes references to square
        let body = interp.db.lookup_word("fourth_power", None).unwrap().unwrap();
        assert_eq!(body, "square square");
    }

    // Clean up
    let _ = fs::remove_file(db_path);
}

#[test]
fn test_word_redefinition_persistence() {
    let db_path = "test_redefinition.db";

    // Clean up any existing test database
    let _ = fs::remove_file(db_path);

    // Session 1: Define a word
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();
        interp.execute_word(": greeting 42 ;").unwrap();

        interp.execute_word("greeting").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(42));
    }

    // Session 2: Redefine the word
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Verify old definition works
        interp.execute_word("greeting").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(42));

        // Redefine it
        interp.execute_word(": greeting 100 ;").unwrap();

        // Test new definition
        interp.execute_word("greeting").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(100));
    }

    // Session 3: Verify redefinition persisted
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        interp.execute_word("greeting").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(100)); // Should be the new definition

        // Verify only one word named "greeting" exists
        let words = interp.db.list_words(None).unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0], "greeting");
    }

    // Clean up
    let _ = fs::remove_file(db_path);
}