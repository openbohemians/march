# Strict stack baseline

2026-09-27: March's execution direction is **strict by default, with a real
runtime stack**. Laziness belongs in explicit constructs and data abstractions,
not every ordinary computation. Graphs remain useful compiler representations;
they are not the semantic execution model. This supersedes the lazy-by-default
direction, not the CAS, immutability, context, or FORTH self-extension goals.

## Runnable first slice

From `march6/`:

```sh
cargo run --locked --offline --bin march-fast -- --stack --eval ': square dup * ; 7 square'
# [Int(49)]
cargo run --locked --offline --bin march-fast -- --stack examples/fast/state.march
# [Int(10), Int(12)]
cargo run --locked --offline --bin march-fast -- --stack --eval '[ 5 "x" store.put 1 ] apply 0 1 drop "x" store.get'
# [Int(5)]
cargo run --locked --offline --bin march-fast -- --stack examples/fast/strict-data.march
# [Text("original"), Text("changed")]
```

`fast::stack::Machine` executes the existing canonical `Definition::Sequence`
items in source order on a `Vec` data stack and an explicit continuation stack.
There are no pending value cells, dependency-graph evaluation, computation memo
tables, state-edge chains, or calls into the old executor in this runtime. Calls are checked against their
inferred input/output counts and cannot consume a caller's protected stack prefix.
This is a straightforward interpreter, not threaded bytecode or a native backend.

Supported: integer arithmetic/comparison, structural equality, booleans/unit,
shared text and tuples, tuple construction/projection/replacement, UTF-8 text
operations, stack shuffles, closed quotations, static/dynamic calls, context
lookups, ordered guarded families, recursion, and store reads/writes with text
or tuples of text for namespace paths. Quotation lookup uses exact CIDs,
not current name bindings. Guards execute on copies of their argument slice;
only the selected body runs. Guards can read state, but writes are rejected even
through dynamic application. Recursion uses bounded explicit continuations,
not the host call stack. Compatible tail calls now reuse their return check;
see the tail-call checkpoint below for the exact condition and measurements.

The immutable store is now simply an invocation-local current snapshot updated
in instruction order. A successful invocation returns its final stack and store;
a failing invocation returns neither and leaves the host's input store untouched.
The CLI still starts an empty store per invocation. Atomicity here is for in-memory
snapshot publication, not a promise to roll back future external I/O.

## Deliberate semantic differences

| Program | Earlier lazy runtime | Strict stack baseline |
|---|---|---|
| `9223372036854775807 1 + drop` | Unused overflow skipped | Overflow before `drop` |
| `"missing" store.get drop` | Unused read skipped | Missing-entry error |
| `[ failing-code ] drop` | Body deferred | Body still deferred: a quotation is code |
| Writing quotation, `apply … drop` | Undemanded dynamic call bypasses non-writing check | Call executes and write occurs |
| `true a b select` | Pure unselected work may be skipped | Both words execute; then a value is selected |
| Guarded family | Selected body only | Selected body only |

The old dynamic-apply check's undemanded-call gap was reported by Claude. The
three reported examples are regression cases for the stack baseline. This work
does not silently change the old evaluator's semantics or repair it by adding
more state-chain machinery.

## Transitional boundaries (not completed features)

- `--stack` is explicit; the CLI default remains the previous evaluator during
  this bounded migration. Unsupported features error; there is no fallback into
  graph evaluation. Compiler primitives are not yet supported by the stack runtime.
- The existing March-defined input-stream compiler still runs on the previous
  engine and still constructs lowered graphs for inference/validation. Runtime
  stack execution reads canonical definitions, not those graphs. This is not yet
  a migrated bootstrap, a second lexer, or a new source syntax.
- Dynamic `apply N M` and `recur N M` retain the current source counts, checked
  against the target's inferred shape at runtime. Count-free typed application
  requires the planned stack type/signature work; counts are not the final design.
- No `lazy` constructor, shared explicit thunk, compiler-inserted force, roles,
  distinct type system, units, or automatic runtime thunk checks were added.
  Closed quotations defer code but do not themselves memoize its execution.
- Stack/continuation lengths, tuple node/field creation, text size and work fuel
  are bounded. Values are reference-counted immutable data; these limits and
  allocation counters are not total RSS accounting or a general memory planner.
- CLI stack-mode image loading/saving is rejected. Evaluation policy affects
  observable meaning, so identical canonical bytes cannot acquire strict semantics
  under an implicitly unchanged semantic CID. Before making this the default,
  version the semantic domain/primitives/image boundary deliberately. No old-image
  migration requirement is being introduced. The low-level host experiment uses
  existing `Program` objects only under an explicit choice of engine.

## Verification and measurements

`tests/fast_stack.rs` covers strict errors, stack composition, conditional bodies,
dynamic and stored writing quotations, immutable publication, read-only guards,
bounded recursion, fuel cuts, text/stack limits, supported scalar agreement with
the previous runtime, and CLI selection/image rejection.

Initial scalar checkpoint: **607 tests passed in both debug and release**, including 13 new stack
integration tests. All-target Clippy with warnings denied, rustdoc, formatting,
and diff-whitespace checks pass. The previous 594 tests remain intact; they
validate the retained engines, not strict semantics by themselves.

Run the bounded release probe with:

```sh
timeout 90 cargo run --locked --offline --release --example stack_bench -- 20000
```

It compares this unoptimized baseline with the previous runtime including its
scalar and tail-loop fast paths. Source compilation/setup is reported separately,
invocations are warmed, inputs vary, and checksums are checked. Results include
result-vector allocation/copying and execution safety checks. Rust is only a
value-result reference: its `n + 1` does **not** implement a persistent store, and
its constant-zero function does **not** implement recursive dispatch. Do not read
those columns as equivalent algorithmic or store-performance comparisons.

First local measurement (2026-09-27), median of three unpinned release runs,
20,000 invocations each, nanoseconds per invocation:

| Workload | Strict baseline | Previous runtime |
|---|---:|---:|
| `dup *` | 147.8 | 56.8 |
| Two square calls | 270.0 | 63.6 |
| Guarded countdown, inputs 0–31 | 5,358.4 | 766.3 |
| Two writes and two reads | 734.8 | 2,338.4 |

The previous runtime wins the arithmetic/recursive cases with its specialized
fast plans and tail loop. The new baseline wins this small store case without
state-edge scheduling/deep-transfer tasks. This is a tiny mixed-result probe,
not an application-speed or peak-memory claim. Three samples are insufficient
for statistical conclusions; unpinned timings vary. The strict countdown's
last invocation retained 100 continuation tasks at peak versus 3 for the store
case. These are pre-tail-optimization measurements; the checkpoint below records
the subsequent control-stack improvement.

## Next bounded steps

1. Establish the strict semantic identity/image boundary before changing the
   default engine. Immutable tuples/text operations are now implemented (below).
2. Implement/check row-polymorphic stack types and quotation shapes; prototype
   count-free application before coupling it to namespace/compiler unification.
3. Add a small pure, explicitly constructed shared lazy value. Try compiler-
   inserted forcing at typed strict consumers; explicit `force` is the fallback,
   not automatic runtime thunk checks everywhere. Lazy-field syntax remains open.
4. Optimize remaining measured stack dispatch/call overhead. Compatible tail-call
   return reuse is implemented below. Reuse a register/SSA path
   only when it preserves strict execution, errors, effects, and code identity.
   In particular the old fast plans may omit discarded work and are not safe to
   reuse wholesale merely because a word is pure.
5. Migrate compiler execution and then unify state. Keep current compiler/store
   interfaces until the strict core and type contracts are sufficiently tested.

Roles are planned as erased aliases, including named signatures; intentional
distinctness belongs to types. Units are later work. Rewriting is a compiler/
metaprogramming facility over derived forms, not the evaluator. Neither purity
alone nor a rewrite's mathematical validity licenses changing checked-overflow,
error, or termination behavior.

## Shared immutable data checkpoint, 2026-09-28

The strict runtime now returns its own `stack::Value`, not `fast::Value` with
evaluator-owned tuple handles. Scalars remain inline; text uses `Arc<str>`;
nonempty tuples use a private, immutable reference-counted field vector. Values
remain valid after their machine and Program are destroyed or reset. The host
API still accepts `Literal` invocation arguments; general structured host inputs
are not added by this slice.

- `dup`, `over`, projections and result/guard copies share text/tuple payloads.
  Text literals share the Program's existing text storage; they do not allocate
  or scan/copy the string on each push. There are no deferred fields or captures.
- `tuple N`, `untuple N`, `pair`, `first`, `second`, `tuple-length`, `nth`, and
  `tuple-set` work on fully evaluated fields. `unit` is the empty tuple and
  `pair` is a two-field tuple, preserving existing value identities.
- `tuple-set` creates a new outer field vector, sharing unchanged nested data.
  This is O(tuple width), not a large persistent-array implementation.
- `text-bytes`, `text-chars`, `text-concat`, and `text-slice` retain existing
  semantics. Slices use byte offsets and reject split UTF-8 characters. Concats
  and slices allocate new text; duplication and store text import share bytes.
  Text ordering compares UTF-8 bytes; mixed-type ordering still errors.
- Equality is iterative, structural and fuel-bounded in the machine. Shared
  tuple-pair visits are memoized *for that comparison*, avoiding exponential
  expansion of shared data. This is not computation memoization or lazy evaluation.
- Last-owner tuple destruction drains children iteratively. Deep acyclic data
  does not require recursive Rust destruction. Tuple construction is private and
  immutable, so this representation cannot form reference cycles. Host equality
  is iterative too; Debug bounds nesting and node count (not text byte output).
- Store writes explicitly export a flat, shared tuple DAG and compute value
  CIDs with the existing `march-fast-value-v1` encoding. Ordinary tuple creation,
  duplication, equality, and projection do not hash values. Store reads rebuild
  tuple nodes and preserve sharing *within each imported value*; repeated reads
  currently rebuild that structure rather than sharing a decoded-value cache.
  Text bytes are shared across this boundary. Quotation bodies are never executed
  by export/import, and missing referenced code fails instead of rebinding names.
- Namespace paths may now be a tuple of exact text components. Dotted text
  remains one component, with the existing conflict and length checks.

`value_node_limit` bounds cumulative tuple construction/import nodes and each
individual frozen export/import's node count; `tuple_field_limit` bounds cumulative
tuple field allocation and each export's field count. `text_byte_limit` bounds
each referenced text's size and cumulative newly allocated text bytes. Work fuel
also covers copying field references, comparisons, hashing input and transfers.
These are deliberately conservative work limits, not live-byte limits: dropping
a tuple does not restore the invocation's allocation allowance. Frozen export
scratch buffers, hash tables, reference-count headers, store history held by the
host, and aggregate retained text bytes are not covered by a process-wide cap.

The new `tests/fast_stack_data.rs` checks pointer sharing, detached lifetime,
strict errors, UTF-8 boundaries, nested namespace/quotation round-trips, format
compatibility, all fuel cuts through transfers, and resource failures. It also
tests 20,000 nested tuples and a logical binary tree with 2^80 leaves whose frozen
representation occupies 82 nodes. Inline scalar leaves can occur more than once
in that representation; shared tuple substructure is exported once.

Verification: **622 tests passed in both debug and release**, including 15 new
strict-data tests. All-target Clippy with warnings denied, rustdoc, formatting,
and whitespace checks pass. The deep-data tests were repeated in both profiles
after the final iterative-destructor adjustment.

### Data allocation probe

```sh
timeout 90 cargo run --locked --offline --release --example stack_data_bench -- 10000
```

This example wraps the system allocator to count allocations/reallocations and
requested bytes, with compilation and warmup excluded. Timings include allocator
instrumentation. Bytes are cumulative allocation requests, **not** peak/live
memory; realloc requests count their complete new requested size. VM result
vectors, temporary buffers, and store structures are included.

Observed allocations per invocation:

| Workload | Allocation requests | Requested bytes |
|---|---:|---:|
| Duplicate/drop 16-byte text, return byte length | 1 | 40 |
| Duplicate/drop 64-KiB text, return byte length | 1 | 40 |
| Construct four-field tuple, return length | 3 | 240 |
| Same tuple plus `dup drop` | 3 | 240 |
| Replace one field, return length | 5 | 440 |
| Shared nested tuple store write/read, return length | 43 | 2,426 |
| Concatenate two four-byte texts, return byte length | 3 | 72 |

The constant-size text result is the intended sharing result, not proof that
all string operations are constant-time. The store row exposes substantial
export/hash/import scratch overhead still present in this first representation.
The global store remains for published state, not computational scratch work.

One final local release run (10,000 invocations, unpinned, allocator instrumentation
enabled) measured 188/186 ns for the 16-byte/64-KiB text cases, 237 ns for tuple
construction, 299 ns with the added `dup drop`, 382 ns for replacement, 1,565 ns
for the nested store round-trip, and 233 ns for concatenation. These are small
diagnostic workloads, not application performance targets. Allocation counts were
stable across repeated runs; timings varied with machine activity.

## Tail-call checkpoint, 2026-09-28

Thomas requested tail-call optimization before revisiting cumulative allocation
limits. **Allocation statistics and limits are unchanged**: temporary tuple/text
allocations continue to accumulate in the invocation's counters, even if their
values are dropped. No live-memory accounting or removal of caps was introduced.

The strict machine now:

1. Omits the end-of-sequence `Next` task when there is no following instruction.
2. On call entry, reuses the immediately pending `Return` check only if both its
   protected stack base and expected output count exactly match the callee's.

The pending check validates both calls. This works for static calls, dynamic
quotation calls, `recur`, and selected family bodies. Argument/quotation shape
checks still run before entry, and the callee still gets its own word/recursion
binder and stack floor. A pending `Next` or guard `Decide` prevents return reuse:
non-tail work must still execute and guards must still select their body. Tail
calls whose base/output contracts do not match retain their separate checks.

This is a conservative optimization, not a promise to eliminate every possible
tail position or turn non-tail recursion into a loop. Word CIDs, evaluation order,
store publication, and value allocation behavior are unchanged. Fuel counts and
continuation-limit outcomes may differ because fewer control tasks are needed.

`Machine::tail_call_optimization` defaults to true; disabling it is a diagnostic
baseline. `Stats::tail_calls` counts entries reusing a return check, including
compatible primitive calls, not just recursive calls. The benchmark now compares
enabled/disabled execution in the same binary and also runs a 100,000-iteration
countdown with a continuation limit of 16.

In one local unpinned release run, that long countdown completed in **34.45 ms**
with **5 control tasks and 2 data-stack entries at peak**. Tests check equal peak
control depth for 10, 1,000 and 100,000 iterations. The small varied-input countdown
probe (0–31, 20,000 invocations) used 5 control tasks instead of 100 at its final
invocation; times were 5,608 ns optimized versus 5,786 ns unoptimized. The timing
difference is modest/noisy; the bounded control depth is the primary result.
Per-iteration guard argument copies and dispatch remain, so this does not claim
allocation-free execution or parity with the previous specialized scalar tail loop.

`tests/fast_stack_tail.rs` adds eight tests: long bounded countdowns, 2,000 static
tail wrappers, dynamic/stored/static quotation calls, preserved caller prefixes,
zero/multiple outputs and stateful loops, non-tail continuations, guard/errors and
contract checks against the disabled baseline, unchanged allocation caps/counters,
and fuel-bounded divergence/reset. **630 tests pass in debug and release**;
all-target Clippy with warnings denied, rustdoc, format and whitespace checks pass.
