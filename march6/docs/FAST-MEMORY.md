# Runtime memory follow-up — 2026-09-25

Source paths and shell commands in this report are relative to `march6/`.

Two improvements are now implemented: conservative scalar tail-call elimination
and explicit lazy-heap collection between host observations. The first section
records the tail-loop checkpoint; the [collection section](#explicit-boundary-collection)
records the later generic stream work. Exact eligibility and API limits are in
[FAST-SPIKE.md](FAST-SPIKE.md#scalar-family-tail-loops).
The earlier countdown-retention measurements remain reproducible through
`start` / `force`; optimized `run` / `run_into` now avoid that allocation.

## Memory evidence

Same countdown family, fresh executor, finite fuel, result zero:

| Depth | Selective evaluator cells / frames | Tail-loop registers / lazy cells / frames |
|---:|---:|---:|
| 100 | 1,309 / 403 | 3 / 0 / 0 |
| 1,000 | 13,009 / 4,003 | 3 / 0 / 0 |
| 10,000 | 130,009 / 40,003 | 3 / 0 / 0 |

The test suite also reaches depth 100,000 under a 16-cell/register limit and
two-argument-slot limit. It uses exactly three registers and one current
argument. The host input conversion/output buffers are constant-sized, not
zero allocations. Counts are logical entries, not RSS or allocator capacity.
A reused executor can retain vector capacity from an earlier generic run.

Tail-call elimination alone does **not** bound general non-tail recursion or
live lazy structures. The later collector reclaims unreachable history between
observations, but cannot remove live continuations during a single force.

## Same-code timing

Build and run:

```sh
cargo build --offline --release --example fast_tail_bench
timeout --kill-after=2s 30s taskset -c 0 target/release/examples/fast_tail_bench
```

Same machine/toolchain as FAST-BENCHMARKS.md, CPU 0 pinned. Three final-build
processes, one capacity-warming batch plus seven measured batches each.
Each invocation resets its memo state. Inputs alternate between depth and
depth+1, passed through black_box; every result is checked. Repetitions per
batch: 100 at depth 100, 10 at depth 1,000, 2 at depth 10,000.
No other project test/benchmark process was running during these final samples;
the machine as a whole is not isolated and timing drift is visible.

All medians and within-process batch ranges, microseconds per invocation:

| Run | Depth | Path | Median µs | Batch range µs |
|---:|---:|---|---:|---:|
| 1 | 100 | lazy | 90.692 | 90.011–91.534 |
| 1 | 100 | tail | 3.283 | 3.281–3.337 |
| 1 | 1000 | lazy | 928.358 | 915.005–945.330 |
| 1 | 1000 | tail | 32.882 | 32.729–34.191 |
| 1 | 10000 | lazy | 10671.294 | 10410.658–10821.837 |
| 1 | 10000 | tail | 343.578 | 342.474–349.455 |
| 2 | 100 | lazy | 100.976 | 99.773–102.380 |
| 2 | 100 | tail | 3.800 | 3.794–3.990 |
| 2 | 1000 | lazy | 1006.725 | 1003.699–1017.425 |
| 2 | 1000 | tail | 38.016 | 37.717–38.796 |
| 2 | 10000 | lazy | 11851.757 | 11637.598–12012.776 |
| 2 | 10000 | tail | 390.161 | 383.949–391.939 |
| 3 | 100 | lazy | 109.341 | 108.280–110.212 |
| 3 | 100 | tail | 4.056 | 4.037–4.106 |
| 3 | 1000 | lazy | 1109.492 | 1100.813–1129.677 |
| 3 | 1000 | tail | 40.598 | 40.487–41.078 |
| 3 | 10000 | lazy | 12745.715 | 12324.271–12867.457 |
| 3 | 10000 | tail | 422.392 | 414.062–428.990 |

At nominal depth 10,000, same-process speedups are about **30–31×**:
10.67–12.75 ms for the selective evaluator versus 0.344–0.422 ms for
the tail loop. This measures one qualifying scalar family, not general March
speed or native-code equivalence. Logical primitive counts match on both
paths; task/fuel counts do not, as already allowed by the resource-limit contract.
Final timing-sample stats use depth+1, so they report 130,022 lazy cells and
40,007 frames, versus the exact-depth table above.

## Existing-workload smoke check

One final-build CPU-0-pinned `fast_bench 20000` run (seven batches) gave
these reused-executor medians: Square 22.6 ns, Quad 27.3 ns, Conditional 221.7 ns,
SharedCall 29.1 ns, ContextFamily 563.0 ns, Chain64 256.3 ns, SourceSquare 24.8 ns.
Direct-Rust baselines were 2.0–2.3 ns except Chain64 at 18.0 ns.
These values sit within the earlier final-checkpoint ranges; a single noisy
smoke run is not a rigorous no-regression proof. No existing workload gains
the tail plan in this table; the recursion probe does.

The optimization leaves canonical code/image bytes unchanged, reconstructs its
derived plan on load, and keeps the general evaluator as a differential control.
Fourteen new tests cover safe eligibility and conservative fallback, error order,
cycle-versus-budget behavior, image round trips, malformed projections,
compile limits, independent outputs, and constant workspace.

Final verification: 415 tests pass across all targets in both debug and release;
all-target Clippy with warnings denied, formatting, and whitespace checks pass.
The countdown source example also runs successfully through the release CLI.

Independent review: Claude reviewed the tail-loop proof and found it sound
under these restrictions. His 17 additional adversarial tests were read and
rerun locally in debug and release, all passing (432 tests total, 87 fast-engine
tests). They cover construction/instance sharing, the independent-equal-call
gap, lazy failures and application checks, cycles/fuel, recursion binding, and
tail-loop agreement with dynamic-application fallback. The shared left-first
demand-order invariant is now cross-referenced in the compiler and evaluator.

## Explicit boundary collection

`src/fast/collect.rs` implements `Executor::collect(roots, budget)`. This is an
opt-in Rust embedding API, not automatic collection during CLI execution.
The host supplies every handle it will need after collection. Success returns
replacement roots in the same order (including duplicates) and invalidates all
old handles through a fresh epoch. Fields from previously returned pairs must
either be supplied as roots or obtained again by forcing a retained pair.
Omitting a root deliberately relinquishes it; retaining a stream head pins its
reachable prefix.

Collection never evaluates pending work. Pending cells retain their operation
dependencies; ready pairs retain fields; ready call bundles retain callee outputs.
Ready scalars and semantic failures retain their outcome, not their construction
history. Frames needed by pending cells still reserve contiguous operation slots,
but unneeded slots in those blocks are padding, not strong references.
Stable argument-identity tags preserve recursive-cycle checks across relocation;
the tags are not roots and are distinct from code/value CIDs.

Tracing/remapping builds a replacement heap before committing. Invalid roots,
collection-budget exhaustion, and storage-limit errors leave the old evaluator,
handles, and statistics unchanged. Collection fuel is separate from evaluation
fuel; it does not refill fuel or retry failures. Aborted invocations reject
collection and require a new start. Empty roots release all lazy cells/frames
and error memos in a non-aborted invocation. Code images and CIDs are unchanged.

### Stream evidence

Reproduce from `march6/`:

```sh
cargo build --offline --release --example fast_stream_bench
timeout --kill-after=2s 30s taskset -c 0 target/release/examples/fast_stream_bench
```

The source is `: from dup 1 + recur 1 1 pair ; from`, started with runtime
input zero through generic `start` / `force`. Each step forces/checks its head
and advances to its tail. The two-consumer variant also reads the same stream
32 positions behind. Every case performs exactly N−1 additions, including with
two consumers: collection preserves sharing rather than recomputing values.
The optimized scalar/tail paths are not used.

Collection runs every 64 steps plus once at the end. Collected runs have a
1,024-cell limit, 256-argument limit, 20,000-unit collection budget, and finite
evaluation fuel. Each benchmark process has a 30-second external cap. Three
CPU-0-pinned release processes were run sequentially with no project tests
running; the host as a whole was not isolated.

| Values consumed | Consumers | Collection | Peak cell slots | Final cells / frames | Sampled peak vector bytes | Final vector bytes |
|---:|---:|---|---:|---:|---:|---:|
| 10,000 | 1 or 2 | none | 60,004 | 60,004 / 10,001 | 6,058,408 | 6,058,408 |
| 10,000 | 1 | every 64 | 390 | 6 / 1 | 54,448 | 432 |
| 10,000 | 2, lag 32 | every 64 | 453 | 69 / 1 | 47,272 | 3,960 |
| 1,000,000 | 1 | every 64 | 390 | 6 / 1 | 54,448 | 432 |
| 1,000,000 | 2, lag 32 | every 64 | 453 | 69 / 1 | 47,272 | 3,960 |

The collected plateau is unchanged from 1,000 through 1,000,000 consumed
values. Final heaps contain one padding slot, hence 5 / 68 non-padding cells.
The million-element cases perform 999,999 additions and 15,626 collections.
The two-consumer vector-capacity peak happens to be smaller because vector
growth starts from different compacted capacities; it is not less live data.

Elapsed milliseconds, including checks, collection, and storage sampling:

| Values / consumers / collection | Run 1 | Run 2 | Run 3 |
|---|---:|---:|---:|
| 10,000 / 1 / none | 7.562 | 7.740 | 7.451 |
| 10,000 / 1 / every 64 | 6.067 | 6.235 | 6.095 |
| 10,000 / 2 / none | 7.188 | 7.258 | 7.362 |
| 10,000 / 2 / every 64 | 6.426 | 6.612 | 6.629 |
| 1,000,000 / 1 / every 64 | 640.536 | 599.439 | 588.228 |
| 1,000,000 / 2 / every 64 | 739.170 | 629.834 | 654.757 |

These are single checked traversals per case/process, not seven-batch medians
or a universal speedup claim. The important result is bounded retained storage
with unchanged computation counts. No uncollected million-element run was
attempted; the smaller controls already show linear retention.

### Accounting and remaining limits

- `Storage::cells` counts allocated slots, including live-frame padding.
  `live_cells` counts non-padding slots, not traced reachability before collection.
  Stats retain cumulative/high-water counts; collection does not reset them.
- Vector bytes count structural vector capacities, sampled before collection
  and at the final boundary. They exclude allocator overhead, hash-table
  buckets, context/error payloads, immutable code, and temporary collector
  scratch. They are not RSS or a peak measurement inside `force`/collection.
  Fresh compacted cell/frame vectors shrink capacity; reused scalar-register
  scratch can retain capacity from an earlier run.
- Collection work scales with the allocated heap plus traced edges and retained
  blocks, not just the live graph. Old/new heaps and marking/remapping tables
  coexist temporarily. The work budget is not a hard byte or wall-clock bound;
  cell/argument limits are entry limits, not a complete memory cap.
- Merely walking tails without forcing heads can keep the whole deferred
  addition chain live, even after releasing the original head. Claude's fixture
  confirms that forcing the current head resolves that chain, after which
  collection releases its history. The collector must not silently force it.
- There is no collection inside `force` or `content_id`, nor automatic root
  registration. A recursive March consumer inside one force may still retain
  live continuations. General forwarding/tail behavior and automatic root
  management are separate remaining work; this collector alone cannot fix them.

Stable identity bookkeeping adds a tag per cell and an identity vector to
cycle-tracked frames. Thus the uncollected generic heap is somewhat larger than
before this change. One final-build CPU-0-pinned `fast_bench 20000` smoke run
gave reused-executor medians (ns): Square 18.5, Quad 23.5, Conditional 177.7,
SharedCall 23.4, ContextFamily 470.6, Chain64 210.0, SourceSquare 21.4. These do
not show an obvious slowdown, but timing noise/preexisting drift prevents using
this single run as proof of zero identity-bookkeeping overhead.

Verification: 460 tests pass across all targets in debug and release, including
115 fast-engine tests. Eleven new collector tests plus Claude's 17 independent
semantic/reclamation fixtures cover bounded windows, shared pending work,
retained heads, semantic errors, atomic collection failure, stale handles,
relocation-safe cycle identity, and unchanged code images. Claude also reviewed
the collector's root edges and remapping. All-target Clippy with warnings denied,
formatting, and whitespace checks pass.
