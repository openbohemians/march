# March research prototype

**Active direction (2026-09-25):** a fast, content-addressed, immutable, lazy,
context-oriented FORTH using a conventional execution engine. Interaction-net
development is retired. See [DIRECTION.md](DIRECTION.md) for the current plan;
the overview and research links below describe retained prototypes, not active
INet milestones. The existing CAS reducer remains useful evidence, not a
promise of fast execution or a mandatory backend representation.

**Working conventional spike:** [FAST-SPIKE.md](FAST-SPIKE.md) has runnable
examples, current semantics, and known gaps. [FAST-BENCHMARKS.md](FAST-BENCHMARKS.md)
has measured execution costs and memory retention. Quick start:

```sh
cargo run --offline --release --bin march-fast -- --eval ': square dup * ; 7 square'
```

## Retained reference and research overview

This directory is a deliberately small experimental successor to March 4 and
March 5. It now includes a small executable seed language, not a complete March
implementation. Its purpose is to test the
claims that would make a new March architecture worthwhile.

The prototype keeps three ideas together:

1. Programs, contexts, residual programs, and immutable values are canonical
   content-addressed graphs.
2. Compilation and execution use the same reducer.  Compilation supplies the
   facts known now and persists the residual graph; execution supplies the
   facts learned later and continues reducing it.
3. An optional use-topology analysis predicts exact last-use reclamation where
   it can prove future wire fate, while reporting rather than hiding the cases
   that require ordinary runtime reclamation.

The immutable term/DAG reducer is the semantic control experiment.  A second,
small backend now uses real agents, ports, principal-principal active pairs,
and local rewrites.  The semantic graph now includes ordered guarded definition
families, compact residual dispatch, and recursion.  A deliberately narrow
interaction-net `Call` lowering exercises that model without allocating
rejected clauses.  It is still too small to settle the interaction-net
question, but it no longer uses “INet” as a name for an ordinary DAG.
The reference reducer itself is an explicit, budgeted work-list machine: deep
source recursion and structural graph traversals do not consume the Rust call
stack.

Run everything offline:

```sh
cargo test --offline
cargo run --offline -- demo
cargo run --offline -- eval 'square : ( dup * ) ; 7 square'
cargo run --offline -- eval-forth ': square dup * ; 7 square'
```

See [RESEARCH.md](RESEARCH.md) for the hypotheses, invariants, measurements,
and stop conditions.  [RESULTS.md](RESULTS.md) records what the current evidence
does and does not establish.  [DECISION.md](DECISION.md) gives the resulting
go/conditional-go decision and the next falsification gate.  [MODEL.md](MODEL.md)
states the emerging FORTH-inspired, self-extending language model and separates
essential net topology from optional teleological topology analysis.
[BOOTSTRAP.md](BOOTSTRAP.md) turns the self-extension claim into the B0 gate:
a syntax-free nucleus must load a seed image whose March-defined `:` and `;`
can extend that same image reproducibly.
[REFLECTION.md](REFLECTION.md) records the implemented B0a boundary: ordinary
March records describe code, and a strict, budgeted, syntax-neutral reducer
operation interns only validated closed quotations and families.
[READER.md](READER.md) records the B0b token/dictionary foundation and
graph-defined reader resume tests. [SYNTAX.md](SYNTAX.md) fixes provisional
name-first spellings. [SEED.md](SEED.md) describes the executable B0c subset:
the seed compiler lives in image graphs, both surfaces compile the same square,
and source-defined parsing aliases work without changing the nucleus. B0d
aligns inline and factored expression demand, with explicit observation at
constant binding and successful EOF; pauses and images preserve pending work.
[COLLECTION.md](COLLECTION.md) describes explicit-root reclamation for the CAS
store; the CLI collects intermediate history between token batches.
[NEXT-PHASE.md](NEXT-PHASE.md) tracks the next reference-core and interaction-net
milestones, task ownership, and review gates. [DEMAND-CONTRACT.md](DEMAND-CONTRACT.md)
records the first executable demand fixtures and isolated protocol-probe scope.
[DEMAND-COMPARISON.md](DEMAND-COMPARISON.md) compares the integrated A/B/C probes,
their remaining costs, and eager evaluation.
[N0-PROBE.md](N0-PROBE.md) records the experimental closed-code/application
probe, including the canonical-sharing and cycle-detection gaps found in review.
