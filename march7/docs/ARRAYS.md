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
the depth onto the mark stack, which is per call frame like the scratch stack,
so marks nest and cannot leak; `gather` (47) pops it and moves the cells above
it into a new persistent vector. Marks were first kept on the scratch stack,
where a literal inside a loop body, `3 [ ( i0 i0 ) ] times`, hid the loop's
index from `i0`. While compiling, `(` and `)` emit them; at top level,
`interpret` runs them itself, in its own frame (dictionary flag bits 5 and 6).

An array is one cell: a handle to a persistent vector of cells
(`merkle_champ::Vector`), which the machine keeps in a region
slot, so handles, generations and `region-free` work as for regions. The
vector is a 32-way trie of canonical shape with cached SHA-256 identities, the
companion of merkle-champ's map, in the same crate. Primitives: `vector-length` (48),
`vector-at` (49), `vector-push` (50, appends in place, for building) and
`vector-set` (51, a new version with one element replaced). Surface words:
`length ( a -- n )` and `at ( a i -- x )`, which compile to primitives 48
and 49; reading past the end traps with a memory error. Byte access to a
vector fails.

Regions stay what they were: the system track's mutable arrays of bytes and
cells (`region-new`, `@`, `!`, `c@`, `c!`).

## Types

An array's type is a byte like the number types (docs/CHECKER.md), by element
type and rank:

| Type | Array of |
|---|---|
| 3, 4, 5 | i64, f64, elements of unknown type |
| 6, 7, 8 | arrays of i64, of f64, of unknown: each rank adds 3 |
| 252 | elements whose known types differ, as in `( 1 2.5 )` |
| 254 | nothing yet: the empty array `( )` |

The checker models `mark` and `gather` exactly: it keeps the depth at each
open `mark`, so `gather` knows how many cells it takes, and types the array
from its elements. `at` gives its result the element type, so
`( 1.5 2.5 ) 1 at 1.0 +` adds floats; for 252 the element's type is unknown.
At top level the interpreter's type stack does the same.

**A count that varies is fine inside a literal.** `gather` takes whatever is
above its mark, so a loop or a branch that changes the depth inside `( … )`,
as in `( 5 [ i0 ] times )` or `( 1 2 3 flag [ drop ] if )`, does not stop the
checker: it marks the literal as varying, and `gather` still leaves exactly
one array. Wherever paths meet at different depths inside the literal, the
checker adds both paths' values above the mark to a summary of the elements
(kept in the mark), so the array's type covers every element either path
leaves: both examples are arrays of i64, and `( 1 2 3.5 flag [ drop ] if )`
is 252. Outside a literal such a loop is still an error. This settles the
surface examples' finding F15 for literals.

**Types on the scratch stack.** The checker tracks the types of the first
eight scratch slots too, so `i0` is an i64, and `each` and `map`, which keep
their array there, know its elements' type.

**Words that build arrays from their inputs are generic.** In
`: pair >r >r ( r> r> ) ;` the elements are the inputs, whose types are
unknown, so `pair` is generic: `( 1 2 ) ( 3 4 ) pair` gets an instance whose
result is an array of arrays of i64 (6), and `1.5 2.5 pair` one whose result
is an array of f64.

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
- **Lifting goes all the way down,** as APL's scalar functions do: when the
  elements are arrays, the element operation is itself a lifted word, so
  `( ( 1 2 ) ( 3 ) ) 10 +` is `( ( 11 12 ) ( 13 ) )` and
  `( ( 1 2 ) ( 3 4 ) ) ( 10 20 ) +` is `( ( 11 12 ) ( 23 24 ) )`. To work at
  one level instead, use `map`.
- **Arrays of unknown element types (5) lift too,** with the i64 version for
  the elements, the same rule as for scalars of unknown type. **Arrays whose
  elements' known types differ (252) do not:** the i64 version would be wrong
  for some of them, so `( 1 2.5 ) 1 +` is an error (trap 23).
- **Generic words lift inside their instances:** `: double dup + ;` on
  `( 1.5 2.5 )` gets an instance whose `+` is lifted.
- **At top level too:** `( 1 2 3 ) 1 + 2 at` is 4.

## Consuming arrays: `each`, `fold` and `map`

Lifting covers a family applied to every element. The consumers cover the
rest:

```
( 1 2 3 4 ) 0 [ + ] fold                      -- 10
0 ( 1 2 3 ) [ + ] each                        -- 6: fold is swap, then each
( 0 5 - 7 ) [ dup 0 lt? [ negate ] if ] map   -- ( 5 7 ): a branch per element
( ( 1 2 ) ( 3 ) ) [ length ] map              -- ( 2 1 ): the level chosen
10 ( 1 2 3 ) [ over + ] map                   -- 10 ( 11 12 13 )
( 10 20 30 ) [ i0 + ] map                     -- ( 10 21 32 ): i0 is the index
( xs [ dup 3 lt? [ drop ] if ] each )         -- keep the elements of 3 or more
```

- **`a [ body ] each`** runs body on each element, on top of the stack, in
  order. Outside a literal body must consume the element (net effect -1), as
  any loop body must leave the depth unchanged; inside a literal it may leave
  any number of values, so `( xs [ … ] each )` is a comprehension that keeps,
  drops or repeats elements.
- **`a x [ body ] fold`** runs body on an accumulator, x at first, and each
  element: `swap` then `each`.
- **`a [ body ] map`** builds the array of body's results with `vector-push`,
  not on the data stack, so it has no length limit. Body must leave one value
  in place of each element; checked mode rejects one that does not.
- **Consumers, like `times`.** They inline the pending quotation, so the
  checker sees ordinary code: the element has the array's element type, and
  families in the body resolve on it (`( 1.5 2.5 ) [ 1 + ] map` adds 1.0). The
  body sees the values beneath the array, and `i0` is the element's index.
- **Typed results.** `map`'s result type comes from its body's result type:
  the array it builds starts empty (254), and where paths meet at the loop's
  head an empty array and an array of t make an array of t.
- At top level too: `( 1 2 3 ) [ 1 + ] map` compiles a temporary word, typed
  by its input, and runs it (docs/QUOTATIONS.md).
- Not yet on a quotation that is a value, which needs typed quotation
  parameters.
- **Mixed arrays.** An element of an array whose elements' types differ
  (252) may be either type, so a family call on it is an error (trap 23):
  `( 1 2.5 ) [ 1 + ] map` stops, while `( 1 2.5 ) [ drop 7 ] map` runs.

## Type errors

Known types that cannot work stop the definition at `;`, which is not
installed:

- **Trap 23:** a family call whose input types match no clause and do not
  lift, such as `( 1 2.5 ) 1 +`.
- **Trap 26:** an array used as a condition, by `if`, `while` or `until`.
  This catches lifting through a generic word with a branch, which used to
  give a wrong answer silently: with `: mag dup 0 lt? [ negate ] if ;`,
  `( 0 5 - 7 ) mag` lifts `lt?` to an array of flags, then branches on the
  array. Write `[ mag ] map` instead.

A definition is analysed for types when it has an array literal or calls a
family or a generic word, or in checked mode. A definition with none of these
is not analysed, so `: w mk [ 3 ] if ;` with `mk` returning an array is caught
only in checked mode.

## Showing arrays

The command line prints the stack after its depth, as March source writes
values: `<3> 1 2.5 ( ( 1 2 ) ( 3 ) )`. Integers are signed, floats have a
point, and arrays show their elements by their element type, the first
sixteen and then how many more. The types come from March: a rebuilt system's
boot returns a fourth token, `describe`, which gives the host the
interpreter's type stack, a byte per value (`Driver::types` and
`Driver::show`). Values whose types March does not know show as integers: a
quotation's token, an element read from a mixed array, and everything on a
system without `describe`, such as generation 0.

## Memory

Every array takes a region slot, charged 8 bytes per element against the
machine's live-byte limit, and nothing frees them yet; memory management is
the 2.0 work. Versions share structure inside the vector, so an array made
with `vector-set` from another costs a path, not a copy.

## Next

- **Laziness:** lifted operations, `map` and ranges produce views that fuse
  and are computed when forced (indexing, folding, printing, storing), and
  maps get the same treatment.
- **Indexing by arrays:** `xs ( 0 2 ) at` selecting elements, as APL does;
  today `at` takes one index.
- **Effects with a varying count,** so a word whose body is a comprehension
  can say how many values it leaves.
