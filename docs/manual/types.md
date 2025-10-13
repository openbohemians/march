# Type System in March2

## Overview

March2 features a **first-class type system** with compile-time type checking and type-based dispatch. Types are values that can be manipulated at runtime, and functions can have multiple implementations selected by their type signatures.

Key features:
- **First-class types** - Types are values on the stack
- **Compile-time checking** - Type errors caught during compilation
- **Type signatures** - Declare input/output types for words
- **Multiple dispatch** - Same word name with different signatures
- **Abstract types** - Generic types for families of values (planned)

## Type Constants

Types are first-class values accessible as constants:

```forth
i64           -- Type for 64-bit integers
string        -- Type for strings
quotation     -- Type for code blocks
array         -- Type for immutable arrays
map           -- Type for immutable maps
type          -- Type for types themselves!
```

## Type Operations

### Type Checking: `?`

Generic type predicate: `( value type -- bool )`

```forth
10 i64 ? .                -- prints: 1 (true)
"hello" i64 ? .           -- prints: 0 (false)
"hello" string ? .        -- prints: 1 (true)
```

### Type Casting: `!`

Generic type conversion: `( value type -- converted-value )`

```forth
"42" i64 ! .              -- prints: 42
123 string ! .            -- prints: 123
```

### Type Introspection: `type`

Get the type of a value: `( value -- type-string )`

```forth
10 type .                 -- prints: core.i64
"hello" type .            -- prints: core.string
i64 type .                -- prints: core.type
```

## Type Signatures

### SIGNATURE. Word

Declare type signatures for word definitions:

```forth
SIGNATURE. input1 input2 ... -> output1 output2 ... ;
```

The signature applies to **all subsequent word definitions** until changed.

**Examples:**

```forth
-- Binary operation: two i64 inputs, one i64 output
SIGNATURE. i64 i64 -> i64 ;
: add + ;
: mul * ;
: sub - ;

-- Unary operation: one i64 input, one i64 output
SIGNATURE. i64 -> i64 ;
: square dup * ;
: double dup + ;
: negate 0 swap - ;

-- No inputs, one output
SIGNATURE. -> string ;
: greeting "Hello, World!" ;

-- Multiple outputs (planned)
SIGNATURE. i64 -> i64 i64 ;
: dup-add dup dup + ;     -- returns value and value+value
```

## Compile-Time Type Checking

When a signature is active, March2 tracks a **type stack** during compilation and verifies:

1. **Input types** - Stack has required inputs before calling a word
2. **Operation types** - Each operation receives correct types
3. **Output types** - Final type stack matches declared outputs

### Type Stack Tracking

```forth
SIGNATURE. i64 i64 -> i64 ;
: add + ;

-- Compilation of 'add':
-- 1. Signature declares inputs: [i64, i64]
-- 2. Type stack initialized: [i64, i64]
-- 3. Compile '+' (signature: i64 i64 -> i64)
-- 4. Check: type stack has [i64, i64] ✓
-- 5. Update: pop 2, push 1 → [i64]
-- 6. Verify: final stack [i64] matches signature output ✓
```

### Type Errors

Errors are caught at compile time, not runtime:

```forth
SIGNATURE. i64 i64 -> i64 ;
: bad "hello" + ;

-- ERROR at compile time:
-- Type error in '+': expected core.i64 at position 1 but got core.string
```

```forth
SIGNATURE. i64 i64 -> i64 ;
: bad 5 + ;

-- ERROR at compile time:
-- Type error in '+': expected 2 inputs but type stack only has 1
```

## Type-Based Dispatch (Planned)

Multiple implementations of the same word, selected by signature:

```forth
-- Integer square
SIGNATURE. i64 -> i64 ;
: square dup * ;

-- Float square (different implementation)
SIGNATURE. f64 -> f64 ;
: square dup * ;

-- String square (repetition)
SIGNATURE. string -> string ;
: square dup concat ;
```

### Dispatch Storage

Each signature creates a separate CID in the database:

```
Words table:
(namespace, name, signature) → CID

Examples:
("math", "square", "i64 -> i64")     → CID_1
("math", "square", "f64 -> f64")     → CID_2
("math", "square", "string -> string") → CID_3
```

At call time, the type stack determines which implementation to use.

### Conflict Resolution

When multiple signatures could match, March2 uses:

**1. Specificity Score**

More specific types win over more general types:

```forth
SIGNATURE. Integer -> Integer ;      -- Abstract type (score: 1)
: abs ...;

SIGNATURE. i64 -> i64 ;              -- Concrete type (score: 2)
: abs ...;

5 abs    -- Calls i64 version (more specific)
```

**Specificity rules:**
- Concrete types (i64, string): Score = 2
- Abstract types (Integer, Number): Score = 1
- Multiple parameters: Sum of scores

**2. Definition Order (Tiebreaker)**

If specificity scores are equal, **last defined wins**:

```forth
SIGNATURE. i64 string -> string ;
: format ...;                        -- Version 1

SIGNATURE. i64 string -> string ;
: format ...;                        -- Version 2 (overrides)

42 "x" format    -- Calls version 2
```

This allows intentional overriding and makes order deterministic.

**3. Error on True Ambiguity**

If two signatures have equal specificity and cannot be ordered (e.g., defined in different imported namespaces), compilation fails:

```
Error: Ambiguous dispatch for 'process'
  Could be: lib1.process (i64 string -> string)
  Could be: lib2.process (string i64 -> string)

Please use qualified name (lib1.process or lib2.process)
```

## Signature Introspection

### sig Word

Display the signature of a word:

```forth
SIGNATURE. i64 i64 -> i64 ;
: add + ;

sig add    -- prints: (core.i64 core.i64 -> core.i64)
```

## Concrete Types

Currently implemented types:

| Type | Description | Example Values |
|------|-------------|----------------|
| `i64` | 64-bit signed integer | `42`, `-17`, `0` |
| `string` | UTF-8 string | `"hello"`, `"world"` |
| `quotation` | Code block | `( dup * )`, `( 5 . )` |
| `array` | Immutable vector | `[1 2 3]` (planned syntax) |
| `map` | Immutable hashmap | `{:a 1 :b 2}` (planned) |
| `type` | Type values | `i64`, `string`, `type` |

## Abstract Types (Planned)

Generic types that encompass multiple concrete types:

```forth
-- Integer includes i64, i32, i16, i8
SIGNATURE. Integer Integer -> Integer ;
: add + ;

-- Works with any integer type
5i64 3i64 add     -- i64 + i64 → i64
5i32 3i32 add     -- i32 + i32 → i32
```

**Type hierarchy (planned):**
```
Number
  ├─ Integer
  │   ├─ i64
  │   ├─ i32
  │   ├─ i16
  │   └─ i8
  └─ Float
      ├─ f64
      └─ f32

Collection
  ├─ Array
  └─ Map
```

### Monomorphization

Abstract types are resolved at compile time to concrete types:

```forth
SIGNATURE. Integer -> Integer ;
: square dup * ;

-- When called with i64:
5i64 square    -- Compiler generates: square_i64 with CID_i64

-- When called with i32:
5i32 square    -- Compiler generates: square_i32 with CID_i32
```

Each concrete instantiation gets its own CID for optimal code generation.

## Type Namespaces

Types live in namespaces like words:

```forth
core.i64          -- Built-in integer type
core.string       -- Built-in string type
mylib.Point       -- Custom type from mylib namespace
```

Type names follow the same qualified/unqualified rules as words.

## Parametric Types (Future)

Generic types with parameters:

```forth
Array[i64]        -- Array of integers
Map[string, i64]  -- Map from strings to integers
Result[i64, string]  -- Result with value or error
```

Type parameters enable type-safe generic data structures.

## Type Inference (Future)

Automatic type inference from usage:

```forth
: double dup + ;

-- Infer signature from usage:
5 double .        -- Infers: i64 -> i64
3.14 double .     -- Infers: f64 -> f64

-- Generates multiple CIDs automatically
```

## Type Errors and Messages

### Compile-Time Errors

**Wrong type:**
```forth
SIGNATURE. i64 i64 -> i64 ;
: bad "hello" + ;

Error: Type error in '+': expected core.i64 at position 1 but got core.string
```

**Not enough inputs:**
```forth
SIGNATURE. i64 i64 -> i64 ;
: bad 5 + ;

Error: Type error in '+': expected 2 inputs but type stack only has 1
```

**Wrong output:**
```forth
SIGNATURE. i64 -> string ;
: bad dup * ;

Error: Type error in 'bad': signature expects 1 outputs but type stack has 1
       Expected: core.string
       Got: core.i64
```

### Runtime Errors (Type Casts)

Type casting failures occur at runtime:

```forth
"not-a-number" i64 !

Error: Cannot cast "not-a-number" to i64
```

## Best Practices

### 1. Use Signatures for Public APIs

Always declare signatures for library functions:

```forth
NAMESPACE. mylib ;

SIGNATURE. i64 i64 -> i64 ;
: add + ;

SIGNATURE. i64 -> i64 ;
: square dup * ;
```

### 2. Group Related Signatures

Define words with the same signature together:

```forth
-- All binary i64 operations
SIGNATURE. i64 i64 -> i64 ;
: add + ;
: sub - ;
: mul * ;
: div / ;

-- All unary i64 operations
SIGNATURE. i64 -> i64 ;
: square dup * ;
: cube dup dup * * ;
: negate 0 swap - ;
```

### 3. Leverage Type Dispatch

Use the same name for conceptually similar operations:

```forth
-- Convert to string
SIGNATURE. i64 -> string ;
: str ... ;

SIGNATURE. f64 -> string ;
: str ... ;

SIGNATURE. array -> string ;
: str ... ;

-- Now any value can be converted:
42 str .
3.14 str .
[1 2 3] str .
```

### 4. Document Type Contracts

Use comments to clarify type signatures:

```forth
-- Calculate distance between two points
-- Point is represented as [x y] array
SIGNATURE. array array -> f64 ;
: distance ... ;
```

### 5. Start Concrete, Generalize Later

Begin with concrete types:

```forth
SIGNATURE. i64 i64 -> i64 ;
: max ... ;
```

Later, generalize to abstract types:

```forth
SIGNATURE. Integer Integer -> Integer ;
: max ... ;
```

## Implementation Details

### Type Stack

The type stack is a `Vec<Type>` that shadows the data stack during compilation:

- **Initialize**: Copy signature inputs
- **Literals**: Push literal's type
- **Words**: Check inputs, update with outputs
- **Verify**: Final stack matches signature outputs

### Type Checking Function

```rust
fn check_and_update_types(&mut self, sig: &Signature, word_name: &str) -> Result<(), String> {
    // 1. Check stack depth
    if self.type_stack.len() < sig.inputs.len() {
        return Err(...);
    }

    // 2. Check types match
    for (i, expected_type) in sig.inputs.iter().enumerate() {
        let actual_type = &self.type_stack[...];
        if actual_type != expected_type {
            return Err(...);
        }
    }

    // 3. Pop inputs, push outputs
    for _ in 0..sig.inputs.len() {
        self.type_stack.pop();
    }
    for output_type in &sig.outputs {
        self.type_stack.push(output_type.clone());
    }

    Ok(())
}
```

### Signature Storage

Signatures are stored in the `Word` struct:

```rust
pub struct Word {
    pub xt: XT,                        // Execution token
    pub cid: Option<CID>,              // Content identifier
    pub immediate: bool,               // Execute during compilation?
    pub signature: Option<Signature>, // Type signature
}
```

Future: Multiple signatures will be stored as multiple entries in the namespace dictionary, keyed by `(name, signature)`.

## Testing

Type signatures can be tested using the TEST. framework:

```forth
-- Test type checking
SIGNATURE. i64 i64 -> i64 ;
: add + ;

TEST. add-works 5 3 add 8 eq? ;
TEST. add-type 10 20 add i64 ? ;
```

## Current Limitations

**Single signature per word:** Currently, each word can only have one signature. Multiple dispatch is planned but not yet implemented.

**No type inference:** Signatures must be explicitly declared. Automatic inference is planned.

**No abstract types:** Only concrete types (i64, string, etc.) are implemented. Abstract types (Integer, Number) are planned.

**No parametric types:** Generic types like `Array[T]` are not yet supported.

## Related Documentation

- `docs/manual/namespaces.md` - Namespace system and qualified names
- `docs/manual/cids.md` - Content-addressed IR and compilation
- `tests/signatures.fth` - Signature definition tests
- `tests/typechecking.fth` - Compile-time type checking tests
