# March2

A Forth-like stack-based programming language with modern features.

## Features

- **Stack-based execution** with type tracking
- **Multi-method dispatch** with type signatures
- **Adaptive collections**: `{ }` creates Arrays (homogeneous) or Tuples (heterogeneous)
- **State variables** with `->` and `=>` operators
- **Quotations** (code as data): `[ ... ]`
- **Control flow**: `if`, `#do`, `call`
- **Type system** with type variables for polymorphism
- **Comments**: `--` line comments, `( )` block comments

## Quick Start

### Build

```bash
make build
```

Or with cargo:

```bash
cargo build --release
```

### Run

```bash
./target/release/march2
```

### Test

Run all tests:

```bash
make test
```

Run tests with output:

```bash
make test-verbose
```

Run a specific test:

```bash
make test-one TEST=tests/test_if.forth
```

## Examples

### Basic arithmetic
```forth
5 3 + .    -- prints 8
```

### State variables
```forth
$ count = 0 ;               -- simple literal
$ x = 5 3 + ;               -- comptime expression
$ arr = { 1 2 3 } ;         -- array
$ derived = count 2 * ;     -- reference other vars
count 1 + -> count          -- increment
count .                     -- prints 1
```

### Conditionals
```forth
= i64 -> i64 ;
: abs dup 0 lt [ 0 swap - ] [ ] if ;
-5 abs .  -- prints 5
```

### Arrays and Tuples
```forth
{ 1 2 3 } .           -- homogeneous array: {1 2 3}
{ 1 "x" 3 } .         -- heterogeneous tuple: (1, "x", 3)

{ 10 20 30 } 1 @ .    -- element access: 20
4 { 1 2 3 } 1 ! .     -- element update: {1 4 3}
```

### Multi-methods
```forth
= a -> a ;
: identity ;

= i64 i64 -> i64 ;
: add + ;
```

## Language Reference

### Data Types
- **Numbers**: `i64`
- **Strings**: `"hello world"`
- **Quotations**: `[ code ]`
- **Arrays**: `{ 1 2 3 }`
- **Tuples**: `{ 1 "x" 3 }`

### Stack Operations
- `dup` - Duplicate top
- `drop` - Remove top
- `swap` - Swap top two
- `over` - Copy second to top
- `rot` - Rotate top three

### Arithmetic
- `+ - * /` - Basic math

### Comparison
- `eq lt gt lte gte` - Comparisons (return -1 for true, 0 for false)

### State Variables
```forth
$ name = value ;     -- Declare (comptime evaluation)
$ x = 5 3 + ;        -- Expressions allowed
$ arr = { 1 2 3 } ;  -- Collections too
name                 -- Fetch value
42 -> name           -- Pop and store
42 => name           -- Copy and store
```

### Control Flow
```forth
cond [ true ] [ false ] if   -- Conditional
[ code ] N #do               -- Execute N times
[ code ] call                -- Execute once
```

### Collection Access
```forth
collection index @           -- Get element (array or tuple)
value collection index !     -- Set element (array only, returns new array)
```

### User-Defined Words
```forth
= inputs -> outputs ;   -- Type signature
: name ... ;            -- Definition
```

## Project Structure

- `src/main.rs` - Interpreter implementation
- `tests/` - Test suite (13 tests)
- `Makefile` - Build and test automation
- `PROGRESS.md` - Development log
- `EXAMPLE.md` - Design examples

## Status

Current implementation is a fully-featured interpreter suitable for language design validation. Future plans include compilation to interaction nets.
