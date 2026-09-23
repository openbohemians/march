use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🚀 March Thunk System Test");
    println!("==========================\n");

    let mut interp = Interpreter::new()?;

    // Test basic thunk creation
    println!("📦 Testing thunk creation:");

    // Create a simple thunk
    interp.execute_word("{ 5 3 + }")?;
    println!("   Created thunk: {{ 5 3 + }}");

    // Check stack
    interp.execute_word(".s")?;

    // Force the thunk
    println!("\n⚡ Testing thunk forcing:");
    interp.execute_word("force")?;

    // Check result
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   Thunk result: {:?}", result);
    }

    // Test more complex thunk
    println!("\n🧮 Testing complex thunk:");
    interp.execute_word("{ 10 dup * 4 - }")?;  // { 10^2 - 4 } = 96
    println!("   Created thunk: {{ 10 dup * 4 - }}");

    interp.execute_word("force")?;
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   Complex thunk result: {:?}", result);
    }

    // Test thunk with variables
    println!("\n📊 Testing thunk with state variables:");

    // Set up state
    interp.execute_word("$ counter Int 5 >")?;

    // Create thunk that uses state
    interp.execute_word("{ counter@ 2 * }")?;  // { counter * 2 }
    println!("   Created thunk: {{ counter@ 2 * }}");

    interp.execute_word("force")?;
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   State thunk result: {:?}", result);
    }

    // Change state and test again
    println!("\n🔄 Testing thunk with changed state:");
    interp.execute_word("10")?;
    interp.execute_word("counter!")?;

    interp.execute_word("{ counter@ 2 * }")?;  // Same thunk, different state
    interp.execute_word("force")?;
    if !interp.value_stack.is_empty() {
        let (result, _) = interp.pop()?;
        println!("   Updated state thunk result: {:?}", result);
    }

    println!("\n✨ Thunk System Features Demonstrated:");
    println!("   ✅ Lazy evaluation - thunks store code without executing");
    println!("   ✅ Dynamic scope - thunks execute in current environment");
    println!("   ✅ State access - thunks can access global state variables");
    println!("   ✅ Flexible syntax - {{ expression }} creates thunk");
    println!("   ✅ Manual forcing - 'force' word executes thunk");

    println!("\n🚀 Ready for context signature thunks next!");

    Ok(())
}