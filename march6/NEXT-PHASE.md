# Next phase: a stronger reference and a decisive INet experiment

Status: implementation authorized by Thomas; the first C0/protocol experiments
are in progress. Prepared after `23d6279` (218 passing tests). Candidate
encodings remain alternatives, not architecture decisions. Bootstrap is deferred.

See `DEMAND-CONTRACT.md` for executable reference fixtures and the precise
scope of isolated scalar protocol probes. The real port-level backend stays
unchanged until a candidate passes its tests and has explicit interaction rules.

Current implementation checkpoint: C0 has twelve new reference fixtures, including
explicit reflected Church `two two` across collection/image reload and two
independently demanded output-template witnesses. The shared scalar probe API,
A-inspired sequential memo control, B fan-routed probe, and both C control-port
reply topologies run sixteen shared tests. This is reference/semantic evidence,
not completion of N0 or N1. See `DEMAND-COMPARISON.md` for integration findings,
including retained storage and unbudgeted cleanup. R1 still needs a
representation decision before production word-interface changes.

N0a is now integrated as an experimental module with closed Quote/Apply and
dynamic parameter-proxy sites. It is not a gate pass: independent review found
that per-instance keys lose sharing introduced by canonical substitution and
miss the reference's active-CID cycle detection. Next N0 task is canonical
instantiated-work identity, before the reflected Church/N0b extension. Preserve
all current shared-evaluation requirements; see `N0-PROBE.md` for counterexamples.

## Objective

Strengthen the reference at word, data, and demand boundaries while testing
whether local interaction nets can implement the same semantics. Do not wait
for a complete source language before testing the difficult INet questions.
Keep the reference useful even if the INet hypothesis fails.

The existing baseline includes staged CAS reduction, closed code, guarded
families and recursion, images, the scalar source seed, demand-aligned
factoring, and explicit-root collection. The INet backend already handles
arithmetic sharing, restricted guarded calls/recursion, staging, and cell reuse.
It does not yet support general deferred argument computation or guard demand.

## Sequence and dependencies

| Milestone | Deliverable | Depends on | Lead |
| --- | --- | --- | --- |
| C0 | Shared demand/interface contract and adversarial workload catalog | Morning review | March |
| N0 | Early higher-order code duplication/partial-application design and witnesses | C0; existing closed-code reference | Claude + March |
| R1 | Reference representation for independently demanded outputs/data | C0 and representation decision | March |
| R2 | General word interfaces: zero/multiple results and explicit closed-code values | R1 | March + bounded implementation sub-agent |
| R3 | Combined guarded/contextual recursive-data workloads; minimal source exposure | R1/R2 as needed | March |
| N1 | Suspended scalar arguments, local demand/erasure, and shared forcing | C0; existing reference suffices | Claude, proposed |
| N2 | Ordered guards demanding arbitrary arguments | N1 | Claude, proposed |
| N3 | Structured-data and nested-sharing INet experiment | R1/R3 + N2 | Claude + March |
| V | Differential, staging, collection, and scaling evidence at every milestone | Respective interface agreed | Test sub-agents; March integrates |

After C0, R1 and N1 can proceed in parallel. N1 does NOT wait for source
syntax, lazy aggregate changes, a full type system, or self-bootstrap. N1/N2
should produce a useful go/narrow/stop decision before expanding to N3.
N0 starts alongside them, not after N3; a candidate must face nested code
duplication and explicit captured inputs before promotion. A scalar sharing
probe alone cannot pass that higher-order gate.

## C0 — specify the observations before changing representations

Write a small demand matrix and executable graph-level examples covering:

- Constructing, passing, duplicating, discarding, selecting, and observing a
  value, including values whose computation would overflow.
- Guard order and exactly which arguments a guard must inspect. A constant
  matching guard must not demand an unrelated argument; later guards and
  rejected bodies remain dormant.
- Successful EOF and constant-binding observations versus token pauses,
  collection, image serialization, and reload. Saving a pending graph is not
  the same operation as deeply normalizing its eventual value.
- Explicit state/context arguments, closed quotations, and capability values.
  No implicit lexical capture or ambient mutable dispatch state.
- Error comparison: define demanded error kinds and ordering where observable.
  Separate errors from unknown/residual, unsupported lowering, and work-budget
  exhaustion. A finite fuel failure is not proof of semantic divergence.

First examples: a guarded call ignores a computed overflowing argument in one
context and demands it in another; an unrelated first argument is unused while
the second selects a clause; a shared computation is demanded by two consumers;
one output is discarded while another is observed; a field is selected without
forcing another field. Mark which examples already pass the reference and
which require R1. Tests intended for future behavior must not be silently
counted as passing evidence before the implementation exists.

Acceptance: reviewed contract and fixtures with explicit expected outcomes,
plus an agreed list of supported graphs for the first INet gate. No new syntax
or blanket change to constructor strictness is needed to finish C0.

## Reference track

### R1 — independently demanded data and result bundles

Current `Pair` and `Record` reduction is strict. Packing multiple results in
an ordinary pair and projecting one would force the other, potentially breaking
factoring again. The seed's state records, reflection descriptions, equality,
groundness checks, and dictionary operations also use these constructors.

Compare two explicit designs before implementation: a distinct delayed
aggregate/result representation with explicit observation, or a deliberate
change to constructor semantics with audited demand boundaries. Prefer the
smallest coherent change, but do not choose it merely to minimize code edits.
Keep metadata normalization and `Intern` validation requirements explicit.

Acceptance: select one field/output without evaluating an unused failing one;
shared selected work is reused within an epoch; demanded failures still occur;
partial results preserve bindings through staging, collection, and images;
code closure, guard purity, and capability-linearity checks remain intact.
Review reducer identity, reflection schema, node encoding, and image compatibility
whenever semantics or representation changes. Do not silently reinterpret old
images under changed rules.

### R2 — general word interfaces

Remove the seed's one-integer-result restriction in bounded slices: first
zero/multiple outputs with explicit input/output stack effects, then structured
data and closed quotations passed as values with explicit application.
Define how recursive words obtain/check an interface; do not attempt a whole
type-inference system as a prerequisite. Preserve top-first input wiring and
static links across rebinding. New surface spellings stay minimal and provisional.

Acceptance: cut/name/replace and inverse inlining preserve values AND demanded
failures for zero-, one-, and multi-result fragments, including partially
discarded results, nested calls, and unused arguments. A closed quotation can
be passed and invoked without introducing an implicit environment. Existing
constant-versus-code binding behavior remains explicit.
Pure zero-output words have no result to demand; they must not become a hidden
effect escape hatch. Any effectful extension must preserve an explicit live
capability/result token rather than relying on invocation alone.

### R3 — combined context and recursive-data tests

Use existing guarded families and recursion rather than replacing them. Build
one finite recursive immutable-data workload with explicit context, guards
inspecting more than one input, shared substructure, and delayed work discarded
on one path. Start with graph-built programs. Expose only the source operations
needed to express agreed examples; full module syntax can wait.

Acceptance: direct versus staged evaluation, multiple fact-arrival orders,
inline versus factored execution, and image/collection boundaries agree.
Deep cases remain stack-safe. Resource limits and fallback paths stay visible.

## INet track

### N1 — delayed scalar arguments before richer data

Write a short encoding/rule sketch first, including suspended-work identity,
activation, multiple consumers, erasure, and fresh per-invocation wiring. Then
lift the rejection of computed auxiliary arguments for a precisely bounded
subset, initially retaining the existing first-argument guard restriction.

Acceptance:

- An ignored overflowing or indefinitely recursive argument is never activated;
  the observed finite result agrees with the reference within the test budget.
- Demanding the same overflowing argument produces the agreed error.
- Two consumers share a pending computation; erasing one does not destroy work
  needed by the other, and erasing all consumers reclaims its execution cells.
- Partial binding reduces available work and later binding continues the same
  net. No hidden reference-reducer calls implement missing net behavior.
- Existing lowest/highest-wire schedules agree on specified observations;
  add deterministic randomized schedule tests where meaningful. Tests across
  several schedules are evidence, not a confluence proof.

### N2 — general ordered guard demand

Replace the hardwired parameter-zero decision with an explicit ordered
decision mechanism that requests only needed arguments. Include constant-first
guards, decisions on different parameters, repeated guard/body use of a shared
argument, and recursive calls with fresh wiring. Rejected bodies must not
become live executing nets.
Initially support Boolean parameters and equality against atoms on arbitrary
parameter indices; composed/arithmetic guards are a separate extension, not
silently included in the claim. An unknown earlier guard blocks later clauses,
even if a later guard could already succeed or fail. Unsupported constructs
must still be rejected explicitly by lowering.

Acceptance: match the reference's clause choice, demand, result, and relevant
error behavior across staging and schedules. Increasing a rejected body from
10 to 10,000 nodes must not increase its live execution-cell allocation.
Report template storage and validation/materialization costs separately.

### N3 — richer structures and harder sharing

Only after N1/N2 pass, lower the chosen R1 data representation and R3 workload.
Exercise nested shared delayed work, projections, recursive data, and erasure.
Keep higher-order closed-code application as a separate subgate if it needs
new interaction rules; unrestricted closures are not part of this plan.

Compare behavior, not physical wire IDs. Non-empty residual serialization and
canonical residual identity need a separate encoding design; do not claim them
from equal final outputs or empty-net snapshots.

## Evidence and stop conditions

For each workload, record source DAG/template size, lowered net size, reduction
work, materialization work, duplicated computation, peak/final live cells,
fresh/reused/freed cells, and reference store/collection metrics. Sweep several
input sizes and sharing depths. Separate semantic correctness from speed.

Require size proportional to the supported source DAG/templates for initial
lowering, and no exponential expansion caused solely by duplicating shared
work in the chosen stress family. Recursive runtime work may legitimately grow
with the computation. Measure it instead of promising universal linearity.

Existing `Call` processing materializes selected immutable templates. Account
for that work honestly; do not label all rewrites constant-cost. Prohibit hidden
CAS evaluation or whole-active-net scans to implement demand/sharing. The
existing scheduler's own selection scans should be reported separately.

If N1/N2 cannot preserve demand with local rules and controlled expansion,
record the smallest counterexample and narrow/reconsider the encoding before
adding more features. Keep the reference as the viable baseline. The result
need not be either "all of March on INets" or "discard INets entirely".

## People, file ownership, and integration

- **March (lead):** C0, reference semantic decisions and core edits, R1/R2/R3
  integration, compatibility/version decisions, review, checkpoints. Thomas
  has final say on language choices. Own `src/reduce.rs`, `src/net.rs`,
  `src/seed.rs`, reflection/image changes, and core design docs by default.
- **Claude (ownership provisionally accepted):** N0/N1/N2 encoding note and
  implementation in `src/inet.rs` and `src/lower.rs`, with focused INet tests;
  independent review of the reference contract. Implementation is now approved;
  the first bounded assignment is an isolated B probe in `src/inet_demand_b.rs`,
  not edits to the established backend. Claude has acknowledged this assignment.
- **Differential-test sub-agent:** after C0, own a new dedicated test module
  for graph fixtures, generated comparisons, error classification, and schedule
  sweeps. Start with existing APIs; agree fixture interfaces before parallel edits.
- **Persistence/invariant sub-agent:** own separate tests for epoch splits,
  images, collection roots, closed code, capability safety, and low-stack cases
  as R1/R2 land. Review core changes without editing the same files as the lead.
- **Bounded implementation sub-agent:** when an interface is stable, may take
  a named slice such as seed stack-effect plumbing. This replaces a test slot
  temporarily; do not create overlapping owners of `seed.rs`.

Use at most the lead plus two internal sub-agents during parallel work;
Claude is coordinated through Arcana. Agents propose changes but do not
independently commit or change semantic scope. Main agent reviews and runs
debug/release tests, fmt/clippy, and targeted low-stack/scaling checks before
each checkpoint. Claude's instance is not awakened by Arcana mail alone;
queued proposals are not acknowledged assignments or completed work.
Two read-only planning sub-agents reviewed these tracks before this draft.
The first authorized implementation assignments are an isolated A probe in
`src/inet_demand_a.rs` and a common differential harness in
`tests/demand_protocols.rs`. March owns the shared API in `src/demand.rs` and
reference fixtures in `tests/demand_contract.rs`. First-round probe machines
are explicitly not full interaction-net encodings. No agent may silently
weaken shared evaluation into copying/recomputation to make a candidate pass.

## Morning review and explicit deferrals

1. Approve/reorder this plan and confirm Claude's INet ownership.
2. Review the C0 examples and R1 representation alternatives; settle observation
   and output-interface choices before implementing semantic changes.
3. Start C0 fixtures and N1 rule design, then implement the two bounded tracks.
4. Review evidence after each gate, before proceeding to larger features.

Defer self-bootstrap, broad syntax redesign, implicit closures, a complete
type/module system, real external I/O, parallel execution, full INet image
canonicalization, and a new production memory architecture. Keep capability
regressions even though real I/O is deferred. Teleological memory planning and
HAMT work remain valuable follow-ups, not prerequisites for N1/N2.

The existing uncommitted historical addition to `DECISION.md` is preserved.
Before committing it, qualify "immutable values cannot form cycles" as a fact
about the current acyclic CAS representation, and describe RC as a candidate
fallback rather than the only possible sharing mechanism. Older omnibus gates
in `DECISION.md` remain research goals; this document proposes smaller next
steps, not evidence that those larger gates have been passed.

Revisit the source-bootstrap fixed point only when source can express the
necessary state, code/data interfaces, and guarded recursion, and those
interfaces have passed the reference gates above.
