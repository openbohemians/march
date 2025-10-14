# Plan: Static Type System with Function Overloading

## Current State

**What we have:**
- Type signatures on words: `SIGNATURE. i64 i64 -> i64 ;`
- Type checking at compile time
- Single word per name in each namespace
- `HashMap<String, Word>` - one Word per name

**Problem:**
Cannot define multiple words with same name but different signatures.

## Goal

Allow function overloading based on type signatures:
```forth
SIGNATURE. i64 -> i64 ;
: double dup + ;

SIGNATURE. string -> string ;
: double dup string-concat ;

-- At call site, type checker selects correct variant
5 double      -- calls i64 version
"hi" double   -- calls string version
```

## Design Options

### Option 1: Multiple Words per Name
Change namespace to: `HashMap<String, Vec<Word>>`

**Pros:**
- Simple to implement
- All variants stored together
- Easy to enumerate variants

**Cons:**
- Lookup becomes more complex
- Need to match signature at call site
- What if no signature at call site? (dynamic dispatch?)

### Option 2: Mangled Names
Encode signature in name: `double__i64_i64` internally

**Pros:**
- Existing HashMap structure works
- Fast lookup
- No ambiguity

**Cons:**
- User-facing complexity if exposed
- Need signature at definition time (already have this)
- Name mangling/demangling logic

### Option 3: Signature-based Namespaces
Each signature gets its own namespace, words by name within

**Pros:**
- Clean separation
- Could support signature-based imports

**Cons:**
- Major architectural change
- Namespace system becomes more complex

## Recommended Approach: Option 1 (Multiple Words per Name)

### Data Structure Change
```rust
// Before:
pub namespaces: Vec<HashMap<String, Word>>

// After:
pub namespaces: Vec<HashMap<String, Vec<Word>>>
```

### Word Lookup Algorithm
1. Get all variants for name: `dict.get(name) -> Option<Vec<Word>>`
2. If only one variant, return it
3. If multiple variants:
   - If we have type context (compile time), match signature
   - If no type context, error: "Ambiguous word, multiple definitions"
4. If no match found, error: "No matching signature"

### Type Context Tracking
- At compile time, track expected types on type_stack
- When looking up a word, check if any variant's signature matches
- Signature matching:
  - Check input types match what's on type_stack
  - Update type_stack with output types

### Definition Flow
```forth
SIGNATURE. i64 -> i64 ;
: double dup + ;
```

1. `SIGNATURE.` sets `current_signature`
2. `:` starts compilation with that signature
3. `;` creates Word with signature
4. **New:** Instead of `dict.insert(name, word)`, do:
   - Get or create `Vec<Word>` for name
   - Check if signature already exists (error if duplicate)
   - Push word to vector

### Lookup Flow (Compile Time)
When compiling `5 double`:
1. Push i64 onto type_stack (from literal 5)
2. Look up "double" -> gets Vec<Word>
3. For each Word, check if signature matches:
   - Inputs match type_stack
   - If match found, use that Word
   - Update type_stack with outputs
4. If no match or multiple matches, error

### Lookup Flow (Runtime - for now)
At runtime (in interpreter), we don't have type information:
- Could compile to specific variant at compile time
- Or error if ambiguous

## Implementation Steps

1. **Change namespace data structure**
   - `HashMap<String, Vec<Word>>`
   - Update all insertion points

2. **Update word definition (`;`)**
   - Check for duplicate signatures
   - Append to Vec instead of replacing

3. **Update word lookup**
   - Return Vec<Word> instead of Word
   - Add signature matching logic

4. **Update type checker**
   - Match signature at lookup time
   - Select correct variant

5. **Handle edge cases**
   - No signature at definition (single variant only?)
   - Runtime lookup (compile-time resolved?)
   - Import/Alias with overloading

## Open Questions

1. **What if word has no signature?**
   - Allow only if it's the only variant for that name?
   - Require signatures for all overloaded words?

2. **Runtime dispatch?**
   - Compile to specific variant (resolved at compile time)?
   - Or keep some dynamic dispatch capability?

3. **Error messages?**
   - How to show "no matching signature found"?
   - Show available signatures?

4. **Interaction with primitives?**
   - Can we overload `+` for strings, arrays, etc.?
   - Or keep primitives single-definition?

## Testing Plan

1. Basic overloading test:
   ```forth
   SIGNATURE. i64 -> i64 ;
   : double dup + ;

   SIGNATURE. string -> string ;
   : double dup string-concat ;

   TEST. int-double 5 double 10 eq? ;
   TEST. string-double "hi" double "hihi" eq? ;
   ```

2. Error cases:
   - Duplicate signature (should error)
   - Ambiguous call (should error)
   - No matching signature (should error)

3. Complex types:
   - Multiple input types
   - Multiple output types
   - Nested quotations

## Future Enhancements

- Type inference (infer signature from implementation)
- Generic types (polymorphism)
- Type classes/traits
- Automatic coercion

## Notes

- Keep it simple first - exact signature matching only
- Add sophisticated matching later if needed
- Document clearly what works and what doesn't
- Test thoroughly before expanding
