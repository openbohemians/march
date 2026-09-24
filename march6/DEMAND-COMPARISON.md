# Demand protocols versus eager evaluation

Status: B and both C reply variants are integrated into the shared harness.
The established port-level backend, reducer semantics, and image format are
unchanged. These are scalar experimental machines, not a selected architecture.

## What integration establishes

All four configurations (A, B, C/data, C/control) run the same sixteen shared
tests. B/C quiescent invariants are checked after every harness run, including
budget exhaustion and conflicting bindings. Added review tests cover 24 small
generated DAGs at 80 budget cuts each, retry, then additional context facts;
logical versus allocated storage; and the cost of discarding a large branch.
The reference's twelve demand fixtures include the eager-boundary comparison.

The existing 12 B and 13 C unit tests also run in the real workspace, rather
than only in Claude's scratch copy. Their sharing measurements reproduce here:

| Workload | A transitions | B transitions | C/data transitions | C/control transitions |
| --- | ---: | ---: | ---: | ---: |
| Nested sharing, depth 30 | 122 | 213 | 336 | 304 |
| Wide sharing, 32 uses | 130 | 256 | 266 | 292 |
| Dynamic first consumer, false branch | 18 | 29 | 53 | 37 |

C now carries the caller's site in the token, making merges stateless and
returns direct. Previously it retraced merge marks on return; those older C
counts were 397/425, 300/389 and 62/47 respectively. Direct site lookup is
counted as a transition but not as merge routing; it is not proof of local
port rewrites or zero-cost communication.

These are NOT timings or equal-cost instruction counts. A uses direct map
lookup, B combines some routing/copy work, and C charges distribution steps
separately. Construction, map operations, cleanup, and cancellation are not
fully represented by these totals. There is no eager-backend benchmark here.

## Downsides, separated by cause

| Candidate | What is useful | What it costs or leaves unresolved |
| --- | --- | --- |
| A-inspired control | Small sequential memo baseline; direct requests | Retains memo cells; host continuation stack; does not implement A's proposed incoming request ports |
| B | One fan network routes demand and shares replies; releases dead cells | Fans carry caller marks and parked replies; request cost depends on tree depth; fan collapse may discard a cached reply; host continuation stack |
| C/data | Demand and data have separate wiring; callers can use parked values without routing another request | Maintains both trees; distributes copies even to consumers later erased; completion waits for distribution before returning |
| C/control | Same separation; carries a result directly back only when requested | Repeated consumers traverse control merges on calls; current code still allocates the data-tree structure even though this topology does not broadcast |

C removes the host **evaluation frame stack**, not continuation state or all
host worklists. Waiting cells and their return-site references encode the continuation.
Distribution carries a growable pending-work vector in the token; cleanup has
another work vector. They are not extra logical agents in `peak_live`.

The common limitations matter more than the small scalar measurements:

1. **Logical reclamation is not reclaimed backing storage.** B/C remove dead
   cells and clear dead fan/site entries, but append-only arrays keep vacant
   slots and capacity. They do not yet reuse those slots. On depth 30, both
   finish with two logical agents; B retains 91 issued array slots (90 vacant),
   C retains 121 (120 vacant). `storage()` reports lengths, capacities and
   backing-array capacity bytes, excluding maps and nested allocations.
   Templates, zeroed static-use-count entries, and per-node statistics remain.
   Thus final live=2 is not constant total memory or a production allocator result.
2. **Skipping computation can still require walking its structure.** Discarding
   our 2,000-node branch performs over 4,000 use releases in B/C without
   evaluating the branch. The cascade runs inside a transition; cancellation
   also walks paths outside fuel. A 100-transition budget is not a 100-operation
   or latency limit. Compact suspension boundaries and incremental, budgeted
   cleanup remain work to do.
3. **Sharing currently relies on static consumer counts.** Those counts cover
   even dormant consumers, preventing premature deletion and recomputation.
   Fresh applications and explicit captured pending inputs introduce dynamic
   consumers; this scalar solution does not yet cover that. The separate N0a
   probe now creates dynamic argument-proxy sites, but its per-instance keys
   miss canonical sharing after substitution (including within one body).
   This is not a requirement to add implicit closures. See `N0-PROBE.md`.
4. **C's clarity does not prove arbitrary scheduling safe.** It uses one token,
   finishes distribution before returning, drains erasures atomically, and
   does no ahead-of-token evaluation. Removing those restrictions needs rules
   for value arrival versus waiting, cancellation, and deletion. Neither
   parallel speedup nor confluence has been demonstrated.
5. **Long-lived partial computations retain state.** Unknown/error results
   retry in later epochs; completed scalar values are shared. Dormant consumers
   can keep a computed value alive. Error precedence can change after binding,
   so errors are not permanent ground memo values. These are real obligations
   of the semantics, not just the placement of ports.
6. **The A/B/C scalar scope is narrow.** No protocol `Apply`, `Quote`, `Intern`, general
   guards, effects, dynamic captured inputs, or persisted paused-machine codec.
   In-memory cloning and retry are not image save/reload. The main reference
   supports more than these probes do. The separate N0a experiment adds closed
   `Quote`/`Apply` but has known canonical-sharing and cycle-observation gaps;
   it is not an N0 pass.

## What eager evaluation would change

Here "eager" means ordinary call-by-value: evaluate arguments before entering
the function. It does NOT mean evaluate both branches of a conditional, run
the bodies of held code values, or recompute a shared value on every use.
The distinction is illustrated in
[Cornell's evaluation-strategy lecture](https://www.cs.cornell.edu/courses/cs3110/2009fa/Lectures/lec25.html).

For March, the trade-offs would be:

| Question | Demand-driven March | Eager argument boundary |
| --- | --- | --- |
| Unused failing argument | Can stay dormant | Fails before entering the function |
| Shared argument | First demand computes it; later demands reuse it | Compute once before the call; pass/reuse the resulting value |
| Runtime machinery | Pending state, requests, memo update, use lifetimes | No first-force protocol for ordinary arguments; call order and value lifetimes still required |
| Memory pressure | May retain a pending graph and its dependencies | May release intermediate work sooner, or build large results that are never used; no universal winner |
| Function/module guards | Inspect only needed arguments, in guard order | Ordinary strict arguments have already run before the guards select a body |
| Staging | Unknown facts leave residual computations | Still possible through a separate specializer; eagerness does not preclude compile-time reduction |
| Higher-order values | Pending captures add sharing/lifetime obligations | Evaluated captures simplify forcing, but dynamic references and code lifetime still need management |

An executable witness in `tests/demand_contract.rs` makes the seam concrete:

```text
choose(c, a, b) = if c then a else b

if true then 7 else overflow     -> 7
choose(true, 7, overflow)         -> 7       (current March)
choose(true, 7, overflow)         -> error   (eager argument boundary)
```

The eager-boundary test deliberately normalizes arguments before using the
same closed code. It is a small semantic control, not a new eager evaluator
or an eager throughput comparison. Explicit held code could defer the bad
argument in an eager language, but the calling interface must then express
that suspension; automatic reuse of its result still needs a sharing policy.

An eager language can support normal extract-function refactoring when both
forms preserve strict argument evaluation. What it would not preserve is
March's stronger promise that moving conditional demand behind an ordinary
word boundary leaves unused *argument computations* dormant automatically.

## Working recommendation

Keep demand behavior as the baseline while testing eager execution *within
regions proved to be demanded*. For example, a demanded integer addition can
consume already available values without the general first-force machinery.
Such optimizations must preserve error order, termination behavior and sharing;
"pure" or "used somewhere" alone does not justify evaluating earlier.

C is a promising way to make those boundaries explicit, not a demonstrated
performance winner. Next gates are canonical instantiated-work identity for N0, reusable storage
and honest work budgets, then comparisons against an eager control on matched
workloads. Keep both C reply topologies and B until that evidence exists.
