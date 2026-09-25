# Computed conditions and nested activation

Status: rules independently reviewed by Claude and all eight predicted counts
confirmed. Implementation is on hold: `CROSS-BRANCH-SHARING.md` now has priority
at Thomas's request, to examine erasing a use without destroying shared work.
This extends the standalone selective-activation experiment, not March's
production semantics. No source-level conditional syntax is being decided.

## Questions

1. Can observing `if (2 + 3 == 5) then shared + shared else unused` activate
   the condition first, and then only the selected branch?
2. Can a nested conditional be used as a branch, so selecting it invokes it,
   but rejecting it erases its condition and branches without execution?

Use a bounded Tick/Spin diagnostic for unused work or an inner condition to
make accidental activation visible. Preserve the cap of 128 rewrites **per
run call**, external 30-second timeouts, and no background jobs. Test scripts
do not repeatedly resume a divergent net indefinitely. Every erasure is counted.

## New rules

Auxiliary ends are listed in parentheses; each agent has one principal port.
Existing IfDormant and Select rules remain unchanged, preserving prior traces.

```text
IfComputed(cc, cond, tc, tv, fc, fv)  # principal: invoking consumer
EqK(n)(out)                        # principal: integer being compared with n
Enter(code, out)                   # principal: branch control
```

IfComputed has seven ports total; EqK has two; Enter has three.

```text
Need(result) >< IfComputed(cc, cond, tc, tv, fc, fv)
    => Release[p=cc]; Select[p=cond; result, tc, tv, fc, fv]

Int(m) >< EqK(n)(out) => Bool(m == n)[p=out]

Release >< Enter(code, out) => Need[p=code; result=out]
```

The invocation's seven external ends are preserved: cc receives Release;
cond becomes Select's principal; result/tc/tv/fc/fv are its five auxiliaries.
Enter is a linear control-to-invocation adapter, not a host call or evaluator.
It has no continuation to discard: the nested result is delivered on `out`.

Additional erasure rules, each with one eraser on every auxiliary end:

```text
Erase >< IfComputed(cc,cond,tc,tv,fc,fv)
    => Erase[cc]; Erase[cond]; Erase[tc]; Erase[tv]; Erase[fc]; Erase[fv]
Erase >< IfDormant(cond,tc,tv,fc,fv)
    => Erase[cond]; Erase[tc]; Erase[tv]; Erase[fc]; Erase[fv]
Erase >< Enter(code,out) => Erase[code]; Erase[out]
Erase >< EqK(n)(out) => Erase[out]
Erase >< Bool(b) => empty
```

No new Erase/Select rule or post-invocation cancellation is claimed. No Fan
activation/copy rule for either suspended conditional is added. A shared control
owner, independent-consumer activation, passive code identity, and cross-branch
producer sharing remain separate questions.

## Computed-condition wiring

```text
IfComputed.cc -- Gc1.p
Gc1.next -- Gc2.p               Gc2.next -- Stop.p
Int(2) -- Gc1.input             Gc1.operation -- Add.p
Int(3) -- Add.rhs               Add.out -- Gc2.input
Gc2.operation -- EqK(5).p       EqK.out -- IfComputed.cond
```

Invocation releases Gc1. Its Release may reach Gc2 before Add finishes, but
EqK still has no integer to interact with. Select still has no Bool. Neither
branch control is released until the Bool/Select pair forms and fires.

An unknown numeric input replaces Int(2) with a free boundary. After invocation,
the gates may open but Add waits. Supplying 2 later must continue the same net,
not rebuild or reinvoke either branch. A divergent condition instead gates a
Tick/Spin producer whose output faces Select; no Bool is ever produced.

## Nested branch wiring

```text
outer.tc -- Enter.p
Enter.code -- inner.IfComputed.p
Enter.out -- outer.tv
```

For an inner plain IfDormant the wiring is identical. The inner conditional has
its own condition and branch-control chains, each with private Stops. Inner
outputs are not separately observed: the result routes to the outer consumer.

If outer chooses true, Release/Enter produces Need, which invokes the inner
conditional by the ordinary pair rule. If outer chooses false, Erase/Enter
produces one eraser facing the inner conditional and one facing the outer
discarded true-value eraser. Erase/IfComputed then tears down its condition and
both branches. No host recursion follows this graph; the erasers interact one
pair at a time. The fixture builder may construct this fixed graph in Rust,
but must not evaluate any condition or choose a runtime branch in Rust.

## Proposed trace checks

- Uninvoked computed conditional: no arithmetic, comparison, branch activation,
  or Spin; graph remains dormant under either pair order.
- `2+3==5`, shared finite true branch, Spin false branch: Answer(12), condition
  Add once, EqK once, shared Mul once, branch Add once, zero Spin, only Answer
  live after complete erasure. Both pair schedules must agree on this result.
- `2+4==5`, Spin true branch, zero false branch: Answer(0), condition Add once,
  EqK once, no true-branch Add/Mul/Spin, full cleanup.
- A late numeric input: no branch-choice interaction before the input arrives;
  then reproduce either finite result without a second invocation.
- Selected divergent condition: stop at cap, no branch-choice interaction and
  no answer. It is safe to run this because Spin is one bounded step at a time.
- Reject an inner conditional with a divergent condition and divergent branches:
  Answer(0), no inner invocation, no EqK or Spin, full cleanup.
- Reject an inner finite computed conditional: zero inner arithmetic/comparison.
- Select an inner computed conditional: both invocations and choices occur via
  rules, producing the finite result. Also test rejecting an inner IfDormant
  with a Bool so the separate Bool erasure rule is exercised.
- Pause/resume at every cut through the finite traces, including cleanup;
  audit every intermediate wiring. No invalid internal-auxiliary redex may be
  silently bypassed by the splice implementation.

Fixture definitions for checking costs: a shared finite branch is the existing
two-gate Mul/Fan/Add unit; a standalone Spin branch is one gate around Tick/Spin
with its own Stop; a zero branch is one gate around Int(0). A numeric condition
uses two gates around Add and EqK; a Spin condition uses one around Tick/Spin.
Provisional hand counts, including observation and complete cleanup:

| Fixture | Predicted interactions |
| --- | --- |
| Computed true, shared finite / Spin branches | 22 |
| Computed false, Spin / zero branches | 16 |
| Numeric input missing after invocation | 4, then quiescent residual |
| Outer false, inner Spin condition and two Spin branches | 23 |
| Outer false, inner computed true with shared / Spin branches | 33 |
| Outer false, inner ready Bool with two Spin branches | 19 |
| Outer true, inner computed true with shared / Spin branches | 29 |
| Outer true, inner computed false with Spin / zero branches | 23 |

These counts are predictions to check, not implementation shortcuts. For
example, rejecting the all-Spin inner conditional requires three five-step
unit erasures, erasing Enter and IfComputed, and annihilating the discarded
inner output erasers: 18 cleanup interactions plus five outer-result steps.

## Review boundary

This tests local activation composition for explicit acyclic control chains.
It does not establish general code invocation, a self-bootstrap, or whether an
arbitrary source fragment can be compiled into these interfaces. A producer
shared between branches still needs an account of which control owns activation
and how erasing one use preserves other uses. Do not silently add a global
registry, graph sweep, or general demand walk to make these traces succeed.
