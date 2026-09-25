# Conventional spike measurements

Source paths and shell commands in this report are relative to `march6/`.

These are implementation checkpoints, not forecasts of finished-language
performance. The benchmark is `examples/fast_bench.rs`. No interaction-net runner
is used by these measurements.

## Reproduction

From `march6/`:

```sh
timeout --kill-after=2s 30s cargo build --offline --release --example fast_bench
timeout --kill-after=2s 30s taskset -c 0 target/release/examples/fast_bench 100000
timeout --kill-after=2s 30s cargo test --offline --test fast_reference
timeout --kill-after=2s 30s cargo test --offline --release --test fast_reference
```

CPU affinity is optional; select an allowed CPU on the host. The measured host
was an Intel Core Ultra 7 155H, Linux x86-64, Rust 1.90.0, with CPU 0 selected.
Frequency scaling remained enabled and the host was not reserved for this task.
Each benchmark invocation is externally time-bounded; each March invocation has
an independent 10,000-step budget. The executable refuses debug builds and
iteration counts outside 100..=200,000. No dependencies were added.

The later recursion-retention probe uses fixed depths 100, 1,000, and 10,000,
with a separate `100 * depth + 100` step budget, 500,000-cell cap, and
100,000-argument-slot cap. It reports correctness/counters, not throughput.

## Timing scopes

Each execution row warms up with 256 checked invocations, then measures seven
batches. Inputs vary across 127 small integers; the Boolean input alternates.
Inputs and results pass through `std::hint::black_box`. Every timed batch checks
its checksum against equivalent checked-i64 Rust computation outside the timer.
Reported times include the varied-input loop, black boxes, and checksum
accumulation. They are per complete invocation, not per primitive operation.

- **Direct checked Rust:** equivalent numerical computation, without language
  dispatch, argument decoding, runtime bookkeeping, context cloning, or output
  allocation. The Rust compiler may simplify the expression.
- **Fresh Executor + run + drop:** an already-built Program, then a new executor,
  run, result decoding, and executor/result destruction on every invocation.
- **Reused Executor::run_into:** one executor and output buffer reused across
  invocations; reset/context handling remains timed. This is not cross-invocation
  result memoization.
- **Cold Program build + drop:** construction, validation, identity hashing, and
  execution-plan generation, then destruction. SourceSquare additionally includes
  host-side parsing/compilation. This row uses 1,000 iterations per batch when the
  main iteration count is 100,000.
- **Reference CAS cold end-to-end:** a fresh Store, source graph construction,
  hashing/interning, fresh reducer, execution, lookup, and destruction. This
  performs different work from compiled-program execution and must not be
  presented as an evaluator-only comparison. It uses 1,000 iterations per batch.
- **Reference CAS warm reduce:** all 254 integer/Boolean combinations have
  prebuilt input graphs in one Store; immutable result nodes are populated before
  timing. Each invocation creates a fresh reducer and reduces the original input
  graph, so its memo starts empty. Input hashing/construction is excluded, but
  result interning/lookup still occurs. It uses 5,000 iterations per batch. The
  larger retained Store can make this slower than a tiny cold Store.

Contexts for ContextFamily are constructed before timing. Both immutable
contexts are retained, and the varying Boolean selects the context reference for
the corresponding invocation. Runtime context copying/lookup is still timed.

## Workloads

| Workload | Result / intended work |
|---|---|
| Square | `x * x`; one checked multiplication |
| Quad | shared square multiplied by itself; two multiplications |
| Conditional | shared `x * 3`, then only the chosen `+ 1` or `+ 2` |
| SharedCall | call square once, reuse that result in an addition |
| ContextFamily | context `square=true` selects square; otherwise selects `x+x` |
| Chain64 | 64 dependent checked additions of 3 |
| SourceSquare | compile `: square dup * ; square`, then invoke with `x` |

Chain64 measures dispatch amortization for an interpreted instruction stretch;
Rust may simplify the repeated additions. Its direct baseline is not required to
retain 64 machine instructions. ContextFamily, Chain64, and SourceSquare omit CAS
comparisons rather than quietly comparing different source representations.

## Initial checkpoint: before extended workloads / call-plan optimization

Three separate CPU-0-pinned executions, seven batches each, 100,000 invocations
per execution row. The following ranges are the minimum and maximum of the
**three run medians**, not confidence intervals or individual invocation latency.

| Workload | Direct Rust (ns) | Reused executor (ns) | Fresh executor (ns) |
|---|---:|---:|---:|
| Square | 1.2–1.7 | 16.4–16.7 | 50.1–53.5 |
| Quad | 1.2 | 18.3–18.9 | 54.9–55.6 |
| Conditional | 1.3–1.4 | 121.7–122.9 | 224.3–232.5 |
| SharedCall | 1.2 | 131.4–135.2 | 235.4–246.5 |

| Workload | Program build (ns) | CAS cold (µs) | CAS warm (µs) |
|---|---:|---:|---:|
| Square | 274–277 | 1.639–1.667 | 1.669–1.728 |
| Quad | 667–673 | 2.575–2.628 | 2.697–2.753 |
| Conditional | 688–723 | 6.503–6.737 | 6.111–6.187 |
| SharedCall | 673–696 | 5.478–5.527 | 5.024–5.111 |

At this checkpoint, Square and Quad use the compiled straight-line scalar plan;
Conditional and SharedCall use the generic lazy evaluator. Primitive counts are
respectively 1, 2, 2, and 2. Conditional records 15 diagnostic steps and 11 peak
cells; SharedCall records 15 steps, 7 peak cells, and 2 peak frames.

## Extended checkpoint: bounded scalar-call plan inlining

The scalar plan now follows eligible direct calls and output projections, while
preserving unused arguments and distinct invocation instances. This is a compact
interpreted instruction plan: **no JIT, native code generation, or generated Rust
is involved**. The original word identity remains separate from this derived
execution plan. Dynamic selection and context-family dispatch still use the
generic lazy engine.

Three further CPU-0-pinned processes used the same 100,000-invocation/seven-batch
protocol and the same release binary. Timing ranges below again span the three
run medians. The host varied noticeably: Rust and reference-CAS timings also fell
across these runs. Do not attribute every difference from the earlier checkpoint
to a code change; the expanded benchmark also changes surrounding compiled code.

| Workload | Direct Rust (ns) | Reused executor (ns) | Fresh executor (ns) |
|---|---:|---:|---:|
| Square | 1.6–2.1 | 17.1–22.2 | 54.5–70.8 |
| Quad | 1.7–2.2 | 19.5–26.2 | 57.3–76.8 |
| Conditional | 1.7–2.3 | 125.2–176.9 | 230.4–330.7 |
| SharedCall | 1.6–2.2 | 18.5–24.4 | 55.7–76.8 |
| ContextFamily | 1.5–1.9 | 176.8–217.1 | 316.1–399.7 |
| Chain64 | 12.9–15.8 | 168.3–202.0 | 222.9–273.7 |
| SourceSquare | 1.5–1.9 | 16.2–18.6 | 51.5–63.4 |

The important structural result is that SharedCall and SourceSquare now use the
same class of scalar plan as hand-built arithmetic. Source-level factoring need
not introduce a runtime lazy frame for these supported cases. Chain64 amortizes
the invocation overhead to approximately 2.6–3.2 ns per addition in this test,
including invocation scaffolding. That is not an isolated arithmetic-instruction
latency, and it remains slower than optimizing Rust.

### Raw run medians

All entries in this table are **nanoseconds per whole invocation/build**. A dash
means no corresponding baseline was measured. Each cell is the median of seven
batches; the process/run number is retained rather than selecting one best run.

| Run | Workload | Rust | Reused | Fresh | Program build | CAS cold | CAS warm |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | Square | 2.1 | 22.2 | 70.8 | 400.2 | 2301.6 | 2309.6 |
| 1 | Quad | 2.2 | 26.2 | 76.8 | 959.9 | 3625.4 | 3779.2 |
| 1 | Conditional | 2.3 | 176.9 | 330.7 | 1050.0 | 9404.4 | 8270.0 |
| 1 | SharedCall | 2.2 | 24.4 | 76.8 | 1132.8 | 7038.3 | 6687.4 |
| 1 | ContextFamily | 1.9 | 217.1 | 399.7 | 2222.6 | — | — |
| 1 | Chain64 | 15.8 | 202.0 | 273.7 | 4144.2 | — | — |
| 1 | SourceSquare | 1.9 | 18.6 | 63.4 | 1616.1 | — | — |
| 2 | Square | 2.0 | 20.9 | 66.8 | 371.0 | 2174.7 | 2221.0 |
| 2 | Quad | 2.1 | 24.7 | 72.0 | 904.3 | 3413.6 | 3480.6 |
| 2 | Conditional | 2.1 | 152.2 | 288.9 | 937.5 | 8263.5 | 7348.0 |
| 2 | SharedCall | 2.0 | 22.1 | 65.5 | 1015.1 | 6276.4 | 5959.0 |
| 2 | ContextFamily | 1.8 | 194.7 | 360.9 | 2044.5 | — | — |
| 2 | Chain64 | 14.3 | 186.0 | 247.4 | 3714.2 | — | — |
| 2 | SourceSquare | 1.7 | 17.9 | 56.2 | 1513.8 | — | — |
| 3 | Square | 1.6 | 17.1 | 54.5 | 301.8 | 1737.1 | 1786.8 |
| 3 | Quad | 1.7 | 19.5 | 57.3 | 708.7 | 2675.4 | 2811.4 |
| 3 | Conditional | 1.7 | 125.2 | 230.4 | 757.2 | 6721.0 | 6152.9 |
| 3 | SharedCall | 1.6 | 18.5 | 55.7 | 870.0 | 5463.4 | 5140.9 |
| 3 | ContextFamily | 1.5 | 176.8 | 316.1 | 1804.5 | — | — |
| 3 | Chain64 | 12.9 | 168.3 | 222.9 | 3410.2 | — | — |
| 3 | SourceSquare | 1.5 | 16.2 | 51.5 | 1373.5 | — | — |

Final-invocation counters were identical across these runs:

| Workload | Steps | Primitives | Memo hits | Calls | Peak cells | Peak frames | Fast runs |
|---|---:|---:|---:|---:|---:|---:|---:|
| Square | 2 | 1 | 0 | 1 | 0 | 0 | 1 |
| Quad | 3 | 2 | 0 | 1 | 0 | 0 | 1 |
| Conditional | 15 | 2 | 2 | 1 | 11 | 1 | 0 |
| SharedCall | 3 | 2 | 0 | 1 | 0 | 0 | 1 |
| ContextFamily | 16 | 1 | 2 | 4 | 8 | 4 | 0 |
| Chain64 | 66 | 64 | 0 | 1 | 0 | 0 | 1 |
| SourceSquare | 2 | 1 | 0 | 1 | 0 | 0 | 1 |

Calls count execution frames, not source-language call occurrences after
inlining. Zero memo hits on the scalar plan does not mean a shared operand was
recomputed: its register is computed once and reused. The ContextFamily final
sample uses `square=false`, so both guards are consulted before its body runs.
Peak cells/frames describe the generic engine; scalar registers are a separate
allocation and zero here is not a zero-memory claim.

## Final semantic checkpoint: local sharing and recursive-demand guards

This checkpoint supersedes the earlier timings as the current implementation
measurement. Program construction now canonicalizes equal local operations,
including equal direct calls in the same word. The runtime also detects recursive
demands using normalized argument references at words containing recursion,
dynamic application, or family dispatch. These are ordinary runtime hash keys,
not content hashing of every runtime value. The guards and storage limits are
part of the measured implementation, not disabled benchmark options.

Three CPU-0-pinned processes again ran the seven workloads with 100,000
invocations per batch. All checksums passed. Host noise was more pronounced:
even direct Rust ranged from 1.5 to 6.0 ns for tiny scalar workloads, and the
reference baselines moved too. Retain the full ranges rather than claiming a
precise regression ratio from runs taken under different conditions.

| Workload | Direct Rust (ns) | Reused executor (ns) | Fresh executor (ns) |
|---|---:|---:|---:|
| Square | 1.5–4.5 | 16.3–27.8 | 58.4–152.8 |
| Quad | 1.6–2.8 | 19.3–33.9 | 60.5–99.2 |
| Conditional | 1.8–3.0 | 181.2–317.1 | 292.8–491.1 |
| SharedCall | 2.0–3.5 | 23.4–42.3 | 72.3–125.0 |
| ContextFamily | 1.9–3.3 | 524.6–692.1 | 690.2–1130.3 |
| Chain64 | 17.6–22.8 | 243.8–331.2 | 322.4–408.9 |
| SourceSquare | 2.4–6.0 | 24.2–35.0 | 83.7–113.8 |

ContextFamily now includes recursive-demand bookkeeping. Its cost is materially
higher than the earlier version; preserving semantics is not a free optimization.
Straight-line scalar paths still avoid that bookkeeping. Chain64 now measures
approximately 3.8–5.2 ns per addition including invocation overhead, not the
earlier checkpoint's 2.6–3.2 ns range.

### Final raw run medians

All entries are nanoseconds; row scopes and iteration counts are unchanged.

| Run | Workload | Rust | Reused | Fresh | Program build | CAS cold | CAS warm |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | Square | 4.5 | 25.7 | 152.8 | 588.6 | 2388.1 | 2483.0 |
| 1 | Quad | 2.4 | 27.4 | 82.0 | 1421.5 | 3600.7 | 3761.6 |
| 1 | Conditional | 2.2 | 229.1 | 358.9 | 2327.5 | 9294.0 | 8733.4 |
| 1 | SharedCall | 2.4 | 29.3 | 85.4 | 2034.8 | 8006.8 | 7498.1 |
| 1 | ContextFamily | 2.3 | 692.1 | 839.5 | 4526.0 | — | — |
| 1 | Chain64 | 22.8 | 331.2 | 408.9 | 19812.4 | — | — |
| 1 | SourceSquare | 6.0 | 35.0 | 113.8 | 3830.4 | — | — |
| 2 | Square | 4.4 | 27.8 | 93.7 | 715.7 | 2887.6 | 2963.3 |
| 2 | Quad | 2.8 | 33.9 | 99.2 | 1823.8 | 4679.8 | 4871.3 |
| 2 | Conditional | 3.0 | 317.1 | 491.1 | 3312.5 | 13477.4 | 12663.0 |
| 2 | SharedCall | 3.5 | 42.3 | 125.0 | 2978.2 | 11719.9 | 10885.8 |
| 2 | ContextFamily | 3.3 | 685.6 | 1130.3 | 4339.7 | — | — |
| 2 | Chain64 | 21.4 | 270.1 | 371.7 | 15536.1 | — | — |
| 2 | SourceSquare | 2.4 | 25.8 | 83.7 | 2879.3 | — | — |
| 3 | Square | 1.5 | 16.3 | 58.4 | 404.5 | 1655.1 | 1718.7 |
| 3 | Quad | 1.6 | 19.3 | 60.5 | 1033.8 | 2666.8 | 2767.1 |
| 3 | Conditional | 1.8 | 181.2 | 292.8 | 1931.8 | 7714.9 | 7170.7 |
| 3 | SharedCall | 2.0 | 23.4 | 72.3 | 1655.2 | 6706.0 | 6229.5 |
| 3 | ContextFamily | 1.9 | 524.6 | 690.2 | 3497.7 | — | — |
| 3 | Chain64 | 17.6 | 243.8 | 322.4 | 14884.4 | — | — |
| 3 | SourceSquare | 2.4 | 24.2 | 87.3 | 2851.3 | — | — |

Counters match the preceding checkpoint's table except ContextFamily, whose
final invocation now records **14 steps**, not 16. Its primitives (1), memo hits
(2), calls (4), peak cells (8), peak frames (4), and peak argument slots (4) are
unchanged. Conditional has 2 peak argument slots; scalar-plan runs have none.
These diagnostic step counts are not an instruction-level cost model.

### Recursion workspace retention

The benchmark also compiles this tail-recursive source family and invokes its
`countdown` word directly, without a source-root wrapper:

```forth
: zero 0 = ;
: always drop true ;
: base drop 0 ;
: step 1 - recur 1 1 ;
family countdown 1 1 zero base always step ;
```

Each depth used a fresh executor. All three benchmark processes reproduced these
exact counters, and each result was asserted equal to integer zero:

| Input depth | Result | Steps | Primitives | Memo hits | Peak cells | Peak frames / calls | Peak argument slots |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 0 | 2,212 | 201 | 101 | 1,309 | 403 | 403 |
| 1,000 | 0 | 22,012 | 2,001 | 1,001 | 13,009 | 4,003 | 4,003 |
| 10,000 | 0 | 220,012 | 20,001 | 10,001 | 130,009 | 40,003 | 40,003 |

This is linear retention: about 13 cells and 4 frames/argument slots for each
additional countdown level. The evaluator avoids recursive Rust stack growth,
but **does not perform tail-call elimination or reclaim completed frames/cells
within an invocation**. Counters describe stored entries, not resident bytes or
allocator capacity. Reset logically clears the invocation workspace and may keep
vector capacity for reuse; dropping the executor releases its allocations. This
is an explicit remaining limitation for long-running programs and lazy streams,
not evidence that memory management is solved.

## Tail-loop follow-up

[FAST-MEMORY.md](FAST-MEMORY.md) records the next implementation checkpoint:
qualifying scalar contextual tail loops now use constant workspace. The old
retention table above remains the selective-evaluator control, not the optimized
`run` result. The follow-up includes same-code timing, explicit scope, and a
smoke check of the existing workloads. It does not claim general reclamation.

## Interpretation limits

This supports the narrow conclusion that the conventional implementation can
execute these cases much more economically than the previous research runners.
It is not evidence of native-code speed, general strictness analysis, persistent
image throughput, parallel scaling, or bounded memory in a long-lived lazy
application. The tiny direct baselines particularly magnify fixed dispatch and
API overhead. Integer inputs are intentionally small and all arithmetic remains
within i64; overflow/laziness are separate correctness tests.

The CAS baselines are the existing reference implementation, with its own
semantics and data structures. Equal scalar answers on these examples do not
establish whole-language semantic equivalence.
