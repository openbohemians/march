# March2 digest (archived lineage review)

Source examined read-only: `/home/trans/my/com/tabcomputing/march/march2` (working tree = ref `archive/march2/preserved/wip-2026-09-23`), plus git refs `archive/march2/main`, `archive/march2/bootstrap-forth`, `archive/march2/preserved/wip-2026-09-23` in `/home/trans/my/com/tabcomputing/march`. Builds and tests were run only in scratch copies (`scratchpad/m2copy` = working tree, `scratchpad/m2main` = `git archive archive/march2/main`).

## 0. How the refs relate (read this first)

march2 contains **two different interpreters**, and the docs describe both of them mixed together.

| Ref | Head | What it contains |
|---|---|---|
| `archive/march2/main` | `3c618a5` "First attempt." (2025-10-11) | Interpreter A. One 2,160-line `src/main.rs`, a parse/eval "first attempt". It has multi-method dispatch, `?` guard constraints, `$` state, `raise` re-dispatch, `{ }` collections. Tests are 24 `tests/*.march2` files. README.md, EXAMPLE.md, NAMESPACE_DESIGN.md and SYNTAX.md (older version) describe this interpreter. |
| `archive/march2/bootstrap-forth` | `96e3f24` "Defer database work, plan type system overloading" (2025-10-14) | 12 commits on top of main. They **delete Interpreter A** and replace it with Interpreter B, "a true bootstrap design" (PROGRESS.md:13): a modular Rust FORTH (`src/forth.rs` and friends) with an XT dictionary, namespace stack, CIDs, a SQLite DB, and `TEST.` in `.fth` files. The main-branch PROGRESS.md was renamed OLD_PROGRESS.md ("We started fresh, so this no longer applies", OLD_PROGRESS.md:3). The diff against main is 38 files, +7036/−2610. |
| `archive/march2/preserved/wip-2026-09-23` | `a526085` "Archive uncommitted March 2 design state" (2026-09-23) | bootstrap-forth plus one commit that only edits DESIGN.md. It deletes the "Phase 2: Toward Compilation", "Phase 3: Interaction Net Construction" and "Phase 4: Inet Optimization" sections (−229 lines) and bumps the version from 0.2 to 0.4. |

Note: the `.march2` test files, Makefile and README.md in the working tree still belong to Interpreter A. The current binary cannot run them: `Makefile:91` loops over `tests/*.march2`, and README.md:153 says "`src/main.rs` - Interpreter implementation". A history-only file, `DESIGN3.md` ("Symbols all the way down"), was added in `f7050c6` and deleted in `36c1ef5`. I read it from history.

---

## 1. Execution model as implemented

### 1a. Interpreter B (bootstrap-forth / working tree), the "bootstrap FORTH experiment"

- **Shape.** A token-at-a-time outer interpreter over a refillable line buffer (`src/input.rs:35-65`). `Forth::eval_token_with_input` (`src/forth.rs:253-370`) tries the following in order: string literal, i64 literal, namespace name (sets a one-shot `lookup_namespace`), dictionary word, global variable. A word runs immediately if it is `immediate` or if the interpreter is interpreting. Otherwise it is appended to the current definition or to the open quotation. `:`, `::`, `;`, `(`, `)`, `--`, `NAMESPACE.`, `IMPORT.`, `ALIAS.`, `VARIABLE.`, `SIGNATURE.` and `TEST.` are all native immediate words (`src/forth.rs:104-172`). This is the "Everything is a word, including `:` and `;`" claim (PROGRESS.md:242). It is true.
- **Compiled form.** `XT` is an enum (`src/xt.rs:10-58`). It has 23 primitives, `Compiled(Vec<XT>)`, `Literal(Value)`, `Native(fn)`, and `Cid(CID)`. `Cid` is an error if executed (`src/forth.rs:581-584`). Execution is a recursive tree walk over `XT::Compiled` (`src/forth.rs:570-574`). There is **no threaded code, no instruction pointer, and no use of the return stack for calls**. The return stack is only user-visible storage for `>r r> r@` (`src/forth.rs:497-513`).
- **Binding is by value copy (early, inlined).** When a word is compiled into another word, its whole `XT` is cloned in (`src/forth.rs:333`). Consequences I observed in a scratch run:
  - Redefining `sq` after `: quad sq sq ;` leaves `quad` unchanged (`3 quad .` printed 81 after `sq` was redefined as `dup +`).
  - Recursion is impossible: `: fact ... fact ... ;` gives "Error: Unknown word: fact". The name is only inserted at `;` (`src/forth.rs:886-887`).
- **Values.** `Value` (`src/value.rs:60-75`) is one of: Number(i64), Quotation(Vec<XT>), String, Type (first-class), Word(Box<Word>), `im::Vector` Array, `im::HashMap` Map, and Mutable variants. No syntax creates arrays or maps. They exist only for `mutable`/`immutable` conversions (`src/forth.rs:1159-1185`).
- **Strictness.** Fully strict and eager. The data stack is a plain `Vec<Value>` (`src/forth.rs:18`).
- **Quotations.** `( ... )` are immediate words that push a `QuotationContext`, so nesting works (`src/forth.rs:897-941`). `call`, `if` (cond t f) and `iff` (cond q) execute the XT vector (`src/forth.rs:514-563`). There are no loops in B: no `#do`, `times` or `begin`.
- **Immediate words (`::`).** `::` marks the definition by prefixing the name with `__IMMEDIATE__` (`src/forth.rs:802`, stripped at `824`). Immediate words run against the live data stack during compilation. There is no `LITERAL`/`POSTPONE`. Observed: `:: five 5 ; : g five ;` leaves 5 on the compile-time stack, and `g` compiles to nothing.
- **Variables.** `VARIABLE. x expr ;` evaluates the expression right away and stores the result in `global_state: HashMap<String,Value>` (`src/forth.rs:1043-1097`). The docs call this "immutable" (PROGRESS.md:270-275), but `->` just overwrites the HashMap entry (`src/forth.rs:1099-1120`). **Inside a definition, a variable reference is compiled as a Literal of its current value** (`src/forth.rs:361-363`). Observed: `: show counter . ;` then `5 -> counter` then `show` still prints 0. `->` cannot be compiled at all ("Error: Word '->' has no CID - cannot compile"). The reason: `->` is a non-immediate native that reads its operand from the input stream at run time (`src/forth.rs:1103`), and it has no CID.
- **How far the bootstrap got.** The primitives, `:`/`;`/`::`, quotations, if/iff, strings, namespaces, variables, first-class types, `SIGNATURE.`, CIDs, the SQLite layer and the `TEST.` runner were all done in about 4 days (commits 2025-10-11 to 10-14; PROGRESS.md:3-7 "v0.4 - Type Checking Edition"). The following were **not** reached: recursion, loops, late binding, `CONTEXT.`/guards (`src/forth.rs:943-949` returns "CONTEXT. not yet implemented"), overloading (planned in PLAN-TYPE-OVERLOADING.md), writing the language in itself, and any inet or native compilation. The last commit deliberately paused the DB: "DATABASE FEATURE IS CURRENTLY DEFERRED" (`src/database.rs:1-10`).

### 1b. Interpreter A (archive/march2/main, "First attempt")

- **Shape.** The REPL reads **one line at a time** and calls `eval(line)` (`main:src/main.rs:2119-2153`). Tokens are pre-split, and special forms (`=`, `?`, `%`, `<>`, `TEST.`, `$`, `:`/`::`, `->`, `=>`, `raise`) are hard-coded in `eval_tokens` (`main:src/main.rs:1163-1483`). This hard-coding is what PROGRESS.md:312-314 later criticises. A definition must fit on one line. Multi-line `: [*]` definitions fail with "Expected ';' to end definition", and any error exits the process (`main:src/main.rs:2150-2151`).
- **Value and stack representation.** The stack is a persistent singly linked list of `Rc<StackNode{value, typ}>` (`main:src/main.rs:6-64`). Each value carries its runtime type ("shadow type stack"). `fork()` shares the tail in O(1) (`main:src/main.rs:56-63`), and guard evaluation uses it. Values (`main:src/main.rs:105-115`) are: Number, Quotation(Vec<Word>), String, Symbol, Array(Vec), PersistentArray(im::Vector), Tuple, ErrorValue(type name), and an ArrayMarker used by `{ }`.
- **Binding is late, by name.** User words compile to `Word::NamedWord(name)`. Each call is re-dispatched through `eval_token` (`main:src/main.rs:1043-1046`, `1987-1989`). A placeholder entry allows self-recursion (`main:src/main.rs:1369-1374`). Observed: `10 fact` = 3628800 passes. There is a recursion-depth limit of 1000 and a 1,000,000 total-iteration limit (`main:src/main.rs:236-239`, `553-556`, `925-928`).
- **Strictness.** Strict and eager.
- **Quotations.** `[ ... ]` are parsed eagerly into `Vec<Word>`. `call`/`eval`, `if`, `#do`/`times` with `i0`, and the `#[ ... ]` sugar are supported (`main:src/main.rs:919-978`, `1485-1524`).
- **Immediate words (`::`)** are executed at compile time via `immediate_words` (`main:src/main.rs:1612-1615`). test_immediate.march2 passes 3/3.

---

## 2. Namespaces, persistence, content addressing, global state, effects

### 2a. NAMESPACE_DESIGN.md in depth (written for Interpreter A)

- Namespaces are to be **first-class values**: `Type::Namespace(String)` and `Value::Namespace(String)` (NAMESPACE_DESIGN.md:9-20), integrated with multi-method dispatch.
- Storage options considered: a separate `namespaces: HashMap<String, HashMap<...>>` (NAMESPACE_DESIGN.md:26-33) versus the decision "Fully qualified names in main dictionary for simplicity" (NAMESPACE_DESIGN.md:123).
- Syntax: `<> math ;` defines a namespace, `: math.div ... ;` defines a word by its qualified name, and `math div` is a dynamic call by pushing a namespace (NAMESPACE_DESIGN.md:38-52). Import `< math ;`, alias `< mymath=math ;` and transitive imports are listed as future work ("User mentioned Elixir limits to one level", NAMESPACE_DESIGN.md:113-115).
- Lookup algorithm (NAMESPACE_DESIGN.md:59-85):
  1. If the top of stack is a Namespace value, pop it and look up `ns.token`.
  2. Else, if the token contains a dot, do a direct lookup.
  3. Else, do a normal lookup.
- Key decisions (NAMESPACE_DESIGN.md:117-123): a qualified name is one token; a namespace is a value ("enabling dynamic dispatch"); qualified names are resolved at compile time and namespace dispatch at runtime; "The multi-method dispatch system doesn't need to know about namespaces at all!" (:149).
- Example of storing a namespace in state: `$ mynamespace = math ; 10 5 mynamespace div .` (NAMESPACE_DESIGN.md:143-144).
- Open questions (NAMESPACE_DESIGN.md:153-155): does `<>` push or register; is `NamespaceRef` needed; one dictionary or many.

What Interpreter A actually implemented is **weaker than this design**. `<>` only inserts the name into a `HashSet` (`main:src/main.rs:1258-1272`). `parse_tokens` textually glues `math div` into `math.div` at parse time (`main:src/main.rs:1766-1775`). There is no Namespace value and no dispatch on top of stack, so the "store namespace in state" example has no implementation. test_namespaces.march2 passes 4/4 (qualified and two-token forms).

Interpreter B implemented a different design, documented in docs/manual/namespaces.md:
- There is an indexed `Vec<HashMap<String,Word>>`, a `namespace_stack: Vec<usize>` and parallel `namespace_names` (`src/forth.rs:20-22`).
- `NAMESPACE. n ;` pushes (or re-pushes) a namespace, and it becomes where definitions go (`src/forth.rs:951-985`).
- `IMPORT. n ;` inserts the namespace *below* the current one (`src/forth.rs:987-1012`; bug fix in commit `3460099`).
- `ALIAS. new q.name ;` copies the word (`src/forth.rs:1014-1041`).
- Qualified lookup tries the longest namespace prefix, splitting at dots from the right (`src/forth.rs:199-233`).
- A bare namespace token sets a one-shot `lookup_namespace` for the next word (`src/forth.rs:308-313`). This is the only remnant of the "namespace as value" idea. Observed: `a w .` works.
- There is no word to pop or leave a namespace. The stack only grows.
- The built-in `march` namespace holds `save`, `load`, `words` and `cid` (`src/forth.rs:149-165`).

### 2b. Content addressing (Interpreter B only; implemented)

- `CID` is 32 bytes (`src/cid.rs:17-58`):
  - Bit 7 = 0: SHA-256 of the concatenated child CIDs (a "sequence" is code).
  - `0x80`: primitive, a fixed u16 id with no hash. Ids 1-23 are hard-coded (`src/xt.rs:288-320`).
  - `0xA0`: SHA-256 of MessagePack-serialized `SerializableValue`, with the tag overwriting the top 3 bits (`src/cid.rs:35-44`; `src/serializable.rs:159-229`).
- Every compiled word gets `cid = from_sequence(current_def_cids)` and keeps `cids` (`src/forth.rs:833-858`). Quotations are hashed the same way and then wrapped as a literal `SerializableValue::Quotation(cid)` (`src/forth.rs:915-934`). A call to another word contributes **that word's CID**, so a word's hash is a Merkle hash over its callees. Observed: `quad` = [CID(double), CID(double)].
- Gaps in the code:
  - Variable reads inside quotations or definitions push no CID (`src/forth.rs:359` "TODO: Also add CID for variable value", `362`). Those words hash as if the read were absent.
  - Native words have no CID and cannot be compiled (`src/xt.rs:316`).
  - Signatures and names are not part of the hash.
- DESIGN.md:91-107 designs "Content-Addressed Distribution" of inet IR. Only the CID-of-XT-sequence part exists.

### 2c. Database / persistence (march2.db)

- Implemented in `src/database.rs` with rusqlite:
  - `cids(cid BLOB PK, content_type, data)`.
  - `words(namespace, name, cid, signature TEXT, immediate, PK(namespace,name))` (`src/database.rs:66-99`).
  - `"ns" march.save` and `"ns" march.load` always use `./march2.db` (`src/database.rs:293-335`).
- `save_namespace` stores only the sequence blob of each compiled word and the word row (`src/forth.rs:644-671`). It **does not store the literal or quotation CIDs** the sequence references. `load_namespace` recursively resolves CIDs to `XT::Compiled` (`src/forth.rs:674-766`), so a load fails as soon as any word contains a literal. Quotation literals cannot be revived either: "Quotation CID resolution not yet implemented" (`src/serializable.rs:238-241`).
- Observed with `test_database.sh`: the save succeeds, but the load fails with "Error: CID not found in database: CID:LIT:b19caacd…" (the literal `3` in `triple`). The load is all-or-nothing, so `mylib.double` is also "Unknown word".
- A round trip **did** work for words made only of primitives and other saved words. Observed: `: double dup + ; : quad double double ;` saved, reloaded in a fresh process, and `5 m.quad .` printed 20.
- The archived `march2/march2.db` (24 KB) holds exactly step 1 of test_database.sh: 3 sequence CIDs plus rows `mylib.{double,triple,quadruple}` with signature `core.i64 -> core.i64`. Its CIDs are byte-identical to my re-run, so hashing is deterministic.
- docs/manual/database.md over-claims. Examples: "forth2.eval(\"10 mylib.double .\")?; // prints 20" (:522), and "Load time: <10ms … Tested limits: 100,000 CIDs" (:337-349). No code or benchmark exists for those numbers, and `Forth::eval` does not exist in B. database.md:194-195 itself admits "uses placeholder XTs". Commit `12e79a5` says "Database save/load tested and working". That holds only for literal-free words.
- DESIGN.md:227-333 ("Phase 5: Primitive Selection": a SQLite `primitives`/`implementations` table of per-arch asm snippets chosen by features and cycles) is **design only**.

### 2d. Global store / state and effects ordering

- **Interpreter B.** There is one `global_state: HashMap<String,Value>` (`src/forth.rs:23`). Effects (`.` printing and `->` stores) happen in plain execution order of the strict tree-walk. There is no effect tracking, no state sequencing and no transactions. `TEST.` and `VARIABLE.` snapshot and restore only the **data stack** (`src/testing.rs:370,393`; `src/forth.rs:1051,1091`).
- **Interpreter A.** State is `HashMap<String,(Type,Value)>` with type-checked stores. `->` pops and `=>` copies (`main:src/main.rs:739-782`). Arrays are converted to `im::Vector` on store (`main:src/main.rs:377-388`, `484-493`).
  - Guard isolation is implemented. During a `?` guard the interpreter forks the stack and installs a `state_tempmap` overlay. Writes go to the overlay, reads check it first, and both are discarded afterwards (`main:src/main.rs:366-399`, `1700-1727`).
  - Observed: a guard `? 999 -> x -1 ;` leaves `x` at 10 (`TEST. x 10 eq` passed). Variables are read late at run time (`Word::VarFetch`, `main:src/main.rs:1026-1033`), unlike B. Observed: `: show x ;` after `42 -> x` gives 42.
- **Designed only.**
  - DESIGN.md:38-56: "State variables declared up front", "State queries become special inet nodes that read global state cells".
  - DESIGN.md:436-441: "Decision: State cells as special nodes. Effects are inet nodes with side effects."
  - DESIGN.md:10: "No hidden global state".
  - DESIGN3.md (history) §2.3/§7: explicit `@`/`!` cells, a "mutable cell" per `$` variable, and a rejected lvalue `&T` idea ("I do not see the point of this at all", DESIGN3.md:217).

---

## 3. Types and overloading, syntax, examples

### 3a. Interpreter A (implemented, runtime dispatch)

- Signatures: `= in... -> out... ;` sets `current_signature`, and every `:` **requires** one ("Word definition requires a signature", `main:src/main.rs:1366-1367`). The signature is cleared after each definition (`:1419`).
- The dictionary maps a name to `Vec<(Signature, Word)>` (`main:src/main.rs:224`). Types (`main:src/main.rs:67-79`): i64, quot, str, sym, `{a}` arrays, tuples, single-lowercase-letter type variables with consistent binding (`:328-340`), and Named types with a `%` subtype declaration `% T < Parent ;` (`:1231-1256`). DivideByZero < MathError < Error are built in (`:273-278`).
- Dispatch runs **at every call, at runtime** (`main:src/main.rs:1625-1698`):
  1. Candidates are pre-sorted by "specificity", the count of non-type-variable inputs (`:321-326`, `:1395-1410`).
  2. For each candidate, the shadow types on the stack are checked, then the `?` guard runs.
  3. The first match executes.
  4. If none matches, the interpreter tries the `[*]` wildcard family with `current_word` set, so `[*]` inside it re-dispatches the original operator. This was meant for array broadcasting.
  5. Otherwise it errors with "No matching signature".
- Ties: equal specificity keeps definition order, so the **first** definition wins. Observed: `3 quad` = 81 with the original `sq`. This contradicts docs/manual/types.md:196-208 "last defined wins" (design, for B).
- Errors as dispatch: `raise T` pushes an `ErrorValue(T)`, restores the word's original arguments from the stack saved at entry, and re-dispatches the **same word name**. So a variant `= DivideByZero i64 i64 -> i64 ;` acts as the handler (`main:src/main.rs:979-1011`, `1646-1648`). Observed: `10 0 safe-div 0 eq` PASS.
- There is no static checking at all in A. Types are checked at runtime per value.

### 3b. Interpreter B (implemented, compile time only)

- `SIGNATURE. i64 i64 -> i64 ;` is sticky and applies to all following definitions (`src/forth.rs:1235-1276`). The type stack is initialised from the inputs at `:` (`:783-788`). Only words **that carry a signature** are checked: the four arithmetic primitives (`:59-74`). The final type stack is compared with the outputs at `;` (`:860-883`).
- The stack words have no signatures ("TODO: add signatures for these", `src/forth.rs:76`), and quotation literals and comparisons do not touch the type stack. As a result **ordinary valid code emits spurious type errors**. Observed: `SIGNATURE. i64 -> i64 ; : double dup + ;` prints "Error: Type error in '+': expected 2 inputs but type stack only has 1". The word is still defined anyway, because the XT is pushed before the check (`:333` vs `:343-345`), and the tests still pass.
- Type checking also fires when there is no SIGNATURE (empty type stack). The typechecking.fth "should fail" cases are commented out (tests/typechecking.fth:17-20).
- First-class types: `i64 string quotation array map` push `Value::Type`. `?` is a type predicate and `!` a cast (string<->i64 only) (`src/forth.rs:1188-1232`). `type` returns a string such as "core.i64" (`src/forth.rs:564-569`). Equality is numeric only: `"hi" "hi" eq?` gives "Expected number". types.fth comments out string tests for this reason ("TODO: String comparison needed", tests/types.fth:13).
- Only one word per name is allowed. Overloading is designed only (see 3c).

### 3c. PLAN-TYPE-OVERLOADING.md (design only; the final commit)

- Goal: several words with the same name and different signatures, selected "At call site, type checker selects correct variant" (PLAN:17-27).
- Three options were weighed: `HashMap<String,Vec<Word>>`, name mangling `double__i64_i64`, and per-signature namespaces. Option 1 was recommended (PLAN:31-76).
- Lookup (PLAN:79-85): one variant is returned directly; with several, match against the compile-time type stack; with no type context, error "Ambiguous word, multiple definitions". Runtime dispatch is left open: "Compile to specific variant … Or keep some dynamic dispatch capability?" (PLAN:118-122, 152-154).
- Notes: "Keep it simple first - exact signature matching only" (PLAN:197). This is a **regression in expressiveness** from Interpreter A, which already had runtime multi-dispatch with type variables, subtyping and guards.
- docs/manual/types.md adds more design, all unimplemented (types.md:525-533 "Current Limitations"):
  - Specificity scores: concrete = 2, abstract = 1, summed (types.md:175-192).
  - "last defined wins" tie-breaking (:196-208).
  - Ambiguity error across imports (:210-220).
  - Abstract Integer/Number hierarchy and monomorphisation "Each concrete instantiation gets its own CID" (:248-294).
  - Parametric types and inference.

### 3d. Context-oriented dispatch (guards)

- DESIGN.md:19-36 and 156-193 (design): `? guard ;` before `:`. "Context are compiled just like regular words, but get attached to words as part of their context … A one-to-many relation … with order priority" (DESIGN.md:165-166). Also "Static types are context gaurds too, but they are compile time guards" (DESIGN.md:186). And "At compile time, generates conditional inet with three branches" (DESIGN.md:36). The deleted DESIGN.md Phase 2 had an `expand_word` that picks the first guard that evaluates true (git `f9fe3e4` DESIGN.md, "Expansion Algorithm").
- docs/manual/context.md is 6 lines: "Constext are runtime distach rules." (context.md:5).
- **Implemented only in Interpreter A.** `?` attaches a guard `Vec<Word>` to the current signature (`main:src/main.rs:1203-1229`). The guard runs on a forked stack and a discarded state overlay, and must leave a truthy i64 (`:1700-1727`). An empty guard or no guard means "always", which acts as the fallback. Observed: two-variant `max` passes 2/2, and a guard mutation of `x` is invisible afterwards.
- Interpreter B has `CONTEXT.` as a stub that errors (`src/forth.rs:943-949`). SYNTAX.md:52-104 sketches `CASE.`/`CONTEXT.` blocks grouping several definitions under one guard.

### 3e. Syntax evolution (SYNTAX.md, EXAMPLE.md, DESIGN3.md)

- **Sigil-ops** (EXAMPLE.md:65-73): `<>` program/namespace, `<` imports, `>` exports, `$` state, `=` signature, `?` context, `:` word. The first example uses indentation, not `;` (EXAMPLE.md:27-28).
- **"Symbols all the way down"** (EXAMPLE.md:1-16; DESIGN3.md): bare identifiers push symbols, `sym .` executes, `sym @` reads and `v sym !` writes. It was rejected for noise: "I tried to work around this but it just made things too complicated" (EXAMPLE.md:10).
- **COBOL-like keywords** (SYNTAX.md:48-96): `PROGRAM. IMPORT. EXPORT. RUNTIME. CASE./CONTEXT. SIGNATURE./TYPE. DEFINE. COMTIME.`. The rationale: "since code will end up in a database … the text format is really a serialization format" (SYNTAX.md:50-51). B implements `DEFINE.`, `SIGNATURE.`, `NAMESPACE.`, `IMPORT.`, `ALIAS.`, `VARIABLE.` and `TEST.` as aliases or words (`src/forth.rs:109-147`).
- **"Nascent refs on stack"** (SYNTAX.md:118-139): an undefined word pushes a nascent ref that a following `:` or `=` consumes (`age : 12 ;`). It was "almost adopted" and not implemented.
- **Concrete surface differences.** A uses `[ ]` for quotations, `{ }` for adaptive array/tuple collections, `eq lt gt` returning -1/0, `$ x = … ;`, and `= … ;` signatures. B uses `( )` for quotations (so it has no block comments), `eq? lt?` returning 1/0, and `VARIABLE.`/`SIGNATURE.`.
- **Examples.** `examples/define_syntax.fth` (B; `:` vs `DEFINE.`, notes "SIGNATURE. and CONTEXT. are not yet implemented", :23). Deeper examples are in the `.march2` tests: guards, raise, broadcast and persistent arrays.

---

## 4. What worked, what failed or was abandoned, and test results

### Test results I observed

**Interpreter B (working tree copy, `cargo build --offline`, edition 2024):**
- `cargo test --offline`: 13 unit tests passed (cid, xt, serializable, database) and 1 integration test passed. The integration test is a placeholder: "This test just ensures the binary compiles" (tests/database_integration.rs:10).
- `march2 test`: 6 files, **49/49 PASS**, exit 0. The same run printed **6 spurious "Type error in '+'/'*'"** lines while compiling valid words in typechecking.fth, signatures.fth and database.fth. PROGRESS.md:375 says "All 47 tests passing!", while testing.md:122 and commit `3101938` say 49.
- `run_forth_tests.sh`: 49 PASS, 0 FAIL.
- `test_database.sh`: save OK, **load FAILED** ("CID not found in database: CID:LIT:…").

**Interpreter A (main copy):** there is no aggregate runner, so I ran each `.march2` file.
- Files with `TEST.` assertions (✓ = pass): test_if 11✓, test_immediate 3✓, test 4✓, test_namespaces 4✓, test_quotations 3✓, test_recursion 6✓, test_simple 3✓, test_state 6✓, collection/test_collections 5✓. test_fail 1✗ (intended).
- Errors:
  - test_broadcast and test_broadcast_demo: "Expected ';' to end definition". Multi-line definitions are unsupported, so **array broadcasting via `[*]` was never demonstrated**. My one-line rewrite also failed, on test-code stack-order bugs.
  - test_constraint_state: "No matching signature for '+'". The test bug: `= i64 -> i64` with body `+`.
  - collection/test_collection_access: "No matching signature for 'get-first'". `a` does not match array types.
  - collection/array/test_new_arrays: "Cannot pop with array marker on top".
  - test_loop_limit hit the 1,000,000 iteration limit, which is the intended error.
- The rest (constraints, constraint_stack, constraint_array_mutation, error_handling, persistent_arrays, signatures, state_ops, strings, types, comments, comptime_state) ran without error but contain **no assertions** (they use `drop`).
- My own assertion probe on A: guard-selected `max` 2/2 PASS; guard state rollback PASS; `raise DivideByZero` handler PASS (the other safe-div case failed because of the test's own `swap` bug); type-variable vs concrete specificity PASS; recursion `10 fact` PASS; late-read variable PASS.

### What worked

- **A:** runtime multi-method dispatch over shadow types, with type variables, a named-type subtype hierarchy and guard predicates evaluated on an O(1) forked persistent stack with a discarded state overlay. `raise` as re-dispatch to an error-typed variant. Late binding and recursion.
- **B:** the FORTH outer interpreter with immediate `:`/`;`, the namespace stack with import and alias, Merkle CIDs over word bodies, deterministic hashes, and a SQLite save/load round trip for literal-free words. A test runner with per-file fresh interpreters.

### What failed or was abandoned, and why (quotes)

- **A was abandoned wholesale.** "Hardcoded special forms - `:` and `;` were hardcoded in eval loop", "Brittle nested parsing - Complex token consumption logic that broke with `[ { } ]`", and "Not following FORTH philosophy - Tried to parse everything upfront" (PROGRESS.md:312-314). "We started fresh, so this no longer applies" (OLD_PROGRESS.md:3). The rewrite dropped A's guards, multi-dispatch, loops, recursion, `raise` and collections. None had been restored when the lineage stopped.
- **Database deferred.** "We are NOT actively working on database persistence right now. Focus is on getting the core FORTH interpreter solid first" (`src/database.rs:3-4`). Commit `96e3f24`: "Core interpreter should be solid before adding persistence".
- **Inets dropped from the plan.** The wip commit deleted DESIGN.md's inet construction and optimisation phases, and the doc says "STOP HERE. WE ARE ON PHASE 1." (DESIGN.md:195). DESIGN.md's "Open Questions" still carry decisions on inets and tail-recursion-only (DESIGN.md:427-461).
- **Symbol model rejected** (EXAMPLE.md:10, quoted above). The **lvalue proposal rejected**: "I do not see the point of this at all" (DESIGN3.md:217).
- **Known defects in B:**
  - Spurious type errors (`src/forth.rs:76`).
  - Variables frozen at compile time inside definitions (`:361-363`).
  - `->` not compilable.
  - No recursion or late binding (`:333`, `:886`).
  - DB load broken whenever a literal is present (`:644-671`).
  - Numeric-only `eq?` (`:487-491`).
  - `-> x` works only at interpret level (including inside `TEST.`/`VARIABLE.`, which evaluate in interpret mode), never inside `:` definitions.
- **Unfinished commitments in DESIGN.md:** "Tail recursion only" (:14, :198-220), `pure` ("Not sure we need this, compiler should be able to tell automatically", :89), dependent types via guards (:58-71), assembly emission (:337-424), and success metrics such as "Within 2x of hand-written C" (:470), all marked ✅ as goals but not implemented.

---

## 5. Ideas bearing on march6's open questions

Each bullet says whether the idea was implemented or designed-only in march2.

- **Lazy vs strict.** Both interpreters are strictly eager. There is no laziness anywhere, implemented or designed. DESIGN.md's inet pipeline (the deleted phases, and "Automatic detection of independent subgraphs", :454) is the only dataflow-evaluation idea, and it is design only. Nothing in march2 bears evidence on lazy semantics.
- **How dynamic quotation calls knew stack effects.** *They did not.*
  - In B, `call`/`if`/`iff` have no signatures (`src/forth.rs:98-102`), quotation literals add nothing to the compile-time type stack (`:915-934`), and a word with no signature is not type-tracked. Anything flowing through a quotation is simply unchecked, and the checker produces false errors instead.
  - In A there is no static effect at all. Types are checked at runtime per dispatch against the shadow types on the stack (`main:src/main.rs:1729-1757`), and `.types` shows them.
  - DESIGN3.md §6 wanted "A type stack mirrors it, enabling static checking and multimethod dispatch", but it has no quotation-effect story.
- **How effects and state were ordered.**
  - Implemented: plain sequential execution order of a strict interpreter, with no effect tokens.
  - One mechanism of interest (A, implemented and tested): **speculative guard evaluation with isolated effects**. The stack is forked via Rc tail sharing, and a copy-on-read `state_tempmap` overlay is discarded after the guard (`main:src/main.rs:366-399`, `1700-1727`). Guards may compute with state writes, and those writes never escape.
  - Designed only: "State cells as special nodes. Effects are inet nodes with side effects." and "Effect tokens passed through wires? Monadic wiring?" (DESIGN.md:436-441).
- **Context-oriented dispatch (guarded definition families).** A has the only working implementation across these refs (details in §3a/§3d):
  - The family is `name -> Vec<(Signature{inputs, outputs, constraint}, body)>`.
  - Selection order: type match, then guard, ordered by specificity then definition order.
  - `[*]` is a family-level fallback that re-dispatches the original name.
  - `raise T` is a re-entry into the same family with an error-typed first argument.
  - Designed extensions: guards compiled statically into a conditional net when resolvable ("Guards check type constraints at compile time when possible, runtime otherwise", DESIGN.md:71), types treated as compile-time guards (:186), grouped `CONTEXT.`/`CASE.` blocks (SYNTAX.md:52-104), and specificity scoring with ambiguity errors (types.md:171-220).
  - B's overloading plan narrows this to compile-time exact-signature selection (PLAN:79-85, 197).
- **Global store and namespaces.**
  - Implemented in B: names map to (namespace, name) and then to a content hash; bodies are Merkle hashes of callee hashes; storage is `words(namespace,name)->cid` plus `cids(cid)->blob` (`src/database.rs:66-99`). Words are inlined by value, so a stored word's hash pins its dependencies, but in-memory redefinition does not propagate either.
  - Missing from the hash: signatures, names, and variable reads (`src/forth.rs:359`).
  - Namespaces are an indexed stack with import-below-current and alias-by-copy. The "namespace as a first-class stack value" design (NAMESPACE_DESIGN.md) was never realised beyond a one-shot lookup prefix.
  - The global variable store is a single mutable HashMap in both interpreters. A converts arrays to `im::Vector` on store, so state holds persistent structures.
- **Self-bootstrapping FORTH.** B made the definers (`:` `;` `::` `(` `)` `--` and the `*.` keywords) ordinary immediate native words. That part is implemented. But no part of the language is defined in March itself: the dictionary is seeded entirely in Rust (`src/forth.rs:57-172`). Immediate words cannot emit code (no LITERAL/POSTPONE/COMPILE,). It stopped before any metacompilation.
- **Performance numbers.** None were measured in the archive. The database.md timings (:335-349) are unsupported by code or tests. My rough timings on this machine, release builds:
  - A: `[ i0 drop ] 999999 #do` took about 0.037 s in total, roughly 37 ns per iteration.
  - B: 200,000 calls of `: f 1 2 + drop ;` from stdin took about 0.023 s including tokenising.
  - These are indicative only. No benchmark exists in the lineage.

---

## 6. Every file read (one line each)

- `march2/DESIGN.md`: v0.4 vision (FORTH to inets to native, context dispatch, explicit state, CAS), phase 1 FORTH plan, primitive DB and asm design, open questions.
- `git a526085^:DESIGN.md` (via diff): deleted Phase 2-4 text on the compilation pipeline, context expansion algorithm, inet node types and inet optimisation passes.
- `git archive/march2/main:DESIGN.md` (via diff): v0.1 with `context` keyword syntax, implementation-phase timeline, key features.
- `git f7050c6:DESIGN3.md`: deleted "symbols all the way down" design with explicit `.`/`@`/`!`, type stack and rejected lvalues.
- `march2/EXAMPLE.md`: symbol-vs-traditional FORTH question, sigil-op program example.
- `march2/NAMESPACE_DESIGN.md`: first-class namespace values, qualified tokens, TOS-namespace dynamic lookup design.
- `march2/OLD_PROGRESS.md`: Interpreter A feature list and session notes (comments, strings, state, Option B-lite strategy).
- `git archive/march2/main:PROGRESS.md` (via diff): same content as OLD_PROGRESS.md under the title "Progress Log".
- `march2/PLAN-TYPE-OVERLOADING.md`: plan for multiple words per name selected by compile-time type stack.
- `march2/PROGRESS.md`: Interpreter B status v0.4, features, design decisions, lessons from the first attempt, next steps (CIDs).
- `march2/README.md`: Interpreter A user overview (`{ }` collections, `$` state, `#do`, multi-methods).
- `march2/SYNTAX.md`: syntax explorations (sigils, thunks, COBOL-like keywords, nascent refs). The main-branch variant was read via diff.
- `march2/docs/manual/context.md`: 6-line stub, "Constext are runtime distach rules."
- `march2/docs/manual/cids.md`: CID bit layout, primitive ids, MessagePack literals, planned schema, ColorForth inspiration.
- `march2/docs/manual/database.md`: SQLite schema and API, save/load usage (partly over-claimed), future work.
- `march2/docs/manual/namespaces.md`: B's namespace stack, NAMESPACE./IMPORT./ALIAS., lookup rules.
- `march2/docs/manual/types.md`: B's first-class types, SIGNATURE., compile-time checking, planned dispatch, abstract types and monomorphisation.
- `march2/docs/manual/testing.md`: TEST. word, `march2 test` runner, counters, patterns, planned features.
- `march2/Cargo.toml`: B dependencies (im, rustyline, sha2, serde, rmp-serde, hex, rusqlite bundled).
- `march2/src/main.rs`: B entry point, `test`/`repl` subcommands.
- `march2/src/value.rs`: B Type and Value enums.
- `march2/src/word.rs`: B Signature and Word (xt, cid, cids, immediate, signature).
- `march2/src/xt.rs`: B XT enum, primitive to CID id table, unit tests.
- `march2/src/forth.rs`: B core interpreter, dictionary seed, compile/execute, type check, DB save/load/resolve, all native words.
- `march2/src/input.rs`: B refillable token buffer, string-literal reader.
- `march2/src/repl.rs`: B rustyline REPL, piped mode, `.fth` test runner.
- `march2/src/testing.rs`: B `TEST.` and `test.print-results.`.
- `march2/src/cid.rs`: B 32-byte CID with tags, hashing, tests.
- `march2/src/serializable.rs`: B MessagePack SerializableValue and CID derivation.
- `march2/src/database.rs`: B SQLite layer, `march.save`/`march.load`, deferral banner, tests.
- `march2/examples/define_syntax.fth`: B `:` vs `DEFINE.` examples.
- `march2/tests/basic.fth`, `variables.fth`, `types.fth`, `signatures.fth`, `typechecking.fth`, `database.fth`: B test files (25+5+8+5+4+2 tests).
- `march2/tests/database_integration.rs`: placeholder integration test.
- `march2/tests/*.march2` (24 files incl. `collection/`) and `collection/array/test_new_arrays.march`, `collection/tuple/test_tuples.march`: Interpreter A tests (guards, raise, broadcast, state, collections). The `.march` files are unimplemented future syntax.
- `march2/run_forth_tests.sh`: pipes each `.fth` through the binary and counts ✓/✗.
- `march2/test_database.sh`: save `mylib` then load in a fresh process.
- `march2/Makefile`: A-era build/test targets over `tests/*.march2`.
- `march2/march2.db`: inspected with sqlite3 read-only; 3 sequence CIDs plus 3 `mylib` word rows.
- `march2/.claude/settings.local.json`: permission allowlist only.
- `git archive/march2/main:src/main.rs` (2,160 lines, fully read): Interpreter A.
- `git archive/march2/main:Cargo.toml`: A dependencies (im, atty).
- Git logs and commit messages of all three refs, `git diff --stat main..bootstrap-forth`, `git show a526085`.
