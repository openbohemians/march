# Arrays and the lifting rule

Status: strict arrays, built 2026-10-04 (Thomas: "you can do strict first"),
as persistent vectors from merkle-champ, our in-house persistent collections
crate. Laziness for
collections, arrays and maps alike, comes next on the same representation.
Apart from six primitives, everything is March code in `seed/system.march`.

## Literals

`( … )` gathers whatever its body leaves into an array:

```
( 1 2 3 )            -- three elements
( 1 2 + 3 4 * )      -- computed: 3 and 12
( ( 1 2 ) ( 3 ) )    -- arrays of arrays
( 5 [ i0 ] times )   -- 0 1 2 3 4: the body may loop
```

A literal gathers from the data stack, so it holds at most the stack's
65,536 cells; larger arrays are built by operations on arrays, such as
lifting, which append to their result directly.

`(` marks the data stack's depth and `)` gathers the cells above the mark,
both at run time, so loops and conditionals inside work (the lineage
research's "runtime depth marker"). Two primitives do it: `mark` (46) pushes
the depth onto the scratch stack, which is per call frame, so marks nest and
cannot leak; `gather` (47) pops it and moves the cells above it into a new
persistent vector. While compiling, `(` and `)` emit them; at top level,
`interpret` runs them itself, in its own frame (dictionary flag bits 5 and 6).

An array is one cell: a handle to a persistent vector of cells
(`merkle_champ::Vector`), which the machine keeps in a region
slot, so handles, generations and `region-free` work as for regions. The
vector is a 32-way trie of canonical shape with cached SHA-256 identities, the
companion of merkle-champ's map, in the same crate. Primitives: `vector-length` (48),
`vector-at` (49), `vector-push` (50, appends in place, for building) and
`vector-set` (51, a new version with one element replaced). Surface words:
`length ( a -- n )` and `at ( a i -- x )`; reading past the end traps with a
memory error. Byte access to a vector fails.

Regions stay what they were: the system track's mutable arrays of bytes and
cells (`region-new`, `@`, `!`, `c@`, `c!`).

## Types

Arrays have types 3 (of i64), 4 (of f64) and 5 (anything else, including
arrays of arrays and arrays mixing types), a byte like the number types
(docs/CHECKER.md). The checker models `mark` and `gather` exactly: it keeps the
depth at each open `mark`, so `gather` takes the difference in depth and types
the array from its elements. `at` is a family whose clauses give its result
the element type, so `( 1.5 2.5 ) 1 at 1.0 +` adds floats. At top level the
interpreter's type stack does the same.

**A count that varies is fine inside a literal.** `gather` takes whatever is
above its mark, so a loop or a branch that changes the depth inside `( … )`,
as in `( 5 [ i0 ] times )` or `( 1 2 3 flag [ drop ] if )`, does not stop the
checker: it marks the literal as varying, and `gather` still leaves exactly
one array, of type 5 since its elements' types are not tracked through the
variation. Outside a literal such a loop is still an error. This settles the
surface examples' finding F15 for literals; typing the elements of a varying
literal would need effects with a varying count, `[ i64 -> i64* ]`.

## The lifting rule

A family call with an array input and no clause of its own applies the
element operation to every element:

```
( 1 2 3 ) 1 +         -- ( 2 3 4 )
10 ( 1 2 3 ) -        -- ( 9 8 7 )
( 1 2 ) ( 10 20 ) +   -- ( 11 22 ); different lengths trap 25
( 1 5 ) 3 lt?         -- ( 1 0 )
( 1.5 2.5 ) 2.0 *     -- ( 3.0 5.0 )
( 1.5 2.5 ) 1 +       -- ( 2.5 3.5 ): the literal becomes 1.0
```

- **Resolved at compile time, like clauses.** The element operation is the
  family's clause for the element types, or the family itself for i64. The
  call is patched to a *lifted word*: one of three templates (array with
  value, value with array, two arrays) whose calls to a placeholder,
  `lift-op`, are pointed at the element operation. Lifted words are
  remembered by template and operation, so equal liftings share one word.
- **Lifted words are ordinary words.** They call the element operation
  directly, so the checker analyses them and checked mode accepts their
  callers. Their output type, an array of the element result's type, is
  recorded when they are made, since their own analysis sees only a region.
- **Lifting chains:** `( 1 2 3 ) 1 + 2 *` lifts twice, making an intermediate
  array. Fusing the two passes is what laziness will add.
- **Arrays of unknown element types (5) lift too,** with the i64 version for
  the elements, the same rule as for scalars of unknown type.
- **Generic words lift inside their instances:** `: double dup + ;` on
  `( 1.5 2.5 )` gets an instance whose `+` is lifted.
- **At top level too:** `( 1 2 3 ) 1 + 2 at` is 4.

## Memory

Every array takes a region slot, charged 8 bytes per element against the
machine's live-byte limit, and nothing frees them yet; memory management is
the 2.0 work. Versions share structure inside the vector, so an array made
with `vector-set` from another costs a path, not a copy.

## Next

- **Laziness:** lifted operations and ranges produce views that fuse and are
  computed when forced (indexing, folding, printing, storing), and maps get
  the same treatment.
- **Consumers:** `each`, `map` and `fold`, inlined like `times`.
- **Effects with a varying count,** so comprehensions that keep or drop
  elements check.
