# DESIGN.md - FORTH-to-Inet Language

## Vision

A FORTH-like language that compiles through interaction nets to optimal native code across multiple architectures. Explicit state management, context-based dispatch, and content-addressed primitives enable both clarity and performance.

## Core Principles

1. **State is explicit and visible** - No hidden global state
2. **Context determines behavior** - Functions exist in domains defined by state and type conditions
3. **Inets as compilation target** - Interaction nets enable optimization and parallelism
4. **Content-addressed primitives** - Share implementations across network via cryptographic hashes
5. **Tail recursion only** - Simplifies compilation, prevents stack overflow
6. **Architecture portability** - Same source compiles optimally for any target

---

## Architecture Overview

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

---

## Phase 1: FORTH Parsing

### Traditional FORTH Parser

Uses immediate words for compile-time execution:

```forth
: square  dup * ;
: distance  square swap square + sqrt ;

\ Parser creates word definitions:
square → [dup, *]
distance → [square, swap, square, +, sqrt]
```

**Output:** Word dictionary mapping names to token lists.

### No Innovations Here

FORTH parsing is well-understood. Use standard approach with immediate words, compilation semantics, and interpretation semantics.

---

## Phase 2: Context-Based Expansion

### Context System

Words can have multiple implementations based on runtime conditions:

```forth
\ Context 1: laser is on
context laser-on?
: fire  "Pew pew!" print ;

\ Context 2: laser is off
context not laser-on?
: fire  "Click click" print ;

\ Context 3: base case for recursion
context dup 1 =
: factorial  drop 1 ;

\ Context 4: recursive case
context
: factorial  dup 1 - factorial * ;
```

### Context Guards

Guards can check:
- **State conditions:** `laser-on?`, `ammo > 0`
- **Type conditions:** `type = Enemy`, `length > 0`
- **Value conditions:** `dup 1 =`, `> 0`

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

### Tail Recursion

**Only tail recursion allowed:**
```forth
: countdown ( n -- )
  dup .
  1 -
  dup 0 > if
    countdown  ; TAIL CALL - becomes jump
  else
    drop
  then ;
```

**Compiles to cyclic inet:**
```
┌─→ print-node → dec-node → guard[>0] ─true─┐
│                                            │
└────────────────────────────────────────────┘
                                     └false→ end
```

This is just a loop in the final assembly - no stack frames needed.

---

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

---

## Phase 5: Primitive Selection

### Primitive Database

SQLite database stores all primitive implementations:

```sql
CREATE TABLE primitives (
    name TEXT PRIMARY KEY,
    stack_effect TEXT NOT NULL,
    pure BOOLEAN NOT NULL,
    commutative BOOLEAN
);

CREATE TABLE implementations (
    id INTEGER PRIMARY KEY,
    primitive_name TEXT NOT NULL,
    architecture TEXT NOT NULL,
    variant TEXT NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    file_path TEXT NOT NULL,
    required_features TEXT,
    cycles INTEGER,
    code_size INTEGER,
    registers_used TEXT,
    metadata TEXT,
    FOREIGN KEY (primitive_name) REFERENCES primitives(name)
);
```

### File Structure

```
primitives/
  x86_64/
    scalar/
      add.asm     → hash: a3f5d8e2...
      mul.asm     → hash: b7c4e1f9...
    sse2/
      add.asm
    avx2/
      mul_packed.asm
  aarch64/
    scalar/
      add.asm     → hash: 5c8e3a7b...
    neon/
      add.asm
  wasm/
    add.wat       → hash: 2f6b9d4a...
```

### Selection Algorithm

```rust
fn select_primitive(
    primitive_name: &str,
    target: &TargetInfo,
) -> Implementation {
    
    let query = "
        SELECT content_hash, file_path, cycles
        FROM implementations
        WHERE primitive_name = ?
          AND architecture = ?
          AND all_features_in(required_features, ?)
        ORDER BY 
            CASE WHEN variant = 'avx2' THEN 1
                 WHEN variant = 'sse2' THEN 2
                 ELSE 3 END,
            cycles ASC
        LIMIT 1
    ";
    
    db.query_row(query, [
        primitive_name,
        target.architecture,
        target.cpu_features,
    ])
}
```

**Selection priorities:**
1. Must support target architecture
2. Must have required CPU features
3. Prefer SIMD variants (when beneficial)
4. Prefer lower cycle count
5. Prefer smaller code size (tie-breaker)

### Content Addressing

Each primitive has a cryptographic hash of its contents:

```bash
# When adding a primitive:
$ sha256sum primitives/x86_64/scalar/add.asm
a3f5d8e2b4c7f9a1d3e5b8c2f4a6d8e0... add.asm

# Store in database:
INSERT INTO implementations 
VALUES ('add', 'x86_64', 'scalar', 'a3f5d8e2...', ...)
```

**Benefits:**
- Verify integrity before use
- Share primitives peer-to-peer
- Immutable reference
- Automatic deduplication

---

## Phase 6: Code Generation

### Assembly Emission

Generate assembly by walking the optimized inet:

```rust
fn emit_assembly(inet: &Inet, primitives: &PrimitiveMap) -> String {
    let mut asm = String::new();
    
    // Preamble
    asm.push_str(".text\n.global _start\n_start:\n");
    asm.push_str("    mov rsp, stack_top\n");
    
    // Walk inet in topological order
    for node in inet.topological_order() {
        match node.kind {
            NodeKind::Literal(v) => {
                asm.push_str(&format!("    mov rax, {}\n", v));
                asm.push_str("    push rax\n");
            }
            
            NodeKind::Primitive(name) => {
                let impl = primitives[name];
                asm.push_str(&format!("    ; {}\n", name));
                asm.push_str(&impl.assembly);
            }
            
            NodeKind::Conditional { guard, .. } => {
                emit_conditional(guard, node, &mut asm);
            }
            
            NodeKind::TailCall(target) => {
                asm.push_str(&format!("    jmp {}\n", target));
            }
        }
    }
    
    // Exit
    asm.push_str("    mov rax, 60\n");
    asm.push_str("    syscall\n");
    
    asm
}
```

### Primitive Assembly Example

```nasm
; primitives/x86_64/scalar/add.asm
; Stack effect: ( a b -- c )
; Calling convention: TOS in rax, stack grows down

primitive_add:
    pop rbx          ; Get second operand
    add rax, rbx     ; Add to first operand (TOS)
    ret

.metadata:
    .stack_effect: -1
    .cycles: 1
    .pure: true
```

### Calling Convention

**Stack layout:**
- `rax` holds Top-of-Stack (TOS)
- `[rsp]` holds Next-on-Stack (NOS)
- Stack grows downward

**Example: `5 10 + 3 *`**
```nasm
    mov rax, 5
    push rax
    mov rax, 10
    ; Stack: [5], rax = 10
    
    call primitive_add
    ; Stack: [], rax = 15
    
    push rax
    mov rax, 3
    ; Stack: [15], rax = 3
    
    call primitive_mul
    ; Stack: [], rax = 45
```

---

## Key Features

### 1. Context-Based Dispatch

Multiple implementations selected by guards:

```forth
context type = Enemy and laser-on?
: attack  fire-laser 100 damage ;

context type = Enemy and not laser-on?
: attack  fire-bullets 20 damage ;

context type = Ally
: attack  heal 50 ;
```

At compile time, generates conditional inet with three branches.

### 2. Explicit State Management

State variables declared up front:

```forth
state laser-on? : boolean = false ;
state ammo : int = 100 ;
state targets : list Enemy = [] ;

: fire-weapon
  laser-on? if
    fire-laser
  else
    ammo 1 - !ammo
    fire-bullet
  then ;
```

State queries become special inet nodes that read global state cells.

### 3. Dependent Types (Future)

Type constraints based on values:

```forth
: take ( vec n -- vec' )
  \ Requires: n <= length(vec)
  \ Returns: first n elements
  
context n <= length(vec)
: take  ... implementation ... ;

context n > length(vec)
: take  "Index out of bounds" error ;
```

Guards check type constraints at compile time when possible, runtime otherwise.

### 4. Pure Functions

Mark functions as pure for aggressive optimization:

```forth
pure : square  dup * ;
pure : distance  square swap square + sqrt ;
```

Pure functions:
- No side effects
- Deterministic output
- Can be memoized
- Can be reordered
- Can be parallelized

### 5. Content-Addressed Distribution

Share compiled functions across network:

```
Function compilation:
  FORTH source → Inet IR → Optimize → Hash

Share:
  Upload inet + metadata to content-addressed store
  Others download by hash
  Verify integrity before use

Import:
  Reference function by hash
  Local compiler optimizes for target architecture
```

---

## Implementation Phases

### Phase 0: Proof of Concept (2-4 weeks)
- [x] Basic FORTH parser
- [ ] Word expansion (no contexts yet)
- [ ] Simple inet IR
- [ ] Inet → textual representation
- [ ] One primitive (add) in assembly
- **Goal:** `5 10 + .` compiles and runs

### Phase 1: Core Language (2-3 months)
- [ ] Full FORTH semantics
- [ ] Context system
- [ ] All basic primitives
- [ ] Inet optimizer (inline, constant fold)
- [ ] x86_64 code generation
- [ ] Stack-trace debugger
- **Goal:** Factorial, fibonacci, fizzbuzz work

### Phase 2: Advanced Features (3-4 months)
- [ ] Dependent types
- [ ] Complex guards
- [ ] State management system
- [ ] Primitive database with selection
- [ ] Multiple architectures (ARM, WASM)
- [ ] Inet visualizer
- **Goal:** Real programs compile efficiently

### Phase 3: Performance (2-3 months)
- [ ] JIT compilation option
- [ ] Parallel execution
- [ ] Profile-guided optimization
- [ ] Benchmark suite
- [ ] Comparison with C/Rust
- **Goal:** Competitive performance

### Phase 4: Production (ongoing)
- [ ] Standard library
- [ ] Package manager
- [ ] Content-addressed distribution
- [ ] IDE/editor integration
- [ ] Documentation and tutorials
- **Goal:** Usable by others

---

## Open Questions

### 1. Recursion Beyond Tail Calls
- Can we support general recursion efficiently?
- Trampoline? CPS transformation?
- Or enforce tail-recursion-only discipline?

**Decision:** Tail-recursion only for now. Keeps compilation simple.

### 2. Mutation and Side Effects
- How do effects compose in inets?
- Effect tokens passed through wires?
- Monadic wiring?

**Decision:** State cells as special nodes. Effects are inet nodes with side effects.

### 3. Error Handling
- Exceptions vs. error wires?
- Result types with explicit handling?

**Decision:** TBD. Likely error wires that must be handled.

### 4. Concurrency
- Thread-per-wire parallelism?
- Explicit parallel blocks?
- Automatic from inet structure?

**Decision:** Automatic detection of independent subgraphs for phase 3.

### 5. FFI to C Libraries
- How to call external code?
- Special escape-hatch nodes?
- Type marshaling?

**Decision:** Special FFI nodes, phase 2+.

---

## Success Metrics

The language succeeds when:

1. ✅ **Clarity:** State and control flow are explicit and understandable
2. ✅ **Performance:** Within 2x of hand-written C for numeric code
3. ✅ **Portability:** Same source compiles to x86, ARM, WASM, GPU
4. ✅ **Optimization:** Automatic parallelization of independent operations
5. ✅ **Correctness:** Type system prevents entire classes of errors
6. ✅ **Simplicity:** Core language fits in <10,000 LOC
7. ✅ **Distribution:** Primitives and programs shareable via content addressing

---

## References

- **Interaction Nets:** Lafont, "Interaction Nets" (1990)
- **Optimal Reduction:** Lamping, "An Algorithm for Optimal Lambda Calculus Reduction" (1990)
- **FORTH:** Moore & Leach, "FORTH: The Language for Interactive Computing" (1970s)
- **Dependent Types:** Xi & Pfenning, "Dependent Types in Practical Programming" (1999)
- **Content Addressing:** IPFS, Git, and other content-addressed systems

---

**Version:** 0.1  
**Date:** 2025-10-06  
**Status:** Design phase - ready for prototyping

