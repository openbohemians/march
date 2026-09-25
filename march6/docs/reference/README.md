# Retained CAS reference implementation

These notes document code that remains available as a semantic control and as
evidence for future bootstrap work. They are **not the current language or
runtime specification**. In particular, the old seed's syntax, strict data
constructors, staged unknowns, and live graph images differ from `march-fast`.

- [Seed compiler](SEED.md): graph-defined syntax handlers and demand boundaries.
- [Reader](READER.md): syntax-neutral token and dictionary operations.
- [Reflection](REFLECTION.md): validated construction of closed code from data.
- [Collection](COLLECTION.md): explicit-root reclamation for the CAS store.
- [Seed syntax](SYNTAX.md): the provisional surfaces accepted by that seed.

From `march6/`, the retained executable is available explicitly:

```sh
cargo run --offline --bin march-research -- eval-forth ': square dup * ; 7 square'
```

For current work, start with the [documentation index](../README.md).
Retired INet and research-plan notes are recoverable from Git, as described there.
