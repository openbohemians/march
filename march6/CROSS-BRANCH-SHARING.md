# Cross-branch sharing: erase a use, not its producer

Status: independently reviewed candidate rules and hand traces; unimplemented.
Retained as a narrower control for the independent-demand experiment below.

Priority update: Thomas authorized proceeding with Claude's two-active-port
Join for independent consumers. `DEMAND-JOIN.md` records its implementation in
a separate port-aware runner. The single-principal runner remains unchanged.
The restricted common-dependency model below must not be mistaken for that
Join design. `COMPUTED-CONDITIONS.md` remains reviewed but unimplemented.

## Question

```text
shared = 2 * 3
result = if condition then shared + 1 else shared + 2
```

There is one physical pending multiplication with a Fan distributing its result
to two private branch consumers. Discarding either branch must preserve the
producer needed by the other. Expect 7 for true, 8 for false; one multiplication
and one branch addition. A missing producer input lets us inspect the graph
after branch erasure but before the shared work can finish.

This is one execution instance. There is no immutable passive-code store here,
and no rule deletes a reusable definition. There is no reference-count table,
memo lookup, global reachability sweep, or host inspection of consumer demand.

## The activation obligation is explicit

The previous IfDormant/Select interface has control only for each private
branch. A producer used by both cannot simply be placed on both branch control
chains: it has one control port, and rejecting a branch must not erase it.

We will first show the old rules with this common control left at a declared
external boundary. Selecting and erasing a branch leaves the shared producer
intact but unable to run. This is an **open activation obligation**, not an
impossibility result or a valid closed program claimed to be broken.

The candidate adds one explicit common-dependency control port to the selector.
Its contract is restricted: **both alternatives require this producer's value**.
Once the condition is known, either choice therefore justifies activating the
producer. The selector emits one Release for that common dependency and one
for the selected branch. There is no arbitration between independent consumers.

This contract is supplied by the fixture wiring, not automatically inferred.
It does not solve cases where only one branch uses the producer, all uses may
disappear, or consumers in independent invocations trigger it at different times.
It is not a proposed privileged March source-language conditional.

## Exact rules

One principal per agent. Parenthesized names are auxiliaries in the listed order.

```text
IfShared(cond, sc, tc, tv, fc, fv)     # principal faces invoking Need
SelectShared(result, sc, tc, tv, fc, fv) # principal faces condition
```

Both have seven ports total. `sc` is the common producer's single control wire.

```text
Need(result) >< IfShared(cond, sc, tc, tv, fc, fv)
    => SelectShared[p=cond; result, sc, tc, tv, fc, fv]

Bool(true) >< SelectShared(result, sc, tc, tv, fc, fv)
    => result--tv; Release[sc]; Release[tc]; Erase[fc]; Erase[fv]

Bool(false) >< SelectShared(result, sc, tc, tv, fc, fv)
    => result--fv; Release[sc]; Release[fc]; Erase[tc]; Erase[tv]
```

All seven invocation interface ends, and all six selection interface ends,
occur exactly once. These are explicit rule choices, not a hidden operation
that discovers the common producer. All arithmetic, gate, fan, and erasure
rules remain those already reviewed in `SELECTIVE-ACTIVATION.md`.

## Port wiring

Shared producer (private to neither branch):

```text
sc -- Gshared.p                 Gshared.next -- StopShared.p
Int(2) -- Gshared.input          Gshared.operation -- Mul.p
Int(3) -- Mul.rhs                Mul.out -- Fan.p
```

True consumer:

```text
tc -- Gtrue.p                   Gtrue.next -- StopTrue.p
Fan.left -- Gtrue.input         Gtrue.operation -- AddK(1).p
AddK(1).out -- tv
```

False consumer:

```text
fc -- Gfalse.p                  Gfalse.next -- StopFalse.p
Fan.right -- Gfalse.input       Gfalse.operation -- AddK(2).p
AddK(2).out -- fv
```

The result port connects to the original observation. Need requests the root;
it does not start an independent graph traversal. An uninvoked root has a free
entry boundary. An unknown condition has a free condition boundary.

## Erasure stops at the shared-use boundary

For a true condition, erasure removes Gfalse, StopFalse, and AddK(2). The
discarded branch output erasers annihilate. The remaining input eraser faces
Fan.right, an **auxiliary**, so it cannot interact with Fan through that edge.
In particular it cannot apply `Erase >< Fan` on Fan's principal, nor reach Mul.

If Mul is still waiting for an input, both Mul and the selected consumer remain.
Once Mul produces 6, `Int(6) >< Fan` emits one value per output. The right value
meets the waiting Erase; the left reaches the selected AddK(1), producing 7.
False is symmetric and produces 8. If the producer finishes before branch
erasure, the unwanted value is still erased by the same local rules.

This safe ordering does **not** implement general early deletion of a dead use
from a fan: the eraser may remain until the producer completes. If all uses
disappear while the producer is dormant or divergent, these rules do not thereby
prove prompt reclamation. Retention is recorded, not presented as solved.

## Hand traces and checks

1. **Open control, old selector:** leave sc at a named boundary. Invoke the
   old IfDormant. Branch selection/erasure finishes but the result is pending:
   Mul exists, one discarded-use Erase faces a Fan auxiliary, no Mul interaction
   has happened. No host action silently fills this missing activation source.
2. **Uninvoked IfShared:** no pair interactions, even with a known Bool.
3. **Invoked, unknown condition:** one invocation interaction, then quiescent;
   no producer or branch gate released and neither branch erased.
4. **Known true/false:** Answer(7)/Answer(8), one Mul, one AddK, one value-copy;
   cleanup reaches only the surviving Answer. Both low/high pair-ID orders.
5. **Known choice, unknown shared input:** after available work settles, Mul,
   its rhs Int(3), Fan, the selected AddK, Observe, and the discarded-use Erase
   remain (six live agents). No multiplication has started. The eraser's actual
   neighbor must be Fan's discarded auxiliary, not just a matching node count.
   Supply Int(2) at the declared boundary and obtain the expected answer from
   this same net, without reinvoking or duplicating the producer.
6. **Late condition:** supply true or false to the same dormant residual;
   obtain the appropriate result with one invocation and one multiplication.
7. **Every fuel cut:** pause/resume through selection, private erasure, common
   activation, and value delivery. Validate reciprocal wiring at every step.

Predicted cost for either fully finite choice: 16 interactions: invocation and
selection (2), two releases and two Stops (4), Mul's two steps (2), Fan (1),
AddK (1), Observe (1), four discarded-branch erasure steps (4), and erasing
the unwanted copied value (1). With the producer input missing, 10 interactions
settle before the six remaining value-production/consumption steps.

All runs remain single-threaded and capped at 128 interactions per call, with
external 30-second command timeouts. This first cross-branch fixture needs no
divergent work; the missing input exposes the lifetime question without a loop.

## Review questions

Does branch erasure ever reach the common control or producer principal? Does
the common-control rule make a restricted assumption explicit rather than hide
an analysis? Are both schedules safe when the value arrives before/after the
discarded-use eraser? Is the six-node pending residual exactly as drawn? Does
any rule require a remote port read or new unsupported internal-auxiliary wire?

The checkpoint sought is narrow: one discarded branch does not destroy a
producer still required by the other. General passive-description identity,
independent-consumer activation, and all-uses-discarded reclamation stay open.
