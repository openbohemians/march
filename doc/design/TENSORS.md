# Persistent tensors

Status: design note, 2026-10-04. Nothing here is built. It responds to
Thomas's research note `gemini-code-1791143432610.md` ("Hybrid
Merkle-Hilbert Spatial Chunk Tensor") in this directory, and records what to
keep from it, what to change, and the order in which to get there.

## 1. Where March is today

March7's arrays (march7/docs/ARRAYS.md) are one-dimensional:

- **Representation.** An array is a handle to a `merkle_champ::Vector` of
  64-bit cells: a 32-way trie with a tail. Its shape depends only on its
  length. Each node caches its SHA-256 identity when first asked.
- **Types.** Element types are i64 or f64, or the array nests other arrays,
  which may be ragged: `( ( 1 2 ) ( 3 ) )`.
- **Operations.** Families lift over arrays all the way down, and `each`,
  `fold` and `map` consume them. Everything is strict. Laziness was agreed
  for all collections, but strict first.

In those terms, today's array is already a rank-1 tensor with 32-element
chunks.

## 2. The reference design in brief

Dense N-dimensional chunks, such as 32×32×32 floats (128 KB), sit at the
leaves, with their elements in Hilbert-curve order. Above them, a CHAMP trie
is keyed by the Hilbert index of each chunk's position. Every node carries a
BLAKE3 hash of its children. A point write copies one chunk and the path
above it. A slice returns the chunks it touches, with offsets, and copies
nothing. The Rust types in the note are a sketch: they do not compile
(`ARITY: DIMS: const usize usize`).

## 3. What we keep

- **A fixed global chunk grid.** Chunk boundaries depend only on the shape,
  so equal data always gives equal chunks and equal identities. This is the
  vector's rule ("canonical shape depends only on length") in N dimensions,
  and the most important property in the note.
- **Hashes computed when needed.** Equality checks and storage compute them;
  chains of arithmetic do not. The vector already works this way: a node
  computes its identity on first request and caches it.
- **Nodes not yet loaded,** referenced by identity and fetched on demand.
  merkle-champ's stored form (FORMAT.md section 9) already stores nodes as
  their identity preimages, so this falls out of persistence.
- **Slices as views** that share the chunks they cover. This is the lazy
  view already planned for arrays.

## 4. What we change, and why

1. **A dense trie, not CHAMP, for dense tensors.** CHAMP's bitmaps exist to
   skip absent children. In a dense tensor every chunk is present, so every
   bitmap is full and buys nothing. The vector's dense trie over the chunk
   index is simpler and faster.

   CHAMP is right for a **sparse** variant: a map from chunk index to chunk,
   where an absent chunk means all zeros (the fill value). That is a second
   representation behind the same interface, not the base.

2. **Row-major chunks, not Hilbert order.** The note's own guideline 4
   re-lays chunks out into linear slices before SIMD, which contradicts its
   claim of high SIMD use.
   - Elementwise operations, which are what lifting does, stream two chunks
     with the same layout in step, whatever that layout is.
   - Reductions along an axis, transposes, matrix products and axis slices
     want the last axis contiguous, which row-major gives and Hilbert order
     does not.
   - Hilbert indexing is costly per element. Morton order is cheap, but
     still breaks contiguity on every axis.

   The chunk grid already provides locality across axes at the coarse
   level. Order the chunks themselves row-major too, so the chunk index is
   computed like any other row-major offset.

3. **Smaller chunks, updated in place when not shared.**
   - With 128 KB chunks, every single-element write copies 128 KB. That
     suits bulk numeric work and is wrong for March's per-element updates.
   - Aim for 4–32 KB: for example 32×32 f32 is 4 KB, 64×64 f64 is 32 KB, and
     a rank-1 chunk of 4096 cells is 32 KB.
   - A chunk that nothing else references is updated in place
     (`Arc::make_mut`), as `vector-push` already does when building, so
     building and transient updates do not copy at all.
   - The exact size is a benchmark question, measured against the vector's
     32-cell leaves.

4. **An identity that covers what the tensor is.**
   - The note's branch hash leaves out the shape and the element type, so a
     2×3 and a 3×2 tensor with the same bytes get one identity.
   - Leaf and branch hashes need separating domain strings, as all of
     merkle-champ's do.
   - Keep SHA-256, like every other identity in the crate.

   A draft is in section 6.

5. **Rank decided at run time.** The note fixes rank at compile time
   (`const DIMS`). March needs it at run time: APL's rank polymorphism means
   one `+` for vectors, matrices and cubes. Shapes are short vectors of
   lengths.

6. **No global table of chunks by hash.** The note's O(1) deduplication
   needs one, which is a memory-management decision (deferred to 2.0).
   Deduplication by identity already happens when storing, in the codec's
   object set.

## 5. Plan, in stages

Each stage is useful on its own, and each is the ground the next stands on.

### Stage 1: lazy views over vectors

A view is a base vector, an offset, a stride and a length. Slicing, taking,
dropping and reversing return views and copy nothing. Lifted operations and
`map` over views produce *deferred* views (an operation over one or two
inputs), so `( … ) 1 + 2 *` makes one pass, not two.

A view is computed (forced) when indexed, folded, shown, stored, or asked
for its identity. A view's identity is its materialization's identity, so
equality never depends on how a value was produced. That is the same rule as
the vector's canonical shape. This stage is the "Laziness" item in
ARRAYS.md, for arrays first and maps after.

### Stage 2: shape

An array becomes a shape plus a row-major vector of cells: `reshape` is free,
and a matrix is a vector with shape `( 2 3 )`. Lifting follows APL's rules:
- equal shapes combine element by element;
- a scalar extends to any shape;
- leading-axis agreement (a vector against the rows of a matrix), as in J
  and BQN, needs a decision.

`at` takes an index per axis, and an array of indices selects (ARRAYS.md
"Next").

### Stage 3: strided views

Generalize stage 1's view to an offset, a stride per axis, and a shape. Then
transpose, broadcast, axis slices and diagonals are views too. Views stay
lazy, and force into canonical, row-major, unstrided data.

### Stage 4: chunked storage

When large numeric data needs it, a `tensor` module in merkle-champ, beside
`vector`:
- dense row-major chunks on the canonical grid;
- chunks indexed by the vector's trie over the chunk index;
- the sparse variant over CHAMP;
- element types narrower than a cell (f32, u8, i32).

The narrower element types need March types of their own. Stages 1–3 do not
depend on this one: they work the same over a plain vector, so the switch
from vectors to chunks is a change of storage, not of language.

## 6. Identity, a draft

Following merkle-champ's conventions (FORMAT.md): integers little-endian,
`||` concatenation, SHA-256, one domain string per node kind.

```
chunk  = SHA-256( "merkle-champ/tensor/chunk/v1"  || element bytes, row-major )
branch = SHA-256( "merkle-champ/tensor/branch/v1" || count (u8) || identity(child)… )
tensor = SHA-256( "merkle-champ/tensor/v1" || element type (u8) || rank (u8)
                  || shape (u64 each) || chunk shape (u64 each)
                  || identity(root) if any element )
```

- **Edge chunks are clipped, not padded.** A chunk at the far edge of an
  axis holds only the in-bounds elements, row-major in its clipped shape,
  so the identity cannot depend on padding values.
- **Chunk shape is a function of shape and element type,** fixed by the
  format version, so it is implied. It is written into the root anyway, to
  keep the root self-describing.
- **Elements are their bytes.** f64 uses March's canonical NaN, and -0.0 and
  0.0 stay distinct, as their bits are.
- **The sparse variant** gets its own domain, since its children are a map
  rather than a sequence.

## 7. Costs, for comparison

| Operation | Vector of cells (today) | Chunked tensor (stage 4) |
|---|---|---|
| Read one element | trie walk, log₃₂ n | trie walk over chunks, then an offset |
| Write one element, shared | copy one 32-cell leaf and the path | copy one chunk and the path |
| Write one element, unshared | in place | in place |
| Elementwise over equal shapes | per element through the trie | chunk by chunk, contiguous, vectorizable |
| Slice | copy (views: free) | free (a view of chunks) |
| Identity after one write | rehash the path (3 µs at 1M) | rehash one chunk and the path |

## 8. Questions for Thomas

1. **Nested and multidimensional.** Today's arrays of arrays are nested and
   may be ragged. Stage 2 adds rectangular arrays with a shape. APL2 and
   Dyalog keep both, with mix and split converting between them. My lean is
   both: shaped arrays as the numeric workhorse, nesting for ragged data.
   The type byte would then need to tell rank from nesting.
2. **Leading-axis agreement** in lifting: J and BQN have it, APL does not.
3. **Narrower element types** (f32, u8, i32): wanted in March itself, or
   only inside stored tensors?
4. **When to build stage 4.** My recommendation is not before stages 1–3,
   and not before a workload that needs it.
