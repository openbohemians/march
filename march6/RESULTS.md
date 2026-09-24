# Experimental results

Status: promising control prototype; the broad March hypothesis is not yet
proved or disproved.

The commands used for the current result are:

```sh
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
cargo run --offline -- demo
```

All checks pass. There are currently 249 unit, integration, generated, and
differential test cases, with none ignored.  One passing test performs 10,440
generated staging comparisons.  `ADVERSARIAL-REVIEW.md` tracks weaknesses
found by external review and whether they are fixed, narrowed, or still open.

## Demand-contract and protocol-control checkpoint

C0 adds eleven executable reference fixtures for arbitrary-parameter guard
demand, independent static progress around unknowns, error sequencing, strict
aggregate projection, closed suspensions inside strict containers, and explicit
higher-order code arguments. A reflected Church `two two` returns closed code
that, applied to increment and then zero across an image boundary, yields 4;
its dependencies are explicit, not an implicit lexical environment.
Independent output-template witnesses preserve unused-input laziness and
share an instantiated recursive computation across distinct outputs, without
changing Pair/Record semantics. They are not yet general seed word interfaces.
These preserve existing semantics rather than
silently making all constructors lazy. An observed error around unknown inputs
can change after binding; the probes therefore cache errors only per epoch.

The first A-inspired scalar protocol control lives in `inet_demand_a.rs`, with
a shared probe vocabulary in `demand.rs`. It has iterative request/reply
continuations, shared memo values, retained partial state between epochs,
atomic binding-conflict rejection, and recoverable budget exhaustion. Four
unit tests and fourteen independent differential tests pass, including 720
arbitrary DAG/context and 240 typed staged comparisons. Two lowering-boundary
tests reject missing references and unsupported dormant nodes. A depth-30
repeated-doubling DAG evaluates its 31 distinct nodes once each, rather than
expanding the shared work into a tree.

This is NOT the port-level INet backend or an implementation of A's proposed
multi-principal request ports. Direct CID lookup models requests, and retained
memo storage is counted as live until the machine is dropped. No confluence,
parallelism, complete last-consumer reclamation, higher-order net encoding,
or canonical residual-image result follows from these tests. The independent
B probe is assigned to Claude but has not yet landed; there is no A/B winner.
See `DEMAND-CONTRACT.md` and `NEXT-PHASE.md` for the remaining gates.

## Staged content-addressed reduction

The reference graph treats a named hole uniformly: a compile context may fill
it now, or a runtime context may fill it later.  The same reducer handles both.
The central tested equality is:

```text
reduce(reduce(program, static), dynamic)
    == reduce(program, merge(static, dynamic))
```

The sample specializes a static choice into this residual graph:

```text
(+ ?input 1)
```

Supplying `input = 41` later produces the content-addressed value `42`.
Specialization identity includes source artifact, complete supplied context,
and reducer/rule-set identity.  The current v7 identity records the
history-independent guard-demand sharing, linear-capability summary, and
validated reflection and syntax-neutral text/dictionary rules.
Primitive operation tags are part of node identity, closing the March 5
add/sub CID collision.

Reduction uses explicit continuation/work lists rather than the native call
stack.  Reduction, substitution, binding instantiation, linearity validation,
and memoized groundness checks all charge one configurable work budget (512
visits by default), and the reducer reports its peak pending-frame count.
Exhaustion is an execution-resource outcome, not a language result and not part
of semantic reducer identity.  Resetting a budget between compile and runtime
can therefore change whether a resource limit is reached; the staging equality
below concerns values and semantic errors when both runs have sufficient
resources.  A production service still needs a total request budget carried
across phases.

An in-memory specialization cache now exercises that identity.  Repeating the
same source/context/reducer request is a hit with no reducer steps; changing
even irrelevant context is a miss with a distinct key, while changing only the
execution budget reuses a successful semantic result.  Failed reductions are
not cached.
The cache is not yet persisted in an image, and the semantic rule-set version
is still maintained explicitly rather than derived from rule artifacts.

The reducer's v6 identity adds syntax-neutral validated reflection.  A strict
`Intern` operation now reduces an ordinary record/list description to the
exact CID of a closed quotation or guarded family.  Decoding is iterative,
memoized by description CID, charged to the explicit work budget, and atomic
with respect to newly reflected nodes.  Unknown descriptions residualize for a
later epoch.  There is no textual-CID authority path and no operation for
constructing a trace capability.  This establishes the B0a construction
boundary, not the complete B0 bootstrap. B0b now adds token stepping, optional
decimal conversion, and dynamic dictionary keys. B0c adds a small executable
seed compiler using those operations without changing the nucleus.

B0b's graph-defined token-frequency reader completes 10,000 tokens, including
on a 256 KiB native test-thread stack. Save/reload at every boundary of an
eight-token input produces the same final state and image CIDs as a single
run. Unknown source text can also residualize and resume after image reload.
A dictionary-held quotation survives reload and computes `49` from parsed
`7`. Eight primitive tests additionally cover 500 generated integer
round-trips, UTF-8 cursor boundaries, all input-subset staging combinations,
reflection/substitution, guard purity, capability safety, and byte budgets.
See `READER.md` for exact contracts and limits, and `SYNTAX.md` for provisional
surface choices. The B0b witnesses alone do not compile the chosen surface
syntax. Flat dictionaries and
existing whole-text clones can still cause superlinear host work.

An independent B0b review found no source bug and added 15 tests: generated
tokenizer/reference comparisons, integer boundary/decorated-value checks,
canonical dynamic updates, strict operand types, pure name/number guards,
reflection schemas, budget sweeps, and dynamically derived capability forks.
It also identified a deferred text-access gap: exact strings and line comments
need slicing/search beyond the current whitespace step. Per the user's
decision, integer literals take precedence and cannot be definition names;
exact UTF-8 keys are an explicit provisional seed policy.

B0c now compiles `square : ( dup * ) ;` to the exact hand-built quotation CID
and evaluates `7 square` to `49`. A FORTH-style seed produces the same code
under the same reducer. Source-defined parsing aliases work in the same file.
Nineteen seed tests exercise constants, compiled composition, explicit input
wiring, static links across rebinding, context isolation, staged source, and
every-token-boundary reload. Four CLI tests cover both surfaces, failure
exit statuses, and collection/EOF boundaries. The seed tests also pass on a
256 KiB thread stack.
See `SEED.md` for runnable examples and measured fixed-dictionary costs.

Seven independent seed tests additionally compare 60 generated programs under
both surfaces with a test-side symbolic compiler and concrete stack evaluator,
resume generated programs across token boundaries, exercise 300 random token
sequences under both surfaces, test no implicit capture and scalar results,
sweep work budgets, and report retained nodes per token. B0d now aligns
top-level and quotation demand: both build pending expression graphs. Eleven
additional demand tests cover discarded/demanded overflow, unused arguments,
constant-binding isolation, dormant code, whole-stack EOF observation, and
quota/image boundaries, plus 216 generated inline/factored value-or-error
comparisons across both surfaces. A post-reload sharing witness observes a 64-addition
graph once versus sixteen times in 923 versus 1,508 charged steps. Sharing is
per reduction run; reducer memo tables are not persisted in images.
Six independent B0d tests also exercise repeated cut/name/replace within
definition bodies and across composed calls, inverse inlining, replay at
token boundaries, and identical-call demand sharing. No discrepancy was found
in the tested integer/single-result subset. Constant construction is explicitly
not the same transformation as extracting a callable quotation.
The seed also rejects numeral definition names and excessive inferred arity
without extending the nucleus.

This establishes the narrow B0c witnesses, not the complete bootstrap gate:
self-extension is currently alias-level; arbitrary source-defined parsing
handlers, source-chunk streaming, full source self-hosting, and general linear
scaling remain unproved. No INet or memory-planner capability changed here.

Independent seed review identified persistent retention of transient compiler
states (roughly 100 store nodes per source token in its repeated-call probes).
The explicit-root checkpoint baseline now compares a 64-call run against
16-token image/reload epochs: 19,400 uncollected nodes versus 1,936 peak nodes
within an epoch and 348 retained nodes after final reload, with the same final
image. Reduction steps increase from 56,326 to 62,074, excluding image codec
cost. This remains a checkpoint control, not a bound on peak byte usage.

An in-memory explicit-root collector now marks structural reachability and
sweeps obsolete nodes and validation metadata at safe points. Its 16-token
version of the same fixture peaks at 1,936 nodes and ends with 348, preserving
exact final image bytes without serialization/reload inside the loop. The CLI
uses 64-token batches with a shared total reduction budget. Thirteen new tests
cover collector invariants, host roots, deep graphs, staging and demand, image
equivalence, CLI EOF boundaries, and driver fuel. This does not bound a single
epoch's allocations or account for collection in reducer fuel; transient and
persistent storage still share one map. See `COLLECTION.md`.

Reflection tests round-trip 300 generated closed code values, mutate 400
descriptions without a panic, sweep budgets, and compare original versus
reloaded-image results.  Additional cases exercise every reflectable node
shape, malformed schemas and lists, scope and guard-purity violations, exact
DAG sharing, runtime code generation, and attempts to turn spelled CIDs or
effect values into authority.  These are construction and validation tests;
the generated bodies are not necessarily well-typed executable programs.

Rollback covers decoding and code-validation failures within an `Intern`
attempt.  It does not make an entire reduction transactional: successful
earlier construction or ordinary normalization may remain after a later
failure.  The existing cached closure/purity validator is iterative but does
not yet charge its own visits; the reflection decoder does.  The work budget
therefore is not a complete CPU or memory quota.

The equality is no longer represented by a single example.  A deterministic
generator currently checks 10,440 combinations of small programs, runtime
values, Boolean choices, and four static/dynamic binding splits.  Outcomes
include both values and errors.  Unknown `if` branches are lazy: compile-time
facts are captured structurally, but an unselected error is not evaluated.
Ground stuck terms are rejected, and integer overflow has the same checked
error semantics in debug and release builds.

Explicit immutable records model state transitions.  Explicit trace tokens
model effect ordering; compilation leaves an effect blocked until its runtime
token arrives and never performs hidden host I/O.  A conservative DAG-use
check rejects a token with multiple consumers.  This is not yet a typed linear
effect system, nor does it execute or benchmark real host effects.  Because
the E0 trace stand-in is purely content-addressed, two independently intended
tokens with identical traces currently have the same CID; real host
capabilities will need explicit lineage/identity or an edge-based type rule.
The conservative check propagates linearity through shared containers and
validates the program plus all supplied binding values as one invocation
graph, rejecting aliases across distinct context names.  It also validates the
residual before returning, so an unbound token fork is rejected in the epoch
that constructs it rather than waiting for a later binding.  One consequence
of content-only token identity is that an otherwise unused binding equal to a
literal token in the program is conservatively treated as an alias.

## Guarded definitions and reduction epochs

The semantic graph now has ordered `Family` clauses, `Dispatch` calls, explicit
parameters, and lexical `Recur` calls.  A guard is an ordinary pure March
expression over the family parameters.  Literal `true` is the otherwise case;
clause order participates in the family CID and is observable precedence.

An unknown guard leaves one compact dispatch.  Its bodies are neither reduced
nor instantiated, so changing an unselected body from 10 nodes to 10,000 nodes
does not change the charged reduction work needed to suspend the call.  Closed
code validation does scan a newly encountered family once.  An erroneous later
guard is not observed after an earlier match.  Selected recursive calls are
memoized by CID within a reduction, and a shared recursive subcall is evaluated
once.  The same residual can receive facts in either epoch order, and a
suspended dispatch survives canonical image serialization before continuing.

Three boundaries were made explicit after adversarial review:

- quotations and families are closed code values; ambient named holes are
  rejected rather than silently losing facts between epochs;
- reusable code cannot embed a literal trace/world capability; effects must
  cross the boundary as explicit parameters so they cannot be minted or
  duplicated by repeated application;
- guards are syntactically pure, and effect/world tokens may be neither fanned
  nor erased by the interaction-net backend.

The host reference evaluator now completes 10,000 nested guarded calls without
using the native call stack.  Guard evaluation shares only argument reductions
it actually demanded with the selected body; undemanded arguments remain lazy.
Only statically strict parameters of the guards actually evaluated are shared,
so, given the same bindings and reducer identity, unrelated earlier memo hits
cannot change a residual CID.  Selection and application validate new aliases
against cached code-template summaries rather than re-walking carried argument
graphs, and groundness facts are cached per run.  Step-ratio regressions are
linear for a lazy accumulator, a structural list walk, and a world-threading
effect loop.  The 10,000-call sum performs fewer than 300,000 charged visits
instead of repeatedly rebuilding and walking a growing symbolic argument.
Separate depth-20,000 regressions exercise ordinary evaluation, quotation
substitution, binding capture beneath an unresolved branch, and groundness
checking.  The explicit work budget remains the resource stop condition.

## Canonical images

The graph reachable from ordered image roots can be encoded canonically.
Loading an image recomputes and checks every node CID and verifies all
references.  Tests establish:

- round-trip bytes and image CID are stable;
- store insertion order does not change image bytes;
- payload corruption is detected;
- unreachable store history does not change an image;
- noncanonical record order, duplicate record fields, and noncanonical Boolean
  bytes are rejected;
- the reloaded graph retains the same formatted behavior graph.

## Compile-time memory plan

The planner keeps types separate from abstract identity/ownership.  Inputs are
explicitly `Borrowed`, `Unique`, or `Shared`; only local allocations and valid
unique inputs receive unconditional static frees.  Shared inputs now emit
explicit residual release operations and corresponding runtime check counts
rather than merely incrementing a fallback-input counter.

A deliberately slow concrete checker recomputes graph reachability after every
instruction.  A second concrete execution uses ownership transfers and dynamic
reference counts, without reachability walks, and records the exact instruction
at which each local or unique input reaches zero.  Exhaustive enumeration of
more than 500 small well-typed, straight-line immutable programs currently
reports agreement between the static plan and both checks:

- zero premature frees;
- zero missed reclaimable objects;
- zero delay from oracle death to planned release.
- zero plan/reference-count free-time mismatches.

The enumeration includes nested pairs, projections, stack shuffles, explicit
duplication, drops, and diamond-shaped sharing.  A deterministic stress test
adds 2,048 programs of 64 instructions with deeper randomly generated nested
sharing shapes.  Additional cases enumerate borrowed/unique/shared contracts
over aliased and distinct top-level inputs.  A violated unique-input alias
contract is rejected by the concrete verifier, and reuse of a consumed unique
input is rejected by the planner.  The plan artifact itself assumes its input
ownership contract; a real call boundary must validate or establish it.

The reachability checker is intentionally not presented as an independent
proof: it applies the same reachability criterion to concrete identities rather
than symbolic ones.  The new reference-count execution is algorithmically
independent of those graph walks, making a shared implementation bug less
likely, but it still implements the same stack IR and is not a semantic proof.
Representative workloads and external-input graphs with nested aliases remain
required.  Shared-input fallback releases are also outside both exact-free
oracles; the current fragment treats one shared parameter handle as owned until
its final known alias disappears.

Dynamic branches are tested path-sensitively.  Allocation sites in mutually
exclusive branches retain distinct static identities, and each selected path
agrees with the reachability oracle at its actual last-use points.  A naïve
path-specialization experiment also makes the scaling boundary concrete: eight
independent Boolean branches produce 256 plans.  The current experiment does
not consume a runtime condition or type-check joins; it establishes the cost of
the naïve representation, not a finished control-flow planner.  Compatible
prefixes, joins, and residual erasers must be shared.

For 100 sequential pair temporaries:

| Strategy | Physical slots retained | Runtime accounting |
|---|---:|---:|
| Static plan with slot reuse | 1 | 100 planned releases, 0 liveness tests |
| Per-temporary region/reset | 1 | 100 resets |
| Frame-wide arena | 100 | one bulk reset |
| Naïve reference-count baseline | peak 1 | 100 fused decrement/zero branches |
| Mark/sweep every 32 allocations | 33 | 4 collections; 3 roots, 3 marked objects, 6 edges, 103 heap entries swept |
| Ownership-aware RC/Perceus-style | expected peak 1 | not implemented; may eliminate these tests too |

The static plan makes 99 direct reuse decisions.  This is the desired middle
ground: arena-like cheap allocation without retaining every temporary to the
frame boundary, and prompt release without reference-count tests.  It does not
imply zero runtime work—destructors and planned release/reuse instructions still
execute.

That first workload is also the best case for regions: the operand stack is
empty after every temporary, so an executable reset-at-empty-stack region
baseline uses one slot with 100 resets.  A second workload keeps one pair live
while creating and dropping the same 100 temporaries:

| Strategy | Physical slots retained | Runtime accounting |
|---|---:|---:|
| Static plan with slot reuse | 2 | 101 planned releases, 0 liveness tests |
| Reset-at-empty-stack region | 101 | one reset |
| Frame-wide arena | 101 | one bulk reset |
| Naïve reference counting | peak 2 | 101 fused decrement/zero branches |
| Mark/sweep every 32 allocations | 34 | 4 collections; 6 roots, 6 marked objects, 12 edges, 107 heap entries swept |
| Ownership-aware RC/Perceus-style | expected peak 2 | not implemented; may compile unique releases/reuse statically |

This establishes a case where prompt static reuse materially differs from a
simple scope region.  It does not establish an advantage over inferred nested
regions or ownership-aware reuse; both can exploit much of the same analysis.
The tracing figures come from an executable mark/sweep simulation over the same
object trace.  Its 32-allocation cadence deliberately permits dead objects to
remain until the next collection; it is a count baseline, not a tuned garbage
collector or a wall-clock benchmark.

Static allocation sites are kept distinct from dynamic instances.  A closed
loop-body plan can be stored once and replayed for an input-dependent iteration
count.  In the current 10,000-invocation plan replay:

- one allocation site denotes 10,000 distinct `(invocation, site)` objects;
- all 10,000 are freed exactly;
- one physical slot serves every iteration through 9,999 planned reuses;
- a loop-wide frame arena retains 10,000 slots, while a per-iteration arena
  also needs only one;
- the reference-count baseline performs 10,000 zero tests.

This confirms the distinction between a static site and dynamic instances; it
is not a language loop benchmark.  The body has no carried value, input, or
result, and its counts are a replay of the compiled plan.  Open inputs and
escaping loop bodies are rejected until real invocation contracts and loop
invariants exist.

## Port-level interaction-net gate

The second reducer has fixed-arity agents with port zero designated principal.
It applies rules only to agents joined principal-to-principal.  Implemented
local rules currently cover:

- integer add/multiply pipelines;
- Boolean compile-time selection;
- ordered world/effect tokens;
- explicit `Fan` duplication;
- explicit `Erase` destruction;
- guarded `Call` selection from immutable family templates;
- labeled result observation.

A dynamic hole blocks the net during compilation.  Replacing that hole with a
runtime value lets the same reducer continue.  CAS integer-expression trees are
now lowered into this net rather than rebuilt only by hand.  A binary operator
locally consumes its left value and becomes a unary operator facing the right
subnet.  Repeated CAS nodes are built once and explicit binary fan trees
distribute their strict result.  The repeatedly shared depth-12 graph
`e[n+1] = e[n] + e[n]` lowers to 26 agents rather than an exponential tree.

For `(x + 1) * 2`, all inputs from -32 through 32 match the reference DAG
evaluator.  For `(x + 1) * (y + 2)`, all 289 input pairs from -8 through 8
match under both tested schedules.  Both left-known/right-late and
right-known/left-late staging orientations are covered; type errors, integer
overflow, and fuel exhaustion are explicit errors rather than silent stuck or
successful partial nets.

The arithmetic demo supplies `x = 40` at compile time and reduces three active pairs,
leaving a residual net blocked on `y`.  Supplying `y = 1` performs four more
rewrites, deletes eight agents, reuses three agent cells, allocates zero new
agent cells, and ends with `123`.
Independent lowest-wire and highest-wire schedules produce the same labeled
outputs for tested disjoint redexes.  Their equal final snapshot CID is weak
evidence because those test nets are empty after observation; non-empty
residual identity still contains physical agent and wire indices.

The guarded-call slice lowers first-parameter Boolean/equality decisions and a
small body language containing values, parameters, arithmetic, nested dispatch,
and recursion.  Family templates are immutable metadata, not live agents.
Only the selected body becomes an executing net, and a rejected body of 10,000
nodes has the same initial and peak live-agent count as a ten-node body.  A
recursive example produces the same value under lowest-wire and highest-wire
schedules and leaves no live agents.

This lowering intentionally rejects a family whose first parameter is not
semantically demanded before clause selection; otherwise the `Call` agent would
change the source language's laziness.  It also rejects computed auxiliary
arguments: their detached subnets could otherwise fail before a selected clause
erased them.  General multi-argument guard demand and delayed argument agents
are needed to lift those restrictions.  Fans still distribute strict values,
not arbitrary unevaluated computations.  This is evidence that local
active-pair selection and destruction/reuse fit the staged model, not evidence
that general higher-order March calls, data, or effects already have a
satisfactory net encoding.

## Corrections to the inherited design

The March 4 categorical note is not a proof of compositional liveness.  Its
graphs/graph-homomorphism category does not have the stated pushout composition,
its reachability map is oriented incorrectly, and caller demand can make local
and composed liveness disagree.  Any later categorical account needs open
graphs or cospans plus explicit ownership, multiplicity, and backwards demand.

Likewise, an allocation-site node is not a single runtime object: loops and
recursion create arbitrarily many instances.  Resource plans must be templates
over runtime values/wires.  The prototype therefore claims exactness only for
the measured fragment and treats unresolved sharing as residual work.

The CAS-to-memory lowering makes hash-consing policy explicit for its small
pair-value subset.  A repeated child may become one allocation plus `dup`, or
be rematerialized as two allocations.  This demonstrates that content identity
must not be silently equated with unique physical ownership; general DAG
sharing remains open.

## Current boundary and next falsification gates

The following remain unresolved and prevent an overall viability verdict:

1. Compact resource plans for dynamic joins and exceptional exits.  Semantic
   guarded dispatch is compact, but the memory planner still enumerates paths
   and demonstrates exponential growth.
2. General loop-carried values and recursive resource summaries.  Guarded
   semantic recursion and closed loop-body templates work; escaping bodies are
   currently rejected.
3. Closure capture, higher-order escape, and shared lazy thunks.
4. General CAS hash-consing and sharing in the memory lowering beyond the
   directly repeated-pair case now tested.
5. Pairs, higher-order application, general subnet choice, multi-argument guard
   demand, and fan/erase propagation through unevaluated operators.  A narrow
   guarded `Call` works and strict arithmetic DAG sharing uses explicit fans,
   but `Fan` still duplicates values rather than commuting through a general
   computation.
6. Canonical identity for cyclic or schedule-dependent residual nets.
7. Representative workloads and stronger baselines (ownership-aware RC and
   inferred nested regions) large enough to compare byte-time, peak storage,
   rewrite growth, and code-size/specialization growth.  Concrete
   reset-at-empty-stack region and periodic tracing baselines are now included.

B-trees remain plausible for image-pack extents, large free ranges, and
persistent free-space indexes.  Their logarithmic traversal and metadata cost
make them an unlikely hot-path allocator for individual reduction agents; the
measured slot-reuse path is the more relevant object-level mechanism.

The strongest hypothesis still supported by the evidence is:

> The experiments remain consistent with March deriving exact destruction and
> physical reuse for an immutable affine/linear fragment, while staged
> interaction-net reduction leaves explicit local fan, erase, and fallback
> operations where dynamic information prevents static resolution.  Stronger
> baselines and higher-order workloads are still needed to establish value.
