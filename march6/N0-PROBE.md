# N0 probe: quotations and application as demand-driven agents

Author: Claude; integration and independent review: March. Status: integrated
experimental N0a checkpoint, **not an N0 gate pass**. Code: `src/inet_n0.rs`,
with 13 internal tests and seven independent tests in `tests/inet_n0_review.rs`.
Review fixed residual-groundness propagation but found canonical sharing and
cycle-detection gaps (section 7). Those are characterized, not waived.

Everything below is stated for the N0a subset: scalars, holes, `Add`,
`Mul`, `If`, closed `Quote`, `Param`, exact-arity `Apply`. Section 8 lists
the machinery a second slice (N0b, reflected code through `Intern`) would
need; none of it is implemented.

## 1. What the slice claims, and does not

Tested (section 6): outcomes equal to the CAS reducer on the
witnesses and on generated programs, direct and in staged epochs in either
fact order; one evaluation per template node per instance per uninterrupted epoch; code
passed as a value through several calls; a shared argument evaluated once
however many parameters and outside consumers reach it; an argument no
parameter demands never evaluated; an erased consumer preserving work it
shared with an application; instances kept across epochs and never
re-created on retry or after a budget cut; logical reclamation to two live
agents on the completed examples; a consistent machine after budget cuts.
Instance records and their argument vectors remain allocated afterward.

Not claimed: confluence under a free scheduler, parallelism, effects,
token forking, backing-storage reclamation, persisted paused nets, any node
kind outside the subset, and anything about `Intern`. The probe is an
explicit transition machine over agents and wires, not a Lafont net, as
with probes A, B, and C. Per-instance memoization is weaker than the CAS
reference's canonical instantiated-work sharing. The self-application witness
also differs in cycle/error observation; agreement on other fixtures does not
establish agreement over the whole syntactically accepted subset.

## 2. Agents, ports, and wires

The base is the revised variant C, reply-on-control only: a value rides
back with the token to the site that asked for it, so there are no data
fans and no distribution step. Three things are added.

**Cells.** One per demanded `(instance, template node)` pair (a `Key`).
A cell has a control tree (its use sites, merged pairwise), operand slots
(one site per operand edge), a memo (`result`, epoch-stamped), and a
control state (`Idle` or `Waiting{slot, phase, return_to}`).

**Sites.** One per operand edge of a materialized cell, plus one for the
observation, one per instance root, and one per proxying parameter. A site
knows its child (the cell it uses), its consumer (the cell and slot it
serves), and its position in the child's control tree.

**Merges.** Stateless two-to-one nodes of the control tree, as in C.

**Instances.** An instance is `(body template, argument keys)`. Instance 0
is the program. Instance `k` is created by exactly one `Apply` cell, on
the first epoch in which that cell learns its function operand is code
with matching arity. The cell holds one use of the instance root
(`Key{k, body}`) through an ordinary site in an extra slot.

**Parameters as proxies.** `Param(i)` in instance `k` has no operands of
its own. When first materialized it attaches a site on the caller's
argument key `args[i]` and forwards demand there. That attachment raises
the argument's use count by one at that moment; it is released when the
parameter cell is dropped. Parameter cells in instance 0 answer `Unknown`,
matching the reference's treatment of free parameters.

**Code values.** Demanding a `Quote` node yields `Code(cid)`, a reference.
Copying a code value copies the reference. Nothing walks into a template
until an application instantiates it.

## 3. Rules

Written as (token at agent) -> (next token), with side effects.

| Token | Rule |
|---|---|
| `Call(site)` | count a request; climb from the site |
| `Climb` at a merge | one routing step; climb to the merge's parent |
| `Climb` at a cell root | `AtCell{key, from}` |
| `AtCell` with a valid memo | cell hit; `Resume{from, memo}` |
| `AtCell`, unmaterialized | materialize (create operand sites; a proxy attaches to the caller's argument), then evaluate |
| `AtCell` at `Value`/`Hole`/`Quote` | complete with the value, the binding or `Unknown`, or `Code(node)` |
| `AtCell` at `Add`/`Mul` | wait on slot 0 in phase `Left`; `Call` it |
| `AtCell` at `If` | wait on slot 0 in phase `Condition` |
| `AtCell` at `Apply` | wait on slot 0 in phase `Function` |
| `AtCell` at `Param` (instance > 0) | wait on slot 0 in phase `Proxy` |
| `Resume` in `Left` | left error completes; otherwise wait on slot 1 in phase `Right` |
| `Resume` in `Right` | complete with `combine` (reference order: left error, right error, type on known non-integers including code, overflow, else unknown) |
| `Resume` in `Condition` | `Bool`: erase the rejected branch, wait on the chosen one; `Int`: type fault; `Code` or residual: preserve stuck-term groundness (section 7); `Unknown`/error: pass through |
| `Resume` in `Function` | `Code` of matching arity: create the instance if this cell has none, then wait on the instance-root slot in phase `Body`; `Code` of wrong arity: arity fault; scalar: type fault; `Unknown`/error: pass through, arguments untouched |
| `Resume` in `Branch`/`Body`/`Proxy` | complete with the outcome |
| `Resume` at the observation site | done |

`complete` stamps the memo with the epoch. A `Value`, `Code`, or ground residual result
releases all of the cell's operand sites (last-consumer release), which for
an `Apply` includes the instance-root site, and for a proxy the site on the
caller's argument. `Unknown` and errors keep their sites and are retried in
the next epoch through the same cells and instances.

Release (`drain`) is a cascade run to completion: detach the site
(collapsing merges), decrement the child's use count; at zero, drop the
cell and release what it held (its sites if materialized, its static
operands in the same instance if dormant).

## 4. Use counts: static per template, dynamic per instance

The static count of a node is its number of operand edges within its
template, computed once per template body and cached (`template_walks`
charges every node visited, once per body ever). A cell's remaining count
starts from the static count of its template node and rises by one for
each dynamic use: the observation, an instance root, a proxy attachment.

This handles explicit argument sharing, not the whole sharing question. An argument node lives
in the caller's instance; two parameters reaching it, or a parameter and an
outside consumer, are two uses of one cell, so a pending shared computation
is evaluated once. Separate body nodes which become identical after
substitution are still evaluated separately here; section 7 records examples.

## 5. The return address is an explicit edge

The token carries `return_to`, the use site to resume. A waiting cell also
records it. Resuming is one transition via direct host lookup, not a search.
A stored reference is not itself a mechanically verified local port-rewrite
rule. Merges stay stateless. The continuation is the chain of waiting cells, one
per operand edge currently under demand; it crosses instance boundaries
through the instance-root site and the proxy sites, which are ordinary
sites. Cancellation after a budget cut walks this chain from the root
through those sites.

## 6. Witnesses and measurements

Tests in `src/inet_n0.rs::tests`; each compares against `Reducer` on the
same store and audits the machine afterward.

- nested `square(square(3)) = 81`: two instances, the body evaluated once
  in each, two proxy sites, template walk once for the body
- `apply_twice(square, 3) = 81`: code as a value, three instances
- `id(id(square))` applied through `call`: code survives three calls;
  observed directly, the result is `Code(square)`
- `square(3) + square(4)`: independent wiring per application
- `square(3) + square(3)` with one application node: one instance, one
  evaluation, a cell hit
- `double(6*7) + (6*7)`: the shared argument evaluated once with two
  parameter uses and an outside use
- `first(7, overflow)` and `second(overflow, 7)`: the dormant argument
  untouched; demanding it does fault
- an erased consumer sharing work with an application: the shared node
  evaluated once either way, the instance created only when chosen
- unknown function then unknown argument: nothing instantiated, then one
  instance blocked inside, then the same instance finishing; the rejected
  quotation never demanded
- faults: arity, non-code function, code as an arithmetic operand, integer
  condition, overflow inside and outside an application
- lowering rejects open code, out-of-range parameters, and `Family`
- every budget cut from zero to the full run: audit passes, the rerun
  finishes with the same instances
- generated programs (120 cases, 16 random nodes over a pool of scalars,
  holes, seven templates, applications with occasional wrong arity or a
  non-code function): direct and staged in both fact orders. Integration removes
  the old exclusion of code-valued conditions.

Independent review tests cover ground/non-ground stuck terms under arithmetic,
application and another condition; parameter-proxy branch groundness across
100 budget cuts; atomic binding conflicts; retained instance records; and
explicit witnesses for the two canonical-identity acceptance gaps.

Costs, one run each, budget unbounded. Columns: transitions, requests,
evaluations, memo hits, erasures, peak live, live at end, routing steps,
instances, template walks, proxy sites, teardown steps.

| program | trans | req | eval | hits | eras | peak | live | route | inst | walks | proxy | tear |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| square(square(3)) | 51 | 11 | 8 | 3 | 12 | 27 | 2 | 7 | 2 | 6 | 2 | 24 |
| apply_twice(square, 3) | 69 | 15 | 12 | 3 | 18 | 37 | 2 | 9 | 3 | 10 | 4 | 36 |
| double(shared) + shared | 43 | 10 | 8 | 2 | 10 | 23 | 2 | 3 | 1 | 8 | 1 | 20 |
| square^1(1) | 27 | 6 | 5 | 1 | 6 | 15 | 2 | 3 | 1 | 5 | 1 | 12 |
| square^4(1) | 99 | 21 | 14 | 7 | 24 | 51 | 2 | 15 | 4 | 8 | 4 | 48 |
| square^8(1) | 195 | 41 | 26 | 15 | 48 | 99 | 2 | 31 | 8 | 12 | 8 | 96 |
| sum of 8 distinct square(n) | 244 | 55 | 40 | 15 | 62 | 43 | 2 | 24 | 8 | 26 | 8 | 124 |
| one square(3), 8 uses | 91 | 20 | 12 | 8 | 20 | 43 | 2 | 11 | 1 | 12 | 1 | 40 |

Readings. A nesting level costs a constant 24 transitions and 12 peak
agents: the chain of instances is the call stack, all live until the
innermost returns, then execution cells are released to two logical agents.
Instance records and argument vectors remain. The square body's walk is
paid once (walks grow by one per level only for the program template).
Eight distinct applications cost eight instances; template census still runs
once per distinct body, including the larger program template. One application used eight times costs one
instance and eight memo hits. Routing steps grow with control-tree depth
because trees are built as chains, as in C; balancing is an open choice.

## 7. Integration findings and acceptance gaps

1. `If` whose condition evaluates to a quotation is left stuck by the
   reducer (only a `Const` condition is type checked), yet the stuck node
   counts as ground when its branches are ground, so `Apply` of it faults with "apply operand is not a
   quotation" and `Add` of it faults with "add expects two integers". The
   initial probe collapsed that distinction into `Unknown`. Integration
   preserves the existing reference semantics: an internal `Residual { ground }`
   observation carries structural groundness to consumers. Dormant branches
   are inspected with parameter substitution and current facts, never evaluated.
   `If(code,1,?x)` is non-ground until x is bound; an overflowing but closed
   dormant branch is ground without being forced. The public root observation
   remains `Unknown`, not a serialized residual. Groundness work is counted
   separately and is outside transition fuel. No reducer identity or image
   semantics changed.
2. The `Arity` fault uses the reducer's `expected`/`actual` fields
   unchanged, so `ProbeError`/`Fault` in `demand.rs` were not extended;
   N0 has its own `N0Fault` and `N0Outcome` (with `Code`) for that reason.
   Folding `Code` and `Arity` into the shared types is a small change if
   the shared harness should run N0.

3. **Canonical sharing is not preserved.** With
   `k = Quote(1, Mul(6,7))`, `Add(Apply(k,[1]), Apply(k,[2]))` evaluates the
   multiply twice in this probe, once in the reference. Within one instance,
   `Quote(2, Add(Mul(P0,7), Mul(P1,7)))` applied to `[6,6]` has the same
   problem: substitution coalesces the two multiplies in CAS, but instance/node
   keys do not. The independent test explicitly records two versus one; it is
   an acceptance-gap characterization, not a passing shared-evaluation gate.
4. **The same identity gap changes cycle observation.** Let
   `omega = Quote(1, Apply(P0,[P0]))`. The reference detects an active canonical
   CID re-entry in `Apply(omega,[omega])` and reports `Cycle`; the probe makes
   fresh instances until fuel runs out. Budget exhaustion is not equivalent
   to the reference error. Cancellation remains consistent, and erasing the
   entire unused application still avoids instantiation. Acyclic source syntax
   and absence of `Recur` do not prevent higher-order self-application.

## 8. N0b: what reflected code would need (not implemented)

The reference's Church `two two` builds code with `Intern` over a
description graph (records with `op` fields: `embed`, `param`, `apply`,
`quote`). Under this machine that is a runtime template, and it needs:

1. **Structured ground values in outcomes.** Descriptions are records,
   pairs, text, and unit. `N0Outcome` would gain a ground-term case (or
   carry a `Cid` of an interned ground value), and `combine`'s type check
   would treat it as a known non-integer.
2. **Strict constructor cells.** `Record` and `Pair` demand every field in
   order (phases per field, like `Left`/`Right` generalized), completing
   with a ground term only when all fields are ground; an `Unknown` field
   leaves the constructor pending for the next epoch. Because they are
   strict, nothing pending is ever embedded: embedding is by value.
3. **An `Intern` cell** with one phase: demand the description; on a
   ground term, run the reference's builder (`reflect.rs`, host-side,
   charged as work, validated closed) and complete with `Code(new)`. The
   built nodes are appended to the program's template table; keys and
   cached counts work unchanged since templates are addressed by `Cid`.
   The store is append-only, so this is an insert, never an overwrite.
4. **Embedded code values** become `Quote` nodes inside the new template,
   which are values: no sites, no dynamic holds. So the dynamic-consumer
   worry from INET-DEMAND.md 10.6 does not arise here either; it would
   arise only if a lazy constructor (R1) let an unevaluated node be
   embedded, which is exactly what strictness forbids today.
5. **Validation at runtime** rather than at lowering: closedness and
   parameter range for built templates, on the built graph only.

Items 1 to 3 are mechanical but touch the outcome type and import the
builder from `reflect.rs`; whether that belongs in this module or in the
shared probe layer is a wiring choice for Codex. No obstruction is known.

## 9. Limitations

- Control trees are chains; routing cost is linear in a node's use count.
- `node_evaluations` is keyed by template node and sums over instances;
  per-instance counts are not exposed beyond `instances`.
- Release cascades and cancellation are outside transition fuel, as in C.
- Structural groundness checks also use host worklists outside fuel. The
  template census now uses an ordered visited set rather than quadratic
  `Vec::contains` scans; its count measures distinct nodes, not host instructions.
- `storage()` includes retained instance/argument/count-map entries and backing
  array diagnostics. It is not a full heap-byte measurement.
- `Program::from_store` walks every template reachable at lowering,
  including bodies never applied; a lazy lowering would be a later change.
- Guarded recursion is absent; self-application is tested and exposes the
  canonical cycle-detection gap above. N0b remains unimplemented.
