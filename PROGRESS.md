# PROGRESS.md - March2 FORTH Development

## Current Status: Bootstrap Architecture Complete ✅

**Date:** 2025-10-11
**Branch:** bootstrap-forth
**Version:** v0.2 - Bootstrap Edition

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
- `dup` `drop` `swap`

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

- [ ] **Comments** - `--` line comments
- [ ] **More stack words** - `over`, `rot`, `nip`, `tuck`
- [ ] **Loops** - Simple loop construct (using quotations?)
- [ ] **String type** - Add strings to Value enum
- [ ] **Print without newline** - `emit` or similar

### Near Term

- [ ] **Arrays/Collections** - `{ 1 2 3 }` syntax
- [ ] **State variables** - `$` declarations with immutable data structures
- [ ] **Error dispatch** - `raise` mechanism with error types
- [ ] **Multi-methods** - Multiple implementations based on types

### Design Questions to Explore

1. **Compile-time quotations** - `(! ... !)` for metaprogramming?
2. **Loop syntax** - `value { true | false }` unified construct?
3. **Comments** - Use `--` for line comments, what about block comments?

---

## Testing

Currently using simple echo-based tests:
```bash
echo -e "test code here\nbye" | ./target/debug/march2
```

Tests verify:
- Basic arithmetic
- Word definitions
- Quotations and conditionals
- Return stack operations
- Comparison operators

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

We now have a **clean, working FORTH** with:
- Proper bootstrap architecture
- Quotations (code as data)
- Conditionals with quotations
- Clean modular code structure
- Great REPL experience

This is a solid foundation to build the advanced features (state, contexts, error dispatch, types) on top of.

**The bootstrap is complete. Time to build upward!** 🚀
