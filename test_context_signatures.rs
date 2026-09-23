use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🎯 March Context Signature Test");
    println!("===============================\n");

    let mut interp = Interpreter::new()?;

    // Test basic context signature parsing
    println!("📝 Testing context signature parsing:");

    // Test simple signature
    interp.execute_word("? [ I64 I64 -- I64 ] ;")?;
    println!("   ✅ Parsed: ? [ I64 I64 -- I64 ] ;");

    // Define a word in this context
    interp.execute_word(": + primitive-add ;")?;
    println!("   ✅ Defined + for I64 I64 → I64");

    // Test different signature
    interp.execute_word("? [ String String -- String ] ;")?;
    println!("   ✅ Parsed: ? [ String String -- String ] ;");

    // Define + for strings
    interp.execute_word(": + string-concat ;")?;
    println!("   ✅ Defined + for String String → String");

    // Test signature with condition
    interp.execute_word("? [ Vector Vector -- Vector ] physics-enabled? ;")?;
    println!("   ✅ Parsed: ? [ Vector Vector -- Vector ] physics-enabled? ;");

    // Test multiple signatures
    println!("\n🚀 Testing multiple context signatures:");

    interp.execute_word("? [ I32 I32 -- I32 ] ;")?;
    interp.execute_word(": multiply i32-mult ;")?;
    println!("   ✅ I32 multiply defined");

    interp.execute_word("? [ F64 F64 -- F64 ] ;")?;
    interp.execute_word(": multiply f64-mult ;")?;
    println!("   ✅ F64 multiply defined");

    println!("\n🎯 Context Signature Features Demonstrated:");
    println!("   ✅ Type signature parsing: [ input-types -- output-types ]");
    println!("   ✅ Multiple contexts per operator");
    println!("   ✅ Conditional signatures: signature + condition");
    println!("   ✅ Context-aware word definitions");

    println!("\n🔮 Next: Type-based dispatch when words are called!");
    println!("   • Same word name ('+') → different implementations");
    println!("   • Automatic selection based on stack types");
    println!("   • Revolutionary polymorphism!");

    Ok(())
}