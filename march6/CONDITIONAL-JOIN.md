# Conditional selection with a shared demanded producer

Status: implemented and locally verified; independent composition/code review
requested from Claude.
This combines the existing selector rules with `DEMAND-JOIN.md`, without a
new activation or sharing rule. No production evaluator changes or effects.

## Question and topology

```text
shared = 2 * 3
result = if condition then shared + 1 else shared + 2
```

Use one Join and one gated producer feeding one value Fan, exactly as in the
independent-consumer fixture. Each Fan output feeds a private LGate/AddK
consumer. Each LGate.demand feeds a different Join input. The branches have
no private Observe agents: their AddK outputs go directly to IfDormant.tv/fv.
IfDormant.tc/fc connect to the LGate control ports. Need.result connects to
one Observe for the whole expression.

Ports of IfDormant are [entry, condition, tc, tv, fc, fv]. Ports of Select
are [condition, result, tc, tv, fc, fv]. Only port zero is active. Need has
active entry and auxiliary result. The host connects Need only at declared
entry/result boundaries; condition and producer input may arrive later.

Reuse the earlier rules unchanged:

```text
Need(result) >< IfDormant(condition, tc, tv, fc, fv)
    => Select[p=condition; result, tc, tv, fc, fv]
Bool(true) >< Select(result, tc, tv, fc, fv)
    => result--tv; Release[tc]; Erase[fc]; Erase[fv]
Bool(false) >< Select(result, tc, tv, fc, fv)
    => result--fv; Release[fc]; Erase[tc]; Erase[tv]
```

The selected LGate forwards Release to its Join input; the discarded LGate
forwards Erase to the other input. The join rules handle either arrival order.
The discarded AddK output meets a separate eraser from Select, so those two
erasers annihilate. There is no discarded-result boundary in this fixture.
The eraser on the discarded Fan output waits until a value arrives; it must
not erase the producer through that auxiliary port.

## Predictions and checks

- Without invocation, even a known condition causes no computation.
- Invocation with an unknown condition creates Select, then stops. No branch
  request, producer activation, multiplication, or Spin occurs.
- True returns 7, false returns 8: one multiplication, one value fan, and one
  addition. Predicted cost is 18 interactions, with only one Answer remaining.
  This is the 16-step independent ask/drop case plus invocation/selection;
  erase-observe is replaced by erase-erase, and the dropped output disappears.
- Force the discarded branch's request erasure to reach Join before the chosen
  request: Join1 must preserve the producer, and the chosen request completes.
  Also force the reverse order to exercise Absorb explicitly.
- With unknown producer input, selection and cleanup settle before arithmetic;
  predicted cost is 12 interactions, six live agents. Supplying 2 finishes at
  18 total. Late condition/input arrival in either order gives the same result.
- If the common producer is Spin, either branch genuinely needs it: invocation
  with a known condition reaches the 128-interaction cap, with one Spin/Tick.
  Unknown condition or no invocation must leave it unstarted.
- Every short fuel cut must preserve audited wiring and resume; test both
  lowest-ID and highest-ID schedules. Neither schedule is a fairness proof.

This does not implement computed conditions, nested selectors, general lazy
instruction chains, recursive Join trees, or effects. It composes one selector
with one two-consumer Join. Each branch sends one request or one discard;
host invocation is single-use. No cancellation of already requested work is
introduced. Diagnostic allocations and trace history remain append-only.

## Executed results

All predicted finite counts hold under both schedules:

| Fixture | Interactions | Result | Live agents |
| --- | ---: | --- | ---: |
| Known condition, uninvoked, finite producer | 0 | Pending | 16 |
| Invoked, unknown condition, finite producer | 1 | Select waiting | 15 |
| True, finite producer | 18 | Answer(7); one multiplication and addition | 1 |
| False, finite producer | 18 | Answer(8); one multiplication and addition | 1 |
| Invoked, unknown condition and producer input | 1 | Select waiting | 14 |
| Condition arrives, producer input still unknown | 12 total | Producer preserved | 6 |
| Input 2 then arrives | 18 total | Answer(7) or Answer(8) | 1 |

The forced discard-first test reaches Join1 with the shared Gate and Mul still
present and no producer release. The later chosen request then completes. The
reverse test reaches Absorb and absorbs the discarded use. Both finish with
one Answer and no stranded erasers. No new semantic rule was needed to compose
these previously reviewed pieces; that is the principal result of this step.

With unknown condition, the Spin fixture stops after invocation (one step,
14 live agents), without starting Spin. With a known false condition it reaches
the 128-step cap: lowest-ID gives 116 Spin steps, six live agents, one active
pair; highest-ID gives 117 Spin steps, eight live agents, two active pairs.
The latter are Tick/Spin and Release/StopShared. Cleanup starvation remains a
scheduling limitation, not duplicate activation. Both alternatives need the
common producer, so a selected Spin must not be erased as unchosen work.

Nine composition tests cover both choices and schedules, late invocation,
late condition and input in either order, forced Join arrival orders, demanded
Spin, 240 fuel-cut/resumption combinations, and atomic rejection of a second
invocation. Together with the 36 earlier example tests, all 45 pass in debug
and release. Clippy with warnings denied passes for all three example binaries.
The shared port engine and the two original example drivers are unchanged.
No production/reference tests were rerun for this isolated change.

Use the bounded verification commands in `DEMAND-JOIN.md`; the `demand_join`
binary now includes these fixtures too. Add `-- --trace` to its `cargo run`
command to see each local rewrite and the remaining active pairs.
