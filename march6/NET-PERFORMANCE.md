# Contextual-net runner: first performance measurements

Measured 2026-09-25 at Thomas's request. This measures the current experimental
Join/selector runner, not the reference evaluator, an optimized INet backend,
or March as a whole. No language or reduction rules were changed.

## Workload and method

```text
shared = x * 3
result = if condition then shared + 1 else shared + 2
```

Each graph completes in 18 interactions, with one multiplication, one addition,
and one live Answer. Inputs vary at runtime (`x` in 1..997 and alternating Bool)
and pass through `black_box`. The native baseline uses the equivalent checked
integer arithmetic in a non-inlined function; its invocation also passes through
a black-boxed function pointer. Batch checksums are verified after timing.
An input/call control provides context for the very small native measurement;
it is NOT subtracted from the results.

- Intel Core Ultra 7 155H, Linux x86_64; pinned to logical CPU 0 with `taskset`.
- Rust 1.90.0, Cargo release profile, offline, single-threaded.
- Seven measured batches per row plus one warm-up batch.
- 1,024 graphs per regular batch; 128 per passive-padding batch; 1,000,000
  native/control calls per batch.
- Every net run capped at 128 interactions; each whole benchmark invocation
  externally capped at 30 seconds. All runs completed, no background process.
- Three process runs. Run 1 predates the construction-only row; runs 2 and 3
  use the final same binary. No fixed CPU-frequency setting, isolated core,
  randomized measurement ordering, or hardware-counter profiling. Between-run
  variation is material; do not treat the decimal places as stable accuracy.

`Full` is the unchanged default behavior: formatted trace, rule validation,
whole-graph audits, and local assertions. `NoTrace` removes only formatted trace
construction/retention. `Lean` additionally skips rule validation and whole-graph
audits during reduction. All three retain rule-name history, local wiring
assertions, matching, node/vector allocation, graph scanning, and actual erasure.
Lean is an instrumentation ablation, NOT an optimized runtime.

Build+reduce+drop includes graph construction, invocation, construction audits,
reduction, result extraction, and destruction of retained history. Reduction-only
prebuilds a batch and excludes its construction, final verification audits, and
destruction; it still includes result extraction and the halt check. These scopes
have different allocator/cache behavior, so their difference is not an exact
measurement of construction time. Construction+drop is measured separately.

## Results

All entries below are medians, in **microseconds per expression**, except the
explicit native/control nanosecond rows.

| Measurement | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Native checked arithmetic, **ns** | 2.5 | 1.7 | 1.7 |
| Input/call control, **ns** | 2.0 | 1.3 | 1.3 |
| Construction+drop only | — | 0.717 | 0.682 |
| Full / lowest-ID / build+reduce+drop | 21.686 | 14.893 | 14.583 |
| Full / lowest-ID / reduction-only | 21.416 | 13.983 | 13.649 |
| NoTrace / lowest-ID / build+reduce+drop | 9.664 | 6.132 | 6.020 |
| NoTrace / lowest-ID / reduction-only | 9.005 | 5.451 | 5.477 |
| Lean / lowest-ID / build+reduce+drop | 6.848 | 4.405 | 4.235 |
| Lean / lowest-ID / reduction-only | 5.692 | 3.640 | 3.453 |
| Full / highest-ID / build+reduce+drop | 25.173 | 15.551 | 14.192 |
| Full / highest-ID / reduction-only | 23.370 | 14.793 | 13.452 |
| NoTrace / highest-ID / build+reduce+drop | 11.174 | 6.800 | 6.055 |
| NoTrace / highest-ID / reduction-only | 9.872 | 6.172 | 5.457 |
| Lean / highest-ID / build+reduce+drop | 7.442 | 5.100 | 4.297 |
| Lean / highest-ID / reduction-only | 6.574 | 4.056 | 3.624 |

Example within-run variation: run 3 Full/lowest-ID/end-to-end spans
14.363–15.026 microseconds across seven batches; Lean spans 4.184–4.332.
Between-process variation exceeds those within-run ranges.

### Sensitivity to unrelated passive graph size

Same 18 reductions, Lean/lowest-ID/reduction-only; add inert Int agents each
connected to a free boundary. They never reduce. Padding construction is outside
the timer; padding occupies real graph storage and is scanned by the runner.

| Extra passive agents | Run 1 (us) | Run 2 (us) | Run 3 (us) |
| --- | ---: | ---: | ---: |
| 0 | 6.517 | 4.021 | 3.647 |
| 100 | 11.954 | 7.411 | 6.734 |
| 1,000 | 80.524 | 53.507 | 47.772 |

The 1,000-agent padding costs approximately **12–13 times** the unpadded case.
The runner discovers active pairs by scanning allocated slots each interaction;
this measurement also includes the larger memory/cache footprint. It is not a
property of the local reduction rules that passive components must be rescanned.

### Retained diagnostic data

For the first input, all modes finish with one live Answer and 37 allocated
agent slots (36 tombstones). Full additionally retains 18 trace strings totaling
1,839 text bytes, excluding their spare capacity/metadata and other allocations.
NoTrace/Lean retain no trace strings. All retain 18 rule-name entries. These
are logical/storage counts, not peak-memory or allocation-traffic measurements.

## Interpretation

1. The current executor is very expensive for tiny arithmetic. Lowest-ID
   end-to-end Full is roughly 9,000 times the native microbenchmark, and Lean
   roughly 2,500–2,800 times, using each process run's own native baseline.
   Ratios are approximate because the native case is near call/loop overhead.
2. Removing tracing and global diagnostic checks improves end-to-end cost by
   about 3–3.5 times. Diagnostics explain a large part, but not the remaining
   thousands-fold gap for this primitive-heavy workload.
3. The remaining gap is NOT a measured lower bound on interaction nets. It
   includes generic rule construction, numerous small vector allocations,
   pointer/index manipulation, local checks, retained rule history, and a
   whole-graph scheduler scan. We have not apportioned those costs individually.
4. One tiny conditional does not establish performance on expensive shared
   computations, avoided work, recursive structures, parallel execution, or
   compiled/staged regions. There is no evidence here for a future native-speed
   March implementation either.

Recommendation: pause additional feature layering for a bounded performance
experiment using a ready-pair worklist and compact/reused storage under the same
rules. Then separately test whether a predictable pure region can lower to
direct code. Preserve the diagnostic runner as an oracle for that work; do not
present queue/storage improvements as sufficient to close the native gap.

## Reproduction and verification

From `march6/`:

```sh
timeout --kill-after=2s 30s cargo build --offline --release --example demand_join
timeout --kill-after=2s 30s taskset -c 0 target/release/examples/demand_join --bench
timeout --kill-after=2s 30s cargo test --offline --example demand_join --example contextual_net --example selective_activation
timeout --kill-after=2s 30s cargo test --offline --release --example demand_join --example contextual_net --example selective_activation
```

The harness is `examples/support/demand_bench.rs`. It refuses a debug build.
The two new tests compare diagnostic modes' rule sequences and full residual
graphs across finite/unknown/Spin producers, conditions, invocation states and
schedules, and check varying numeric inputs against direct arithmetic. All 47
example tests pass in debug and release. Example-binary Clippy with warnings
denied, formatting, and diff whitespace checks pass. Production/reference tests
were not rerun; no production source was changed. Independent benchmark review
has been requested from Claude.
