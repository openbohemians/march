# Explicit-root CAS collection

Reference implementation only: this describes the retained CAS store/reducer,
not the conventional `march-fast` runtime. See the [reference index](README.md).
Source paths and commands are relative to `march6/`.

The reference store now supports `Store::collect(&roots)`. This is a host-side
safe-point operation, not a new graph node, reduction rule, or image version.
It removes obsolete intermediate compiler/evaluation graphs from memory. It
does not change the separate INet cell-reuse machinery or static memory planner.

## Contract

The caller supplies the complete set of node roots it needs to keep. The
collector iteratively marks all structural descendants, including dormant
quotation/family bodies, unselected branches, pending expressions, and shared
data. It does not evaluate code. After validating the reachable graph, it
sweeps unmarked nodes in place and prunes their code-validation cache entries.
A missing root or reachable child returns an error before any mutation. An
empty root list intentionally releases the whole store.

This is not automatic root registration. CIDs are identifiers, not owning
handles. Keep roots for other sessions, saved in-memory states, host-held
values, binding values, and cache residuals if they must remain usable.
`SpecializationCache::roots()` exposes cached residual node roots; its cache
keys and other artifact hashes are not node roots. Binding values are available
through `Bindings::0`. Fully serialized images are independent of the store.

For example, an application with a live runner/state plus a retained old state
must collect with all three roots. Keeping only the current state intentionally
invalidates unreferenced historical CIDs. Reinserting identical content later
produces the same CID, but a CID alone does not reconstruct the deleted content.

Collection requires exclusive access to the store and runs between reductions,
after the reducer's frames and per-run memo tables have gone away. It does not
collect inside an active reduction or treat memo tables as permanent roots.
It has no host-resource finalization semantics: effect tokens remain explicit
language values, and the collector does not execute effects or close handles.

## CLI policy

`eval` and `eval-forth` now resume the seed in batches of 64 tokens, retaining
the runner and current state after every successful reduction. All roots the
CLI needs are reachable from those two. The 20-million-step reduction budget
is shared across all batches, not renewed per batch.

The adapter checks explicit reader errors and finishes only after a
positive-quota resume returns the same successful state. With this seed,
token processing advances the cursor, so this fixed point certifies EOF
handling and observation. Merely reaching the last token's byte offset does
not: the next resume may still demand a result and report overflow. No source
token parsing or arithmetic has moved into the host adapter.

CLI output reports collection epochs, cumulative reclaimed nodes, peak store
node count at batch boundaries, and final retained nodes. Individual reduction
runs normally grow the store, but these metrics are not peak allocated bytes
or RSS; reflection rollback and temporary host structures are not accounted for.

## Evidence and limits

The 64-call/drop fixture retains about 19,400 nodes without collection. With
16-token collection batches, its peak is 1,936 nodes and its final live store
is 348 nodes, with identical final image bytes. Unlike the older checkpoint
control, the collection loop performs no image serialization or loading.
With the CLI's default 64-token batch, this source peaks at 6,528 stored nodes
and ends with the same 348, across five collection epochs. It uses 58,024
charged reduction steps; this excludes collection and image-output work.

Tests cover missing-reference atomicity, validation metadata, duplicate roots,
shared data, dormant code, deep graphs, cache/binding roots, staged execution,
every token boundary in both seed syntaxes, and pending/demanded overflow.
Driver tests cover EOF, zero quotas, and the cumulative reduction budget.

This is a basic stop-the-world tracing baseline. Marking uses temporary host
memory and sweeping scans the store. Collection work is separate from reducer
fuel; neither token batching nor that fuel is a total host-memory/time bound.
A single batch can still allocate substantially, and genuinely live graphs
can grow indefinitely. Dropped allocations may be reused by the host allocator
without an immediate fall in RSS. Incremental collection, transient versus
persistent storage separation, and topology-planned reclamation remain open.
