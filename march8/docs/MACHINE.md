# The symbolic stack machine

Status: slices 1 and 2, 2026-10-08. March8 is March's compiler written in Rust, as
Thomas decided on 2026-10-08: build the compiler in Rust now, and write it
in March, bootstrapping like a real FORTH, once March is mature. System March
(march7) is frozen. This slice is the symbolic stack machine for the
explicit form (doc/design/TYPES.md 2.2). The surface notation, which lowers
to the explicit form, comes later. The staged-types prototype
(march7/docs/STAGED.md) is its specification, and its cases are the tests,
rewritten in the explicit form. Slice 1 is the machine, types and families
chosen by types; slice 2 adds guards, the choice between clauses at run
time, `map`, and arithmetic lifted over arrays.

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
```

## The explicit form

Words are separated by white space, as in FORTH. `--` starts a comment that
runs to the end of the line, except inside `< >`.

| Form | What it is |
|---|---|
| `1`, `-2` | an integer literal |
| `1.10` | a decimal literal: exact digits until its type is known |
| `"text"`, `'raw'` | a string, with march7's escapes (`\n;`, `\#x2603;`, `\times;`); holes are not built yet |
| `name` | a name, pushed as a value: it does nothing until applied |
| `.` | applies the value on top |
| `name.` | `name .`: each dot at the end of a word applies it (Thomas, 2026-10-08), so no name ends in a dot; `vec..` is `vec . .` |
| `[ … ]` | a quotation: its words, unevaluated |
| `( … )`, `{ … }` | an array and a map literal |
| `< … >` | a bracket: a type expression |

**`.` applies** whatever is on top:

- a **name** means what it is defined as: a primitive, a type, a constructor
  or a family;
- a **type** annotates the value below it: `1 i64.`;
- a **quotation** is evaluated: `[ 2 +. ].`.

**Types are values.** `100 i64 vec.` builds the type "array of 100 i64",
and `x 100 i64 vec..` annotates `x` with it. A bracket is the short
form: inside `< >` every word applies at once, a type's name pushes the
type, a number a length, a single letter a type variable, and a constructor
(`ary`, `vec`, `map`) builds from what is below it. A bracket of n types
annotates the top n values, the deepest first: `x y < i64 f64 >`.

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

## The core vocabulary

`core/core.march` defines `+`, `-`, `*`, `/`, `mod`, the comparisons, `length`,
`at`, `concat` and `print` as families of clauses over primitives, in March.
The primitives are in `src/prims.rs`: a name, a signature and the machine
operations, such as `i64+ < i64 i64 -- i64 >`, which is checked addition.
The literal primitives (`int#+`, `dec#>money`) exist only at compile time.
A primitive applied to values all known folds, unless it has an effect
(`write`).

A few are handled by the stage itself, because their types are not a
signature's: the stack words, which move judgments; `vec-length`, a
constant; `vec-at`, which checks a literal index against the length; and
`vec-concat`, whose length is the sum.

## Code

The stage emits the machine's operations (march7/docs/ENGINE.md), and a
string literal as a data object and `text`. A final call becomes a tail
call, and a return ends the word. Code is content-addressed as in march7,
in its own domain, `march8/code/v1`.

The machine is march7's, with ten primitives added: `pick`; checked
signed `i64+`, `i64-`, `i64*`, `i64/` and `i64mod`, which trap on overflow
and division by zero; signed `i64lt?`; `i64>text` and `f64>text`; and
`scratch-at`, which reads a loop's state.

Branches are labels until the code is sealed, so each alternative of a
choice can be compiled on its own and laid out afterwards.
`march8 --code SOURCE` prints the code a piece of source compiles to.

## Sessions

`Session` (`src/lib.rs`) holds a machine, a stage and the types of the
values on the machine's stack. Each piece of source is compiled as one word
whose inputs are the stack, then run, so the types of the values on the
stack are always known, and the display writes every value by its type:
`{ "k" ( 1 2 ) } ( "a" "b" )` shows as written, which march7 could not.

```sh
cargo run --offline -- --eval '[ < money > 1.10 +. ] fee def. 19.99 fee.'
# <1> 21.09
```

## Errors

Errors are found while compiling, with the line and column, and the words
being applied:

```
1:17: no word `+` for i64 string
  in `f`, defined at 1:3
```

Their kinds: no word; a mismatch (types that disagree, clauses that tie,
values missing or out of reach, a type left unapplied); a literal that
cannot become its type; arithmetic that overflows or divides by zero,
found at compile time; a limit. An error inside a core clause is reported
where the clause was applied. Errors while running are the machine's.

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

## Not yet

- **Instances, recursion and tail calls.** A word is evaluated where it is
  applied, always; a family applying itself to the same types is an error.
  Instances, ghosts (TYPES.md 2.7) and shared code come next.
- **Value patterns** such as `0` in a signature, and OR between contexts.
- **Quotations at run time:** a quotation is always consumed at compile
  time, by `.`, `def` or `map`.
- **The surface notation** and its lowering to this form.
- **Saving** a session as an image; string holes; tuples, records and sums;
  namespaces; showing types.
