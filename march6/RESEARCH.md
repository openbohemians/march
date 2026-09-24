# Research charter

## Central hypothesis

March can represent compilation and execution as successive applications of
one deterministic graph reducer:

```text
program CID + compile-context CID -> residual-program CID
residual-program CID + runtime-input CID -> result CID
```

The residual graph can also carry a compositional resource plan derived from
the same explicit use topology.  For graph fragments whose future wire fate is
known, the plan should reclaim every dead allocation without runtime tracing,
reference counts, or region-wide bulk freeing.  This teleological topology
analysis is an optional optimization; local reduction and reclamation must
remain correct when no useful prediction is available.

## Semantic invariants

- Rewrite rules are fixed.  A "state" changes reduction only by appearing as
  explicit immutable input, never as hidden mutable reducer configuration.
- Canonical bytes include the operation tag, all semantic attributes, and
  child CIDs.  Different primitives cannot accidentally share an identity.
- A specialization cache key includes both the source artifact CID and the
  entire supplied context CID.  A residual CID may coincide when irrelevant
  context facts differ, but cache identity must not.
- Compile reduction followed by runtime reduction must equal direct reduction
  with the union of the same bindings whenever those bindings are compatible.
- Runtime effects are represented by explicit state values or linear tokens.
  The pure reducer itself performs no hidden I/O or mutation.
- A planned free is legal only when no live root can reach the resource and its
  boundary contract permits this invocation to reclaim it.
- Unproved wire fate or boundary authority is visible in the metrics as
  fallback work.  It is never silently treated as a proven last use.
- Teleological topology annotations may change allocation, reclamation, and
  scheduling, but never the observable normal form.

## Questions the prototype must answer

1. Does staged reduction preserve meaning and deterministic identity?
2. Can quotations, state threading, conditionals, and specialization remain
   explicit without turning the reducer into a conventional hidden compiler?
3. For which program classes is the lifetime plan exact?
4. What compact residual information is needed for branches, recursion,
   higher-order values, lazy evaluation, and open-world calls?
5. Does a port-level interaction-net lowering improve locality, parallelism,
   or reclamation enough to justify its complexity?

## Experimental sequence

### E0: semantic control

- Canonical content-addressed term DAG.
- Holes, constants, arithmetic, choice, pairs, immutable records, quotations,
  and application.
- One reducer used first with compile bindings and then runtime bindings.
- Explicit immutable state update as record input/result.
- Symbolic straight-line reference graph with borrowed, unique, and shared
  input contracts.

Implemented and covered by the stack-safe work-list reference reducer,
reachable-image round-trip, conservative effect-token linearity, 10,440
generated staged/direct checks, deep structural regressions, and targeted
differential tests.

### E1: resource-plan boundary

- Add branch-local plans and joins.
- Add word summaries and recursive invocation templates.
- Add closure capture, delayed values, and explicit duplication.
- Compare static free instructions with reference counting and tracing on the
  same allocation traces.

Measurements: allocations, statically planned frees, fallback-managed values,
runtime increments/decrements, zero tests, tracing visits, peak live objects,
and bytes retained.

Partially implemented: straight-line immutable graphs, ownership contracts,
the concrete reachability cross-check, an algorithmically independent dynamic
reference-count free-time oracle, naïve reference-count/frame-arena,
reset-at-empty-stack region, and periodic mark/sweep comparisons, direct slot
reuse, exact enumerated branch paths, and closed
per-invocation loop templates are working.  The branch experiment proves that
naïve path specialization grows exponentially.  Guarded semantic dispatch now
provides a compact residual and guarded recursion, but it is not yet connected
to a compact resource-plan join.  Loop-carried resource summaries, closures,
and shared lazy thunks are still open.

### E2: interaction-net lowering

- Lower the proven E0/E1 calculus into explicit agents, ports, and wires.
- Require one principal connection per rewrite and deterministic observable
  normal forms.
- Represent sharing with explicit duplicator/eraser structure.
- Compare reduction count, allocation count, peak graph size, parallel redexes,
  and resource-plan coverage against the control reducer.

Initial gate and a real CAS-DAG lowering are implemented for binary integer
expression trees, static selection, ordered effect tokens, fan, erase, and
outputs.  Repeated strict arithmetic DAG nodes are computed once and distributed
through explicit fan trees with linear-size lowering.  A narrow guarded `Call`
agent now selects first-parameter-driven immutable templates, instantiates only
the chosen body, supports recursion, and preserves results under two schedules.
Lowering rejects calls whose first argument should remain undemanded and
computed auxiliary arguments that could reduce before selection.  General
guard decision chains, delayed arguments, fan/operator commutation, arbitrary
subnet choice, data constructors, and higher-order calls remain open; see
`RESULTS.md`.

## Failure and pivot criteria

The ambitious architecture should be reconsidered if any of these survives a
serious attempt:

- Specialization depends on facts absent from its cache identity.
- Compile-then-run and direct-run disagree for pure programs.
- Effects require hidden reducer state to be practical.
- Dynamic alias information makes static plans little better than ordinary
  reference counting on representative higher-order programs.
- Interaction-net lowering requires global rewrites or loses deterministic
  observable behavior.
- CAS traffic or canonicalization dominates useful reduction work.

Failure of the interaction-net backend would not invalidate the staged CAS
semantics.  Failure of near-exact reclamation would not invalidate useful
wire-fate summaries or ordinary local reclamation.  The experiments are
separated so a result is not rescued by quietly changing the claim.
