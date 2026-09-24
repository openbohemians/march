# B0c: an executable seed compiler

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
reducer rule, reflection schema, or image format changed for B0c.

The reader, symbolic compiler, input-wire allocation, dictionary updates,
and punctuation behavior all execute as those graphs. They are reachable
from image roots and survive reload without running `Seed::build` again.
`seed::resume(store, runner, state, token_quota, work_budget)` constructs just
an invocation of a held runner, with explicit state and quota arguments.

The dictionary contains tagged cells. Integers push data; quotations are
called; handler families transform the explicit reader state. The initial
handlers include `:`, `;`, `(`, `)`, `quote`, `dup`, `drop`, `swap`, `+`, and
`*`. Arithmetic/shuffle handlers select construction or evaluation behavior
from context. The `binder` field identifies name-first binding delimiters for
lookahead; the handler families themselves decide which modes enable them.

## State and compilation

The immutable state carries source text, byte cursor, dictionary, a top-first
stack, mode, pending definition name, saved outer stacks, inferred input count,
current token, and error. There is no ambient compiler state or lexical capture.

In evaluation mode the stack contains tagged integer or held-code values.
In quotation mode it contains ordinary reflection descriptions. Shuffles
rearrange wires; arithmetic constructs description records. When a word needs
more inputs than the symbolic stack contains, a graph helper introduces
explicit parameter wires beneath it. Parameter zero is the first input pulled
from the caller's top of stack, followed by successively deeper inputs.
Closing a quotation uses `Intern` to validate and seal the resulting code.

For this slice quotations have one integer result and inferred input arity.
Calling an existing word during compilation emits an application description
holding its code, preserving static linkage. At evaluation time a graph helper
constructs a closed application using reflection and invokes it. Dynamic
argument-list construction therefore uses the existing nucleus; no special
host call dispatcher was added.

A definition evaluates its expression on a fresh stack. `;` requires one
value, binds it, and restores the outer stack. Parentheses start a separate
symbolic stack; they do not capture prior runtime stack cells. A quotation can
run inside another definition's construction expression:

```text
square : ( dup * ) ;
answer : 7 square ;
answer
```

This binds the integer `49`, not delayed code.

There is an important semantic seam in this seed: its evaluation stack is
strict, while quotation construction builds a demand-driven graph and erases
discarded symbolic work. Thus `( 9223372036854775807 1 + drop 0 )` produces a
quotation that returns `0`, while evaluating the body directly at top level
overflows before reaching `drop`. A regression preserves this distinction.
The current seed therefore violates the intended factoring invariant described
in `MODEL.md` for failure behavior. Aligning those demand
boundaries is a language-design question for the next slice, not evidence
that the two contexts currently have identical operational behavior.

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
- Tests run on a 256 KiB native thread stack.

Pausing uses a token quota over the **same complete source plus cursor**.
Lookahead can inspect the next token without consuming it. This does not yet
support appending chunks, pausing inside a token, or treating temporary input
exhaustion as a request for more bytes. A quota that stops at the last token
has not necessarily performed EOF validation; resume with additional quota to
check completion. Failed reader states remain stopped on subsequent resume.

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
16,277 / 30,197 / 58,012 charged steps for 16 / 32 / 64 calls in the current
regression. The corresponding retained images are 41,643 / 41,867 / 42,315
bytes, including source text. These are workload measurements, not resource
guarantees: the CAS retains intermediate nodes in memory, dictionaries are
flat, text is cloned, and charging is not a total host-resource quota.

Independent review measured roughly 100 retained CAS nodes per source token
on repeated square/call/drop workloads. The current store conflates persistent
content with transient reduction states: discarding stack cells does not
remove their nodes from this in-memory store. This is an architectural
limitation of the reference implementation, not evidence of efficient runtime
reclamation.

An explicit-root checkpoint baseline runs the 64-call fixture in 16-token
epochs, serializing and reloading only runner and state between epochs. It
produces the identical final state/image while reducing the store-node high
water mark from 19,766 to 1,962, with 337 nodes retained after the last reload.
Charged reduction steps rise from 58,012 to 63,797; serialization, parsing,
hashing, and peak bytes during reload are additional unmeasured costs.
This is a test/control strategy, not an automatic collector in the CLI or a
decision to use arenas. Applications must enumerate **all** live roots before
replacing a store. An incremental collector or distinct transient reduction
storage remains a separate design decision.
