// Testing support - native Forth words for test framework

use crate::forth::Forth;
use crate::input::InputBuffer;
use crate::value::Value;

/// TEST. word - executes test code and reports pass/fail
/// Syntax: TEST. "description" code... ;
/// or: TEST. bare-name code... ;
pub fn native_test(forth: &mut Forth, input: &mut InputBuffer) -> Result<(), String> {
    // TEST. "description" ... ;
    // Reads test description (string), executes code until semicolon,
    // and checks if result is truthy (0 = fail, non-zero = pass)
    // Updates test.pass-count and test.fail-count global variables

    // Read the test description - expect a string literal
    let token = input.next_token()?
        .ok_or("Expected test description string after 'TEST.'")?;

    let test_name = if token.starts_with('"') && token.ends_with('"') && token.len() > 1 {
        // String literal - strip quotes
        token[1..token.len()-1].to_string()
    } else {
        // For backward compatibility, allow bare tokens too
        token
    };

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

    // Update test.pass-count and test.fail-count global variables
    if result {
        let current = forth.global_state
            .get("test.pass-count")
            .and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None })
            .unwrap_or(0);
        forth.global_state.insert("test.pass-count".to_string(), Value::Number(current + 1));
    } else {
        let current = forth.global_state
            .get("test.fail-count")
            .and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None })
            .unwrap_or(0);
        forth.global_state.insert("test.fail-count".to_string(), Value::Number(current + 1));
    }

    // Store result in test results list
    forth.test_results.push((test_name.clone(), result));

    // Print result
    if result {
        println!("✓ PASS: {}", test_name);
    } else {
        println!("✗ FAIL: {}", test_name);
    }

    Ok(())
}

/// test.print-results. word - prints test summary
/// Reads test.pass-count and test.fail-count from global state
pub fn native_test_print_results(forth: &mut Forth, _input: &mut InputBuffer) -> Result<(), String> {
    // test.print-results.
    // Prints test pass/fail counts from global state

    let pass_count = forth.global_state
        .get("test.pass-count")
        .and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0);

    let fail_count = forth.global_state
        .get("test.fail-count")
        .and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0);

    let total = pass_count + fail_count;

    println!("Test Results:");
    println!("  Total: {}", total);
    println!("  ✓ Passed: {}", pass_count);
    if fail_count > 0 {
        println!("  ✗ Failed: {}", fail_count);
    }

    Ok(())
}
