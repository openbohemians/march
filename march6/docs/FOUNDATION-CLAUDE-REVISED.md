# Foundation proposal (Claude, revised with clarified requirements)

Status: second pass, 2026-09-28. **Not blind.** I have read
FOUNDATION-CODEX.md and FOUNDATION-COMPARISON.md, and Codex relayed Thomas's
"Chuck Moore way" conversation. FOUNDATION-CLAUDE.md stays unchanged as the
first pass. Design only. Each point below is marked as one of:

- **[user]**: a stated requirement from Thomas.
- **[mine]**: my recommendation.
- **[open]**: an unresolved alternative.

## What changed and why

**[user]** Thomas wants March built bottom up, with March itself at the
foundation. He wants a system layer that can still be tapped beneath the
high-level language ("a bit like Rust's unsafe"), and that layer is to be
built **first, not after**. The smallest form he suggested is an abstract
assembler stack machine with CAS.

My first pass does not meet that. It made host-provided tagged values (Text,
Tuple, Code) and the CHAMP store the lowest layer. Systems programming would
then be an afterthought layered over a high-level runtime, which is the
inversion he rejected. I withdraw that part.

What survives is the rule, applied one level lower:

> Every feature is either a host **mechanism** (a machine operation or a
> declared host import) or **March code**. Anything with syntax, a name in
> the manual, or a data representation above raw cells is March code.

Under this rule, even Text and Tuple are March code. They are
representations in memory, written in System March.

## Layer 0: the machine (host)

**[mine]** This agrees with Codex's cell machine.

- 64-bit cells, a data stack, and a private return stack.
- Owned memory regions, addressed by region plus offset, never by raw host
  pointers. Byte and cell load and store are bounds-checked.
- Literals, calls, returns, indirect execute, and branches with validated
  targets.
- Integer and bit operations with specified semantics. Wrapping machine
  arithmetic is kept separate from checked application arithmetic.
- Declared host imports: byte I/O, hashing, and image read and write.
- Fuel and stack bounds.

The whole inventory is written down, including hidden services, before
anything is built. **[mine]** A small checked Rust executor comes first,
because it is portable and testable. march4's hand-written assembly was lost
to register-clobbering bugs (the `rbx` fix in b1b26b6), and its C unit tests
stopped compiling. Assembly or native code later comes from a code generator
**written in March**, reading definitions, as in Factor. **[open]** Wasm is a
possible later backend, running outside the browser with WASI for I/O.

## Layer 1: System March (the "unsafe" layer)

**[user]** This layer is built first.

It runs on the same machine as all later code. It holds FORTH's classic
compile state as ordinary variables in memory: the input position (`>IN`),
the next free code slot (`HERE`), the compile/interpret flag (`STATE`), and
the dictionary. It also holds WORD, numbers before lookup (Thomas's rule),
`:` `;` `immediate` `literal` `postpone`, and branch fixups.

**Correction to my first pass.** I put the cursor and builder in the
persistent store so that immediate words would share the one data stack.
That goal is right, but the store was the wrong place: in FORTH, compile
state is memory that every word can reach from the one stack. Thomas's
separation of workspace from published state holds. It also avoids hashing
scratch work on every step, which Codex rightly flagged.

## The sealing boundary: where CAS enters

**[mine]** This is the bridge between the two layers, and it is march4's own
unbuilt design. DESIGN-OVERVIEW §5 separates a mutable "Workspace" heap from
an immutable content-addressed "Global Store", with `freeze` as the only
crossing.

- A builder in working memory is **sealed** into a canonical definition:
  fixed bytes and a CID. Names are separate bindings.
- **Linking** resolves CIDs into executable references, so calls do no
  lookup or hashing (march4 `loader.c`).
- Data crosses the same way: `freeze` copies from working memory into the
  immutable store encoding. Writable aliases never cross.
- The merkle-champ store is a **host service** behind this boundary at
  first, because it is measured and tested. Rewriting it in March is a
  destination, not a prerequisite.
- The image holds code, bindings, and immediate flags. Codex is right that
  flags and bindings must persist. The host must not reinstate them.

Two identity rules follow:

- **Primitives.** A primitive's identity names its **meaning**, with a
  semantic version. Implementations may change; meanings may not change
  silently. This agrees with Codex and corrects my "fixed ID" wording.
- **Recursion.** A local self-call operand covers the first milestone. For
  mutual recursion later, **[mine]** hash each strongly connected group as
  one unit, with members named by index, as Unison does. **[open]**

## Layer 2: Checked March

Typed quotations, roles and named signatures, stack-effect inference, guarded
families, immutable Text/Tuple/array representations, and namespaces over the
store. All of it is March code on the same machine.

**[mine]** This also settles the dynamic-call qualification in the
comparison. At the system layer, `execute` is **unchecked**: bounded by fuel
and stacks, like `unsafe`. Checked March only gives `execute` a shape when
the quotation's type is known (typed quotations or roles). There is no
separate `Apply` count form.

**Guards: [mine] I concede to write rejection.** A discarded store snapshot
cannot undo memory writes or I/O, so march2 A's overlay does not generalize to
a machine with memory. Guards run with no write authority, and the argument
stack is restored. march2 A's `raise`, a re-dispatch to an error-typed clause,
remains worth keeping.

## Reuse (updated)

| Source | Decision |
|---|---|
| march4 code/link split, workspace/store/freeze design | Adopt as the layer 0 to layer 1 boundary |
| march4 `vm.asm` kernel | Reference for the operation set; not the first executor |
| march2 B `native_colon`/`native_semicolon`, `input.rs` | Adapt, as March code, not Rust |
| march6 `stack.rs`/`Value` | Behavioral reference and test oracle for Checked March, not the substrate |
| march6 definition/CID tests, stream seed, store, merkle-champ | Keep; the seed's behavior is retargeted to System March |

## First milestone (Codex's gate, which I adopt)

1. March defines a compiling word, emits another word, and runs it.
2. It saves the code, bindings, and flags.
3. A fresh process reloads it without the host assembler or the old engine.
4. The system layer rebuilds itself for two more generations with identical
   CIDs and behavior.

Bounded failure tests are included, and compile, link and run costs are
measured separately.

## Risks and remaining differences

- **[mine] The main risk is history repeating.** The bootstrap has
  repeatedly lost out to lower-level work: march4 spent its time on three
  memory schemes and assembly bugs. So milestone 1 allows regions with a
  bump pointer and release-whole-region only. No allocator and no
  reclamation scheme. That matches "memory is 2.0" for the high-level
  language, which remains true.
- **[open] Interpreter speed.** High-level values written in March over cells
  will be slow on a simple executor until native code generation exists.
  Measure early.
- **[open] Safety.** What is trusted before `unsafe` is sealed away from
  Checked March. I agree with Codex that bounds checks do not give type or
  lifetime safety.
- **No remaining architectural disagreement with Codex that I know of.**
  The differences left are choices: the operation inventory, the
  recursive-group identity, and when the store moves into March.
