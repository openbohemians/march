# march1 digest

Source: `/home/trans/my/com/tabcomputing/march/march1` (working tree HEAD = `4cab60e` = `archive/march1/preserved/wip-2026-09-23`, clean). Rust, edition 2024, crate `march` 0.2.0, deps `num-bigint`, `num-rational`, `rusqlite 0.31`, `sha2` (`Cargo.toml:1-10`). About 2.9k lines of Rust: `src/lib.rs` has 1812 lines, the rest is small.

History (canonical repo refs):
- `837de31` (tag `archive/march1/v0.1.0`, 2025-10-05 01:39) is the initial import of everything: docs, lib, db, hash, state, the 3 original demos, and 4 integration-test files.
- `2e52e28`: user words can be used inside definitions.
- `cbb29e8`, `8b8dc6c`, `52484b6`: constraint system.
- `e2229e3`: `State:` renamed to `$`.
- `756c393`: `Context` renamed to `?`.
- `caf4a44`, `17b7292`: error registry and `raise`.
- `491a913` (tag `v0.2.0` = `archive/march1/main`, 2025-10-05 15:28): "Error-as-Context-Transition".
- `4cab60e` (2026-09-23): "Archive uncommitted March 1 experiments". This commit adds `+527` lines to lib.rs (thunks, arrays, `?`-from-thunk, type-signature dispatch, operator aliases) and 8 `test_*.rs` binaries.
- No markdown file changed after `837de31`. DESIGN/README/CLAUDE are the original import.

Whole project span: roughly one day (2025-10-05), plus one later uncommitted session.

**Important framing:** the three docs disagree with each other and with the code.
- `CLAUDE.md:24` says "Implementation Language: C".
- `CLAUDE.md:39-40` says "Project is in design/planning phase / No source code implemented yet".
- `README.md:166-173` claims "REVOLUTIONARY BREAKTHROUGH ACHIEVED! ✅ Type-based contextual dispatch ✅ ITC execution with hash→XT lookup".

The code does not support the README's claims (details below).

---

## 1. Execution model as implemented

**Interpreter shape.** It is a string-re-parsing, tree-less text interpreter over a Rust `Vec` stack. There is no real bytecode or threaded-code execution.
- Stacks: `Interpreter` holds `value_stack: Vec<Value>` and a parallel `type_stack: Vec<ConcreteType>` (`src/lib.rs:210-222`). Every `push` writes both (`lib.rs:298-301`).
- Entry point: `execute_word(&str)` (`lib.rs:487-600`). It dispatches on the *whole input string's* leading char:
  - `:` goes to `parse_definition`.
  - `$` goes to state declaration.
  - `?` goes to context.
  - `Error:` goes to error declaration.
  - `{...}` makes a thunk.
  - `[...]` makes an array.
  - a suffix of `@` or `!` means state get/set.
  - Otherwise a hard-coded match handles `dup swap drop .s words .state & = > < true false Int String Rational if then raise force`. The fall-through goes to dispatch.
- Definitions (`parse_definition`, `lib.rs:602-643`) do two things with the body:
  - They "compile" it to a `Vec<WordHash>` (`compile_word_body`, `lib.rs:646-661`). Each token is resolved via `resolve_token_to_hash` (`lib.rs:664-700`): user alias first, then i64 literal hash, then the fixed primitive map. Any other token is `ParseError`.
  - They store *both* the source text and that hash sequence in SQLite.
- Execution ignores the hash sequence. `execute_runtime` (`lib.rs:1000-1027`) handles primitive hashes directly. For user words it calls `db.get_word(hash)` and then `execute_body(&definition.body_source)`, which re-splits the source text on whitespace and calls `execute_word` per token (`lib.rs:702-756`).
- `hash::Runtime` has `hash_to_xt` and a `primitive_table` with a recursive `execute_word_hash` (`src/hash.rs:146-287`). `parse_definition` populates `hash_to_xt` (`lib.rs:626, 654`), but nothing in `lib.rs` ever calls `Runtime::execute_word_hash`.
- `Runtime::resolve_token` maps every number to `WordHash::primitive(0)` "Placeholder" (`hash.rs:229-233`).
- So the README's "ITC Execution: hash → ExecutionToken → native function" (`README.md:126`) is **designed-only / dead code**.

**Value representation** (`lib.rs:49-61`): `Value` is one of
- `I64`
- `BigInt`
- `BigRational`
- `String`
- `Array(Vec<Value>)`
- `Ptr(usize)`
- `TypeOf(AbstractType)`: type-as-value
- `ConstrainedType(AbstractType, Vec<Constraint>)`
- `Thunk(ThunkDefinition)`

`ConcreteType` (`lib.rs:33-46`) lists `I8..I128, F32, F64, BigInt, BigRational, String, Array{elem,size}, Ptr, TypeOf, ConstrainedType, Thunk`. Only I64/BigInt/BigRational/String/Array/TypeOf/Thunk are ever produced. Booleans are FORTH-style: `true` is -1 and `false` is 0 (`lib.rs:539-546`).

**Evaluation order.** Evaluation is strict, left-to-right, token by token. The only laziness is explicit thunks.

**Thunks, exactly** (all added in the WIP commit `4cab60e`):
- `ThunkDefinition { body_source: String, thunk_type: General | ContextSignature }` (`lib.rs:11-21`).
- `{ body }` is only recognised when the *entire* `execute_word` input starts with `{` and ends with `}` (`lib.rs:509-511`). It pushes `Value::Thunk` holding the raw source text (`parse_thunk_creation`, `lib.rs:1397-1419`).
- There is no captured environment, no memoization and no sharing. `force` (`force_top_thunk`, `lib.rs:371-390`) calls `force_thunk` (`lib.rs:335-360`), which re-runs `execute_body(body_source)` in the *current* global environment. That is dynamic scope; `test_thunks.rs:62` itself advertises "Dynamic scope - thunks execute in current environment".
- A `General` thunk pops the top result if the stack grew, else returns `I64(0)`.
- A `ContextSignature` thunk returns `I64(1)` with the comment "TODO: Implement proper type signature extraction" (`lib.rs:353-358`).
- The body runs on the shared stack, so a thunk can consume caller values. There is no arity or stack-effect check.
- `force_if_thunk` (`lib.rs:363-368`) is never called: values are never implicitly forced.
- A thunk is a quoted program forced by name, not a lazy value.

**Words, definitions, dispatch.**
- *Name resolution* uses SQLite `word_names(name, namespace, content_hash)`. A definition always aliases with `namespace = NULL` (`lib.rs:635`). The active context is written only into `words.context_condition` (`lib.rs:629-632`).
- *Context* is a single global `current_context: Option<String>` (`lib.rs:216`). It is set by `? name` (any trailing text becomes the context string, `lib.rs:1219-1227`), by `? default` (resets to None), or by bare `?` with a thunk on the stack (`set_context_from_thunk`, `lib.rs:1309-1394`).
  - The thunk form forces the thunk, expects `[types...] bool`, and turns the array into a string like `"I64 I64 I64"`.
  - If the bool is true, that string becomes `current_context`. If false it prints "Context condition failed" and leaves the context unchanged.
  - The context is a *definition-time* mode: it tags subsequent `:` definitions, like a FORTH vocabulary/compile state.
- *Type dispatch ("context signatures")*, WIP only, `find_matching_context` (`lib.rs:1258-1305`):
  - It takes the top ≤3 stack types as a string (`get_current_stack_signature`, `lib.rs:1235-1255`). Then it tries a hard-coded candidate list: the current context, `"<types> <last>"`, the exact types, `"I64 I64 I64"`, `"String String String"` (`lib.rs:1281-1286`).
  - For each candidate it queries `find_word_hash(name, Some(candidate))`. The lookup uses the *namespace* column, but definitions never write a non-NULL namespace, so a context match can never be found.
  - When the default (NULL) alias exists the function returns `Ok(None)`. `execute_word` then treats that like "no match" and falls through to number / error-name / state-var lookup, and finally "Unknown word" (`lib.rs:560-597`).
  - Net effect in the WIP tree: **no user-defined word and no `+ - * /` can be called by name from `execute_word`**. I confirmed this in test runs (§4).
  - This is also the reason `find_matching_context` runs, and prints diagnostics, for every number token before number parsing.
- *Predicate-guarded dispatch (DESIGN's contexts):* `resolve_word_with_context` and `evaluate_context_condition` (`lib.rs:1533-1572`) evaluate a context word and treat a nonzero result as true. They are `#[allow(dead_code)] // Future`, and they are uncalled in v0.1.0 as well. `db.get_all_definitions` returns at most one row, "TODO: Implement proper contextual lookup with multiple definitions per name" (`src/database.rs:247-260`). **Guarded definition families were never executable.**
- *Operator override:* `initialize_standard_operators` (`lib.rs:262-281`, WIP) aliases `+ - * /` in the DB to primitive hashes, and the hard-coded `"+" => self.add()` arms (present in v0.2.0) were removed. The intent was that a user `: + ... ;` would re-alias `+`. In practice `test_operator_override.rs` fails at the very first `5 3 +` ("Unknown word: +"). `: + swap . . ;` would also fail to compile, because `.` is not a resolvable token.
- *Multimodal execution:*
  - `ExecutionMode {Runtime, TypeChecker, Optimizer, Documentation, Profiler, Debugger}` (`lib.rs:155-162`). `execute_word_hash` switches on the mode and returns an `ExecutionResult` variant (`lib.rs:965-992`).
  - Only Runtime does work.
  - TypeChecker handles `dup drop swap + - * / = > <` on a `type_analysis_stack` and pushes I64 for numeric literals. It skips user words ("TODO: Handle user-defined words", `lib.rs:1141-1171`).
  - Documentation returns `format!("Documentation for word: {}", body_source)` (`lib.rs:1179-1185`).
  - Optimizer returns the constant `vec![0x90, 0x90]` (`lib.rs:1174-1177`).
  - Profiler returns constant `cycles: 42, memory_usage: 1024` (`lib.rs:1187-1193`).
  - Debugger returns constant `step_count: 1` (`lib.rs:1195-1201`).
  - "Multimodal" therefore means "one hash, a mode switch selecting an interpreter". The idea is real but it was implemented only as stubs.

## 2. Content addressing, persistence, namespaces, state, effects

**Hashes** (`src/hash.rs:4-106`) are 32 bytes.
- The high bit of byte 0 marks reserved hashes:
  - `0x80,id` is a primitive.
  - `0x81` is state get.
  - `0x82` is state set.
  - `0xA0` + 8 BE bytes is an inline i64 literal.
- A content hash is SHA-256 over the concatenated 32-byte hashes of the body tokens, with the high bit cleared (`hash.rs:48-61`). Callees are referenced by their current alias's hash, so a definition's hash depends on its dependencies' hashes (Merkle-style).
- The **context is not part of the hash**. The same body in two contexts gets the same hash, and `INSERT OR REPLACE` (`database.rs:83-94`) overwrites `context_condition`.
- `Database::define_word_with_context` (`database.rs:113-134`) uses a *different* scheme: SHA-256 of the source text plus `"CTX:"+ctx`. It is used only by DB unit tests.

**Storage** (`database.rs:32-67`) has two tables:
- `words(content_hash PK, body_source, body_bytecode BLOB, context_condition, context_hash, timestamps)`
- `word_names(name, namespace, content_hash, PRIMARY KEY(name, namespace))`

There is a bug that decides a lot:
- SQLite permits multiple rows with NULL in a non-INTEGER primary key, so `INSERT OR REPLACE ... (name, NULL, hash)` *appends*.
- `find_word_hash` returns the first row, which is the *oldest* definition (`database.rs:137-154`).
- Consequences:
  - Redefinition silently has no effect. The DB tests `test_word_definition` (`database.rs:290`) and `test_contextual_redefinition` (`database.rs:361`) fail, and so do `test_word_redefinition` and `test_word_redefinition_persistence`.
  - `word_names` accumulates duplicates. The shipped `demo.db` has 13 alias rows for 4 distinct words (`sqlite3 demo.db`: square×4, cube×3, double×3, quad×2, matching 4 `words` rows), and the v0.2.0 persistence demo lists "Total words in database: 16".

**demo.db** is a committed SQLite file. It holds 4 words (`dup *`, `dup dup * *`, `dup +`, `dup dup dup * * *`) with hash-sequence bytecode blobs of 64/128/64/192 bytes, all with NULL context.

**Persistence.** Only word definitions persist. State values do not: `tests/state_integration_tests.rs:88` says "note: state values don't persist yet, only definitions". The error registry and current context also do not persist.

**Namespaces.** The `namespace` column exists (`database.rs:49-57`), and `test_namespaces` shows the same content under two namespaces dedups to 1 `words` row. The interpreter never sets a namespace. The WIP code *repurposes* the namespace column as the dispatch-context key, but never writes it.

**State** (`src/state.rs`):
- `ProgramState` is a `Vec<StateVariable{name, abstract_type, concrete_type, constraints, value}>` plus a name→index map (`state.rs:4-41`). It is one global flat store, with no scoping.
- `set_variable` type-checks (exact `ConcreteType` equality) and runs the constraints `> >= < <= OneOf`. For non-I64 values the comparison helpers return `true` ("assume constraint is satisfied", `state.rs:180-218`).
- `$ name Type c1 op1 c2 op2` parses into constraints (`lib.rs:788-834, 879-928`). Variable references in constraints print "not yet implemented" and are treated as 0 (`lib.rs:893-895`).
- The initial value is always 0/""/0-rational, so it is validated against the constraints. That is why `$ counter Int 0 >` fails with TypeMismatch in `demo_content_addressing` at v0.2.0.
- `StateMapping` (`state.rs:22-26, 104-131`) implements DESIGN's import `statemap` rename (`DESIGN.md:188-199`) as a name alias, unit-tested (`test_state_mapping`). The interpreter never uses it.
- Access forms:
  - `name@` / `name!` as single tokens (`lib.rs:519-524`).
  - `name @` / `name !` pairs inside `execute_body` lookahead (`lib.rs:735-749`).
  - A bare `name` pushes the value (`lib.rs:588-592`).
- A *definition* containing a state name cannot compile: `resolve_token_to_hash` rejects `counter` with ParseError, which is why all `state_integration_tests` using `: increment counter @ ... ;` fail. The `0x81/0x82` state hashes carry no variable operand. `ExecutionToken::StateRef` is "TODO" (`hash.rs:281-284`).

**Effects and IO ordering.** There is no effect system:
- IO consists of `println!` diagnostics inside Rust.
- The DESIGN's "every function implicitly receives and returns the entire program state" (`DESIGN.md:64-78`) is realised simply as mutation of one global `ProgramState` in strict token order.
- The DESIGN's per-definition `state_effects` metadata (reads/writes, `DESIGN.md:165-186, 261-264`) was never implemented. There is no table or analysis for it.

## 3. Type system, dispatch, errors, arrays

- **Types.**
  - Runtime tags live on the parallel type stack.
  - Arithmetic requires both operands to have the same variant (I64/I64, BigInt/BigInt, BigRational/BigRational), otherwise `TypeMismatch` (`lib.rs:392-475`).
  - Division by zero is `TypeMismatch`, not a March error.
  - The source has no literal syntax for BigInt or BigRational; only Rust API tests push them.
- **Types as values / constraints-as-words** (`8b8dc6c`): `Int`, `String` and `Rational` push `TypeOf`. `>` and `<` are overloaded (`greater_than_or_constraint`, `lib.rs:1614-1642`): on a `TypeOf` or `ConstrainedType` they build a constraint. On values `>` compares, but the value branch of `<` returns `ParseError` with "we don't have less_than implemented" (`lib.rs:1666-1672`). `$` does *not* use these words; it parses tokens directly.
- **Context signatures**: see §1. As designed, `Context ( I64 I64 -- I64 )` / `? [ I64 I64 -- I64 ] cond ;` (`README.md:130`, `parse_context_signature_declaration` `lib.rs:1481-1531`, which is dead code and never called). As implemented, `{ [ I64 I64 I64 ] true } ?` sets a context string. The `--` separator is parsed (`lib.rs:1350-1353`) but unused, and the tests avoid it ("no -- separator for now", `test_context_signatures_new.rs:9`). Dispatch never succeeds.
- **Error handling** (v0.2.0 headline):
  - `ErrorRegistry` has a parent chain and `is_subtype_of` (`lib.rs:79-135`), with built-ins `Error > MathError > {DivisionByZero, Overflow}` and `NetworkError > {NetworkTimeout, ConnectionLost}`.
  - `Error: Name [extends Parent] ;` (`lib.rs:836-874`). The demo writes `Error: DivisionByZero MathError` without `extends`, so no parent is registered.
  - A registered error name pushes itself as a String (`lib.rs:582-585`).
  - `raise` (`lib.rs:1697-1744`) sets `current_context = error_name`. If `current_executing_word` is set and `lookup_word(word, Some(error))` finds a definition, it re-runs the word by name and then resets the context to None.
  - `is_subtype_of` is never consulted, so there is no hierarchical dispatch.
  - The error-context lookup uses the namespace column, which is never written, so re-dispatch cannot fire.
  - `: risky_divide DivisionByZero raise ;` cannot even compile, because neither token resolves in `resolve_token_to_hash`. `demo_error_handling` fails there with `ParseError` at both v0.2.0 and WIP.
  - Rust-level errors (`RuntimeError::{StackUnderflow, TypeMismatch, ParseError, ...}`) abort the call via `?`.
- **Control flow**: `if` pops and ignores ("for demo purposes, just continue", `lib.rs:1746-1761`), and `then` is a no-op. There are no loops, no conditionals, and no quotation call other than `force`.
- **Arrays** (WIP):
  - `[ ... ]` as a whole input, or a bracket-matched span inside `execute_body` (`lib.rs:710-733`). It is parsed by `parse_array_creation` (`lib.rs:1422-1478`) as a *literal*: integers become I64, `I64`/`String`/`Rational` become `TypeOf`, and anything else becomes a String. Elements are not evaluated.
  - The element type is always recorded as I64 ("TODO: Better type inference").
  - There are no array operations.
  - Arrays exist solely to carry type lists for context signatures.

## 4. What worked, what failed or was abandoned; observed test results

Build: `cargo build --offline --all-targets` in a scratch copy succeeds for WIP (4 warnings, incl. `parse_context_signature_declaration` never used). Results I observed:

| Target | v0.1.0 (tag) | v0.2.0 (main) | WIP (working tree) |
|---|---|---|---|
| compiles as tagged | **no**: E0382 moved `mode` (`set_execution_mode`) + `demo_multimodal.rs:223` syntax error | yes | yes |
| lib unit (12) | 10/12* | 10/12 | 10/12 |
| interpreter_tests (9) | 8/9* | 8/9 | 5/9 |
| persistence_tests (2) | 0/2* | 1/2 | 0/2 |
| state_integration_tests (4) | 1/4* | 0/4 | 0/4 |
| contextual_dispatch_tests (4) | 0/4* | 0/4 | 0/4 |
| **total** | 19/31* | 19/31 | **15/31** |

(*after two one-line scratch fixes to make v0.1.0 compile.)

Failure causes:
- The DB unit tests fail on the NULL-PK redefinition bug.
- `contextual_dispatch_tests` never passed at any version: defining `: running? mode @ 1 = ;` fails because state names cannot be compiled. From v0.2.0 on, the tests also use the obsolete `State:`/`Context` syntax.
- State tests at v0.2.0 and WIP fail with `Unknown word: State: ...` after the `$` rename.
- WIP interpreter and persistence tests fail with "Unknown word: square/+" (the dispatch regression).

Demo binaries:
- At v0.2.0:
  - `demo_persistence` works (5 square=25, 3 cube=27, 2 quad=16, with duplicated listings).
  - `demo_multimodal` runs to completion. Runtime mode computes 25/14. Its type-checker banner prints the hard-coded claim "( I64 -- I64 I64 )" while the computed signature was `[I64] -> [I64]`. The other modes print stub constants.
  - `demo_content_addressing` runs until `$ counter Int 0 >` → TypeMismatch.
  - `demo_error_handling` fails at the first `raise` definition.
- At WIP, all 4 demos fail ("Unknown word: square" / "Unknown word: *"). Of the 8 `test_*` binaries, only `test_arrays` and `test_context_simple` exit 0. They only construct arrays/thunks and set a context string. `test_thunks` fails forcing `{ 5 3 + }` ("Unknown word: +"). `test_type_dispatch`, `test_extensible_operators`, `test_operator_override` and both `test_context_signatures*` fail with ParseError.

What genuinely worked at some point:
- Postfix I64/BigRational arithmetic via Rust API.
- `:` definitions composed of primitives and earlier user words, persisted in SQLite and reusable across sessions (v0.2.0).
- Merkle-style content hashes with dedup (`square` and `square2` share a hash).
- Typed global state with simple numeric constraints (unit level).
- Error-type registry.
- Thunk creation.
- Mode-switched execution with a Runtime mode and a toy type checker.

Designed only / abandoned:
- Predicate-guarded contextual dispatch (DESIGN §2): `resolve_word_with_context` is "Future", and `get_all_definitions` returns one row.
- Static vs dynamic context classification and the compile-time ambiguity error (`DESIGN.md:101-125`).
- Context layers (`DESIGN.md:127`).
- The error-as-context layers (`DESIGN.md:153-157`).
- ITC hash execution (`hash.rs`, unused).
- Import with statemap (only the rename table exists).
- Per-definition effects metadata.
- Incremental compilation to IR (`DESIGN.md:37-42`): the stored hash-sequence "bytecode" is never executed.
- The web IDE and real-time state view.
- Native compilation.

There is no written post-mortem in march1. The repo simply stops after the uncommitted WIP; the evidence of failure is the test table above. Tone note: the commit messages and README are hype-heavy ("REVOLUTIONARY", "This changes programming forever", `README.md:155`) and overstate what runs.

## 5. Ideas bearing on march6's open questions

- **Lazy vs strict.** march1 was strictly eager. Laziness was only an explicit `{ }` quotation holding *source text* and re-parsed on each `force` in the dynamic global environment: no capture, no memo, no sharing (`lib.rs:335-360, 1397-1419`). There was no implicit forcing (`force_if_thunk` is unused). A "thunk" here is really a FORTH quotation. It offers nothing on sharing or memoised demand. The one idea worth keeping: a *typed* thunk kind (`ThunkType::ContextSignature`, "must return [types...] boolean", `lib.rs:11-15`), meaning a quotation whose result shape is fixed by its role.
- **How dynamic quotation calls knew stack effects.** They did not. `force` runs the body on the shared stack, and the result is "top of stack if it grew, else I64(0)". There is no declared or inferred effect, and the TypeChecker mode skips user words. DESIGN had a `signatures(stack_in, stack_out)` table (`DESIGN.md:271-274`) and README had `( I64 I64 -- I64 )` context patterns, but neither was wired.
- **Effects and state ordering.**
  - DESIGN's model is "every word is `( State x -- State y )` with implicit threading" (`DESIGN.md:64-78`). That is conceptually the same as march6's implicit state sequencing.
  - The implementation was just global mutation in token order.
  - DESIGN also proposed per-definition read/write sets as shareable metadata (`DESIGN.md:165-186`), plus import-time `statemap` renaming of required state onto local state (`DESIGN.md:188-199`, `state.rs` `StateMapping`). This amounts to state *requirements as part of a function's interface*, resolved at link time, which is relevant if march6 wants content-addressed definitions that touch the store.
- **Context-oriented dispatch (guarded definition families).** DESIGN's version (`DESIGN.md:80-127`):
  - A definition is `Context <predicate-word>` + `: name ... ;`.
  - Predicates may be state-dependent (dynamic) or type-dependent (static).
  - Combined predicates are allowed.
  - "If multiple contexts match, it's a compile-time error."
  - "More specific contexts override general ones."
  - Errors are just state, so error handling is the same mechanism (`DESIGN.md:129-159`).

  Implementation lessons:
  - (a) A single global "current context" used both as a definition-time tag and as a runtime mode conflated two different things. In the error design, `raise` mutates that global and re-dispatches the *current word by name* (`lib.rs:1716-1741`), which is a dynamic-scope / condition-system flavour.
  - (b) Keying context by a string in a name-alias table, and leaving it out of the content hash, broke both identity and lookup. A guarded family needs its guard to be part of each member's identity and the family to be a first-class collection of (guard, body) pairs, not name overwrites.
  - (c) Type-signature dispatch was attempted by stringifying the top 3 stack types and trying hard-coded candidates. There was no arity notion, so it could not work generally.
- **Global store / namespaces.** Code lived in one SQLite DB: content table + `(name, namespace) → hash` alias table (`database.rs:32-67`). That is the same split as march6's CAS + named namespace. The pitfalls observed were NULL-in-composite-PK duplicates and the namespace column doubling as a dispatch key. The DESIGN stance was "Program = Module … state boundaries don't fragment across sub-modules" (`DESIGN.md:14-15`), with all state global and declared up front (`DESIGN.md:48-62`).
- **Merkle naming choice.** A body hash covers the callees' *current* hashes (`compile_word_body`), so redefining a callee does not change existing callers' hashes. Callers keep pointing at the old callee via stored hash sequence, but execution re-resolved names from source text. That is a latent identity/behaviour mismatch.
- **Performance numbers:** none. The only "profile" output is the hard-coded `cycles: 42, memory_usage: 1024` (`lib.rs:1187-1193`).

## 6. Files read

- `march1/DESIGN.md`: language design (global state, contexts, error-as-context, CAS, DB schema, phases).
- `march1/README.md`: hype README (multi-modal, WordHash, ITC claims, schema, status checklist).
- `march1/CLAUDE.md`: assistant context. Says C implementation, "No source code implemented yet".
- `march1/Cargo.toml`: crate `march` 0.2.0, deps, 12 bin targets.
- `march1/Cargo.lock`: listed only (dependency lock).
- `march1/.gitignore`: `/target`.
- `march1/.claude/settings.local.json`: permission allow `rustc --version`.
- `march1/src/lib.rs`: the interpreter (values, stacks, parser, dispatch, thunks, arrays, constraints, errors, modes, REPL).
- `march1/src/hash.rs`: WordHash bit layout, primitives, unused ITC Runtime, 3 unit tests.
- `march1/src/database.rs`: SQLite schema, store/alias/lookup, 5 unit tests.
- `march1/src/state.rs`: typed global state, constraints, statemap aliases, 4 unit tests.
- `march1/src/main.rs`: REPL or `--test` Rust-API arithmetic smoke run.
- `march1/demo_content_addressing.rs`: hash-sharing and state demo.
- `march1/demo_persistence.rs`: 3-session SQLite persistence demo.
- `march1/demo_multimodal.rs`: 6-mode execution demo.
- `march1/demo_error_handling.rs`: `Error:`/`raise`/`?` handler demo.
- `march1/demo.db`: inspected with sqlite3 (4 words, 13 alias rows).
- `march1/test_arrays.rs`: array literal bin.
- `march1/test_context_signatures.rs`: `? [ sig ] ;` bin.
- `march1/test_context_signatures_new.rs`: thunk-based `?` bin.
- `march1/test_context_simple.rs`: minimal thunk `?` bin.
- `march1/test_extensible_operators.rs`: DB-aliased operators bin.
- `march1/test_operator_override.rs`: user `+` override bin.
- `march1/test_thunks.rs`: thunk create/force bin.
- `march1/test_type_dispatch.rs`: context-signature dispatch bin.
- `march1/tests/interpreter_tests.rs`: 9 arithmetic and definition tests.
- `march1/tests/persistence_tests.rs`: 2 multi-session DB tests.
- `march1/tests/state_integration_tests.rs`: 4 state tests (old `State:` syntax).
- `march1/tests/contextual_dispatch_tests.rs`: 4 predicate-context tests (old `Context` syntax).
- git: `log archive/march1/main`, `log archive/march1/preserved/wip-2026-09-23`, `show` of tags v0.1.0/v0.2.0 and all 10 commit messages, `diff --stat` between refs, `v0.1.0`/`v0.2.0` `src/lib.rs` (execute_word, context functions), `demo_multimodal.rs` diff.
