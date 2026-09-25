# Syntax-neutral reflection

Reference implementation only: these reflection operations have not been
ported into `march-fast`. See the [reference index](README.md). Source paths
and commands are relative to `march6/`.

Status: B0a implemented with independent adversarial tests.  This is one nucleus
mechanism needed by the bootstrap gate; it is not yet the outer interpreter or
the seed image.

## Boundary

`Intern(description)` is a strict reducer operation.  Its input is an ordinary
March value and its result is an already existing or newly content-interned
`Quote` or `Family` value.  The nucleus knows the semantic graph vocabulary,
but no source tokens, word names, delimiters, compiler modes, or dictionary
policy.  In particular, it has no case for `:`, `;`, `to`, or `end`.

The description is normalized by the ordinary staged reducer first.  If any
part remains unknown, `Intern` residualizes with that description and creates
no reflected nodes.  A ground description is decoded by an iterative,
budgeted work list.  Conversion preserves shared description subgraphs with a
CID memo.  The resulting root then passes the same closed-code, parameter,
recursion, capability, and guard-purity validator used for hand-built code.
Only a validated quotation or non-empty family may cross the boundary.

Failed decoding or validation removes every node inserted by that reflection
attempt.  Existing CAS nodes and normalized description values remain, as they
would after any ordinary reduction.

This rollback is local to decoding and code validation; a later failure in
the enclosing reduction does not undo a successful `Intern`.  The existing
closure/purity validator is iterative and cached but does not yet charge its
own traversal visits.  The decoder's charged work is consequently an execution
limit, not a complete host CPU or memory quota.

This boundary is intentionally one-way for B0.  A seed dictionary that needs
to extend a family should retain its construction description beside the
validated code value, rather than asking the nucleus to decompile code.  A
future format/inspection context may define the inverse, but bootstrap does
not depend on it.

## Description schema

Every description is a `Record` with a text field named `op`.  Fields are
exact: missing and additional fields are errors.  Variable-length collections
are ordinary `Pair(head, tail)` lists ending in `Unit`.

| `op` | Other fields | Target node |
| --- | --- | --- |
| `int` | `value: Int` | integer constant |
| `bool` | `value: Bool` | Boolean constant |
| `text` | `value: Text` | text constant |
| `unit` | none | unit constant |
| `embed` | `value: Quote or Family` | an existing held code value |
| `param` | `index: Int` | parameter |
| `add`, `multiply`, `equal` | `left`, `right` | arithmetic/equality |
| `if` | `condition`, `true`, `false` | conditional |
| `pair` | `first`, `second` | pair |
| `first`, `second` | `pair` | pair projection |
| `record` | `fields: List(Pair(Text, description))` | record |
| `get` | `record`, `field: Text` | static-field lookup |
| `put` | `record`, `field: Text`, `value` | static-field update |
| `quote` | `parameters: Int`, `body` | quotation |
| `apply` | `function`, `arguments: List(description)` | quotation application |
| `emit` | `token`, `message` | explicit effect transition |
| `family` | `parameters: Int`, `clauses: List(Pair(guard, body))` | ordered guarded family |
| `dispatch` | `family`, `arguments: List(description)` | family dispatch |
| `recur` | `arguments: List(description)` | lexical family recursion |
| `intern` | `description` | reflective construction in generated code |
| `next-token` | `text`, `position` | whitespace token step |
| `parse-int` | `text` | optional checked decimal conversion |
| `lookup` | `record`, `key` | dynamic text-key lookup |
| `put-key` | `record`, `key`, `value` | immutable dynamic text-key update |

`embed` accepts a graph edge to a code value already held by the description;
there is intentionally no text-, byte-, integer-, name-, or hash-to-CID
operation.  Knowledge of a content hash grants no authority.  Trace/effect
capabilities cannot be described as constants or embedded in reusable code;
they must enter through parameters.

Runtime-generated code may deliberately bake ordinary integers, Booleans,
text, unit values, and already-held code into a new closed definition.  This is
closure construction by partial evaluation, and it is how a seed `;` can seal
compiled words.  The same path cannot bake an effect capability into reusable
code.

March may later fetch code by CID from another image or machine, but that is a
separate authority-bearing operation: an explicitly supplied resolver/import
capability maps a CID to a value when available and authorized.  Its result can
then cross this boundary through `embed`.  Reflection itself remains a pure
function of the description graph it was handed.  On the same principle, a
dictionary or namespace is a grant of the values reachable through it; handing
code a smaller dictionary gives it less authority.

## Established invariants

- Describing `Quote(1, Mul(Param(0), Param(0)))` produces exactly the same CID
  as constructing that quotation directly.
- Independent tests cover 300 generated code round-trips and 400 mutated
  descriptions, plus budget sweeps and image reload comparisons.  These test
  code structure and closure; they do not establish typing or termination of
  the generated bodies.
- Record field insertion order is not semantic, while family clause and
  argument-list order are semantic.
- Unknown descriptions residualize across epochs; malformed ground
  descriptions are structured reduction errors rather than panics.
- Negative or oversized arities/indices, open or scope-invalid code, impure
  guards, captured effect capabilities, improper lists, unknown operations,
  and non-code roots are rejected.
- Deep descriptions use explicit work lists and the reducer work budget.
- The canonical image codec includes `Intern`, so suspended reflective work
  can be saved and resumed.  Adding that canonical node tag advances the
  prototype image envelope and image-CID domain to version 2; existing node
  CIDs remain stable under the additive node-tag scheme.

## What remains for B0

B0b now provides syntax-neutral text stepping and dynamic record-key operations
(see `READER.md`), so a graph can look up a token-derived dictionary key. Decisions
such as whether a token is a number, whether a word is immediate, what ends a
comment or string, and what compiler mode means belong in the seed image, not
in Rust. The hand-built B0c seed now implements a subset of `name : ... ;`,
an alternate FORTH spelling, and deterministic token-quota save/reload.
See `SEED.md` for evidence and remaining limitations; arbitrary source-defined
parsing handlers and streamed input are still unproved.
