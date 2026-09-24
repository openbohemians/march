# INet demand design brief (N0/N1, input to C0)

Status: candidate design from @march-claude, reviewed and integrated by March.
The scalar A-inspired control, B, and both C reply topologies now run in the
shared harness. Earlier sections retain exploratory sketches, not implemented
rules or accepted gate passes. `DEMAND-CONTRACT.md` defines the tested semantics;
`DEMAND-COMPARISON.md` records independently verified results and limitations.
**Section 9 explores demand-token designs:
the most promising direction so far, with open variants A, B and C, not a
decision.**  Literature references are from memory and must be checked before
being cited in results.

## 1. Demand is already structural in the reference

The CAS reducer does not evaluate "everything" or "only what is observed".
It evaluates *strict positions* and leaves *lazy positions* dormant.  Reading
`reduce.rs` and B0d, the positions are:

| Node | Strict (evaluated when the node is) | Lazy (dormant until selected/applied) |
| --- | --- | --- |
| `Add`, `Mul`, `Eq`, `Emit` | both operands, left then right | — |
| `If` | condition | both branches |
| `Pair`, `Record`, `Get`, `Put`, `First`, `Second` | all operands (constructors are currently strict; R1 may change this) | — |
| `Apply` | the function | every argument |
| `Dispatch` | the family; guard-strict parameters of evaluated guards | other arguments, all clause bodies |
| `Quote`, `Family` | — (they are values) | their bodies |
| `Intern`, reader operations | all operands | — |
| seed observation (B0d) | `;` constant value, successful EOF stack | pauses, images, failed states |

**Proposal for C0: state this table in call-by-push-value (CBPV) terms.**
Values are integers, Booleans, worlds, *code values* (closed `Quote`/`Family`,
which are thunks of closed code), and *suspended arguments* (thunks of
pending expressions).  A computation happens only at a strict position.
*Demand* is the act of forcing a thunk at a strict position.  With that
vocabulary, "demand is another context" becomes literal: the difference
between a strict and a lazy position is the difference between computation
and value context, and observation is an explicit force at the root.
Constructor strictness (R1) becomes a precise question: is a field a value
position holding a thunk, or a strict position?

CBPV itself gives call-by-value and call-by-name, not call-by-need.  Sharing is
added separately (below).  I recall McDermott and Mycroft's "extended
call-by-push-value" (ESOP 2019) adding need; worth checking before relying on it.

## 2. Consequence for the net: suspend at lazy boundaries only

Under the current value-driven encoding and schedulers, any available active
pair may fire, so the encoding is effectively eager.  (This is a property of
this encoding, not of interaction nets in general; lazy strategies for nets
exist.)  Laziness therefore has to be built into the encoding, and we need it
only where the reference has lazy positions.  So:

- **Inside a strict region, keep today's value-driven rules.** Operators wait
  on their inputs; values flow; arithmetic sharing uses `Fan` over values.
  This already matches the reference.
- **At every lazy position, the lowered net contains a thunk agent instead of
  a materialized subnet.**
  - `Code(cid)` for a code value.
  - `Susp(template, captured wires)` for a pending argument: a closed CAS
    template for the argument expression plus auxiliary ports wired to the
    values or thunks it references.

  Nothing inside a thunk exists as agents, so nothing inside can fire.  This
  is the structural laziness N1 needs, without demand tokens everywhere.
- **Force is an ordinary interaction.** When a strict consumer's principal
  meets a `Susp` principal, the rule materializes the template.  Its captured
  wires become the subnet's inputs and its output connects to the consumer.
  This is the same template instantiation that `Call` already uses.  It is
  local but template-sized, not constant-size; account for it as
  materialization work, as NEXT-PHASE.md requires.
- **Erase meets `Susp` or `Code`:** erase the captured wires and nothing else.
  An unused overflowing or divergent argument is never materialized.
- **Clause selection is demand-as-context.** The selected clause's template
  knows which parameters it uses strictly.  Its guards are already analysed by
  `strict_guard_parameters`, and the same analysis applies to bodies.  At
  selection:
  - parameters whose *exact* force point is known are forced there and then
    fanned (shared once, as values).  That position must not come before any
    earlier guard, branch, or operand that could fail or diverge; see section
    4, candidate 1;
  - unused parameters are erased;
  - parameters used only lazily (under an `If` branch, or passed on to
    another call) are passed as thunks.

## 3. The higher-order duplication gate (N0)

With code as `Code(cid)` values, `Fan` meets `Code` simply copies the
reference.  `Apply` or `Call` meets `Code` materializes a fresh copy of the
template for that use only.  **No fan ever walks into code.**  So:

- `3 [dup *] dup [apply] dip apply` has the inner `dup` exist only inside each
  fresh materialization.  The outer and inner fans never meet, and the result
  is 81.
- Church `two two` passes `two` as a `Code` argument and fans it as a value.
  Each application materializes its own body, so no stale fan labels exist.

This is the "share results, instantiate code fresh" strategy.  March code
values being closed (no implicit capture) makes it *plausible*.  It is not a
proof: N0 must give concrete nets and rules, including how partially applied
results carry their explicit inputs (section 8, point 3).  If it holds, it
avoids Lamping-style optimal reduction and HVM's fan labels, at a cost:

1. **Materialization work** per application (template size).
2. **Recomputation** where a `Susp` is copied rather than shared (section 4).

N0's job is to *try to break* this claim, not just to confirm the two
witnesses.  Things to attempt:

- instrument and assert that `Fan` never meets a non-value agent;
- code values produced at run time by `Intern`;
- code passed through several calls;
- quotations applied to quotations at several depths;
- `Code` values captured in `Susp` templates.

If an honest counterexample needs fan-through-code, record it as the smallest
case and escalate.  The bounded risk is partial application.  With explicit
implicit lexical closures excluded by default, an explicit captured-code value
could be `Code` plus captured *values or
thunks*, which copy by reference.

## 4. Sharing: where call-by-need is and is not preserved

In *this* fan encoding, demand from one of several consumers cannot travel
back through a fan, because it would arrive on the fan's auxiliary ports.
That is an obstacle of this encoding, not a general impossibility: known
call-by-need encodings use token passing or sharing-graph machinery.  I recall
Sinot's token-passing nets (around 2005–2006) and Mackie's lazy evaluators; to
be checked.  Candidates, in increasing cost:

1. **Force-then-fan, only where the force point is exact.** This is safe only
   when the forced value's position in the reference's evaluation order is
   known, and nothing that could fail, diverge, or be skipped precedes it:
   earlier guards, an untaken branch, earlier operands.  "Strict somewhere in
   the body" is NOT sufficient (section 8, point 2).  Where the exact force
   point isn't known, this candidate does not apply.
2. **Lazily used values with several consumers are copied as thunks.** This
   is call-by-name for those: correct, but work may repeat.  Measure the
   recomputation explicitly (duplicated-computation counter) and report it
   honestly.  NEXT-PHASE already lists it.
3. **A memo agent (true call-by-need).** The first force at its real
   evaluation-order position computes the value, and later forces reuse it.
   This is the only candidate here that gives exact sharing AND exact ordering
   in general.  After Codex's point 2 it may be needed early.  It is not host
   mutation: a pending agent reduces under demand to its value, which is the
   usual lazy "update", and that rewrite is local.  (Thomas's framing.)  The
   three questions it raises:
   - *When is sharing needed?* Only where a pending value has more than one
     consumer, which in March is explicit: `dup`, or a parameter used twice.
     With a single consumer, force and consume; no memo agent.
   - *When is it done?* The fan tree is the consumer count.  Erasing a branch
     collapses its fan, and when the last branch is erased the memo agent is
     reclaimed locally.  No collector is needed.
   - *The actual departure from pure nets:* the first request comes from a
     consumer on a fan's auxiliary side, and in Lafont's formalism only
     principal ports interact.  So one rule must let a request reach the
     sharing node from any branch.  HVM's lazy duplication relaxes the same
     point; to check.  Once the value exists, distribution to the other
     branches is the ordinary fan-copies-value rule.
   Treat that single request rule as the deliberate, bounded extension with
   its own tests.

Section 9 explores how a demand token could provide candidate 3's exact
sharing; each variant there still needs some departure from strictly
principal-only interaction.  Candidate 2 is a relaxation of the N1 sharing
gate, not a pass.  It needs
adversarial measurement: repeated and nested lazy sharing where copying could
recompute exponentially, not only typical workloads.  This resembles
strictness analysis plus thunks, not optimal reduction.

## 5. Error ordering without weakening it (candidate)

Codex's constraint is to keep the reference's order.  The following is a
candidate, not proven schedule-independent.  It still has to be checked
against residual unknowns, indirectly called code, guards, and type failures
(section 8, point 4).  Proposal:

- **Errors become Poison values inside the net** (kind plus origin), rather
  than aborting the whole net.
- **Binary operators already consume their left operand first** (`AddPair` →
  `Add(left)`).  A right-side Poison can arrive early, but it waits on an
  auxiliary wire until the left operand is known.
  - Left is Poison: the result is the left error, and the right operand is
    erased.
  - Left is a value: the right Poison becomes the result.

  The first error in left-to-right order therefore wins *under every
  schedule*, which is exactly the reference's order.
- **Observation converts Poison at the root into the error.**
- **Divergence caveat.** If the left operand errors while the right diverges,
  the reference reports the error without ever evaluating the right.  An eager
  net may already be expanding the right subnet.  Erasure must then chase the
  expansion, and that race depends on the schedule.
  - Proposed rule: inside a strict region, operands containing a `Call` or
    `Apply` are demanded sequentially, i.e. the right is materialized only
    after the left is a non-Poison value.
  - Pure arithmetic, which always terminates, may stay parallel.
  - Specify this in C0, together with the fuel/divergence distinction
    NEXT-PHASE already requires.

## 6. N1 scope, restated

**Shared evaluation is a requirement, not an option** (Thomas, via Codex).
Every consumer of one shared pending pure computation reuses one evaluation.
Erasing a consumer neither forces the computation nor destroys another
consumer's work.  Call-by-name copying (section 4, candidate 2) is a
comparison baseline only; it never passes a gate.

Scope: scalar values only; existing parameter-0 guards; lazy auxiliary
arguments as `Susp`; shared lazy values and demand through one of the
section 9 variants (still to be chosen); Poison and Unknown results as in
section 9.2; two schedules plus randomized schedules.

Acceptance is as in NEXT-PHASE.md, plus these instrumentation assertions:

- no active pair inside an unforced thunk or `Memo`;
- zero `Fan`-meets-non-value interactions;
- the section 9 token-conservation invariant, checked after every rewrite;
- each `Memo` template materialized at most once per epoch;
- separate counters for materialization work.

N2 then replaces parameter-0 dispatch with ordered demand over arbitrary
parameters.  Guards force exactly their strict parameters, in guard order.  An
unknown earlier guard blocks later clauses, as in the reference.

## 7. Open questions

1. Adopt the CBPV vocabulary for C0's demand matrix?
2. Which section 9 variant should be explored first: A (multi-port Memo),
   B (fans pass the token upward), C (a separate control port), or a
   literature encoding?  Each departs from principal-only interaction
   somewhere, so determinism would rest on the token invariant rather than
   on standard confluence.
3. Accept the Poison/Unknown combination table (section 9.2) as the
   error-ordering and staging rule, subject to fixtures?
4. Should `Intern`-produced code, reader operations, and records inside
   thunks be N1 non-goals?  I propose yes; they belong to N3.

## 8. Review notes (Codex, before any implementation approval)

1. **Call-by-name copying of lazily shared thunks relaxes the N1 sharing
   gate; it is not a pass.** Measure adversarial repeated and nested sharing,
   including potential exponential recomputation.
2. **Force-before-fan must not hoist demand** ahead of guards, branches, or
   earlier failing or diverging body work.  Needs exact sequencing fixtures.
   Accepted.  This is why section 4 now limits candidate 1 to exact force
   points and promotes the memo cell.
3. **N0 must concretely encode the higher-order examples** in the
   closed-code, explicit-input calculus, including how partially applied
   results carry their inputs.  Closed code alone is not a proof of the whole
   encoding.  Accepted.  The N0 deliverable is concrete nets and rules for
   `[dup *] dup`, church `two two`, and a partially applied result (code plus
   explicit captured inputs), each checked against the reference.
4. **Poison ordering and the sequential-call rule are candidates,** not proven
   schedule independence.  Residual unknowns, indirect calls, guards, and type
   failures need their own fixtures.  Accepted.
5. **Qualify general claims.** Sections 2 and 4 now describe obstacles of this
   encoding, not universal impossibilities.


## 9. Possibilities: a linear demand token for shared evaluation

Status: exploratory.  This section lays out candidate designs, their
trade-offs, and hand traces.  It is not a chosen design.  The origin is
Thomas's suggestion to reuse March's effect-token discipline for demand.  It
was revised after Codex's review: shared evaluation is required, the
reference continues past unknowns, and token conservation needs a net-level
invariant.

### 9.1 A lesson from the first sketch

The first sketch hung consumers off a `Memo` through an ordinary fan tree and
claimed the token "meets the Memo head-on".  It does not.  The token travels
on *control* (continuation) wires, while consumers reach the shared value
through *data* fans.  A consumer's request therefore arrives at a fan's
**auxiliary** port, and Lafont's nets interact only on principal ports.
Every possibility below relaxes that rule somewhere.  They differ in *where*.

### 9.2 Shared core (common to every variant)

- `Ev[k]`: the demand token.  Principal: what is demanded.  Auxiliary `k`: the
  continuation, which receives the result.
- **Results** on continuation wires: `Int n` / `Bool b`; `Unknown` (stuck on
  a `Hole`, or depending on one); `Poison e` (a demanded error).
- `Susp_t[a…]`: a single-consumer thunk.  Forcing it materializes `t` over its
  captured wires `a…`, and the `Ev` enters the template's root.
- **Shared pending value** (`Memo`) states:
  - `P` (pending);
  - `E` (evaluating for one parked requester);
  - `R` (residual: stuck on an Unknown);
  - `V(v)` (value).

  Evaluation happens at most once per epoch in state `P`.
- **R-Combine** (binary strict operator, left then right results `L`, `R`):
  - `L` is Poison: send `L`.  The right operand is never requested.
  - otherwise, `R` is Poison: send `R`.
  - otherwise, either known operand has the wrong type: send the operator's
    type error, even if the other operand is Unknown.
  - otherwise, either is Unknown: send `Unknown`, and the operator stays as a
    stuck subnet.
  - otherwise, compute, with overflow producing Poison.

  This matches the reference: `Add(?x, err)` is an error, `Add(err, ?x)` is
  an error, and `Add(?x, 5)` is stuck.
- **R-Unknown:** `Ev[k]` ⋈ `Hole` → send `Unknown`; the `Hole` stays.  The
  token continues past unknowns (Codex's correction (a)).  Only ordered-guard
  clause selection blocks, and even then it returns `Unknown` outward.
- **Invariants to test (all variants):**
  - **I1, token conservation per thread:** the number of `Ev` agents plus
    results in flight on continuation wires equals exactly 1.  Parked
    requesters (an operator or shared value waiting for a result) are passive
    continuation state forming a single chain back to the root; they are not
    tokens, so nested requests do not violate I1.  Check it after every
    rewrite.  This is a net-level invariant, not CAS
    linearity (Codex's correction (b)).
  - **I2:** a shared template is materialized at most once per epoch from
    state `P`.
  - **I3:** there is no active pair inside an unforced thunk or a pending
    shared value.
- **Determinism:** each variant has an agent that interacts on a
  non-principal port, so Lafont's strong-confluence argument does not apply
  directly.  Determinism would have to come from I1: with one token per
  thread, two requests can never race at the same agent.  Multi-principal
  extensions of interaction nets exist (I recall Alexiev's work; to check),
  and their results should be consulted.

### 9.3 Variant A: a Memo with one request port per use site

- `Memo_t^k[r1…rk | u | a…]`: `k` interaction-capable request ports, one per
  static use site.  `k` is known at lowering, since sharing arises from `dup`
  or reused parameters.  An update port `u` receives the result.
- A request at `r_i`:
  - state `P`: move to `E(i, k)`; materialize `t` with its output on `u`.
  - state `V(v)`: send `v`; remove `r_i`.
  - state `R`: re-enter the stuck subnet on `u`.
- On result `x` at `u`:
  - a value: move to `V(x)`; answer the parked requester; remove its port.
  - Poison: answer and memoize only for this epoch; retain the dependencies
    needed to retry after new bindings. Error precedence can change then.
  - `Unknown`: move to `R`; answer `Unknown`; **keep** the port.
- `Erase` at `r_j` removes the port.  With no ports left:
  - state `P`: erase the captured wires, never materializing.
  - state `V` or `R`: erase the held value or subnet.

Trade-offs: all sharing logic lives in one agent, and the fan agent is
untouched.  But it introduces a new variable-arity agent kind with multiple
interaction ports, and port removal needs bookkeeping.  Sharing that arises
dynamically, beyond static use sites, would need port growth.

### 9.4 Variant B: fans pass the token upward (Thomas's suggestion)

- The `Memo` keeps a **single principal port**, with states as in 9.2.
  Consumers reach it through an ordinary fan tree.
- Fans gain one ability: an `Ev` arriving at a fan's **auxiliary** port
  climbs to the fan's principal side and on up the tree.  The fan records
  which branch asked.
- When the value (or Poison) comes back down, each fan copies it as fans
  already do.  The asking branch receives its copy with the token.  The other
  branch keeps a **parked copy** that answers its consumer's later request.
- `Unknown` coming down leaves both branches connected, so a later epoch can
  ask again, as in variant A's `R` handling.
- Erasure is ordinary fan/erase.  When the last branch is erased, `Erase`
  reaches the `Memo`: in state `P`, it erases the captured wires without
  materializing.
- This does not conflict with the world-token rule.  `Fan(World)` is rejected
  because it would *duplicate* a linear token.  Passing a demand token
  *through* a fan doesn't copy it; the token stays linear.

Trade-offs: the `Memo` stays simple, erasure and reclamation are exactly
today's fan/erase, and only one existing agent kind gains a capability.  This
may extend naturally to dynamically created sharing.  But fans now carry
per-node state (which branch asked, and the parked copy), and a request's
cost grows with fan-tree depth.

### 9.5 Variant C: a separate control port (Thomas's suggestion)

Give every computation agent two ports that can interact: a *data* principal
port carrying the value it produces, and a *control* port carrying the demand
token in and out.  Values keep a single principal port.  A thunk is a value
whose control port is closed until it is forced.

- **Requests travel only on control wires.** This resolves 9.1 directly: the
  token never arrives at a data fan's auxiliary side, because it never travels
  on data wires at all.
- **The control-side counterpart of a fan is a merge.** A shared cell with
  `k` consumers has `k` control wires entering one control port through a
  merge tree that remembers which caller is active and routes the return to
  it.  Asynchronous circuit design has such an element, CALL (to check).  The
  symmetry is the point: on the data side a fan copies a value out to `k`
  consumers and holds no state; on the control side a merge admits one of `k`
  callers and remembers who it was.
- **Values return either way.** The reply can ride back on the control return
  wire with the token, or the data fan can distribute copies to every consumer
  when the cell completes, each copy parked at its leaf until that consumer is
  authorized.  The second keeps data fans stateless.
- **A and B look like fusions of C (conjecture).** Variant A fuses the merge
  tree into the cell (its request ports) and returns values on the control
  path.  Variant B pushes the merge's memory into the data fans (the asking
  marks) and lets requests climb data wires.  C is the unfused form: explicit
  control wires, stateless data fans, stateful control merges.  This is a
  reading of the design space, not a proven equivalence; section 10 gives
  small trace mappings and the mismatches.
- **Authorize and collect.** Token arrival at a region's control port
  authorizes the region.  The token then visits operators in the reference's
  order, descending into thunks and shared cells through their control ports,
  and collects each result.  Pure arithmetic whose operands are already values
  may fire on the data side ahead of the token; when the token reaches such an
  operator the result is simply collected.  The token parks (a join) only
  where a data rule is still producing the value.
- **Effects.** In CBPV terms the order computations are run in is the effect
  order, so the world can be the token's payload: pure regions never read it,
  and `Emit` reads and replaces it on the control path.  Linearity checks then
  apply to the token.  The cost is representational: pending effects become
  token state rather than `Emit` nodes in a residual graph, so a paused net
  would have to serialize the token's position and payload.  The B probe
  keeps its continuation and marks as explicit in-memory state that can be
  cloned; nothing is persisted through the image codec, and save/reload of a
  paused net is untested for every variant.  Whether to merge the two tokens
  is open.
- **Determinism.** With one token the merge never arbitrates, and the token
  invariant I1 holds as before.  Data-side rules are ordinary Lafont rules and
  confluent on their own.  Separate ports do not by themselves make the rules
  independent: the critical pairs that matter involve several agents at once,
  such as a completion's value distribution against the token's return,
  erasure against a copy in flight, and last-consumer deletion against a
  pending return.  Each must be shown to commute or be resolved by an explicit
  scheduling constraint (for example a join at the use site so the token waits
  for its copy).  One sequential scheduler succeeding is not evidence of
  confluence.  Section 10 enumerates the pairs for the probe.
- **Forking.** A read/write split as in March 5's token pool becomes natural:
  the linear token may fork into copyable read tokens for provably
  independent pure regions, joining by operand position so that the left
  error still wins.  A merge that queues a second caller until the first
  returns keeps shared cells safe under two tokens: the value is computed once
  and the queued caller gets a hit.  What remains unsolved is cancelling a
  sibling region that is still running when the join no longer needs it, the
  same race as section 5.  Open.
- **Costs.** One more port and wire per computation agent, a merge tree per
  shared cell, and more agents than B.  Whether the cleaner rule table pays
  for that is a measurement, not an argument.

### 9.6 Other possibilities worth keeping open

- **Token-passing call-by-need encodings from the literature,** such as
  Sinot's (to check).  Compare them with A and B before choosing.
- **Force-then-fan at exact force points** (section 4, candidate 1), as an
  optimization within either variant where the first use is statically known.
- **Merging the demand token with the world token,** or keeping them separate.

### 9.7 Traces requested by Codex

Each step is written as "the request reaches the shared value".  In variant A
that means *at request port `r_i`*; in variant B it means *climbing the fan
tree to the Memo*; in variant C it means *the token arrives through the
control merge tree at the cell's control port*, so no request traverses a data
fan and the fan only carries values down.  The observable outcome and
invariants are the same in all three unless a step notes otherwise.

The program for traces 1, 3 and 4 is

    h(c, x) = Add( If(c, Add(x, 1), Mul(x, 2)), x )

where `x` is a shared pending `e = Mul(6, 7)`.  It has three use sites: the
`then` template, the `else` template, and the outer right operand.

**Trace 1: the first consumer is chosen at run time; one evaluation; `c = true`.**
1. The root `Ev[out]` meets the outer `Add`, which requests its left operand:
   `Ev[k1]` meets `If`.
2. `If` receives `true`.  `Erase` meets the `else` thunk, and its use site is
   released.
   - A: port `r2` is removed.
   - B: its fan branch collapses.

   The shared value remains in state `P`.
3. The `then` thunk materializes `Add(x, 1)` and receives `Ev[k1]`.  It
   requests `x`, and the request reaches the shared value in state `P`.  The
   requester is parked, and `Mul(6, 7)` is materialized, yielding 42.  The
   state becomes `V(42)`.
   - A: `r1` is answered and removed.
   - B: 42 descends; the asking branch receives it, and the outer-use branch
     keeps a parked copy.
4. The inner `Add` sends 43 to `k1`.  The outer `Add` requests its right
   operand, and the request is answered from the stored value (A) or from the
   parked copy (B).  It receives 42.  The last use is released, and the shared
   agent is reclaimed.
5. The root receives 85.  `e` was materialized exactly once, and I1 held
   throughout.

With `c = false`, the `then` thunk is released instead, and the `else` branch
is the first requester.  The result is 126 (84 + 42), still with one
evaluation.  (Codex's correction; an earlier draft said 84.)

**Trace 2: the token returning through nested calls.**
Take `f(y) = Add(g(y), 1)` and `g(z) = Mul(z, 2)`, called as `f(x)`.
1. `Ev[k]` meets `Call_f`, which selects the clause and materializes
   `Add(Call_g(y), 1)`.  The `Add` requests its left operand: `Ev[k1]` meets
   `Call_g`, which materializes `Mul(z, 2)`.
2. The `Mul` requests `z`, which reaches the shared value (either variant),
   and 42 returns.  The `Mul` sends 84 to `k1`.
3. The token has returned into `f`'s `Add` as the value on `k1`.  The `Add`
   requests `1` and sends 85 to `k`.

Returning from a call is simply delivering a result to the caller's
continuation.  I1 holds at every step.

**Trace 3: one consumer erased.**
Trace 1, step 2 is the case: the use site is released without forcing `e`,
and the remaining consumers share a single evaluation.

**Trace 4: every consumer erased.**
Take `x drop x drop 0`, with `e = Add(MAX, 1)` and two use sites.  Both are
released.
- A: the last port is removed.
- B: the last branch is erased, and `Erase` reaches the `Memo`.

The state is `P`, so the captured wires are erased and **`e` is never
materialized**.  The root receives 0, with no overflow, matching B0d.

**Trace 5: blocked on an unknown, then resumed.**
Take `Add(x, x)`, with `e = Add(Hole y, Mul(6, 7))`.

Epoch 1, with `y` unknown:
1. The left request reaches the shared value in state `P`.  `e` is
   materialized.  `Hole y` returns `Unknown`, and the token **continues** to
   `Mul(6, 7)`, which yields 42 (the static island).  R-Combine yields
   `Unknown`, leaving the stuck subnet `Add(?y, 42)`.
2. The shared value moves to state `R`.  The left use site stays connected
   (A keeps its port; B keeps both branches), and `Unknown` is returned.
3. The right request also gets `Unknown`.  The outer `Add` becomes stuck, and
   the root receives `Unknown`. Saving that pending net as an image is a
   proposed capability, not implemented by the scalar probes.
   The static island was computed once, and nothing else was forced.

Epoch 2, binding `y = 1`:
1. A new root `Ev` re-requests the left operand, re-entering the stuck
   subnet.  `Add(1, 42)` gives 43, and the state becomes `V(43)`.
2. The right request is answered from the stored value (A) or from the parked
   copy (B).
3. The root receives 86, the same as a direct run with `y = 1`.  `Mul(6, 7)`
   was computed once across both epochs.

### 9.8 Parallelism, under any variant

Pure arithmetic may run in parallel only inside a template that has already
been forced, and only over its strict subterms that contain no calls, thunk
forces, or shared-value requests.  The reference demands those subterms once
their region is forced, so running them is authorized.  They combine through
R-Combine, whose result does not depend on which operand finishes first.
Nothing unforced runs, so unused arithmetic errors cannot leak.

### 9.9 Open questions

- **A, B, C, or a literature encoding?** Decide after comparing hand traces,
  rule counts, and the determinism argument.  A small prototype of each on
  the five traces above could settle it cheaply, once implementation is
  approved.
- **One token or two?** Should the demand token and the world token merge?
- **Parallelism measurement:** how much real work falls under 9.8?
- **Multiple results and data (N3):** follows the R1 decision.
- **Cost of re-entering stuck subnets** in epochs that bind nothing new.

## 10. The C probe: state, rules, critical pairs, measurements

Status: implemented as `src/inet_demand_c.rs` for the scalar subset, with
both reply topologies. March has independently integrated and verified A, B
and both C variants against the now-sixteen shared tests, with B/C audits
after each run, plus C's 13 unit tests. It runs under one sequential scheduler;
its determinism
is by construction under the constraints in 10.3, not a confluence result.

### 10.1 Where state lives

| Agent | State |
| --- | --- |
| `Cell` (one per reached node) | cached result with its epoch; evaluating; materialized; operand use sites; control state `Idle` or `Waiting { slot, phase }`; roots of its data tree and control tree |
| `Site` (one per materialized use site) | used cell; consumer `(cell, slot)` or the observer; position in the data tree and in the control tree; parked copy with epoch (reply-on-data) |
| `Fan` (data tree node) | parent and two children; no state |
| `Merge` (control tree node) | parent and two children; the active side |
| continuation | the chain of waiting cells, linked through sites and active merges; no host-side evaluation frame stack |
| holds | static use counts per program node, host-side, as in B |
| token | position and optional carried value; distribution also holds a host pending-work vector |

### 10.2 Rules

Each is one transition touching one agent and a neighbour, except
materialization (template-sized) and the release cascade (S3).

- **Call** at a site: with reply-on-data, a valid parked copy resumes the
  consumer at once (a site hit); otherwise the call climbs the control tree.
- **Climb** into a merge from side `s`: an already active merge is an
  invariant error; otherwise record `active = s` and continue up.  Reaching
  the cell yields **AtCell**.
- **AtCell**: an evaluating cell is a cyclic call (error).  A valid cached
  result returns down the control tree carrying the value (a cell hit).
  Otherwise materialize if needed (operand sites attached to both trees of
  each operand; with reply-on-data a site attached to a cell that already has
  a valid value receives its copy at once), mark evaluating, count the
  evaluation, and either complete (constant, hole) or **Demand** operand 0.
- **Demand**: the cell records `Waiting { slot, phase }` and the token moves
  to that operand's site.
- **Complete**: store the result with the epoch; a value releases the cell's
  operand sites (last-consumer release).  Then, reply-on-data: **Distribute**;
  reply-on-control: **Return** carrying the value.
- **Distribute** (S2): one tree node per transition; a fan pushes its
  children, a site parks the copy.  When nothing is pending, **Return**
  carrying nothing.
- **Return** at a merge: take the active side and descend to it.  At a site:
  a carried value is a late copy (parked, with reply-on-data) and resumes the
  consumer; an empty return resumes with the site's parked copy, which must be
  valid.
- **Resume** at the consumer: its `Waiting` phase decides.  Left: an error
  completes, otherwise demand the right operand.  Right: combine and
  complete.  Condition: a Boolean erases the rejected branch's site and
  demands the selected one; a non-Boolean is a type error; unknown or error
  completes as such.  Branch: complete.  The observer's resume ends the run.
- **Erase**: detach the site from both trees (fans collapse; a merge with a
  call active on its other side is kept), release one static use of the used
  node; when its last use goes, drop its cell and release what it held,
  through its sites if materialized, otherwise through its static operands.

### 10.3 Scheduling constraints and critical pairs

The probe runs under: S1, one token drives every control rule; S2, a
completion's distribution runs to quiescence before its return; S3, a release
cascade runs to completion; S4, nothing fires ahead of the token.  Under a
free scheduler these pairs would have to be resolved:

1. **Distribution against return.**  If the return outran the copies, the
   caller's site would lack its copy.  Resolved here by S2.  Alternatives: a
   join at the site so the token waits for its copy; or reply-on-control,
   where the pair does not exist because the value rides with the token.
2. **Erasure against a copy in flight.**  Either order leaves the same live
   state (the copy is dropped at the collapsed fan, or the parked copy is
   dropped with the site); a fan rule for an erased child is needed.  Here
   S2 and S3 keep them sequential.
3. **Erasure against an active merge.**  Cannot arise: with a DAG and one
   token, an erased branch contains no cell on the token's path.  The machine
   still reports the case as an invariant error rather than assuming it.
4. **Last-consumer deletion against a pending return.**  Cannot arise: a
   waiting cell is held by its caller's site.  Guarded by an invariant.
5. **Completion against stale parked copies.**  Copies carry epochs; a stale
   copy is ignored and later refreshed by a broadcast or a late copy.
6. **Ahead-of-token firing** (excluded by S4).  If enabled for scalar
   arithmetic, an error in an undemanded branch would exist only as a parked
   value never collected, so outcomes would not change; the costs are wasted
   work and scheduler starvation.  This relies on data rules never diverging,
   true for scalar arithmetic and false once calls exist.

### 10.4 The two reply topologies, measured

Transitions, routing steps, memo hits, peak and final live agents (from the
probe's comparison test; A and B for scale):

| Workload | A | B | C reply-on-data | C reply-on-control |
| --- | --- | --- | --- | --- |
| nested sharing, depth 30 | 122 / 61 / 30 / 122 / 31 | 213 / 90 / 30 / 153 / 2 | 397 / 90 / 30 / 153 / 2 | 425 / 120 / 30 / 153 / 2 |
| wide sharing, 32 sites | 130 / 65 / 31 / 128 / 34 | 256 / 63 / 31 / 158 / 2 | 300 / 0 / 31 / 158 / 2 | 389 / 64 / 31 / 158 / 2 |
| dynamic first consumer | 18 / 9 / 1 / 20 / 8 | 29 / 2 / 1 / 23 / 2 | 62 / 3 / 1 / 20 / 2 | 47 / 2 / 1 / 20 / 2 |

Reading: reply-on-data does no control routing at all for repeat demands
(the wide case shows zero routing and 31 site hits) but pays for every copy
at completion, including copies to sites later erased (65 copies in the wide
case: 32 for the shared node, 30 for the inner additions, 1 to the observer,
2 for the constants).  Reply-on-control makes no copies and pays a merge
traversal per demand.  C charges copies and hops as separate transitions,
which is why its transition counts exceed B's; B's fan-parked copy sits
between the two (copied on demand, climbing to the nearest parked copy).  A
and B agree with C on every outcome and on one evaluation per node.  Only A
retains its cells (final live 31, 34, 8); B and C end with the root value and
its observer.  Teardown work (fan and merge collapses, site removal) is what
B and C pay for that. C's separate teardown counter records 180, 192 and 27
steps on these workloads; B does not expose an equivalent teardown counter.
Logical deletion leaves vacant backing-array slots in both implementations;
these figures are not total memory usage or equal-cost instructions. See
`DEMAND-COMPARISON.md` for the storage and fuel review.

### 10.5 A and B as fusions of C: mappings and mismatches

| Feature | A | B | C |
| --- | --- | --- | --- |
| request routing | direct CID lookup (not a net) | climbs the data fan tree | climbs the control merge tree |
| who remembers the caller | the cell's request port | the fan's asking mark | the merge's active side |
| where copies live | the requesting slot, filled on return | the fan below the turn-around, on demand | every site, at completion (reply-on-data) or nowhere (reply-on-control) |
| continuation | host frame stack | host frame stack | waiting cells plus active merges |
| reclamation | none (cells retained) | last-consumer | last-consumer |

The reading "A fuses the merge into the cell; B moves its memory into the
fans" holds for *state placement*, as the table shows.  It is not a
step-for-step equivalence: transition counts differ (10.4), A's per-slot cache
is filled lazily where C reply-on-data broadcasts, B loses a parked copy when
a fan collapses (the cell serves it), and C reply-on-control needs no
late-copy rule while reply-on-data does.

### 10.6 N0 under C: sketch and obstruction

`Apply(code, args)`: the token enters the apply agent's control port, calls
the function operand (a code value, data side), materializes a fresh instance
of the template with the arguments wired to the instance's operand sites and
the instance's control port wired to the apply agent's continuation, and
enters it.  A fan meeting a code value copies the reference; nothing walks
into code. One candidate partial-application representation would capture an
unevaluated `x` as an explicit *wire* (a site on `x`'s cell). Copying such a
captured-code value can create dynamic consumers beyond the scalar probe's
static hold table; that route needs dynamic lifetime tracking. This is not
a decision to add implicit lexical closures, nor the current semantics of
under-applied `Apply`. The reference's reflected closed-code construction is
another route to compare, including how it preserves pending-work sharing.
The scalar probe does not touch `Intern` or `Quote`, and nothing here claims N0.

### 10.7 Established and not established

Established, for the scalar subset under S1 to S4: outcomes equal to the
reference on the shared contracts and on generated DAGs in either fact order;
one evaluation per node per epoch with ground values never recomputed;
logical last-consumer reclamation to two live agents (not backing-storage
reclamation); control routing accounted
separately from data distribution; recovery from every budget cut.  Not
established: confluence under a free scheduler; save or reload of a paused
net (the machine is cloneable in memory only); effects, token forking, or
ahead-of-token firing; anything about N0. Release cascades and cancellation
are outside transition fuel; the budget is not a total-host-work bound.
