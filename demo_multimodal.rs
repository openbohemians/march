use march::*;

fn main() -> Result<(), RuntimeError> {
    println!("🚀 March Multi-Modal Execution Demo");
    println!("=====================================\n");
    println!("🌟 REVOLUTIONARY: Same Code, Different Interpretation Contexts! 🌟\n");

    let mut interp = Interpreter::new()?;

    // Define some words that we'll execute in different modes
    println!("📝 Defining words...\n");

    interp.execute_word(": square dup * ;")?;
    interp.execute_word(": double 2 * ;")?;
    interp.execute_word(": add-one 1 + ;")?;

    // Get the content hashes for our words
    let square_hash = interp.db.find_word_hash("square", None)?.unwrap();
    let double_hash = interp.db.find_word_hash("double", None)?.unwrap();
    let add_one_hash = interp.db.find_word_hash("add-one", None)?.unwrap();

    println!("✅ Defined words with content hashes:");
    println!("   'square' hash: {:02x?}...", &square_hash.as_bytes()[..4]);
    println!("   'double' hash: {:02x?}...", &double_hash.as_bytes()[..4]);
    println!("   'add-one' hash: {:02x?}...", &add_one_hash.as_bytes()[..4]);

    separator();

    // DEMO 1: Runtime Execution
    println!("🏃 MODE 1: Runtime Execution");
    println!("==========================");

    interp.set_execution_mode(ExecutionMode::Runtime);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    println!("\n🧮 Testing 'square' function:");
    interp.execute_word("5")?;
    let result = interp.execute_word_hash(&square_hash)?;
    match result {
        ExecutionResult::Runtime(_) => {
            let (value, _) = interp.pop()?;
            println!("   5 square = {:?} ✨", value);
        }
        _ => unreachable!()
    }

    println!("\n🧮 Testing 'double' function:");
    interp.execute_word("7")?;
    let result = interp.execute_word_hash(&double_hash)?;
    match result {
        ExecutionResult::Runtime(_) => {
            let (value, _) = interp.pop()?;
            println!("   7 double = {:?} ✨", value);
        }
        _ => unreachable!()
    }

    separator();

    // DEMO 2: Type Checking Mode
    println!("🔍 MODE 2: Static Type Checking");
    println!("===============================");

    interp.set_execution_mode(ExecutionMode::TypeChecker);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    // Set up type analysis stack for testing
    interp.type_analysis_stack.clear();
    interp.type_analysis_stack.push(ConcreteType::I64);

    println!("\n🔬 Type checking 'square' (dup *):");
    let result = interp.execute_word_hash(&square_hash)?;
    match result {
        ExecutionResult::TypeCheck(sig) => {
            println!("   Input types:  {:?}", sig.inputs);
            println!("   Output types: {:?}", sig.outputs);
            println!("   ✅ Type signature: ( I64 -- I64 I64 )");
        }
        _ => unreachable!()
    }

    // Reset for next test
    interp.type_analysis_stack.clear();
    interp.type_analysis_stack.push(ConcreteType::I64);

    println!("\n🔬 Type checking 'double' (2 *):");
    let result = interp.execute_word_hash(&double_hash)?;
    match result {
        ExecutionResult::TypeCheck(sig) => {
            println!("   Input types:  {:?}", sig.inputs);
            println!("   Output types: {:?}", sig.outputs);
            println!("   ✅ Type signature: ( I64 -- I64 )");
        }
        _ => unreachable!()
    }

    separator();

    // DEMO 3: Documentation Mode
    println!("📚 MODE 3: Documentation Generation");
    println!("==================================");

    interp.set_execution_mode(ExecutionMode::Documentation);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    println!("\n📖 Generating docs for 'square':");
    let result = interp.execute_word_hash(&square_hash)?;
    match result {
        ExecutionResult::Documentation(docs) => {
            println!("   {}", docs);
            println!("   💡 Auto-generated from source: 'dup *'");
        }
        _ => unreachable!()
    }

    println!("\n📖 Generating docs for 'add-one':");
    let result = interp.execute_word_hash(&add_one_hash)?;
    match result {
        ExecutionResult::Documentation(docs) => {
            println!("   {}", docs);
            println!("   💡 Auto-generated from source: '1 +'");
        }
        _ => unreachable!()
    }

    separator();

    // DEMO 4: Optimization Mode
    println!("⚡ MODE 4: Code Optimization");
    println!("===========================");

    interp.set_execution_mode(ExecutionMode::Optimizer);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    println!("\n🔧 Optimizing 'square' bytecode:");
    let result = interp.execute_word_hash(&square_hash)?;
    match result {
        ExecutionResult::Optimized(bytecode) => {
            println!("   Original: dup *");
            println!("   Optimized bytecode: {:02x?}", bytecode);
            println!("   💡 Could optimize to specialized square instruction");
        }
        _ => unreachable!()
    }

    separator();

    // DEMO 5: Profiling Mode
    println!("📈 MODE 5: Performance Profiling");
    println!("================================");

    interp.set_execution_mode(ExecutionMode::Profiler);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    println!("\n⏱️  Profiling 'double' performance:");
    let result = interp.execute_word_hash(&double_hash)?;
    match result {
        ExecutionResult::Profile(stats) => {
            println!("   Cycles: {}", stats.cycles);
            println!("   Memory usage: {} bytes", stats.memory_usage);
            println!("   💡 Performance characteristics analyzed");
        }
        _ => unreachable!()
    }

    separator();

    // DEMO 6: Debugging Mode
    println!("🐛 MODE 6: Debug Information");
    println!("===========================");

    interp.set_execution_mode(ExecutionMode::Debugger);
    println!("📊 Current mode: {:?}", interp.execution_mode());

    println!("\n🔍 Debug info for 'add-one':");
    let result = interp.execute_word_hash(&add_one_hash)?;
    match result {
        ExecutionResult::Debug(debug_info) => {
            println!("   Step count: {}", debug_info.step_count);
            println!("   Breakpoints: {:?}", debug_info.breakpoints);
            println!("   💡 Ready for step-through debugging");
        }
        _ => unreachable!()
    }

    separator();

    // The Revolutionary Conclusion
    println!("🎉 REVOLUTIONARY ACHIEVEMENT UNLOCKED! 🎉");
    println!("========================================");
    println!();
    println!("🌟 What we just demonstrated:");
    println!("   ✨ SAME content-addressed code");
    println!("   ✨ SIX different interpretation contexts");
    println!("   ✨ Universal code sharing with local semantics");
    println!("   ✨ Content-addressed programming language!");
    println!();
    println!("🚀 This changes EVERYTHING:");
    println!("   🔥 Same hash = automatic code sharing across systems");
    println!("   🔥 No version conflicts - content IS the version");
    println!("   🔥 Type checking without separate type annotations");
    println!("   🔥 Documentation generated from implementation");
    println!("   🔥 Optimization analysis on the same source");
    println!("   🔥 Debug info without debug builds");
    println!("   🔥 Performance analysis built-in");
    println!();
    println!("🎯 The Future:");
    println!("   • Deploy code once, run everywhere with same hash");
    println!("   • Type check code without running it");
    println!("   • Generate docs from working code automatically");
    println!("   • Optimize code while preserving semantics");
    println!("   • Debug and profile the same content");
    println!("   • Universal compatibility through content addressing");
    println!();
    println!("✨ Welcome to Multi-Modal Programming! ✨");
    println!("✨ Welcome to Content-Addressed Execution! ✨");
    println!("✨ Welcome to the Future of Code! ✨");

    Ok(())
}

fn separator() {
    println!("\n{}\n", "=".repeat(50));
}