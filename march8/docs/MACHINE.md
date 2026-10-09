# The symbolic stack machine

Status: slices 1 to 3, 2026-10-08. March8 is March's compiler written in Rust, as
Thomas decided on 2026-10-08: build the compiler in Rust now, and write it
in March, bootstrapping like a real FORTH, once March is mature. System March
(march7) is frozen. This slice is the symbolic stack machine for the
explicit form (doc/design/TYPES.md 2.2). The surface notation, which lowers
to the explicit form, comes later. The staged-types prototype
(march7/docs/STAGED.md) is its specification, and its cases are the tests,
rewritten in the explicit form. Slice 1 is the machine, types and families
chosen by types; slice 2 adds guards, the choice between clauses at run
time, `map`, and arithmetic lifted over arrays; slice 3 adds recursion,
instances and tail calls.

```
[ < money -- money > 1.10 +. ] fee def.
19.99 fee.                          → 21.09, exactly: 2109 cents
[ dup. *. ] sq def.
3 sq.                               → 9, compiled as the literal 9
{ "k" ( 41 2 ) } "k" at. 0 at. 1 +.        → 42
[ 0 gt?. ] positive? def.
[ < i64 positive? > drop. 1 ] sign def.
[ < i64 > drop. 0 ] sign def.
x sign.                             → 1 or 0, chosen at run time
5 sign.                             → the literal 1, chosen now
( 1 2 3 ) 10 *.                     → ( 10 20 30 ): it lifts
[ 0 eq?. ] zero? def.
[ < i64 zero? > drop. 1 ] fact def.
[ < i64 > dup. 1 -. fact. *. ] fact def.
10 fact.                            → 3628800, by fact's i64 instance
```

## The explicit form

Words are separated by white space, as in FORTH. `--` starts a comment that
runs to the end of the line, except inside `< >`.

| Form | What it is |
|---|---|
| `1`, `-2` | an integer literal |
| `1.10` | a decimal literal: exact digits until its type is known |
| `"text"`, `'raw'` | a string, with march7's escapes (`\n;`, `\#x2603;`, `\times;`) and holes (`\[ code ]`, `\_`) |
| `name` | a name, pushed as a value: it does nothing until applied |
| `\name` | a symbol, in any word: `\times` is `×`, `x\_1` is `x₁` (core/symbols.txt; march7/docs/SURFACE.md) |
| `.` | applies the value on top |
| `name.` | `name .`: each dot at the end of a word applies it (Thomas, 2026-10-08), so no name ends in a dot; `vec..` is `vec . .` |
| `[ … ]` | a quotation: its words, unevaluated |
| `( … )`, `{ … }` | an array and a map literal; inside an array literal, whatever the code leaves is collected |
| `_.` | in a literal, a copy of a value from below it (Comprehensions) |
| `< … >` | a bracket: a type expression |

**`.` applies** whatever is on top:

- a **name** means what it is defined as: a primitive, a type, a constructor
  or a family;
- a **type** annotates the value below it: `1 i64.`;
- a **quotation** is evaluated: `[ 2 +. ].`.

**Types are values.** `100 i64 vec.` builds the type "array of 100 i64",
and `x 100 i64 vec..` annotates `x` with it. A bracket is the short
form: inside `< >` every word applies at once, a type's name pushes the
type, a number or a string a value, a single letter a type variable, and a
constructor (`ary`, `vec`, `map`) builds from what is below it, `vec` taking
a value as its length; `.` gives a value the type above it, as `0 i64.`. A
bracket of n types annotates the top n values, the deepest first:
`x y < i64 f64 >`. In a signature, a value left over is a value pattern.

**A definition is data:** a quotation, a name and `def`:
`[ < i64 -- i64 > 1 +. ] inc def.`. A bracket first in the quotation is
the clause's signature, its context (TYPES.md 2.15): the types of its
inputs, then after `--` the outputs it promises. Clauses of one name are a
family.

## Judgments

The machine runs the explicit form on a stack of **judgments** (TYPES.md
2.3). A judgment is a type, and the value when it is known at compile time:

| Known value | Of |
|---|---|
| an integer | an integer literal, an i64, or money in cents |
| digits and places | a decimal literal |
| a float, a string | an f64, a string |
| a type | a type, as a value |
| a name, a quotation | a word not yet applied, a quotation |
| nothing | a value at run time, on the machine's stack |
| pinned | a value at run time known to equal a value: a value pattern's input, in its clause |

**A known value has no code until a value at run time needs it.** When an
operation that runs takes it, or the word ends, the value is
**materialized**: its literal is emitted, under the values at run time
above it if there are any (by `swap`, `rot rot`, or the scratch stack). So:

- **constants fold through everything,** words, families and stack words
  alike: `3 sq.` is the literal 9, and `1 2 swap.` is two literals in the
  other order, with no swap;
- **stack words move judgments,** and emit code only for the values at run
  time among them: `dup` of a known value copies the judgment, and `swap`
  of a known value and a value at run time emits nothing;
- **the code holds only what must happen at run time.**

The prototype emitted a placeholder for each literal where it appeared and
patched it once its type was known, keeping code order equal to judgment
order. Materializing late folds more, at the price of moving values at run
time now and then.

## Types as data

`src/types.rs`. A type is a term, a constructor and its arguments, in a
table where equal terms are one entry, so a type is a small number and
equal types are equal numbers.

| Type | Meaning |
|---|---|
| `i64`, `f64` | numbers |
| `money` | exact cents in an i64; a placeholder until March has precise numbers and newtypes (TYPES.md 2.11) |
| `string` | UTF-8 text |
| `int#`, `dec#` | an integer and a decimal literal, undecided (TYPES.md 2.10); the names are placeholders (TYPES.md 3.4) |
| `type`, `symbol`, `quote` | a type, a name not yet applied, a quotation: values at compile time only |
| `a ary` | an array of any length |
| `3 a vec` | an array of exactly 3: the length is known at compile time |
| `k v map` | a map |
| `a` to `z` | type variables, in patterns |
| `atom` | in patterns, any type that is not a container: a number, a string, a literal. A class, not a type: it binds nothing, so `< atom atom >` takes two of different types |

**An array literal is a vec:** `( 7 8 9 )` is a `3 i64 vec`, so its length
is a constant. A vec is accepted where an array of the same elements is
wanted, forgetting its length, at any depth: a `string 2 i64 vec map` is a
`string i64 ary map`. It costs nothing, since both are the same at run time.

## Families

A family applied chooses its clause by the values on top (TYPES.md 2.8):

- **Matching.** Each clause's signature is matched against as many values
  as it has inputs, scoring:
  - 4 for each part of a pattern that is not a variable, and 2 for an `ary`
    that a vec stands for;
  - for a literal: 4 for its own type, 2 for its default type, 1 for any
    other type it becomes;
  - nothing for a variable, which binds; the same letter must be the same
    type, and a variable bound to a literal takes a type the literal
    becomes.

  A clause with no signature matches anything, with no score.
- **The highest score wins; a tie is an error.** No clause matching is no
  word, the same error as a name with no definition (TYPES.md 2.15).
- **The chosen clause is evaluated where it is applied, on the caller's
  judgments** (TYPES.md 2.6). So `sq` is integer multiplication in one
  place, float multiplication in another, and a constant on a literal.
- **A signature settles the inputs:** each takes the type its pattern names,
  so literals convert; the body cannot reach below its inputs; and the
  outputs it promises are checked, a literal result taking the type
  promised.
- **A clause whose signature types all its inputs** is also compiled where
  it is defined, on values at run time of those types, so its errors show
  there. If it fails, it is not defined.
- **A clause with the same context as one before replaces it,** as a FORTH
  redefinition does; otherwise the two would always tie.

### Guards: the choice at run time

A word in a signature that is not a type is a **guard**: `< i64 positive? >`
(TYPES.md 2.15). It looks at the inputs before it, as many as it takes,
without taking them, and leaves a flag. How many it takes comes from its
definition, by a pass over its words: `positive?`, `[ 0 gt?. ]`, takes one,
and `lt?` takes two, so `< i64 i64 lt? >` compares the two inputs. A word
that does not leave one value cannot be a guard.

When some clauses that match have guards, the choice has two phases:

1. **By types, now.** The literals among the inputs take the types of the
   best match, and the clauses are matched again.
2. **By guards, at run time.** For each guarded clause, in the order they
   were defined: a test of its guards on copies of its inputs, then its
   body. Last, the best clause with no guards, or, if there is none, the
   trap for no word, so that no clause whose guard holds is no word at run
   time, as at compile time.

- **A guard on known values is decided now.** One that fails drops its
  clause; one that holds makes its clause the choice, and the rest are never
  compiled. So `5 sign.` is the literal 1.
- **Every alternative starts from the same judgments** and is compiled into
  code of its own. Its results then take the types all the alternatives
  share, literals taking the others' type as in an array literal, or else
  the choice is an error (TYPES.md 2.9). Then the code is laid out: each test
  branches past its clause when it fails, and each clause jumps to the end.

```
x sign.        dup 0 swap lt? 0branch L
               drop 1 branch END
            L: drop 0
          END:
```

This is if-free code (TYPES.md 2.12): a sign, a minimum, a partial
function, written as clauses.

### Value patterns

A value left in a signature, not taken as a `vec`'s length, is a **value
pattern**: the input must equal it (TYPES.md 2.12).

```
[ < 0 > ] fib def.
[ < 1 > ] fib def.
[ < n > dup. 1 -. fib. swap. 2 -. fib. +. ] fib def.
```

- **It is a guard,** testing the input `eq?` the value, so it is chosen in
  the order defined, decided now on a known value, and tested at run time
  otherwise. `fib`'s instance tests `dup 0 eq` and `dup 1 eq` before its
  step, and `0 fib.` is the literal 0.
- **An untyped value matches an input of any type it becomes,** compared in
  that type: `< 0 >` takes an i64, an f64 or money, and `< 0.5 >` an f64.
  The input keeps its own type. `< 0 i64. >` takes an i64 only, as `0 i64.`
  is an i64 in code.
- **Strings too:** `< "quit" >`.
- **Inside its clause the input is known to equal the value:** it is
  *pinned*, a value known at compile time whose cell is still on the
  machine's stack. Passed through, it costs nothing, so `fib`'s `< 0 > ;` is
  no code; copied, the copy is the known value; folded, its cell is dropped
  and the result is a constant, so `[ < 0 > 1 +. ] g def.` compiles that
  clause to `drop 1`, and `< "quit" > "bye: " swap. concat.` to a drop and a
  constant string.
- Value patterns belong in signatures, among the inputs: one in a bracket
  in a body, or among the outputs, is an error.

### Effects

**A guard must not write** (Thomas, 2026-10-08). Otherwise its write would
happen or not as the choice is made, and on known values it would vanish
with a clause dropped at compile time. It may read, which makes it a choice
at run time.

So the stage works out effects as it works out types. Each primitive has a
set of effects, for each domain whether it reads and whether it writes, as
march5's effect rows; so far only `write`, which writes `io`, has any. The
stage adds up the effects of the primitives it emits, and an instance
records its effects beside its results' types, so the effects of a word
are found through everything it applies. A guard whose effects include a
write is refused, where its clause is defined if the clause's signature
types all its inputs, else where it is applied:

```
1:54: the guard `noisy?` writes io, and a guard must not write
  in `f`, defined at 1:48
```

A primitive with an effect is never folded: a read is a fact of the run, and
a write must happen then. march5 also had effect tokens at run time, to put
effects in order in its interaction nets; code here runs in order, so
nothing needs them yet.

### `map` and lifting

`ary quote map.` applies the quotation to each element. The quotation is
compiled once, on a value of the element type at run time, as the body of a
loop; the array, the new array and the index are on the scratch stack, read
with `scratch-at`. The quotation may read the values below its element,
`100 ( 1 2 3 ) [ over. +. ] map.`, but must leave them as they were and leave
one value. A vec maps to a vec as long. `map` given two types builds a map
type instead.

Lifting is six clauses in `core.march`, an array and a number in either
order for `+`, `-` and `*`:

```
[ < a ary a -- a ary > swap. [ over. +. ] map. swap. drop. ] + def.
```

So `( 1 2 3 ) 1 +.` is `( 2 3 4 )`, a literal taking the elements' type, and
the prototype's original bug, `{ "k" ( 1 2 ) } "k" at. 1 +.`, lifts.

### Literals are clauses too

A clause defined under a type's name says how a literal becomes that type:

```
[ < int# -- i64 > int#>i64. ] i64 def.
[ < dec# -- money > dec#>money. ] money def.
```

Applying a type to a literal converts it by this clause, and so does a
clause that wants the type and is given the literal. So literal handling is
an open family, as Thomas framed it ("just another type signature"). Two
literals fold by clauses on the literal types, `< int# int# -- int# >`,
exactly, and stay literals, so `1 2 +. 3.5 +.` is the decimal 6.5. A
literal takes its default type, i64 or f64, only when it must be a value at
run time.

### Loops: `each`, folds and reductions

`ary q each.` applies `q` to each element in turn, from the first;
`each-right` from the last. The quotation takes its element and may change
the values below it, an accumulator, but must leave as many as there were,
of the same types, since the next turn starts from them: the loop's
**invariant**.

```
0 ( 1 2 3 ) [ +. ] each.          → 6
0 ( 1.5 2.5 ) [ +. ] each.        → 4.0: the literal 0 becomes an f64
```

The stage compiles the body once to see what it leaves, and throws that code
away. A value it changes becomes, before the loop, a value at run time of the
type it keeps, a literal taking the body's type as the ghost of a recursion
does; then the body is compiled again, until nothing changes. So the 0 above
is the i64 0, under the array, and the body is one `i64+`. The array and an
index are on the scratch stack.

The rest are words in `core.march`:

| Word | Means |
|---|---|
| `xs x q fold` | from the left: ((x e0) e1) e2 |
| `xs x q fold-right` | from the right: e0 (e1 (e2 x)), its quotation taking an element under the accumulator |
| `xs q reduce` | `fold` from the first element; `reduce-right`, `fold-right` from the last |
| `xs q scan` | the running reductions from the left: `( 1 2 3 ) + scan.` is `( 1 3 6 )`, a comprehension |
| `n q repeat` | `q` n times, as `each` over `n range` |
| `first`, `last`, `rest`, `most`, `slice` | parts of an array, and `slice` of a string, from element i to element j, both included |
| `xs v k insert`, `xs k remove` | v put in the gap after element k: `0 insert` prepends, `-1 insert` appends; element k taken out |

**Elements count from 1, gaps from 0** (Thomas, 2026-10-09). An array of n
has elements 1 to n, and -1 to -n from the end; there is no element 0. Its
n+1 gaps count from 0, before the first, each gap after the element of its
number, so gap -1 is after the last. So `( 5 6 7 ) 1 at.` is 5 and `-1 at.`
7; `2 -2 slice.` drops the first and the last; `0 insert` prepends and
`-1 insert` appends, with no -0 needed; and `n range.` is 1 to n. Counting
from 0, the gap before element k is k, so -1 cannot append (Python's
`insert(-1, x)`); counting elements from 1 makes the ends symmetric. A known
index is settled at compile time, and checked against a known length; one
known only at run time is settled by a few instructions.
| `xs q within` | `q` run with the array's last elements as its stack, as many as it takes, and what it leaves put in their place: `( 1 2 3 ) [ ~. ] within.` is `( 1 3 2 )`; `[ 4 ] within.` appends, `[ drop. ] within.` drops the last |
| `spread` | an array's elements on the stack: a vec's each a value; any array's, in a literal, a run |
| `sort` | an array in order: numbers by value, strings by text |
| `keys`, `values` | a map's keys and values, in one order, the map's own |

`within` is an adverb in J's sense (Thomas, 2026-10-09): it takes a word
that acts on the stack and makes it act on the end of an array, as Joy's
`infra` and Factor's `with-datastack` do. How many elements it takes comes
from the effect pass; only those are loaded, and the rest of the array is
kept by a slice and joined to what the word leaves, so it costs what the word
does, not the array's length. A vec's new length is known.

With `-` on `( 1 2 3 )`, `fold` from 0 is −6 and `fold-right` is 2, as APL's
`-/`; for an associative word the two agree. A consumer takes a word as well
as a quotation, so `( 1 2 3 4 ) + reduce.` is APL's `+/`, and in the surface,
`'` will leave a word unapplied for it. `q r compose.` joins two quotations at
compile time; `n range.` is the array 1 to n, a vec when n is known, and
`reverse` an array reversed.

### Comprehensions

Inside an array literal, whatever the code leaves is an element, as many as
it leaves: `(` marks the machine's stack and `)` gathers what is above the
mark (march7/docs/ARRAYS.md). So `each` and the literal together are map,
filter, flat-map and scan, with one loop:

```
( xs [ 10 *. ] each. )            map
( xs keep each. )                 filter: keep's clauses leave the element or nothing
( xs [ dup. ] each. )             each element twice
( 0 xs [ over. +. ] each. )       scan: ( 0 1 3 6 ), reading the last collected
( 3 [ 7 ] repeat. )               ( 7 7 7 )
```

- **A run** is a new kind of judgment: zero or more values of one type,
  counted only when the literal gathers them. A loop in a literal whose body
  leaves values above the loop's is collecting, and leaves a run; clauses
  chosen at run time that leave different numbers of values, the filter's
  one or none, leave a run. The none is Thomas's mirror, and here it costs
  nothing: the gather counts what is there. A literal with a run is an array
  of any length, `T ary`; without one, a vec, as before.
- **Nothing reaches beneath a run,** whose depth is not known: a run can only
  be gathered, so adding to a filter's result is an error. The elements
  before it become values at run time first, of the one element type.
- **A collecting body may read the values below its element,** which are
  elements already collected, of the element type, as a scan reads the last;
  it may not take them.
- **`_.` pulls a value from below the literal.** The first `_.` written takes
  the deepest, so `1 2 ( _. _. /. )` is `( 1 2 /. )`, as march7 decided for
  string holes; the literal takes what it pulls, its inputs. A pull copies,
  so one in a loop reads the same value each pass, and it is found from the
  literal's mark (`mark-pick`), exact even above a run:
  `10 ( xs [ _. +. ] each. )` adds 10 to each element.

Outside a literal the rules stay strict: a loop keeps its shape, and clauses
chosen at run time leave as many values. A value that may be absent outside a
collection needs a union with nothing, `a or nil` (TYPES.md 3.6).

### Strings with holes

A hole, `\[ code ]`, writes the value of its code into a string, and `\_` an
input, short for `\[ _. ]` (march7/docs/STRINGS.md):

```
"1 + 2 = \[ 1 2 +. ]"           "1 + 2 = 3", a constant
1 2 "\_ and \_"                 "1 and 2": the first input is the deepest
[ "<\[ _. 1 +. ]>" ] f def.     5 f. is "<6>", 1.5 f. is "<2.5>"
"H\_2;O"                        "H₂O": a subscript's name and `;` follow
```

- **The reader** reads a hole's code with its own words, to a `]` where a
  word would start with the hole's own brackets closed, so text resumes right
  after it (`"\[ n. ]apples"`), and strings nest.
- **The stage** pushes each piece: the text, and each hole's value written
  in by `>string`, a string's own text or any other value as `show` writes
  it; and joins them with `concat`. On known values everything folds, and
  the string is one constant. A hole leaves one value and reaches only what
  it makes.
- **`_.` in a hole** pulls an input from below the string, as in an array
  literal, the first written taking the deepest, and the string takes its
  inputs, so a word whose body is such a string takes them. A string makes
  no mark: nothing above its inputs is a run, so a pull is a plain copy.
- **`show`,** in `core.march`, writes a value as the display does, so it
  reads back: a string quoted, money with its cents, an array in
  parentheses, element by element, and a map in braces, sorted by key (Thomas,
  2026-10-08), as the display sorts it. `print` writes any value, by
  `>string`.

### Arrays as tensors

Scalars, vectors and matrices are tensors of rank 0, 1 and 2 (Thomas,
2026-10-09), and in March a tensor of rank n is arrays nested n deep. A vec
type gives its shape at compile time: `2 3 i64 vec vec` is a 2×3 matrix.

```
( ( 1 2 ) ( 3 4 ) ) 10 ⋅.                 ( ( 10 20 ) ( 30 40 ) )
( ( 1 2 ) ( 3 4 ) ) ( 10 20 ) +.          ( ( 11 22 ) ( 13 24 ) ): across each row
( 1 2 3 ) ( 4 5 6 ) dot.                  32
( ( 1 2 ) ( 3 4 ) ) ( ( 5 6 ) ( 7 8 ) ) dot.   ( ( 19 22 ) ( 43 50 ) ): the matrix product
( 1 2 ) ( 3 4 5 ) ×.                      ( ( 3 4 5 ) ( 6 8 10 ) ): the outer product
( ( 1 2 3 ) ( 4 5 6 ) ) transpose.        ( ( 1 4 ) ( 2 5 ) ( 3 6 ) ), a 3 2 i64 vec vec
```

- **Element by element.** `+`, `-`, `⋅` and `÷` on two tensors of one shape
  pair their elements, by `xs ys q zip`, level by level. A tensor and one of
  lower rank, a number or a row, pair along the leading axis, recursively,
  so the lower broadcasts across the higher's trailing axes. A number goes
  into a tensor of any rank: `atom` in a pattern matches any type that is
  not a container ("Types as data"), so four clauses an operation cover every rank.
  A tensor and a row two or more ranks lower do not pair yet.
- **`×`, the outer or tensor product:** each element of A times all of B, so
  ranks add: two matrices make a tensor of rank 4. `xs ys q table` applies
  any word to every pair, as APL's `∘.` and Uiua's `⊞`.
- **`dot` contracts A's last index with B's first:** vector · vector is a
  number, matrix · vector a vector, matrix · matrix the matrix product. Its
  symbol is open: `⋅` multiplies element by element, as everything else
  works, and `⊙`, a dot in a circle, is the Hadamard product in mathematics.
- **`transpose`** swaps a matrix's two indices, written in March with `map`
  and `range`.
- **Shapes are checked at compile time** when they are known: lengths paired
  by `zip`, and so the contracted lengths of `dot`, a 2×3 by a 2×2 refused
  before anything runs; at run time otherwise. A clause keeps a vec's length
  when its signature asks for an array, so shapes reach the words that check
  them.

### Recursion and instances

A family is evaluated where it is applied, on the caller's judgments,
unless it is recursive. **Recursion is found as it happens:** when a family
is applied again, to the same types, inside its own application, the
compiler unwinds to the outer application, puts back its judgments and code,
and makes it a call to an **instance**:

- An instance is the family compiled once for its input types, as a word
  of its own, guards and all, and cached: every word applying `fact` to an
  i64 calls the same code, by its content identity. A literal input takes
  the type of the best match, or its default, first.
- Inside the instance, the family applied to those types again is a call
  to itself, `recur`.
- Recursion through other words is found the same way. Only recursion
  needs an instance; every other word is inlined, so folding is unchanged.

**The ghost.** A recursive call's results have the types of the
alternatives that finish without recursing (TYPES.md 2.7). An alternative
that reaches the recursion before any has finished **waits**; once others
have finished, their results type the recursion and it is compiled again.
So the base case need not come first:

```
[ 0 gt?. ] positive? def.
[ < i64 > ] down def.                       the base, with no guard
[ < i64 positive? > 1 -. down. ] down def.  tested first, compiled after
```

A literal result, as `fact`'s 1, takes the type of an input it can become,
so `fact` on an f64 multiplies floats; if the instance then leaves other
types than the ghost said, it is compiled again with the types it found. A
family that applies itself with no clause finishing without doing so has no
types for its results, an error where it is defined, unless its signature
promises outputs after `--`: then it is a loop.

**Tail calls.** A call followed only by a return, perhaps after branches,
is a tail call, and a call to itself one in its own frame, so recursion that
loops runs in constant space: a million levels of `sumto`.

```
march8 --code '5 fact.'      source:  5  tail fact-i64
                             fact-i64:
                               dup 0 eq 0branch L  drop 1  branch END
                             L: dup 1 i64- recur i64*
                           END:
```

## The core vocabulary

`core/core.march` defines `+`, `-`, `*` (written `⋅`, mathematics first:
march7/docs/SURFACE.md), `/`, `mod`, the comparisons, `length`,
`at`, `concat`, `slice`, `print`, the folds, reductions and `repeat` as families
of clauses over primitives, in March. Also there: `=` and `~` as `dup` and
`swap`, and doubled, `==` and `~~`, as FORTH's 2DUP and 2SWAP (`dup2`,
`swap2`); the guards `zero?`, `positive?` and `negative?`, and `abs`, `min` and
`max` as clauses chosen by them, with no `if`; `sqrt`; `pow`, x to the y,
exact for two literals and checked for integers; and `divmod`, the quotient
and remainder, floored (`-7 3 divmod.` is `-3 2`), with `div` and `mod` each
taking one of them, so the three agree, and `/` as `div` for now; and
`floor`, `ceil`, `round` and `trunc`, which narrow a float to an integer,
saying how it rounds (`round` halves away from zero), exact on a decimal
literal, an error for a float with no i64. doc/design/WORDCHART.md
charts the words beside APL's and Uiua's.
The primitives are in `src/prims.rs`: a name, a signature and the machine
operations, such as `i64+ < i64 i64 -- i64 >`, which is checked addition.
The literal primitives (`int#+`, `dec#>money`) exist only at compile time.
A primitive applied to values all known folds, unless it has an effect
(`write`).

A few are handled by the stage itself, because their types are not a
signature's: the stack words, which move judgments, the values at run time
among them moving by the scratch stack only if their order changes; `vec-length`, a
constant; `vec-at`, which checks a literal index against the length; and
`vec-concat`, whose length is the sum.

## Code

The stage emits the machine's operations (march7/docs/ENGINE.md), and a
string literal as a data object and `text`. A final call becomes a tail
call, and a return ends the word. Code is content-addressed as in march7,
in its own domain, `march8/code/v1`.

The machine is march7's, with twenty-five primitives added: `pick`; checked
signed `i64+`, `i64-`, `i64*`, `i64-quot` and `i64-rem`, which trap on
overflow and division by zero, and `i64-divmod`, floored; signed `i64lt?`; `i64>text` and `f64>text`;
`scratch-at`, which reads a loop's state; `range` and `reverse`;
`mark-pick`, which reads below an array literal's mark; `money>text` and
`string-show`; `sort-ints`, `sort-floats` and `sort-texts`; `f64-sqrt`,
`f64-pow` and `i64-pow`; and `f64-floor`, `f64-ceil` and `f64-round`.

Branches are labels until the code is sealed, so each alternative of a
choice can be compiled on its own and laid out afterwards. The stage seals
code itself, hashing it into blobs, and the session publishes them, callees
first. `march8 --code SOURCE` prints the code a piece of source compiles to,
and the code of the words it calls.

## Sessions

`Session` (`src/lib.rs`) holds a machine, a stage and the types of the
values on the machine's stack. Each piece of source is compiled as one word
whose inputs are the stack, then run, so the types of the values on the
stack are always known, and the display writes every value by its type:
`{ "k" ( 1 2 ) } ( "a" "b" )` shows as written, which march7 could not.

```sh
cargo run --offline -- --eval '[ < money > 1.10 +. ] fee def. 19.99 fee.'
# <1> 21.09
cargo run --offline -- --types --eval '{ "a" ( 1.5 ) } 2'
# <2> { "a" ( 1.5 ) } < string 1 f64 vec map > 2 < i64 >
```

With no arguments, `march8` is a REPL: each line is compiled and run, and
the stack shown after it. A line ending inside a bracket or a string goes on
to the next, and an error leaves the session as it was. Commands start with
`:`, which March does not use: `:types` shows each value's type after it, in
a bracket, so the line reads back as March; `:code SRC` shows the code SRC
compiles to; `:quit`. `march8 fmt` rewrites `\name` escapes as their symbols,
from stdin to stdout, in code and comments but not strings.

## Errors

Errors are found while compiling, with the line and column of the word
being applied, not its `.`, since a name remembers where it was written, and
the words being applied:

```
1:15: no word `+` for i64 string
  in `f`, defined at 1:3
```

Their kinds: no word; a mismatch (types that disagree, clauses that tie,
values missing or out of reach, a type left unapplied); a literal that
cannot become its type; arithmetic that overflows or divides by zero,
found at compile time; an effect where none is allowed; a limit. An error
inside a core clause is reported where the clause was applied. Errors while
running are the machine's.

Warnings are planned, at three levels: informative, minor and severe
(Thomas, 2026-10-09). The first will be a slot that may hold more than one
type at run time (TYPES.md 3.6).

## What it found

1. **Most of the language fits in March.** Arithmetic, comparisons,
   containers, printing and literal conversion are families in
   `core.march`; Rust holds the primitives and the stage's rules.
2. **Late materialization folds more than placeholders did,** and needs no
   patching: constants pass through stack words and families, and only
   values at run time cost code.
3. **No match is no word, everywhere.** Errors the prototype reported as type
   mismatches (`< i64 f64 > +`) are no word here, since `+` has no clause for
   them, which is the design's rule.
4. **An array literal is a vec,** so lengths are constants wherever arrays
   are written out, and a vec is accepted as an array.
5. **Guards fold like everything else.** A guard is a word applied to
   copies of the inputs, so on known values it is decided at compile time
   with no extra mechanism, and the choice disappears.
6. **Compiling each alternative into code of its own** lets the results'
   types be joined before any code for them is fixed, so a literal in one
   clause adapts to the type another leaves (TYPES.md 2.7).
7. **Lifting needs no special case:** with `map`, it is ordinary clauses.
8. **Recursion is better found as it happens than from the text.** A test
   for a family's name in its bodies said `+` was recursive, since lifting
   applies `+` to the elements; applied to other types, that is not
   recursion. Finding the same types again during evaluation is exact, and
   finds recursion through other words too.
9. **Waiting alternatives free the order of clauses.** The prototype needed
   a guarded base first; here any alternative that finishes types the
   recursion.

## Not yet

- **Deferred terms and rewriting clauses.** `range` and `reverse` build
  arrays. Since everything is a value until `.` applies it, applying a pure
  word could leave a term, "`reverse` of this array", made code only when
  something forces it, and a clause could match the term's form: `fold` on a
  `reverse` would be a loop backwards and `n range each` a counted loop, with
  no array built (Thomas, 2026-10-08). That is rewriting, as GHC's rules, in
  March itself, with no views in Rust. Only pure words may be deferred, which
  the effects say; a rule's two forms are trusted to be equal.
- **`#` for `count`,** as J's tally, perhaps also meaning `each` on a
  quotation (`xs [ f. ] #.`), the literal then deciding what is collected
  (Thomas, 2026-10-08, noted). It needs headings to start lines, as in
  Markdown, since SURFACE.md's headings are words that read what follows.
  For now the word is `count`, the same as `length`.
- **Array literals typed by what follows.** `( 2 4 )` settles at its `)`, to
  integers, so `( 6.0 8.0 ) ( 2 4 ) ÷.` is no word; written `( 2.0 4.0 )` it
  works.
- **Unions,** set-theoretic (TYPES.md 3.6): `a or nil` first, a value and a
  tag, for FORTH's words that return a value or nothing; clauses split on
  the tag, with a light warning where a slot may hold more than one type.
- **Glyphs:** APL's and Uiua's for these words, and whether `/` is reduce,
  are deferred.

- **Mutual recursion** between instances, which would make a cycle of
  content identities; and words used before they are defined.
- **Sharing large words** that are not recursive, as instances, to save code:
  everything else is inlined, which is fast and folds constants, but grows
  code. An instance keyed by its known inputs as well as its types, returning
  its known results as judgments, means exactly what inlining means, so the
  choice is only one of size and speed (2026-10-08).
- **Definitions' identities.** A CID names compiled code: an instance, or a
  piece of top-level code, with names, types and inlined words gone. A
  definition has no identity yet. Open: its signature and words as written,
  families resolved where it is used; pinned, as Unison, each name resolved
  when it is defined to the family as it is then, so a new clause makes new
  identities; or modules pinning what they import. To decide with images and
  modules (doc/design/TYPES.md, open question 2).
- **OR between contexts.**
- **Effects in signatures,** declared and checked like outputs, for words
  whose bodies the compiler cannot see; and domains besides `io`, such as
  the global store, with primitives that read.
- **Quotations at run time:** a quotation is always consumed at compile
  time, by `.`, `def` or `map`.
- **The surface notation** and its lowering to this form.
- **Saving** a session as an image; tuples, records and unions;
  namespaces; showing types.
