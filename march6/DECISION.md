# Architecture decision

Status: proceed with a bounded March 6 research line, but narrow the memory
claim and keep the interaction-net backend optional until the next gate.

## Go: immutable staged semantics and images

The strongest original idea survives the prototype.  Compilation and execution
can be the same deterministic reduction relation supplied with different
immutable contexts.  Programs, contexts, residuals, values, and images have
stable content identities, and explicit state/effect values avoid hidden
compiler/runtime mutation.

This should be the normative language layer.  A fresh implementation should be
organized around the CAS term/value graph and canonical image, not around the
March 4 database schema or the March 5 scaffolding.

The stack-safe reducer uses explicit continuation/work lists.  Its execution
work budget is deliberately outside language meaning.  Successful
specializations may be cached by source, context, and semantic reducer identity;
resource-limit failures are not cached or compared as program results.

## Conditional go: interaction nets

The port-level experiment is now real enough to justify another gate:

- rewrites are local principal/principal interactions;
- partial binding reduces and leaves a residual net;
- binary dynamic arithmetic agrees with the reference reducer under two
  schedules, including error paths;
- shared strict CAS subgraphs are computed once and distributed through
  explicit fan trees with linear-size lowering;
- a narrow guarded `Call` selects immutable family templates locally,
  instantiates no rejected clause, supports recursion, and agrees under two
  schedules;
- agent cells are destroyed and reused locally.

It has not yet justified becoming the only backend.  Current fans duplicate
values, not arbitrary unevaluated computations; the guarded call only handles
families whose first parameter drives selection and whose auxiliary arguments
are passive atoms; data, closures, higher-order application, and canonical
residual identity are absent.  Keep the CAS reducer as the semantic oracle and
treat the INet as an experimental lowering until those cases work without
global rewrites or exponential growth.

## Narrow, do not discard: teleological topology analysis

The broad “near-perfect memory management” claim is not established.  Exact
prompt frees and static slot reuse work for a useful affine/linear,
straight-line fragment with explicit boundary authority.  They agree with both
concrete reachability and reference-count executions, and they improve
retention versus simple arenas and periodic tracing on the measured traces.

But the planner currently knows the complete instruction trace.  Dynamic
branches are expanded into paths, loop execution is plan replay, external
inputs are opaque leaves, and higher-order escape is absent.  An
ownership-aware RC or Perceus-style compiler may obtain the same wins on this
fragment.  Therefore advance prediction of wire fate should be an optimization
that emits explicit fallback operations, not a foundational promise that
constrains the language.  The executing net's actual topology and local
fan/erase behavior remain essential; predicting that topology before reduction
does not.

Arenas remain useful as scoped fallbacks and allocation mechanisms.  They are
not the semantic memory model.  B-trees may make sense for persistent image
extents and free-space indexes, but not for individual reducer-agent allocation.

## Recommended architecture

1. A typed, immutable, content-addressed semantic graph is authoritative.
2. Compilation is partial reduction under an explicit immutable context and
   produces a canonical residual plus dependency identity.
3. State and effects travel as typed linear/affine values; host adapters sit at
   the boundary.
4. A use-topology analysis annotates proven wire fate, destruction, and reuse,
   and leaves local fan/erase, RC, region, or tracing operations where proof is
   unavailable.  Linear ownership is reserved for capabilities and host
   resources that genuinely require it.
5. Canonical images persist roots and reachable CAS objects.  Cache entries and
   reducer rule artifacts can later live in the same image format.
6. The first dependable execution backend may be a conventional graph/bytecode
   engine.  The interaction-net lowering competes against it rather than being
   assumed superior.

## Next falsification gate

Do not broaden syntax before this gate.  Build one typed workload family that
contains all of the following:

- an input-dependent branch represented by a compact join, not path
  enumeration;
- a loop-carried or recursive immutable structure;
- caller-supplied nested sharing with unique, borrowed, and shared boundaries;
- one captured closure or delayed value that can escape;
- duplicated higher-order work with nested fans whose identities must be fresh;
- the same executions under the static planner, ownership-aware RC, inferred
  regions, and tracing.

Measure plan/code growth, peak storage, static versus fallback releases,
retains/releases and uniqueness tests, tracing work, and actual byte-time.  In
parallel, lower general guard demand to a local INet decision chain and add
subnet choice plus fan/erase propagation.

Continue the ambitious architecture only if the compact plan stays
approximately proportional to program/summary size, retains a meaningful
static-release advantage over ownership-aware RC on unknown control flow, and
the INet stays linear in the source DAG while preserving observable behavior.
If memory planning merely reproduces ownership-aware RC, keep its annotations
but drop the near-perfect-management thesis.  If general INet lowering requires
global rewrites or explosive fan structure, retain staged CAS semantics and use
a conventional backend.

## Decision

March is worth continuing as a research language because the state-indexed
staged CAS model is coherent and now has an executable control implementation.
It is not yet justified as an interaction-net language or as a solution to
general memory management.  Those are two separable, falsifiable backend
hypotheses for the next phase—not assumptions the language front end must bet
on.
