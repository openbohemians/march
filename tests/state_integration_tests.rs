use march::*;

#[test]
fn test_state_declaration_and_access() {
    let mut interp = Interpreter::new().unwrap();

    // Declare state variables
    interp.execute_word("State: velocity : Integer").unwrap();
    interp.execute_word("State: mode : String").unwrap();

    // Test initial values
    interp.execute_word("velocity@").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(0));

    // Set state variables
    interp.execute_word("100").unwrap();
    interp.execute_word("velocity!").unwrap();

    // Get state variable
    interp.execute_word("velocity@").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(100));
}

#[test]
fn test_state_in_word_definitions() {
    let mut interp = Interpreter::new().unwrap();

    // Declare state
    interp.execute_word("State: counter : Integer").unwrap();

    // Define words that use state
    interp.execute_word(": increment counter @ 1 + counter ! ;").unwrap();
    interp.execute_word(": get_counter counter @ ;").unwrap();

    // Test initial value
    interp.execute_word("get_counter").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(0));

    // Increment and test
    interp.execute_word("increment").unwrap();
    interp.execute_word("get_counter").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(1));

    // Increment again
    interp.execute_word("increment").unwrap();
    interp.execute_word("get_counter").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(2));
}

#[test]
fn test_multiple_state_variables() {
    let mut interp = Interpreter::new().unwrap();

    // Declare multiple state variables
    interp.execute_word("State: x : Integer").unwrap();
    interp.execute_word("State: y : Integer").unwrap();

    // Set values
    interp.execute_word("10").unwrap();
    interp.execute_word("x!").unwrap();
    interp.execute_word("20").unwrap();
    interp.execute_word("y!").unwrap();

    // Define word that uses both
    interp.execute_word(": sum x @ y @ + ;").unwrap();

    // Test
    interp.execute_word("sum").unwrap();
    let (value, _) = interp.pop().unwrap();
    assert_eq!(value, Value::I64(30));
}

#[test]
fn test_state_persistence() {
    let db_path = "test_state_persistence.db";

    // Clean up any existing test database
    let _ = std::fs::remove_file(db_path);

    // Session 1: Define state and set values
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        interp.execute_word("State: global_counter : Integer").unwrap();
        interp.execute_word("42").unwrap();
        interp.execute_word("global_counter!").unwrap();

        interp.execute_word(": increment_global global_counter @ 1 + global_counter ! ;").unwrap();
        interp.execute_word("increment_global").unwrap();
    }

    // Session 2: State should persist (but note: state values don't persist yet, only definitions)
    {
        let mut interp = Interpreter::with_database(db_path).unwrap();

        // Redeclare state (values reset to defaults)
        interp.execute_word("State: global_counter : Integer").unwrap();

        // But word definitions should persist
        interp.execute_word("10").unwrap();
        interp.execute_word("global_counter!").unwrap();
        interp.execute_word("increment_global").unwrap();

        interp.execute_word("global_counter@").unwrap();
        let (value, _) = interp.pop().unwrap();
        assert_eq!(value, Value::I64(11));
    }

    // Clean up
    let _ = std::fs::remove_file(db_path);
}