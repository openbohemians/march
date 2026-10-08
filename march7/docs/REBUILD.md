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
| 1 | generation 0 compiling `system.march` | `22eabcf1…` |
| 2 | generation 1 compiling `system.march` | `d84a5cd3…` |
| 3 | generation 2 compiling `system.march` | `d84a5cd3…` |

Generations 2 and 3 are byte-identical, so the fixed point holds from the
second rebuild. Until tail calls (2026-10-04) it held from the first; see
below. After generation 0, only images and March source are involved.

```sh
cargo run --offline --bin march7-seed -- seed/system.asm gen0.image
cargo run --offline -- gen0.image --fuel 30000000 seed/system.march --system gen1.image
cargo run --offline -- gen1.image seed/system.march --system gen2.image
cargo run --offline -- gen2.image seed/system.march --system gen3.image
cmp gen2.image gen3.image
cargo run --offline -- gen2.image --eval ': square dup u* ; 7 square'
```

## Why the fixed point comes so early

Every compiled call in `system.march` targets a word defined earlier in the same
file. Words from the running dictionary are only executed at compile time
(`:` `;` `immediate`, the phase-1 `[` `]` `prim,`). Executing a word leaves no
reference to it in the output. Numbers, calls and quotations are encoded the same
way by every generation, and primitives are inlined by the file itself (see
"Inlining" below). So the compiled output depends on which generation compiled
it in one place only: what `;` does at the end of a definition.

Since tail calls, it does something. A call or `recur` just before a return
becomes a tail call (opcode 9) or a tail recur (opcode 11), and that happens
in `;`, which belongs to the running system. Generation 0's `;` is frozen and
emits no tail calls, so generation 1's code has none; generation 1's `;` is the
source's own, so generations 2 and 3 have them and agree. This is the usual
shape of a compiler bootstrap, which compares its second and third stages
because the first is built by the old compiler.

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
| Machine steps to compile `system.march` on generation 0 | 22.7 million |
| The same on generation 1 | 8.94 million |
| Driver's default budget | 10 million steps |
| Rebuilt image size | 181,373 bytes |
| `system.march` | 3,606 lines |
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
  wrapper (`p.dup`, one primitive op and a return) is what `init` binds as
  `dup`, flagged inline, so `' dup` and `call` work in a booted system. Then an
  immediate word named `dup` emits the primitive op. Within the file every use
  of a primitive therefore compiles inline, whatever system compiles it.
- **`get` and `put` are emitters too,** from the compiler words onward: one
  primitive each, a load or store in working region 1 (44, 45). So are `0=`,
  `out` and `here`. The words above them keep calling them. The hand-counted
  `ops` depends on that, and those words only run at compile time; a faster,
  incremental `ops` and the control words that use it are defined again below
  the emitters, as are `c,`, `prim,`, `,` and `literal`. `0=` is exported as
  `p.0=`, a function with the same code.
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

The dictionary is now hashed: 1,024 buckets, each a chain through the
entries, newest first so shadowing still finds the newest definition. Names
hash with `hash-bytes`, the same function merkle-champ uses to place keys,
so a name hashes the same in the working dictionary and in a store. A test
checks it against the crate itself. Compiling the system on generation 1
dropped from 12.5 to 5.3 million steps, and later generations rebuild within
the default budget again. Generation 0's frozen compiler still scanned
linearly (until the re-freeze below), so its cost grew faster than the
source: 18.3 million steps as of the quotation work, 34.8 million as of
checker slice 2. A rebuild from generation 0 on the command line needed
`--fuel`, and the tests gave it 60 million.

The driver's step budget (10 million by default) is a safety net against
runaway programs, and it stays tight on purpose while runaways are common
(decided with Thomas, 2026-09-30). Compile cost is tracked separately: the
rebuild tests use an explicit budget of 30 million steps (120 million before
generation 0 was re-frozen), and a named canary test asserts that a rebuilt
system compiles the system source in under 9.5 million steps. It is 8.94
million with the staged-types prototype's fifth step, within 1.1 million of
the driver's default budget, which the plain rebuild command uses; the
fourth step's 480 lines took it to 8.50 million, 15% more, in proportion,
and the canary from 7.5 to 9.5 million. The third step took it to 7.40
million, the second to 7.08, and the first step's 210 lines from 6.23 to
6.68 million and the canary from 6.5 to 7.5 million. It was 6.23 million as
of the second string slice (escapes, holes, raw literals and the words on
text), which added 11% to the source and 0.7 million steps; it was 5.51
million after the byte primitives (below), down from 8.90 million with
strings and maps, which had come within 0.1 million of the canary of the
time and 1.1 million of the driver's default budget. The canary was raised
to 9 million when `each`, `fold`, `map`, nested array types and scratch
types (2026-10-04) took it to 8.09 million: the code grew 6.6% in tokens and
the steps 6.5%, so the cost per token did not change. It took 7.59 million
as of arrays on persistent vectors (6.65 million as of checker slice 2: its
code added 1.3 million to the 3.8 before it, typing at top level about a
million more, mostly analysing the words the source runs at top level once
per session, and keeping types by stack position rather than for the top
eight slots half a million; arrays and lifting added 0.8 million). `an@` and
`an!` became inline emitters along the way.

Generation 0 grew faster: 93.4 million steps as of the second string slice
(77.2 as of the byte primitives, 76.5 as of maps, 65.5 as of consumers at
top level, 55.7 as of arrays), and the tests' budget had been raised from 80
to 120 million. A profile put 93% of those steps in the listing's `find`,
`get` and `put`, mostly the scan in `find`, which read each entry's fields
through `get`: it scanned the whole dictionary for every word, and the
common words, defined early, were at the far end of the chain, so the cost
grew with the square of the number of definitions.

**Re-freezing the listing (2026-10-06).** The listing's `find` and `install`
became hashed, as the rebuilt system's had: 1,024 buckets at offset 4096 of
working memory, entries from 12288 with a bucket link at offset 32 and the
name from offset 40, the same layout as the rebuilt dictionary. Names hash
with the `byte-hash` primitive (FNV-1a), whose top ten bits pick the bucket,
and compare with `bytes-eq?`. About fifty lines of the listing changed, and
it stays hand-written. Generation 0 now compiles the system in 14.8 million
steps, a sixth of before, and 0.06 seconds instead of 0.37, and its cost
grows with the source rather than its square. Generations 1, 2 and 3 are
byte-identical to those built before the change, so only the pinned hash of
generation 0 changed (`b9b9c609…` became `578a2424…`). The rebuild tests'
budget went down to 30 million. A rebuild from generation 0 on the command
line still needs `--fuel`, since the driver's default is 10 million; when
the command line exhausts the budget, it says so and suggests `--fuel`.

**Working-memory primitives (2026-10-04).** Adding symbol names
(docs/SURFACE.md) took compiling the system on generation 1 to 8.5 million
steps, over the canary, then 8 million. A per-word step profile showed every
token cost about 900 steps to compile, mostly in `find`, `hash-bytes` and
reading characters, and that the common cost was working memory: each `get` or
`put` ran as four instructions (the offset, region 1, a swap, the load or
store). Primitives 44 and 45 load and store a cell in region 1 directly, so
`get` and `put` compile to two instructions. Compiling the system on
generation 1 fell to 5.5 million steps, and a token to about 620.

**Less work per token (2026-10-04).** A second profile drove five changes,
all in March, which took compiling the system on generation 1 from 5.5 to 3.8
million steps, and a token to about 460:

- **Counting instructions incrementally.** `then`, `else`, `cycle` and
  `repeat` need the instruction index where they stand, and `ops` counted it
  from the start of the definition each time, so long definitions cost
  quadratic time. `ops` now remembers the region and offset it reached
  (working memory 848-864) and continues from there; a different region, or
  an offset behind it, counts again.
- **Hashing while reading.** `read-word` computes FNV-1a over the word as it
  reads it and keeps that with the word's position (816-840); `find` finishes
  the hash for the word just read instead of reading its bytes again. Any
  other name is hashed in full, so a stale hash cannot be used for a different
  word.
- **Inline `0=`, `out` and `here`,** as emitters like `get` and `put`.
- **The stack instead of working memory** for the read position in
  `read-word` and `line-comment`, and for the offsets in `find`'s comparison.
- **An unrolled `,`**, eight byte stores with no loop counter.

The canary was lowered to 4.5 million so it still catches growth.

**Byte primitives (2026-10-05).** Strings and maps took compiling the system
on generation 1 to 8.90 million steps, against a canary of 9 million and the
driver's default budget of 10 million. A per-word step profile showed where:

| Word | Before | After |
|---|---:|---:|
| `read-word` | 1.87 million (21%) | 0.69 million |
| `find` | 1.17 million (13%) | 0.43 million |
| `line-comment` | 0.58 million (6.5%) | under 0.03 million |
| `int-number` | 0.51 million (5.8%) | 0.18 million |
| total | 8.90 million | 5.51 million |

Six primitives (62 to 67) take the per-byte loops out of March, each keeping
the word's meaning exactly:
- `byte-find`, `byte-past` and `byte-upto` ( r o end b -- k ): the first offset
  whose byte is b, is above b, or is at or below b. `read-word` skips spaces
  and finds a word's end with them, and comments and string literals find
  their end with `byte-find`; `symbolize` checks for a backslash first.
- `byte-hash` ( r o n -- h ): FNV-1a over a span, as `read-word` and
  `hash-bytes` hashed byte by byte.
- `bytes-eq?` ( r1 o1 r2 o2 n -- flag ): `find` compares a name in one step.
- `decimal` ( r o n -- value status ): digits to a number, with status 0 for
  a span that is not all digits and 2 for one that overflows 64 bits; the
  sign and the range of a signed integer stay in `int-number`.

The rest of the profile is spread out: `read-word` itself, emitting code a
byte at a time (`c,`), `interpret`, `find`, then the checker's analyses.
The canary was lowered to 6.5 million so it still catches growth.

Next candidates:

- **Keep temporaries on the scratch stack.** Words like `find` and `number`
  still use fixed temporary offsets. That is safe now, because `evaluate` is
  reentrant and those words do not call back into user code, but the stack is
  the cleaner convention for new code.
- **The remaining profile.** After the byte primitives, reading words is
  about an eighth of compile time, emitting code a byte at a time about a
  twelfth, then `interpret` and `find`.
- **The listing is frozen** (2026-09-30). `system.asm` only reproduces
  generation 0, and a test pins the SHA-256 of its assembled image. It was
  re-frozen once, on 2026-10-06, to hash its dictionary (below).

`tests/rebuild.rs` now has nine tests, adding: scratch stack and nested
`evaluate`; checked signed arithmetic and its traps; definitions spanning
inputs; and the inlined bytes of `: sq dup u* ;`.
