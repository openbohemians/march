# C0: executable demand baseline and first protocol probes

This records the reference's current behavior, not a decision that every
detail must be March's final semantics. `tests/demand_contract.rs` pins the
boundaries before R1 changes representations. Existing guarded, staging,
reflection, seed-demand, capability, and collection tests remain applicable.

## Current demand boundaries

| Form | What it evaluates | What stays dormant |
| --- | --- | --- |
| Arithmetic/equality | Left then right; arithmetic checks operand types afterward | Nothing once both operands are needed; a child error aborts the remaining traversal |
| `If` | Condition, then selected branch | Both branches if condition unknown; the unselected branch otherwise |
| `Apply` | Function, then the instantiated body's demands | Unused argument expressions |
| `Dispatch` | Family and ordered guard demands, then selected body | Later clauses behind an unknown guard; rejected bodies; unused arguments |
| `Quote`/`Family` values | Closed-code/purity/capability validation | Body execution |
| Pair/record construction and projection | Constructor contents, even before projecting a field | No implicit lazy fields |
| `Intern` | Description normalization and validated reflection | Execution of the resulting held code |
| Seed `;` | Bound constant, or held code as a value | Saved outer stack and held code bodies |
| Successful seed EOF | Remaining stack observations | Held code bodies |
| Quota pause, graph image, collection | No extra evaluation | Pending work remains pending |

Unknown input is not a global stop instruction. `Add(?x, Mul(6,7))`
residualizes to `Add(?x,42)`. An unknown earlier guard does block later clauses,
but independent work outside that dispatch can proceed. Images preserve the
residual graph; they do not demand its eventual value.

### Errors are more precise than "leftmost bad operand"

The reference evaluates children before the parent performs its type check.
`Add(true, Mul(MAX,2))` therefore reports multiply overflow, not an add type
error. A child that itself errors stops evaluation of later siblings.
`Add(add-overflow, multiply-overflow)` reports the left add overflow.

A provisional error around unknowns is not a permanent memoized scalar:
`Add(Mul(?x,2), true)` reports an add type error with no binding, but multiply
overflow when evaluated with `x = MAX`. The existing reference does not cache
failed specializations. Probe epochs cache values across compatible bindings;
they cache unknown/error observations only within the current epoch. This
distinction must survive any proposed Poison/error-value protocol.

## R1 representation evidence, not a blanket strictness change

Projecting the good field from `Pair(7, overflow)` still overflows today. The
same holds for records. Therefore ordinary pairs cannot simply become a
multi-result calling convention while preserving independent result demand.

The additive witness packs two closed zero-argument quotations into a strict
pair. Construction, collection, and image replay leave both bodies dormant;
projecting and applying the good quotation returns 7; applying the other fails.
No new nucleus operation or evaluation policy is needed for this limited case.

This does NOT yet supply suspensions over arbitrary runtime inputs, multi-output
word compilation, or general lazy data. The next representation experiment
must address those explicitly. Keep strict metadata plus explicit suspensions
and a broader demand-mode redesign as alternatives until their interfaces and
costs are understood. Do not silently change Pair/Record, reflection, equality,
or old image meaning to make an example pass.

Two further witnesses test an output-template alternative: a word can hold
several closed output quotations with the same explicit input interface. Select
the desired code and then apply it, rather than packing applied expressions
into a strict pair. An output that ignores an overflowing input returns its
constant; selecting the identity output demands the overflow. Two different
output templates that use the same instantiated recursive call share that
call within a reference reduction: returning sum(3) and sum(3)+1 instantiates
the four recursive clauses once, not twice. This gives R1 an executable
candidate without changing constructors. General word metadata, zero/multiple
output seed compilation, and cross-epoch sharing policy still need design.

## N0 reference witnesses and remaining work

The reference fixtures execute a shared square quotation twice (81 from 3),
and pass it as an explicit code argument to a closed apply-twice quotation
(also 81). A third fixture constructs Church `two` as closed code that uses
`Intern` to explicitly build `x -> f(f(x))` from its supplied closed code `f`.
Applying `two` to itself, then to increment, produces a closed unary quotation
that increments four times. After collecting every other root and reloading its
image, applying that returned code to zero yields 4. No implicit lexical
environment is needed: held code dependencies are explicit graph edges.

These establish executable higher-order and partial-application witnesses in
the reference. The construction uses reflection; it does not prove that a
scalar net probe supports runtime code construction. Corresponding concrete
net encodings and fresh wiring still have to pass N0; this is its control, not
an N0 backend success.

The dynamically selected first-consumer example in the initial design brief
also has an arithmetic correction: for
`h(c,x) = Add(If(c, Add(x,1), Mul(x,2)), x)` and `x=42`, the two answers are
85 and **126**, not 85 and 84. Executable fixtures are the comparison authority.

## Isolated scalar protocol probes

`src/demand.rs` supplies shared input and observation types for experiments.
It accepts only integer/Boolean values, holes, addition, multiplication, and
`If`; it rejects all other nodes, including unsupported dormant branches.
There is no silent fallback to the reference reducer. Program data describes
an immutable scalar DAG; machines maintain their own demand state.

These are protocol transition machines, deliberately separate from the actual
principal-port backend in `inet.rs`. A sequential continuation driving a memo
table is NOT evidence of a pure interaction-net encoding, confluence, dynamic
port correctness, full token conservation, or efficient parallel execution.
The first probes isolate semantic and accounting mistakes before investing in
the full port/rule implementation. They do not replace the N0/N1 gates.

Both candidate routes must eventually face the same differential fixtures.
The A probe models direct requests to shared memo state. Claude has accepted
the B assignment to model requests routed through a fan tree. Neither is selected as
the winning design. Results are reported only for implementations actually
available and tested, not for a queued assignment or a hand-worked trace.

Common checks include demanded versus discarded failure, dynamic first use,
repeated/nested sharing, unknown/static-island reduction, epoch-local error
caching, compatible incremental bindings, and bounded transition work. Partial
observations report `Unknown`; that is not canonical residual serialization.
Bindings are monotone; conflicts must fail before mutating the machine.
A budget failure may retain completed values, but retry starts a fresh epoch,
not a persisted mid-transition continuation.

Sharing counts are cumulative per source node; ground values must not repeat
evaluation across epochs. Unknown/error nodes may retry in a later epoch.
Execution/memo storage and routing costs must be counted; persistent templates
are reported separately. Any probe that retains memo cells until machine drop
must say so, even when it releases use-site edges or continuations. Releasing
those edges is not a demonstration of complete last-consumer reclamation.
