# March 5 (α₅) digest

Source: `/home/trans/my/com/tabcomputing/march/march5` (read-only). Its own `.git` HEAD is `aa89906 Archive uncommitted March 5 implementation work`, which is the same commit as `archive/march5/preserved/wip-2026-09-23`. All paths below are relative to `march5/` unless stated otherwise.

Tags used: **[IMPL+TEST]** means implemented and covered by a passing unit test. **[IMPL]** means the code exists and I ran it through the CLI, but no unit test covers it. **[DESIGN]** means it appears only in docs. **[BROKEN]** means I reproduced a defect.

History: 36 commits on `archive/march5/main`, dated 2025-10-23 (`215eb59 AI generated prototype document.`) through 2025-11-02 (`e91c734 Renamed design documents...`). The wip commit adds `src/surface.rs` (1025 lines), `src/sexpr.rs` (200), `docs/INET.md` (228), and changes to `catalog.rs`, `yaml.rs` and `inet.rs` (git diff stat: 1799 insertions, 359 deletions). Many docs were written for an AI implementer: "It is written so an assistant (e.g., Codex) can implement it directly" (docs/design/DESIGN-OVERVIEW.md:3), and there are commits named "Codex contrib..." and "OpenAI updates to docs".

---

## 1. Execution model as implemented

### 1.1 Interaction nets: were programs lowered to nets and reduced? No.

- **The thing that executes words is not an interaction net.** It is a recursive, demand-driven evaluator over a content-addressed dataflow DAG of `node` objects (src/interp.rs:370-545 `eval_node`, :547-589 `eval_return`). The code calls these "Mini-INet nodes" (src/node.rs:1), but they have no principal ports, no active pairs and no rewrite rules. Each node is a CBOR record with its inputs given as `[producerCID, port]` edges.
- **A separate real i-net scaffold exists** in `src/inet.rs` [IMPL+TEST, isolated]:
  - `Net` holds agents with named ports and a `Vec<NetWire>` (inet.rs:101-192).
  - `find_active_pair` scans every wire linearly for a port-0 to port-0 connection (inet.rs:178-191).
  - `reduce_step` hard-codes one rule, pair/unpair annihilation (inet.rs:194-261).
  - `Reducer` loads `rule` objects from SQLite and applies a small S-expression rewiring DSL: `connect`, `disconnect`, `new`, `delete` (inet.rs:263-523).
  - Tests cover pair/unpair, disconnect then connect, a gtype/if rewire, and deopt delete (inet.rs:525-877).
  - There is no reduction loop. `step()` applies one rule and nothing iterates it to normal form.
  - Rules are keyed by agent-name strings (inet.rs:266). Agent and rule CBOR are key/value maps, unlike the positional arrays used for nodes (inet.rs:26-62).
- **Nothing connects the two models.** Outside inet.rs, the only uses of `inet::` are the CLI `agent add` and `rule add` storage commands (src/cli/commands/mod.rs:81-118). A grep for `Reducer`, `reduce_step` and `Net` finds no call site in the interpreter, builder or CLI.
- The planned bridge is listed as unfinished work: "Build the graph→inet translator that opportunistically reduces nets and falls back to the interpreter where rules are missing" (docs/planning/PLAN-INET.md:12). The rules still needed are listed at PLAN-INET.md:11 and DESIGN-INET.md:63-72 ("Next Session Plan").
- docs/INET.md opens with "NOTE: This is a long term goal. IGNORE FOR NOW!" (INET.md:3-5).

### 1.2 Node, word and object model (canonical CBOR, CIDs)

- **CID** = SHA-256 of hand-emitted canonical CBOR, stored as 32 raw bytes (src/cid.rs:6-13, src/cbor.rs). Names are deliberately kept out of CIDs: "Names are NOT part of object CIDs to maximize deduplication" (DESIGN-OVERVIEW.md:19).
- **Object tags** (positional arrays): prim=0, word=1, iface=3, namespace=4, node=6, guard=7, gstate=8. Effects, agents and rules are maps instead.
- **node** [IMPL+TEST]: `[6, nk, inputs[[cid,port]...], outTypes[], effectCIDs[], payload]` (src/node.rs:97-111).
  - 15 kinds: LIT, PRIM, CALL, ARG, LOAD_GLOBAL, RETURN, PAIR, UNPAIR, QUOTE, APPLY, IF, TOKEN, GUARD, DEOPT, DISPATCH (node.rs:59-77, tags at :121-139).
  - LIT carries only i64 (node.rs:195-201, 340-343).
  - RETURN's payload is `[vals, deps]`. `deps` is sorted and deduplicated; `vals` keeps its order (node.rs:187-194).
- **RETURN node design** (docs/design/DESIGN-RETURN-NODE.md) is implemented [IMPL+TEST]:
  - RETURN is always a word's root.
  - `vals` holds the result edges and `deps` holds the edges that sequence effects. "RETURN encodes the contract: what to return (vals), what must run (deps), and where callers begin" (DESIGN-RETURN-NODE.md:104-108).
  - Zero, one and many results work. Tests: `run_word_supports_void_result` and `run_word_supports_multi_result_literals`.
- **word** [IMPL+TEST]: `[1, rootCID, params[], results[], effectCIDs[], effectMask, guardCIDs[]]` (src/word.rs:29-63).
  - Types are string atoms: i64, f64, ptr, text, unit, and token kinds (src/types.rs:76-122).
  - The docs describe a 5-element word (DESIGN-OVERVIEW.md:196-204). The code writes 7 elements, adding the mask and guards.
- **prim**: `[0, zeroRoot, params, results, effects, mask]` (src/prim.rs:41-62). **Defect: this has no operation identity** (section 4).
- **Hash-consing**: each node is inserted with `INSERT OR IGNORE` (src/db.rs:104-110). Identical subgraphs therefore share storage.
  - Effectful nodes do not collapse into each other, because each one consumes the previous node's token output. That makes their inputs, and so their CIDs, distinct (builder.rs:717-721, 756-775).

### 1.3 Words and the builder (stack to graph)

`GraphBuilder` (src/builder.rs) keeps a compile-time stack of `(cid, port, type)` entries. [IMPL+TEST]

- **Stack shuffles emit no nodes.** dup, swap, over, drop, nip, tuck, rot and -rot only rearrange wiring (builder.rs:601-680). Test: `dup_swap_over_are_wiring_only`.
- `begin_word` seeds one ARG node per parameter (builder.rs:143-176).
- `apply_prim`, `apply_word` and `apply_quotation` all go through `apply_general`, which:
  - pops the arity,
  - checks the concrete types,
  - adds one token input per effect domain,
  - emits token outputs first and data outputs after them (builder.rs:683-832).
- `finish_word` builds the RETURN node from the remaining stack plus the current tokens, and the effect frontier becomes `deps` (builder.rs:843-1074).
- **[BROKEN] Result ordering for mixed types.** `finish_word` compares `results[idx]` against the stack item `len-1-idx`, i.e. top first, and then reverses `vals` (builder.rs:865-884). The interpreter checks results in stack order (interp.rs:267-281).
  - Reproduced: a word leaving `unit, i64` (i64 on top) is only accepted by the builder when declared `results: [i64, unit]`.
  - Running it then fails with `error: result 0 type mismatch: expected I64, got Unit`.
  - Homogeneous result lists hide the bug.

### 1.4 Quotations [IMPL+TEST, static only]

- `QUOTE(wordCID)` pushes `Value::Quote(cid)`, whose type tag is `ptr` (builder.rs:581-599; interp.rs:336, 444-447).
- `APPLY` carries the quotation id **statically in the node payload**: `[qid, typeKey?]` (node.rs:241-254).
  - The caller must pass the params, results and effects itself (`apply_quotation`, builder.rs:815-832).
  - At runtime `eval_apply` just calls `run_word(qid)`. `_type_key` is ignored (interp.rs:717-728).
- **No dynamic apply exists.** No node takes a quote value from the stack and calls it.
- The surface syntax (`StackOp`, src/surface.rs:162-171) and the REPL have no apply or call-quote operation.
- In practice, quote values are only used as keys for state primitives (`quote_key`, interp.rs:753-761).
- `EXEC_GENERIC`/`QTABLE` for indirect calls exist only in the ABI design [DESIGN] (docs/design/DESIGN-INET-ABI.md:75-81).

### 1.5 Strict vs lazy

The interpreter is demand-driven from the root and memoizes per word invocation. Its inputs are evaluated eagerly.

- Evaluation starts at RETURN. It evaluates all `deps` first, then `vals` (interp.rs:578-586).
- Each node recursively evaluates **all** of its inputs before running its own kind (interp.rs:397).
- Results are memoized in a `HashMap<CID, Vec<Value>>` that is created fresh per `run_word` call (interp.rs:239, 376-378, 543).
- Anything not reachable from RETURN is never run. Pure dead code is therefore skipped, and effectful nodes are forced only through `deps` or token `vals`.
- IF and GUARD evaluate only the chosen continuation. Their branches are `NodeInput`s held in the payload, not inputs (interp.rs:453-491).
- Every node load is a SQLite read plus a CBOR decode (interp.rs:380-382). There is no in-RAM `GraphRAM`. The C-struct lowering at DESIGN-OVERVIEW.md:319-352 is [DESIGN].
- The designed parallel work-stealing ready-queue interpreter (DESIGN-OVERVIEW.md:409-415) is [DESIGN]. The actual interpreter is sequential recursion.

### 1.6 Control flow

- IF, GUARD, DEOPT and DISPATCH work in the interpreter [IMPL+TEST]:
  - `run_word_handles_if_true_branch`
  - `run_word_guard_match_branch`, `run_word_guard_else_branch`, `run_word_guard_else_deopt`
- They are not reachable from any surface syntax:
  - The CLI `node` subcommands are lit, prim, call, arg and load-global (src/cli/mod.rs:211-251).
  - The REPL commands are begin, lit, prim, call, attach-guard, dup, swap, over, finish (src/cli/commands/builder.rs).
  - YAML and S-expression `StackOp` have no `if` (surface.rs:162-171).
- `GraphBuilder::branch_if` is `#[allow(dead_code)]` (builder.rs:94-140). It builds an IF node with `out: Vec::new()`, which `validate_node` always rejects ("non-RETURN node must declare at least one output type", node.rs:333-335). **[BROKEN, unused]**
- **There are no loops.** `LOOP` is listed as "(Later)" (DESIGN-OVERVIEW.md:55).
- Deopt is signalled by an error whose message contains `"deopt triggered"`, detected with `err.to_string().contains(...)` (interp.rs:770-772).

### 1.7 Primitives and "JIT"

- **Primitives are dispatched by mutable name, not by CID** [BROKEN]. `eval_primitive`:
  - looks up every `name_index` name for the prim CID,
  - picks a base name using the hard-coded `PRIMITIVE_PRIORITY` list (`eq_i64, gt_i64, ge_i64, lt_i64, le_i64, and, or, not, add_i64, sub_i64`),
  - otherwise matches exact names such as `state.read_i64` (interp.rs:840-1084).
- Primitives that exist in total: 10 arithmetic/logic ones and 8 `state.*` read/write ones. **There is no print or other IO primitive**, so the IO token has nothing to sequence except test prims.
- **"JIT"**: `src/exec.rs` mmaps two hard-coded 7-byte x86-64 functions (add and sub), marks them executable, and calls them for `add_i64`/`sub_i64` (exec.rs:17-60; interp.rs:873-890). That is the whole native path.
- The `code_cache` table is created (db.rs:86-93) but no code reads or writes it.

### 1.8 The kernel

- `kernel/x86-64/*.asm` holds 44 NASM files: `vm.asm`, a tagged-cell inner interpreter ("Executes pre-compiled cell streams with tagged dispatch", vm.asm:1-17), plus one file per Forth primitive (add, dup, swap, `>r`, 0branch, docol, execute, ...).
- It was added in commit `41335d6` ("Load compiled x86-64 primitives and support args").
- **No Rust or Cargo file references it.** Grepping for `kernel` or `.asm` under `src/` and `Cargo.toml` finds nothing. It is the march4-style VM kernel, carried along and never built. [unused]

---

## 2. Namespaces, interfaces, bindings, lockfiles, global store, persistence, effects, web UI, YAML

### 2.1 Namespaces (design, in depth) [DESIGN]

Main source: docs/design/DESIGN-NAMESPACES.md.

**What a namespace is.** It is a compile-time construct only: "Namespaces are static, compile-time constructs. They do **not** exist as runtime values" (:5). It does three jobs: exports, the external interfaces it requires (`bindings`), and non-semantic `use` aliases (:6-9).

**Canonical shape**, in the map form: `{kind, bindings:[{interface:CID}], exports:[{name, word}], interface: ownIfaceCID}` (:15-26).
- `bindings` means "The real dependency list used for linking and compatibility checks" (:29-31).
- `use` aliases, the human-readable name and docs are all excluded from the CID (:39-44).

**Resolution algorithm** (:86-103):
1. An absolute name resolves directly.
2. A qualified `alias.symbol` goes through the AliasTable.
3. An unqualified name checks local exports first, then each `use ... as *`.
- An ambiguous unqualified name is an error, and local exports shadow imports.
- Every reference that resolves outside the current namespace adds the provider's **interface CID** to a `bindings_set`. After graph construction: `bindings = sort(unique(bindings_set))` (:124-133).

**Binding rule.** "A namespace's `bindings` is the set of **interface CIDs** it requires. It is **not** a list of specific symbol→CID pairs. Symbol-level identity already lives inside graphs as `NodePayload::Word(wordCID)`" (:275-278).

**Namespaces rejected as runtime values** (:146-160), because that "breaks determinism... breaks static linking... ruins tooling".

**Implemented** [IMPL]:
- `NamespaceCanon {imports: ifaceCIDs, exports: (name, wordCID), iface}` is encoded as `[4, iface, sortedImports, exports]` (src/namespace.rs:9-38).
- CLI `namespace add|list|show` exists. If `--iface` is omitted, it derives and stores the interface from the exported words' metadata (src/cli/commands/namespace.rs:11-53; src/iface.rs:47-73).

**[BROKEN] Namespace export encoding.**
- `encode_exports` writes an array header of N and then 2N items, a name and a CID per export (namespace.rs:57-65).
- Reproduced: `namespace show demo` decodes to `[4,[iface…],[],["hello"]]`. The 34 bytes of the word CID sit after the top-level array. The CID still hashes them, but decoding loses the name→word mapping.

**Not implemented:**
- No resolver uses namespaces. `run` and the builder resolve through the flat `name_index` (`get_name(conn,"word",name)`, cli/commands/mod.rs:43-44; `lookup_named_cid`, util.rs:220-228).
- The catalog importer skips namespaces: `"skipping namespace ... (not yet supported by catalog import)"` (catalog.rs:104-109).
- Surface namespace blocks only prefix names. `normalize_reference` turns `.` into `/` and prepends the scope (surface.rs:784-798).
- `use` aliases are not implemented: "Namespace imports currently drop alias metadata (`use` sugar)" (STATUS.md:13).

### 2.2 Interfaces [IMPL+TEST]

- An interface is `[3, [[name, params[], results[], effectCIDs[]]...]]`, sorted by name with effects sorted (src/iface.rs:30-37, 75-100).
- The design calls it "Interfaces over versions: Interface CIDs + test gates replace semver" (DESIGN-OVERVIEW.md:14). The compact array form is argued at DESIGN-NAMESPACES.md:302-361.
- Test: `derive_interface_from_words`.
- Interface compatibility checking at bind time is [DESIGN]: "Require equality to ifaceCID_req (or allow supersets as policy)" (DESIGN-OVERVIEW.md:394-398).
- The test gate ("Run imported namespace test suite... Update lockfile upon success", DESIGN-OVERVIEW.md:400-407) is [DESIGN].

### 2.3 Lockfiles [DESIGN only]

- **DESIGN-OVERVIEW §11** (:455-478): `march.lock` pins toolchain, entry, and name→CID maps for namespaces, words, ifaces and globals.
- **§4.5** (:248-253): the program object may later gain an optional sorted `[namespaceCID...]` slot.
- **DESIGN-NAMESPACES** (:135-145, :228-247): the lockfile records, per dependency, `{cid: nsCID, interface: ifaceCID}`.
  - Key idea: "If a dependency's implementation changes but the `interface` CID remains the same, the lockfile does not need updating" (:144, repeated at :245).
  - "Lockfiles improve reproducibility and prevent silent provider substitution" (:247).
- **PLAN-LOCKFILE.md:5**: "Resolver and name-index machinery exist, but no lockfile artifacts are written/read yet." Its next steps are to design the schema, capturing "catalog sources, resolved CIDs, effect masks" (:9).
- A grep for `lockfile`, `march.lock` and `bindings` in `src/` finds nothing.
- The `program` object (tag 5) has no encoder in `src/`. [DESIGN]

### 2.4 Global store (src/global_store.rs, PLAN-GSTORE) [IMPL+TEST]

- It is a process-global `static STORE: Lazy<RwLock<GlobalStore>>` wrapping a `BTreeMap<String, Value>` (global_store.rs:14-38, 78).
  - The docstring says it holds "immutable values keyed by namespace-qualified names" (:14), but it is written in place.
- Operations: `read`, `write`, `snapshot`, `restore`, `reset` (:80-110).
- Snapshots are encoded canonically as `[8, [[key, [typeAtom, payload]]...]]` and stored as `gstate` objects (:112-135). Values supported: i64, f64, unit, quote, tuple, text (:156-201).
- The interpreter's `state.read_*` and `state.write_*` primitives key the store by the **hex of a quote CID** (interp.rs:964-1076). Tests: `state_read_write_roundtrip`, `..._f64_...`, `..._ptr_tuple`, `..._text`.
- The CLI has `state snapshot|reset|save|load|list` (src/cli/commands/state.rs).
  - Because the store lives in one process, `state load` followed by a separate `run` invocation shares nothing, and `cmd_run` never restores a snapshot (cli/commands/mod.rs:36-66).
  - Persistence is therefore snapshot objects only, with no automatic durability. PLAN-GSTORE.md:11 says: "Add durability options (persist snapshots automatically, scheduled checkpoints) once transaction work lands."
- PLAN-GSTORE.md:17: "Keep the global store strictly opt-in for words that declare the appropriate effect tokens."
- Designed but not built [DESIGN]:
  - Durability flags V/B/D, CHECKPOINT, and SQLite write-through (DESIGN-INET-ABI.md:269-320).
  - MVCC transactions TXN_BEGIN/COMMIT/ABORT (DESIGN-INET-ABI.md:154-180). PLAN-TRANSACTIONS.md:5: "Pending – no prototype exists yet."
  - Immutable `global` objects and LOAD_GLOBAL (DESIGN-OVERVIEW.md:255-276, 425-430). The interpreter says `4 => bail!("LOAD_GLOBAL not supported by runner (yet)")` (interp.rs:427).

### 2.5 Database / SQLite [IMPL+TEST]

- Tables:
  - `object(cid BLOB PK, kind, cbor) WITHOUT ROWID`
  - `name_index(scope, name, cid)` with PK `(scope, name)`
  - `code_cache` (unused)
  - indexes on kind and on `(scope, cid)` (src/db.rs:70-101)
- Pragmas: WAL, synchronous=NORMAL, mmap 256MB, cache about 256MB (db.rs:60-68).
- `name_index` is the only mutable part and is overwritten with `INSERT OR REPLACE` (db.rs:112-119). Scopes in use: word, prim, guard, namespace, iface, effect, gstate, agent, rule.
- DESIGN-DATABASE.md describes this faithfully.
- There is no format version or migration. PLAN-DATABASE.md:18: "Any schema change should ship with a migration story (even if “drop and rebuild” during alpha)."
- **[BROKEN]** The committed example DB uses an older map-based encoding. `run org.march.helloworld/hello` fails with `error: invalid type: map, expected tuple struct WordRecord` (reproduced).
- PLAN-DATABASE.md:5 refers to `src/store.rs`, which does not exist. Storage lives in `db.rs`.

### 2.6 Effects, effect tokens, and IO ordering [IMPL+TEST]

**Two layers of effect description:**
- Effect CIDs, from `effect` objects `{kind:"effect", name, doc?}` (src/effect.rs:25-43).
- A runtime bitmask: IO=1, STATE_READ=2, STATE_WRITE=4, TEST=8, METRIC=16. This maps to domains Io, State, Test, Metric, each with its own token type (`io.token`, `state.token`, ...) (src/types.rs:10-68).
- If a node has effect CIDs but a zero mask, the mask is normalized to IO (builder.rs:713-716; interp.rs:124-130).
- PLAN-EFFECTS.md:6 notes: "Effect masks currently collapse to `effect_mask::IO`" when unspecified.

**How ordering works:**
- Tokens are **ordinary graph values on node ports**.
- The builder keeps a `TokenPool: HashMap<EffectDomain, NodeInput>` (builder.rs:30-51).
- For each effectful node, `apply_general` appends the current token of each domain as an input (creating a `TOKEN` source node on first use) and makes the node's first output ports the new tokens, which it records back in the pool (builder.rs:717-775, 1094-1116).
- Effects in one domain are therefore totally ordered by data dependency. Separate domains advance independently.
- At `finish_word`, the final token of each domain becomes a leading `val` of RETURN, and the `effect_frontier` (keyed by effect CID) becomes `deps` (builder.rs:886-909).
- Words with effects return their tokens as the first outputs. Test `run_word_with_multiple_tokens` expects `[Token(Io), Token(State), I64(3)]` (interp.rs:1123-1151).

**Runtime:**
- `consume_token_inputs` and `validate_output_tokens` check the domain of each token (interp.rs:149-215).
- A token is a unit-like marker `Value::Token(Option<EffectDomain>)` (interp.rs:324).
- Because evaluation is demand-driven, an effect runs only if its token or dep path reaches RETURN. Test `finish_word_tracks_effect_dependencies`.

**Token boundaries across calls.** Tokens do not cross a call boundary as parameters.
- A CALL node consumes the caller's current token and emits a fresh one (interp.rs:412-418).
- The callee creates its own TOKEN source internally, via its own builder session.
- Ordering between caller and callee is therefore enforced at the CALL node.

**Design ideas not implemented** (DESIGN-TOKEN-POOL.md; DESIGN-INET-ABI.md:22-39):
- Split R/W tokens: duplicable `RTOKEN`, linear `WTOKEN[d,TID,epoch]`.
- A post-build linearity verifier (DESIGN-TOKEN-POOL.md:112-116).
- Dev/Prod elision of optional effects (logs, tests, metrics) to `ERASE` (:78-88, :149-151).
- "Don't hide the token behind a singleton. Treat it as a first-class IR value" (:152).
- The implemented token pool is the single-token-per-domain baseline.

### 2.7 Web UI [IMPL, no tests]

- `src/bin/webui.rs` (679 lines) runs a `tiny_http` server.
- The index HTML lists namespaces, interfaces, words and prims.
- JSON endpoints: `/api/{iface,agent,rule,namespace,word}/<name>` and `/api/list/<scope>` (webui.rs:61-128). CBOR is converted to JSON.
- It is read-only: there are no POST or authoring forms. Those are planned at DESIGN-INET.md:74-76.
- `webui/` is an empty directory.
- It builds (`cargo build --bin webui` exit 0). I did not start the server.

### 2.8 YAML and S-expression catalogs [IMPL+TEST]

- `src/yaml.rs` is a **hand-rolled YAML subset parser** (indentation, sequences, mappings and `!tags`; yaml.rs:14-256) that decodes into `surface::CatalogEntry`.
  - Tags: `!effect`, `!prim`, `!word`, `!guard`, `!overloads`, `!snapshot`/`!state` (yaml.rs:383-400).
  - Stack ops: `!prim`, `!word`, `!dup`, `!swap`, `!over`, `!quote`, `!lit` (yaml.rs:609-650).
- `run --args-yaml` parses typed argument lists (`!i64 !f64 !text !tuple !quote !unit`; yaml.rs:258-325).
- The `catalog` command accepts either YAML or S-expression files. Extensions `.sexpr`, `.sxp`, `.scm` and `.lisp` route to `surface::parse_catalog_from_sexpr_str` (catalog.rs:20-26, 180-188).
- **Verified end to end (my runs):**
  - A YAML catalog defining `core/add_i64` and `demo/double: dup add_i64`: `run demo/double --arg 21` gives `42`.
  - An `!overloads demo/add` plus a word calling `!word demo/add`, loaded in a later catalog: `run demo/useadd --arg 3 --arg 4` gives `7`.
- **Ordering limitation.** Within one catalog, words are built before overload sets (catalog.rs:127-163). A word that references an overload defined in the same file fails with `word 'demo/add' not found and no overloads registered` (reproduced).

---

## 3. Types and syntax (surface.rs), examples

### 3.1 Types

- A closed enum `TypeTag {I64, F64, Ptr, Text, Unit, Token, StateToken, IoToken, TestToken, MetricToken}` (types.rs:76-88).
- Tuples and quotes both report type `ptr` (interp.rs:335-336).
- There are no type variables, no polymorphism and no inference. Checks are exact equality on concrete tags, in the builder (builder.rs:694-703) and at runtime (interp.rs:229-238, 267-281).
- Typedef CIDs are planned: "Once we have typedef CIDs, format will be..." (DESIGN-OVERVIEW.md:128-138). [DESIGN]
- Guard and dispatch "type keys" are the type atom text zero-padded to 32 bytes, not hashes (builder.rs:218-224; interp.rs:763-768).

### 3.2 Surface syntax (src/surface.rs)

- It is **an S-expression catalog language, not Forth text**. The module doc: "Shared surface-level AST for March definitions... Frontends such as the YAML loader or a future S-expression parser can decode their input into these specs" (surface.rs:1-7).
- Forms: `(namespace NAME (imports ...) (iface ...) (exports ...) <nested defs>)`, `(effect ...)`, `(prim N (params ...) (results ...) (effects hex...) (emask io state ...))`, `(guard ...)`, `(word N (params) (results) (stack ...) (guards ...))`, `(overloads N (entry ...)...)`, `(state N (key value)...)` (surface.rs:199-246).
- `StackOp` is limited to `prim`, `word`, `dup`, `swap`, `over`, `lit`, `quote` (surface.rs:162-171, 657-696). There is no drop, if, loop or apply.
- The catalog applier only accepts `lit` values of type i64 (catalog.rs:284-289).
- The `Definition` enum declares `Interface`, `Agent` and `Rule` variants (surface.rs:28-30), but no parser branch creates them, and catalog.rs bails on them (:110-116).
- The Forth-like textual syntax envisaged by "Textual serialiation is Forth like" (DESIGN-OVERVIEW.md:5) was never built. The `GraphBuilder` REPL (`march5 builder`: begin, lit, prim, call, dup, swap, over, attach-guard, finish, finish-guard) is the closest thing.
- Test: `parse_catalog_sexpr_basic` (surface.rs:885-1024) produces 8 entries.

### 3.3 Examples

- `examples/helloworld/helloworld.march5.db` plus its README: word `org.march.helloworld/hello` returns literal 42, and namespace `org.march.helloworld` exports it (README.md:3-35). The DB is stale (section 2.5).
- The README's `!overloads` example uses a placeholder concat body, "`- !prim core/add_i64   # placeholder; replace with proper concat prim/word`" (examples/helloworld/README.md:54). It says "The base symbol is reserved for a future dispatcher or static resolver" (:69-72).
  - That is out of date. `catalog.rs:298-354` now resolves base names statically by top-of-stack types, emitting a direct CALL when exactly one unguarded candidate matches and a DISPATCH node otherwise. The top-level README.md:264 also still says "calling an overloaded symbol by base name is not yet wired".
- Fresh-DB CLI flow I ran: `new` → `node lit 42` → `word add` → `run demo/hello` prints `42`.

---

## 4. What worked, what failed, and why it was abandoned

### 4.1 Test results (observed)

Run on a scratch copy with `cargo test --offline`:
- **63 passed, 0 failed, 0 ignored** in the library unit tests.
- 0 tests in the `main` and `webui` binaries, 0 doc-tests.
- One warning: `method 'peek_top_inputs' is never used` (builder.rs:201).
- There is no `tests/` directory.
- Covered: cid, cbor/node/word/prim/iface/namespace/effect encoding, builder (wiring-only shuffles, RETURN ordering, void words, effect deps, pair/unpair, quotation apply, guard persistence, dispatch lowering), interpreter (multi-result, multi-token, state round-trips for four value kinds, guard pass/fail/else/deopt, IF, boolean prims), global_store snapshots, inet (4 DSL rewrites plus agent/rule storage), sexpr, surface, yaml.
- Both binaries build (`--bin march5`, `--bin webui`).

### 4.2 CLI reproductions (my runs)

| Run | Result |
| --- | --- |
| Committed helloworld DB | fails: `invalid type: map, expected tuple struct WordRecord` |
| Fresh DB, literal word | `42` |
| Catalog `double` (add_i64 only) | `42` |
| Overload resolution across two catalogs | `7` |
| **Catalog defining both `core/add_i64` and `core/gt_i64` (same signature)** | both stored with the **same CID** `8a4a6207…`; `run demo/double --arg 21` gives **`0`** (it ran gt, not add) |
| Namespace export decode | word CID lost (section 2.1) |
| Mixed-type multi-result word | builder/interpreter order mismatch (section 1.3) |

The same-CID collision happens because the prim CID covers only the signature (prim.rs:41-62), and the interpreter chooses among colliding names with `PRIMITIVE_PRIORITY`, where `gt_i64` comes before `add_i64` (interp.rs:846-857). The 2026-09-23 audit reproduced the same defect with add/sub (`doc/AUDIT-2026-09-23.md` "March 5 critical primitive-identity defect").

### 4.3 What worked (evidence-backed)

- A canonical, hash-consed, names-outside-CIDs object store: nodes, words, ifaces, guards and snapshots.
- Stack-to-graph lowering in which shuffles are pure wiring.
- An explicit RETURN node with vals and deps, including multi-result and void words.
- **Effect tokens as ordinary graph values**, one per domain, ordering side effects structurally, with a demand-driven interpreter that runs effects only when reachable.
- Guards lowered into graphs as `IF(cond) → LIT 1 | DEOPT` dependencies (builder.rs:918-1032).
- Build-time overload resolution plus a runtime DISPATCH node for guarded candidates.

### 4.4 What failed or was never reached

**(a) The interaction-net approach, which is the author's stated failure.** The repository contains no rejection note. The evidence is structural:
- The "Mini-INet" that runs is a DAG interpreter. The real i-net (`inet.rs`) is an isolated scaffold with a single-step reducer, string-keyed rules and 4 toy tests. It is never invoked on a program (section 1.1).
- The plan never got past its first step. The "Next Session Plan" (DESIGN-INET.md:63-77) and PLAN-INET.md:11-12 list core rules (guard, if, deopt, call/apply, return/token threading) and the graph→net translator as not done. The last dated status is `(2025-03-05)` (DESIGN-INET.md:54; STATUS.md:5). Those 2025-03-05 dates are older than the first commit (2025-10-23), so they appear to be erroneous.
- The later long-form i-net doc was demoted: "NOTE: This is a long term goal. IGNORE FOR NOW!" (docs/INET.md:3-5).
- The ABI spec sketches agents for everything (continuation ports `k` on every agent, tokens, transactions, constraint agents, test/log agents; DESIGN-INET-ABI.md), but "REWRITE SKETCHES (ILLUSTRATIVE, ENGINE-SPECIFIC)" (:207) never became rules.
- The model tension is visible:
  - DESIGN-INET.md:4-7 makes "agent kinds + rewiring rules" the core, while DESIGN-OVERVIEW.md:13 says "Mini-INet as the only IR".
  - The implemented IR has non-linear fan-out: a stack item can be consumed many times through `dup`, which only duplicates wiring (builder.rs:601-610).
  - In a true interaction net, sharing needs explicit duplicator agents. None exist in either model.
  - The 2026-09-23 audit concluded: "It currently demonstrates a promising CID-addressed graph interpreter, not a viable interaction-net backend" (doc/AUDIT-2026-09-23.md:235-236), and "The current graph is not an interaction net" (:215).

**(b) The content-addressing invariant broke for primitives** (above). Behaviour depended on mutable `name_index` aliases.

**(c) There was no format-evolution boundary.** The project's own example DB no longer decodes (section 2.5).

**(d) The language surface stayed thin.**
- Only i64 literals and no textual Forth.
- No control flow, loops or dynamic quotation apply in any surface.
- No IO primitive.
- LOAD_GLOBAL is unimplemented.
- Namespaces, lockfiles, transactions, the code cache and parallel execution are design only.

**(e) Native compilation** is two hand-assembled stubs (exec.rs:17-18).

The project lasted about 11 days of commits (2025-10-23 to 2025-11-02), with a later uncommitted surface and S-expression layer.

---

## 5. Ideas bearing on march6's open questions

**Lazy vs strict**
- March 5 used demand-driven evaluation from a RETURN root: inputs are evaluated eagerly, unreachable nodes are skipped, only the chosen IF/GUARD branch runs, and results are memoized per invocation (section 1.5).
- Effects are kept alive not by evaluation order but by explicit `deps` edges and token `vals` on RETURN. That is the transferable idea: **a demand-driven evaluator stays correct for effects if the root structurally demands the effect frontier** (DESIGN-RETURN-NODE.md:20-24, 57-62).
- There are no performance numbers anywhere in march5. The interpreter reads SQLite per node (interp.rs:380), so none would be meaningful.

**How dynamic quotation calls knew stack effects: they did not.**
- APPLY is static. The quotation CID is in the node payload, and the builder is handed params, results, effects and mask explicitly (builder.rs:815-832).
- There is no operation that pops a quote value and calls it.
- The designed answer, not built, was a `TypeKey := hash of (arg ground types, result ground types, effect mask)` (DESIGN-INET-ABI.md:14) carried on `APPLY(qid@TypeKey)`, plus `EXEC_GENERIC`/`QTABLE` for indirect calls with `TAG`/`GUARD`/`DEOPT` specialization (:67-85, :209-220).
- Pure quotations take no token ports. Effectful ones take one per domain in their effect row (:18-20).

**How effects and state were ordered (effect tokens)**
- Per-domain linear tokens are threaded as real data edges.
- The builder-side `TokenPool: HashMap<Domain, NodeInput>` holds the current token of each domain (section 2.6).
- The design's growth path: split into duplicable R and linear W tokens with transaction id and epoch; make only optional domains (test, metric, log) erasable in Prod; add a linearity verifier pass (DESIGN-TOKEN-POOL.md).
- The global store was keyed by quote CID, used an in-process map, and persisted through CBOR snapshot objects.
- DESIGN-TOKEN-POOL.md:151: "Only optional effects should be erasable... Core semantics (like your language “global variables”) must require a token or compilation should fail."

**Context-oriented dispatch (guarded definition families)**
- Implemented as pure guard quotations (i64 result; guards with effects are rejected; interp.rs:84-95, builder.rs:416-424) attached to words.
- They are lowered into the word as an `IF → DEOPT` dependency, or combined into a DISPATCH node over candidate CALLs with type keys plus lowered guard CALL nodes. The first candidate whose type keys match and whose guards all pass wins (interp.rs:492-538; builder.rs:226-393).
- Overload sets are named `<base>#<params->results>`. Build time picks the candidates matching top-of-stack types, emits a direct CALL for a unique unguarded one, and emits DISPATCH otherwise (catalog.rs:298-354).
- Designed extensions:
  - `GUARDCTX(pred)` that reads state through R/W tokens (DESIGN-INET-ABI.md:98-102, 200-204).
  - Context objects `{atoms:[ctxCID...]}` with `GUARD_CTX` nodes "or resolve at link-time if context is known" (DESIGN-OVERVIEW.md:432-453).
  - Guard elimination when statically known (docs/INET.md:208-217).
  - PLAN-GUARDS.md:16: "prefer encoding policy as guard stacks instead of new bespoke plumbing."
- The original expansion idea: try each context candidate's guard in order and expand the first that matches, erroring if none do (docs/INET.md:48-56).

**Global store and namespaces**
- March 6's DEPENDENCY-EVOLUTION.md already cites March 5 for interface identity.
- The ideas most worth keeping from DESIGN-NAMESPACES.md:
  - Bindings are interface CIDs, collected automatically during resolution.
  - The lockfile maps each dependency to `{nsCID, ifaceCID}`, and an implementation change under an unchanged interface CID needs no lock update.
  - Aliases are lexical sugar outside the CID.
  - Namespaces are forbidden as runtime values.
- None of this was implemented beyond storing iface and namespace objects, and namespace export decoding is broken.

**Performance numbers**: none recorded in any march5 file.

---

## 6. Every file read

**Markdown, all 24 read completely:**
- README.md: CLI usage, YAML catalog, `!overloads` notes, agent/rule CLI.
- STATUS.md: session hand-off dated 2025-03-05; TODOs (transactions, `use` aliases).
- docs/README.md: doc directory map and upkeep rules.
- docs/INET.md: Forth→Inet compilation pipeline vision, marked "IGNORE FOR NOW!".
- docs/design/DESIGN-OVERVIEW.md: master α₅ spec (CIDs, Mini-INet nodes, CBOR, SQLite, lockfile, context, JIT).
- docs/design/DESIGN-NAMESPACES.md: namespaces, interfaces, bindings, resolution, `use` aliases, lockfile, compact encoding.
- docs/design/DESIGN-DATABASE.md: SQLite schema, pragmas, db.rs facade.
- docs/design/DESIGN-INET.md: agents and rules model, incremental path, status, next-session plan.
- docs/design/DESIGN-INET-ABI.md: agent ABI (tokens, quotations, guards, store, TXN, durability, constraints, test/log agents).
- docs/design/DESIGN-RETURN-NODE.md: RETURN node with vals and deps, multi-output.
- docs/design/DESIGN-TOKEN-POOL.md: token pool, R/W and TID tokens, Prod elision, verifier.
- docs/planning/OPEN-QUESTIONS.md: empty.
- docs/planning/PLAN-CLI.md: CLI module split, next steps.
- docs/planning/PLAN-DATABASE.md: encoders and name index done; migrations pending.
- docs/planning/PLAN-EFFECTS.md: per-domain tokens done; masks collapse to IO.
- docs/planning/PLAN-GSTORE.md: global store values and backend done; durability pending.
- docs/planning/PLAN-GUARDS.md: guard stages 1 and 2 done.
- docs/planning/PLAN-INET.md: builder and scaffolding done; rules and translator pending.
- docs/planning/PLAN-LOCKFILE.md: no lockfile written or read.
- docs/planning/PLAN-OPTIMIZE.md: code cache not wired.
- docs/planning/PLAN-TRANSACTIONS.md: no prototype.
- docs/planning/PLAN-WEBUI.md: read-only web UI done.
- docs/planning/PLAN-YAML.md: YAML catalog done.
- examples/helloworld/README.md: hello DB transcript and `!overloads` example.

**Rust source read in full, or all non-test code:**
- src/lib.rs: module list and re-exports.
- src/exec.rs: mmap'd add/sub machine-code stubs.
- src/cid.rs: SHA-256 CID and hex helpers.
- src/cbor.rs: canonical CBOR emitters.
- src/inet.rs: agent/rule encoding, Net, single-step reducer, rule DSL, tests.
- src/node.rs (lines 1-420): node kinds, encoding, validation.
- src/interp.rs (lines 1-620, 700-1100, 1120-1205): runner, eval_node, tokens, dispatch, primitives, token/state tests.
- src/builder.rs (lines 1-1137, plus the apply-quotation test): GraphBuilder, token pool, dispatch, guards, finish_word.
- src/types.rs: TypeTag, effect masks and domains.
- src/global_store.rs (lines 1-260): in-process store and snapshot encoding.
- src/surface.rs (all): S-expression catalog AST and parser.
- src/namespace.rs: namespace encoder (export bug).
- src/iface.rs (lines 1-110): interface encoder and derivation.
- src/word.rs (lines 1-140): word encoder and loader.
- src/guard.rs (lines 1-70): guard encoder.
- src/prim.rs (lines 1-80): prim encoder (no op identity).
- src/effect.rs (lines 1-60): effect encoder.
- src/db.rs (lines 1-120): schema, pragmas, put and name helpers.
- src/cli/commands/mod.rs: run, agent and rule commands.
- src/cli/commands/util.rs: CLI parsing helpers and name lookup.
- src/cli/commands/namespace.rs: namespace add/list/show.
- src/cli/commands/state.rs: state snapshot/reset/save/load/list.
- src/cli/commands/catalog.rs: catalog apply and overload resolution.

**Skimmed (outline or head only):**
- src/yaml.rs: function outline and header (subset YAML parser).
- src/sexpr.rs: header.
- src/cli/mod.rs: command outline and NodeCommand variants.
- src/cli/commands/builder.rs: REPL command list.
- src/bin/webui.rs: routes and function outline.
- kernel/x86-64/vm.asm: header and init.
- kernel/x86-64/add.asm: whole file.

**Not opened:** src/cli/commands/{effect,guard,iface,new,node,prim,word}.rs, src/main.rs, the other 42 .asm files, Cargo.lock.

**Also read:**
- Cargo.toml: dependencies (rusqlite bundled, serde_cbor, tiny_http, libc, ...).
- `/home/trans/my/com/tabcomputing/march/VERSIONS.md`: full; the March 5 entry and its archive refs.
- `/home/trans/my/com/tabcomputing/march/doc/AUDIT-2026-09-23.md` (lines 1-40, 60-240, plus a grep of 240-363): March 4/5 viability audit.
- Archive refs: `archive/march5/{main e91c734, pre-guard e208d09, preserved/wip-2026-09-23 aa89906, remotes/origin/main 9ca5d85}` and their mirrors under `refs/remotes/origin/`; logs inspected and a diff stat of main against wip.
- Scratch artifacts, all under the scratchpad (not deliverables): `m5copy/` (the copy, with `target/`), `m5test.log`, `m5test-full.log`, `e2e*.sh`, `cat1-4.yaml`, `t1-t4.march5.db`.
