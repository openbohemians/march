# March 6

A working prototype of a fast, content-addressed, immutable, lazy,
context-oriented FORTH. The active runtime is a conventional engine;
interaction-net development is retired.

## Run it

From this directory:

```sh
cargo run --offline --release --bin march-fast -- --eval ': square dup * ; 7 square'
cargo run --offline --release --bin march-fast -- examples/fast/lazy-stream.march
cargo run --offline --release --bin march-fast -- --context enabled=true examples/fast/context.march
```

The prototype supports shared lazy evaluation, immutable pairs, closed
quotations, recursion, guarded word families, and canonical code images.
General memory reclamation, full contextual modules, staging, and self-hosting
remain unfinished; this is not a complete March implementation.

## Documentation and checks

Start with the [documentation index](docs/README.md) or the
[implementation guide](docs/FAST-SPIKE.md).
[Measurements](docs/FAST-BENCHMARKS.md) and the
[memory follow-up](docs/FAST-MEMORY.md) record what has actually been tested.

```sh
cargo test --offline --all-targets
cargo test --offline --release --all-targets
cargo clippy --offline --all-targets -- -D warnings
```

The older CAS reference implementation and experimental code/tests are retained.
Its [reference notes](docs/reference/README.md) are separate from the active
documentation. Superseded research notes are preserved in Git; the documentation
index explains [how to recover them](docs/README.md#retired-research-and-recovery).
