# March: a FORTH-inspired self-extending reducer

## Thesis

March is not primarily a functional language with FORTH-like notation. It is
an attempt to give FORTH's unusually thin boundary between language, compiler,
interpreter, and persistent environment a graph-reduction substrate:

> March is a self-extending, content-addressed graph-reduction system with a
> FORTH-like surface and image model. Compilation is reduction under one
> explicit context; execution is continued reduction under another.

Programs, definitions, contexts, residual programs, and immutable values are
graphs. A canonical persistent graph has a content identity. An executing
interaction net is the ephemeral, locally rewritable form of some part of that
graph. The persistent image names the roots from which the current language
and environment are reachable.

This changes the architectural question from “how should an external compiler
translate March?” to “what is the smallest reducer and seed image from which
March can construct, specialize, persist, and execute more March?”

## The FORTH correspondence

FORTH is relevant for more than its stack notation. Its words, dictionary,
interpreter, compiler state, and saved image form one extensible system. March
aims for a related unity, with graphs and reduction in place of threaded code.

| FORTH | March |
| --- | --- |
| dictionary | content-addressed definition graph |
| dictionary entry | guarded definition family |
| threaded code | connected reduction template |
| data stack | explicit compositional interface |
| `DUP` | fan or shared-reference structure |
| `DROP` | eraser or loss of reachability |
| interpreter/compiler state | explicit reduction context |
| immediate word | definition enabled in a construction context |
| saved system image | image root CID and its reachable graph |
| execute a word | instantiate and reduce its open net |

Surface stack effects remain valuable because they make composition compact
and make interfaces visible. They do not require the implementation to be a
traditional mutable stack machine. `dup`, `drop`, and `swap`, for example,
may lower to graph sharing, erasure, and rewiring.

## State-indexed reduction

Reducer rules are fixed. State affects reduction by being an explicit input,
not by changing a hidden mode in the reducer:

```text
artifact CID + context CID -> residual-or-result CID
```

Useful contexts may include reading, constructing, specializing, executing,
reflecting, and extending an image. These names do not have to be privileged
machine modes. They can be ordinary immutable values used when selecting a
definition.

Thus “compile time” and “run time” describe what is known during two stretches
of the same reduction relation. Construction reduces what the construction
context justifies and leaves an open residual. Execution supplies later facts
and continues reducing it.

## Demand and lazy evaluation

Graph reduction makes lazy evaluation a natural default, but laziness must
still be part of the language definition rather than an accidental property of
the scheduler. A March stack can contain references to expressions or open
graphs; placing one on the stack need not normalize it. Operations create
demand only for the forms they require:

- an arithmetic agent demands numeric forms from its operands;
- a constructor need not demand its fields;
- guarded dispatch demands only enough information to select a clause;
- an unselected clause is never instantiated, or is erased without being
  evaluated;
- an observation, serialization, or host operation demands its specified form.

This gives non-strict evaluation. Call-by-need additionally requires sharing:
one demanded computation must be reduced once and its result distributed,
rather than copied into independent computations. Interaction nets can express
that with fan structure and local rewrites, but the encoding matters. Merely
choosing arbitrary active pairs is not sufficient: eagerly reducing a detached
or ultimately erased divergent subnet would violate the expected termination
behavior of a lazy language.

March should therefore specify demand at open-net interfaces and distinguish
at least weak observable form from deep canonical form. A graph denoting an
unevaluated computation can have a stable CID without being confused with the
CID of its eventual value. Producing a canonical persistent value may demand
deeper normalization than passing an internal value to another word.

Effects remain explicit. A world or capability token supplies both demand and
ordering, so speculative graph work cannot silently perform, duplicate, or
discard an effect. If the program's observable result includes the final world
token, the corresponding effect chain is necessarily demanded.

Laziness and teleological analysis are independent. The ordinary reducer can
discover demand locally and reduce only the demanded frontier. Advance
analysis may fuse demand propagation, predict sharing, or reclaim an erased
subnet earlier, but lazy semantics must remain correct without it. The current
prototype implements residual unknown branches and value-level fans; general
lazy calls and fan/erase propagation through arbitrary subnets remain part of
the next interaction-net gate.

## Guarded definition families

A March word is naturally a family of condition-free definitions selected by
explicit arguments and context. Instead of placing control flow inside one
function:

```text
fib when n = 0  => 0
fib when n = 1  => 1
fib otherwise   => fib(n - 1) + fib(n - 2)
```

The family is an indexed set of open graph templates sharing a public
interface. Selection with known facts instantiates only the chosen template.
Selection without enough facts remains as a compact dispatch node; it must not
materialize every combination of possible paths.

This turns much conventional control flow into definition selection:

- pattern matching selects definitions;
- a state machine selects definitions indexed by its explicit state;
- a loop is a guarded family of recursive transitions;
- a surface conditional can elaborate to guarded continuations.

The first version should require pure guards, explicit inputs, exhaustiveness
or explicit failure, and either disjoint guards or deterministic precedence.
An `otherwise` clause is meaningful only relative to a sealed, versioned
family, so the complete dispatch set participates in the family's identity.
Open-world contextual composition can be investigated later.

Every clause exposes the same external value interface, while permitting a
more precise internal wire-fate summary. A clause may consume, pass through,
erase, duplicate, inspect, or retain each input. If selection is unresolved,
the caller uses the sound common envelope of those summaries. Specialization
can recover the selected clause's more precise account.

## Topology, wire fate, and “ownership”

Ownership is an implementation technique and a useful boundary discipline,
not the general semantic model of immutable March values. Rust must own its
allocations, but that does not imply that March programs should be organized
around Rust-like ownership.

The more native question is the **use topology** or **wire fate** of a value:

- how many consumers receive it;
- whether it is passed through, inspected, duplicated, or erased;
- which outputs remain connected to which inputs;
- when its last reachable use disappears;
- whether it escapes the current open-net interface.

In an interaction-net representation, linear wiring and explicit fan/erase
agents make much of this topology concrete. Local rewrites allocate and
reclaim cells as they transform the net. This actual topology and the
correctness of the rewrite rules are indispensable.

## Teleological topology analysis

**Teleological topology analysis** predicts some future use topology before
the corresponding reductions occur. It asks where wires are going, rather
than merely recording where they are now. It can derive, for example:

- a likely or proven last-use point;
- cell or slot reuse opportunities;
- fan/erase cancellation and rewrite fusion;
- bounded allocation for a closed reduction template;
- a favorable reduction schedule or region boundary;
- a compact resource summary for an open definition.

This analysis is an optimization, not a prerequisite for executing ordinary
pure March programs. The baseline reducer can follow the actual net, perform
local rewrites, and reclaim agents as erasure and interaction make them dead.
A sound analysis may be deliberately incomplete: when it cannot prove a
future topology, execution falls back to ordinary local allocation and
reclamation. Analysis results must not alter the observable normal form.

There are two important qualifications:

1. Linear capabilities for files, devices, foreign buffers, or an effect/world
   token can be a correctness rule, not merely an optimization. Such values
   belong at the impure boundary rather than defining the semantics of all
   immutable values.
2. Persistent CAS storage is a different lifetime domain from an ephemeral
   executing net. Local interaction can reclaim net cells, but image objects
   may still need reachable-root packing, compaction, or storage collection.

Accordingly, advance topology knowledge must never be required to avoid a
semantic leak. It may avoid allocation, shorten retention, eliminate runtime
bookkeeping, or establish resource bounds. Failure to predict merely loses
those benefits.

The existing “compile-time memory map” is best understood as one early,
straight-line form of this analysis. Its complete-trace assumptions are not a
language restriction. The research problem is whether useful compositional
summaries survive guarded dispatch, recursion, higher-order escape, and
sharing without expanding all paths.

## Three lifetime domains

March should keep three kinds of reclamation conceptually separate:

1. **Ephemeral reduction cells.** Interaction rules, fans, and erasers manage
   the live executing net locally. Teleological analysis may optimize this.
2. **Persistent content-addressed objects.** Image roots determine durable
   reachability. Packing or storage collection reclaims unreachable objects.
3. **External resources and effects.** Explicit linear/affine capabilities
   ensure that host resources are used and finalized lawfully.

Conflating these domains would either impose unnecessary linearity on ordinary
values or falsely suggest that local net reclamation collects the persistent
image and host resources as well.

## Bootstrap target

The self-hosting path should be observable in stages:

1. A small trusted host nucleus implements canonical graph storage, CID
   verification, primitive ports and agents, local interaction, image loading,
   and host capabilities.
2. A seed image defines structural words, guarded-family construction,
   quotations, parsing, and image extension.
3. March definitions construct and specialize further March definitions by
   reducing them under explicit contexts.
4. The system can rebuild a functionally equivalent image from the nucleus
   and declarative source, with deterministic identities for each fixed set of
   rules and inputs.

Rust is then the material of the initial nucleus, not the conceptual blueprint
of the language.

## First guarded-family experiment

The first control-flow slice now replaces an enumerated path with an ordered,
content-addressed guarded definition family.  An unresolved selection is one
compact `Dispatch`; clause bodies remain dormant and only the selected body is
instantiated.  Recursive calls are lexical placeholders resolved to the
selected family's CID, so the persistent CAS remains acyclic.  The residual
dispatch survives an image round trip and can continue reducing when facts
arrive in a later epoch.

This first slice deliberately imposes two safety restrictions.  Code values
must be closed over named holes: epoch context must cross the interface as an
explicit parameter.  Guards are syntactically pure, so selection cannot consume
an effect later needed by the body.  Clause order is semantic; an unknown
earlier guard prevents the reducer from skipping ahead to a later match.

The interaction-net backend can lower the subset whose first parameter drives
selection.  The family templates live in an immutable table rather than as
live agents; a `Call` demands the first argument, installs only the chosen
body, and uses fans and erasers for its actual parameter uses.  A lowering that
would make an otherwise undemanded first argument strict is rejected.  This is
evidence for demand-directed local selection, not yet a general encoding of
multi-argument guards, closures, or fan/operator commutation.

The next gate is therefore sharper: compile general guard demand into a local
decision chain, then test higher-order duplicated computations with nested,
fresh fan identities.  In parallel, connect clause-specific wire-fate summaries
to the resource planner.  Eight independent unknown decisions must remain
proportional to eight dispatches and shared templates, never 256 enumerated
paths.
