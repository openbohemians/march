# B0c/B0d: an executable, demand-driven seed compiler

Reference implementation only: this describes the retained graph-defined seed,
not the host-side `march-fast` compiler. See the [reference index](README.md).
Source paths and commands are relative to `march6/`.

The working surface now runs. From `march6/`:

```sh
cargo run --offline -- eval 'square : ( dup * ) ; 7 square'
cargo run --offline -- eval 'answer : 6 7 * ; square : ( dup * ) ; answer square'
cargo run --offline -- eval-forth ': square dup * ; 7 square'
```

The first and third commands print `stack (top first): [49]`. Both seeds
construct exactly `Quote(1, Mul(Param(0), Param(0)))`, with the same code CID.
Their whole-image CIDs differ: the images include different seed policy and
source text. Formatting is outside the generated quotation's identity.

## Where the compiler lives

`src/seed.rs` assembles a finite set of closed graphs, an initial dictionary,
and a guarded reader loop. This is the initial seed's construction recipe.
It never iterates over source tokens or interprets them in Rust. The host
starts ordinary reduction and presents the returned state. No nucleus node,
reducer rule, reflection schema, or image format changed for B0c or B0d.

The reader, symbolic compiler, input-wire allocation, dictionary updates,
and punctuation behavior all execute as those graphs. They are reachable
from image roots and survive reload without running `Seed::build` again.
`seed::resume(store, runner, state, token_quota, work_budget)` constructs just
an invocation of a held runner, with explicit state and quota arguments.

The dictionary contains tagged cells. Integers push data; quotations are
called; handler families transform the explicit reader state. The initial
handlers include `:`, `;`, `(`, `)`, `quote`, `dup`, `drop`, `swap`, `+`, and
`*`. Arithmetic/shuffle handlers build expression graphs in both contexts;
only compilation may infer missing input wires. The `binder` field identifies
name-first binding delimiters for lookahead; the handler families themselves
decide which modes enable them.

## State and compilation

The immutable state carries source text, byte cursor, dictionary, a top-first
stack, mode, pending definition name, saved outer stacks, inferred input count,
current token, and error. There is no ambient compiler state or lexical capture.

Both modes use tagged expression cells holding ordinary reflection descriptions.
Evaluation can also hold observed integers and dormant code/handler values.
Shuffles rearrange wires; arithmetic constructs description records. When a word needs
more inputs than the symbolic stack contains, a graph helper introduces
explicit parameter wires beneath it. Parameter zero is the first input pulled
from the caller's top of stack, followed by successively deeper inputs.
Closing a quotation uses `Intern` to validate and seal the resulting code.

For this slice quotations have one integer result and inferred input arity.
Calling an existing word in either mode emits an application description
holding its code, preserving static linkage. At observation time a graph helper
constructs a closed expression using reflection and invokes it. Dynamic
argument-list construction therefore uses the existing nucleus; no special
host call dispatcher was added.

A definition builds its expression on a fresh stack. `;` requires one
value, observes it, binds it, and restores the outer stack. Parentheses start a
separate symbolic stack; they do not capture prior runtime stack cells. A quotation can
run inside another definition's construction expression:

```text
square : ( dup * ) ;
answer : 7 square ;
answer
```

This binds the integer `49`, not delayed code.

## Demand boundaries (B0d)

Arithmetic and word calls now build pending expressions in both evaluation
and quotation contexts. `9223372036854775807 1 + drop 0` returns `0` both inline
and extracted into a word. Dropping a pending call or an unused argument does
not execute it. Demanding the overflowing result still reports `IntegerOverflow`.

There are two observation boundaries in this seed:

- `;` observes the single definition value. An integer constant is computed
  now; a code/handler value remains dormant. The saved outer stack is untouched.
- Successful EOF observes all remaining stack cells, top first. Code and
  handler cells remain dormant here too.

A token-quota pause, image snapshot/reload, or failed/incomplete reader does
not observe pending expressions. Structural errors (underflow, unknown words,
unsupported cell kinds, malformed definitions) remain immediate. Quotation
construction still validates closed code even if the quotation will be dropped.
This is demand alignment for well-formed integer, single-result fragments with
the same explicit stack interface, not a claim about arbitrary malformed source
or moving code across an observation boundary.

Shared pending cells keep the same graph identity. The reference reducer memo
shares their demand within a reduction run; it is not persisted between epochs.
After reloading a pending 64-addition graph, observing one versus sixteen
copies takes 923 versus 1,508 charged steps in the regression. This is a sharing
witness, not a general complexity bound. No new source effects or surface
guards are introduced by this change.

## Source-level parsing aliases

This is a passing self-extension witness:

```text
end : quote ; ;
begin : quote : ;
square begin ( dup * ) end
7 square
```

`quote` retrieves the next dictionary entry without invoking it. The source
binds held parsing handlers under new names and immediately uses them. The
lookahead policy recognizes `begin` through dictionary metadata; it does not
require a new Rust delimiter case.

This establishes **alias-level** parsing extension. It does not yet establish
source-defined arbitrary state transformers, new token grammars, or compiling
the entire seed assembler from source. Those require a richer surface for
records, code descriptions, and handler composition.

## Tested boundaries

- Exact square code identity under both seed syntaxes, and execution to `49`.
- Constants, compiled word composition, top-first parameter wiring, code
  aliases, construction-time calls, and static linkage across rebinding.
- Identical final states/images after save/reload at every raw-token boundary
  of the definition witness; parser aliases and pending reader modes also
  survive every tested token boundary.
- Source text can be unknown in one epoch and supplied after reloading its
  residual image. Seed/final identity is unaffected by unreachable store history.
- Reader syntax/type/underflow failures are explicit error states. Nucleus
  failures, such as integer overflow or work exhaustion, remain `ReduceError`s.
- Eleven B0d demand tests cover observation boundaries, dormant code, sharing,
  and 216 inline/factored value-or-error comparisons across both surfaces.
- Six independent factoring tests exercise repeated fragment extraction inside
  definitions, composed calls, inverse inlining, image replay, constant-binding
  boundaries, and sharing of identical pure calls.
- Tests run on a 256 KiB native thread stack.

Pausing uses a token quota over the **same complete source plus cursor**.
Lookahead can inspect the next token without consuming it. This does not yet
support appending chunks, pausing inside a token, or treating temporary input
exhaustion as a request for more bytes. A quota that stops at the last token
has not performed EOF validation or final observation; resume with additional
quota to check completion and observe results. Failed reader states remain
stopped on subsequent resume.

## Deliberate limits

- Whitespace-delimited tokens; no comments, strings, arrays, signatures,
  surface guards, recursive source definitions, or implicit captures.
- Nested quotations, zero/multiple-result quotations, and non-integer callable
  inputs/results are outside this seed. `quote` retrieval is evaluation-only.
- Ordinary bound values may be redefined. Existing handler words take
  precedence over name-first binding lookahead, so handler spellings are
  reserved in that seed. This precedence is provisional image policy.
  A redefinition attempt may therefore report an ordinary handler failure
  (for example, underflow for `dup : 5 ;`) rather than a reserved-name error.
  `quote :` must retain its next-token-reading meaning, so improving this
  diagnostic needs a considered parsing-precedence rule.
- Integer literals precede dictionary lookup and cannot be definition names
  in either seed. Keys are exact UTF-8, without Unicode normalization. See
  `SYNTAX.md` for the current numeric grammar boundary.
- Input inference stops at 65,535 parameters and reports a reader error
  before attempting an unrepresentable reflected quotation.
- The FORTH seed demonstrates equivalent quotation definitions; it does not
  promise identical expression syntax or every name-first convenience.
- Image loading validates graph encoding/references, not a hostile compiler
  state schema. The seed API expects its own record/cell layout; malformed
  supplied state may produce ordinary reducer errors.
- This is the CAS reference reducer. It does not expand the INet backend.

The scaling gate remains open. Repeated calls with a fixed dictionary use
15,887 / 29,375 / 56,326 charged steps for 16 / 32 / 64 call/drop pairs in the
current regression (the discarded arithmetic is not demanded). The corresponding
retained images are 42,636 / 42,860 / 43,308 bytes, including source text.
These are workload measurements, not resource
guarantees: uncollected reduction retains intermediate nodes in memory,
dictionaries are flat, text is cloned, and charging is not a total host-resource quota.

Independent review measured roughly 100 retained CAS nodes per source token
on repeated square/call/drop workloads without collection. Persistent content
and transient reduction states still share one store: discarding stack cells
does not itself remove their nodes. Explicit-root collection now reclaims
unreachable history between reductions; it is not immediate last-use freeing.

An explicit-root checkpoint baseline runs the 64-call fixture in 16-token
epochs, serializing and reloading only runner and state between epochs. It
produces the identical final state/image while reducing the store-node high
water mark from 19,400 to 1,936, with 348 nodes retained after the last reload.
Charged reduction steps rise from 56,326 to 62,074; serialization, parsing,
hashing, and peak bytes during reload are additional unmeasured costs.
The checkpoint/reload path remains a test/control strategy. The CLI now uses
in-memory `Store::collect` after each 64-token batch, without image codec work.
The corresponding 16-token collection fixture peaks at 1,936 nodes and ends
with 348, preserving the final image. Applications must enumerate **all** live
roots; no arena or transient-storage architecture is implied. See
`COLLECTION.md` for root obligations, metrics, and remaining limitations.
