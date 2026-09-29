# Foundation proposal (Claude, blind first pass)

Status: independent draft, frozen 2026-09-28. Written without reading
FOUNDATION-CODEX.md. Design only.

## Disclosures

- I wrote the lineage digests in `doc/lineage/`, which Codex also used, so
  shared conclusions are not independent evidence.
- I wrote merkle-champ and argued for it as the store. I argued for strict by
  default. I reviewed `fast/stack.rs` and found it sound.
- Assumed from recorded decisions: strict semantics, stack model, CAS
  identity by token content, a CHAMP store as dictionary and state, memory
  management deferred, roles as aliases.

## The problem to fix

Every lineage grew the host until the language lived there. The march6 host
supplies 43 `stream.*` compiler primitives. Many encode one piece of syntax
each: `family-guard`, `family-inputs`, `tuple`, `apply`, `recur`,
`emit-text`. march2 B made `:` and `;` immediate, but the dictionary was
"seeded entirely in Rust" and immediate words could not emit code
(march2 digest §5). march4 kept `: ; [ ] ( )` as special C tokens, and
CORE-ARCHITECTURE's plan to turn them into words produced only
`march.alloc` (march4 digest §1.4). A feature list becomes host code because
no rule says where a feature must live.

## The rule

A feature is exactly one of three things. Anything that fits none of them is
a design question to settle before code is written.

1. **A value kind** (host). Integer, Boolean, Text, Bytes, Tuple, Quote (a
   code CID), Code (a definition value).
2. **A primitive** (host). It has a fixed ID and a fixed stack effect. It
   knows no syntax and no names.
3. **March words.** Everything with syntax or a name in the manual:
   definitions, quotations, families, guards, type checking, roles, arrays,
   lazy constructs, modules.

The host is a machine, not a language.

## Layer 0: the host contract (Rust)

- **Machine.** Strict, left to right, real data and return stacks, free tail
  calls. `fast/stack.rs` already is this machine.
- **Code.** A definition is a value: a sequence of cells, each a primitive
  ID, a literal value, or a call by CID. Its identity is the hash of the
  canonical encoding. Names are never part of identity. march4 had fixed
  primitive IDs, with kernel changes not altering code identity (LINKING.md
  194-217). march4 also let the blob kind, not an opcode, decide
  call-versus-push (LINKING.md 468-475). Keep both.
- **Store.** One merkle-champ map from namespace paths to values. Code and
  data are both entries. Every word is (stack, store) to (stack, store), as
  Thomas stated. A failed run returns the store it started with.
- **Primitives, about 40.** Stack words and arithmetic. Text and bytes. Tuple
  construction and access. `call`, `if`, `store.get`, `store.put`. And one
  compiler primitive: `define`, which canonicalizes a cell sequence, hashes
  it, and returns a Code value. Stream reading is ordinary bytes and a cursor
  held in the store. There are no family, tuple-syntax, or number-syntax
  kernels.
- **Image.** A serialized store. The host loads an image and runs a named
  entry word. It contains no reader for March source.

## Layer 1: the nucleus (March, in the image)

- **The outer interpreter.** It reads a word, tries it as a number, then looks
  it up, then compiles or executes it. The existing `stream-seed.march`
  already does this in March. It moves onto layer 0 primitives.
- **Compile state lives in the store.** That includes the cursor, the open
  definition as a tuple of cells, and the mode. So an immediate word is an
  ordinary word, and it shares the one data stack. This removes the
  "compiler-state versus runtime-stack interface" that BOOTSTRAP.md lists as
  unresolved.
- **Definers.** `:` `;` `immediate` `[ ]` `--` literal text, with `literal`
  and `postpone` equivalents. march2 B lacked those two, which is why its
  immediate words could not emit code. `;` is `define` followed by
  `store.put` under the current namespace.

## Layer 2: the language (March)

- **Families** compile to sequences of `if` over guard quotations. A guard
  runs against the current store, and the store it returns is discarded. That
  gives march2 A's isolated guards (`main.rs:366-399, 1700-1727`), the one
  working precedent, for free with an immutable store. Its `raise`
  re-dispatch to an error-typed clause is worth keeping.
- **The type checker** is a March word run by `;`. It reads definitions
  through reflection and infers stack effects. Typed quotations carry their
  signature (march4 QUOTATIONS.md, never built). Roles and named signatures
  give dynamic `apply` its shape. It starts advisory. It is never a separate
  host checker: march2 B's host checker issued false errors on valid code.
- **Namespaces and modules** are store paths plus March words.

## Layer 3: libraries

Arrays and their families, lazy data constructs, units, and I/O. For systems
work: bounded `Bytes` load and store primitives now. Later, a code generator
written in March that reads definition values, as Factor does.

## Reuse, adapt, reject

| Source | Decision | Evidence |
|---|---|---|
| march6 `stack.rs`, `Value` | Reuse as layer 0 | Strict, sound sharing, tail calls reviewed |
| merkle-champ, `store.rs` | Reuse | Measured; identity per namespace |
| march6 canonical definitions, CIDs | Adapt | Keep hashing; drop `Apply`/`Recur` counts and `Family` as host forms |
| `stream-seed.march`, rebuild fixed-point test | Adapt | Already March; retarget to layer 0 |
| march6 lazy Executor, graph lowering, state edges | Park | Superseded by strict; keep as benchmark reference |
| 43 `stream.*` kernels | Replace with `define` plus bytes and cursor | Syntax belongs in March |
| march2 B early binding by value copy | Reject | Broke recursion (`forth.rs:333, 886`) |
| march4 per-call-site monomorphization as identity | Reject | A definition had no standalone meaning, and token definitions were never persisted |
| march4 type-erased stack | Defer | Reflection and dynamic dispatch need tagged values now |
| SQLite | Reject | The store image replaces it |

## First milestone: the strict self-rebuild

1. A boot binary holds only layer 0.
2. It loads `seed.image` and runs `stream-seed.march` through the March
   interpreter.
3. The output is a new image. Generations 2 and 3 are byte-identical.
4. `: square dup * ; 7 square` gives 49 from the rebuilt image.
5. `source::compile` is not on the boot path.
6. The host line count of the boot path is recorded.

Generation 0 is produced once by today's host reader and is then retired.

## Open choices needing design or measurement

- **Early or late binding.** Compiling a name to a CID pins dependents, and
  DEPENDENCY-EVOLUTION.md covers rebuilds. Late binding through `store.get`
  and `call` stays available explicitly.
- **Open families.** Clauses added after the family's definition need either
  rebuilding the family or dispatch through a store entry.
- **Dispatch speed.** Families compiled to `if` chains must be measured
  against a host dispatch form before deciding.
- **Interpreter speed.** A strict machine over `Arc` values is still
  unmeasured against the old specialized plans.
- **The type checker in March.** It needs richer reflection than `describe`.
- **Tags and systems work.** Tagged values today versus type erasure for
  low-level code.
