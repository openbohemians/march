# March 6 documentation

March aims to be a fast, content-addressed, immutable, context-oriented FORTH
with strict stack execution and explicit, typed laziness. The implementation is
in `src/fast/`; the strict baseline is selectable with `--stack`, while the older
lazy evaluator remains the default during this bounded transition.
Interaction nets are no longer an execution direction.

## Current work

1. [Strict stack baseline](STRICT-STACK.md): current direction, runnable first
   slice, transition boundaries, and next steps. [Earlier implementation guide](FAST-SPIKE.md)
   documents the previous lazy-by-default runtime and retained compiler.
2. [Direction](DIRECTION.md): goals/status table, execution decisions, and open contracts.
3. [Bootstrap](BOOTSTRAP.md): the March-defined input-stream interpreter,
   numbers-first lookup, defining words, and the remaining native nucleus.
4. [Measurements](FAST-BENCHMARKS.md): execution baselines and their limitations.
5. [Memory follow-up](FAST-MEMORY.md): bounded scalar tail loops, explicit lazy-heap
   collection, stream measurements, and the remaining continuation/root problem.
6. [Dependency evolution](DEPENDENCY-EVOLUTION.md): initial design for interface
   and implementation locks, reproducible rebuilds, and explicit updates.
7. [Store and state sequencing](STORE.md): Merkle-CHAMP snapshots, evaluated
   writes, implicit runtime state, and the remaining compiler/context boundary.

Source paths and shell commands in these notes are relative to `march6/`,
unless explicitly stated otherwise.

## Reference implementation

The [reference notes](reference/README.md) describe the retained CAS reducer's
seed compiler, reader, reflection, collection, and syntax. They remain useful
implementation evidence, but do not specify `march-fast`. Keeping that
distinction explicit matters: staging, live images, constructor demand, and
source syntax differ between the two implementations.

## Retired research and recovery

Superseded INet designs, probe reports, research plans, and old status documents
were removed from the working tree after a documentation-only checkpoint:
**`6f17b9b`**. The original, longer bootstrap note is also preserved there.
No implementation code or tests were removed by this cleanup.

From the repository root, inspect an old note without restoring it:

```sh
git show 6f17b9b:march6/INET-DEMAND.md
git show 6f17b9b:march6/BOOTSTRAP.md
git ls-tree --name-only 6f17b9b march6/
```

That checkpoint includes the previously uncommitted net notes and the latest
edits to existing notes. Earlier lineage histories and their archival branch
names are documented in [VERSIONS.md](../../VERSIONS.md). The repository-level
`doc/` audit/design material and March 1–5 directories were left untouched.
