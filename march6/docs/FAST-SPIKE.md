# Conventional March spike

Source paths and shell commands in this guide are relative to `march6/`.

2026-09-25. A working implementation of the new direction, **not an interaction-net
backend** and not yet a complete March. See [DIRECTION.md](DIRECTION.md) for the
language goals and [FAST-BENCHMARKS.md](FAST-BENCHMARKS.md) for measurements.

## Try it

From `march6/`:

```sh
cargo run --offline --release --bin march-fast -- --eval ': square dup * ; 7 square'
# [Int(49)]
cargo run --offline --release --bin march-fast -- examples/fast/lazy-stream.march
# [Int(11)]
cargo run --offline --release --bin march-fast -- --context enabled=true examples/fast/context.march
# [Int(7)]
cargo run --offline --release --bin march-fast -- examples/fast/fibonacci.march
# [Int(55)]
```

Source can leave explicit inputs for the invocation:

```sh
cargo run --offline --release --bin march-fast -- --arg 7 --eval 'dup *'
```

Save/reload code with `--save-image PATH` and `--load-image PATH`. For example,
add `--save-image square.mimg` to the preceding command, then load with
`--load-image square.mimg --arg 9` to obtain `[Int(81)]`. Saving overwrites the
requested path. Images contain canonical code and dictionary roots, **not**
context, pending execution, or memo tables.

Default execution fuel is 100,000; use `--budget N` to change it. Resource
exhaustion aborts that invocation; it is not a resumable pause or a language
value. CLI pairs print opaque handles, not recursively normalized contents.
Use `first` and `second` to inspect fields selectively.

## What works

- Provisional FORTH surface: `: name ... ;`, inferred stack interfaces,
  zero/multiple results, stack shuffles, checked integer arithmetic, comparisons.
- Closed quotations: `[ dup * ] call`, `quote square call`, or `' square call`.
  Dynamic `apply N M` checks N inputs and M outputs. No implicit lexical captures.
- Lazy branches, arguments, individual call outputs, and immutable pairs.
  `select` takes condition, true alternative, false alternative in that order.
  Rejected work is not evaluated, including erroneous or nonterminating work.
- Ordered guarded word families over explicit immutable invocation context.
  First true guard wins; later guards and rejected bodies are not evaluated.
- Recursion via `recur N M`. A family body recurs into its selecting family.
  A standalone word recurs into itself.
- Shared pending instances memoize their result or semantic error. Independent
  outputs remain usable when another output fails.
- Canonical composed definitions addressed by ordered dependency CIDs/literals;
  quotation identity; separately computed value content identities; deterministic,
  validated images. Stack wiring and optimized plans are derived, not word identity.
- A March-defined input-stream interpreter: whitespace-delimited words,
  **numbers before dictionary lookup**, and input-consuming dictionary words.
  Colon, comments, and user-defined immediate words participate in compilation;
  the interpreter and dictionary flags persist in images. See [BOOTSTRAP.md](BOOTSTRAP.md).
- Canonical definition inspection/construction through ordinary text/tuple
  descriptors and explicit compiler state; computed names bind constructed
  code without reparsing source. `examples/fast/reflection.march` demonstrates it.
- Explicit evaluation task stack, stale/cross-executor handle checks, bounded
  fuel and storage, and demanded-slot recursive-cycle detection.

Comparisons use postfix predicate names, each taking `a b` and returning a
Boolean:

| Word | Meaning |
|---|---|
| `eq?` | a equals b |
| `lt?` | a is less than b |
| `gt?` | a is greater than b |
| `lte?` | a is less than or equal to b |
| `gte?` | a is greater than or equal to b |

The seed does not bind `=`, `<`, `>`, `<=`, or `>=`. These are still legal names
for user definitions. `eq?` and `lt?` bind the existing semantic primitives;
their CIDs do not change with the source spelling. The other three are ordinary
composed words, not additional native operations:

```forth
: gt? swap lt? ;
: gte? lt? false true select ;
: lte? gt? false true select ;
```

Ordering is defined for two integers or two texts, not across types. The
reversed comparisons inherit the demand order of `swap lt?`. No universal
type ordering has been introduced.

The CLI uses `stream::compile`, not the host `source::compile` test/seed reader.
Both readers now produce the same definition identity and quotation equality.
`stream-seed.march` owns the read/number/lookup/execute-or-compile loop. Native
primitives provide input advancement, immutable compiler state, definition
builders, graph lowering, stack inference, and validation. The initial seed is assembled once through the
older host reader; loaded seed images need no such reader for subsequent input.
The interpreter now passes a three-generation saved-image rebuild fixed-point
test; native lowering/primitive semantics and host alias installation remain.
This is not full self-hosting. Only definition-only `MARCHF05` images are
supported; old formats must be regenerated from source. See BOOTSTRAP.md for the
identity boundary, explicit recursion contracts, and compatibility details.

Words are whitespace-delimited, including `:`, `;`, `[` and `]`. Punctuation is
not split automatically. `--` is a dictionary word that consumes through newline;
`(` consumes through the first `)` and legacy backslash is a line-comment alias.
`ctx key` reads a context value. `family name N M guard body ... ;` seals an
ordered family. Guards take the same N arguments and return one Boolean; bodies
return M outputs. Parenthesized comments are comments, not checked signatures.
Definitions must precede references; `recur` provides self-recursion. Dictionary
names are redefinable, including primitive/defining/comment words. Numeric names
are legal but numeric input wins over lookup; explicit quotation can still look
up such a name. Signed decimal i64 overflow is an error before lookup.
Each stream compilation is atomic on failure. `--load-image PATH --extend SOURCE`
uses the saved March interpreter/dictionary and executes the new entry.

## Representation and execution

`src/fast/mod.rs` separates an immutable, content-addressed word DAG from an
invocation-local execution representation. Code links resolve to local word
indices. Calls allocate indexed frames/cells on demand; stack shuffles and
argument aliases share handles. No CAS insertion or cryptographic hashing occurs
for each arithmetic operation.

A derived scalar plan follows demanded static calls and projections, inlines
their operations, and executes directly in reusable registers. It preserves
demand/error order and skips unused inputs and outputs. Unsupported operations
fall back to the lazy task engine. This is neither a JIT nor native code generation.
Recursive-demand detection uses ordinary hash-table keys with full collision
checks, not content-addressing of intermediate values.

`Executor::start` / `force` expose selective observation; `run` requests all
outputs, and `run_into` reuses output/register storage. Pair fields remain lazy
even when the pair itself is returned. `content_id(handle)` is a **deep** demand:
it forces the finite value needed for its identity. An infinite sequence can be
used by prefix, but attempting to hash the entire sequence exhausts its budget.

The context is fixed for an invocation. Resetting invalidates all prior handles
and memos. Compiled code retains its original dependencies after dictionary
rebinding. Images verify dependency CIDs and canonical encoding; they do not
serialize the derived execution plan.

## Sharing: the precise current boundary

1. **Construction sharing:** equal structural operations/calls within a word
   are merged at compilation. `5 big 5 big +` shares the call in that word.
2. **Instance sharing:** multiple uses of one suspended instance compute it
   once, including across callers and separate output observations.
3. **Independent runtime equality:** separate call instances that become equal
   only after argument substitution are **not** automatically merged. Some old
   reference fixtures have stronger canonical sharing at this boundary. The
   spike characterizes this difference; it has not established equivalence.

`dup` does not imply recomputation. Conversely, this is not global memoization
of every pure function by argument value: naive recursive Fibonacci still has
repeated calls. Cycle checking recognizes identical argument-cell identities
and demanded slots, not arbitrary equal-valued recursive states. Different
outputs of a recursive call must not be falsely rejected as a cycle.

## Measurements and limits

The pre-stream-frontend release measurements across three pinned processes, using reused
executors and runtime inputs: source square **24.2–35.0 ns**, shared call
**23.4–42.3 ns**, conditional **181.2–317.1 ns**, contextual family
**524.6–692.1 ns**, 64-add chain **243.8–331.2 ns** per invocation. Host noise
was substantial. These are small microbenchmarks, not a general speed ratio
against Rust or a like-for-like replacement of every reference semantic.
The benchmark document includes cold/fresh costs, baselines, raw medians, and
the final checkpoint superseding earlier faster timings.

**Memory is still unfinished, but scalar tail recursion has a bounded path.**
The contextual countdown now runs in three registers, with no lazy frames/cells,
including the tested depth 100,000. This optimization applies to the one-input,
one-output family subset described below. The generic evaluator now supports
explicit collection between host observations, described below; without it,
frames/cells remain until invocation reset/drop. The selective countdown control
at depth 10,000 still retains 40,003 frames and 130,009 cells during one force.
There is no automatic collection within a force. Storage limits cap entries,
not total bytes. This is not a solved general memory-management design.

### Scalar family tail loops

`src/fast/tail.rs` derives an optional execution plan without changing code CIDs
or the image format. A family qualifies only when:

- It takes one argument and returns one output.
- Its first guard's scalar plan starts by demanding that argument, before any
  operation that could fail. This proves the next recursive argument is needed.
- All guards and terminal bodies have scalar plans; a recursive body's output
  is directly output zero of `recur 1 1`, whose argument also has a scalar plan.
- The derived plan fits bounded compilation limits (1,024 clauses and 16,384
  instructions across the entry, guards, and bodies).

`run` / `run_into` reuse a register buffer and replace the current argument on
each tail transition. Only the selected body executes. The same-argument-cell
cycle rule remains: a direct parameter alias cycles; a freshly computed equal
value does not become a value-keyed cycle. Fuel still bounds nontermination.
`Stats::tail_iterations` and `peak_registers` distinguish this path from lazy
frames and the existing straight-line scalar path.

A single static call wrapper such as `10000 countdown` also qualifies. Arbitrary
nested wrappers, multiple arguments/results, context-reading guards, ordinary
non-tail recursion, and lazy structures fall back. `start` / `force` deliberately
retain the selective evaluator as a semantic control, even for qualifying code.
This is conservative tail-call elimination, not a GC or general eager calling.
Example: `examples/fast/countdown.march`. Same-code comparison benchmark:
`cargo run --offline --release --example fast_tail_bench`.

### Explicit boundary collection

The Rust embedding API provides
`Executor::collect(&[Handle], budget) -> Result<Vec<Handle>, Error>`.
Supply every handle the host wants to retain; the collector traces pending
dependencies and memoized values without evaluating anything. It compacts the
heap and returns replacement roots in the same order, preserving duplicates:

```rust,ignore
let roots = executor.collect(&[fast_cursor, slow_cursor], 20_000)?;
fast_cursor = roots[0];
slow_cursor = roots[1];
```

Success invalidates **all** previous handles, including fields of previously
returned pairs. Re-force a retained pair to obtain fresh field handles, or
include separately held fields among the roots. Invalid roots, insufficient
collection budget, or storage-limit errors leave the old heap/handles unchanged.
The collection budget is separate from evaluation fuel; collection does not
refill fuel or revive an aborted invocation. Semantic error memos survive while
reachable. Empty roots release the lazy heap of a non-aborted invocation.

With collection every 64 observations, the incrementing-stream benchmark
consumes a million values with a peak of 390 cell slots, or 453 with two consumers
32 positions apart, with no recomputation. This uses generic `start` / `force`,
not scalar tail-loop optimization. Holding the stream head retains its prefix;
walking only tails without demanding heads retains their pending arithmetic
chain. Those are live dependencies, not garbage.

This is an opt-in embedding API, not automatic CLI reclamation. It cannot run
inside `force` / `content_id` and does not remove live recursive continuations.
Collection scans the allocated heap and temporarily holds old/new storage plus
marking tables. See [FAST-MEMORY.md](FAST-MEMORY.md#explicit-boundary-collection)
for costs, measurements, and the exact storage-accounting limits.

Other missing pieces:

- Missing context is currently an error, not an unknown value that produces a
  residual program. Explicit execution of compiler words now works; general
  partial evaluation/staging and pending-state images remain a significant gap.
- Guarded word families are not yet full module-level context groups.
- No effectful I/O protocol, live development image, self-hosted reader/compiler,
  persistent general-value store, rich collection library, or native backend.
- No implicit parallelism or closure capture.
- Execution budgets count implementation work and differ between scalar and
  generic paths; they are safety limits, not semantic cost measurements.

## Text and lazy tuples

Text is a distinct immutable UTF-8 value, not an integer list. Literals use
`"..."`/`""` with `\"`, `\\`, `\n`, `\r`, and `\t`. Exact decoded bytes
determine identity: composed/decomposed Unicode spellings remain distinct.
`eq?` compares text bytes and `lt?` orders UTF-8 bytes, not locale collation.
Code-point operations do not claim grapheme/user-perceived-character behavior.

| Word | Stack contract | Behavior |
|---|---|---|
| `tuple N` | N values → tuple | Fixed arity; no fields demanded. |
| `untuple N` | tuple → N values | Checks exact arity when an output is demanded; dropped outputs stay lazy. |
| `tuple-length` | tuple → integer | Demands the container, not fields. |
| `nth` | tuple index → value | Zero-based; container, index, range check, then selected field. |
| `tuple-set` | tuple index replacement → tuple | New tuple sharing other fields; replacement stays lazy. |
| `text-bytes` | text → integer | UTF-8 byte length. |
| `text-chars` | text → integer | Unicode scalar count; linear scan. |
| `text-concat` | text text → text | Copies into a new immutable buffer. |
| `text-slice` | text start end → text | Half-open byte offsets; rejects invalid UTF-8 boundaries; copies. |

Zero fields is `unit`; one field is a distinct singleton tuple; two fields are
the existing pair representation. `first`/`second` select tuple fields 0/1.
Host callers still see `Value::Pair` for two fields and `Value::Tuple` for
other nonempty arities. Tuple equality compares arity, then fields left-to-right
using their scalar/tuple equality rules, short-circuiting at a difference.
`eq?` returns false across different value types, including mismatched tuple
field types; it does not coerce values. Both operands are still demanded, so
evaluation errors and divergence remain observable. Internal compiler-state
capabilities are not comparable. Ordering predicates and arithmetic remain type-strict.
Even two references to the same tuple do not skip potentially failing fields.
Full `content_id` observes all fields with bounded traversal. `untuple 0` has
no result to demand, so does not force or validate its input.

```forth
"hello" 7 ctx absent tuple 3 0 nth
-- Text("hello"); the missing context field is never demanded.
```

Program text literals are interned under a 32 MiB aggregate byte cap. Their
definition/image encoding contains text bytes, never local text IDs. Runtime
text buffers and non-pair tuple fields use simple executor arenas; explicit
collection traces/remaps them alongside cells. `text_byte_limit` defaults to
32 MiB and `tuple_field_limit` to 1,000,000 slots. Byte-scanning/copying operations
charge fuel. `Storage` reports runtime text bytes and tuple field slots; program
literals and host-owned returned strings are excluded. Limits are logical
payload limits, not a hard process RSS guarantee. Collection remains atomic,
requires explicit roots, and invalidates previously returned field handles.
Slices copy now; shared-buffer slicing and advanced memory planning are deferred.

The native host API supports `Program::intern_text`, `Literal::Text`, and
`InputNode::Text`/`Tuple`; local literal IDs belong to their Program, like word
IDs. Code images still do not contain live tuple heaps or pending computations.

## Verification and next decisions

Tests are split into `fast_core`, `fast_source`, `fast_image`, `fast_reference`,
`fast_values`, `fast_tail`, `fast_adversarial_claude`, `fast_collection`,
`fast_reclamation_claude`, `fast_stream`, `fast_definition`, `fast_data`,
`fast_comparison`, `fast_dictionary`, `fast_reflection`, and
`fast_reflection_claude`; the old implementations
and tests are retained. These cover
selective demand, sharing, overflow/error order, recursion, dynamic quotation
arity, image corruption and canonicality, lazy streams, value identity, and
explicit reference differences. Run `cargo test --offline --release --all-targets`.
Benchmarks: `cargo run --offline --release --example fast_bench`.

Initial spike verification: **401 tests pass in both debug and release** across
all targets, including 56 new fast-engine tests. All-target Clippy with warnings
denied, formatting, and diff whitespace checks pass. CLI smoke checks cover all
four source examples, both context choices, closed quotation invocation,
skipping an infinite branch, and saving/reloading a code image with new inputs.
The first 30-second debug-suite attempt timed out in historical tests; the
complete rerun used a 180-second external limit and finished successfully.

The tail-loop follow-up adds 14 tests for constant workspace, differential
results/errors, unchanged-argument cycles, finite fuel/storage, nonstrict-guard
fallback, multiple lazy outputs, malformed projections, compile limits, source
wrappers, and image round trips. See the benchmark follow-up for timing evidence.
The full 415-test suite passes in both debug and release, and Claude's 17
additional independent tests also pass locally in both profiles: 432 tests
total, including 87 fast-engine tests. [FAST-MEMORY.md](FAST-MEMORY.md) records constant-workspace evidence
and the same-code timing comparison.

The explicit-collection checkpoint adds 11 collector tests plus Claude's 17
independent semantic/reclamation tests: **460 tests total, 115 fast-engine
tests**, passing in debug and release. All-target Clippy with warnings denied,
formatting, and whitespace checks pass. The tests preserve sharing, lazy errors,
cycle identity across relocation, atomic failure, and bounded consumed-stream
windows. `examples/fast_stream_bench.rs` measures the same generic code with and
without collection.

The stream-nucleus checkpoint has 16 tests in `fast_stream`: **476 total, 131
fast-engine tests**, passing in debug and release. They replace the discarded
token-plan prototype's tests. Numeric priority, raw stream consumption,
redefinable punctuation, new defining words, contextual immediate behavior,
lazy runtime semantics, and persisted interpreter code are exercised. See
`examples/fast/compiler.march` and BOOTSTRAP.md for the remaining native
responsibilities, explicit compiler-state interface, and resource limits.

The definition-identity checkpoint adds tests for exact sequence hashing,
dependency changes, quotation/context/family identity, symbolic recursion,
optimization-independent identity, obsolete-image rejection, hostile definition
records, and the interpreter rebuild fixed point.
The follow-up cleanup removes old-image compatibility and graph-based source
identity, pins kernel tags, and checks host/stream identity agreement.
Verification: **494 total tests, including 149 fast-engine tests**, pass in
debug and release, along with all-target Clippy (warnings denied), formatting,
and whitespace checks.

The text/tuple checkpoint adds 22 integration tests and one kernel-state
containment test: **517 total tests, including 172 fast-engine tests**, pass in
debug and release. All-target Clippy (warnings denied), formatting, and
whitespace checks pass. Coverage includes seed-controlled string recognition,
UTF-8 boundaries, independently lazy tuple fields, structural equality and
value CIDs, code-image round trips, bounded divergent observations, storage
limits, and atomic collection of tuple fields and runtime text.

The cross-type equality follow-up adds three tests covering all-pairs value
comparisons, mixed tuple fields, preserved demand errors, and noncomparable
compiler-state capabilities. **520 total tests, including 175 fast-engine
tests**, pass in debug and release; all-target Clippy (warnings denied),
formatting, and whitespace checks pass.

The postfix-predicate follow-up adds five tests for `eq?`, `lt?`, `gt?`,
`lte?`, and `gte?`, including host/stream identity, composed definitions,
image round trips, and rebinding. **525 total tests, including 180 fast-engine
tests**, pass in debug and release, along with all-target Clippy (warnings
denied), formatting, and whitespace checks. The CLI predicate smoke test passes.

The persistent-dictionary follow-up uses `imbl` 7.0.2 HAMTs for names and
immediate flags. Five new tests cover snapshot isolation, retained versions,
canonical image order, immutable compiled references, and failed-compilation
atomicity. **530 total tests, including 185 fast-engine tests**, pass in debug
and release, with all-target Clippy (warnings denied), formatting, and whitespace
checks passing. A code image saved before/after the storage change is byte-identical.
FAST-BENCHMARKS.md records the container-only probe. This is not a language-visible
map or namespace implementation, and other Program fields still copy on update.

The definition-data checkpoint adds ten tests: every seed definition and every
item kind round-trips through March data; constructed/transformed code has the
expected CID and runs across image reload; snapshot skew, malformed input,
failure atomicity, depth/code limits, and divergent descriptor fields are
covered. **540 total tests, including 195 fast-engine tests**, pass in debug and
release, with all-target Clippy (warnings denied), formatting, and whitespace
checks passing. CLI save/load/extend of `examples/fast/reflection.march` produces
42 and 43 before and after reload. Claude reviewed the interface and supplied
15 independent implementation tests covering CID/quote equivalence, snapshot
isolation, newly constructed references, recursive families, atomic rejection,
immediacy, numeric priority, image reachability, and undemanded queries.
The combined checkpoint passes **555 total tests, including 210 fast-engine
tests**, in debug and release, plus all-target Clippy (warnings denied),
formatting, and whitespace checks.

The next bootstrap priority is reducing native seed assembly and graph/stack
construction, and resolving the compiler-state versus runtime-stack interface.
Advanced memory-management research is deferred to a later "2.0"; for now retain
correctness and safety limits using straightforward storage. Definition data
now exposes inspection/construction through text/tuples; the next data slice is
language-visible immutable maps and namespaces, building on the persistent
dictionary without tying language identity to its implementation.
Richer contextual groups and general staging
remain separate priorities. Before claiming this replaces the reference, settle the independent
equal-call sharing contract and the missing-context/residualization contract.
Self-hosting should build on those decisions, not conceal them.
