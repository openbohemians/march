# March VM — Design Document v0.4
Spec Level: Engineering + Mathematical (b2)
Audience: Implementers, language engineers, formal semantics readers

This is the authoritative semantic specification of the March VM.  
It defines values, the heap graph, regions, liveness, quotations, polymorphism, and the specialization engine used by both the JIT and AOT compiler.

======================================================================

A.1  OVERVIEW

March is a statically typed, stack-based language with deterministic, compile-time memory management.  
The goals:

• Stack-based execution model (FORTH-like but typed)  
• First-class quotations  
• Composite objects allocated on a managed heap  
• A reference graph Gₜ describing heap containment  
• Compile-time liveness analysis and explicit FREE instructions  
• Polymorphism allowed, but resolved by specialization  
• A unified specialization engine used in both AOT and JIT modes

Semantic invariant:

    No runtime heap analysis is performed. All memory lifetime decisions
    (FREE insertions) happen before execution, either by OI or by specialization.

======================================================================

A.2  VALUE STACK

Values:

• Scalars: int, float, bool, char, addr  
• References: ref τ, pointing to a heap node  
• Quotations: typed code blocks with mini-graphs  
• Type literals [τ] (compile-time only, must be consumed)  

Stack typing:

    S(p) = [v₀ … v_k]
    Γ(p) = [τ₀ … τ_k]

For references, Γ(p)[i] = ref τ carries node_id = n.

Aliasing:

    Two stack slots alias if they share the same node_id.

Stack operations:

• dup: duplicates the node_id  
• drop: destroys a root and may free nodes  
• swap/over/rot: permute roots only  

======================================================================

A.3  HEAP GRAPH  Gₜ

Definition:

    Gₜ = (N, E, T)

N = heap node IDs  
E = containment edges (n → m means "n owns m")  
T(n) = type of object at node n

Node creation occurs during compilation upon:

• Composite literals  
• ALLOT of composite type  

Edges reflect structural containment, never aliasing.

======================================================================

A.4  REGIONS

Every heap node belongs to exactly one region R.  
Regions approximate lexical lifetime structure and form a dominance hierarchy.

Rules:

• Nodes created in a word W belong to region R_W.  
• If a node is returned from W, it becomes part of the caller’s region.  
• No runtime region tracking exists; regions are compile-time concepts.

Region constraints help prove memory safety (A.10).

======================================================================

A.5  LIVENESS ANALYSIS

Roots(p) = all node_ids referenced in stack at program point p.

Reachability:

    Reach(p) = nodes reachable from Roots(p) by following edges E.

Dead nodes:

    Dead(p → p') = Reach(p) \ Reach(p').

These nodes are freed, with:

• children freed before parents  
• every FREE inserted explicitly into IR  

This ensures no use-after-free.

======================================================================

A.6  EFFECTS

Effects annotate words with non-memory semantic behaviors (io, state, error).  
They do not influence Gₜ or liveness.  
Effects constrain reordering.

======================================================================

A.7  OUTER INTERPRETER (OI)

The OI compiles surface March code into polymorphic or monomorphic IR.

Responsibilities:

• Parse input  
• Maintain type stack Γ  
• Maintain Gₜ at compile time  
• Emit IR instructions  
• Resolve monomorphic types whenever possible  
• Produce polymorphic IR only when required  
• Never perform liveness — that's done by the specialization engine (A.11)

Immediate words:

• dup/drop/swap update Γ and Gₜ  
• [τ] pushes a type literal  
• allot consumes type literals and emits ALLOC τ  
• quotations ( … ) capture IR and mini-graphs

If concrete types are known, OI generates monomorphic IR directly.

======================================================================

A.8  TYPE LITERALS AND ALLOCATIONS

Type literals [τ] are compile-time-only tokens.

ALLOT consumes [τ] and optional size/count to create runtime allocations:

    ALLOC τ(count)

This creates a node n with type T(n)=array[τ] or other composite type.

The node is added to Gₜ and a reference is pushed to the stack.

======================================================================

A.9  IR, MINI-GRAPHS, AND CODEGEN

Each word W has:

• IR_W: code  
• MG_W: mini-graph describing node creation, edges, aliasing patterns  
• TS_W: input/output type schemes (possibly polymorphic)

MG_W includes:

• nodes created by W  
• edges within W  
• markers for escaped/returned nodes  
• type holes for polymorphic components  

On calls:

• MG_callee is loaded  
• type holes are filled using caller stack types  
• graphs are merged  
• resulting graph is passed to specialization (A.11)

The IR produced by the OI is incomplete if polymorphic types are present.

======================================================================

A.10  MEMORY SAFETY THEOREM (SKETCH)

Theorem: March programs cannot dereference a freed node at runtime.

Argument:

• Every reference in Γ(p) refers to some node in Gₜ.  
• FREE is inserted only when node ∉ Reach(p') for all future p'.  
• Region rules forbid nodes returning to contexts where they'd be dead.  
• Child → parent deallocation ordering prevents dangling children.

Therefore use-after-free is impossible.

======================================================================

A.11  SPECIALIZATION ENGINE (AOT + JIT)

The specialization engine is the final semantic phase of compilation.

It is invoked:

• during AOT compilation for deployment  
• during runtime for dev/REPL (JIT mode)  
• whenever a polymorphic word or quotation is called with concrete types  
• whenever a mini-graph contains type holes that must be concretized

Its job is to transform:

    (IR, MG, TS, concrete types)
into:
    monomorphic IR with all types known,
    fully stitched graphs,
    and explicit FREE instructions.

A.11.1  Inputs

• Polymorphic IR_W  
• Mini-graph MG_W  
• Concrete type vector from the stack  
• Caller graph G_caller  
• Effects and region summary  

A.11.2  Steps

1. **Instantiate type holes**  
   Substitute concrete τ into polymorphic IR and MG.

2. **Merge graphs**  
   Compose MG_W into caller’s G_caller.  
   Resolve node_ids, children, and edges.

3. **Compute liveness**  
   For each IR instruction boundary compute Reach(p).  
   Identify Dead(p → p').

4. **Insert FREE instructions**  
   In correct child-before-parent order.

5. **Emit monomorphic IR instance**  
   Cache by (CID, concrete type vector).

A.11.3  Caching

Once specialized, a word W instantiated at type vector [τ₁ … τ_n] is cached.  
Future calls reuse this instance.

A.11.4  Modes

(1) AOT Mode  
    Specialize entire program ahead of time. No JIT at runtime.

(2) JIT/Dev Mode  
    Specialize lazily at first call. Compile only the needed instantiations.

A.11.5  Guarantees

• No runtime GC  
• No runtime type resolution  
• No runtime escape analysis  
• No runtime liveness  
• Deterministic memory behavior  
• Identical semantics in AOT and JIT

======================================================================

END OF DOCUMENT (v0.4)


