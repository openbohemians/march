# March Core Architecture Issues and Redesign

## Current Problems

### 1. Special Syntax vs Immediate Words

Currently these are special token types handled in switch statements:
- `:` (TOK_COLON) - start word definition
- `;` (TOK_SEMICOLON) - end word definition
- `[` (TOK_LBRACKET) - start array/quotation
- `]` (TOK_RBRACKET) - end array/quotation
- `(` (TOK_LPAREN) - start comment/quotation
- `)` (TOK_RPAREN) - end comment/quotation

In FORTH, these are all **immediate words** that execute at compile time.

### 2. Compile-Time Tracking Problem

Some operations need both:
- **Runtime behavior** (e.g., allocate memory)
- **Compile-time tracking** (e.g., update type graph, reference counting)

Current approach:
- Primitives like `alloc` are registered as regular words
- Compiler code manually calls them and manually updates tracking
- This creates duplication and inconsistency

Example from `compile_rbracket()`:
```c
dict_entry_t* alloc_prim = dict_lookup(comp->dict, "alloc");
cell_buffer_append(comp->cells, encode_xt(alloc_prim->addr));  // Emit runtime code
encode_primitive(comp->blob, alloc_prim->prim_id);
// Manual type stack update:
push_type(comp, TYPE_ARRAY);  // Should be push_heap_value!
```

### 3. Dual Nature Words

Some words need BOTH behaviors:
- User writes: `32 alloc`
- Compile-time: Track heap allocation in type graph
- Runtime: Execute allocation primitive

This suggests three word types:
1. **Runtime-only**: Pure primitives (e.g., `+`, `-`, `dup`)
2. **Immediate-only**: Compile-time only (e.g., `\`, comments)
3. **Dual**: Both immediate handler AND runtime primitive

## Potential Solutions

### Option A: Shadow Words

Register pairs of words:
- `alloc` - runtime primitive
- `alloc` (immediate) - shadow that emits code + updates graph

Problems:
- Dictionary lookup ambiguity
- Need special "shadow" bit or dual-registration

### Option B: Unified Immediate Handlers

All words that need tracking become immediate:
- Handler emits runtime code
- Handler updates type graph
- No separate primitive registration

```c
static bool compile_alloc(compiler_t* comp) {
    // Get size from type stack
    if (comp->type_stack_depth < 1) return false;

    // Emit runtime primitive
    dict_entry_t* alloc_prim = ...;
    cell_buffer_append(comp->cells, encode_xt(alloc_prim->addr));
    encode_primitive(comp->blob, alloc_prim->prim_id);

    // Update type graph
    pop_type(comp);  // size
    push_heap_value(comp, TYPE_PTR);  // allocated pointer

    return true;
}
```

### Option C: Core Allocation Primitives

Provide low-level building blocks:
- `_alloc` - raw allocation (no tracking)
- `alloc` - immediate word that uses `_alloc` + tracking

User code uses `alloc`, which handles everything.

## Proposed Architecture

### 1. Three Word Types

```c
typedef enum {
    WORD_RUNTIME,    // Execute at runtime only
    WORD_IMMEDIATE,  // Execute at compile-time only
    WORD_DUAL,       // Both immediate handler AND runtime code
} word_kind_t;
```

### 2. Registration Pattern

```c
// Runtime-only primitive
REG_PRIM("+", PRIM_ADD, op_add, "i64 i64 -> i64", WORD_RUNTIME);

// Immediate-only
dict_add(dict, "\\", NULL, NULL, 0, &sig, false, true,
         compile_backslash, NULL, WORD_IMMEDIATE);

// Dual word
dict_add(dict, "alloc", &op_alloc, NULL, PRIM_ALLOC, &sig, false, true,
         compile_alloc, NULL, WORD_DUAL);
```

### 3. Immediate Words for Syntax

Convert special tokens to immediate words:

```forth
: :          \ Start word definition (immediate)
  ... ;

: ;          \ End word definition (immediate)
  ... ;

: [          \ Start array literal (immediate)
  ... ;

: ]          \ End array literal (immediate)
  ... ;
```

This simplifies the tokenizer - only needs:
- TOK_WORD
- TOK_NUMBER
- TOK_STRING
- TOK_EOF

## Implementation Strategy

### Phase 1: Core Primitives (Small)

Keep tokenizer as-is, but provide core allocation tracking:

1. Register `alloc` as immediate+runtime (WORD_DUAL)
2. Handler emits runtime code AND updates type graph
3. Use in array compilation instead of manual calls

### Phase 2: Syntax → Immediate (Large)

1. Simplify tokenizer to remove special tokens
2. Re-implement `:` `;` `[` `]` as immediate words
3. Bootstrap: hand-code these in C initially
4. Future: self-host (define in March)

## Questions

1. **Shadow words**: Can same name have both immediate and runtime versions?
   - Requires lookup context (compile vs interpret mode)
   - FORTH STATE variable approach?

2. **Type graph updates**: Should ALL allocations go through tracked words?
   - Yes - ensures no leaks in tracking
   - Primitive `_alloc` for low-level use

3. **Backward compatibility**: Can we migrate incrementally?
   - Phase 1: Add dual words alongside current code
   - Phase 2: Remove manual tracking code
   - Phase 3: Remove special tokens

## Next Steps

1. Define `word_kind_t` enum
2. Add kind field to `dict_entry_t`
3. Implement `compile_alloc()` immediate handler
4. Test with array creation
5. Measure complexity reduction
