# PROGRESS.md - March2 FORTH Development

## Current Status: Variables & Type System Complete ✅

**Date:** 2025-10-12
**Branch:** main
**Version:** v0.3 - Variables & Types Edition

---

## Major Milestone: Clean Bootstrap Implementation

We successfully rebuilt March2 from the ground up using proper FORTH architecture, replacing the old parse/eval approach with a true bootstrap design.

### Architecture

**Modular Structure:**
```
src/
├── main.rs       - Entry point
├── repl.rs       - REPL with rustyline support
├── forth.rs      - Core interpreter
├── input.rs      - Token stream with refillable buffer
├── word.rs       - Word metadata (immediate flag)
├── xt.rs         - Execution tokens (compiled form)
└── value.rs      - Value types (Number, Quotation)
```

**Total:** ~500 lines of clean, well-organized Rust code

---

## Completed Features

### ✅ Core FORTH

- [x] **Data stack** - Standard FORTH data stack
- [x] **Return stack** - For calls, loops, temp storage (`>r`, `r>`, `r@`)
- [x] **Dictionary** - HashMap-based word lookup
- [x] **Token-by-token execution** - Proper input buffer with refill
- [x] **Compile/interpret modes** - Switch between modes correctly

### ✅ Word Definition

- [x] **`:` and `;`** - Define regular words (now proper words, not hardcoded!)
- [x] **`::` and `;`** - Define immediate words
- [x] **Immediate words** - Execute even during compilation
- [x] **Native words** - Rust functions with access to interpreter state
- [x] **User-defined words** - Compiled to execution token sequences

### ✅ Primitives

**Arithmetic:**
- `+` `-` `*` `/`

**Stack manipulation:**
- `dup` `drop` `swap` `over` `rot`

**Comparison operators:**
- `lt?` `gt?` `lte?` `gte?` `eq?` `neq?`
- Return 1 for true, 0 for false

**Return stack:**
- `>r` - Move to return stack
- `r>` - Move from return stack
- `r@` - Copy from return stack (peek)

**I/O:**
- `.` - Print top of stack

### ✅ Quotations (Code as Data)

- [x] **`( ... )`** - Create quotations (code blocks)
- [x] **`call`** - Execute a quotation
- [x] **Nested quotations** - Quotations can contain quotations
- [x] **Quotations in definitions** - Can compile quotations into words
- [x] **Quotations as values** - First-class values on the stack

Example:
```forth
( dup * ) call           ( execute quotation )
: square ( dup * ) call ;  ( quotation in definition )
```

### ✅ Conditionals

- [x] **`if`** - `( cond true-quot false-quot -- )` Execute branch based on condition
- [x] **`iff`** - `( cond quot -- )` Execute quotation only if true

Examples:
```forth
-5 ( 0 swap - ) ( ) if     ( absolute value using if )
5 ( 100 . ) iff             ( print 100 only if condition true )
```

### ✅ REPL Improvements

- [x] **Rustyline integration** - Arrow keys, command history, line editing
- [x] **Interactive mode detection** - Rustyline for terminal, simple buffer for pipes
- [x] **Ctrl+C handling** - Graceful exit
- [x] **Command history** - Up/Down arrows navigate history

### ✅ String Literals

- [x] **`"..."` syntax** - String literals with escape sequences
- [x] **Escape sequences** - `\"`, `\\`, `\n`, `\t` supported
- [x] **Whitespace preservation** - Spaces and formatting preserved correctly

Examples:
```forth
"Hello World" .           ( prints: Hello World )
"Line 1\nLine 2" .        ( prints across two lines )
"She said \"Hi\"" .       ( prints: She said "Hi" )
```

### ✅ Namespaces

- [x] **NAMESPACE. word** - Create new namespaces
- [x] **Dotted qualified names** - `io.net.http.request` style
- [x] **Namespace stack** - Efficient O(1) qualified lookups, O(k) unqualified
- [x] **IMPORT. word** - Import namespace onto stack for unqualified access
- [x] **ALIAS. word** - Create word aliases across namespaces

Examples:
```forth
NAMESPACE. mylib
: mylib.greet "Hello from mylib" . ;
mylib.greet                          ( qualified call )

IMPORT. mylib
greet                                ( unqualified after import )

ALIAS. hello mylib.greet
hello                                ( using alias )
```

### ✅ Variables (Immutable State)

- [x] **VARIABLE. word** - Create variables with compile-time computed values
- [x] **Optional = syntax** - `VARIABLE. x 10 ;` or `VARIABLE. x = 10 ;`
- [x] **Compile-time computation** - Values computed during variable definition
- [x] **-> operator** - Store values to variables
- [x] **Automatic fetch** - Variables auto-push their values when referenced
- [x] **Immutable storage** - All state stored as immutable (using `im` crate)
- [x] **mutable/immutable operators** - Convert between mutable/immutable collections

Examples:
```forth
VARIABLE. x 10 ;              ( create with value 10 )
x .                           ( prints: 10 )
20 -> x                       ( store new value )
x .                           ( prints: 20 )

VARIABLE. sum = 5 5 + ;       ( compile-time computation )
sum .                         ( prints: 10 )
```

### ✅ Type System (First-Class Types)

- [x] **Type as Value** - Types are first-class values on the stack
- [x] **Type constants** - `i64`, `string`, `quotation`, `array`, `map`
- [x] **Generic type check** - `value type ?` → bool (replaces specific predicates)
- [x] **Generic type cast** - `value type !` → converted-value
- [x] **type word** - Get type name as string

Examples:
```forth
10 i64 ? .                    ( prints: 1  - is i64? )
"hello" i64 ? .               ( prints: 0  - not i64 )
"42" i64 ! .                  ( prints: 42 - string to i64 )
123 string ! .                ( prints: 123 - i64 to string )
10 type .                     ( prints: core.i64 )
i64 type .                    ( prints: core.type )
```

### ✅ Testing Framework

- [x] **TEST. word** - Define and run tests
- [x] **Pass/Fail reporting** - Visual output with ✓/✗
- [x] **Stack isolation** - Tests don't affect each other
- [x] **Test files** - `tests/basic.fth`, `tests/variables.fth`, `tests/types.fth`

Example:
```forth
TEST. addition 2 3 + 5 eq? ;     ( ✓ PASS: addition )
TEST. bad-test 1 2 eq? ;         ( ✗ FAIL: bad-test )
```

### ✅ Comments

- [x] **`--` line comments** - Everything after `--` is ignored

---

## Key Design Decisions

### 1. Bootstrap Philosophy

**Everything is a word**, including `:` and `;`. They're not hardcoded parser special cases anymore - they're immediate native words that manipulate the interpreter state.

### 2. Symmetrical Delimiters

- Quotations: `( ... )` - Simple parentheses for code blocks
- Comments: TBD (will use `--` for line comments)

### 3. Predicate Suffix

Comparison operators use `?` suffix to clearly indicate they're predicates:
- `lt?` not `<` (saves `<` and `>` for future use)
- `eq?` not `=` (saves `=` for type signatures)

### 4. Input Buffer Architecture

Proper FORTH-style input buffer that:
- Consumes tokens one at a time
- Refills from source when exhausted
- Supports multiple input sources (stdin, files, strings)
- Enables words like `:` to read ahead for the word name

### 5. First-Class Types

Types are values that can be on the stack, stored in variables, and passed to functions. This enables:
- **Generic operators**: `?` and `!` work for all types
- **Extensibility**: Adding new types doesn't require new operators
- **Metaprogramming**: Types can be computed and manipulated

### 6. Immutable State

All global state is immutable by default (using `im` crate for persistent data structures):
- Values can be converted to mutable for fast operations
- Storage automatically converts back to immutable
- Enables structural sharing and efficient copying

---

## Code Quality

### Clean Separation of Concerns

- **Value types** - What can be on the stack
- **Execution tokens** - Compiled representation of words
- **Words** - Dictionary entries with metadata
- **Forth interpreter** - Core evaluation loop
- **Input buffer** - Token stream management
- **REPL** - User interface

### No Code Smells

- No hardcoded parsing (`:` and `;` are words!)
- No giant match statements in eval loop
- No complex nested token parsing
- Clean error handling with `Result<>`

---

## What We Learned From First Attempt

### Problems with Old Design (parse/eval style)

1. **Hardcoded special forms** - `:` and `;` were hardcoded in eval loop
2. **Brittle nested parsing** - Complex token consumption logic that broke with `[ { } ]`
3. **Not following FORTH philosophy** - Tried to parse everything upfront

### Bootstrap Architecture Wins

1. **`:` and `;` are words** - Immediate words that manipulate state
2. **Token-by-token** - Process one token at a time, refill when needed
3. **Immediate words** - Clean mechanism for compile-time execution
4. **Native words** - Escape hatch for Rust functions that need interpreter access

---

## Next Steps

### Immediate Priorities

- [ ] **Array/Collection operations** - Build out array manipulation words
- [ ] **String operations** - Comparison, concatenation, substring, etc.
- [ ] **Map operations** - Key/value manipulation for dictionaries
- [ ] **Loops** - Simple loop construct (using quotations?)
- [ ] **File I/O** - Read/write files
- [ ] **More type conversions** - Expand the `!` operator coverage

### Near Term

- [ ] **Abstract types** - `Integer` parent type for i64/i32/i16/etc
- [ ] **Type polymorphism** - Multiple implementations per type
- [ ] **Error dispatch** - `raise` mechanism with error types
- [ ] **Modules/Packages** - Load code from files
- [ ] **Standard library** - Core utility functions in namespaces

### Design Questions to Explore

1. **Arrays vs Tuples** - Should `[ 1 2 3 ]` be array or tuple?
2. **Type hierarchy** - How to implement abstract types with concrete dispatch?
3. **Effect system** - How to track I/O and other effects?

---

## Testing

Test files in `tests/`:
- **basic.fth** - 25 tests for arithmetic, stack ops, comparisons, definitions
- **variables.fth** - 5 tests for variable creation, storage, computation
- **types.fth** - 8 tests for type predicates and casting

Run tests:
```bash
./target/debug/march2 < tests/basic.fth
./target/debug/march2 < tests/variables.fth
./target/debug/march2 < tests/types.fth
```

**All 38 tests passing!** ✓

---

## Build & Run

```bash
# Build
cargo build

# Run REPL (with readline support)
cargo run

# Run tests
make test   (once we convert old tests)
```

---

## Summary

We now have a **feature-rich FORTH** with:
- ✅ Proper bootstrap architecture
- ✅ Quotations (code as data)
- ✅ String literals with escapes
- ✅ Namespaces with qualified names
- ✅ Variables with immutable state
- ✅ First-class type system
- ✅ Testing framework
- ✅ Clean modular code structure (~900 lines)
- ✅ Great REPL experience

**Core language features complete. Ready for collections and standard library!** 🚀
