# Adversarial review disposition

An external read-only review was run after the first E0/E1/E2 slice.  It found
substantive bugs and overclaims.  This file keeps those findings visible so a
passing test suite cannot quietly redefine the research question.

## Corrected

- Unknown `if` conditions formerly reduced both branches.  Branches are now
  lazy; static bindings are captured without evaluating deferred errors.
- One staged/direct example has been replaced by 10,440 generated comparisons
  over values, errors, branches, and static/dynamic binding splits.
- Ground stuck terms now fail rather than masquerading as residual programs.
- Integer overflow is a checked error in both the DAG and INet reducers, so
  debug and release behavior cannot silently diverge.
- Reintroducing a consumed unique input is rejected by the planner.
- Shared inputs emit explicit residual release operations and runtime-check
  counts when their containing graph becomes unreachable.
- Closed loop templates reject open inputs instead of producing contradictory
  free/double-free metrics.
- Images now contain only reachable nodes; noncanonical records and Boolean
  encodings are rejected.
- Effect-token fan-out is rejected by a conservative linear-use validation.
- Linear-use validation propagates capabilities through shared containers and
  treats all supplied binding values as roots of one invocation graph, closing
  indirect-container and cross-binding token forks.
- Residuals receive the same validation before leaving an epoch, so a token
  fork hidden behind an unbound hole is rejected when it is constructed.
- The CAS DAG now has explicit lowerings into the memory IR and unary INet
  subset.  Repeated pair children expose a preserve-sharing versus
  rematerialize policy instead of assuming CID identity implies uniqueness.
- A separate concrete ownership-transfer/reference-count execution now records
  zero-count times and agrees with the static free schedule over exhaustive
  small programs and 2,048 deeper generated programs.  It does not call the
  reachability checker.
- Integer CAS expression trees with two dynamic operands now lower into local
  port-net rewrites.  A 289-input-pair differential test agrees with the DAG
  reducer under both schedules; partial binding leaves a reducible residual
  net rather than invoking a host evaluator.
- Reduction now has an explicit configurable work budget and charges reducer,
  substitution, binding-instantiation, validation, and memoized groundness
  traversals.  Exhaustion is classified as execution policy, not language
  semantics, and does not fragment successful cache entries.
- A concrete reset-at-empty-stack region baseline matches the planner on fully
  separated temporaries but retains 101 slots versus two when one long-lived
  anchor keeps the scope open across 100 short-lived allocations.
- A periodic mark/sweep simulation now measures collections, scanned roots,
  marked objects and edges, swept heap entries, frees, and peak resident locals
  over the same traces.
- Unsupported INet active pairs and fuel exhaustion now fail explicitly;
  differential tests cover type and overflow errors and both staging
  orientations.
- Strict arithmetic CAS sharing now lowers linearly: each shared computation is
  built once and its value is distributed by explicit fan trees.
- Guarded families are closed code values.  Ambient named holes are rejected,
  explicit parameters carry epoch context, and dormant clauses are not walked
  by binding or linearity traversals.
- Guards are syntactically pure, so selection cannot consume an effect token
  also needed by the body.  Selected quotation bodies are checked for illegal
  effect-token duplication.
- Reduction, substitution, binding capture, and groundness checking now use
  explicit work lists.  Deep guarded recursion completes without native-stack
  growth, and demanded guard arguments are shared with the selected body so
  the 10,000-call regression performs linear rather than quadratic work.
- Guard-demand sharing is restricted to strict positions in guards already
  evaluated, making residual CIDs independent of unrelated memo hits.
  Selection-time linearity uses cached template/argument summaries, and
  groundness uses a per-run cache; lazy accumulators, structural list walks,
  and world-threading loops have linear charged-step ratios.
- Interaction-net `Fan(World)` and `Erase(World)` are rejected rather than
  duplicating or discarding the linear effect boundary.
- The narrow guarded-call lowering rejects definitions where demanding
  parameter zero would change source laziness.
- Reusable quotations and families reject embedded literal trace/world values;
  capabilities must arrive through explicit parameters rather than being
  captured and potentially duplicated by repeated application.
- The guarded-call lowerer rejects computed auxiliary arguments whose detached
  nets could otherwise raise an error before the selected clause erased them.
- Syntax-neutral reflection now converts strict ordinary record/list
  descriptions into closed quotations and non-empty families with an
  iterative, budgeted, memoized work list.  The existing closure,
  parameter/recur, capability, and guard-purity checks remain the single
  validation authority; rejected construction rolls back newly reflected
  nodes.
- Reflection has no data-to-CID operation.  It can embed only a quotation or
  family already held through a graph edge, so knowing a hash does not grant
  access and effect capabilities cannot be minted.  Remote CID resolution is
  deferred to a separately supplied authority-bearing resolver.
- The `Intern` node has a canonical tag, reducer semantics advance to v6, and
  the image envelope/image-CID domain advance to v2 while existing node CIDs
  stay stable.

## Claims narrowed

- B0b adds syntax-neutral token/dictionary primitives and a graph-defined
  reader witness, not the surface compiler. Token-quota pause/reload uses the
  same complete source plus byte cursor; streamed chunk concatenation and
  inside-token suspension are not established. Byte/field work is budgeted,
  but flat dictionary copies and existing text cloning leave the broad B0
  linear-scaling gate open. These additions advance reducer identity to v7
  using additive node tags under the existing V2 image envelope.

- The concrete reachability checker mirrors the planner's criterion in a
  different representation.  It is useful for implementation consistency but
  is not an independent proof or semantic last-use oracle.
- The planner symbolically replays a fully known straight-line trace.  Agreement
  with reachability and reference-count executions validates this fragment but
  says little about compact plans when control flow or caller topology is
  unknown.
- The 2,048 deeper generated programs contain nested local sharing through
  `dup`, not nested external-input graphs.  Ownership/alias input cases remain
  small and top-level.
- The 10,000-iteration result is a plan replay with distinct dynamic instance
  identities, not execution of a language loop or validation of a loop
  invariant.
- The RC baseline is naïve.  Ownership-aware RC or Perceus-style reuse may
  match the static plan on the current unique fragment.
- A per-iteration region uses one slot just like the current static plan; only
  the deliberately frame-wide arena retains all 10,000 objects.
- The reset-at-empty-stack region is intentionally simple; inferred lexical
  subregions could match the anchored workload.  Its 101-versus-two result is a
  mechanism illustration, not evidence against serious region inference.
- INet cell-reuse counts follow from the tiny unary rules.  They demonstrate
  mechanism, not a performance advantage.
- The schedule test proves equal labeled results only for disjoint reductions;
  its final nets are empty, and non-empty canonical net identity is unresolved.
- Branch-path plans do not yet consume a runtime condition or validate joins.
  The 256-path result measures the naïve enumerator, not an unavoidable lower
  bound for structured resource plans.
- Guarded CAS dispatch is now compact and recursive, but that is a semantic
  control-flow result, not yet a compact memory-plan join or recursive resource
  summary.
- Guarded INet calls keep rejected templates out of the live agent count, but
  the immutable template table is still host metadata.  It is not yet a
  self-hosted net representation of definitions.
- Reflection proves only that March values can construct validated code.  It
  does not yet prove that a March-defined outer interpreter owns token meaning,
  dictionary policy, immediacy, or alternate surface spellings.

## Still open and high priority

1. Representative workloads plus external input graphs with nested alias
   shapes.  The new dynamic reference-count oracle covers nested local sharing,
   but not caller-owned object topology.
2. Strong baselines: ownership-aware RC and inferred nested regions, including
   byte-time and runtime instruction cost on representative workloads.  The
   simple scope-reset region and periodic tracing controls are now measured.
3. Compact typed resource-plan joins, exceptional exits, loop-carried values,
   and recursive resource summaries.  Compact guarded semantic dispatch and
   recursive execution are now present.
4. Closures, captured values, higher-order escape, and shared lazy thunks.
5. General memory-DAG sharing and the boundary between transient unique objects
   and persistent hash-consed CAS objects.
6. General subnet choice, multi-argument guard decision chains, fan/erase
   propagation through operators, higher-order calls, and non-empty
   schedule-independent residual identity.  Strict value fan-out and a narrow
   guarded first-argument call are working.
7. Canonical net identity up to the chosen equivalence, or an explicit
   normative construction/reduction schedule included in artifact identity.
8. Effect-token typing across exclusive branches and quotation application,
   plus real host-adapter effect tests and hashing/storage-cost measurements.
9. Persistent specialization-cache entries in images and reducer identity
   derived from versioned rule artifacts rather than a maintained semantic
   version string.
10. Higher-order nested-fan stress: duplicating a function and then duplicating
    work inside each copy needs fresh/dynamic fan identities.  The current
    value-level fan rules cannot express this test and must fail honestly rather
    than silently capture or duplicate the wrong subnet.
11. Replace the content-only E0 `Trace` stand-in with a typed capability model
    that can distinguish independently created resources with equal history.
12. Complete bootstrap gate B0 above reflection: syntax-neutral text stepping,
    token-derived dynamic dictionary keys, a hand-built outer interpreter and
    seed image, alternate `: ... ;` / `to ... end` spellings, self-extension,
    and deterministic split/reload image construction.
