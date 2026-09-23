# DESIGN.md - FORTH-to-Inet Language

## Vision

A FORTH language that compiles through interaction nets to optimal native code across multiple architectures. 
Explicit state management, context-based dispatch, and content-addressed primitives enable both clarity and performance.

## Core Principles

1. **State is explicit and visible** - No hidden global state
2. **Context determines behavior** - Functions exist in domains defined by state and type conditions
3. **Inets as compilation target** - Interaction nets enable optimization and parallelism
4. **Content-addressed primitives** - Share implementations across network via cryptographic hashes
5. **Tail recursion only** - Simplifies compilation, prevents stack overflow
6. **Architecture portability** - Same source compiles optimally for any target

## Key Features

### 1. Context-Based Dispatch

Multiple implementations selected by guards:

```forth
= Enemy ;
? laser-on?
: attack  fire-laser 100 damage ;

? not laser-on?
: attack  fire-bullets 20 damage ;

= Ally ;
? type = Ally ;
: attack  heal 50 ;
```

At compile time, generates conditional inet with three branches.

### 2. Explicit State Management

State variables declared up front:

```forth
$ laser-on? : boolean = false ;
$ ammo : int = 100 ;
$ targets : list Enemy = [] ;

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
-- Returns: first n elements
? swap vec.size lt? ; 
: take ( vec n -- vec' )

? swap vec.len > ;
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

Not sure we need this, compiler should be able to tell automatically.

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

## Phase 1: FORTH Interpretor

### Traditional FORTH

First requirement is a word dictionary.

We also need a mode switch. Start with two modes: runtime and comptime.

Then a small compile time evaluation loop.

Then a small runtimes evaluation loop. The loop should basically take the pre-compiled tokens to lookup in jump table for where to send execution,
a return stack tracks where we jumped from so it cen return to it (sub-routines). 

We will enhance this later to handle dyanamic dispatch as well for context constraints that can't be determined at runtime. 
FORTH doesn't have this so it's compiled "tokens" are essentially direct jump (via an indirect pointer) gernally called *xt*. 
Maybe we can do that too.

Pre-define and pre-load dictionary with required immediate words for compile-time execution.

* `:` and `;` to define new runtime words.
* `::` and `;;` to define new immediate words (words that run immediately at compile time).

Define core primative runtime words and preload the dictionary with them as well.

### Silly Example

```forth
: square  dup * ;
: distance  square swap square + sqrt ;

\ Parser creates word definitions:
square → [dup, *]
distance → [square, swap, square, +, sqrt]
```

This provides the word dictionary mapping names to token lists.

### No Innovations Here

FORTH is well-understood. Use standard approach with immediate words, compilation semantics, and interpretation semantics.

Remarks use `-- ` word (another immediate mode word) to end of line.

---

### Context System

Words can have multiple implementations based on runtime conditions:

`?` is used to define a context. We need another mode switch for compiling constraints.

Now we are in the context compiler evalution procedure, until we hit ';'. So `;` is a very special word that
decides what to do with all that has come before.

Context are compiled just like regular words, but get attached to words as part of their context rather than as regular word in the dictionary.
A one-to-many relation -- one word can have many possible contexts, and with order priority (at least for tie breaking if we can determine specificity score).

```forth
-- Context 1: laser is on
? laser-on? ;
: fire  "Pew pew!" print ;

-- Context 2: laser is off
? not laser-on? ;
: fire  "Click click" print ;

-- Context 3: base case for recursion
? dup 1 eq? ;
: factorial  drop 1 ;

-- Context 4: recursive case
? ;   -- empty context
: factorial  dup 1 - factorial * ;
```

Static types are context gaurds too, but they are compile time guards.

`=` defines a new type guard/context.

```
= string -> string ;
: do-something-to-string-and-output-a-string ;
```

STOP HERE. WE ARE ON PHASE 1.


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

**Version:** 0.4
**Date:** 2025-10-06 
**Status:** Design phase - ready for prototyping

