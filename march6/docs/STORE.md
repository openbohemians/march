# Evaluated state and the Merkle-CHAMP store

**Transition note, 2026-09-27:** the store's evaluated immutable value boundary
remains. The [strict stack baseline](STRICT-STACK.md) sequences its supported
reads/writes directly in program order, without the dependency chain described
below. That chain documents the previous lazy runtime, still the CLI default
until the bounded stack migration is ready. Compiler/store unification is on hold.
The strict engine now supports tuple/text values and tuple namespace paths too;
its owned `stack::Value` representation converts to/from the same frozen value
format without using any old evaluator handles. See the strict guide's data
checkpoint for sharing, allocation costs, and transfer limits.

The first store slice uses the published `merkle-champ = "=0.1.0"` crate.
It now has **implicit invocation-state threading** and March `store.get` /
`store.put` words as well as a host API. There is no store-image format yet.
"Persistent" here means
updates preserve earlier immutable snapshots, not that snapshots are saved to
disk. The compiler dictionary remains on `imbl`, separate from this store until
the state/compiler unification step. A measured migration was not retained:
that lookup-only container does not yet benefit from Merkle identities.

## Settled semantic boundary

The store is evaluated state. A write recursively demands the value, including
all tuple fields. Implicit pending calculations cannot enter the store.
Explicit deferred representations stop that traversal: a stored quotation is
code, and its body is not executed just because the quotation is stored.

The current implementation supports the existing closed quotations as that
boundary. It does not add closures, captured environments, or a new general
thunk-wrapper syntax. The internal shared demand cells still serve ordinary
lazy execution, but are never themselves stored.

A failing, divergent, over-budget, or oversized value does not publish a new
snapshot. Work already performed may remain memoized in the evaluator; failure
atomicity applies to state publication, not undoing internal evaluation.
Infinite data cannot be materialized as ordinary state within finite limits.
Compiler-state capabilities are rejected, including inside tuples.

## Language-level sequencing

| Word | Ordinary stack interface | Behavior |
|---|---|---|
| `store.get` | `( path -- value )` | Lazy read of the snapshot preceding this operation. Missing entries fail when demanded. |
| `store.put` | `( value path -- )` | Validate the path, deeply evaluate data, and produce the next implicit snapshot. |

A path is a text value for one exact component, or a nonempty tuple of text
components. `"a.b"` remains one component; `"a" "b" tuple 2` is two. No store
token or state argument is placed on the ordinary March stack.

```forth
: bump "counter" store.get 1 + "counter" store.put ;
10 "counter" store.put
"counter" store.get
bump bump
"counter" store.get
-- results: 10, 12
```

Run this with `cargo run --bin march-fast -- examples/fast/state.march`.

The evaluator keeps an invocation-local chain of state dependencies alongside
the existing lazy value cells. Each operation records its preceding state edge.
Reads therefore retain their snapshot even if a later read is demanded first.
The final state is a separate output root: dropping all ordinary results does
not drop writes. Pure unused computations and unused reads remain lazy.
Successive writes are sequenced and deeply evaluated even when a later write
overwrites the same key. This preserves failures and termination behavior.

Ordinary static calls (including `call` on a known quotation), selected guarded
family bodies, and recursion propagate their state outputs. Only selected
family bodies execute writes. In contrast, `select` chooses between values;
it does not retroactively cancel writes of words already composed before it.
For example, if `a` and `b` write state and return a value, `true a b select`
keeps `a`'s value but still sequences both words' writes. Use guarded families
for conditional stateful execution in this first slice.

The current dynamic `apply N M` contract is **non-writing**. A demanded writing
target is rejected; an undemanded pure target is not forced merely to discover
effects. Consequently this is not yet general higher-order effect typing.
Effectful dynamic quotation application needs an explicit checked contract in
a follow-up. Guards cannot write state; direct guard recursion into a writing
family is also rejected. Runtime context (`ctx`) is still the separate immutable
invocation context, not yet a view into the store.

Runtime store operations are unavailable during immediate/compiler execution
until compiler state and runtime state are unified. Known reads/writes are
rejected before executing an immediate word; indirectly demanded store operations
are also rejected. Merely compiling those operations into runtime words is fine.

`Program::effects` exposes a derived `{ reads, writes, dynamic }` summary.
`dynamic` conservatively marks applications that may read state through an
unknown non-writing target, including through wrappers. These calls and all
state-sensitive operations are excluded from inappropriate common-subexpression
merging. Summaries are regenerated from canonical definitions on image load;
they are not a separate source of code identity or a complete type/effect system.
Stateful recursion's demand-cycle key includes its input state edge.

## Invocation boundaries

- `start_with_store(..., &initial)` returns lazy stack-result handles.
- `finish_state()` completes and returns the invocation's state output, even
  when the ordinary result stack is empty. Calling it again reuses the result.
- `run_with_store(..., &initial)` observes ordinary results and completes state,
  returning `(results, final_store)` only on success. The caller's initial
  snapshot is never modified, including if a later write fails after earlier
  internal transitions succeeded.
- Ordinary `run`/CLI calls containing statically known store operations start
  with an empty store and complete writes before returning; the CLI does not yet
  preserve that final store between runs. For dynamically supplied read-only
  quotations that use the store, use the explicit host state-invocation API.

Separate observations are separate commitments: after a host accepts a snapshot
from `finish_state`, a later demand of a lazy stack result can still fail.
`finish_state` does not claim to evaluate all those results. Any evaluator-task
error in a stateful invocation prevents a subsequent successful `finish_state`;
starting a new invocation resets that failure.

Writes use resumable tasks on the same evaluator work stack, not nested calls
to public `force`. Deep freezing is bounded by invocation fuel and storage caps;
ready stored data is imported into ordinary runtime cells on demand.

## Implemented API

`src/fast/store.rs` provides the underlying host operations:

- `Executor::freeze(handle) -> FrozenValue`: a bounded deep observation that
  exports a detached value. It consumes invocation fuel and checks cell, tuple
  field, and text-byte limits. Resource failures abort the invocation, just as
  other bounded observations do.
- `Executor::store_put(&store, path, handle) -> Store`: validates the path before
  demanding the value, freezes it completely, then returns a new snapshot.
- `Store::with(path, frozen_value) -> Store`: insert previously frozen data.
- `Store::get(path)`, `namespace(path)`, and `names()`: inspect snapshots.
- `Store::without(path) -> Store`: remove a value, pruning empty ancestors.
- `Store::cid()`: identify a snapshot without invoking the evaluator.
- `FrozenValue::to_input_graph(&program)`: prepare ordinary structured inputs
  for another invocation, resolving quotation CIDs against that exact Program.
  Missing code is an error, never a substitution by current name.

Paths are explicit arrays of exact UTF-8 components. `["a.b"]` and `["a", "b"]`
are different paths. Source-level qualification/import syntax is not decided
by this host representation, and existing dotted dictionary names retain their
current meaning. Components are not Unicode-normalized.

A value and a namespace cannot occupy the same path. Conflicts are errors;
creating a deeper path does not overwrite a prefix value, and inserting a
value does not discard a subtree. Missing namespaces are created on insertion.
Namespace deletion is not implicit. Paths have at most 64 components, each
nonempty and at most 4096 bytes, with at most 65536 bytes total. Empty paths
are allowed only for looking up the root namespace. `names()` returns direct
children in deterministic trie order, not lexical order.

Frozen values own a flat, topologically ordered DAG of integers, booleans,
unit, exact text bytes, quotation CIDs, and tuple edges. The root is last.
Export preserves shared fields; neither native recursion nor a recursively
owned tuple tree is required, including during destruction. Runtime handles,
WordIds, memo state, and allocation addresses do not enter stored identity.

Run the end-to-end host example from `march6/`:

```sh
cargo run --locked --offline --example fast_store
```

It evaluates a tuple containing `42` and an explicit quotation, stores it,
destroys the evaluator, imports the frozen value into another invocation,
checks its identity, and removes the entry without changing the old snapshot.

## Identity contract

Frozen value CIDs use the existing `march-fast-value-v1` encoding, matching
`Executor::content_id`. Tuple/pair equivalence, exact text bytes, and quotation
code identities remain unchanged. DAG sharing is not part of value identity.
Frozen value equality uses CID equality under the usual CAS collision
assumption, not the incidental exported graph layout.

Store identity adds a separate layer:

- Namespace maps use `merkle-champ` 0.1.0's format-v1 canonical structure and
  deterministic `String` placement hashing/key encoding.
- A value entry feeds byte `0`, the ASCII domain `march/frozen-value/v1`,
  and the value's 32 CID bytes to the node hash.
- A namespace entry feeds byte `1` and the child store's 32 CID bytes.
- A store CID is `Cid::digest(b"march-fast-store-v1", &map.identity())`, using
  March's domain/length framing. Child namespaces use the same rule.

CHAMP caches subtree hashes; the small March store wrapper hash is recomputed
when requested. Changing placement, canonical tree rules, entry encoding, or
value semantics requires reviewing the identity-format version, independently
of Rust API compatibility. The dependency is pinned deliberately.

No store identity is currently folded into word CIDs or required per execution.
Code definitions retain their composed-definition identity. Current F05 code
images still sort names/flags explicitly and do not include runtime store data.

## Remaining work and limits

- Unify code bindings, data entries, contextual state, and compiler state while
  retaining binding metadata, exact compiled references, and protection rules.
- Expand the initial read/write words with imports, deletion, protection rules,
  contextual reads, and effectful dynamic-call contracts.
- Define explicit thunk wrappers beyond existing closed quotations if needed.
- Add store images and code-retention roots. A stored Quote CID currently does
  not retain the referenced Program or automatically include it in a code image.
- Add project resolution snapshots and update tooling as described in
  [DEPENDENCY-EVOLUTION.md](DEPENDENCY-EVOLUTION.md).
- Account for total store size/retained history as runtime writes expand.
  Current limits bound individual exports and the number of local state versions,
  not aggregate retained payload bytes or process RSS. State versions remain
  retained until invocation reset. Stateful sessions reject `collect` with
  `CollectionBusy` until state-edge/root tracing is implemented; they must not
  silently drop required writes or historical read snapshots. Pure sessions keep
  the existing collector. `storage()` reports state versions/link slots and
  structural vector bytes, excluding shared persistent-store payload allocations.
  Host input reconstruction also allocates a new input graph.
- Published 0.1.0 lookup takes `&String`, so current `&str` APIs allocate a
  temporary key for store paths. The dictionary probe measures this cost
  separately, and the existing compiler dictionary remains on `imbl` rather than
  taking this regression without an identity benefit. Other Program fields
  still copy during compilation.
- Deterministic noncryptographic placement hashing retains the library's
  adversarial-key limitation. This is not a hardened untrusted-input service.

`imbl` remains the dependency for the current compiler dictionary.
`ARRAY-FAMILIES.md` is a separate design draft, not implemented by this work.
