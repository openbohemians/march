# March 6 documentation

March is a fast, content-addressed, immutable, lazy, context-oriented FORTH.
The active implementation is the conventional engine in `src/fast/`.
Interaction nets are no longer an execution direction.

## Current work

1. [Implementation guide](FAST-SPIKE.md): run the language, understand its
   semantics, and check what is still missing.
2. [Direction](DIRECTION.md): language goals, execution decisions, and open contracts.
3. [Bootstrap goals](BOOTSTRAP.md): FORTH-style self-extension, context-driven
   compilation, and what remains to be built.
4. [Measurements](FAST-BENCHMARKS.md): execution baselines and their limitations.
5. [Memory follow-up](FAST-MEMORY.md): bounded scalar tail loops and the remaining
   general reclamation problem.

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
