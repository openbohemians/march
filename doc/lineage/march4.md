# march4 (α₄) digest

Source: `/home/trans/my/com/tabcomputing/march/march4` (read-only). I read all 25 tracked Markdown files in full; two of them, `PLAN-CORE-INTRASTRUCTURE.md` and `PLAN-MEMORY-MANAGMENT.md`, are empty. I also read the docs that were deleted from the tree but are still in git history. I surveyed and partly read the source. I built `marchc` at HEAD in a scratch copy and ran several `.march` tests (results are in §4.3).

Git history: 87 commits, 2025-10-14 → 2025-11-13. Most commits are co-authored by Claude Code. **STATUS.md headings say "2024-11-0x"; they are typos for 2025-11-0x.** The git dates confirm this (for example, cd13a5a is 2025-11-04). STATUS.md was **not updated** after 2025-11-05. The last four commits (11-12 → 11-13) fixed the "BLOCKED" array crash, implemented `_` and `pick`, and added `march.alloc`. These changes appear only in commit messages.

Labels used throughout:
- **[IMPL+TESTED]**: code exists and something exercised it (a doc says so, or my own run did).
- **[IMPL, stub/broken]**: code exists but is a no-op or fails.
- **[DESIGN ONLY]**: prose or plans, no code.

---

## 1. Architecture as implemented

### 1.1 Toolchain history
- **OCaml bootstrap compiler, then C.** The first compiler was OCaml: lexer, parser/AST, `typecheck.ml`, `codegen.ml`, SHA256 via digestif, writing to SQLite (PROGRESS.md §3, lines 485-516).
  - It was abandoned on 2025-10-18/19 for a C rewrite: "OCaml's strengths (ADTs, pattern matching) pull toward AST/tree traversal - FORTH compiles token-stream, one-pass (no AST!)" (PROGRESS.md 650-656).
  - It was removed in commit 88918ce "Remove abandoned OCaml compiler implementation". Only `resources/{types,typecheck,codegen}.ml` remain, as reference.
- **What is live at HEAD.**
  - A C compiler/loader/runner in `src/`: ~3,000-line `compiler.c`, plus `tokens.c`, `dictionary.c`, `database.c`, `loader.c`, `runner.c`, `refgraph.c`, `hamt.c`, `debug.c`, `marchc.c`.
  - A NASM x86-64 kernel: `kernel/x86-64/*.asm`, 64 files.
  - SQLite (`march.db`, `schema.sql`).
- **Rust runtime, never linked.** `runtime/` (`im::HashMap<u64,u64>` behind a C FFI) builds, but it is never linked. The Makefile comment says "without runtime for now - not needed for VM tests" (Makefile:86). PROGRESS §2: "Built, not yet integrated with VM tests".

### 1.2 Execution model: a stack machine over tagged 64-bit cells, with a central dispatch loop
- **Registers** (`kernel/x86-64/vm.asm` 1-17): `rsi` = data stack pointer, `rdi` = return stack pointer, `rbx` = IP. Both stacks are 1024 qwords (8 KB) in `.bss` (vm.asm 23-26).
- **Cell tags** (vm.asm 10-17, 99-209; DESIGN-OVERVIEW.md §3; PROGRESS §7):

  | Tag | Name | Meaning |
  |---|---|---|
  | `00` | XT | Machine-code address; address 0 = EXIT |
  | `01` | LIT | 62-bit signed immediate |
  | `010` | LST | Symbol id |
  | `110` | LNT | The next N cells are raw 64-bit literals |
  | `111` | EXT | Reserved; currently a NOP (vm.asm 207-209) |

- **Threading, as built.** `vm_dispatch` fetches a cell, decodes the tag, and for XT does `jmp rcx` (vm.asm 125-135). Every primitive is machine code that ends in `jmp vm_dispatch` instead of `ret` (for example `dup.asm`). Commit e1d7e88 (2025-10-25) converted the system "from subroutine threading to direct threading".
  - A user word or quotation is a heap cell array plus a small **mmap'd trampoline**. The trampoline is created at link time: it loads the cells' address into `rax` and jumps to `docol` (`src/loader.c:265-307`).
  - `docol.asm` pushes IP onto the return stack, sets IP to the cells, and jumps to `vm_dispatch`. EXIT pops IP (vm.asm 140-152).
  - In short: FORTH-style code-field direct threading, but with a **shared NEXT loop and tag decoding on every cell**, not an inlined NEXT.
  - No benchmark of this was ever run. PLAN-DIRECT-THREADING.md:271 lists "Benchmark performance improvement" and it was never done.
- **Bytecode, two forms.**
  1. The **storage form** is a CID-referencing byte stream: 2-byte tags. Bit 0 = 0 means a primitive id; bit 0 = 1 means a blob kind followed by a 32-byte CID. Inline literals are also allowed (`PRIM_LIT` 0 plus 8 bytes). See LINKING.md 42-114 and `types.h:21-35`.
  2. The **runtime form** is the tagged cells above. The loader turns (1) into (2) (§3.2).
  - The compiler still emits both a "legacy" cell buffer and a CID blob in parallel (for example `compile_if`, compiler.c 2215-2280).
- **Native kernel.** 62 primitive IDs (`types.h:35-101`):
  - arithmetic, comparison, bitwise, logical
  - `@ ! c@ c!`
  - return stack `>r r> r@ rdrop 2>r 2r>`
  - `branch 0branch execute i0 pick`
  - `alloc`/`free`, `memcpy`
  - array ops, `mut`, `str-length`
  - map (HAMT) ops

  **There is no I/O primitive**: no print/emit/type/key. Results are observed only by the runner dumping the stack (`marchc -r word -s`).

### 1.3 Data stack and value representation
- The data stack is **type-erased**. "Each slot is a machine word… No `Value {tag, payload}` wrapper at runtime; static typing removes the need" (DESIGN-OVERVIEW.md 56-62). DESIGN-DETAILS.md:16: "We decided no tags in stack slots."
- Heap objects carry a header.
  - Arrays and strings use a **32-byte header**, `[count u64][elem_size u8][pad 7][elem_type u64][reserved u64]` followed by data (PROGRESS §14, lines 262-265).
  - HAMT nodes use a 32-byte header too (HAMT.md 79-95).
  - The design comment on this: "Stored elem_type for introspection, but VM never dispatches on it" (PROGRESS 294-298).
- Types at compile time (`types.h:108-125`): `i64 u64 f64 ptr bool str str! array array! any`, plus type variables `a`-`z`. **There is no quotation or function type.** A quotation value is typed `ptr` (compiler.c:1851, `push_type(comp, TYPE_PTR)`).

### 1.4 Words, definitions, dictionary
- **Dictionary.** A C hash table of entries: name, `prim_id` / `addr` / `cid`, `type_sig`, `is_immediate` + handler, and `word_def` (stored tokens) (LINKING.md 836-851; `dictionary.c`).
  - Overloads are resolved at compile time by `dict_lookup_typed` against the current type stack. The OCaml version used "specificity scoring (exact match=100, concrete→typevar=50, polymorphic=10)" (PROGRESS 541-545).
  - The DB has `UNIQUE(name, namespace, type_sig)` to allow overloading (schema.sql; PROGRESS 531-533).
- **Syntax at HEAD.**
  - `: name body ;`
  - an optional preceding signature `$ a -> a a a ;` (`compile_type_sig_decl`, compiler.c 2706; `--` is accepted as `->`)
  - `--` line comments
  - `( … )` quotations, `[ … ]` array literals, `"…"` strings
- **"Design B": words are stored as tokens and monomorphized at the call site** [IMPL+TESTED for simple cases].
  - Rationale (DESIGN-CONSIDERATION.md): per-word AOT cannot know call-site types, so "Monomorphize on first use… (NOTE: I like DESIGN B!)" (line 99).
  - `compile_definition` (compiler.c 2764+) **does not compile**. It collects tokens into a `word_definition_t` held in the compiler's in-memory cache. Token definitions are *not* stored in the DB (PROGRESS 410: "No database storage of token definitions yet").
  - At a call site, `compile_word` (692-897) looks up a specialization cache keyed by (name, concrete input types), linear search with max 512 (commit 4188ce4). On a miss it calls `word_compile_with_context` (compiler.c:1085) with the concrete types, stores the specialized blob as `BLOB_WORD` with a type-sig CID, and emits a CID reference.
  - The runner compiles top-level words on demand (commit c51fa44). STATUS.md 255: "Words are now like quotations - stored as tokens, compiled when used with concrete types."
  - **Caveat.** The call-site type-stack update uses `entry->signature`. That is either the declared `$` signature or a placeholder `0 inputs → 1 TYPE_UNKNOWN` output (compiler.c ~2911-2925). It is *not* the specialization's inferred outputs. "Polymorphic words still require explicit type signatures (`$ a -> a a a ;`)" (STATUS.md 251).
- **Immediate words.**
  - `if`, `times`, `true`, `false`, `dup`, `drop`, `swap`, `over`, `rot`, `_`, `march.alloc` are all registered as immediate words with C handlers (compiler.c 256-331). The stack words are immediate so they can track slot/node ids on the compile-time type stack.
  - `:` `;` `[` `]` `(` `)` are still special token types. CORE-ARCHITECTURE.md proposes making them immediate words and introducing `WORD_RUNTIME / WORD_IMMEDIATE / WORD_DUAL` kinds. Only `march.alloc` was done, as the "first dual word" (commit 8a7738b).

### 1.5 Quotations
- **What the docs say.**
  - Quotations are "thunks, but… not closures" (QUOTATIONS.md 9-10).
  - Two planned kinds: *lexical* `( body )`, which "must be consumed in current scope" and is inlined by immediate words at zero cost, and *typed* `( _i64 body )`, whose input types are declared with `_type` markers so it can be passed, stored, and `execute`d (QUOTATIONS.md 15-110).
  - The header says: "THIS DOCUMENT IS PARTIALLY OUT OF DATE. WE FOUND A WAY TO HANDLE QUOTES WITHOUT HAVING TO TYPE THEM -- treat them just like words, which have to be rarfified by the JIT." (QUOTATIONS.md 3-5)
  - Its checklist (352-361) shows `_type` markers, the LEXICAL/TYPED distinction, scope checking, and `map`/`each` as **not done**.
- **As implemented (QUOT_LITERAL, PLAN-QUOT-LITERALS.md; commit 8827b29)** [IMPL+TESTED for `if`/`times`].
  - `(` starts capturing *tokens*, not code (compile_lparen/rparen, compiler.c 1238-1363).
  - The consuming immediate word calls `quot_compile_with_context(comp, quot, parent_type_stack, depth)` (compiler.c 971-1080). This compiles the tokens with the caller's type stack as context, so polymorphic `<` in `( 3 < )` resolves.
  - `if` and `times` **inline** the compiled cells with `0branch`/`branch` and backpatched offsets (compile_if, 2160-2305).
  - `times` dispatches on how many quotations are on the compile-time quotation stack: 1 means a counted loop, 2 means an until loop (compiler.c 1871-1885).
- **Runtime (first-class) quotations** [IMPL, partly broken].
  - When a *non-immediate* word follows pending quotations, `materialize_quotations` (1749-1866) compiles each one with an **empty type context** (`quot_compile_with_context(comp, quot, NULL, 0)`, line 1760).
  - It stores the result as an anonymous `BLOB_QUOTATION` with a `sig_cid` (anonymous blobs avoid "words table pollution", PROGRESS 720-730) and pushes `TYPE_PTR`.
  - At link time a quotation reference becomes `[LIT addr-of-trampoline]` (LINKING.md 447-450, 470-473).
  - `execute` pops the address and `jmp`s to it (`execute.asm`).
- **How `execute` got its stack effect: it didn't** (details in §5b). The primitive is registered as `"a ->"` (primitives.c:158). The type checker just pops one value and does not apply the quotation's inferred effect.
- **Quotation `outputs`.** These are recorded as the *entire* type stack after compiling the body in context (compiler.c 1050-1053). `compile_if` then pushes `true_quot->outputs` onto a stack that still holds the context (2283-2285). It also never checks that both branches have equal effects ("Both branches should have same output types - use true_quot", 2282). From reading the code this looks wrong whenever the context is non-empty. I did not verify this beyond reading.

### 1.6 Evaluation order
- **Strictly eager, left to right, FORTH style.**
  - Nothing is lazy at runtime.
  - The only "deferral" is at **compile time**: word bodies and quotation bodies are kept as tokens and compiled late, at the use site or at first execution, with concrete types.
  - The word "lazy" appears only in two places: as "lazy AOT / AOT-per-specialization" (DESIGN-CONSIDERATION.md 95) and for NULL-root maps (HAMT.md 436-439).
- **Designed thunk typing, never built.** PLAN-TYPES.md sketches `Thunk[σ — τ ! E]`, `call`, `compose`, effect rows `!{IO|e}`, staged `Code^k[Γ ⊢ τ]`, and a C demo runtime of stack-effect thunks with *tagged* values and an effect bitmask. It is [DESIGN ONLY]; the "PROGRESS" section at line 5 says only "Where are we with all this?"

### 1.7 Effects and I/O
- **No I/O was ever implemented.** There are no I/O primitives. PROGRESS 636-637 lists "String literals" (later done) and "Print primitive - No way to output strings yet"; print never appeared.
- **Ordering.** Effects are ordered only by the sequential execution of the cell stream.
- **Designed effect tracking (not wired up).**
  - `defs` table columns `is_pure`, `effects` ("IO=1, ERR=2"), `escapes` (schema.sql). `defs` has 0 rows in `march.db`.
  - PLAN-TYPES.md §4 and §7 (effect sets in thunk types, E ∪ F under composition).
  - DESIGN-ARRAYS.md:28 ("The compiler will need to track usage, possibly using effect tokens").
  - All [DESIGN ONLY].

---

## 2. Memory model

### 2.1 Workspace ("stack's heap") vs global store: design only
- DESIGN-OVERVIEW.md §5 (78-90) describes two heaps.
  1. The **Stack's Heap (Workspace)**: "Short-lived, mutable allocations… Automatically freed when the word/frame returns. No reference counting required; purely linear ownership."
  2. The **Managed Global Store**: "Immutable, content-addressable objects persisted in the database… Updated via 'freeze': converts a mutable workspace object into an immutable snapshot, computes its hash, and stores it."
- The deleted `docs/FFI.md` (git bb84fd3^) is similar: "an in-memory immutable global store with periodic persistence to SQLite… SQLite only stores *snapshots*". It also lists memory layers: Workspace heap, then Global store (in memory, CID nodes), then SQLite snapshots.
- **As built.**
  - There is no `freeze`/`thaw` anywhere in `src/` or `kernel/` (grep finds nothing).
  - The `state` and `state_history` tables exist in schema.sql but are unused (0 rows, no code references them).
  - The Rust `im` state store is not linked.
  - Runtime heap values are plain `malloc` (`alloc.asm`, `mut.asm`, `array-concat.asm`).
  - String literals are the one "store-like" thing: `BLOB_STRING` in the DB, loaded once and cached by the loader. "Database = Persistent storage… Loader = Runtime cache… Heap = Mutable runtime allocations" (PROGRESS 289-292).

### 2.2 Uniqueness / borrow inference: design only
- DESIGN-OVERVIEW.md §6 (93-123) describes an SSA lattice ⊥/U/B/E/⊤.
  - Fresh alloc, thaw, or fetch_global gives U.
  - Alias gives B.
  - Store, return, or send gives E and inserts `freeze`.
  - A value that is U and non-escaping lowers to an in-place `set-name!`; otherwise the update is a persistent structural share.
- DESIGN-DETAILS.md §8 repeats this: "insert freeze exactly at store/return boundaries".
- **None of it was implemented.** In its place came explicit mutability types: `array`/`str` plus `mut` → `array!`/`str!` (a deep copy), with `array!` accepted wherever `array` is expected (PROGRESS §15; compiler.c type compatibility).
- The docs contradict each other on the default.
  - STATUS.md:116 (2025-11-04): "**Mutable-by-default**: Arrays/strings are mutable by default… Explicit conversions: `dup` aliases, `copy` duplicates, `imm` makes persistent."
  - PROGRESS §15 (11-02): "Immutable types: `array`, `str` (default)".
  - PLAN-ARRAYS.md 91-93: "Decided NOT to implement naive copy-on-write… Current `array` type remains simple, semantically immutable by convention."

### 2.3 Compile-time memory tracking, in three successive attempts
1. **Compile-time reference counting** (before 2025-10-30).
   - Abandoned: "Reference counting approach broke polymorphic words - immediate handlers (dup, drop) need type context during compilation, but polymorphic word definitions (`: ddup dup dup ;`) have no concrete types." (PROGRESS 424)
   - The `test/test_rc_*.march` files are leftovers.
2. **Slot-based tracking** (commit a5bcb96, 2025-10-30).
   - The type stack holds `{type, slot_id}`. At `;`, the compiler emits FREE for "allocated - returned" slots (PROGRESS §11, 426-433).
   - Reported tests: `"hello" drop` → FREE emitted; `"keep" "drop" drop` → only slot 1 freed.
   - **But the runtime FREE is a stub**: `free.asm` "TODO: Implement runtime slot array and free(slots[slot_id])… For now, this is a no-op" (free.asm 1-20). PROGRESS §11: "FREE primitive added (free.asm) - runtime stub, infrastructure TODO".
3. **Reference graph ("refgraph")**, docs MEMORY-MANAGEMENT.md, PLAN-REFGRAPH.md, research/mem.md.
   - **Design.**
     - Per-word "mini-graphs": nodes = heap objects, edges = containment, stack slots reference node ids, `dup` aliases the same node.
     - Liveness is mark-sweep reachability from the type stack plus escaped nodes; FREE is emitted for unreachable nodes, children first.
     - Mini-graphs are stored with the CID and "stitched" across words at JIT time.
     - It then concludes: "(IMPORTANT! Further analysis reveals that it might not matter the concrete types, and all of it can be done at compile time instead.)" (MEMORY-MANAGEMENT.md 203), and 298-343 argue that compile-time FREE bytecodes suffice, with no JIT.
     - Open issues: dynamic `at` indices ("Start conservative (all children potentially live)", PLAN-REFGRAPH 519-526); control flow (straight-line only, then conservative, then flow-sensitive); slot vs node lifetime.
     - "Escaped" nodes are permanent roots, a "safety valve".
     - The fallback rule: "Any node whose lifetime cannot be proven finite… is marked as non-managed by this system and must be freed via other mechanisms." (MEMORY-MANAGEMENT.md 295)
     - `research/mem.md` gives a categorical reading: a strong symmetric monoidal functor 𝒢: Word → RefGraph, composition = pushout, liveness = a counit ε selecting the reachable subgraph. The "Theorem — Functorial Liveness Preservation" is a proof *sketch* only.
   - **Implemented** [IMPL, largely untestable]: `src/refgraph.c` (195 lines; create/free/alloc-node/add-child), plus `node_id` on type-stack entries.
     - `compile_rbracket` creates nodes and parent→child edges (compiler.c ~1682-1722).
     - `emit_free_for_dead_nodes` (compiler.c 448-525) runs **only at word end** inside `word_compile_with_context`, not at `drop`. `compile_drop` says "In slot model, we don't free here" (≈2390).
     - It emits `LIT slot_id; XT free` into the no-op FREE.
     - `type_is_heap()` returns `false` unconditionally (compiler.c 335-340).
     - `mark_escaped` and `at`-extraction edges are not implemented.
   - **Outcome.**
     - STATUS 2025-11-05 was "BLOCKED" by the array segfault. The fix came in b1b26b6 on 11-12: primitives were clobbering `rbx`/IP, and STORE had its operand order reversed. That commit says "This unblocks Phase 2 reference graph testing", but no later commit tests refgraph FREE behaviour.
     - **Nothing was ever freed at runtime, and nothing was measured.**

### 2.4 The underscore conventions (`_`)
`_` has meant three different things.
1. **Typed-quotation input markers (design).** In `( _i64 _i64 + )`, the `_type` prefix declares quotation inputs: "Type markers use underscore prefix: `_typename`" (QUOTATIONS.md 90-110). Never implemented.
2. **A raw/untracked primitive prefix (design).** `_alloc` = raw allocation with no tracking, versus `alloc` = the immediate, tracked version (CORE-ARCHITECTURE.md 86-92, 168-170).
3. **Array-comprehension "pull"** (implemented; this is what all the `test_underscore*.march` files at the repo root exercise). DESIGN-ARRAYS.md 40-55 defines it.
   - `[ … ]` are "natural comprehensions": `[ 1 2 + ]` gives `[ 3 ]`.
   - "Values on the stack can be pulled into the construction of an array using `_`, e.g. `3 [ _ ]` results in `[ 3 ]`, and `5 [ _ 1 + ]` results in `[ 6 ]`."
   - The old implicit rule (`1 1 [ + ]` → `[ 2 ]`, consuming the pre-`[` values) was rejected as "too confusing and has too strange edge cases. Being explicit with `_` seems a much better choice."
   - **How it is built** (`compile_underscore`, compiler.c 2570-2657; commits 028a691, d3c4870):
     - `_` is an immediate word that works only inside `[ ]`.
     - The *k*-th `_` copies the *k*-th value below the `[` marker, counting from the marker downward, using `dup`/`over`/`n pick` (the new `pick.asm`). It records the node/slot id.
     - At `]` the consumed pre-marker values are dropped (`>r drop … r>`).
     - Earlier `_` was a runtime identity primitive (`PRIM_IDENTITY`, PROGRESS §13). Its registration is now commented out (primitives.c:102).
   - **My runs at HEAD:**
     - `5 [ _ 1 + ] 0 march.array.at` → 6
     - `5 7 [ _ _ + ] 0 at` → 12
     - `2 3 [ _ _ + ]` → an array
     - Unpulled values are not consumed: `5 [ 3 [ _ ] 0 at ] 0 at` leaves 5 and 3.
   - `test_no_underscore*.march` check the rejected implicit form. `1 2 [ + ] 0 march.array.at` returned `3 0` in my run, so it is not a clean array.

### 2.5 HAMT and persistent data structures [IMPL+TESTED in C; lightly in March]
- **C implementation.** `src/hamt.c` (462 lines) + `hamt.h`: a 32-way bitmap trie, FNV-1a hashing of i64 keys, path copying, NULL for the empty map, size cached in each node, `march.map.free` for manual recursive free (HAMT.md 426-451).
- **The key lesson.** A per-slot pointer tag (bit 0 = leaf value) replaced node-level flags, which "caused segfaults" (HAMT.md 121-130, 493-497). This was learned from mkirchner/hamt, which is vendored as `src/hamt/`.
- **Primitives:** `march.map.new/get/set/remove/size/free` (map-*.asm). Keys and values are i64 only; there is no `map` type distinct from `ptr` in `types.h`.
- **Tests:** top-level `test_hamt*.c`, `test_libhamt.c`, and `test/test_map*.march`.
- **Measured: nothing.** The performance section (HAMT.md 350-384) is expected or asymptotic only ("~12% overhead", "Batch updates 10-100× faster" for transients that were never built).
- **RRB-tree vectors:** planned only (PLAN-PERSISTENT-DS.md, all ☐). `march.array.imm.*` is "reserved" (PROGRESS 21-25).

---

## 3. Content addressing, database, linking, namespaces, types, syntax, arrays

### 3.1 Content addressing
- CIDs are SHA-256; they were first hex strings, then 32-byte binary (commit 2feed52).
- **Words.** The CID is the hash of the encoded blob, not of the source: "different source producing the same code gets the same CID" (LINKING.md 240-248). EXIT is not stored.
- **Primitives** have **fixed IDs, not hashes**. "Assembly implementations can change… Compiled code must remain valid" (LINKING.md 194-217). Resolution is an O(1) `primitive_dispatch_table[256]` (commit 4074dd9).
- This is a departure from the original CLAUDE.md, which said primitives are "stored with an index key that is a SHA256 content-hash of the machine code… also index by architechure" (docs/CLAUDE.md 16-19).
- **Literals** are hashed per value (`BLOB_DATA`); small ones were later inlined (commit c8b9f5a).
- **Type signatures** are deduplicated in `type_signatures` (sig_cid = SHA256 of "in|out") (PROGRESS 724-736).
- **Specializations** are stored as separate `BLOB_WORD`s with a concrete sig, so each monomorphized instance has its own CID.
- **Designed but not done:** `bytecode_version` per code object ("A little scared of this idea though", DESIGN-DETAILS.md 50-58), manifests/roots GC (DETAILS §5), and the `edges` table (0 rows; "edges table - Not populated yet", PROGRESS 638).

### 3.2 Linking (LINKING.md, `src/loader.c`) [IMPL+TESTED]
- The loader recursively resolves CIDs, caching CID → address.
- **The blob kind decides the linking behaviour:**

  | Kind | Emitted cell |
  |---|---|
  | `BLOB_WORD` | `[XT trampoline]` (call it) |
  | `BLOB_QUOTATION` | `[LIT trampoline-address]` (push it) |
  | `BLOB_DATA` | `[LIT value]` |
  | primitive tag | `[XT &op]` |

- "No operation tags needed in the CID sequence! The kind field in the metadata provides all the information." (LINKING.md 468-475)
- "The loader can link code without examining type signatures." (LINKING.md 515-517)
- Code is fully relinked into fresh memory at every load; the in-memory image is never persisted.

### 3.3 Database (`schema.sql`, 313 lines; `march.db`)

| Table | Status |
|---|---|
| `blobs` (cid, kind, sig_cid, flags, len, data) | used, 83 rows |
| `type_signatures` | used, 3 rows |
| `words` (name, namespace default `'user'`, def_cid, type_sig, is_primitive, architecture, is_immediate, UNIQUE(name,namespace,type_sig)) | used by code, 0 rows in the shipped db |
| `defs` (bytecode_version, is_pure, effects, escapes, source_text, source_hash) | 0 rows |
| `edges` | unused |
| `modules`, `module_exports`, `module_imports` | unused |
| `state`, `state_history` | unused |
| `symbols` | unused |
| `metadata` | march_version 'α₄' |

- `src/schema.sql` is a smaller 45-line variant.
- SYNTAX.md 102-112 also proposes a `parse_operations` table for lossless round-tripping. Not implemented.

### 3.4 Namespaces
- These exist only as **dotted naming conventions** for primitives: `march.array.*`, `march.array.mut.*` (in-place, requires `array!`), `march.array.imm.*` (reserved), `march.map.*`, `march.alloc`.
- The rename away from the `!` suffix aimed to keep "clean names like `set`, `fill`, `reverse`" for later type-dispatched overloads (PROGRESS §19, 27-32; PLAN-ARRAYS 80-84).
- The DB `namespace` column always defaults to `'user'`. There is no namespace resolution, no import/export, and no module system. PLAN-DIRECT-THREADING.md 234-245 mentions `march.ts.*` token-stream words for self-hosted parsing words [DESIGN ONLY].

### 3.5 Types (PLAN-TYPES.md + implementation)
- **Implemented:**
  - static checking with a shadow type stack
  - overloads selected by concrete type
  - type variables `a`-`z` in signatures
  - `array!`/`str!` mutability variants
  - output inference from the body when there is no signature
  - input inference is missing ("Can't infer : square dup * ;", PROGRESS 632)
- **PLAN-TYPES.md** is mostly an AI-written exploration: thunk typing options 1-12 and two identical copies of a ~470-line C "thunks.c" demo (lines 185-681 and 691-1159).
  - The recommended first steps: "Start with stack-effect thunks: `Thunk[σ—τ]`, `call`, `compose`, `map`. Add effects as annotations…" (175-181).
  - The author's note on closures and lifetimes: "Doubt we will eve need this since we don't have closures." (58)
  - Nothing from it is in the code.
- **DESIGN-CONSIDERATION.md** is the key per-word-AOT argument: polymorphism, dup/drop resource ops, and region lowering "all want call-site info", so keep those ops symbolic. Design A is call-site adapters; Design B is monomorphize on first use, and B was chosen.
- **SYNTAX.md:** "Guards are runtime checks, defining execution contexts"; type signatures are compile-time constraints; both are inherited by subsequent definitions (79-100). [DESIGN ONLY]

### 3.6 Syntax (SYNTAX.md) [mostly DESIGN ONLY]
- **Premise.** "March's syntax is a serialization format, since the database is the true primary source" (3-5).
- **Multi-context execution:** runtime, compile ("emit database operations"), and format (regenerate text from stored operation sequences) (29-51).
- **Parsing words** as user-defined grammars; Logo-style `to … end` as an alternative to `: … ;`; a "Syntax Option 2" (`name = [ … ]`); and an S-expression format ("NOT WORKED ON YET").
- The text marks "--- STOP READING HERE ---" at line 131. Everything after it is speculative.
- PLAN-DIRECT-THREADING.md 158-250 raises the question of whether `:` should require an explicit quotation (`: foo [ … ]`) or build one implicitly. It poses the question: "Do we allow the parsing words to handle the internal details… Or should parsing words only be allowed to modify the token stream".
- **As built:** a fixed C tokenizer with special tokens for `: ; [ ] ( )` and `$` signatures.

### 3.7 Arrays (DESIGN-ARRAYS.md, PLAN-ARRAYS.md) [IMPL+TESTED]
- **Design intent:** the programmer "typically will not need to worry about the underlying implementation; the compiler will pick the best implmentation to use base on usage analysis" (DESIGN-ARRAYS.md 8-10), with a user override. This is DESIGN ONLY.
- **Built:**
  - `[ … ]` literals, with the `]` codegen doing alloc, header write, and stores. Arrays are homogeneous only (heterogeneous tuples are rejected).
  - Nested arrays; `march.array.length/at/concat`; `march.array.mut.set/fill/reverse`; `mut`.
  - Out-of-bounds `at` returns 0.
- "Arrays are production-ready for mutable use cases!" (PLAN-ARRAYS 114). Yet array execution segfaulted until 2025-11-12 (see §4).

---

## 4. What worked, what failed or was abandoned, and why

### 4.1 Worked (according to the docs and commits)
- **Cell VM with 4 tags.** "All 6 VM tests now passing… VM is now fully operational" (PROGRESS 619-627).
- **The C compiler.** "All 215 tests passing!" (PROGRESS 672). Later "All 220 tests pass across 7 test suites" (commit 4074dd9). *But see §4.3: those suites no longer compile at HEAD.*
- **The full pipeline:** "Source → Compile → Store → Load → Execute" (PROGRESS 694). `: answer 21 21 + ;` → 42.
- **User → user calls** after direct threading: "': ten-via-five five five + ;' → Stack: 10 ✓" (e1d7e88).
- **Conditionals and quotation execute** (687c986, 9418f2e), and **counted/until `times`** (8827b29 etc.).
- **Design B** monomorphization and specialization cache (c16fa9f, 4188ce4, c51fa44).
- **HAMT** ("Implementation Status: ✅ Complete", HAMT.md 470).
- **Strings:** interned via the DB with a loader cache; "String 'hello' repeated 1000 times: Before: 1000 alloc + 1000 memcpy calls / After: 1 DB fetch + 999 pointer returns" (PROGRESS 283-287).

### 4.2 Failed, abandoned, or never finished
- **OCaml compiler:** abandoned because it pulled toward an AST (PROGRESS 650-656).
- **Subroutine threading (call/ret):** replaced because it "mix[ed] CPU stack with FORTH's return stack" (9418f2e); `branch`/`0branch` had corrupted the return stack (687c986).
- **Compile-time RC:** abandoned; it broke polymorphic words (PROGRESS 424).
- **Slot-based FREE:** runtime "no-op" stub (free.asm).
- **Refgraph Phase 2:** "BLOCKED: Cannot test until array runtime bug is fixed" (STATUS.md 46). The bug was fixed a week later, but refgraph FREE was never validated, and the runtime FREE is still a no-op.
- **Array runtime:** "Array execution completely broken - even simple `[ 1 2 3 ]` crashes with segfault" (STATUS.md 16). Root cause: "Multiple assembly primitives were using rbx (VM instruction pointer) as a temporary register" plus reversed STORE order (b1b26b6).
- **Typed quotations (`_type`), `map`/`each`, polymorphic quotations, specialization of `execute`:** not done (QUOTATIONS.md 352-361).
- **Global store / freeze / uniqueness analysis / Rust `im` store:** not done (§2.1-2.2).
- **I/O / print:** never done.
- **Edges and GC, modules:** not done.
- **Syntax words as immediate words (`: ; [ ]`)** and a self-hosted outer interpreter: planned in CORE-ARCHITECTURE.md and PLAN-SELF-HOSTING.md; only `march.alloc` was done as a dual word.
- **INET.md** (deleted 2025-11-04; git d7fa11d): "Interactive Nets Compilation Target — NOTE: This is a long term goal. IGNORE FOR NOW!" It sketched FORTH → primitives → interaction-net IR ("Stack becomes wires… Words become net fragments") → optimize → native asm. This is the seed of what march5 later tried.

### 4.3 My own check at HEAD (scratch build: `make vmlib`, `src/make all`; marchc builds)

**The C unit tests do not compile.** `make test` fails with `runner_create` / `dict_add` arity errors in test_immediate, test_dict, test_quotations, and test_loader, plus link errors in test_database and test_compiler.

Running `marchc -r <word> -s`:

| Test | Result |
|---|---|
| `: test-if-true 1 ( 111 ) ( 222 ) if ;`, `: test-if-false 0 ( 333 ) ( 444 ) if ;`; ran `-r test-if-false` | 3 stack items: 444 and two pointer-like values |
| `ddup` test | 5 5 5 ✓ |
| `test-exec 5 ddup drop drop` | 5 ✓ |
| `3 ( i0 ) times` | 0 1 2 ✓ |
| `test_no_refgraph` | 42 ✓ |
| `test_refgraph_simple` / `march.alloc` | return a pointer ✓ |
| `( 42 ) execute ( 99 ) execute` | **segfault** |
| `( 32 add-ten ) execute` | "Type error: no matching overload for word: +", because materialized quotations and the words inside them are compiled with an empty type context |
| until-style `0 ( 3 < ) ( 1 + ) times` | empty stack (expected 3) |
| `5 ( 1 + ) ( 2 * ) times` | empty stack |

---

## 5. Bearing on march6's open questions

### (a) Lazy-by-default vs strict stack execution with explicit deferral
- march4 was **strict at runtime**. All deferral was compile-time: tokens were stored for words and quotations and compiled at the use site with the caller's concrete type stack.
- **What that bought:** overload resolution for polymorphic ops inside quotations (`( 3 < )`) and per-call-site monomorphization without whole-program analysis.
- **What it cost:**
  - A definition has no standalone compiled meaning; it has meaning only per call-site type context. The DB therefore holds many specialization CIDs, and the token definitions themselves were never persisted.
  - Output types at call sites still needed explicit `$` signatures.
- The docs never propose runtime laziness. PLAN-TYPES' "thunks" are explicit `call`/`force` values.

### (b) How dynamic quotation application knew stack effects
- **It didn't.**
  - `execute` is typed `a ->` (primitives.c:158), and a runtime quotation is just `ptr` on the type stack (compiler.c:1851).
  - The quotation's inferred signature is stored as `sig_cid` on its blob, but it is never consulted when typing the `execute` site. The loader ignores signatures too (LINKING.md 515-517).
- The only sound case was the **immediate-consumer path**: `if`/`times` pop quotations from a *compile-time* quotation stack, compile them in context, and inline them.
- **Proposed but never built:**
  - typed quotations with `_type` input markers and inferred outputs (QUOTATIONS.md)
  - "Specialize on first call, cache result" for `5 ( _a _a ) execute` (QUOTATIONS.md 277-282)
  - `Thunk[σ—τ ! E]` with a typed `call` rule (PLAN-TYPES.md §7)
- The later pivot "treat them just like words, which have to be rarfified by the JIT" (QUOTATIONS.md 5) means monomorphizing quotations at the use site like words. For first-class quotations that requires knowing the quotation at compile time, which is exactly the case `execute` cannot guarantee.
- The observed failures in §4.3 (`execute` segfault; empty-context compile failures) are symptoms of this gap.

### (c) Effects, I/O, and store-write sequencing
- **Nothing implemented.** There are no I/O primitives, no store writes (the `state` table is unused, there is no freeze), and no effect tracking.
- Sequencing was implicit: the linear order of the cell stream on a single stack machine.
- **Designs on record:**
  - Effect bits on `defs` (`is_pure`, `effects`, `escapes`).
  - Effect rows in thunk types that union under composition (PLAN-TYPES §4 and §7).
  - "freeze" inserted at escape boundaries (store/return/send), as the one point where workspace data enters the global store (DESIGN-OVERVIEW §5-6; DETAILS §8).
  - FFI.md: in-memory immutable store, persisted to SQLite "on commits"/"snapshots", with `state_history` in the schema for per-name value history.

### (d) Global store and namespaces
- The **design is a two-tier heap**: a mutable, linear workspace freed at word return, and an immutable CAS global store updated by freeze.
- That was never built. The only CAS objects were code, literals, strings, and type sigs in SQLite.
- The Rust `im::HashMap` "State" (`march_state_get/set`) and the in-C HAMT are the two persistent-map candidates. Neither was wired to named state.
- Namespaces were a naming convention only (`march.array.mut.*`). The `words.namespace` column and the module tables are unused.
- Note the recurring intent to reserve short names for future type-dispatched overloads.

### (e) Performance numbers
- **There are none measured.** No benchmarks, timings, or perf runs exist in docs, tests, or commits.
- The only figures:
  - encoding size estimates: "~44% space savings", 4 bytes vs 64 for `dup +` (LINKING.md 925-974)
  - instruction counts: "String 'hello world'… 60+ instructions → 5 instructions" (PROGRESS 364-366)
  - the string-cache alloc count (PROGRESS 283-287)
  - asymptotic HAMT tables
- PLAN-DIRECT-THREADING's "Benchmark performance improvement" was never done.

### Other march4 ideas worth carrying forward (as ideas, not specs)
- **Fixed primitive IDs** rather than hashing machine code, so compiled CIDs survive kernel changes.
- **Blob kind decides link behaviour** (call vs push vs value), with no per-reference opcodes.
- **Compile-time quotation stack** separate from the type stack, with immediate consumers inlining.
- **Dual words:** an immediate handler plus a runtime primitive, as the answer to "compile-time tracking + runtime behavior".
- **The explicit `_` pull** instead of implicit consumption into `[ ]` comprehensions.
- **The refgraph's honest fallback clause** (MEMORY-MANAGEMENT.md 295) and its conservative treatment of dynamic indices.
- **The categorical framing** in mem.md (words as morphisms, composition as pushout), which matches a dataflow-graph view.

---

## 6. Files read

### Markdown in the tree (all read fully)
- `docs/CLAUDE.md` (47): α₄ overview. FORTH core, SQLite program store, SHA256 CIDs for primitives, words, and literals; STATUS/PROGRESS conventions.
- `docs/STATUS.md` (257): session log 10-31 → 11-05. Design B phases, slot memory, refgraph Phase 1/2, "BLOCKED" array segfault; dates mislabeled 2024.
- `docs/PROGRESS.md` (822): feature ledger §1-19. VM, OCaml → C, 215 tests, quotations/execute, slot memory, Design B, arrays, strings, headers, `mut`, namespacing; "What's NOT Working" list.
- `docs/design/DESIGN-OVERVIEW.md` (158): typed-FORTH execution model, cell tags, type-erased stack, workspace vs global store, freeze, U/B/E uniqueness lattice.
- `docs/design/DESIGN-DETAILS.md` (90): CID → Word* relocation, no stack tags, EXT form, backpatching, manifests/GC, bytecode versioning, FFI ABI, auto-mutability, peepholes.
- `docs/design/DESIGN-CONSIDERATION.md` (142): per-word AOT vs call-site info. Design A adapters vs Design B monomorphize on first use ("I like DESIGN B!").
- `docs/design/CORE-ARCHITECTURE.md` (183): special tokens vs immediate words; WORD_RUNTIME/IMMEDIATE/DUAL; `_alloc` vs `alloc`; phased plan.
- `docs/design/DESIGN-ARRAYS.md` (56): naming, concrete array types chosen by the compiler, comprehension syntax with explicit `_`; rejection of implicit pull.
- `docs/design/QUOTATIONS.md` (361): lexical vs typed (`_type`) quotations, inlining rules, performance table, "partially out of date" note, status checklist.
- `docs/design/MEMORY-MANAGEMENT.md` (388): refgraph design, mini-graphs, type holes, liveness, cross-word stitching, escape/fallback, compile-time vs JIT free placement, slots + graph.
- `docs/design/HAMT.md` (497): HAMT algorithm, layout, pointer tagging lesson, FNV-1a, ops, decisions, status/tests.
- `docs/design/LINKING.md` (1040): 2-byte tag blob encoding, CID rules per blob kind, fixed primitive IDs, recursive cached linker, worked examples, migration, space analysis.
- `docs/design/SYNTAX.md` (291): DB-as-source, multi-context (runtime/compile/format), parsing words, guards vs types, Logo/alt/S-expr syntaxes (speculative after line 131).
- `docs/planning/PLAN-DIRECT-THREADING.md` (335): 4-tag encoding + direct threading plan (marked complete), FORTH outer interpreter, `:` as a parsing word, `march.ts.*`.
- `docs/planning/PLAN-QUOT-LITERALS.md` (106): QUOT_LITERAL token capture + `quot_compile_with_context`; marked complete.
- `docs/planning/PLAN-ARRAYS.md` (210): array API status, namespacing, `]` codegen walkthrough, ALLOC fix.
- `docs/planning/PLAN-PERSISTENT-DS.md` (71): HAMT done; RRB-tree vectors planned.
- `docs/planning/PLAN-REFGRAPH.md` (554): refgraph Phases 1-6 with C pseudocode, FREE_REF asm, 5 tests, open issues.
- `docs/planning/PLAN-TYPES.md` (1189): thunk/quotation typing options (stack-effect, staged, effects, lifetimes), practical defaults, two copies of a C thunk demo.
- `docs/planning/PLAN-SELF-HOSTING.md` (14): vision of the compiler in March via immediate words; "no AST".
- `docs/planning/PLAN-CORE-INTRASTRUCTURE.md` (0): empty.
- `docs/planning/PLAN-MEMORY-MANAGMENT.md` (0): empty.
- `docs/research/mem.md` (274): categorical semantics of refgraphs (functor Word → RefGraph, pushout, liveness counit, string diagrams).
- `runtime/README.md` (42): Rust `im`-based State store with C API; "will eventually be rewritten in March".
- `skills/testing/SKILL.md` (1): TODO placeholder.

### Deleted docs, read from git history (skimmed)
- `docs/INET.md` (d7fa11d): interaction-net compilation target, long term.
- `docs/FFI.md`: global store + OCaml/Rust FFI phasing.
- `docs/design/DESIGN-STATUS.md`: 2025-10-29 status snapshot, 45 primitives, ~6,400 LOC C.

### Source surveyed or read
- `kernel/x86-64/vm.asm` (read fully), `docol.asm`, `execute.asm`, `free.asm` (the no-op stub), `dup.asm`, `i0.asm`, `alloc.asm`; plus the file list of 64 asm files.
- `src/compiler.c`: registrations 256-331; type/slot helpers 335-430; `emit_free_for_dead_nodes` 448-525; `compile_word` 692-897; `quot_compile_with_context` 971-1080; rparen 1291-1363; `materialize_quotations` 1749-1866; `compile_if` 2160-2305; `compile_drop` 2372-2410; `compile_underscore` 2570-2657; `compile_definition` 2764+.
- `src/types.h`: blob kinds, PRIM ids, type enum.
- `src/loader.c`: function list, DOCOL trampoline.
- `src/primitives.c` (`_` and `execute` registration); `src/tokens.c` (comments); `src/refgraph.c` (API); `src/Makefile`; top-level `Makefile`.
- `schema.sql` (read), `src/schema.sql` (head); `march.db` (row counts).
- `runtime/src/lib.rs` (API).
- `test/` (72 `.march` files, names plus several contents); top-level `test_*.march` (underscore/alloc/store files read); `test_*.c` (HAMT/VM tests, names).
- `examples/`, `resources/*.ml` (names).
- Git log and commit messages for all 87 commits; full messages read for the key ones.
