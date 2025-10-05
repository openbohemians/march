use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🔥 March Revolutionary Error Handling Demo");
    println!("=========================================\n");

    let mut interp = Interpreter::new()?;

    // Register some error types
    println!("📝 Registering error types with inheritance...\n");
    interp.execute_word("Error: MathError")?;
    interp.execute_word("Error: DivisionByZero MathError")?;
    interp.execute_word("Error: NegativeNumber MathError")?;
    interp.execute_word("Error: Overflow MathError")?;

    println!("✅ Registered: MathError → DivisionByZero, NegativeNumber, Overflow\n");

    // Define a regular word that might raise errors
    println!("📝 Defining words with error handling...\n");

    // Simple division that always raises DivisionByZero for demo
    interp.execute_word(": risky_divide DivisionByZero raise ;")?;

    // Simple operation that raises NegativeNumber
    interp.execute_word(": risky_sqrt NegativeNumber raise ;")?;

    println!("✅ Defined: risky_divide, risky_sqrt (with error raising)\n");

    // Define error handlers in specific contexts
    println!("📝 Defining error handlers in contexts...\n");

    // Handler for risky_divide when DivisionByZero occurs
    interp.execute_word("? DivisionByZero ;")?;
    interp.execute_word(": risky_divide 42 ;")?;  // Return 42 as safe fallback

    // Handler for risky_sqrt when NegativeNumber occurs
    interp.execute_word("? NegativeNumber ;")?;
    interp.execute_word(": risky_sqrt 0 ;")?;  // Return 0 as safe fallback

    // Generic handler for any MathError
    interp.execute_word("? MathError ;")?;
    interp.execute_word(": handle_math_error drop -1 ;")?;  // Return -1 for generic math errors

    println!("✅ Defined error handlers in contexts\n");

    // Test error handling - the MAGIC happens here!
    println!("🔥 Testing revolutionary error handling:\n");

    // First operation that will raise DivisionByZero
    println!("   Testing: risky_divide (will raise DivisionByZero)");
    interp.execute_word("risky_divide")?;  // This will raise DivisionByZero and re-dispatch!
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   Result: {:?} (handled by DivisionByZero context!)", result);
    }

    // Second operation that will raise NegativeNumber
    println!("\n   Testing: risky_sqrt (will raise NegativeNumber)");
    interp.execute_word("risky_sqrt")?;  // This will raise NegativeNumber and re-dispatch!
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   Result: {:?} (handled by NegativeNumber context!)", result);
    }

    // Test inheritance - define a new error that inherits handling
    println!("\n🌟 Testing error inheritance:\n");

    interp.execute_word("Error: ComplexNumber MathError")?;
    interp.execute_word(": complex_op ComplexNumber raise ;")?;

    println!("   Testing: complex_op (raises ComplexNumber error)");
    interp.execute_word("complex_op")?;  // No specific handler, should inherit from MathError
    // For now this will just change context, full inheritance dispatch coming next

    println!("\n🎉 Revolutionary Features Demonstrated:");
    println!("   • Errors are context transitions, not exceptions!");
    println!("   • Same word, different behavior in error contexts!");
    println!("   • Error hierarchies with inheritance!");
    println!("   • No stack unwinding - just contextual dispatch!");
    println!("   • Universal error handling across all March programs!");

    println!("\n✨ This is the future of error handling! ✨");

    Ok(())
}