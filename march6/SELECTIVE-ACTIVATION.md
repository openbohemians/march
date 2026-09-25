# Selective activation: a conditional frontier

Status: rules independently reviewed by Claude; standalone variant implemented
and tested, 2026-09-24 (local date). Claude independently reproduced the nine
variant tests and CLI results and approved the implementation's local rules.
Thomas authorized a bounded experiment; no change to production semantics.
The forward-release runner remains a comparison, not the language specification.

## Question and scope

Can a local branch decision keep an unselected shared computation inactive?

```text
shared = 2 * 3                 # replace with diagnostic Spin in the other case
result = if condition then shared + shared else 0
```

Requested observations:

- Before invocation, no execution interaction occurs.
- After invocation with an unknown condition, both branches remain inactive.
- False produces 0 without starting shared, even when shared denotes Spin.
- True with finite shared produces 12, with one multiplication and one addition.
- True with Spin exhausts a small interaction budget; it does not hang the host.

This first slice uses **one physical pending producer shared by wires**. It
explores activation of that producer, not global memoization or code identity.
The passive graph becomes an active residual in place through local rewrites.
It does not preserve a separate immutable passive copy, implement a reusable
template-to-instance map, or let arbitrary consumers independently activate
one pending unit. Those parts of Thomas's passive/active proposal remain open.
In particular, all of the shared producer's control belongs to the true branch
in this fixture. A producer shared across both branches (or independent calls)
needs a separate activation/cancellation account; discarding one use must not
destroy another live use. This experiment does not establish that account.

The condition here is a supplied Bool or a free interface. Computed conditions
with their own activation protocol are a later compositional test; the runner
must not claim to have a general demand walker merely by following this fixture.

This combines consumer activation at the conditional's boundary (one Need/If
interaction) with forward release inside its selected branch: a D/F hybrid,
not a blanket call-by-need strategy. The fixture's Need represents the request
to observe this expression. It is not injected by a host graph traversal.
No Fan/IfDormant rule is proposed; deciding that duplication itself activates a
suspension would be an additional policy, not a consequence of ordinary sharing.
Possible policies there include activating once and fanning the value, copying
the suspension, or leaving the interaction unsupported. This slice deliberately
chooses the last; it does not endorse either other policy as March semantics.

## Safety envelope

- Single-threaded, no background workers or recursive host evaluation.
- Each `run()` call has a hard limit of 128 pair interactions. This is not a
  lifetime limit on a net; the driver/tests only use explicitly bounded resumes.
- The diagnostic `Tick >< Spin(out)` recreates only Tick and Spin. It is not
  native nonterminating code and never recursively invokes the reducer.
- The runner checks the budget between every bounded local rewrite; tests also
  run under an external wall-clock timeout.
- Record answer, remaining active pairs, live agents, and issued slots. Do not
  infer mathematical divergence from a budget result. The self-reproducing rule
  supplies the specific nontermination argument.
- Do not stop after seeing an answer: continue until quiescence or the cap, so
  unwanted work cannot hide behind an early observation.

## Rule notation and agents

As in `CONTEXTUAL-NET-EXPERIMENT.md`, each agent has exactly one principal port.
The named arguments below are auxiliary ports, in the listed order. Every
external end must occur exactly once on the RHS. Boundary ends are interfaces,
not executable Hole agents. Rules only inspect the two interacting labels.

New agents:

| Agent | Principal faces | Auxiliary ports |
| --- | --- | --- |
| `Need(result)` | suspended conditional | result |
| `IfDormant(cond, tc, tv, fc, fv)` | invoking consumer | condition, true control/value, false control/value |
| `Select(result, tc, tv, fc, fv)` | condition | result, true control/value, false control/value |
| `Bool(b)` | selector | none |

Invocation, then branch choice:

```text
Need(result) >< IfDormant(cond, tc, tv, fc, fv)
    => Select[p=cond; result, tc, tv, fc, fv]

Bool(true) >< Select(result, tc, tv, fc, fv)
    => result--tv; Release[p=tc]; Erase[p=fc]; Erase[p=fv]

Bool(false) >< Select(result, tc, tv, fc, fv)
    => result--fv; Release[p=fc]; Erase[p=tc]; Erase[p=tv]
```

There is no branch selection or branch traversal in host code. An unknown
condition is a free wire facing Select's principal; it has no matching partner.
Supplying a Bool at that declared input is a host boundary action, not secretly
an interaction rule or inferred stack parameter.

The existing nullary Release / three-auxiliary Gate rule is unchanged:

```text
Release >< Gate(next, input, operation)
    => input--operation; Release[p=next]
Release >< Stop => empty
```

The branch selector is the sole holder of each branch control wire. This
linearity is a restriction of this candidate. Branch control chains end in
their own Stop; cancelling a branch must not cancel a common continuation.

## Branch wiring

True branch (finite case):

```text
tc -- Gate1.p
Gate1.next -- Gate2.p         Gate2.next -- Stop.p
Int(2) -- Gate1.input         Gate1.operation -- Mul.p
Int(3) -- Mul.rhs             Mul.out -- Fan.p
Fan.left -- Gate2.input       Gate2.operation -- Add.p
Fan.right -- Add.rhs          Add.out -- tv
```

The shared computation is exactly the one Mul node, not two copies. Fan waits
on its auxiliary output. For the divergent case substitute Tick for Int(2)
and Spin for Mul, omitting Int(3); keep the shared output and downstream Add.

False branch:

```text
fc -- Gate0.p                 Gate0.next -- Stop.p
Int(0) -- Gate0.input         Gate0.operation -- fv
```

The incoming Need's result port connects to Observe.p; Observe.result connects
to an external result boundary. No actual print or other host effect is used.

## Local cancellation rules (unreleased branches only)

These are Claude's control-side alternative C1, now explicitly under review as
part of this variant. Output-side erasure alone remains insufficient to cancel
an already active input-facing producer.

```text
Erase >< Gate(next, input, operation)
    => Erase[p=next]; Erase[p=input]; Erase[p=operation]
Erase >< Add(rhs,out) => Erase[p=rhs]; Erase[p=out]
Erase >< Mul(rhs,out) => Erase[p=rhs]; Erase[p=out]
Erase >< AddK(n)(out) => Erase[p=out]
Erase >< MulK(n)(out) => Erase[p=out]
Erase >< Fan(left,right) => Erase[p=left]; Erase[p=right]
Erase >< Spin(out) => Erase[p=out]
Erase >< Tick => empty
Erase >< Stop => empty
Erase >< Erase => empty
Erase >< Int(n) => empty
```

Cancellation reaches the operation's principal through the gate's operation
edge. It does not inspect or recursively delete the whole subnet. Each eraser
step counts against fuel. It is possible to stop midway and resume the same
valid residual. Allocation remains diagnostic and append-only: logically erased
nodes leave recorded slots, so zero live branch nodes does not mean zero heap.

## Hand traces and assertions for independent review

1. **Uninvoked:** wire the IfDormant principal and the result observation input
   to separate free boundaries, rather than allocating Need. No active pairs.
   An available Bool meets IfDormant's auxiliary condition port, not Select.
2. **Invoked, unknown:** Need/IfDormant is the only interaction. Select waits
   at the condition boundary. No gate opens; no arithmetic or Spin step occurs.
3. **False, finite or Spin:** the next interaction is Bool(false)/Select.
   Only Gate0 receives Release. Gate1 receives Erase, which erases both true
   operation stages locally. Observe gets 0; cleanup must reach quiescence.
   Require zero `mul-left`, `mul-done`, `add-left`, `add-done`, and `spin` steps.
4. **True, finite:** Bool(true)/Select releases Gate1; it releases Gate2.
   Mul consumes 2 and 3 once; Fan copies the resulting 6; Add consumes 6 twice
   and produces 12. False branch erases. Require one multiplication, one
   addition, one value fan interaction, and only the Answer agent surviving.
5. **True, Spin:** Gate1 exposes Tick/Spin. Gate2 may open but no value arrives
   at Fan. At the cap there is no Answer and a Spin pair remains. Activation
   happens once, while Spin interactions repeat; no duplicate producer exists.
6. **Late condition:** run case 2 to quiescence, attach a Bool at the explicit
   boundary, and repeat cases 3/4/5 without rebuilding the branches.
7. **Schedules and pause:** check oldest/youngest pairs, plus every short fuel
   cut followed by resume. False/Spin must terminate under either schedule;
   no active Spin exists for a scheduler to choose in that case.

Lead's provisional exact counts, to be checked rather than baked into the
implementation: invoked unknown = 1 interaction; false/finite = 16;
false/Spin = 15; true/finite = 15. The counts include all branch cleanup and
the final Observe interaction. False/finite's 11 true-branch cleanup steps are
two gate erasures, two literal erasures, Mul/Fan/Add erasures, Stop erasure,
and three Erase/Erase annihilations. These are ordinary counted interactions,
not a whole-subgraph deletion hidden behind one step.

Review questions: are all interfaces preserved? Can cancellation create an
unsupported wire internal to a redex in the small runner? Are gates sufficient
for dormancy for **these exact branches**, without implying arbitrary quoted
subnets are protected? Is a hidden host-level branch test or memo lookup being
smuggled into any step? Does one control owner merely move the unresolved
independent-consumer question outside this fixture? (It does; report that.)

## Implementation boundary

After rule review: a separate example using the small port runner, retaining the
original F fixtures/rules as the comparison. No CAS, source frontend, effects,
global reachability pruning, copied executable template, or production changes.
If the generic passive/active identity mechanism remains unspecified, leave it
unspecified instead of representing it as a silently privileged host memo table.

## Executable results

[`examples/selective_activation.rs`](examples/selective_activation.rs) supplies
the new agent kinds and exact rule table. Both examples share the reciprocal
port/rewiring/audit machinery in `examples/support/port_runner.rs`; neither calls
the March evaluator. The variant implements only the reviewed fixture rules,
not the optional nested-conditional or Fan/IfDormant extensions.

Run from `march6/` (the external timeout is an extra safety boundary):

```sh
timeout --kill-after=2s 30s cargo run --offline --example selective_activation
timeout --kill-after=2s 30s cargo run --offline --example selective_activation -- --trace
timeout --kill-after=2s 30s cargo test --offline --example selective_activation
```

Observed counts, including local erasure and observation:

| Case | Result | Interactions | Completed shared mul | Spin steps | Live agents at stop |
| --- | --- | --- | --- | --- | --- |
| Uninvoked, finite | quiescent, no answer | 0 | 0 | 0 | 14 |
| Invoked, unknown, finite | quiescent residual | 1 | 0 | 0 | 13 |
| False, finite | Answer(0), quiescent | 16 | 0 | 0 | 1 |
| False, Spin | Answer(0), quiescent | 15 | 0 | 0 | 1 |
| True, finite | Answer(12), quiescent | 15 | 1 | 0 | 1 |
| Late true, finite | Answer(12), quiescent | 15 total | 1 | 0 | 1 |
| True, Spin, low-ID order | Budget, no answer | 128 | 0 | 119 | 5 |
| True, Spin, high-ID order | Budget, no answer | 128 | 0 | 121 | 8 |

All finite/false/unknown counts above agree under both tested orders. True/finite
also has exactly one completed addition and one value-copy interaction. Only
the result Answer survives full cleanup. Dormant residuals intentionally retain
their agents. Dead allocation slots and trace history remain retained by the
diagnostic engine; live counts are not physical heap reclamation claims.

The two Spin counts differ because high-ID scheduling can starve other pending
interactions, including the second gate release. Neither is called quiescent.
Do not assert all setup/cleanup necessarily finishes before a selected Spin
starts running. There is exactly one live Spin and one Tick under both orders.
In this exact high-ID fixture the eight survivors are Observe, Tick, Spin, Fan,
Add, Gate2, Release, and its Stop: the false branch has already been erased.
The false/Spin result is safe under both orders because no Spin pair ever becomes
active there, not because either scheduler gives cancellation priority.

Nine variant tests pass in debug and release, alongside the original thirteen.
They include known and late-supplied conditions; all fuel cuts 0 through 17 and
resume for both choices, both producer kinds, and both schedules; rejecting a
129-step request; and deliberately reporting `StuckWithoutRule` for Fan meeting
IfDormant. Formatting and clippy checks pass. Test/CLI processes completed under
the external 30-second timeout; no nonterminating process is left running.

Finding: **this local conditional can leave an unselected producer inactive,
erase it without executing it, and share a selected producer's value.** This
does not establish a general lazy net, independently triggered shared instances,
or a passive-description memoization architecture. Computed conditions, nested
conditionals, invocation sharing, and post-activation cancellation remain open.

The next authorized rule sketch is `COMPUTED-CONDITIONS.md`, covering computed
conditions and nested invocation/erasure. Its new rules are reviewed separately;
the results above continue to describe the original ready-Bool fixture.

Priority update: those computed-condition rules have now passed independent
review, but implementation is on hold. `CROSS-BRANCH-SHARING.md` investigates
Thomas's question about preserving a producer when one of its uses is erased.
