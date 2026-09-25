# Active direction: a fast, CAS, immutable, lazy, context-oriented FORTH

Source paths and shell commands in these notes are relative to `march6/`.

Thomas's decision, 2026-09-25: abandon interaction nets as March's execution
architecture. This supersedes the INet gates, research schedules, and proposed
net-performance optimization work in the older documents. This is a product
direction, not a claim that the existing implementation already meets it.

## Keep the language; replace the execution hypothesis

- FORTH-style concatenative composition, words, explicit stack interfaces,
  dictionary, quotations, and a self-extensible development environment.
- Content-addressed definitions, immutable values, reproducible artifacts, and
  persistent images. Context participates in identity where meaning depends
  on it. Content identity and evaluation state are distinct.
- Immutable language-visible data and explicit context. Rust implementation
  ownership rules are not March's programming model.
- Lazy, shared evaluation: unused work does not run; multiple uses of the same
  pending computation reuse its result. Duplication must not silently become
  recomputation. Strict execution is permitted where it preserves demand,
  termination, errors, and explicit effect behavior.
- Context-oriented definition groups: pure argument/context guards select
  condition-free bodies. Word families already implement part of this idea;
  module-level contextual groups and their composition still need work.
- Self-extension and eventual self-hosting without an INet requirement. A
  small native nucleus may execute March-defined compiler/dictionary behavior.

## Retire from the active plan

No further Join, fan/eraser, active-port, net scheduling, or INet lowering work.
No tuple-space/matching runtime is adopted as a replacement by this decision.
No new requirement for perfect static lifetime prediction, distributed
execution, or implicit lexical closures. Runtime management must be correct
without an advanced memory planner; any later analysis is optional.

Keep prior implementation code and git histories. Superseded research notes
belong in Git history, not the active documentation tree; see the
[documentation index](README.md) for recovery instructions. Do not rewrite
history, create a new numbered project/repo, or migrate formats as an incidental
part of this pivot.

## Working execution direction

Separate canonical representation from fast execution representation. The
existing reducer hashes and interns intermediate nodes; retaining its semantic
features does not require making that the inner execution loop.

Start with a conventional compact word/bytecode engine as the working candidate,
with direct primitive operations, explicit call frames, and shared demand cells
for genuinely deferred work. Do not commit to a JIT or native backend before
measuring a useful small execution slice. Code loading can resolve CIDs to local
handles; canonicalization can occur when identity, reflection, specialization,
or persistence actually requires it. This is an implementation proposal, not
permission to weaken observable content identity or sharing guarantees.

Language values remain immutable. An internal memo cell may move from pending
to evaluating to completed without exposing a mutable variable to March code.
Recursive demand, errors, and context-sensitive memo reuse must be specified;
work-budget exhaustion must not become a cached language result.

The canonical-sharing policy is a design obligation, not an optimization detail
to discard silently. Audit existing same-CID sharing tests and decide explicitly
which guarantees carry over. Separate reuse of the same suspended instance from
memoizing independently constructed equal calls. Cache keys cannot omit context
or conflate pure work with effectful executions.

Contextual selection must have explicit precedence/overlap behavior and demand
only what selection needs. Preserve the selected family's identity under code
rebinding. The existing sealed, ordered families provide an initial control;
they are not yet the full contextual-module interface Thomas described.

Fast arithmetic must not require allocating a lazy object for every known
strict operation. Stack shuffles should move values/references; calls should
not rebuild canonical expression graphs simply to execute ordinary words.
Construction-time evaluation and runtime execution must agree semantically,
but need not literally use the same slow evaluator. No speed claim follows
until measurements support it.

## Existing assets and limits

- `src/cid.rs`, `src/net.rs`, `src/image.rs`, `src/collect.rs`: identities,
  immutable term representation, image persistence, explicit-root collection.
  The `net.rs` name here denotes the CAS term DAG, not interaction nets.
- `src/reduce.rs`: stack-safe reference reduction, demand/memoization, guarded
  dispatch, explicit context, specialization. Reuse tests/semantics selectively;
  do not promote its representation or performance to requirements.
- `src/seed.rs`, reader/reflection modules, [reference seed notes](reference/SEED.md): working source surfaces,
  closed quotations, dictionary/image behavior, parsing aliases, and factoring
  tests. Source quotations currently have one integer result; general FORTH
  stack effects, source contextual groups, and richer data remain incomplete.
- March 4: consult conventional execution, persistent-map, and memory work
  when it answers a concrete implementation question; no wholesale restoration.

Relevant March 4 memory notes are preserved on
`archive/march4/preserved/docs-2026-09-23`: `docs/design/MEMORY-MANAGEMENT.md`,
`docs/planning/PLAN-REFGRAPH.md`, `docs/design/FORMAL-MODEL.md`, and
`docs/design/HAMT.md`. They explore per-word reference-graph summaries, call-site
composition, and persistent sharing. These are inputs to future lifetime work,
not proof that the current lazy runtime can avoid ordinary reclamation.
Immutability alone is not a general proof that recursive lazy structures are
acyclic or have statically predictable lifetimes.

## First implementation milestone

1. Establish non-INet performance baselines: cold build/load versus repeated
   word execution, primitive chains, calls, shared lazy work, unused divergent
   work, context dispatch, and immutable structure updates. Compare runtime
   inputs with equivalent direct Rust; cap all potentially divergent tests.
2. Specify the small word interface and value/demand representation. Include
   zero/multiple outputs, explicit quotations without implicit captures, and
   invocation/context identity. Name semantic gaps before coding around them.
3. Implement one conventional execution slice, not a general framework. Measure
   allocation, cold preparation cost, steady-state execution, and retention.
   Check result/demand/sharing behavior against retained reference fixtures,
   without forcing incidental reference bookkeeping or step counts.
4. Expand only after the slice demonstrates acceptable performance for its
   workload. Report actual ratios and costs; do not substitute passing semantic
   tests for a performance gate. Agree quantitative targets from these baselines.

## Implemented spike checkpoint

The conventional engine is now implemented in `src/fast/`, with a source compiler,
`march-fast` CLI, canonical code images, lazy pairs, closed quotations, recursion,
and ordered contextual families. [FAST-SPIKE.md](FAST-SPIKE.md) is the runnable
guide and precise scope; [FAST-BENCHMARKS.md](FAST-BENCHMARKS.md) records timings
and invocation-workspace retention. This does not restart the retired INet plan.

Construction-time structural sharing and shared-instance memoization work.
Independent calls that become equal only after runtime argument substitution
are not automatically merged: stronger reference sharing remains an explicit
open contract, not a silently satisfied requirement. Missing-context
residualization and live pending-state images are also not implemented here.

Thomas has moved FORTH-style self-extension ahead of further general memory work:
it is foundational, not an optional feature to attach to a finished language.
The first token-view/action-plan prototype was the wrong boundary and has been
replaced. The current nucleus runs a March-defined input-stream loop: read a
whitespace-delimited word, recognize numbers FIRST, then dictionary lookup and
execution/compilation. Defining/comment words consume input themselves. The seed
and immediate flags persist in images. Word identity now hashes the composed
definition (dependency CIDs/literals), not inferred stack wiring. The interpreter
rebuilds itself across three saved-image generations with stable identities.
Native stream operations, definition builders, lowering, stack inference, and
initial seed assembly remain; see [BOOTSTRAP.md](BOOTSTRAP.md)
for the precise scope and compiler-state/runtime-stack distinction.

A conservative scalar family tail-loop path
now keeps the countdown probe in constant workspace. Explicit-root collection
between host observations also bounds the consumed-stream window, preserving
shared results and lazy failures. It is not automatic: general recursion inside
one force can still retain live continuations and frames until invocation reset.
Next memory work must address those continuations and automatic root management,
not merely add collection calls. Neither perfect lifetime planning nor a new
computational substrate is required. See FAST-SPIKE.md for the exact boundaries
and FAST-MEMORY.md for evidence and remaining costs.

## Goals and status — conventional engine

These statuses describe the prototype, not production readiness or the older
reference implementation. See FAST-SPIKE.md for verification checkpoints.

| Goal | Status | Remaining boundary |
|---|---|---|
| FORTH composition | Working core | Words, inferred stack interfaces, recursion, closed quotations; practical library remains. |
| Fast execution | Partial, measured | Scalar and restricted tail paths work; broader workloads and optimization remain. |
| Content-addressed code | Composed-definition identity | Ordered dependency CIDs/literals; execution graphs derived; no remote/general value store or pre-hash arithmetic normalization. |
| Immutable values | Working core | Scalars and lazy pairs; richer persistent collections remain. |
| Lazy evaluation | Working core | Selective branches/arguments/outputs/fields; no automatic parallelism. |
| Shared computation | Working with open contract | Instances share; independently equal runtime calls are not automatically merged. |
| Memory reclamation | Partial | Scalar tail loops and explicit boundary collection; automatic roots/live continuations remain. |
| Contextual programming | Partial | Guarded families and compiler context; module-level groups/composition remain. |
| Compiler self-extension | Stream proof working | March read/number/lookup loop and defining words persist; native graph builders and explicit compiler-state interface remain. |
| Full bootstrap | Interpreter fixed point tested; not full bootstrap | Three image generations stabilize; primitive semantics, lowering, initial assembly, and alias installation still native. |
| Staging | Explicit compiler execution only | General partial evaluation and unknown-context residualization remain. |
| Images | One definition-only format (F05) | Old prototype formats rejected; no live values/suspended computations/session snapshots. |
| Data/library support | Minimal | Strings, maps, arrays, and useful libraries remain. |
| Effects / language I/O | Not implemented | Need an explicit protocol compatible with lazy sharing. |
| Source syntax | Stream-fed FORTH | Whitespace words, numbers before dictionary, ordinary comment/defining words; final richer notation remains open. |
