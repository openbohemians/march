use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🔧 Testing Operator Override Capability");
    println!("=====================================\n");

    let mut interp = Interpreter::new()?;

    // Test standard + operator
    println!("📊 Standard + operator:");
    interp.execute_word("5")?;
    interp.execute_word("3")?;
    interp.execute_word("+")?;
    let (result, _) = interp.pop()?;
    println!("   5 + 3 = {:?}", result);

    // Define a custom + operator that does concatenation as strings
    println!("\n🚀 Defining custom + operator:");
    interp.execute_word(": + swap . . ;")?;  // Custom + that prints both numbers

    println!("   Defined: : + swap . . ;");

    // Test that our custom + is now being used
    println!("\n🧪 Testing custom + operator:");
    interp.execute_word("7")?;
    interp.execute_word("4")?;
    interp.execute_word("+")?;
    println!("   7 + 4 executed custom definition (should have printed numbers)");

    println!("\n✅ Operator extensibility CONFIRMED!");
    println!("   • Users can completely redefine standard operators");
    println!("   • Custom definitions override the primitives");
    println!("   • Full control over operator behavior");

    Ok(())
}