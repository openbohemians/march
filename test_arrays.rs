use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("📚 March Array System Test");
    println!("==========================\n");

    let mut interp = Interpreter::new()?;

    // Test empty array
    println!("📦 Testing empty array:");
    interp.execute_word("[]")?;
    println!("   ✅ Created: []");
    interp.execute_word(".s")?;

    // Clear stack
    interp.execute_word("drop")?;

    // Test numeric array
    println!("\n🔢 Testing numeric array:");
    interp.execute_word("[ 1 2 3 4 5 ]")?;
    println!("   ✅ Created: [ 1 2 3 4 5 ]");
    interp.execute_word(".s")?;

    // Clear stack
    interp.execute_word("drop")?;

    // Test type array (for context signatures)
    println!("\n🎯 Testing type array:");
    interp.execute_word("[ I64 I64 I64 ]")?;
    println!("   ✅ Created: [ I64 I64 I64 ]");
    interp.execute_word(".s")?;

    // Clear stack
    interp.execute_word("drop")?;

    // Test mixed array
    println!("\n🌈 Testing mixed array:");
    interp.execute_word("[ I64 String 42 Vector ]")?;
    println!("   ✅ Created: [ I64 String 42 Vector ]");
    interp.execute_word(".s")?;

    // Clear stack
    interp.execute_word("drop")?;

    // Test array in thunk (the key use case!)
    println!("\n🚀 Testing array in thunk (context signature):");
    interp.execute_word("{ [ I64 I64 I64 ] true }")?;
    println!("   ✅ Created thunk: {{ [ I64 I64 I64 ] true }}");
    interp.execute_word(".s")?;

    // Force the thunk to see what it returns
    println!("\n⚡ Forcing context signature thunk:");
    interp.execute_word("force")?;
    interp.execute_word(".s")?;

    println!("\n✨ Array System Features Demonstrated:");
    println!("   ✅ Empty arrays: []");
    println!("   ✅ Numeric arrays: [ 1 2 3 ]");
    println!("   ✅ Type arrays: [ I64 String I64 ]");
    println!("   ✅ Mixed arrays: [ I64 42 Vector ]");
    println!("   ✅ Arrays in thunks: {{ [ types... ] condition }}");

    println!("\n🎯 Ready for Context Signature Thunks:");
    println!("   • Arrays provide type lists for signatures");
    println!("   • Thunks can return [types...] boolean");
    println!("   • Perfect foundation for type dispatch!");

    Ok(())
}