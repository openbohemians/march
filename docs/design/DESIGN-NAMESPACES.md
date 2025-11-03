# DESIGN.md Part II -- March α₅: Namespaces and other requirements

## Namespaces (Canonical vs RAM)

Namespaces are static, compile-time constructs. They do **not** exist as runtime values.
They serve three purposes:
  1. Define which words the namespace exports.
  2. Declare which external interfaces it depends on (bindings).
  3. Provide ergonomic name resolution (via `use` aliases), which are non-semantic.

### Canonical Namespace Object (CID’d)

A namespace is stored in the object store as a canonical CBOR object:

  {
    "kind": "namespace",
    "bindings": [
      { "interface": "<interfaceCID_io>" },
      { "interface": "<interfaceCID_math>" }
    ],
    "exports": [
      { "name": "hello", "word": "<wordCID_hello>" },
      { "name": "goodbye", "word": "<wordCID_goodbye>" }
    ],
    "interface": "<interfaceCID_ns>"
  }

Rules:
  - `bindings` lists external interfaces required by this namespace.
    This is the real dependency list used for linking and compatibility checks.
    Sorted and deduplicated.
  - `exports` lists public symbols provided by this namespace.
    Sorted by `name` for determinism.
  - `interface` is this namespace’s own Interface CID (hash of its exports).

When capturing exports from tooling, pass them as `symbolName=<wordCID>` pairs so the
canonical array can be emitted deterministically while retaining the word linkage.

The canonical namespace CBOR **excludes**:
  - lexical `use` aliases,
  - human-readable namespace name,
  - documentation or metadata.

This guarantees content addressability: logical identity depends only on exported surface and bindings.

### Interface Object

Interfaces describe what a namespace promises to export:

  {
    "kind": "interface",
    "names": [
      { "name": "hello",   "type": { "params": [], "results": ["unit"] }, "effects": ["io"] },
      { "name": "goodbye", "type": { "params": ["str"], "results": ["unit"] }, "effects": ["io"] }
    ]
  }

Canonicalization rules:
  - `names` sorted lexicographically by `name`.
  - `effects` sorted.
  - Omit empty arrays.

Hashing this CBOR yields the Interface CID used in `bindings` and in the namespace `interface` field.

During compilation the interface can be synthesized automatically by inspecting
each exported word: lift its parameter/result types and accumulated effect set
into the `names` array so the contract always mirrors the actual graph semantics.

> **Terminology note:** entries are stored under `names` rather than `words`
> to avoid clashing with the canonical `word` objects themselves and to leave
> room for future exports that are not direct word entrypoints (e.g., globals or
> dict entries). Tooling should treat each `name` record as a reference to a
> concrete word CID.

### RAM View (Compiler-only)

For compilation, load the canonical namespace into a RAM structure:

  struct NamespaceRAM {
      cid: [u8; 32],
      symbol_map: BTreeMap<String, [u8; 32]>,
  }

This allows O(log N) name → wordCID lookups.

### Resolution Algorithm

Given a reference `R`:

1) If absolute (ns.path.symbol):
     resolve directly using the namespace index.
     If provider namespace != current namespace, add its `interface` CID to bindings.

2) Else if qualified (alias.symbol):
     look up alias in the AliasTable, expand to an absolute namespace, resolve, and add binding if external.

3) Else (unqualified symbol):
     a) check current namespace exports,
     b) check each `use ... as *` alias in declaration order.
     If resolved externally, add binding.

Ambiguous unqualified matches produce a compile-time error.
Shadowing: local exports outrank imported names.

### Lexical `use` Aliases

Aliases are compile-time only sugar and do not affect CIDs.

Example surface forms:

  use std.io as io;
  use foo as *;
  use lib.hash;

These populate an AliasTable used only by the resolver:

  struct Alias {
      alias: String,          // e.g., "io" or "" for open-import (*)
      target_ns: String       // absolute namespace, e.g. "std.io"
  }

Lexical aliases do not appear in canonical objects and do not affect hashing.

### Binding Import Collection

Every time a reference resolves to a symbol outside the current namespace, record the provider’s Interface CID in a set:

  bindings_set.insert(providerNamespace.interfaceCID);

After graph construction:
  bindings = sort(unique(bindings_set));

These bindings drive linker compatibility checks and lockfiles.

### Lockfile Integration

Build tooling records which provider namespaces were bound:

  "namespaces": {
    "io":   { "cid": "<nsCID_io>",   "interface": "<interfaceCID_io>" },
    "math": { "cid": "<nsCID_math>", "interface": "<interfaceCID_math>" }
  }

If a dependency’s implementation changes but the `interface` CID remains the same, the lockfile does not need updating.

### Why Namespaces Are Not Runtime Values

Reject patterns like:

  : io ( -- namespace ) std.io ;
  io.print

Reasons:
  - breaks determinism,
  - breaks reproducible hashing,
  - breaks static linking,
  - complicates optimization and JIT,
  - ruins tooling (jump-to-def, rename).

Namespaces are **compile-time** constructs only.

### Summary

Namespaces provide:
  - exported symbol lookup,
  - static interface identity,
  - binding import declarations via Interface CIDs,
  - compile-time aliasing (`use`) for readability.

They are stored canonically as:
  - bindings (external interfaces),
  - exports (names → wordCIDs),
  - interface (namespace’s own Interface CID).

Everything else is non-semantic sugar excluded from hashing.


## Canonical CBOR Key Order (Required)

For deterministic hashing, all canonical objects MUST encode keys in the exact order listed below.

### Node (already implemented)
kind, nk, ty, in, eff (omit if empty), pl

Field abbreviations keep the canonical encoding compact:
- `kind`: literal `"node"` discriminator shared by all node objects.
- `nk`: node kind tag (e.g., `LIT`, `PRIM`, `CALL`).
- `ty`: legacy single-result type atom (left temporarily for backward compatibility; superseded by `out`).
- `in`: ordered list of input edges (producer CID + port).
- `eff`: sorted effect CID list attached to the node; omitted when empty.
- `pl`: payload map specific to the node kind (e.g., literal value, prim CID).

### Interface
kind, names

### Namespace
kind, bindings, exports, interface

No extra fields may appear. Key order is fixed and MUST NOT be sorted alphabetically.

---

## Validation Rules (Required)

Canonical objects MUST satisfy:

### Namespace
- bindings MUST be a sorted, deduplicated array of objects `{ interface: <cid> }`.
- exports MUST be a sorted array of objects `{ name: <string>, word: <cid> }`, sorted lexicographically by name.
- interface MUST be a 32-byte CID computed from this namespace’s exported surface.
- No duplicate export `name` values allowed.
- A namespace MAY have zero exports (not an error).

### Interface
- names MUST be sorted lexicographically by name.
- effects arrays MUST be sorted.
- omit empty arrays.

### Node
- input list MUST be sorted by port, then by producer CID.
- effects array MUST be sorted.
- port indices MUST be unique within the node.

Violations MUST cause an immediate error.

---

## Lockfile (Recommended)

Build tooling writes a small lockfile recording which provider namespaces were chosen for each required interface:

{
  "namespaces": {
    "io": {
      "cid": "<nsCID_io>",
      "interface": "<interfaceCID_io>"
    },
    "math": {
      "cid": "<nsCID_math>",
      "interface": "<interfaceCID_math>"
    }
  }
}

If a dependency changes implementation but preserves the same interface CID, the lockfile does not require updating.

Lockfiles improve reproducibility and prevent silent provider substitution.

## Effects, Interfaces, Namespaces — Responsibilities & Boundaries

### What each thing is

- **Namespace** (compile-time, CID’d object)
  - Groups words (exports) and declares external **bindings** (required interfaces).
  - Canonical fields: `bindings`, `exports`, `interface`.
  - Human **names** and `use` aliases are non-canonical metadata.

- **Interface** (compile-time, CID’d object)
  - The **contract** a namespace promises: for each exported entry, its **name**, **type signature**, and **effect set**.
  - Canonical fields: `names` (sorted).
  - Hash of the canonical encoding = **interface CID**.

- **Effects** (semantic capability labels)
  - Describe what a word can do besides pure computation (e.g., `io`, `heap`, `clock`, `net`, `random`, `gpu`, `ffi`).
  - Appear:
    - in **interface** entries (each symbol’s `effects`),
    - in **node** objects (`effects` array) so schedulers/JIT can serialize side effects.

### Why they are separate

- Namespaces = organization & exports.  
- Interfaces = structural API identity (for compatibility & substitutability).  
- Effects = semantic constraints (purity, permissions, scheduling).

### Binding rule

A namespace’s `bindings` is the set of **interface CIDs** it requires.  
It is **not** a list of specific symbol→CID pairs. Symbol-level identity already lives inside graphs as `NodePayload::Word(wordCID)`.

### Source-to-store flow

1. Author writes code, optionally with `use` aliases (non-canonical).
2. Resolver maps references to exported **word CIDs** and notes external providers.
3. Builder constructs the namespace’s **interface** by listing each exported symbol: `(name, type, effects)`.
4. Namespace `bindings` = the set of **interface CIDs** of all external providers used.
5. Persist:
   - `interface` object → **interface CID**,
   - `namespace` object → **namespace CID**.

### Validation

- Interface `names` are sorted by `name`; each entry encodes:
  - `params` (type tags),
  - `results` (type tags),
  - `effects` (sorted list of effect IDs; see encoding below).
- Namespace:
  - `bindings` is a sorted, deduped list of `{ interface: <cid> }`.
  - `exports` is sorted by `name`.

---

## Interface Encoding — Canonical & Compact

The readable JSON you’ve seen is for docs. On the wire we use a **compact, deterministic CBOR** with short keys and arrays to avoid repetition.

### Type tags
Use a tiny enum (atoms), not freeform strings:
- `i64`, `f64`, `ptr`, `unit`, `str`, …

### Effects
Effects are identity-bearing. Use **effect CIDs** (32-byte) in canonical form (sorted).  
(If you later standardize a small core set, you can map well-known effect CIDs to 1-byte codes in encoding, without changing semantics.)

### Canonical key order
- Interface object keys in this exact order: `kind`, `names`.
- Name entries use arrays (no per-entry key repetition).

### Canonical interface (compact form)

Readable shape:
  kind = "interface"
  names = [
    // each entry is: [ name, params[], results[], effects[] ]
    [ "print", ["str"], ["unit"], [ <ioEffectCID> ] ],
    [ "read",  [],      ["str"],  [ <ioEffectCID> ] ]
  ]

CBOR-friendly shape (pseudocode):
  {
    "kind": "interface",
    "names": [
      [ text(name), array(param_type_atoms), array(result_type_atoms), array(effectCIDs) ],
      ...
    ]
  }

Notes:
- `names` sorted by `name`.
- `effects` sorted lexicographically by CID bytes.
- Omit nothing inside each entry (always 4 items) for stable layout.

### Example (human-readable, compact)

  {
    "kind": "interface",
    "names": [
      [ "print", ["str"], ["unit"], [ "EFFECT:IO:CID" ] ],
      [ "read",  [],      ["str"],  [ "EFFECT:IO:CID" ] ]
    ]
  }

Hash the CBOR bytes → **interface CID**.

This keeps identity precise, while the encoding is small and consistent.

---

## Why not a string-inflated map per entry?

Maps would repeat keys (`"name"`, `"type"`, `"effects"`) for every export entry.  
Array entries avoid that repetition, speed up parsing, and reduce hashing ambiguity.

---

## Practical loader tips

- Validate determinism: reject unsorted `names` or `effects`.
- Keep a tiny table mapping type atoms (`"i64"`, `"str"`, …) → `TypeTag` enum at load time.
- Effects in RAM can be a **bitmask** for scheduling, but the canonical object stores the **CIDs**. The bit positions should be derived from a stable, global mapping (e.g., ordered by effect CID).

---

## Future extensions (won’t break CIDs if added as new kinds)

- Add an optional, non-canonical `"doc"` field for documentation (ignored for hashing).
- Add an optional `"deprecations"` side table (non-canonical).
- Add `"names_ext"` with extra metadata; keep `"names"` as the canonical core.
