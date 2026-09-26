# Dependency identity, compatibility, and updates

Status: initial design note, 2026-09-26. This records the direction discussed
with Thomas, not an implemented dependency manager or a settled file format.

## Content addressing does not eliminate change management

A word's implementation CID identifies its canonical definition, including
the dependency CIDs it references. Rebinding a name does not change an existing
definition. Adopting different dependencies produces new definitions and CIDs;
it does not replace the contents behind an old CID.

CIDs remove the need for manually assigned version numbers to identify exact
code. They do not decide which changes to adopt, establish compatibility, or
identify a preferred successor. Human release labels and change descriptions
can still be useful.

## Two independent commitments

| Commitment | What it identifies | When it changes |
|---|---|---|
| Interface lock | The required typed contract | When that required contract changes |
| Implementation lock | The exact selected implementation | When different code is adopted |

An implementation update can preserve the interface lock while changing the
implementation lock. Neither identity substitutes for the other.

An interface-only lock intentionally leaves the provider choice open. An exact
resolved selection must also be retained when reproducibility is required.
Whether these commitments live in one artifact, separate artifacts, or an
immutable project image remains open; "lock" here describes their meaning,
not a chosen lockfile syntax.

The candidate interface identity follows March5: hash a canonical description
of exported names, input/output types, and effects, separate from implementation
identity. Polymorphic relationships and constraints belong to the typed
contract, not merely input/output counts. The exact March6 representation and
granularity are not yet decided.

## Rebuilding and upgrading are different operations

The intended default is:

**Rebuilding preserves dependency selections. Upgrading explicitly changes them.**

Re-lowering an existing canonical definition uses its existing CID references.
Compiling source text requires name resolution, so it also requires a selected
resolution environment. Resolving against whichever global dictionary happens
to be current would allow an unrelated dependency update to change the build.

A proposed solution is an immutable project namespace/resolution snapshot:
names resolve to the implementations selected for that project. Editing source
uses that environment unless the developer explicitly changes its selections.
An explicit update may itself be automated under an agreed policy; ordinary
rebuilds should not silently become updates.

For example, initially:

```text
foo -> implementation A, interface I
bar -> implementation B, referring to A
```

A new `foo` implementation C becomes available with the same interface I. The
project remains on A. A small edit to `bar` can create implementation D while
retaining its reference to A.

If the project later adopts C, its implementation selection changes, while its
requirement for interface I remains unchanged. Updating `bar` to reference C
creates another definition, and affected callers acquire new CIDs transitively.
Old definitions remain unchanged.

The update workflow should expose the affected definitions, check their
contracts, run the selected tests, and let the developer accept or reject the
new project snapshot. Rollback requires retaining the old snapshot and its
reachable code; immutable identities alone do not guarantee retention.

## What compatibility checking can guarantee

March's typed input/output signatures are central to checking replacements:
a replacement must satisfy the caller's input requirements, output expectations,
and relevant polymorphic constraints. Effects matter too: code requiring new
capabilities does not automatically satisfy the old contract.

An identical interface CID is a useful exact-contract check, assuming that
implementations are validated against the interface they claim. Compatible but
non-identical interfaces may also be possible; their substitutability rules
remain to be specified rather than inferred from unequal hashes.

Matching signatures do not establish identical behavior. Addition and
subtraction can have identical types. Tests, specifications, and developer
judgment remain necessary. Keeping two implementations available also does not
prove that their data representations or assumptions interoperate.

For context-oriented March, we must still determine which guard/context
requirements are part of a checked interface and which changes require other
validation. A matching interface must not silently authorize an update that
changes contextual behavior.

## Relationship to the store and current implementation

Persistent namespace snapshots fit the proposed immutable store direction.
However, code-resolution dependencies must remain distinguishable from runtime
state dependencies. This note does not require hashing the entire runtime store
into every word CID, choose a map implementation, or settle the unified-store
proposal.

The current fast engine already preserves compiled references when a dictionary
name is rebound. Recompiling textual source against a changed dictionary can
resolve a different implementation. Project dependency snapshots, interface
locks, compatibility-checked upgrades, and their user-facing workflow are not
implemented yet. Existing stack inference is not the full typed-contract system
described here.

## Open details

- Contract granularity: individual words, namespace interfaces, or both;
  whole-interface identity versus checking only the exports a caller requires.
- Canonical type identities, polymorphic constraints, effects, and contextual
  requirements; rules for compatible but non-identical contracts.
- How project snapshots capture source resolution and compiler/defining-word
  dependencies, alongside runtime code dependencies.
- Update scope and propagation: which dependents to rewrite, whether mixed old
  and new implementations are appropriate, and how to review the result.
- Interface-only provider selection policy, recording exact resolutions, and
  image/export formats for reproducible builds.
- Snapshot history and retention roots for dependable rollback.

## Earlier design lineage

- [March5 interface design](../../march5/docs/design/DESIGN-OVERVIEW.md):
  interface CIDs from names, types, and effects; interface checks plus test gates.
- [March5 namespaces](../../march5/docs/design/DESIGN-NAMESPACES.md):
  interface requirements, provider identities, and lockfile proposals.
- [March4 linking](../../march4/docs/design/LINKING.md): input/output signature
  checking during compilation and possible verification during linking.

Read March5's statement that an unchanged interface needs no lock update as
applying to the **interface commitment**. Adopting a different implementation
still changes an exact implementation commitment. This distinction preserves
the useful idea without conflating compatibility and reproducibility.

These are design inputs, not wholesale adoption of the earlier namespace,
database, or execution architecture.
