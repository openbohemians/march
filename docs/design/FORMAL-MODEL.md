APPENDIX A — FORMAL MODEL (MEDIUM LEVEL)
-----------------------------------------

This appendix formally defines the March semantics using an algebraic
model and small-step operational semantics. Not all rules are included;
only the core definitions needed to give unambiguous meaning to the IR
in the main spec.

### A.1 State Model

A machine state is:

S = (D, T, R, H, G, E, PC, IR, Γ)

Where:

• D — domain of runtime values
• T — type system
• R — region mapping (nodes → region)
• H — heap (mapping node_id → runtime object)
• G = (N, E, Theap) — heap graph
• E — effect context (not modeled operationally; annotation only)
• PC — program counter
• IR — instruction list
• Γ — type stack (the unique root set)

### A.2 Values

v ∈ D ::=
    n                    // scalar
  | ref(n)               // pointer to node n
  | quote(IR, σ_in, σ_out)

### A.3 Type Stack as Unique Root Set

For each p:

Roots(p) = { n | ∃i. Γ(p)[i] = ref τ (node_id = n) }

This is **the only** root set, except escaped nodes.

### A.4 Reachability

Reach(p) = μX. Roots(p) ∪ { m | (n → m) ∈ E ∧ n ∈ X }

### A.5 Small-Step Semantics

A transition:

(G, Γ, S, PC, IR) → (G′, Γ′, S′, PC′, IR)

selected rules:

**DUP**

(IR[PC] = DUP) and top(S) = v
---------------------------------------------  
(G, Γ, S, PC) → (G, Γ · τ, S · v, PC+1)

**DROP (non-ref)**

(IR[PC] = DROP) and top(S) = v (scalar)
-----------------------------------------------  
(G, Γ, S, PC) → (G, Γ′, S′, PC+1)

Γ′ removes last τ; S′ removes last v.
No effect on G.

**DROP (ref)**

(IR[PC] = DROP) and top(S) = ref(n)
Let Δ = Reach(p) \ Reach(p+1)
-----------------------------------------------  
(G, Γ, S, PC) → (G′, Γ′, S′, PC+1)

G′ = G minus nodes in Δ, respecting child-before-parent deletion.

**ALLOC τ**

(IR[PC] = ALLOC τ)
Create fresh node k; S′ = S · ref(k)
-----------------------------------------------  
(G, Γ, S, PC) → (G ∪ {k}, Γ · ref τ, S′, PC+1)

**GET_INDEX**

top(S) = i (scalar), second(S) = ref(n)
T(n) = array[τ] with child c = nth element
-----------------------------------------------  
Push ref(c) on stack; n consumed from stack; free logic via Δ.

### A.6 Typeflow Rules

Γ ⊢ DUP : Γ · τ

Γ ⊢ DROP : Γ′   removing top element

Γ ⊢ ALLOC τ : Γ · ref τ

Γ ⊢ GET_INDEX : ( …, ref array[τ], int ) → ( …, ref τ )

Full inductive system is consistent and supports inference.

### A.7 Liveness and Free Insertion

Dead(p → p′) = Reach(p) \ Reach(p′)

Nodes in Dead sets are freed at this boundary.

Ordering property:

(n → m) ∈ E ⇒ FREE(m) precedes FREE(n)

### A.8 Memory Safety Theorem (Sketch)

If FREE is inserted precisely for nodes in Dead sets, then:

1. subject reduction holds (typing preserved)
2. no ref in Γ points to a freed node
3. parent never freed before children
4. escaped nodes never freed within a word

Proof uses induction on instruction boundaries and the definition of Reach.

======================================================================

END OF SPECIFICATION v0.7

