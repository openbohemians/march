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
| 1 | generation 0 compiling `system.march` | `129daab3…` |
| 2 | generation 1 compiling `system.march` | `129daab3…` |
| 3 | generation 2 compiling `system.march` | `129daab3…` |

Generations 1, 2 and 3 are byte-identical. The fixed point holds from the first
rebuild. After generation 0, only images and March source are involved.

```sh
cargo run --offline --bin march7-seed -- seed/system.asm gen0.image
cargo run --offline -- gen0.image --fuel 40000000 seed/system.march --system gen1.image
cargo run --offline -- gen1.image seed/system.march --system gen2.image
cmp gen1.image gen2.image
cargo run --offline -- gen1.image --eval ': square dup u* ; 7 square'
```

## Why the first rebuild is already the fixed point

Every compiled call in `system.march` targets a word defined earlier in the same
file. Words from the running dictionary are only executed at compile time
(`:` `;` `immediate`, the phase-1 `[` `]` `prim,`). Executing a word leaves no
reference to it in the output. Numbers, calls and quotations are encoded the same
way by every generation, and primitives are inlined by the file itself (see
"Inlining" below). So the compiled output does not depend on which generation
compiled it.

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
| Machine steps to compile `system.march` on generation 0 | 24.4 million (80 ms) |
| The same on generation 1 | 5.5 million (18 ms) |
| Driver's default budget | 10 million steps |
| Rebuilt image size | 61,724 bytes |
| `system.march` | 1,147 lines |
| `system.asm` listing | 1,403 lines |

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

### Inlining inside the system source

The dictionary flags serve programs compiled by a booted system. The system
source cannot rely on them, because they belong to whichever system is
compiling it, and generation 0 has none. So the file inlines by itself:

- **Primitive names are emitters.** Phase 2 defines each primitive twice. The
  wrapper (`p-dup`, one primitive op and a return) is what `init` binds as
  `dup`, flagged inline, so `' dup` and `call` work in a booted system. Then an
  immediate word named `dup` emits the primitive op. Within the file every use
  of a primitive therefore compiles inline, whatever system compiles it.
- **`get` and `put` are emitters too,** from the compiler words onward: a
  literal 1 (working region 1), a swap, and the load or store. The words above
  them keep calling `get` and `put`. The hand-counted `ops` depends on that, and
  those words only run at compile time.
- These emitters are compile-time devices. Nothing exported refers to them, and
  the file never uses a primitive name outside a definition.

An earlier version handed the rest of the file to its own compiler partway
through (`take-over`) and marked the primitives inline after that point. That
left the compiler words themselves uninlined. Recompiling them in a second pass
reached the same bytes, but cost more steps than it saved, because the second
pass ran on the uninlined first-pass compiler. The emitters replace both.

### Costs and what comes next

| Compiling `system.march` | Before | Now |
|---|---|---|
| On generation 0 | 10.2 million steps | 7.8 million |
| On generation 1 | 15.2 million steps | 7.3 million |
| Rebuilt image | about 43 KB | 31.8 KB |

After inlining, every generation rebuilt within the driver's default budget.
The stack-effect checker (docs/CHECKER.md) then added about 230 lines, and
compiling the system now takes 13.8 million steps on generation 0 and 12.5
million on generation 1. The rebuild tests caught that growth. The cost is the
dictionary: every lookup scanned it linearly, so compile cost grew with tokens
times entries.

The dictionary is now hashed: 1,024 buckets, each a chain through the entries,
newest first so shadowing still finds the newest definition. Names hash with
`hash-bytes`, the same function merkle-champ uses to place keys, so a name
hashes the same in the working dictionary and in a store. A test checks it
against the crate itself. Compiling the system on generation 1 dropped from
12.5 to 5.3 million steps, and later generations rebuild within the default
budget again. Generation 0's frozen compiler still scans linearly (18.3
million steps as of the quotation work), so a rebuild from generation 0 on the
command line needs `--fuel`.

The driver's step budget (10 million by default) is a safety net against
runaway programs, and it stays tight on purpose while runaways are common
(decided with Thomas, 2026-09-30). Compile cost is tracked separately: the
rebuild tests use an explicit budget of 40 million steps, and a named canary
test asserts that a rebuilt system compiles the system source in under 6.5
million steps. When the command line exhausts the budget, it says so and
suggests `--fuel`.

**Working-memory primitives (2026-10-04).** Adding symbol names
(docs/SURFACE.md) took compiling the system on generation 1 to 8.5 million
steps, over the canary, then 8 million. A per-word step profile showed every
token cost about 900 steps to compile, mostly in `find`, `hash-bytes` and
reading characters, and that the common cost was working memory: each `get` or
`put` ran as four instructions (the offset, region 1, a swap, the load or
store). Primitives 44 and 45 load and store a cell in region 1 directly, so
`get` and `put` compile to two instructions. Compiling the system on
generation 1 fell to 5.5 million steps, and a token to about 620. The canary
was lowered to 6.5 million so it still catches growth.

Next candidates:

- **Keep temporaries on the scratch stack.** Words like `find` and `number`
  still use fixed temporary offsets. That is safe now, because `evaluate` is
  reentrant and those words do not call back into user code, but the stack is
  the cleaner convention for new code.
- **Read and hash in one pass.** `read-word` walks a word's characters and
  `hash-bytes` walks them again; folding the hash into the read would save the
  second walk on every token. The profile puts reading at about a fifth of
  compile time and hashing at a tenth.
- **The listing is frozen** (2026-09-30). `system.asm` only reproduces
  generation 0, and a test pins the SHA-256 of its assembled image.

`tests/rebuild.rs` now has nine tests, adding: scratch stack and nested
`evaluate`; checked signed arithmetic and its traps; definitions spanning
inputs; and the inlined bytes of `: sq dup u* ;`.
