# Contextual nets: rules before an evaluator

Status: candidate rules and hand traces independently reviewed by Claude;
standalone runner implemented, 2026-09-24 (local date). Claude independently
reviewed the original runner and reproduced all 13 tests and reported counts.
This investigation supersedes the immediate reference-matching agenda in
`NEXT-PHASE.md`, not the existing implementations or their recorded results.
March leads the rules/traces and integration; Claude independently challenges
the rules before a small standalone runner is built. No production changes.

## 1. Candidate model (one-page core)

Question: can explicit local activation, dataflow, and sharing support March's
construction/residual/execution model? Do not assume the answer is call-by-need,
or that a failure requires reproducing the CAS reference's implementation.

Notation: `A[p; a,b]` has one principal port `p` and auxiliary ports `a,b`.
Each port has one wire. `A >< B` denotes a principal-to-principal connection.
A rule replaces only that pair and preserves every external wire end. `u--v`
in a replacement means splice those external ends together, not inspect their
remote agents. A free input is an interface wire, not an error or a value.

| Agent | Principal role | Auxiliary roles |
| --- | --- | --- |
| `Int(n)` | faces consumer | none |
| `Add`, `Mul` | first operand | `rhs, out` |
| `AddK(n)`, `MulK(n)` | second operand | `out` |
| `Fan` | faces producer | `left, right` |
| `Erase` | faces producer | none |
| `Gate` | activation | `next, input, operation` |
| `Release` | gate to release | none |
| `Stop` | release to consume | none |
| `Observe` | faces producer | `result` |
| `Answer(n)` | external result | none |

Candidate scalar rules (letters name the pair's external wire ends):

```text
Release >< Gate(next, input, operation)
    => input--operation; Release.p--next
Release >< Stop => empty
Int(n) >< Add(rhs, out) => AddK(n)[p=rhs; out]
Int(m) >< AddK(n)(out) => Int(n+m)[p=out]
Int(n) >< Mul(rhs, out) => MulK(n)[p=rhs; out]
Int(m) >< MulK(n)(out) => Int(n*m)[p=out]
Int(n) >< Fan(left, right) => Int(n)[p=left], Int(n)[p=right]
Int(n) >< Erase => empty
Int(n) >< Observe(result) => Answer(n)[p=result]
```

`Release` is nullary: its replacement is a fresh Release on the gate's `next`
end, so no return
wire is silently lost. Integers and arithmetic here are mathematical; a runner
must explicitly bound its numeric domain rather than invent March overflow
semantics. A gate blocks the edge through its two data auxiliaries, not an
entire connected region. It may later be fused into per-operation activation
states; that is a candidate encoding, not a new semantic requirement.

This particular rule releases work when control arrives, independently of
output demand. The question remains whether a different local activation
protocol gives the desired selective behavior. Shared scalar production below
does not settle general quotation sharing, cancellation, or recursive data.

### Construction, quotations, and modes: open choices

An open arithmetic net can settle, keep a free input, and continue when a value
is connected later. That is a testable kernel of staging, **not yet** a reader,
stack-underflow inference algorithm, dictionary, quotation application rule,
or self-bootstrap. Source examples below describe intended uses of the net.

Neither a quotation's auxiliary boundary nor one input gate prevents unrelated
active pairs inside its body. For example, `Add >< Int(2)` can reduce inside
such a boundary without any external activation. We must choose and demonstrate
one of: a systematically inactive body encoding; staged per-agent activation;
or explicit mode-sensitive scheduling/regions, with their extra assumptions.
Do not silently pick one by storing an executable body as opaque host data.

Reduction modes and guarded-family context arguments are separate concepts.
Closed-quotation folding is also unresolved: unrestricted folding can encounter
divergence. This experiment does not promote the earlier `MODEL.md` context
mapping or the reference demand contract into mandatory mechanics.

In particular, keep three candidate activation policies distinct:

- **F: forward release**, the concrete rules below. Control enables work; ready
  inputs determine whether that enabled work can actually interact.
- **D: consumer-sensitive activation**, still to be specified. A consumer must
  be able to request a producer without automatically activating discarded
  outputs. No backward token or remembered return route is assumed necessary.
- **M: explicit mode eligibility**, also still to be specified. A mode permits
  some pair rules and holds others back. This directly represents the user's
  original mode intuition, but needs a precise eligibility rule, scope, and
  definition of switching modes. Do not replace it without argument with a
  context wire or claim it is already supplied by the F rules.

These are not necessarily mutually exclusive. The first traces characterize F;
they neither reject D/M nor decide that every March operation needs a separately
allocated gate. A whole-net mode policy must be reported as such, not hidden
inside an ostensibly context-free active-pair scanner.

## 2. Hand traces

### T1: discarded finite work

Wire `Int(2)` through a gate to `Add.p`; `Int(1)` to `Add.rhs`;
`Add.out` to `Erase.p`. Release enters the gate; its continuation is `Stop`.

```text
Release >< Gate => input spliced to Add.p, Release >< Stop
Int(2) >< Add => AddK(2) facing Int(1)
Int(1) >< AddK(2) => Int(3) facing Erase
Int(3) >< Erase => empty
Release >< Stop => empty              (may occur earlier)
```

Five interactions, including two arithmetic interactions. Erase cannot meet
Add through principal ports: the connection is to Add's auxiliary output.
Under these rules discard does not cancel the producer. With no Release,
the gated producer is stuck instead. Scheduling Stop earlier changes neither
fact. This is evidence about this candidate, not impossibility of local demand.

### T2: discarded nonterminating work

Use a diagnostic loop with `Spin[p; out]`, `Tick[p; ]` and the sole rule:

```text
Tick >< Spin(out) => Tick' >< Spin'(out)
```

Gate the Tick--Spin connection and connect `Spin.out` to Erase. Once released,
one internal step restores the same graph up to fresh agent identities. Erase
still faces an auxiliary output. Thus this graph has an infinite reduction
sequence and no full normal form under these rules. This is a direct structural
argument, not an inference from exhausted test fuel. Spin is a diagnostic rule,
not an implemented March recursive word.

An independent `Int(7) >< Observe` can nevertheless yield Answer(7). Record
both **observation obtained** and **whole net quiescent**. A scheduler that
always favors Spin can starve that observation. Fairness addresses starvation,
not cancellation or reclamation. Stopping on the first answer is a host policy,
not a local erasure proof and not sufficient for general effects or outputs.

### T3: shared pending sum, then square

```text
Int(2) -- Gate -- Add.p       Int(1) -- Add.rhs
Add.out -- Fan.p
Fan.left -- Mul.p            Fan.right -- Mul.rhs
Mul.out -- Observe.p         Observe.result -- result interface
Release -- Gate.p            Gate.next -- Stop.p
```

After release: Add consumes 2, AddK consumes 1, Int(3) meets Fan, two Int(3)
agents feed Mul and MulK, and Int(9) reaches Observe. Including Stop, there are
eight interactions. Exactly one completed addition and one multiplication.
Before addition completes, Fan.p faces Add.out (later AddK.out), both auxiliary;
there is no rule copying either producer. This trace demonstrates sharing of
this one scalar producer, not global equality-based sharing or general laziness.

### T4: first element of an unbounded sequence

First expose the failure of an **ungated recursive tail**, rather than pretend
an unspecified quotation automatically protects it. Diagnostic agents:
`From[p; out]`, `Cons[p; head,tail]`, `Head[p; out]`.

```text
Int(n) >< From(out)
    => Cons[p=out; head=h, tail=t], Int(n)[p=h],
       From'[p=s; out=t], Int(n+1)[p=s]
Head(out) >< Cons(head,tail) => out--head; Erase[p=tail]
```

Start `Int(0) >< From`, whose output faces Head; Head's output faces Observe.
The first From interaction constructs a head 0 and an already active tail
producer. Head selects 0 and Erase faces the tail producer's auxiliary output.
Answer(0) is obtainable in three interactions, but recursive tail production
continues. An optional `Erase >< Cons` rule propagates erasers to both fields;
it still cannot stop the From producer, and does not make this net normalize.

A gated tail without a release stays dormant, but then the rules lack a way
for a future tail consumer to release it. **That missing connection is the
design question.** Do not claim the first-element test passes lazily merely
because the runner stops after seeing 0. No complete lazy sequence encoding is
proposed here yet.

### T5: quad with a missing input

This fixture is the intended open residual **after** stack elaboration and any
construction gates have been discharged. We do not yet derive it from source.

```text
x -- Fan1.p
Fan1.left -- Mul1.p           Fan1.right -- Mul1.rhs
Mul1.out -- Fan2.p
Fan2.left -- Mul2.p           Fan2.right -- Mul2.rhs
Mul2.out -- result interface
```

With x free, there are no active pairs. In particular Fan2 faces Mul1's
auxiliary output, so the inner square remains one producer with two consumers.
Supply Int(3) at x and Observe at the result:

```text
Fan1 copies 3
Mul1 consumes 3; MulK(3) consumes 3 => 9
Fan2 copies 9
Mul2 consumes 9; MulK(9) consumes 9 => 81
Observe receives 81
```

Seven interactions; two completed multiplications, not three. This demonstrates
continuing an open shared arithmetic net. Automatic inference of one input
from `quad : sq sq ;`, code instantiation, recursive stack effects, and the
meaning of invoking the quoted `sq` remain separate unproved front-end steps.
For this arithmetic example, elaborated inline and factored forms should have
the same wiring, but general compositional equivalence is not established.

## 3. Additional safety traces before broader claims

- **Quotation interior:** put an active constant addition behind a Quote-like
  auxiliary boundary. A pair scanner still finds it. Specify body construction
  before claiming dormancy or compile-versus-run behavior.
- **Effects:** if Release opens Print A then Print B without waiting for A's
  data, B can become ready first. Merely visiting gates in order is insufficient
  to require completion in order. A candidate effect rule should pass the
  capability on completion; we must draw its data-waiting states and interfaces.
- **Both consumers disappear:** value-only Erase/Fan rules do not imply
  cancellation of an upstream pending producer. Retention must be measured,
  not described as automatic memory management.

## 4. Review and bounded runner plan

Claude: independently challenge external-wire preservation, the five traces,
hidden context inspection, quotation dormancy, and activation/cancellation.
Keep alternative mechanisms as alternatives. No need to force all five to pass
under one provisional rule set.

March: after review, implement an isolated rule runner for the agreed explicit
subset, not another reference evaluator. Requirements:

1. One declared principal port per agent; rewrites only on principal pairs.
2. Validate reciprocal wiring and preserve rule interfaces at each step.
3. Record pair kinds, rewrites, counts, observation, remaining active pairs,
   and fuel exhaustion separately. No CAS memo, global demand walk, or hidden
   execution of quotation templates.
4. Exercise at least two pair orders. Invariance claims only for the actual
   tested observations; counters are not a confluence proof.
5. Test sharing/staging successes **and** discarded-work/ungated-tail failures.
6. Keep the library, CLI, reference contract, and prior probes unchanged.

The go/no-go is understanding these rules, not a claim of full March semantics.
If broader activation rules remain unresolved, report them rather than invent
machinery to satisfy reference tests.

## 5. Independent review and executable findings

Claude independently checked the original port rules and all five hand traces:
interfaces and counts agreed; T1/T2/T4 are negative results for F as specified.
He also proposed separate control-side cancellation, completion-ordered effects,
a gated stream tail, and a constructor-code decoder. These are follow-up
alternatives, **not implemented** in this runner or accepted wholesale.

In particular, his proposed fan on a pending unit's control wire cannot receive
either consumer's activation through its auxiliary ports. This locates a gap in
that specific wiring; it is not a general impossibility result about sharing.
The shared producer in T3 already reduces once and provides both consumers the
value. No separate permission or ownership abstraction is needed for that.

Standalone source: [`examples/contextual_net.rs`](examples/contextual_net.rs).
It imports no March library modules. Run from `march6/`:

```sh
cargo run --offline --example contextual_net
cargo run --offline --example contextual_net -- --trace
cargo test --offline --example contextual_net
```

Each recorded step identifies the pair, rule, new agent IDs, and wire endpoints.
The driver scans principal pairs in oldest/youngest ID order; these are diagnostic
schedules, not an asserted execution policy. Every rule checks that all external
and new ports occur exactly once. Every rewrite validates reciprocal wiring.
More precisely, `Oldest`/`Youngest` select the lowest/highest lexicographic pair
`(min(agent ID), max(agent ID))`, not the actual time a pair became active.
The labels are shorthand; neither schedule is a general fairness guarantee.

The reviewed port mechanics are now shared in `examples/support/port_runner.rs`,
text-included by each isolated example with its own agent kinds and rule table.
The original rules/fixtures are unchanged. The shared driver rejects budgets
above 128 interactions per call and checks for ambiguous rule orientation;
equal-kind symmetric rules (such as Erase/Erase in the second example) are
allowed. `Budget` means a further rule was available at the cap; reaching
quiescence on the last permitted interaction still reports `Quiescent`.

Measured results (40-interaction cap on infinite fixtures):

| Fixture | Oldest-pair result | Youngest-pair result |
| --- | --- | --- |
| T1 discarded sum | quiescent after 5 steps; 1 addition | same |
| Unreleased sum | quiescent at 0 steps; 6 live agents retained | same |
| T3 shared sum squared | Answer(9), quiescent after 8 steps; 1 add, 1 mul | same |
| T2 discarded loop + independent 7 | Answer(7), still active at cap | no answer at cap; observation starved |
| T4 ungated stream head | Answer(0), still active at cap | no answer at cap; tail continually unfolded |
| T5 open quad | quiescent at 0 steps; 4 residual agents | same |
| T5 supplied with 3 | Answer(81), quiescent after 7 steps; 2 mul | same |

Thirteen tests cover those observations, fuel-cut/resume, external boundary
accounting, atomic unsupported/numeric-limit stops, and a connected upstream
addition that reduces despite a downstream unreleased gate. The last test
demonstrates the edge-local scope of gating; it is not a full quotation encoding.

Important limits:

- `Quiescent` means no principal-principal pairs. It does not mean all outputs
  are values: an open residual or unreleased gate can be quiescent. Principal
  pairs without a supplied rule are reported separately as `StuckWithoutRule`.
- Integers use checked i64. `NumericDomainLimit` leaves the net unchanged; it
  is a probe limit, not a March arithmetic exception. No divergence conclusion
  is drawn from fuel; the Spin argument is the explicit self-reproducing rule.
- The small splice implementation rejects auxiliary wires connecting ports
  within the same redex, before mutation (`UnsupportedInternalAuxWire`). Our
  fixtures do not need them. It is not yet a general-purpose net engine.
- Allocation is append-only for readable agent histories. Dead slots and the
  trace are retained; both live agents and issued slots are reported. T2 under
  oldest ordering holds 4 live agents after observation but allocates two fresh
  agents per loop step. This is **not** a memory-reclamation experiment. The
  surviving Answer agent is intentionally included in live counts.
- No parser, stack inference, Quote/Apply, general family dispatch, effect
  implementation, production-mode change, or bootstrap claim has been added.
- The tests establish these fixtures, not full language conformance, universal
  sharing, or a scheduler-independent observation guarantee for divergent nets.

## Sources and limits

[Sinot, Token-Passing Nets: Call-by-Need for Free (2005 preliminary)](https://lsv.ens-paris-saclay.fr/Publis/PAPERS/PDF/sinot-dcm05.pdf),
section 2, supplies the port/local-interface conventions used here. Its own
call-by-need construction uses multiple principal ports (introduction and
section 3); that is not evidence that every possible sharing encoding requires
them. The candidate rules and traces above are our proposals, not results
claimed by that paper.
