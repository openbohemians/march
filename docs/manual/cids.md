# Content-Addressed IR in March2

## Overview

March2 uses **content-addressed identifiers (CIDs)** to represent all code as immutable, verifiable, shareable data. Every piece of code—primitives, literals, quotations, and user-defined words—gets a unique CID based on its content.

This enables:
- **Deduplication**: Identical code shares the same CID
- **Verification**: Hash proves code integrity
- **Sharing**: Reference code by CID across network/database
- **Portability**: CIDs are platform-independent intermediate representation (IR)

## CID Structure

CIDs are 32-byte (256-bit) identifiers using SHA256 hashing with a bit-flag format:

```
Byte 0, Bit 7: 0 = content-addressed code, 1 = special
Byte 0, Bits 6-5: type tag (when bit 7 = 1)
  00 (0x80): primitive operation
  01 (0xA0): literal data
  10, 11: reserved

Bytes 1-31: Hash or assigned value
```

### Three Types of CIDs

**1. Content-Addressed Code (bit 7 = 0)**
- Hash of CID sequence
- Used for user-defined words and quotations
- Example: `: add2 2 + ;` → SHA256([CID_2, CID_plus])

**2. Primitive Operations (0x80)**
- Assigned IDs for built-in operations
- Examples: Add=1, Sub=2, Mul=3, Dup=5
- CID format: `[0x80, 0x00, ID_high, ID_low, 0x00...]`

**3. Literal Data (0xA0)**
- Hash of MessagePack-serialized value
- Numbers, strings, quotations stored this way
- Example: `42` → serialize with MessagePack → SHA256 → set tag 0xA0

## Compilation Flow

### Word Compilation

When you define a word, March2 compiles it to both XTs (for execution) and CIDs (for storage):

```forth
SIGNATURE. i64 -> i64 ;
: add2 2 + ;
```

**Compilation steps:**
1. Literal `2` → serialize to MessagePack → hash → `CID_lit`
2. Word `+` → lookup primitive ID (3) → `CID_prim(3)`
3. CID sequence: `[CID_lit, CID_prim(3)]`
4. Word CID: `SHA256([CID_lit, CID_prim(3)])`
5. Store: `("", "add2") → Word_CID` in dictionary

### Quotation Compilation

Quotations are compiled like words but without names:

```forth
( dup * )
```

**Compilation steps:**
1. Start quotation → push new context
2. `dup` → `CID_prim(5)`
3. `*` → `CID_prim(3)`
4. End quotation → pop context
5. Quotation CID: `SHA256([CID_prim(5), CID_prim(3)])`
6. Wrap as literal: serialize quotation CID → hash → literal CID

### Nested Quotations

Nested quotations compile recursively:

```forth
( ( 5 . ) call )
```

**Compilation:**
1. Outer `(` → push context
2. Inner `(` → push nested context
3. Compile `5 .` into nested context
4. Inner `)` → create inner quotation CID, add as literal to outer context
5. Compile `call` into outer context
6. Outer `)` → create outer quotation CID

## Primitive ID Assignments

Built-in operations have assigned IDs:

| ID | Operation | CID |
|----|-----------|-----|
| 1  | Add (+)   | 0x8000000100...00 |
| 2  | Sub (-)   | 0x8000000200...00 |
| 3  | Mul (*)   | 0x8000000300...00 |
| 4  | Div (/)   | 0x8000000400...00 |
| 5  | Dup       | 0x8000000500...00 |
| 6  | Drop      | 0x8000000600...00 |
| 7  | Swap      | 0x8000000700...00 |
| 8  | Over      | 0x8000000800...00 |
| 9  | Rot       | 0x8000000900...00 |
| 10 | Dot (.)   | 0x8000000A00...00 |
| 11 | Lt (<)    | 0x8000000B00...00 |
| 12 | Gt (>)    | 0x8000000C00...00 |
| 13 | Lte (<=)  | 0x8000000D00...00 |
| 14 | Gte (>=)  | 0x8000000E00...00 |
| 15 | Eq (=)    | 0x8000000F00...00 |
| 16 | Neq (!=)  | 0x8000001000...00 |
| 17 | ToR (>r)  | 0x8000001100...00 |
| 18 | FromR (r>)| 0x8000001200...00 |
| 19 | RFetch (r@)| 0x8000001300...00 |
| 20 | Call      | 0x8000001400...00 |
| 21 | If        | 0x8000001500...00 |
| 22 | Iff       | 0x8000001600...00 |
| 23 | Type      | 0x8000001700...00 |

## Serialization

Literals are serialized using **MessagePack**, a compact binary format with cross-platform support.

### Supported Types

**Numbers:**
```forth
42  →  MessagePack: [0xD2, 0x00, 0x00, 0x00, 0x2A]  →  SHA256  →  CID
```

**Strings:**
```forth
"hello"  →  MessagePack: [0xA5, 0x68, 0x65, 0x6C, 0x6C, 0x6F]  →  SHA256  →  CID
```

**Quotations:**
```forth
( dup * )  →  Compile to CID sequence first
           →  Store quotation CID
           →  Serialize as SerializableValue::Quotation(CID)
           →  SHA256  →  Literal CID
```

### SerializableValue Format

```rust
pub enum SerializableValue {
    Number(i64),
    String(String),
    Type(SerializableType),
    Quotation(CID),           // Quotation stored as its CID
    Array(Vec<SerializableValue>),
    Map(Vec<(String, SerializableValue)>),
}
```

## Database Schema (Planned)

### CID Table
```sql
CREATE TABLE cids (
    cid BLOB PRIMARY KEY,           -- 32-byte CID
    content_type TEXT NOT NULL,     -- "primitive" | "literal" | "sequence"
    data BLOB NOT NULL              -- MessagePack serialized data
);
```

### Words Table
```sql
CREATE TABLE words (
    namespace TEXT NOT NULL,
    name TEXT NOT NULL,
    cid BLOB NOT NULL,              -- Foreign key to cids.cid
    signature TEXT,                 -- Optional type signature
    immediate BOOLEAN NOT NULL,
    PRIMARY KEY (namespace, name),
    FOREIGN KEY (cid) REFERENCES cids(cid)
);
```

### Benefits

1. **Deduplication**: Multiple words with same implementation share one CID entry
2. **Integrity**: Can verify code hasn't been tampered with by recomputing hash
3. **Sharing**: Export/import namespaces by CID references
4. **Caching**: Load frequently-used CIDs into memory
5. **Versioning**: Same word name can have different CIDs over time

## Example: Content Sharing

Two different words with identical implementations:

```forth
NAMESPACE. math ;
: double dup + ;

NAMESPACE. util ;
: twice dup + ;
```

Both compile to the same CID sequence: `[CID_dup, CID_plus]`

In the database:
```
CID: 0x7A4B...   → Sequence: [CID_prim(5), CID_prim(1)]

Words:
  (math, double)  → 0x7A4B...
  (util, twice)   → 0x7A4B...
```

Only one copy of the actual code exists in the CID table!

## Architecture Layers

```
┌─────────────────────────────────────┐
│  Source Code (FORTH text)           │
└─────────────────┬───────────────────┘
                  │ Compile
                  ↓
┌─────────────────────────────────────┐
│  CID Sequences (content-addressed)  │  ← Shareable, verifiable IR
│  Stored in database                 │
└─────────────────┬───────────────────┘
                  │ Load/Resolve
                  ↓
┌─────────────────────────────────────┐
│  XT Sequences (execution tokens)    │  ← Runtime, platform-specific
│  In-memory, ready to execute        │
└─────────────────────────────────────┘
```

**Key insight:** CIDs are the portable intermediate representation. XTs are the runtime execution format. The same CID can resolve to different XTs on different platforms.

## Future: AOT Compilation

```
Source  →  CID sequences  →  Native code (by platform)
                ↓
            Database
                ↓
        (CID → x86_64 ASM)
        (CID → ARM64 ASM)
        (CID → WASM)
```

Each CID could have multiple compiled representations for different platforms, all verified by the same content hash.

## ColorForth Inspiration

This design is inspired by **ColorForth's** block-based storage model, where code is stored in a database-like structure rather than text files. March2 extends this with cryptographic content addressing for:
- Distributed code sharing
- Cryptographic verification
- Automatic deduplication
- Platform portability

## Implementation Files

- `src/cid.rs` - CID struct, hashing, type checking
- `src/serializable.rs` - MessagePack serialization for literals
- `src/xt.rs` - Primitive ID mappings, XT ↔ CID conversion
- `src/word.rs` - Word metadata includes optional CID field
- `src/forth.rs` - Compilation generates both XT and CID sequences

## Performance Considerations

**Hash computation:** SHA256 is fast (~100s of MB/s), so hashing during compilation is negligible.

**Lookup overhead:**
- Primitive CIDs: O(1) - direct ID lookup
- Word CIDs: O(1) - HashMap lookup in namespace
- Database CIDs: O(1) with proper indexing on CID column

**Memory:**
- Each CID: 32 bytes
- Typical word: 5-10 CIDs = 160-320 bytes
- Cache frequently-used CIDs in memory

**Trade-off:** Slight compilation overhead (hashing) for major benefits (sharing, verification, deduplication).
