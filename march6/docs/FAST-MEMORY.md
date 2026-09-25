# Tail-loop memory follow-up — 2026-09-25

Source paths and shell commands in this report are relative to `march6/`.

The first memory improvement is conservative tail-call elimination, not a
collector. Exact eligibility and remaining limitations are in
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

This does **not** bound general non-tail recursion or live lazy structures.
Their existing selective evaluator and retention behavior are unchanged.

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
