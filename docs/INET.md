# Interactive Nets Compilation Target

NOTE: This is a long term goal. 

IGNORE FOR NOW!

## Architecture Overview - Compilation Pipeline 

```
┌─────────────────┐
│  FORTH Source   │
└────────┬────────┘
         │ Parse (immediate words)
         ↓
┌─────────────────┐
│   Word AST      │
└────────┬────────┘
         │ Expand with contexts
         ↓
┌─────────────────┐
│   Primitives    │ ← Only primitives + control flow
└────────┬────────┘
         │ Build dataflow graph
         ↓
┌─────────────────┐
│  Interaction    │
│     Net IR      │ ← Architecture-agnostic
└────────┬────────┘
         │ Optimize (reduce, inline, fold)
         ↓
┌─────────────────┐
│  Optimized Net  │
└────────┬────────┘
         │ Select primitives from DB
         ↓
┌─────────────────┐
│    Assembly     │ ← x86_64, ARM, WASM, etc.
└────────┬────────┘
         │ Assemble & link
         ↓
┌─────────────────┐
│  Native Binary  │
└─────────────────┘
```

### Expansion Algorithm

```
function expand_word(word_name, context):
    candidates = lookup_all_contexts(word_name)
    
    for candidate in candidates:
        if evaluate_guard(candidate.guard, context):
            return expand_definition(candidate.body)
    
    error("No matching context for", word_name)

function expand_definition(tokens):
    result = []
    for token in tokens:
        if is_primitive(token):
            result.append(token)
        else:
            # Recursively expand
            expanded = expand_word(token, current_context)
            result.extend(expanded)
    return result
```

**Output:** Flat sequence of primitives + control structures (if/then, loops).

---

## Phase 3: Interaction Net Construction

### Inet Fundamentals

An interaction net consists of:
- **Nodes** - Operations (primitives, literals, guards)
- **Wires** - Data flowing between nodes
- **Ports** - Connection points on nodes
- **Interaction Rules** - How nodes reduce when connected

### Mapping FORTH to Inets

**Stack becomes wires:**
```
FORTH stack: [5] [10] [15]
Inet wires:  wire-0 → wire-1 → wire-2
```

**Stack operations become nodes:**
```forth
5 10 +

Inet:
literal[5] → wire-0 ┐
                    ├→ add-node → wire-2
literal[10] → wire-1┘
```

**Words become net fragments:**
```forth
: square  dup * ;

Inet fragment:
input-wire → dup-node → wire-A ┐
                   └──→ wire-B ┘
                               ├→ mult-node → output-wire
```

### Node Types

```rust
enum InetNode {
    // Data
    Literal(Value),
    
    // Stack operations
    Dup,      // ( a -- a a )
    Drop,     // ( a -- )
    Swap,     // ( a b -- b a )
    Over,     // ( a b -- a b a )
    Rot,      // ( a b c -- b c a )
    
    // Arithmetic
    Add,      // ( a b -- c )
    Sub,
    Mul,
    Div,
    
    // Logic
    Eq,       // ( a b -- bool )
    Lt,
    Gt,
    And,
    Or,
    Not,
    
    // Control
    Conditional { 
        guard: Guard,
        true_branch: InetFragment,
        false_branch: InetFragment,
    },
    
    TailCall {
        target: WordId,
    },
    
    // Effects
    Print,
    Read,
    WriteState { var: StateVar },
    ReadState { var: StateVar },
}
```

### Wire Representation

```rust
struct Wire {
    id: WireId,
    source: Port,      // Where data comes from
    target: Port,      // Where data goes to
    data_type: Type,   // Static type info
}

struct Port {
    node: NodeId,
    index: usize,      // Which port on the node
}
```

## Phase 4: Inet Optimization

### Reduction Rules

Inets optimize via local graph rewriting:

**Constant folding:**
```
Before:
  literal[5] → add[+3] → result

After:
  literal[8] → result
```

**Dead code elimination:**
```
Before:
  literal[10] → unused-wire → (no consumer)

After:
  (removed entirely)
```

**Inline small fragments:**
```
Before:
  call[square] → ...

After (if square is small):
  dup → mult → ...
```

**Guard elimination:**
```
Before:
  conditional[laser-on? = true] ─true→ impl-A
                                └false→ impl-B

After (if laser-on? statically known true):
  direct-wire → impl-A
  (impl-B removed)
```

### Optimization Passes

1. **Inline expansion** - Substitute small word bodies
2. **Constant propagation** - Evaluate literals through operations
3. **Dead wire removal** - Remove unused wires and nodes
4. **Guard simplification** - Eliminate provable guards
5. **Common subexpression elimination** - Share identical subgraphs
6. **Parallelization** - Identify independent subgraphs


