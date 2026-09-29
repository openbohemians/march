# Parametric words at chunk grain: a draft of the core array families

Author: Claude, at Thomas's request, 2026-09-25. Status: design draft for
discussion, not a specification and not implemented. It assumes the current
engine (definitions with canonical CIDs, guarded families, lazy shared demand
cells, explicit boundary collection) and proposes what a columnar array value
and its core vocabulary would look like on top of it. Where a choice is open
it is marked as such.

## 1. The value model

**Element.** A scalar of a fixed dtype: `i64`, `bool`, later `f64`, `text`,
`quote`. Dtype is part of an array's identity.

**Chunk.** A contiguous immutable block of elements of one dtype, bounded in
size (a fixed maximum such as 64 Ki elements or a byte budget), held in native
storage, with a content identity `chunk-cid = H(dtype, length, bytes)`.
Chunks are a representation unit. No word in the vocabulary observes chunk
boundaries; they exist so that demand, sharing, caching, collection and
distribution pay per chunk instead of per element.

**Array.** A value with a dtype, a rank, a shape, and a columnar body. (The
APL word; "array" is reserved for machine-learning-facing documents and is
not the type name.) In this
draft rank 1 (a column) is primary; rank 2 is a shape plus a row-major chunk
list; higher ranks are a later extension of the same rule. Two kinds of length:

- *bounded*: the row count is known metadata; the body is a finite list of
  chunk references;
- *unbounded*: the row count is unknown; the body is a lazy stream of chunks,
  today's lazy pair at chunk grain. This is the streaming case.

Identity: `array-cid = H(dtype, shape, [chunk-cid...])`, a Merkle identity.
An unbounded array has no array CID until it is bounded; its chunks do.

**Table.** FORTH has no records and neither does this draft. A table is a set
of named columns of equal length. Structural words (`group`, `join`, `sort`)
consume key columns and produce *index arrays* (permutations, offsets, index
pairs); other columns are then rearranged with `gather`. Rows are not
materialized by the columnar words. This is the kdb+/q discipline and it is
what makes a columnar engine fast; it also keeps the vocabulary small.

**Tuple.** March does have tuples: a fixed-arity, heterogeneous, immutable
value (the lazy pair generalized to n fields). A tuple is the row form. A
rank-one array counts as a tuple wherever a tuple is expected: it is a tuple
whose fields happen to share one dtype, so tuple words accept it unchanged and
array words get the more capable form. A mixed tuple is never an array. A
table can be transposed into an array of tuples with `rows` and back with
`columns`, so mixed-dtype data can be handled row-wise when that is the
natural shape of a problem (per-row logic in `map`, small result sets,
interoperation). The cost is explicit: an array of tuples is one cell per
row and has no kernels, so it is orders of magnitude slower than columns and
is never the representation the structural words work on. Transposition is
lazy at chunk grain: `rows` produces tuples chunk by chunk, so a pipeline
that ends in `take 20 rows` builds twenty tuples, not the table.

**What the executor sees.** An array is a value like a pair is today: a head
of metadata (dtype, shape) plus references to chunk cells. Each chunk cell is
an ordinary demand cell: pending, evaluating, ready, or failed. A kernel is an
`Op::Kernel`-style native operation whose arguments and result are chunks.
Everything the engine already guarantees, one evaluation per shared instance,
dormant unselected work, failure isolation per cell, boundary collection with
the live window as root set, applies unchanged with the chunk as the unit.

## 1a. Literal syntax (decided with Thomas, 2026-09-25)

- `[ ... ]` is a quotation, as in the stream seed today.
- `( ... )` is the array/tuple literal. `(` is an ordinary stream-consuming
  word: it reads to the matching `)`, evaluates the body at construction time
  (the same mechanism a defining word such as `answer:` uses), and inspects
  the results. If every element has one dtype, and nested elements one shape,
  the literal is an array of that dtype and rank; otherwise it is a tuple.
  Array when it can be, tuple when it must be. So `( 1 2 )` is an `i64`
  array, `( 1 "george" )` is a tuple, `( 1 1 + 3 )` is the array `( 2 3 )`,
  `( ( 1 2 ) ( 3 4 ) )` is a rank-two array, and `( ( 1 2 ) ( 3 4 5 ) )` is
  a tuple of two arrays.
- `( 1 )` is a one-element array, not a one-tuple. `( )` is the empty array
  with an open dtype that is fixed by first use, like an unbound hole; it is
  the natural seed for `concat` and `reduce`.
- Elements must be ground at construction time; an array whose contents
  arrive at run time is built by `columns`, `take`, or a reader, not by a
  literal.
- `{ ... }` is reserved for maps; `< ... >` is held open (Thomas keeps `<`
  and `>` free for an angle-bracket form; comparisons are `lt?` etc.).
- FORTH's `( n -- n )` stack-effect comment therefore no longer parses as a
  comment; `--` and `\` remain the comment words. This is a deliberate break.
- There is no tuple literal word; mixed content in parentheses is the tuple
  literal. A stack-built constructor (like `pair`) exists for definitions that
  assemble rows dynamically.

All of these are seed-dictionary words, so the assignment is cheap to revise;
the engine does not know any bracket.

## 2. Guards are on metadata, never on data

A family clause is selected by pure guards. For arrays the guards read
dtype, rank, and shape, which are metadata available without forcing any
chunk. This is the rule that keeps selection cheap and keeps "demand only what
selection needs" true at scale:

- `scalar?`, `array?`, `rank1?`, `dtype=?` read metadata only.
- `same-shape?` compares shapes; for two bounded arrays this is metadata. For
  an unbounded operand it is *unknown*, and the clause for unbounded operands
  checks agreement lazily, chunk by chunk, reporting a length mismatch as an
  error at the first chunk that disagrees. That error is the same value-level
  fault as an overflow: it belongs to the chunk cell that hit it.

Shape agreement rule adopted in this draft (open choice, my recommendation):
**strict equal shape, plus scalar extension**. Leading-axis agreement (APL) or
trailing-axis broadcasting (NumPy) can be added later as further clauses with
their own guards; nothing below depends on them.

## 3. The arithmetic and comparison family

Written in the current family syntax: `family name inputs outputs guard body
... ;`, first true guard wins. Guard and body names are illustrative.

```forth
-- Guards (pure; metadata only).
: both-scalar?      ( a b -- bool )  ...
: array-scalar?    ( a b -- bool )  ...    -- array on the left, scalar right
: scalar-array?    ( a b -- bool )  ...
: same-shape?       ( a b -- bool )  ...    -- both bounded, equal dtype and shape
: streamable?       ( a b -- bool )  ...    -- equal dtype, at least one unbounded
: always            ( a b -- bool )  drop drop true ;

-- Bodies.
: add-scalars       ( a b -- c )     +.native ;
: add-extend-right  ( t s -- t' )    [ s + ] map-chunks ;      -- s captured by staging, see §6
: add-extend-left   ( s t -- t' )    swap add-extend-right ;
: add-zip           ( t u -- v )     ' +.chunk zip-chunks ;    -- kernel over aligned chunk pairs
: add-stream        ( t u -- v )     ' +.chunk zip-stream ;    -- lazy, checks lengths as it goes
: add-mismatch      ( a b -- )       shape-mismatch ;          -- explicit failing clause

family + 2 1
  both-scalar?   add-scalars
  array-scalar? add-extend-right
  scalar-array? add-extend-left
  same-shape?    add-zip
  streamable?    add-stream
  always         add-mismatch ;
```

`-`, `*`, and the comparisons `eq?`, `lt?`, `lte?`, `gt?`, `gte?` are the same
family shape with their own kernels; the comparisons produce a `bool` array.
Elementwise `eq?` is total like scalar `eq?`: a dtype mismatch between two
arrays is a clause-selection result (all `false`), not a type error. The explicit last clause turns a shape mismatch
into a clause-selection error with a message rather than a kernel panic.

What the two chunk bodies do:

- `zip-chunks` walks the two chunk lists in step and creates one *pending*
  output chunk cell per aligned pair, keyed by `(kernel-cid, left-chunk-cid,
  right-chunk-cid, context-cid)`. Nothing runs until a chunk is demanded.
- `zip-stream` is the same over lazy chunk streams: the output is an unbounded
  array whose next chunk is a pending cell holding the two input streams'
  next-chunk cells.

Consequence for `[ 1 2 3 ] [ 1 2 3 ] +`: both bounded, same shape, one chunk
each, one pending output chunk; observing the result runs the kernel once.

## 4. Structural words

Each is a family over the same guard vocabulary. Signatures in stack order.

**map** `( t q -- t' )` applies a closed quotation elementwise. Clauses:
scalar body applied per chunk through a generic per-element loop (slow but
correct); a body that lowers to a known elementwise kernel sequence (`+`, `*`
with scalar operands, comparisons) fused into one kernel by staging (§6).
Output dtype is the body's output dtype, inferred at compile time when the
body is closed; the guard rejects a body with the wrong stack effect.

**filter** `( t p -- t' )` with `p` a predicate quotation. Body: `map` to a
`bool` mask (chunk-grained, lazy), then `compress`, which produces a bounded
array from a bounded one or an unbounded array from an unbounded one. The
mask is a value and may be shared by several consumers.

**reduce** `( t q z -- s )` folds with a binary quotation and a seed; APL's
`+/`. Clauses: known associative kernels (`+`, `*`, `max`, `min`, `and`,
`or`) reduce per chunk then combine the partials, so the reduction over a
bounded array is one pending cell per chunk plus one combine cell, and it
parallelizes and distributes by chunk without any further mechanism. A
general quotation gets the sequential clause.

**scan** `( t q z -- t' )` is the prefix version; same clause split.

**take / drop** `( t n -- t' )`: metadata operations on bounded arrays
(select a prefix or suffix of the chunk list, slicing at most two chunks);
on an unbounded array `take` bounds it and is the ordinary way to make a
stream finite. `take 10` on the end of a long elementwise pipeline demands
exactly one chunk from every stage.

**gather** `( t ix -- t' )` selects rows by an index array. This is the
universal rearrangement: sorting, grouping and joining all produce index
arrays and apply them with `gather`.

**sort** `( k -- ix )` returns a permutation of a key column, not the sorted
column; `k ix gather` is the sorted column and `v ix gather` reorders a
sibling column identically. Clauses by dtype and by bounded/unbounded (an
unbounded sort is an error clause; sort needs the whole key).

**group** `( k -- keys offsets ix )` returns the distinct keys, the group
boundary offsets, and the permutation that brings equal keys together. A
per-group aggregate is `v ix gather offsets segmented-reduce`. No record
values anywhere.

**join** `( ka kb -- ia ib )` returns aligned index arrays for matching
rows; the joined columns are `va ia gather` and `vb ib gather`. Clauses: hash
join for bounded operands; a streaming clause for one unbounded side against
a bounded build side; an error clause for two unbounded sides.

**window** `( t n -- t' )` sliding or tumbling windows over an unbounded
array; each window is a bounded array chunk; the natural streaming
aggregate is `window reduce`.

**concat / reshape / columns**: metadata operations; `concat` appends chunk
lists (rechunking only at the seam), `reshape` reinterprets shape over the
same chunks, `columns` packs several equal-length arrays as a table.

**rows / columns** `( table -- tuples )` and `( tuples -- table )`: the
transposition between the columnar form and the tuple form. `rows` is lazy
per chunk; `columns` on an array of tuples must scan the tuples to rebuild
each column and is the expensive direction. Both are ordinary families whose
clauses are selected by the field dtypes, so a table whose fields are all one
dtype can transpose through a kernel while a mixed one goes through the
generic per-row clause.

## 5. Sharing and identity at chunk grain

Every pending chunk cell carries the key `(word-cid, input-chunk-cids,
context-cid)`. Two consumers of the same array share its cells as today. Two
*independently constructed* expressions that reach the same key are the L3
case, which the scalar engine deliberately does not merge. At chunk grain the
recommendation flips: merge by key, because hashing a handful of CIDs is
negligible against a kernel over 64 Ki elements, and because this merge is
exactly what a dataset cache is. The same key is the lineage of a result:
which code, which inputs, which context. A result's identity is
reproducible by anyone holding the same image and the same input chunks.

Unbounded arrays carry no array CID, only chunk CIDs, so a streaming
pipeline caches per chunk and a batch pipeline caches per array; nothing
else distinguishes them.

## 6. Fusion is staging, not an optimizer

`a b + c *` at chunk grain is two kernel passes per chunk. The array
languages that are fast fuse elementwise chains into one pass. In March that
is a compile-time specialization: the composed elementwise words, applied to
operands whose dtype and shape are known when the definition is built, lower
to one kernel. The bootstrap Codex is building is what makes this expressible
in March rather than in Rust: a defining word can inspect the pending
elementwise sequence and emit the fused kernel call. Until then, the
unfused clause is correct and merely slower.

`add-extend-right` above writes `[ s + ] map-chunks` with `s` taken from the
stack; March has no closures, so the actual mechanism is the same one
`answer:` uses today: the defining word emits the scalar as a literal into the
kernel call it constructs. No captured environment exists.

## 7. What must exist before any of this

1. A dtype-carrying chunk value with native storage and a content identity,
   and a `Datum` variant for it; `content_id` extended to hash arrays as
   `H(dtype, shape, chunk-cids)`.
2. Kernels as native operations over chunks for the arithmetic and
   comparison family, `compress`, `gather`, `segmented-reduce`, hash-build and
   probe, and sort, one per dtype, with a fixed chunk size.
3. Metadata guards (`shape`, `dtype`, `rank`, `bounded?`) as pure words.
4. Chunk-grained lazy sequences: the existing lazy pair, with chunks as heads.
5. The chunk cell key and the merge-by-key policy from §5, which is the one
   place where a memo keyed by canonical identity is worth its cost.
6. The `(` literal word from section 1a, built on the stream nucleus.

Effects, I/O, and distribution are not in this draft; they enter at the
boundary where chunks are read from or written to a store by content
identity, which is the same boundary images already use.

## 8. Open choices, listed so they are decided rather than defaulted

- Shape agreement: strict + scalar extension (this draft) versus leading-axis
  versus trailing-axis broadcasting.
- Chunk size: a fixed element count per dtype, or a byte budget.
- Whether a `bool` mask array is a bitmap or a byte column (bitmap halves
  memory; byte column simplifies kernels).
- Missing values: no `null` in this draft; a table with optional fields would
  carry a validity `bool` column beside the value column, kept explicit.
