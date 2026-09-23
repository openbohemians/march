# March VM — Design Document v0.3
Spec Level: Engineering + Mathematical (b2)
Audience: Implementers, VM engineers, formal spec readers

The goal of this document is to define the March VM in a unified, compact, implementable form. It merges the A-series semantics with the compile-time ReferenceGraph model and the SSA/slot-style operational rules into a single coherent system.

======================================================================

A.1  OVERVIEW

March is a stack-based, statically typed, region-aware language with:

• A value stack
• A heap of composite objects
• A compile-time reference graph (Gₜ)
• Quotations (thunks) as first-class values
• No runtime GC or reference counting
• Deterministic memory management via compile-time liveness analysis
• An optional JIT that can erase allocations and frees into native calls

A key invariant:
    The runtime executes no memory analysis. All lifetime decisions
    are made at compile-time and encoded explicitly in the IR.

======================================================================

A.2  THE VALUE STACK

A.2.1  Kinds of Values

Values on the March stack:

• Scalars: int, float, char, bool, addr
• References: ref τ  (a pointer to a heap node)
• Quotations: code blocks with static type and mini-graph
• Type literals: [τ] (compile-time only, must be consumed by immediate words)

A.2.2  Stack Typing

At program point p:

    S(p)  =  [v₀, v₁, …, v_k]
    Γ(p)  =  [τ₀, τ₁, …, τ_k]

If Γ(p)[i] = ref τ, it carries:

    Γ(p)[i].node_id = n

Two stack slots alias if they share the same node_id.

A.2.3  Operational Stack Rules

These operations update both the stack and aliasing relationships:

• dup:
      S: […, n] → […, n, n]
      Graph unchanged; alias count increases.

• drop:
      S: […, n] → […]
      Removes a root reference. Liveness analysis may free n’s subtree.

• swap, over, rot:
      Permute stack slots; graph unchanged.

======================================================================

A.3  HEAP GRAPH Gₜ

A.3.1  Definition

The heap graph at time t is:

    Gₜ = (N, E, T)

Where:
• N = set of node IDs
• T(n) = type of heap object represented by node n
• E ⊆ N × N = containment edges (n → m means “m is a child of n”)

A.3.2  What Creates Nodes

A node is created when compilation encounters:

• A composite literal (array, record)
• A type-directed allocation (e.g., ALLOT applied to [τ])

A.3.3  Parent → Child Relation

For an array literal [x, y, z], or record {a=x, b=y}:

    Outer node A is created
    Children B, C, D created for x, y, z
    Edges added: A → B, A → C, A → D

======================================================================

A.4  REGIONS

March uses regions only as a compile-time tagging mechanism.
Every ref τ belongs to exactly one region R.

Region assignment rule:

    If a node is created inside a word W, its region is W unless it escapes.

A region can be “promoted” if a node is returned from a word.

Regions are used to express high-level dominance of lifetimes and provide a basis for the memory safety theorem in A.10.

======================================================================

A.5  LIVENESS ANALYSIS

Liveness is computed at compile time over the IR of each word.

A.5.1  Roots at Program Point p

Roots(p) = { node_id in stack S(p) }

A.5.2  Reachability

Given Gₜ = (N, E):

    Reach(p) = all nodes reachable from Roots(p) by following edges E

A.5.3  Dead Node Set

For a transition p → p':

    Dead(p → p') = Reach(p) \ Reach(p')

These are freed.

A.5.4  Free Ordering

Children must be freed before parents:

    If n → m in E, then free(m) must appear before free(n).

======================================================================

A.6  EFFECTS (OPTIONAL FOR CORE VM)

Effects do not interact with memory or Gₜ.

They annotate words with categories (io, state, error).  
Effects are compile-time tags; effectful words cannot be reordered across effect boundaries. They do not affect liveness.

======================================================================

A.7  OUTER INTERPRETER AND IMMEDIATE WORDS

March retains a FORTH-style outer interpreter.

Immediate words execute during compilation and update:

• The compile-time stack Γ
• The reference graph Gₜ
• The IR being emitted

Examples:

• dup (immediate):
      updates Γ and the alias relations
      emits RUNTIME_DUP opcode

• [τ] (type literal):
      pushes a compile-time-only value  
      must be consumed by words like allot

• allot (immediate):
      consumes [τ]
      inserts ALLOC τ into IR (runtime)
      and updates Gₜ: new node created and pushed onto Γ

======================================================================

A.8  TYPE LITERALS AND ALLOCATION

A.8.1  Type Literals

A type literal [τ] appears only at compile time and must not reach runtime.

A.8.2  ALLOT

Syntax example:

      [int] 8 allot

Semantics:

1. [int] is a type literal for τ = int.
2. 8 is the count; total size is size(τ) * 8.
3. allot is immediate:
      • verifies [τ] is present
      • emits ALLOC τ with count
      • creates new node n with type array[τ]
      • Γ.push(ref τ with node_id = n)

======================================================================

A.9  IR, MINI-GRAPHS, AND CODEGEN

A.9.1  Word Compilation

For each word W:

• Construct IR (stack-based or SSA-like)
• Maintain a ReferenceGraph Gₜ specific to the word
• Store Gₜ as a mini-graph with the word's CID

A.9.2  Mini-Graph Contents

A word's mini-graph includes:

• Input stack types with node_id holes
• Output stack types with holes
• The heap nodes created inside W
• Containment edges among them
• Slot aliasing information
• Escape markers (if any nodes leave via return)

A.9.3  Stitching at Call Sites

When compiling a call to a word W₂ inside W₁:

• Load W₂'s mini-graph
• Match input stack shapes
• Fill holes with W₁'s current node_ids
• Compose graphs by merging node sets and edges
• Propagate reachability to determine freed nodes

A.9.4  Runtime Behavior

IR contains ALLOC and FREE instructions.
At runtime:

• ALLOC creates heap objects
• FREE deallocates them
• No analysis is performed
• No garbage collection is needed

======================================================================

A.10  MEMORY SAFETY THEOREM (SKETCH)

Theorem: March programs cannot dereference freed memory at runtime.

Proof Sketch:

1. Each node in Gₜ is created exactly once.
2. Stack typing enforces that any ref τ on stack refers to a node n that:
      a. exists in Gₜ
      b. has not yet been freed by any Dead(p → p') set
3. Liveness ensures that a node is freed only when it is absent from all future roots.
4. IR ordering ensures that:
      if n → m, then FREE(m) precedes FREE(n).
5. Therefore:
      any runtime dereference on S happens only for live nodes.
      No double-free or use-after-free is possible.

======================================================================

END OF DOCUMENT (v0.3)

