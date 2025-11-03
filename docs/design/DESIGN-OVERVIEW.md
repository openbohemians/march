# DESIGN.md — March α₅: CID-first Mini-INets, Interfaces, and Context

This document specifies a **CID-only**, **graph-first** architecture for March. It is written so an assistant (e.g., Codex) can implement it directly.

Note, this design is missing some parts -- inspiration was FORTH. Textual serialiation is Forth like.
But code is in a database. Ultimately a DB connected GUI will be built.

---

## 0. Goals

- **Single simple model**: everything semantic is a content-addressed object (**CID**) with a canonical encoding.
- **Mini-INet** as the only IR: both interpreter and compiler/JIT run the same graphs.
- **Interfaces over versions**: *Interface CIDs* + test gates replace semver.
- **Portability**: all content stored in a single **SQLite** file.
- **Performance path**: straight-line pure subgraphs are JIT/AOT-compiled and cached by **subgraph CID**.
- **Determinism**: immutable globals, explicit effects, canonical encodings.

> **Design choice:** **Names are NOT part of object CIDs** to maximize deduplication. Names map to CIDs via a mutable index table.

---

## 1. Identity & CIDs

### 1.1 What is content-addressed
- `prim`: primitive operator descriptors.
- `node`: mini-inet node.
- `word`: root + type signature (entry of a graph).
- `iface`: exported symbol surface (names + types + effects).
- `namespace`: imports + exports + **ifaceCID** (name excluded from CID).
- `program`: entrypoint + required namespaces (names excluded from CID).
- `global`: immutable constant (scalar or blob tree).
- *(Optional)* `bundle`: topologically ordered list of nodes reachable from a root.

### 1.2 What is **not** content-addressed
- The **name → CID** pointers (mutable).
- The **code cache** (target-specific compiled blobs).

### 1.3 Hash
- Canonical encoding (see §4) → **SHA-256** → 32-byte CID (raw; we can add multihash later).

---

## 2. Mini-INet Model (execution IR)

### 2.1 Node kinds (minimal subset to start)
- `LIT` (i64/f64/ptr/blobCID)
- `PRIM` (arithmetic, compare, tuple ops; payload: primCID)
- `ARG` (argument i)
- `CALL` (call a word; payload: wordCID)
- `IF_CALL` (Forthy `if`: payload trueThunkCID/falseThunkCID)
- `IF_CTL` + `PHI` (structured control; no payload in IF_CTL)
- `TUP` / `PROJ` (aggregate construction/selection)
- `LOAD_GLOBAL` (payload: globalCID)
- *(Later)* `LOOP`, `SWITCH`, `TRY`, etc.

### 2.2 Types
- Small enum per output port (e.g., `i64`, `f64`, `ptr`, `unit`).
- Node arity is known by kind and/or payload (e.g., `TUP(n)`).

### 2.3 Effects (explicit)
- Node has `effects: [effectCID...]` in canonical form.
- In-memory: effects bitmask via a table mapping effectCID → bit.

### 2.4 Purity
- No effect CIDs → pure.
- Interpreter/JIT can run pure nodes in parallel; effect-conflicting nodes serialize.

---

## 3. Interfaces (Interface CIDs)

### 3.1 Interface object (`iface`)
Canonical list of exported names with their type signatures and effect sets.
Interfaces use the same compact, positional style:

```
[
  3,  // object tag for "iface"
  [
    [ "hello",
      ["unit"],              // parameter type atoms (temporary strings)
      ["unit"],              // result type atoms
      [ <effectCID>, ... ]   // effect CIDs (sorted; omit array when empty)
    ],
    ...
  ]
]
```

- Entries are sorted lexicographically by the export name.
- Effects lists are sorted lexicographically by CID. Elide the inner array when
  no effects are declared for an export.
- `ifaceCID = sha256(cbor)`; namespaces store this CID in their canonical
  payload for compatibility checks.

Tooling derives the interface directly from exported words: each word’s declared
type/effect metadata is lifted into the names array so the interface
automatically reflects capability requirements (e.g., `io`, `heap`). When type
definitions gain their own CIDs, swap the string atoms for those identifiers
without changing the array shape.


## 4. Canonical Encodings (CBOR)

Use DAG-CBOR or a fixed-byte canonical layout. Keys must be emitted in the exact order shown.

### 4.1 prim CBOR

Canonical encoding keeps the bytes minimal and machine-oriented. For primitives
we serialize a tiny positional array:

```
[
  0,                  // object tag for "prim"
  <rootCID>,          // reserved; 32 zero bytes for primitives
  ["i64","i64"],      // parameter type atoms (temporary strings until typedef CIDs exist)
  ["i64"],            // result type atoms
  [ <effectCID>, ... ]// declared effect CIDs (sorted; omit array when empty)
]
```

- Attribute maps and human-readable names live outside the CID (e.g. in
  `name_index` or debug views) and are **not** part of the canonical payload.
- Effect bitmasks remain a runtime convenience; they are derived from the
  declared effect CID list and deliberately excluded from the hashed encoding.

Once we have typedef CIDs, format will be:

```
[
  0,                                // object tag for "prim"
  <rootCID>,                        // still zero for primitives
  [ <typeCID>, <typeCID>, ... ],    // parameter type CIDs
  [ <typeCID>, ... ],               // result type CIDs
  [ <effectCID>, ... ]              // declared effect CIDs (sorted; omit array when empty)
]
```

### 4.2 node CBOR

Nodes represent individual Mini-INet agents. The canonical encoding mirrors the
other objects with a positional array:

```
[
  6,                      // object tag for "node"
  nk,                     // node-kind tag (u8 discriminator; e.g. 0=LIT,1=PRIM,...)
  [ [cid, port], ... ],   // inputs sorted by (port, cid); empty for RETURN
  [ <outType>, ... ],     // output type atoms (temporary strings until typedef CIDs exist)
  [ <effectCID>, ... ],   // declared effect CIDs (sorted; empty array when none)
  payload                 // kind-specific data (see below)
]
```

- Inputs reference producer node CIDs plus port indices; each entry is `[cid,
  port]` with `cid` as raw bytes and `port` as a small integer.
- Output types list every result port; include token or tuple types exactly as
  they surface to callers.
- Effects array is sorted lexicographically by CID and omitted (encoded as an
  empty CBOR array) when no effects are declared.

**Payload variants**

The final slot encodes node-specific data:

- `nk = LIT`: payload is the literal value (e.g., CBOR int/float or bytes).
- `nk = PRIM`: payload is `<primCID>`.
- `nk = CALL`: payload is `<wordCID>`.
- `nk = APPLY`: payload is `[ <qid>, <typeKey?> ]` where the second entry is
  omitted when no specialization key is provided.
- `nk = ARG`: payload is the zero-based argument index.
- `nk = LOAD_GLOBAL`: payload is `<globalCID>`.
- `nk = QUOTE`: payload is `<wordCID>` of the quoted quotation.
- `nk = IF`: payload is `[ <trueWordCID>, <falseWordCID> ]` (thunk form).
- `nk = TOKEN`, `nk = DEOPT`, etc.: payload is `null` (or empty array) when no
  extra data is required.
- `nk = RETURN`: payload is `[ vals[], deps[] ]`, where `vals` is the ordered
  list of `[cid, port]` feeding return values and `deps` is the sorted,
  deduplicated list of `[cid, port]` edges used to sequence side effects.

Return nodes keep their `inputs` array empty; all value and dependency edges
live in the payload to preserve the shared array shape. Consumers reconstruct
the execution graph by following these CIDs, enforcing determinism via the
sorted input/effect/dependency ordering.

When additional node kinds arrive (e.g., structured control), assign the next
`nk` tag and extend the payload conventions accordingly while preserving the
positional layout above.

### 4.3 word CBOR

Words reuse the same positional layout, with a real root CID referring to the
RETURN node of the graph:

```
[
  1,                  // object tag for "word"
  <rootNodeCID>,      // RETURN node CID
  ["i64","i64"],      // parameter type atoms (temporary strings until typedef CIDs exist)
  ["i64"],            // result type atoms
  [ <effectCID>, ... ]// declared effect CIDs (sorted; omit array when empty)
]
```

Human-friendly names, documentation strings, or registration aliases remain
outside the canonical payload (e.g. in `name_index`). When type definitions gain
their own CIDs, swap the string atoms for those identifiers without changing
the array shape.

### 4.4 namespace CBOR

Namespaces capture their interface CID, required import interfaces, and the
current name→word mapping for exported symbols:

```
[
  4,                        // object tag for "namespace"
  <ifaceCID>,               // this namespace's interface CID
  [ <importIfaceCID>, ... ],// sorted list of required interface CIDs
  [
    [ "hello", <wordCID> ], // exports sorted lexicographically by name
    [ "print", <wordCID> ],
    ...
  ]
]
```

- Namespace names live in `name_index` and remain outside the canonical payload.
- Import interface CIDs are sorted and deduplicated.
- Export entries are sorted by the textual name. Only exported words appear;
  internal words can still be registered directly in `name_index` for advanced
  use, but they do not affect the namespace CID.

### 4.5 program CBOR

Programs tie together the entry word and the canonical root namespace. The
canonical payload stays compact:

```
[
  5,                  // object tag for "program"
  <entryWordCID>,     // word invoked as the entry point
  <rootNamespaceCID>  // canonical namespace CID for the program's root
]
```

- Additional metadata (human-readable program name, author, etc.) lives in
  `name_index` or side channels.
- If you later want to capture exact dependency bindings (e.g., a lockfile of
  resolved namespace CIDs), append another slot with a sorted list
  `[ <namespaceCID>, ... ]`. Keep it optional so existing tooling can ignore it
  when not needed.

### 4.6 global CBOR

Globals may carry scalars or multi-result tuples. Canonical encoding mirrors the
other objects with a positional array:

```
[
  2,                  // object tag for "global"
  ["i64","i64"],      // result type atoms (temporary strings until typedef CIDs exist)
  [ 1, 2 ],           // value payload matching the type list
]
```

- The value slot is always an array whose length matches the type list. Use a
  single-element array for scalars to keep the representation uniform.
- Each entry may be an inline literal (e.g., CBOR int/float) or a CID pointing
  at a blob object when the payload is large. Future work can define a chunked
  Merkle layout for those blob CIDs; the global record itself still stores only
  the root reference.
- Namespaces/names remain in `name_index`; the canonical payload contains only
  types and values. When typedef CIDs land, replace the type atoms with those
  identifiers without changing the array shape.

## 5. SQLite Schema (single-file store)

```sql
-- Content-addressed objects (CBOR)
CREATE TABLE object (
  cid   BLOB PRIMARY KEY,   -- 32-byte SHA-256
  kind  TEXT NOT NULL,      -- "prim","node","word","iface","namespace","program","global","bundle"
  cbor  BLOB NOT NULL
) WITHOUT ROWID;

-- Mutable name -> CID pointers (the only mutability)
CREATE TABLE name_index (
  scope TEXT NOT NULL,      -- "namespace","program","iface","word","global"
  name  TEXT NOT NULL,
  cid   BLOB NOT NULL,
  PRIMARY KEY (scope, name)
);

-- Optional: compiled code cache (target-specific)
CREATE TABLE code_cache (
  subgraph_cid BLOB NOT NULL,
  arch   TEXT NOT NULL,     -- e.g., "x86_64"
  abi    TEXT NOT NULL,     -- e.g., "sysv"
  flags  INTEGER NOT NULL,  -- e.g., CPU features bitset, opt level
  blob   BLOB NOT NULL,
  PRIMARY KEY (subgraph_cid, arch, abi, flags)
);

CREATE INDEX object_kind_idx ON object(kind);
```

PRAGMA defaults

```
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA temp_store=MEMORY;
PRAGMA mmap_size=268435456;   -- tune
PRAGMA cache_size=-262144;    -- ~256MB
```

# 6. In-memory Representations (lean)

## 6.1 Node (executor-side)

```c
typedef enum { NK_LIT, NK_PRIM, NK_CALL, NK_ARG, NK_IF_CALL, NK_IF_CTL, NK_PHI, NK_TUP, NK_PROJ, NK_LOAD_GLOBAL } NodeKind;
typedef enum { TY_I64=1, TY_F64=2, TY_PTR=3, TY_UNIT=4 } TypeTag;

typedef struct {
  uint32_t effects_mask;     // mapped from effectCIDs
  uint8_t  kind;             // NodeKind
  uint8_t  type;             // TypeTag
  uint8_t  in_count;
  uint32_t in_idx[4];        // producer indices (fixed small max)
  union {
    int64_t lit;             // NK_LIT
    uint32_t prim_id;        // NK_PRIM (resolved from primCID)
    uint32_t word_id;        // NK_CALL, NK_IF_CALL true
    struct { uint32_t t_word_id, f_word_id; } if_call; // NK_IF_CALL
    uint32_t arg_index;      // NK_ARG
    uint32_t global_id;      // NK_LOAD_GLOBAL
  } u;
} NodeRAM;

typedef struct {
  NodeRAM *nodes;
  uint32_t nnodes;
  uint32_t root_idx;
} GraphRAM;
```

Keep a side table mapping CID → index/id for prim, word, global, effect.

All 32-byte CIDs live only in loader/persistence, not in hot exec structures.

## 7. Build/Save Workflow

1. Parse/Type-check source into typed stack effects.

2. Build mini-inet:

  * Maintain a stack of producer indices (wires), not values.
  * dup/swap/over become wiring; no nodes created for shuffles.
  * Create `NodeRAM` for `LIT`, `PRIM`, `CALL`, `LOAD_GLOBAL`, etc.

3. Hash-cons & persist:

* For each logical node, build *canonical CBOR* (`node`), `sha256 → nodeCID`.
* `INSERT OR IGNORE` into `object(cid,kind="node",cbor)`.
* Remember `nodeCID → index` mapping.

4. Create word:

* word = { root: rootCID, type_sig }; hash → wordCID; persist in object.
* Optionally add name_index(scope="word", name="<ns>/<sym>", cid=wordCID).

5. Create/Update namespace:

* Calculate iface from exports (symbol, type, effects) → ifaceCID.
* namespace = { imports(iface), exports(wordCID), iface: ifaceCID }; hash → nsCID; persist.
* name_index(scope="namespace", name="<ns>", cid=nsCID).

6. Create/Update program (optional):

* Resolve $main to wordCID; program = { entry_word: wordCID }; hash → progCID; persist.
* name_index(scope="program", name="$main", cid=progCID).

## 8. Resolution & Execution

### 8.1 Name resolution (*.hello)

* scope = current namespace plus its imports.
* Search exports in current namespace; else in imports (left-to-right).
* Return (namespace, symbol) → wordCID.

### 8.2 Interface check (optional strict)

* For each import {name, ifaceCID_req} in namespace:
* Load bound namespace’s ifaceCID from object.
* Require equality to ifaceCID_req (or allow supersets as policy).

### 8.3 Test gate (policy)

On import bind/update:

* Run imported namespace test suite.
* Run dependent namespace test suite.
* Run project tests.
* Update lockfile upon success.

### 8.4 Interpreter

* Build indegree (need[]) from in_count + effect deps.
* Push ready nodes; evaluate pure nodes in parallel (work stealing).
* For CALL/IF_CALL:

Resolve word_id → GraphRAM or compiled entry if present.

### 8.5 JIT / Code cache

* Identify pure straight-line subgraphs (supernodes).
* Compute subgraphCID from canonical encoding of the reachable set in topo order (names excluded).
* Lookup in code_cache via (subgraphCID, arch, abi, flags).
* If missing, emit native block (W^X: mmap RW → write → mprotect RX), store bytes in code_cache.
* Execute by calling entry pointer (ABI: e.g., SysV x86-64; pass args in registers).

## 9. Globals (immutable, namespaced)

* global object: {kind:"global", type, value|blobCID} → globalCID; persist in object.
* name_index(scope="global", name="<ns>/<name>", cid=globalCID).
* Graphs reference via LOAD_GLOBAL(globalCID) node; builder may constant-fold to LIT for scalars.
* Large globals: Merkle blobs (chunk trees) referenced by blobCID.

## 10. Context (runtime guards)

Context narrows which words apply at runtime (orthogonal to types which constrain at compile-time).

### 10.1 Representation

* context object: {kind:"context", "atoms":[ "<ctxCID_x>", "<ctxCID_y>" ] } → ctxCID.
* A word can declare requires: [ctxCID...] in metadata outside its CID (policy), or you can enforce via a guard node:

GUARD_CTX(required_ctxCID) node:

* Inputs: ctxCID_current.
* If satisfied, passes through its data input; otherwise signals a runtime miss (select next candidate).
* Canonical as a node with payload { "guard_ctx": "<ctxCID_req>" }.

### 10.2 Dispatch

* Overloaded name resolves to a set of candidate wordCIDs.
* Insert GUARD_CTX nodes at call sites, or resolve at link-time if context is known.
* Interpreter/JIT executes the first satisfied candidate.

(Start simple: make context checks an interpreter concern, then lift into nodes later.)

## 11. Lockfile

march.lock pins exact artifacts used:

```
{
  "toolchain": "<toolchainCID>",
  "entry": "<progCID or wordCID>",
  "namespaces": {
    "lang.march.helloworld.1": "<nsCID>",
    "io": "<nsCID_io>"
  },
  "words": {
    "lang.march.helloworld.1/hello": "<wordCID_hello>"
  },
  "ifaces": {
    "io": "<ifaceCID_io>",
    "lang.march.helloworld.1": "<ifaceCID_ns>"
  },
  "globals": {
    "math/const/tau": "<globalCID_tau>"
  }
}
```

## 12. Examples

### 12.1 Arithmetic: 4 5 + 9 -

Nodes (index order):

```
0: LIT(4)
1: LIT(5)
2: PRIM(add_i64) in=[0,1]
3: LIT(9)
4: PRIM(sub_i64) in=[2,3]   (root)
```

* Canonical node CBOR for each → nodeCIDs.
* word = {root: nodeCID(4), type: {params:[], results:["i64"]}} → wordCID.

### 12.2 Conditional (Forthy if): cond [t] [f] if

Thunk form (Option A):

```
tWordCID = word( ... )  // graph of true branch
fWordCID = word( ... )  // graph of false branch
condCID  = node(...)    // produces i64 truthy

ifNode = {
  nk: "IF_CALL",
  ty: <ty_of_branch>,
  in: [{cid: condCID, port:0}],
  pl: { "if": { "true": tWordCID, "false": fWordCID } }
} → nodeCID_if

word.root = nodeCID_if
```

Interpreter: evaluate cond; call only chosen thunk.
JIT: test/jcc + call (or inline if tiny).

## 13. API Outline (for Codex)

```c
// Persistence
int store_object(sqlite3* db, const uint8_t cid[32], const char* kind, const void* cbor, size_t n);
int load_object(sqlite3* db, const uint8_t cid[32], char* kind_out, void** cbor_out, size_t* n_out);
int name_put(sqlite3* db, const char* scope, const char* name, const uint8_t cid[32]);
int name_get(sqlite3* db, const char* scope, const char* name, uint8_t cid_out[32]);

// Canonical encoders (CBOR)
int cbor_encode_node(const NodeCanon* nc, uint8_t** out, size_t* n);
int cbor_encode_word(const WordCanon* wc, uint8_t** out, size_t* n);
int cbor_encode_iface(const IfaceCanon* ic, uint8_t** out, size_t* n);
int cbor_encode_namespace(const NsCanon* ns, uint8_t** out, size_t* n);
void sha256(const void* p, size_t n, uint8_t out[32]);

// Builder (stack → graph)
int graph_build_from_tokens(TokenStream*, GraphRAM* out_graph, BuildCtx* ctx);

// Interpreter
int64_t run_graph_i64(const GraphRAM* g, const int64_t* args, int nargs, ExecOptions* opt);

// JIT
void* codecache_get(sqlite3* db, const uint8_t subgraph_cid[32], const Target* tgt);
int   codecache_put(sqlite3* db, const uint8_t subgraph_cid[32], const Target* tgt, const void* blob, size_t n);
void* emit_native_block(const GraphRAM* g, int root_idx, const Target* tgt, EmitStats* st);

// Effects/Context
uint32_t effect_mask_from_cids(const uint8_t** effect_cids, int n);
bool     context_satisfies(const Ctx* have, const Ctx* need);
```

## 14. Security & Perf Notes

* W^X: mmap RW → write code → mprotect RX.=
* CET/IBT (x86-64): place endbr64 on indirect branch targets if required.
* Code layout: keep hot traces contiguous; prefer fallthrough; split cold paths.
* Parallel exec: pure nodes via work-stealing; serialize effect-conflicting nodes by mask.
* No names in CIDs: maximizes dedup; names only in name_index.

## 15. Milestones

* Storage core: object + name_index tables, CBOR encode/decode, SHA-256 CIDs.
* Mini-INet builder: LIT/PRIM/ARG/LOAD_GLOBAL; wiring-only shuffles.
* Interpreter: pure i64; linear scheduler → ready-queue (work-stealing later).
* Namespaces & Ifaces: exports/imports; iface calculation; resolver for *.name.
* Program object & lockfile: $main flow; test gates (shell out).
* JIT v0: straight-line pure subgraphs; code cache keyed by subgraphCID.
* IF_CALL (thunks) + inliner (optional IF_CTL+PHI).
* Globals: scalars folded; blob trees mapped.
* Context: basic runtime guard and overload selection.
* GC: reachability from all heads (name_index + lockfiles) → sweep unreachable objects and code blobs.

## 16. Defaults & Policies

* No per-word semver. Primitives may embed a compat field if needed; otherwise CIDs rule.
* Tests are the gate: import → run lib tests → run project tests → accept → update lockfile.
* Interface CIDs optional but recommended for faster compatibility checks.
* Names excluded from CIDs (namespace/word/global names live only in name_index).

17. Appendix — Example CBOR snippets (pseudo)

node(LIT 9, i64):

```json
{"kind":"node","nk":"LIT","ty":"i64","in":[],"eff":[],"pl":{"lit":9}}
```

node(PRIM sub_i64, in=[cidC, cidD]):

```json
{"kind":"node","nk":"PRIM","ty":"i64","in":[{"cid":"<cidC>","port":0},{"cid":"<cidD>","port":0}],
 "eff":[],"pl":{"prim":"<primCID_sub>"}}
```

word(root=cidE, ()→i64):

{"kind":"word","root":"<cidE>","type":{"params":[],"results":["i64"]}}

namespace(lang.march.helloworld.1):

{"kind":"namespace",
 "imports":[{"name":"io","iface":"<ifaceCID_io>"}],
 "exports":[{"name":"hello","word":"<wordCID_hello>"}],
 "iface":"<ifaceCID_ns>"}   // 'name' excluded from CID, stored in name_index

This is the complete plan. Implement the storage and builder first; you’ll be able to save a word, compute its CIDs, load it, interpret it, and later drop in the JIT without changing artifacts.

Do not assume there are no mistakes in this design. Voice concerns and ask about undecided design decisions when they arise.
