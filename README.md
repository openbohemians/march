# March: Content-Addressed Programming Language

🚀 **Revolutionary Multi-Modal Execution System** 🚀

March is a groundbreaking programming language that combines FORTH heritage with content-addressed programming and multi-modal execution. The same code can be executed in completely different interpretation contexts!

## 🌟 Core Innovation: Multi-Modal Execution

**MIND-BLOWING CONCEPT**: The same content-addressed code can be executed in 6 different modes:

1. **🏃 Runtime Mode** - Normal program execution
2. **🔍 Type Checker Mode** - Static type analysis
3. **⚡ Optimizer Mode** - Code optimization analysis
4. **📚 Documentation Mode** - Auto-generate documentation
5. **📈 Profiler Mode** - Performance analysis
6. **🐛 Debugger Mode** - Debug information extraction

## 🔗 Content-Addressed Programming

- **Universal Code Sharing**: Same definition = Same hash universally
- **No Version Conflicts**: Content IS the version
- **Tamper-Proof**: Can't change code without changing hash
- **Git for Executable Code**: Content-addressed like Git objects

## 🏗️ Architecture

### WordHash System
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WordHash([u8; 32]);  // Fixed 32-byte SHA-256 hash

// Bit interpretation:
// 0x80+ = Reserved (primitives, literals, state ops)
// 0x7F- = Content hash (user-defined words)
```

### Multi-Modal Execution
```rust
pub enum ExecutionMode {
    Runtime,           // Normal execution
    TypeChecker,       // Static type analysis
    Optimizer,         // Code optimization
    Documentation,     // Doc generation
    Profiler,          // Performance analysis
    Debugger,          // Step-through debugging
}
```

## 🎯 Demos

### 1. Content Addressing Demo
```bash
cargo run --bin demo_content_addressing
```
Shows universal code sharing and hash-based word definitions.

### 2. Multi-Modal Execution Demo
```bash
cargo run --bin demo_multimodal
```
**THE REVOLUTIONARY DEMO** - Shows the same code executed in 6 different interpretation contexts!

### 3. Persistence Demo
```bash
cargo run --bin demo_persistence
```
Shows database-backed code storage with content addressing.

## 🧮 Example: Multi-Modal Square Function

Define once:
```forth
: square dup * ;
```

Execute in different modes:

**Runtime Mode:**
```rust
interp.set_execution_mode(ExecutionMode::Runtime);
// Result: Actually computes 5² = 25
```

**Type Checker Mode:**
```rust
interp.set_execution_mode(ExecutionMode::TypeChecker);
// Result: TypeSignature { inputs: [I64], outputs: [I64] }
```

**Documentation Mode:**
```rust
interp.set_execution_mode(ExecutionMode::Documentation);
// Result: "Documentation for word: dup *"
```

**Same hash, different interpretation contexts!** 🤯

## 🔥 Revolutionary Features

### ✨ Universal Code Sharing
- Same definition = Same hash across ALL systems
- No dependency hell - content is the dependency
- Automatic deduplication across projects

### ✨ Multi-Paradigm Programming
- Same code, multiple interpretation contexts
- Type check without separate annotations
- Generate docs from working code
- Optimize while preserving semantics

### ✨ Content-Addressed Execution
- ITC (Indirect Threaded Code) with hash→XT lookup
- Compile to hash sequences for universal compatibility
- Runtime compilation from content-addressed definitions

### ✨ FORTH Heritage + Modern Features
- Stack-based execution with dual value/type stacks
- Contextual dispatch with type-based multiple dispatch
- Compilation state modifiers affect subsequent definitions

## 🏛️ Technical Details

### Hash-Based Runtime
- **Primitives**: Reserved hash space (0x80+) for built-in operations
- **User Words**: Content hash space (0x7F-) for definitions
- **ITC Execution**: hash → ExecutionToken → native function
- **Universal Compatibility**: Same hash works everywhere

### Type System Integration
- **Type Stack**: Parallel to value stack for static analysis
- **Context Patterns**: `Context ( I64 I64 -- I64 )` for dispatch
- **Multi-Modal Types**: Same code, type analysis without execution

### Database Schema
```sql
-- Content-addressed word storage (universal)
CREATE TABLE words (
    content_hash BLOB(32) PRIMARY KEY,    -- WordHash
    body_source TEXT NOT NULL,
    body_bytecode BLOB,                   -- Compiled hash sequence
    context_condition TEXT,
    context_hash BLOB(32)
);

-- Name aliases (local)
CREATE TABLE word_names (
    name TEXT NOT NULL,
    namespace TEXT,
    content_hash BLOB(32) REFERENCES words(content_hash)
);
```

## 🚀 The Future

This changes programming forever:

- **Deploy Once, Run Everywhere**: Same hash = universal compatibility
- **Zero Version Conflicts**: Content addressing eliminates dependency hell
- **Multi-Modal Analysis**: Type check, optimize, document the same source
- **Universal Code Libraries**: Share exact implementations via hash
- **Tamper-Proof Software**: Content can't change without hash changing
- **Git for Executable Code**: Version control built into the language

## 🎉 Status

**REVOLUTIONARY BREAKTHROUGH ACHIEVED!** ✨

- ✅ Content-addressed hash system with bit interpretation
- ✅ Multi-modal execution (6 different interpretation contexts)
- ✅ Database-backed universal code storage
- ✅ Type-based contextual dispatch
- ✅ ITC execution with hash→XT lookup
- ✅ Comprehensive demos showcasing all features

**Welcome to the future of programming!** 🌟

---

*March: Where content is king, hashes are universal, and the same code runs in infinite contexts.* 🚀



