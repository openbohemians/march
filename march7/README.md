# March7: first system-FORTH slice

A small cell machine running a March-written input reader and compiler. The
first slice works; this is **not yet the complete bootstrap or Checked March**.

It depends on merkle-champ (github.com/tabcomputing/merkle-champ) by path
until its version 0.2 is published, so check it out next to this repository:
`../../merkle-champ` from here.

From this directory:

```sh
cargo run --offline --bin march7-seed -- seed/system.asm seed.image
cargo run --offline -- seed.image --eval ': square dup u* ; 7 square'
# <1> 49
cargo run --offline -- seed.image examples/compiler.march --save example.image
# <2> 49 42
cargo run --offline -- example.image --eval 'define cube dup dup u* u* ; 3 cube'
# <1> 27
```

The command line prints the stack after its depth. A rebuilt system
(docs/REBUILD.md) tells it the values' types, so floats and arrays print as
March writes them (`<1> ( 2.5 3.5 )`); generation 0 does not, so it prints
integers.

The first command is the separate generation-zero build tool. Normal execution
loads an image; it does not invoke the assembler or contain a source compiler.
`seed.image` is a generated, ignored build artifact. Other output paths are
explicitly chosen by the caller; do not overwrite images you need to retain.

## What runs where

- [The machine](src/machine.rs): raw cells, checked memory, calls and primitives.
- [The system words](seed/system.asm): WORD, number conversion, dictionary lookup,
  `:`, `,`, `;`, immediate words, failed-definition recovery and dictionary
  serialization. These are March instruction sequences, initially expressed as
  a flat listing because no March reader exists before the first boot. This is
  not x86 assembly and contains no Rust source handlers.
- [A source-level extension](examples/compiler.march): March defines `define`
  and a compiling word that calculates and emits the literal 42.
- [The assembler](tools/assembler.rs): instructions, labels, data bytes and CID
  references only. Its module is absent from the normal runtime library/binary.
- [The driver](src/driver.rs): passes input bytes to execution tokens returned by
  the image's boot word. It never looks up names or interprets dictionary records.

The tests also rebuild `:` and `literal` from March source with unchanged CIDs
across three loads. That is two compiler words, **not a self-rebuild of the whole
interpreter**. Repeated unchanged image exports are byte-identical; recompiling
definitions currently adds dictionary history and need not produce identical
whole images.

## Validation

```sh
timeout 60 cargo test --offline
timeout 60 cargo test --offline --release
cargo clippy --offline --all-targets -- -D warnings
timeout 60 cargo run --offline --release --example bench
```

Integration tests cover self-extension, content identity, early binding,
numbers-first lookup, immutable data references, image reload, failure recovery,
independent compiler instances, resource limits and execution without the
assembler on PATH or in the working directory.

The benchmark separates load/link/boot, source compilation and warm execution.
It is a tiny unpinned measurement, not a language-wide performance comparison.

## Boundaries still visible

- Strict **System March** uses unsigned 64-bit cells. Arithmetic words are
  `u+`, `u-`, `u*`, `u/`, `umod`, and `ult?`; addition, subtraction
  and multiplication wrap. Equality remains `eq?`. Negative literals encode
  two's-complement bits, not a signed runtime type; the CLI prints unsigned cells.
  The familiar `+`, `-`, `*`, `/`, `mod`, `lt?` are not aliases. Checked
  application arithmetic/types, guards, collections, laziness and native code
  generation are not implemented.
- The driver accepts complete source inputs. A multiline interactive terminal
  reader, editor/source-record database and dependency-driven incremental build
  scheduler are not implemented.
- Errors abandon the current unfinished definition. Earlier completed words
  remain; arbitrary immediate-word memory effects are not transactional.
- The current seed uses private scratch cells within one running compiler.
  Separate machines can compile against the same image independently. There is
  no concurrent or reentrant compiler sharing one scratch area.
- Workspace and code-object growth are bounded but not production reclamation.
  Image export currently retains all CAS objects, including superseded ones.
- Quoted code is supported; source string-literal syntax and general I/O words
  are not yet supplied, although immutable data operands and host file I/O work.

See [Foundation contract](docs/FOUNDATION.md),
[first-slice interfaces and reuse](docs/FIRST-SLICE.md), and the
[surface language design](docs/SURFACE.md).
