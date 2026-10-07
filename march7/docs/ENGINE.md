# The engine: how March7 compiles and runs code

Status: overview, 2026-10-07. Each part has its own document, named in its
section. This one shows how the parts fit together.

## Three layers

```
seed/system.march   2,407 lines of March: reader, dictionary, compiler,
                    checker, interpreter. This is the real compiler.
        ▲ compiled by
seed/system.asm     1,414 lines of hand-written instructions (generation 0).
                    Used only to bootstrap; frozen.
        ▲ runs on
src/machine.rs      about 1,300 lines of Rust: a small stack machine.
                    It has no parser, no compiler and no dictionary.
```

The Rust host never reads March source. It runs bytecode, and the bytecode
that reads source is March's own compiler.

## 1. The machine

`src/machine.rs`, with the code format in `src/code.rs`.

**State.**
- A **data stack** of 64-bit cells, holding integers, float bits and handles
  alike. A cell carries no tag saying which; only the checker's types know.
- A **scratch stack** (`>r`, `r>`) and a **mark stack** (for `(` … `)`),
  both per call frame. Returning discards them, so a word cannot leave hidden
  values for its caller (docs/FOUNDATION.md).
- **Regions**, memory named by handle numbers: a byte buffer, an array (a
  merkle-champ `Sequence` of cells), a string (UTF-8 text) or a map (a CHAMP
  map of cells). Region 1 is working memory, where the compiler keeps its
  state (docs/FIRST-SLICE.md).

**Code.** A word is a blob of canonical bytes, in 12 opcodes:

| Byte | Operation |
|---|---|
| `0` | return |
| `1 n` | literal (8 bytes) |
| `2 id` | primitive: 74 of them, pinned by wire ID |
| `3 cid` | call (32-byte CID) |
| `4 cid` | quote: push a word's token |
| `5 n`, `6 n` | branch, branch if zero |
| `7` | recur |
| `8 cid` | data: a read-only region from a data blob |
| `9 cid` | tail call |
| `10 n` | float literal: runs as a literal, typed for the checker |
| `11` | tail recur |

A call names its callee by **CID**: the SHA-256 of `"march7/code/v1\0"` and
the callee's bytes. Names never appear in code.

**Linking.** Before a word runs, the machine decodes its bytes and resolves
each callee's CID to an index into its table of linked words, recursively. A
word's **execution token** is its index plus 1.

**Running.** `Machine::run` is a loop over resolved instructions. Each one
costs a step of **fuel**, 10 million by default, so a runaway program stops
with an error. A call pushes a return frame, a tail call reuses the current
one, and a failure is a trap with a number, which the host reports.

## 2. Images and the driver

`src/image.rs`, `src/driver.rs`; docs/REBUILD.md.

An **image** is a set of blobs, an entry CID and a data root. `Driver::boot`
links and runs the entry word, which leaves the tokens of four March words:

- **evaluate**, which interprets source;
- **recover**, which runs after an error and abandons an unfinished
  definition;
- **export**, which writes the dictionary out, so a session can be saved as
  an image;
- **describe**, which reports the types of the values on the stack, for the
  display.

`march7 IMAGE --eval "…"` puts the text in a region and runs **evaluate** on
it. Everything after that is March.

## 3. Bootstrap

docs/REBUILD.md.

1. `seed/system.asm` is assembled into **generation 0**, a small FORTH-like
   system with a hashed dictionary.
2. Generation 0 compiles `seed/system.march`, which defines a whole new
   system and leaves its `boot` token on the stack.
3. The host exports every code blob reachable from `boot`. That is
   **generation 1**.
4. Generation 1 compiles `system.march` to make generation 2, and generation
   2 makes generation 3. Generations 2 and 3 are byte-identical: the
   self-rebuild fixed point.

## 4. Compiling a definition

`seed/system.march`; docs/QUOTATIONS.md, docs/CHECKER.md.

Take `: square dup u* ;`.

1. **`:`** reads the name and opens a 64 KiB output buffer. The state
   switches to compiling.
2. **Each word** is read with the byte primitives, looked up in the hashed
   dictionary, and handled by its flags:
   - an **immediate** word runs now: compiler words like `if` and `[`;
   - an **inline primitive** emits its op, so `dup` becomes `2 0`;
   - a **number** emits a literal; a **string literal** emits its text as
     data, or, with holes, code that builds it;
   - **anything else** emits `3` and the callee's 32-byte CID;
   - a **family** word such as `+`, or a **generic** word, is emitted
     provisionally, and the definition is marked for resolution.
3. **`;`** finishes the definition:
   - a final call becomes a tail call, and `0` (return) is appended;
   - if marked, the **checker** works out the types of the values in the
     compiled bytes and picks each family call's clause: `+` on integers, on
     floats, or lifted over arrays. A generic word gets an instance for its
     input types;
   - in checked mode, the word's stack effect is verified as well;
   - the bytes are **sealed**: the machine hashes them into a CID, stores the
     blob and links it;
   - the name, token and flags go into the dictionary.

So `square` is the five bytes `2 0 2 7 0`, and its CID is their hash.

**Early binding.** Callers hold their callee's CID. Redefining `square`
makes a new word with a new CID, and words compiled earlier keep calling the
old one.

**Quotations.** `[ … ]` compiles into a buffer of its own and waits. A
consumer such as `if`, `times`, `each` or `map` inlines it into the
definition as plain branches. One that no consumer takes is sealed as an
anonymous word and pushed with a quote op.

## 5. Running at top level

Outside a definition, **evaluate** runs each word at once. Beside the data
stack it keeps a **shadow stack** of types, one byte per value, so it makes
the choices the compiler would: `1.5 2.5 +` takes the float clause, and
`( 1 2 ) 1 +` lifts over the array.

A consumer at top level, as in `( 1 2 3 ) [ 1 + ] map`, compiles its code
into a temporary word, checks it against the types on the stack, and runs
it, so top-level code behaves as compiled code does. The display reads the
shadow stack, which is why a value of unknown type shows as a raw cell.

## 6. The checker

docs/CHECKER.md.

The checker is written in March and reads compiled bytes, not source. For
each word it derives a **stack effect** (inputs, outputs, and whether they
are known) and the **types** of its outputs, lazily, when a caller or the
top level needs them. With them it resolves families, makes instances of
generic words, lifts operations over arrays, and stops known type errors
(trap 23).

A type is one byte: i64, f64, arrays by element type and rank, maps by key
and value kind, strings. Values in nested or mixed containers often have no
type the byte can express, which is an open design question.

## 7. Costs

| Measure | Value |
|---|---|
| Compiling the system on a rebuilt generation | about 6.2 million steps |
| The same on generation 0 | about 14.8 million steps |
| A tight loop | about 11 ns an iteration |

## Where to look

| What | Where |
|---|---|
| Bytecode format and primitives | `src/code.rs` |
| Machine | `src/machine.rs` |
| Images and boot protocol | `src/image.rs`, `src/driver.rs` |
| Command line | `src/main.rs` |
| Reader, compiler, checker, interpreter | `seed/system.march` |
| Generation 0 | `seed/system.asm`, `tools/assembler.rs` |
| Each part in detail | `docs/`: FIRST-SLICE, REBUILD, CHECKER, QUOTATIONS, ARRAYS, STRINGS, MAPS, NUMBERS, SURFACE |
