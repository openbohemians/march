# FORTH-style self-extension: stream nucleus

Source paths and commands are relative to `march6/`.

Thomas's correction: March must not start with a compiler-owned lexer and
preclassified syntax. Words pull input from a stream and decide what to do with
it. His deliberate difference from traditional FORTH is **numbers before
dictionary lookup**.

The earlier token-view/action-plan extension prototype has been replaced, not
retained as another active frontend. This checkpoint is still not a complete
self-hosted language or bootstrap fixed point.

## What actually runs

`src/fast/stream-seed.march` contains ordinary March words implementing:

1. Read the next whitespace-delimited word.
2. Stop at EOF, checking that definitions are complete.
3. Try the word as a number.
4. If numeric, place that literal in the current pending expression/definition.
5. Otherwise look it up in the ordinary dictionary.
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

Native code still supplies:

- Stream cursor/word/delimiter operations and numeric conversion.
- Dictionary lookup, immutable compiler-state storage, and binding/flag updates.
- Validated graph builders, stack-interface inference, literal/call emission,
  quotation/family construction, and canonicalization.
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

The one-time `stream::seed()` construction uses the retained host reader to
assemble the initial March seed and native primitive wrappers. After that,
`stream::compile` runs the saved March interpreter; neither new source nor
new word bodies are passed to that reader. The older `source::compile` API
remains for legacy tests/bootstrap scaffolding, not as the CLI frontend.

The next bootstrap milestone is to reduce that initial native assembly and
move more graph/stack construction into March, then demonstrate a reproducible
compiler bootstrap fixed point. The present proof is stream-driven
self-extension, not completion of that goal.

## Images, bounds, and verification

Images containing kernel words/immediate flags use `MARCHF03`; ordinary
non-kernel code images remain byte-for-byte `MARCHF01`. The unreleased
token-plan prototype's `MARCHF02` is not supported. Loading validates code and
flags without executing the saved interpreter. A loaded stream image can
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

The first implementation retains bounded immutable state snapshots for a
compilation call, copies builders, and uses copy-on-write Program snapshots.
That can be expensive for large sources; no compiler-throughput or constant
compiler-memory claim is made. Runtime scalar/lazy execution paths remain the
existing engine. Stream-state lifetime improvements are separate from runtime
lazy-heap collection.

`tests/fast_stream.rs` covers numeric precedence (including immediate numeric
bindings), overflow, raw-input comments/custom delimiters, redefinable
punctuation, user defining words, compile-time arithmetic, contextual compiler
words, lazy runtime behavior, immutable dependencies, atomic failure, resource
limits, reproducible images, and CLI extension. The old token-plan tests were
replaced because they tested the discarded interface.
