# Demand join: independent consumers of one pending producer

Status: implemented as an isolated experiment, authorized by Thomas on
2026-09-25. Claude independently reviewed the engine and rules, reproduced all
36 original example tests and reported results, and found no blocking issues.
The later conditional composition is tracked in `CONDITIONAL-JOIN.md`. No production
language changes. This takes
priority over the narrower, reviewed `CROSS-BRANCH-SHARING.md` control fixture.

## Model change and scope

Only Join has two active ports: a=0 and b=1. Its output=2 is passive. Every
other agent has one active port, zero. This is an extension of the original
single-principal interaction-net model, not a claim that the old runner already
encoded arbitrary demand. Independent requests use control wires; ordinary
Fan agents distribute values on separate data wires. There is no host memo,
reference counter, return-address stack, or hidden demand traversal.

Each control input receives at most one Release or Erase. Single use is
enforced for host actions by retiring the consumed boundary; for rule-generated
tokens this is a construction/protocol assumption, not a general runtime check.
Join is not reset or reused. It has small explicit coordination states
(Join1 and Absorb), not a
remembered caller or stored result. A late consumer obtains the already-produced
value parked on its data input. This shares an execution instance, not arbitrary
equal source descriptions; passive code storage and effect sequencing are absent.

## Exact rules

Parenthesized names are the non-interacting interface ends. For Join, list the
other input first and output second, whichever input received the token.

```text
Release >< Join.at_a(b, out) => Absorb[p=b]; Release[p=out]
Release >< Join.at_b(a, out) => Absorb[p=a]; Release[p=out]
Erase   >< Join.at_a(b, out) => Join1[p=b; out]
Erase   >< Join.at_b(a, out) => Join1[p=a; out]
Release >< Join1(out) => Release[p=out]
Erase   >< Join1(out) => Erase[p=out]
Release >< Absorb => empty
Erase   >< Absorb => empty
```

One release activates the producer once; subsequent arrivals are absorbed.
One erasure preserves the other use; two erasures discard the unreleased
producer. Post-activation interruption is not implemented.

Each consumer is a unary AddK behind an operand-demanding gate:

```text
Release >< LGate(next, input, operation, demand)
    => input--operation; Release[next]; Release[demand]
Erase >< LGate(next, input, operation, demand)
    => Erase[next]; Erase[input]; Erase[operation]; Erase[demand]
Erase >< Observe(result) => Erase[result]
```

All remaining value, arithmetic, Gate, Stop, and erasure rules are the earlier
explicit ones. LGate requests exactly one operand. Its `next` ends at a private
Stop in this experiment; this does NOT establish laziness for arbitrary forward
instruction chains or multi-operand operations.

## Fixture wiring

```text
consumer A control -- LGateA.p      LGateA.demand -- Join.a
consumer B control -- LGateB.p      LGateB.demand -- Join.b
Join.out -- Gshared.p               Gshared.next -- StopShared.p
Int(2) -- Gshared.input             Gshared.operation -- Mul.p
Int(3) -- Mul.rhs                   Mul.out -- Fan.p
Fan.left -- LGateA.input            Fan.right -- LGateB.input
LGateA.operation -- AddK(1).p        LGateB.operation -- AddK(2).p
LGateA.next -- StopA.p              LGateB.next -- StopB.p
AddK(1).out -- ObserveA.p           AddK(2).out -- ObserveB.p
ObserveA.result -- result A         ObserveB.result -- result B
```

The host test supplies Release or Erase once at a named control boundary. It
does not inspect the producer or choose rewrite outcomes. Erase reaches a
dropped result boundary through Observe; that terminal eraser records a dropped
output and remains counted as live. Unknown input erasure likewise remains at
its boundary. A retired host boundary is tracked explicitly without renumbering
other endpoints. Unconsumed boundaries must still be fully wired.

The unknown-input fixture replaces Int(2) with a free interface. The Spin
fixture replaces Int(2)/Mul/Int(3) with Tick/Spin. All runs have at most 128 local
interactions per `run()` call, with external 30-second test/CLI timeouts. There
is no indefinite host recursion or unbounded background process.

## Required observations

- Neither consumer asks: no work runs; the pending graph is retained.
- One consumer is dropped, the other remains passive: no work runs, and the
  other can still ask later. Erasure waits on the discarded Fan auxiliary.
- Either consumer asks and the other is dropped, in either order: one producer
  evaluation; answer 7 for A or 8 for B; discarded output marked by an eraser.
- Both ask: one multiplication, two additions, answers 7 and 8.
- A asks while B stays passive, then B asks later (and the reverse): one
  multiplication total. No recomputation when the second control is released.
- Both drop: zero arithmetic/Spin; entire internal graph erased. Only result
  erasers, plus an input eraser in the unknown-input case, remain.
- An unknown producer input preserves the shared graph and selected consumers;
  supplying the input later completes the same pending work.
- A demanded Spin stops at the fixed cap, with one Spin/Tick producer, not
  duplicated loops. Unrequested or fully discarded Spin takes zero Spin steps.
- Every finite-fuel cut leaves audited wiring and can resume safely.

## Executed observations

The driver is `examples/demand_join.rs`, using the isolated port-aware engine
`examples/support/multiport_runner.rs`. Finite cases give the same results under
lowest-ID and highest-ID pair selection (called Oldest/Youngest in the runner).
These are deterministic stress schedules, not fair parallel schedulers.

| Fixture | Interactions | Result | Live agents |
| --- | ---: | --- | ---: |
| Neither asks | 0 | Both pending; no arithmetic | 15 |
| A asks, B passive | 10 | A=7; Int(6) parked for B | 7 |
| B asks later | 15 total | A=7, B=8; one multiplication total | 2 |
| Both ask | 15 | A=7, B=8; one multiplication | 2 |
| A asks, B drops | 16 | A=7; B has boundary eraser | 2 |
| Both drop finite | 18 | Zero arithmetic; two boundary erasers | 2 |
| Both drop Spin | 17 | Zero Spin; two boundary erasers | 2 |
| Both drop unknown | 17 | Zero arithmetic; three boundary erasers | 3 |
| Drop A, ask B, input unknown | 10 | Pending producer preserved | 7 |
| Supply input 2 later | 16 total | B=8; one multiplication | 2 |

Demanded Spin reaches the 128-interaction cap with one Tick/Spin producer.
For A asks/B drops, lowest-ID selection executes 118 Spin steps and leaves
7 live agents; highest-ID selection executes 120 and leaves 10. The latter
starves some cleanup, so B's result eraser has not yet reached its boundary.
The three remaining active pairs are Tick/Spin, Release/StopShared, and
Erase/ObserveB; the private StopA has already been consumed. The trace option
now prints pending pairs explicitly to distinguish these cases.
Bounded execution is not a termination, fairness, or prompt-reclamation proof.

Fourteen tests cover all nine passive/ask/drop combinations, both arrival
orders and both schedules, delayed consumers, unknown input, discarded and
demanded Spin, 720 fuel-cut/resumption combinations, explicit Join critical
pairs, passive output and unsupported arrival handling, atomic numeric and
internal-wire rejection, interface linearity, and boundary/budget guards.
Boundary erasers are deliberately not called zero retained memory.

Initial verification: all 36 example tests (14 Join, 13 forward-release, 9 selective
activation) pass in debug and release. Clippy with warnings denied passes for
all three example binaries; `cargo fmt --all --check` and `git diff --check`
pass. The production/reference test suite was not rerun for this isolated
change. Reproduce the bounded checks from `march6/`:

```sh
timeout --kill-after=2s 30s cargo test --offline --example demand_join --example contextual_net --example selective_activation
timeout --kill-after=2s 30s cargo test --offline --release --example demand_join --example contextual_net --example selective_activation
timeout --kill-after=2s 30s cargo clippy --offline --example demand_join --example contextual_net --example selective_activation -- -D warnings
timeout --kill-after=2s 30s cargo run --offline --example demand_join
```

## Local order argument, not a blanket theorem

Under the stated single-arrival/no-reset protocol, the simultaneous-arrival
cases are Release/Release, Release/Erase, Erase/Release, and Erase/Erase. In
either processing order, the final upstream signal is Release if either asks,
otherwise Erase. Each pair of arrivals takes two join-state interactions.
Test both first-arrival choices directly, not only whole-net scheduler orders.

This is a local critical-pair argument for these rules. Full-system confluence,
future extensions, parallel atomicity, and arbitrary ill-formed control graphs
are not claimed by passing the fixtures. A real parallel implementation would
need atomic selection of a rewrite touching the same Join. The experiment is
single-threaded and tries alternative schedules.

## Implementation boundaries

Use an isolated port-aware diagnostic runner. Matching must carry BOTH port
indices; rewrites preserve all non-interacting ends, including Join's other
active input. Port 2 must never be treated as active merely because its agent
is a Join. Unsupported pairs are reported, not silently evaluated by host code.
Keep the single-principal runner and its 22 example tests as controls.

As before, diagnostic IDs/slots and trace history are append-only. Logical
erasure is measured separately from physical storage. No reusable program
description is deleted, and no external effects are performed by these rules.
