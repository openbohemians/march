# The system layer rebuilds itself

2026-09-29. Work by Claude, done in a copy of Codex's first slice that has since
replaced it as `march7/`.

## Result

`seed/system.march` defines the whole March7 system layer in March source.
That includes the reader, the numbers-first outer interpreter, the dictionary,
`:` and `;`, immediate words, literals, quotations, `recur`, comments, recovery,
dictionary save and restore, the generation-zero dictionary, and boot. It also
defines control flow (`if` `else` `then` `cycle` `while` `repeat` `until` `again`
`exit`), `[` `]`, and `s"` data literals. The instruction listing has none of these.

Compiling the file leaves the token of its `boot` word on the stack. The host
exports an image whose entry is that word.

| Generation | Built by | Image SHA-256 (prefix) |
|---|---|---|
| 0 | assembler, from `seed/system.asm` | `b9b9c609…` |
| 1 | generation 0 compiling `system.march` | `fe114689…` |
| 2 | generation 1 compiling `system.march` | `fe114689…` |
| 3 | generation 2 compiling `system.march` | `fe114689…` |

Generations 1, 2 and 3 are byte-identical. The fixed point holds from the first
rebuild. After generation 0, only images and March source are involved.

```sh
cargo run --offline --bin march7-seed -- seed/system.asm gen0.image
cargo run --offline -- gen0.image --fuel 1000000000 seed/system.march --system gen1.image
cargo run --offline -- gen1.image --fuel 1000000000 seed/system.march --system gen2.image
cmp gen1.image gen2.image
cargo run --offline -- gen1.image --eval ': square dup u* ; 7 square'
```

## Why the first rebuild is already the fixed point

Every compiled call in `system.march` targets a word defined earlier in the same
file. Words from the running dictionary are only executed at compile time
(`:` `;` `immediate`, the phase-1 `[` `]` `prim,`). Executing a word leaves no
reference to it in the output. Numbers, calls and quotations are encoded the same
way by every generation. So the compiled output does not depend on which
generation compiled it.

Content addressing also shares byte-identical objects across generations. These
are the name strings and leaf words such as the one-primitive `dup` wrapper. A
test checks that every shared code object is a leaf, so no part of the listing's
call graph is reachable from a rebuilt system.

## How control flow is bootstrapped

Branch operands are instruction indexes, so `then` must know how many
instructions precede it. The word `ops` counts them by scanning the output
bytes, using a packed table of operand lengths. `ops` is the one hand-assembled
word in the file: its two branches are written with `[ 6 c, 28 u32, ]` and
`[ 5 c, 6 u32, ]`. Everything after it uses `if`, `cycle` and the rest.

## Host changes

Only these:

- `Driver::system_image(xt)` exports an image whose entry is the code token
  `xt`. Its data root is empty, and it holds exactly the blobs reachable from the
  entry through code operands. The host traces only code operands and never reads
  March data.
- The command line gained `--system OUT`, which takes the entry token from the
  top of the stack, and `--fuel N`. The host looks up no names.

The assembler, the machine, the image format and the listing are unchanged.

## Measurements

These are local release runs, not benchmarks.

| Measurement | Value |
|---|---|
| Machine steps to compile `system.march` | see "Costs" below |
| Three full rebuilds in the test suite | about 0.1 s |
| Rebuilt image size | about 43 KB |
| `system.march` | 440 lines, 342 non-comment |
| `system.asm` listing | 1,395 lines |

## Tests

The first round added five tests to `tests/rebuild.rs`:

- The fixed point across generations 1 to 3.
- No listing code reachable from generation 1.
- Programs on the rebuilt system: `square`, the compiler example, `if`, loops,
  recursion, signed literals.
- Dictionary save and reload on the rebuilt system.
- Recovery from failed definitions and overflow.

All 22 existing integration tests and 2 unit tests still pass, in debug and
release. Clippy with warnings denied and `cargo fmt --check` pass.

## Second round: reentrancy, signed arithmetic, multi-line, inlining

All four were written in `system.march`. The host changed only to add scratch frames (item 1).

1. **Scratch stack and reentrant `evaluate`.** `>r` `r>` `r@` keep
   temporaries on a scratch stack. It is separate from the machine's private
   return stack. Every call gets its own scratch frame, and returning discards
   whatever the word left there. A tail call discards it too, since that word is
   finished. Popping or peeking below the current frame is a machine stack
   error. So a word can neither leave hidden outputs nor read its caller's
   values, and its stack effect stays its whole interface. This needed three
   machine primitives (ids 33-35), because only the machine sees returns. The
   words are compile-only (trap 13 otherwise) and always emit the primitive
   inline. A call through a wrapper would discard the value on the wrapper's own
   return. `evaluate` saves and restores the caller's input on its scratch frame,
   so a word may evaluate text while its own input is being interpreted. While
   compiling, that makes macros: `: sq-body s" dup u*" nip evaluate ; immediate`.
   `install` now also traps (12) before the dictionary would outgrow region 1.
2. **Checked signed arithmetic.** `+ - * / mod lt? gt? lte? gte? negate abs`
   are March words over the unsigned primitives. Overflow traps with code 10,
   including `MIN -1 /`, and division by zero traps with code 11. Division
   truncates toward zero, and `mod` takes the dividend's sign. The unsigned
   `u+ ... ult?` words are unchanged.
3. **Definitions may span inputs.** `:` copies the name into region 1 at once,
   and reaching the end of input while compiling is no longer an error. An
   error mid-definition still abandons only that definition.
4. **Inlined primitives.** A dictionary entry's flags may mark it as an inline
   primitive (bit 1, with the primitive id in bits 8-15). Compiling such a word
   emits the primitive op instead of a call to its wrapper. `' dup` still gives
   the wrapper's token. A counting loop halves in cost:

| Loop, 5 million iterations | Steps | Time per iteration |
|---|---|---|
| Calling one-instruction words | 45.0 million | 19.3 ns |
| Inlined primitives | 25.0 million | 11.2 ns |

`examples/inline_probe.rs` reproduces this.

### How inlining keeps generations identical

Inlining depends on the running dictionary's flags. So the file stops relying
on the running compiler as soon as it has its own. After defining `interpret`
and `evaluate`, the file executes `take-over`, which runs the new `interpret`
on the rest of the file's input. Everything below that line is compiled by
this file's compiler, whatever generation is running. The primitives are
marked inline only after `take-over`, so the part above it is compiled
identically everywhere. Generations 1, 2 and 3 remain byte-identical
(`fe114689…`).

### Costs and what comes next

Compiling `system.march` now takes about 10.2 million steps on generation 0
(29 ms) and 15.2 million on generation 1 (41 ms). That exceeds the driver's
default budget of 10 million steps, so a rebuild needs `--fuel`. Generation 1
is slower because the compiler words sit above `take-over`. They were compiled
by the previous compiler, so their primitives are still wrapper calls.

Next candidates:

- **Recompile the compiler words below `take-over`.** That would inline them
  too, without duplicating source.
- **Keep temporaries on the scratch stack.** Words like `find` and `number`
  still use fixed temporary offsets. That is safe now, because `evaluate` is
  reentrant and those words do not call back into user code, but the stack is
  the cleaner convention for new code.
- **Freeze the listing.** `system.asm` is only needed to produce generation 0.

`tests/rebuild.rs` now has nine tests, adding: scratch stack and nested
`evaluate`; checked signed arithmetic and its traps; definitions spanning
inputs; and the inlined bytes of `: sq dup u* ;`.
