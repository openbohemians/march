use march::*;

#[test]
fn test_basic_contextual_dispatch() {
    let mut interp = Interpreter::new().unwrap();

    // Set up state
    interp.execute_word("State: mode : Integer").unwrap();

    // Define context condition (using integers for simplicity)
    interp.execute_word(": running? mode @ 1 = ;").unwrap();
    interp.execute_word(": paused? mode @ 2 = ;").unwrap();

    // Define contextual words
    interp.execute_word("Context running?").unwrap();
    interp.execute_word(": update 100 ;").unwrap();  // Return 100 for running

    interp.execute_word("Context paused?").unwrap();
    interp.execute_word(": update 200 ;").unwrap();  // Return 200 for paused

    interp.execute_word("Context ( default )").unwrap();
    interp.execute_word(": update 0 ;").unwrap();    // Return 0 for default

    // Test running context
    interp.execute_word("1").unwrap();
    interp.execute_word("mode!").unwrap();  // Set mode to "running" (1)
    interp.execute_word("update").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(100));

    // Test paused context
    interp.execute_word("2").unwrap();
    interp.execute_word("mode!").unwrap();  // Set mode to "paused" (2)
    interp.execute_word("update").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(200));

    // Test default context
    interp.execute_word("99").unwrap();
    interp.execute_word("mode!").unwrap(); // Set mode to unknown value
    interp.execute_word("update").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(0));
}

#[test]
fn test_complex_context_conditions() {
    let mut interp = Interpreter::new().unwrap();

    // Set up state
    interp.execute_word("State: speed : Integer").unwrap();
    interp.execute_word("State: laser : Integer").unwrap();

    // Define complex context conditions
    interp.execute_word(": high-speed? speed @ 100 > ;").unwrap();
    interp.execute_word(": laser-on? laser @ 1 = ;").unwrap();  // 1 = on, 0 = off
    interp.execute_word(": danger-zone? high-speed? laser-on? & ;").unwrap();

    // Define contextual behavior
    interp.execute_word("Context danger-zone?").unwrap();
    interp.execute_word(": operation 999 ;").unwrap();  // Emergency code

    interp.execute_word("Context ( default )").unwrap();
    interp.execute_word(": operation 42 ;").unwrap();   // Normal code

    // Test safe conditions
    interp.execute_word("50").unwrap();
    interp.execute_word("speed!").unwrap();
    interp.execute_word("0").unwrap();
    interp.execute_word("laser!").unwrap();  // slow, laser off
    interp.execute_word("operation").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(42));

    // Test dangerous conditions
    interp.execute_word("150").unwrap();
    interp.execute_word("speed!").unwrap();
    interp.execute_word("1").unwrap();
    interp.execute_word("laser!").unwrap(); // fast, laser on
    interp.execute_word("operation").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(999));
}

#[test]
fn test_multiple_contextual_definitions() {
    let mut interp = Interpreter::new().unwrap();

    // Set up state
    interp.execute_word("State: level : Integer").unwrap();

    // Define context conditions
    interp.execute_word(": beginner? level @ 1 = ;").unwrap();
    interp.execute_word(": expert? level @ 10 = ;").unwrap();

    // Define multiple contextual definitions for the same word
    interp.execute_word("Context beginner?").unwrap();
    interp.execute_word(": help 100 ;").unwrap();  // Beginner help code

    interp.execute_word("Context expert?").unwrap();
    interp.execute_word(": help 1000 ;").unwrap(); // Expert help code

    interp.execute_word("Context ( default )").unwrap();
    interp.execute_word(": help 50 ;").unwrap();   // Default help code

    // Test different levels
    interp.execute_word("1").unwrap();
    interp.execute_word("level!").unwrap();
    interp.execute_word("help").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(100));

    interp.execute_word("10").unwrap();
    interp.execute_word("level!").unwrap();
    interp.execute_word("help").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(1000));

    interp.execute_word("5").unwrap();
    interp.execute_word("level!").unwrap();
    interp.execute_word("help").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(50));
}

#[test]
fn test_context_persistence() {
    let db_path = "test_context_persistence.db";

    // Clean up any existing test database
    let _ = std::fs::remove_file(db_path);

    // Session 1: Define contextual words
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        interp.execute_word("State: debug : Integer").unwrap();
        interp.execute_word(": debug-mode? debug @ 1 = ;").unwrap();

        interp.execute_word("Context debug-mode?").unwrap();
        interp.execute_word(": log 999 ;").unwrap();  // Debug code

        interp.execute_word("Context ( default )").unwrap();
        interp.execute_word(": log 42 ;").unwrap();   // Normal code
    }

    // Session 2: Contextual definitions should persist
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Redeclare state and context predicates
        interp.execute_word("State: debug : Integer").unwrap();
        interp.execute_word(": debug-mode? debug @ 1 = ;").unwrap();

        // Test that contextual definitions work
        interp.execute_word("1").unwrap();
        interp.execute_word("debug!").unwrap();
        interp.execute_word("log").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(999));

        interp.execute_word("0").unwrap();
        interp.execute_word("debug!").unwrap();
        interp.execute_word("log").unwrap();
        let (result, _) = interp.pop().unwrap();
        assert_eq!(result, Value::I64(42));
    }

    // Clean up
    let _ = std::fs::remove_file(db_path);
}