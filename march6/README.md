# March research prototype

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
and source-defined parsing aliases work without changing the nucleus.
