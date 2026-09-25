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
- Canonical, content-addressed code; quotation identity; separately computed
  value content identities; deterministic, validated binary code images.
- Explicit evaluation task stack, stale/cross-executor handle checks, bounded
  fuel and storage, and demanded-slot recursive-cycle detection.

The host-side source compiler is scaffolding, not the old self-hosted seed.
`ctx key` reads a context value. `family name N M guard body ... ;` seals an
ordered family. Guards take the same N arguments and return one Boolean; bodies
return M outputs. Parenthesized comments are comments, not checked signatures.
Definitions must precede references; `recur` provides self-recursion. Builtin
names are reserved. Source errors do not roll back earlier installed definitions.

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

Final release measurements across three pinned processes, using reused
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
one-output family subset described below. General lazy frames/cells still remain
until invocation reset/drop: the selective evaluator control at depth 10,000
retains 40,003 frames and 130,009 cells. There is no general within-invocation
reclamation. Storage limits cap entries, not total bytes; vector capacity may
remain from a prior run. This is not a solved general memory-management design.

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

Other missing pieces:

- Missing context is currently an error, not an unknown value that produces a
  residual program. Partial evaluation/staging and pending-state images remain
  a significant gap relative to the older reference.
- Guarded word families are not yet full module-level context groups.
- No effectful I/O protocol, live development image, self-hosted reader/compiler,
  persistent general-value store, rich collection library, or native backend.
- No implicit parallelism or closure capture.
- Execution budgets count implementation work and differ between scalar and
  generic paths; they are safety limits, not semantic cost measurements.

## Verification and next decisions

Tests are split into `fast_core`, `fast_source`, `fast_image`, `fast_reference`,
`fast_values`, `fast_tail`, and `fast_adversarial_claude`; the old implementations
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

The next useful work is an allocation/lifetime improvement with these demand
tests held fixed, followed by richer contextual groups and a deliberate staging
interface. Before claiming this replaces the reference, settle the independent
equal-call sharing contract and the missing-context/residualization contract.
Self-hosting should build on those decisions, not conceal them.
