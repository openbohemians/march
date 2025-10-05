use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🚀 March Content-Addressed Programming Language Demo");
    println!("================================================\n");

    let mut interp = Interpreter::new()?;

    println!("📝 Defining words with automatic content addressing...\n");

    // Define some basic words (using legacy interpreter for now)
    // TODO: Use hash-based runtime compilation once fully integrated
    interp.execute_word(": square dup * ;")?;
    interp.execute_word(": double 2 * ;")?;

    println!("✅ Defined 'square' and 'double' words");

    // Test basic execution
    println!("\n🧮 Testing basic execution:");
    interp.execute_word("5")?;
    interp.execute_word("square")?;
    let (result, _) = interp.pop()?;
    println!("   5 square = {:?}", result);

    interp.execute_word("7")?;
    interp.execute_word("double")?;
    let (result, _) = interp.pop()?;
    println!("   7 double = {:?}", result);

    // Demonstrate content addressing
    println!("\n🔗 Content Addressing Magic:");

    // Each word has a unique content hash based on its definition
    let square_hash = interp.db.find_word_hash("square", None)?.unwrap();
    let double_hash = interp.db.find_word_hash("double", None)?.unwrap();

    println!("   'square' hash: {:02x?}...", &square_hash.as_bytes()[..4]);
    println!("   'double' hash: {:02x?}...", &double_hash.as_bytes()[..4]);

    // Show that same content = same hash (universal code!)
    println!("\n🌍 Universal Code Sharing:");
    interp.execute_word(": square2 dup * ;")?;  // Same as square!
    let square2_hash = interp.db.find_word_hash("square2", None)?.unwrap();

    if square_hash == square2_hash {
        println!("   🎉 'square' and 'square2' have IDENTICAL hashes!");
        println!("   Same definition = Same hash = Universal code sharing!");
    } else {
        println!("   Different hashes (expected since we have simple hashing)");
        println!("   In full implementation, identical source would give identical hashes");
    }

    // Demonstrate state with content addressing
    println!("\n📊 State Management:");
    interp.execute_word("State: counter : Integer")?;
    interp.execute_word(": increment counter @ 1 + counter ! ;")?;

    interp.execute_word("increment")?;
    interp.execute_word("increment")?;
    interp.execute_word("increment")?;

    interp.execute_word("counter@")?;
    let (result, _) = interp.pop()?;
    println!("   After 3 increments: counter = {:?}", result);

    // Show database statistics
    println!("\n📈 Database Statistics:");
    let word_count = interp.db.word_count()?;
    println!("   Total unique content hashes stored: {}", word_count);

    let word_names = interp.db.list_words(None)?;
    println!("   Local word names defined: {:?}", word_names);

    println!("\n🔮 The Future:");
    println!("   • Same hash = automatic code sharing across systems");
    println!("   • No version conflicts - content is the version");
    println!("   • Tamper-proof code - can't change without changing hash");
    println!("   • Universal compatibility - hashes work everywhere");
    println!("   • Git for executable code!");

    println!("\n✨ March: The Content-Addressed Programming Language! ✨");

    Ok(())
}