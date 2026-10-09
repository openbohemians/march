# March8: March's compiler in Rust

March8 compiles March in Rust: a symbolic stack machine runs March's
explicit form on judgments, a type and maybe a known value for each stack
item, and leaves bytecode for the machine (docs/MACHINE.md). System March
(march7) is frozen. March will compile itself again once it is mature.

It depends on merkle-champ (github.com/tabcomputing/merkle-champ) by path
until its version 0.2 is published, so check it out next to this repository:
`../../merkle-champ` from here.

```sh
cargo run --offline -- --eval '[ < money -- money > 1.10 +. ] fee def. 19.99 fee.'
# <1> 21.09
cargo run --offline -- --eval '[ dup. *. ] sq def. 2.5 sq. 3 sq.'
# <2> 6.25 9
cargo run --offline            # a REPL; :help for its commands
```

| What | Where |
|---|---|
| The symbolic stack machine | `src/stage.rs`, docs/MACHINE.md |
| Types as data | `src/types.rs` |
| Primitives | `src/prims.rs` |
| The core vocabulary, in March | `core/core.march` |
| The reader | `src/read.rs` |
| The machine and its code | `src/machine.rs`, `src/code.rs` (from march7) |
| The design | `../doc/design/TYPES.md` |
