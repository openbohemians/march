# March Semantic Specification (v0.7)
Spec Level: Engineering + Medium Formal Semantics
Backends: VM + Interaction Nets (backend-agnostic IR)
Primary Invariant: The type stack is the unique root set of the heap graph.

======================================================================

SECTION 1 — INTRODUCTION
-------------------------

This document defines the semantics of the March language at the level of
its intermediate representation (IR), type stack, heap graph, regions,
effects, and specialization. It is backend-agnostic: both the VM backend
(bytecode interpreter / JIT) and the Interaction-Net backend (graph
reduction) are defined as *lowerings* of this IR.

The core guarantees provided by the semantics are:

• Memory safety without runtime GC
• Deterministic allocation/deallocation
• No type checks at runtime
• Quotations as typed, first-class code blocks
• Polymorphism via specialization (AOT and optional JIT)
• Precise deallocation via compile-time liveness analysis

All March implementations must conform to these semantics.

======================================================================

SECTION 2 — VALUES AND TYPES
-----------------------------

### 2.1 Value Categories

Runtime values (V):

1. **Scalars**
   int, float, bool, char, etc.

2. **References**
   ref τ — pointer to a heap node with heap type τ.

3. **Quotations (thunks)**
   quote[σ_in → σ_out] — a first-class code block with its own IR and mini-graph.

Compile-time only values:

4. **Type literals**
   [τ] — used only by the outer interpreter (OI); never appear in runtime IR.

### 2.2 Type Stack Γ

For each program point p:

Γ(p) = [ τ₀, τ₁, …, τ_k ]

Each entry maintains, when τ = ref τ′, an associated **node identity** n:

Γ(p)[i] = ref τ′ (node_id = n)

The type stack:

**is the unique root set of the heap graph.**

No other implicit roots exist except nodes explicitly marked “escaped.”

======================================================================

SECTION 3 — HEAP GRAPH Gₜ
--------------------------

### 3.1 Definition

At compile time t:

Gₜ = (N, E, T)

• N — set of node identities
• E ⊆ N × N — containment edges (n → m means n owns m)
• T(n) — heap type (array[τ], record{…}, thunk, etc.)

### 3.2 Node Creation

Nodes are created by:

• composite literals
• array/record allocators
• ALLOT-style meta operations
• quotation allocation (if closures stored on heap)

### 3.3 Containment Rules

If a composite object has children, edges n → m are created immediately
by the OI.

### 3.4 Escape

A node escapes the current word W when:

• a ref τ containing it is returned
• it is stored globally
• it is passed to FFI with unknown lifetime

Escaped nodes are not freed by W.

======================================================================

SECTION 4 — REGIONS
---------------------

Regions are static lifetime classes determined entirely at compile time.

R(n) ∈ {
    R_local(W),      // local to word W
    R_return(W),     // returned by W
    R_global         // static/global/FFI
}

Nodes remaining in R_local(W) must be freed before W returns.

======================================================================

SECTION 5 — EFFECT SYSTEM
--------------------------

Effects describe observable runtime behavior beyond returned values.

Base effect kinds:

• pure
• heap
• io
• state
• error

Effect sets are propagated upward through calls:

Eff(W) ⊇ Eff(V) whenever W calls V.

Effects do *not* influence liveness; they constrain reordering and optimization.

======================================================================

SECTION 6 — OUTER INTERPRETER AND META NAMESPACE
---------------------------------------------------

The OI is the compile-time engine of March. It operates in two namespaces:

1. **Runtime namespace**
   Words callable at runtime (produce IR).

2. **Meta namespace**
   Compile-time only words.
   They manipulate:
   • the IR of the word being defined
   • the type stack
   • the heap graph
   • type literals
   • quotations
   Meta words do not appear in runtime code.

In REPL compile mode, both namespaces are available to the OI. In run mode, only the runtime namespace is visible.

======================================================================

SECTION 7 — IR (INTERMEDIATE REPRESENTATION)
----------------------------------------------

### 7.1 IR Instructions (semantic categories)

Core IR operations include:

• stack ops: DUP, DROP, SWAP, OVER, ROT
• scalar ops: ADD, MUL, SUB, etc.
• allocation ops: ALLOC τ, ALLOC_RECORD, ALLOC_ARRAY
• container ops: GET_INDEX, SET_INDEX, GET_FIELD, SET_FIELD
• quotation ops: MAKE_QUOTE, CALL_QUOTE
• control flow: BRANCH, RETURN
• memory ops: FREE n (inserted by compiler)

### 7.2 IR Typing

Typing judgment:

Γ ⊢ op : Γ′

The full inductive rules appear in Appendix A.

======================================================================

SECTION 8 — MINI-GRAPHS
-------------------------

Every word W (and quotation) stores:

MG_W =
    • nodes created by W
    • edges raised within W
    • aliasing patterns
    • escape flags
    • type holes (if polymorphic)

MG_W is stitched into the caller’s graph during specialization.

======================================================================

SECTION 9 — LIVENESS
----------------------

Given a program point p with type stack Γ(p):

Roots(p) = { n | ∃i. Γ(p)[i] = ref τ (node_id = n) }

Reach(p) is defined by transitive closure of containment edges:

Reach(p) = μX. Roots(p) ∪ { m | (n → m) ∈ E ∧ n ∈ X }

Dead(p → p′) = Reach(p) \ Reach(p′)

Nodes in Dead(p → p′) must be freed at boundary p → p′ in child-before-parent order.

======================================================================

SECTION 10 — MEMORY SAFETY GUARANTEE
--------------------------------------

The following invariant holds at every program point:

No ref τ appearing in Γ(p) refers to a node that has been freed.

Nodes are freed *only* when:

1. They are absent from Γ(p′)
2. All of their descendants are also absent
3. They are in R_local(W)

This ensures:

• no use-after-free
• no double-free
• deterministic lifetime of all heap values

======================================================================

SECTION 11 — SPECIALIZATION ENGINE (AOT + OPTIONAL JIT)
----------------------------------------------------------

Specialization is the process of eliminating polymorphism and inserting
FREE instructions.

Given:

• IR_W (possibly polymorphic)
• MG_W
• type scheme σ_in → σ_out
• concrete type vector [τ₁, …, τ_n]
• caller’s heap graph

Produce:

• monomorphic IR_W[τ₁, …, τ_n]
• merged heap graph
• FREE placement
• cached compiled code

Specialization always runs in AOT builds.
In developer mode, specialization may run lazily (JIT).

======================================================================

SECTION 12 — BACKEND INDEPENDENCE
-----------------------------------

This spec defines IR semantics only.

Backends must preserve:

• value stack semantics
• typeflow semantics
• liveness and FREE placement
• quotation behavior and instantiation
• aliasing identity
• escape/region rules
• effects ordering constraints

VM backend:
    IR → bytecode → interpreter/JIT

INet backend:
    IR → net nodes → reduction semantics

Both obey the same memory semantics and specialization behavior.

======================================================================


