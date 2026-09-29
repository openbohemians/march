# FORTH-style self-extension: stream nucleus

Source paths and commands are relative to `march6/`.

Thomas's correction: March must not start with a compiler-owned lexer and
preclassified syntax. Words pull input from a stream and decide what to do with
it. His deliberate difference from traditional FORTH is **numbers before
dictionary lookup**.

The earlier token-view/action-plan extension prototype has been replaced, not
retained as another active frontend. The interpreter now has a tested rebuild
fixed point, but this is still not a complete self-hosted language.

## What actually runs

`src/fast/stream-seed.march` contains ordinary March words implementing:

1. Read the next whitespace-delimited word.
2. Stop at EOF, checking that definitions are complete.
3. Try the word as a number.
4. If numeric, place that literal in the current pending expression/definition.
5. Otherwise, if it starts with `"`, consume/decode a text literal; otherwise
   look it up in the ordinary dictionary.
6. Execute a compiler word, or compile/compose the referenced ordinary word
   according to the current mode and the entry's immediate flag.
7. Repeat.

The branches and loop are March families/calls, executed by the existing lazy
`Executor`. There is no second compiler-word evaluator and no native
read/classify/dispatch loop. Rust implements the small operations the seed calls.

The numeric subset is currently signed decimal i64, including leading `+`
and `-`. A syntactically decimal integer outside i64 reports overflow before
lookup; it does not fall through to a dictionary word with the same spelling.
A lone `+` or `-` is not numeric and is looked up normally. Other numeric
formats remain future work.

A dictionary binding named `42` is legal, but ordinary input `42` always
produces the number. An explicitly input-consuming word such as `'` can look
up that spelling deliberately:

```forth
: 42 999 ;
42 ' 42 call
-- results: 42, 999
```

## Words consume their own input

The native `stream.word` operation only skips whitespace and returns the next
word in a new immutable compiler state. It does not split punctuation, strip
comments, recognize brackets, or classify numbers.

The initial ordinary dictionary includes these compilation words:

- `:`: March code `stream.word stream.begin` reads the name and opens a builder.
- `;`: closes that builder and installs its canonical word.
- `immediate`: marks the most recently completed definition in this source
  session as a compiler word.
- `--`: consumes raw input through newline; legacy `\` is a dictionary alias.
- `(`: March code invokes the delimiter reader for `)`. This comment ends at
  the first `)`; it is not the old reader's nested-comment grammar.
- `'` / `quote`, `ctx`, `recur`, and `apply`: read their own following words.
- `[` / `]`: open/close a closed quotation builder, retaining March's provisional
  quotation spelling rather than traditional FORTH's bracket meanings.
- `family`: March code reads its name/counts and guard/body names through `;`.

These spellings are dictionary entries, not reserved reader cases. Runtime
primitives such as `dup` and `+` are dictionary words too. Redefinition changes
subsequent lookup; existing compiled references retain their old code identity.

Comparison bindings are `eq?`, `lt?`, `gt?`, `gte?`, and `lte?`, without default
symbolic aliases. The first two retain the semantic primitive identifiers `=`
and `<` in canonical definitions; this dictionary rename does not alter their
CIDs. The other three are ordinary compositions installed by the seed builder
(see FAST-SPIKE.md), and can be rebuilt identically in March.

Whitespace separates words: `: square dup * ;` works; `:square`, `*;`, and
`7[` are each distinct words, not punctuation split by a lexer. Conversely,
a comment or custom delimiter-reading word can consume arbitrary raw text that
would be invalid code. Nothing pre-parses that text before the word runs.

## Same-session defining word

`examples/fast/compiler.march` contains:

```forth
: define stream.word stream.begin ; immediate
: answer: stream.word stream.begin
    20 22 + stream.emit-literal stream.end ; immediate

define square dup * ;
answer: answer
: 42 999 ;
7 square answer 42
```

`define` behaves as another colon defining word, without syntax registration.
`answer:` reads the next name itself, begins its definition, performs ordinary
March arithmetic during compilation, emits 42, and closes the definition.
The resulting word has the same CID as `: answer 42 ;` in this stream compiler
and executes with zero runtime arithmetic operations.

```sh
cargo run --offline --release --bin march-fast -- examples/fast/compiler.march
# [Int(49), Int(42), Int(42)]
```

## Native boundary and remaining bootstrap work

Text recognition is a March family (`seed.text-or-word`) after number
recognition. `stream.text?` tests the current word's leading quote;
`stream.emit-text` consumes the full literal from the raw input, validates its
terminator/escapes, interns the decoded UTF-8, and emits the literal. WORD itself
remains whitespace-only. Text uses conventional `"hello world"` and `""`, with
`\"`, `\\`, `\n`, `\r`, and `\t` escapes. Unknown escapes and unterminated
strings are errors. A closing quote must be followed by whitespace or EOF in
the stream syntax. Newlines inside a literal are preserved. No normalization
or interpolation is performed.

`tuple N` and `untuple N` are also input-consuming March words; N is a fixed
compile-time stack contract (0–4096), not a demanded runtime count. These emit
canonical definition items, lowered through the same engine on image reload.
`stream.emit-literal` now accepts text, but rejects runtime tuples and compiler
state: saving code must not silently demand a live heap. A word that *constructs*
a tuple is ordinary serializable code. Arrays/auto-classifying parentheses are
not implemented; `(` still denotes the existing comment word for now.

Native code still supplies:

- Stream cursor/word/delimiter operations and numeric conversion.
- Dictionary lookup, immutable compiler-state storage, and binding/flag updates.
- Definition-sequence builders, derived graph lowering and stack-interface
  inference, quotation/family construction, and canonical encoding.
- The ordinary runtime primitive operations and execution engine.

Compiler state is explicit and immutable: source/cursor, current word,
dictionary version, builders, and mode are carried by a typed, session-local
state reference. An operation returns a new state; it cannot mutate an earlier
one. State references are not integers, code identities, or serializable values.
Compiler words are ordinary 1 -> 1 March words receiving/returning that state.
Helpers can perform ordinary arithmetic, call other words, and use contextual
families. The execution context supplies `compiler=true` and the initial
`compiling` mode for that compiler-word invocation; state predicates observe
subsequent mode changes explicitly.

This is **not yet one unified traditional FORTH compile-time/runtime data
stack**. Compiler words have the explicit state interface. Ordinary top-level
work is composed lazily into pending code and demanded when the returned entry
is executed; unused ordinary work remains lazy. General compile-time stack
manipulation, richer reflection/data, and full contextual mode selection still
need design/work. Immediate flags are a first dictionary-level mechanism, not
a claim that the complete contextual compiler model is finished.

The one-time `stream::seed()` construction uses the retained host reader in
composition mode to assemble the initial March seed. Native primitive wrappers
are built directly. After that,
`stream::compile` runs the saved March interpreter; neither new source nor
new word bodies are passed to that reader. The host `source::compile` API remains
for tests/bootstrap scaffolding, not as the CLI frontend. It now emits exactly
the same composed-definition identities; there is no graph-identity source
compiler. Its convenience tokenization (adjacent punctuation, nested comments,
reserved primitive spellings) remains host scaffolding, not March's syntax.

`fast_definition::march_rebuilds_its_seed_across_images_with_stable_definitions`
uses the installed March interpreter to compile `stream-seed.march` for three
generations, saving/reloading between them. Successive images are byte-identical;
the final interpreter still compiles definitions and honors numbers-first.
The host test harness reinstalls the defining-word aliases and immediate flags.
This proves a fixed point for the current interpreter layer, **not** full
self-hosting: initial assembly, primitive semantics, lowering, and alias setup
still have native support. The next bootstrap milestone is to reduce those
responsibilities and resolve the compiler-state versus runtime-stack interface.

### Canonical definition data

Five compiler operations now expose definitions as ordinary immutable data.
They retain the explicit compiler-state interface:

| Operation | Stack contract | Purpose |
|---|---|---|
| `stream.cid-of` | state name-text → cid-text | Resolve a dictionary name. |
| `stream.describe` | state cid → tuple | Inspect that canonical definition. |
| `stream.construct` | state tuple → state | Validate/intern a definition in a new state, without binding or executing it. |
| `stream.last-cid` | state → cid-text | Read the result of the last construct in this state's history. |
| `stream.bind` | state name-text cid → state | Bind existing code under a name in a new state. |

CID inputs accept full 64-character lowercase hexadecimal text or an existing
quotation. Outputs always use CID text. Unknown references are rejected against
the supplied state's Program, not assumed to exist in the executor's Program.
`stream.bind` clears that name's immediate flag and makes it the last completed
name; `stream.immediate` can deliberately mark it again. A new compilation
session has no last constructed CID until `stream.construct` succeeds.

Reflection queries are lazy too: `dup "missing-word" stream.cid-of drop`
does not perform the unused lookup. A demanded lookup does fail. Marking a
word `immediate` checks its state → state signature at marking time, not first
invocation.

The descriptor schema below uses explanatory tuple notation, **not** March
source parentheses. A one-element record is a singleton tuple, not its field;
an empty items/clauses tuple is `unit`.

```text
definition = ("primitive", semantic-name) | ("kernel", kernel-name)
           | ("sequence", items-tuple) | ("family", clauses-tuple)
clause     = (guard-cid, body-cid)
item       = ("word", cid) | ("int", integer) | ("bool", boolean)
           | ("unit",) | ("text", text) | ("quote", cid)
           | ("context", key-text) | ("call",)
           | ("apply", inputs, outputs) | ("recur", inputs, outputs)
           | ("tuple", arity) | ("untuple", arity)
```

Item/clause order is preserved. Primitive names are semantic identifiers (`=`
and `<`, for example), not dictionary aliases (`eq?` and `lt?`). Kernel names
come from the stable kernel table, such as `stream.word`. Dictionary names,
immediate flags, lowered graph slots, and inferred arity are not definition data.

An executor can hold an older immutable Program than the compiler state it is
processing. CIDs prevent a newly constructed word from becoming an invalid
local quotation in that older executor. Constructed code lives in the new
state; bind it and compile subsequent uses, or invoke it through the existing
compiler-word execution mechanism when appropriate. There is no new implicit
execution or unified compile-time/runtime stack.

Construction demands the complete finite descriptor but no unrelated stack
values. The ordinary VM task stack drives field demand; it does not recursively
call public `force` and discard existing continuations. The schema bounds depth
to three edges from the root. Node and text-byte limits bound temporary transfer;
fuel bounds demand and descriptor traversal. Unknown tags, extra fields, bad
contracts, unknown CIDs, and compiler-state capabilities are rejected. The
normal `add_definition` validator/lowerer is shared with source and images.
Failed construction does not alter prior states; failed source compilation
does not publish partially constructed definitions, text, bindings, or flags.

For example, this constructs the same canonical word as `: answer 42 ;`:

```forth
: answer-data "sequence" "int" 42 tuple 2 tuple 1 tuple 2 ;
: make-answer answer-data stream.construct
    dup stream.last-cid "answer" swap stream.bind ; immediate
make-answer
answer
```

`examples/fast/reflection.march` additionally edits this descriptor using tuple
operations to construct `answer43`, leaving `answer` unchanged. The tests
round-trip every seed definition and every item kind, verify source-equivalent
CIDs, construct code across snapshot boundaries, reject hostile descriptors,
bound divergence, and save/reload/extend without host seed reassembly.
Native validation, canonical encoding, lowering, and primitive semantics remain;
this is a bootstrap step, not full self-hosting.

## Definition identity versus execution

The canonical definition of `: square dup * ;` is the ordered sequence
`[word CID(dup), word CID(*)]`, not its inferred register graph. The sequence
also supports tagged literals (including quotation CIDs), context-key reads,
static quotation application, and symbolic recursion/dynamic-application
contracts. `apply N M` and `recur N M` retain their explicit N/M contracts;
ordinary inferred input/output counts and argument wiring are excluded.
Ordered families hash guard/body CIDs; their arity is derived from the clauses.
No parametric type-signature representation has been added yet.

Quotation equality is definition equality: the same ordered words, literals,
context keys, and explicit contracts. Shuffles remain in identity even when the
derived graph erases them. Static `call` derives its target from the preceding
quotation value. `recur` is contextually bound to the active family or standalone
word by the invoking frame; that target is not embedded in the body definition.

Runtime primitive leaves have stable semantic identifiers; kernel leaves have
explicit numeric tags, with a test pinning every tag/name pair and kernel CID.
Neither hashes its Rust implementation. Definitions
use the `march-definition-v1` domain with explicit kinds and lengths. These
are encoding/meaning versions, not compiler release numbers. Names/immediate
flags belong to dictionary images, not individual definition identity.

There is no pre-hash arithmetic normalization: `1 1 +` differs from `2`.
Whitespace, comments, and integer spelling do not survive into the definition.
Execution plans and structurally shared register graphs are derived afterward;
optimizing them cannot change a definition's CID. Runtime context values are
not baked into reusable word identities. Compile-time execution can deliberately
emit a computed literal, as `answer:` does above.

## Images, bounds, and verification

Only `MARCHF05` is supported: every record is a canonical definition. Loading
checks CIDs and lowers definitions afresh; derived execution graphs are never
serialized. All previous prototype formats (F01–F04) are rejected; regenerate
images from source. There is no compatibility reader, writer, or mixed-record
mode. The low-level `add_word` API remains for in-memory evaluator fixtures in
a separate diagnostic identity domain. Such fixtures cannot be saved or
referenced by canonical definitions. Loading validates code and flags without
executing the saved interpreter. A loaded stream image can
interpret further source without rerunning `stream::seed()`.

Use a fresh output path: saving overwrites the named file.

```sh
cargo run --offline --release --bin march-fast -- examples/fast/compiler.march --save-image compiler.mimg
cargo run --offline --release --bin march-fast -- --load-image compiler.mimg --extend 'define cube dup dup * * ; 3 cube'
# [Int(27)]
```

`--extend` executes the newly returned entry, not the old entry. It requires an
image containing `stream.interpret`; an old ordinary code image can still be
executed, but is not automatically augmented with a compiler. Live pending
runtime values/compiler-state snapshots are not saved.

`stream::compile_with_limits` bounds execution fuel shared across nested
compiler calls, state count, evaluator cells, builder nesting, native execution
nesting, source bytes, and code words. Defaults are 2,000,000 fuel, 16,384 states,
1,000,000 cells, 128 builder levels, 64 nested compiler invocations, 4 MiB source,
and 100,000 code words. These are logical limits, not a hard RSS/time guarantee.
Runtime CLI `--budget` is separate. Tests also run under external timeouts.

Source interpretation is atomic at this boundary: on failure the supplied
Program is unchanged. Each successful call starts a fresh input/compiler-state
session with the existing dictionary. It is not yet a persistent REPL cursor
or compile-time data stack across calls.

The implementation retains bounded immutable state snapshots for a compilation
call and uses copy-on-write Program snapshots. Dictionary names and immediate
flags now use `imbl` persistent HAMTs, so those containers share unchanged paths.
Builders, the word vector, text intern tables, and the CID index still incur
copying costs. This is not a fully persistent Program and can remain expensive
for large sources; no compiler-throughput or constant compiler-memory claim is
made. Images explicitly sort dictionary names and flags rather than depending
on hash iteration order. Runtime scalar/lazy execution paths remain the existing
engine. Stream-state lifetime improvements are separate from lazy-heap collection.
The [evaluated-state store](STORE.md) now has runtime `store.get` / `store.put`
words with implicit sequencing, but compiler/context unification remains.
Runtime store operations are explicitly unavailable during immediate/compiler
execution; they do not modify compiler state or silently discard runtime writes.
The dictionary is not migrated merely to change its container: unlike the new
store it does not request Merkle identities, and the CHAMP 0.1.0 probe shows
higher lookup costs, including temporary owned keys for the `&str` interface.

`tests/fast_stream.rs` covers numeric precedence (including immediate numeric
bindings), overflow, raw-input comments/custom delimiters, redefinable
punctuation, user defining words, compile-time arithmetic, contextual compiler
words, lazy runtime behavior, immutable dependencies, atomic failure, resource
limits, reproducible images, and CLI extension. The old token-plan tests were
replaced because they tested the discarded interface.
