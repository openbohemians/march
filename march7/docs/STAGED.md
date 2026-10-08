# Staged types: the prototype

Status: prototype, doc/design/TYPES.md made concrete. Step 1, 2026-10-07:
integers, floats, money, two kinds of literal and `+`, through every stage.
Step 2, 2026-10-08: typed words applied in typed bodies and typed for each
use, brackets that annotate, `*`, `-`, `dup`, `drop` and `swap`. Step 3, the
same day: types as data, built by `ary`, `vec` and `map`; strings; and
`length`, `at` and `concat` typed by structure. Step 4, the same day:
headings, families of clauses chosen by types and by guards, type
variables, `--` outputs, lifting over arrays, comparisons and array
literals. Step 5, the same day: instances, recursion typed by ghosts, tail
calls, and ordinary March calling a family; `over` and `rot`.

## What it does

```
typed sq dup * ;                  a word for any type that has `*`
typed area < f64 > sq ;           2.5 area   → 6.25
typed nine 3 sq ;                 nine       → 9, compiled as the literal 9
typed fee < money > 1.10 + ;      1999 fee   → 2109
typed total 19.99 fee ;           total      → 2109: 19.99 becomes money
typed two 1 1 + ;                 two        → 2, compiled as the literal 2

typed h < string i64 ary map > "k" at 0 at 1 + ;   { "k" ( 41 2 ) } h → 42
typed bad < string i64 ary map > "k" at 1 + ;      compile error: an array
typed fl < string f64 ary map > "k" at 0 at 1 + ;  the 1 is a float
typed v < 3 i64 vec > length ;                     the constant 3

typed positive? 0 gt? ;
# < i64 positive? >
typed sign drop 1 ;
# < i64 >
typed sign drop 0 ;                5 sign → 1, -3 sign → 0, chosen at run time

# < string i64 ary map >
typed h "k" at 1 + ;               { "k" ( 1 2 ) } h → ( 2 3 ): it lifts

typed zero? 0 eq? ;
# < i64 zero? >
typed fact drop 1 ;
# < i64 >
typed fact dup 1 - fact * ;        5 fact → 120, from ordinary March too
```

- **`typed name … ;`** takes a body in the surface form. **Stage 1** lowers
  it to the explicit form, in which `.` applies: every word that is not a
  number gets a `.` after it, so `1 1 +` becomes `1 1 + .`. Brackets are
  copied as they are.
- **`explicit name … ;`** takes a body already in the explicit form:
  `1 i64 . 1 i64 . + .` annotates each literal before adding.
- **Stage 2, the type stage,** reads the explicit form with a stack of
  judgments beside it and leaves plain code behind.
- **Every typed word is recorded,** with its explicit form and its stack
  effect, worked out from its words.
  - A word whose signature types all it takes, or that takes nothing, is
    also compiled, and ordinary March can call it.
  - A word like `sq`, which takes a value without saying its type, has no
    code of its own.
- **A typed word applied in a typed body is evaluated there,** on the
  caller's judgments. So `sq` is integer multiplication in one body,
  float multiplication in another, and a constant when applied to a
  literal. This is "a word's types come from evaluating it per use"
  (TYPES.md 2.6), with the word's code inlined at each use.
- **Brackets, `< t … >`,** declare the inputs when first in a definition,
  and anywhere else type as many values on top, the deepest first (TYPES.md
  2.13). So a signature applied in another body types what its word is
  given: `19.99 fee` makes 19.99 money. Types after `--` are outputs.
- **Headings, `# < … >`,** give the definitions after them their context
  (TYPES.md 2.15). Lowering puts the bracket first in each body, as that
  definition's own signature, so the explicit form has no mode. A heading
  always starts a section, even after an empty one; a heading that names a
  namespace, `# main`, ends the context. (Terms, TYPES.md 2.15: a family
  shares a name, a domain a context, a section is the text under one
  heading.)
- **Typed words of one name are a family,** each a clause. Applied in a typed
  body, a family is resolved there: the clauses whose input types match, the
  most specific winning, a tie an error; then, if some have guards, a choice
  at run time. No clause that matches is the error for an unknown word (trap
  1), whether found at compile time or at run time.

## Types and judgments

| Number | Type |
|---|---|
| 1 | i64 |
| 2 | f64 |
| 3 | money: exact cents, in an i64 |
| 4 | integer literal |
| 5 | decimal literal: digits and how many follow the point |
| 6 | a type, as a value: `i64`, `f64`, `money` |
| 7 | an operation, as a value: `+`, `*`, `-`, `dup`, `drop`, `swap` |
| 8 | a typed word, as a value |
| 9 | string |
| 10 to 35 | type variables `a` to `z` |
| 50 | where an array literal's elements start |
| 256 on | a type built by `ary`, `vec` or `map`: a term in the type table |

A judgment holds a type, whether its value is known, the value, a decimal
literal's scale, and where its literal was emitted.

## How it works

- **A literal is emitted where it appears,** as a placeholder: a literal
  instruction with no value yet. When its type is settled, its opcode and
  value are patched in place. So the code's stack always matches the
  judgments', and nothing is ever reordered.
- **`.` applies the top judgment.**
  - A type annotates the judgment below it: a literal converts to it, and
    anything else must already have it.
  - An operation acts on the judgments below it.
  - A typed word's explicit form is evaluated on them, up to its `;`.
- **`+`, `*` and `-`** work by their operands' types:
  - two literals emitted last fold into one, exactly; decimals align for
    sums and differences, and add their fractions' lengths for products;
  - a literal takes the other operand's type;
  - then both types must agree. f64 uses the float primitives; i64 and
    money the checked words, which trap on overflow at run time. Money is
    not multiplied.
- **Each type converts a literal once:**
  - to i64: an integer literal as it is; a decimal literal is an error;
  - to f64: either kind, as a float;
  - to money: an integer literal is whole units; a decimal literal becomes
    cents exactly, once trailing zeros go, and is an error with more than
    two digits after the point.
- **Types are data.** A built type is a term, a constructor with its
  arguments, in a table where equal terms are one entry (hash-consing). So
  `i64 ary` built twice is one type, and comparing types is comparing
  numbers. The constructors:
  - `ary` takes an element type: an array of any length;
  - `vec` takes a length, a number known at compile time, and an element
    type: an array of exactly that length;
  - `map` takes a key type and a value type.
- **A bracket is a type expression,** a little stack program: a type's name
  pushes the type, a number a length, and a constructor builds from what is
  below it. It leaves one type for each value it types: `< i64 ary a >` types
  two values, an array of i64 and an `a`.
- **Containers are typed by their structure:**
  - `at` gives an array's element type, by an i64 index; a map's value type,
    by a key of its key type; a string's code point. A literal index or key
    takes the type wanted, and a literal index into a vec is checked against
    its length at compile time;
  - `length` is an i64, except of a vec, whose length is a constant: the
    value is dropped and the length is a literal;
  - `concat` joins two strings, or two arrays of one type; two vecs make a
    vec as long as both.
- **Arithmetic lifts over arrays,** as the built-in clause
  `< a ary a -- a ary >`: `+`, `*` or `-` with an array of numbers uses the
  lifted word march's checker already makes for the element operation, and
  a literal takes the elements' type. A map or a string given to arithmetic
  is refused, and money is not multiplied. `lt?`, `gt?` and `eq?` compare
  numbers of one type and leave a flag.
- **Array literals,** `( … )`, mark where their elements start; at `)` the
  elements share one type, literals settling on the others' type, or f64 if
  any is a decimal and i64 otherwise.
- **Clause choice by types.** Each candidate clause's signature is read and
  matched against the values on top: a literal matches a type it converts
  to, scoring 2 for its default type and 1 for another; a variable matches
  anything, binding, the same letter the same type; anything else must
  unify, scoring 4 for each part of the pattern that is not a variable. The
  highest score wins, so `< i64 ary >` beats `< a ary >`, and an exact type
  beats a literal's conversion.
- **Clause choice by guards.** A guard in a signature, a word leaving a flag
  such as `positive?` or `lt?`, looks at the inputs before it without
  consuming them. With guarded candidates, the inputs are settled first,
  the judgments saved, and a chain of tests emitted: for each guarded clause
  in the order defined, copies of the inputs its guards look at (`dup`,
  `over`, or a copy from two down), the guard, and a jump past the clause if
  it fails; then the clause, and a jump to the end. Last comes the most
  specific unguarded clause, or the trap for no word. Every alternative
  starts from the saved judgments, and all must leave the same types.
- **Each family application has a frame of its own,** a small region for its
  candidates, its saved judgments and its jumps, since a clause may apply
  other families.
- **Evaluated where applied, or an instance.** A family applied to values
  all known at compile time, and not already being evaluated so, is
  evaluated where it is applied, so `3 sq` folds to 9. Otherwise its inputs
  are settled, by the primary clause's signature or to their defaults, and
  it is an instance: the family resolved, guard chain and all, in a word of
  its own for those input types, compiled the first time and called, so
  words applying `sq` to an i64 share one instance. The checker is told an
  instance's result types, which its code alone does not show.
- **Recursion.** A family applied within its own instance, while it is
  being compiled, calls itself. Its results are a ghost, typed by the first
  alternative of the instance to finish: the base case, a guarded clause
  defined first. A call to itself last in an alternative becomes a tail
  call, so recursion that loops runs in constant space: a million levels of
  `sumto`.
- **Ordinary March calls the family.** A typed word compiled for itself, if
  its family has other clauses, runs the family applied to values of its
  signature's types, so `5 fact` from ordinary March tests the `zero?`
  guard.
- **String literals** are compiled as anywhere else, and are strings. Stage
  1 copies them whole, spaces and all.
- **Stack words** move judgments as the code moves values:
  - `dup` copies a literal as a new literal, so each copy can take its own
    type, and `1.5 dup +` still folds;
  - `drop` takes back a literal emitted last, leaving no code;
  - `swap` is a run-time swap of the two judgments.
- **A word's stack effect** comes from a pass over its explicit form:
  numbers push, a name pushes a value that `.` applies, a bracket needs as
  many values as it types, and `( … )` leaves one. A word is compiled for
  itself only if its signature types all it takes, with no type variable and
  no guard.
- **At `;`,** declared outputs type the results; a literal no context typed
  defaults, an integer to i64 and a decimal to f64. A type, operation or
  typed word left unapplied is an error.

Errors are traps: 1 when no clause matches, as for an unknown word; 23 when
types disagree, clauses tie, or values are missing; 27 for a limit; 40 for
a word the prototype does not know; 41 for a literal that cannot become its
type; 42 for typed words applied more than 16 deep.

## What it found

1. **Patching literals in place keeps lowering simple.** Code order always
   matches judgment order, so the stage never shuffles the stack.
2. **Stage 1 applies every word, so the surface needs brackets to build
   types** (TYPES.md 2.13). `19.99 money` annotates, but `100 i64 vec` would
   apply `i64` to 100.
3. **Cell stores must be aligned,** and a literal's value is not, so a
   placeholder's value is written a byte at a time.
4. **Making signatures annotations pays off at once.** A typed word applied
   to a literal converts it by the word's own signature, with no extra
   mechanism: `19.99 fee` is money because `fee` says so.
5. **Evaluating per use by inlining is simple, and enough to type every
   use.** Its costs are code size, and that a word with no code of its own
   cannot be called from ordinary March.
6. **The type stage is recursive,** since applying a typed word reads words,
   and System March defines words before use. So the stage reaches its own
   token reader through a working-memory cell, as the interpreter does.
7. **The original bug is gone.** With the map's type known, `"k" at 1 +` is
   an array plus a number: refused in step 3, lifted over the array in step
   4, where `:` quietly adds 1 to a handle. And types reach literals through
   containers: a map of f64 arrays makes `1` a float.
8. **Shared temporaries are fragile.** The prototype keeps its state in
   working-memory cells, and one bug came from a helper reusing a cell its
   caller still needed. A real type stage should keep its state in its own
   regions, or in locals.
9. **Postfix types can hide their arity.** `< i64 ary a >` is two types, but
   a reader must know `ary` takes one argument (Thomas). Aliases for common
   types are the likely remedy: SURFACE.md's roles.
10. **If-free code works.** Guards turn clause choice into the program's
    conditionals: a sign, a minimum, a partial function, without `if`, and
    the same-type rule for choices at run time falls out of saving and
    comparing judgments.
11. **No match is one error at both times,** as Thomas framed it: a word's
    name is its outermost context, so a missing clause at compile time and a
    failed guard at run time are both an unknown word.
12. **Recursion through shared state needs frames.** Family resolution
    nests, since a clause may apply families, so its state lives in a region
    per application rather than in working memory: finding 8, applied.
13. **The ghost falls out of the guard chain.** The chain already saves the
    first alternative's result types to check the others, so a recursive
    call's types come from there: the base case types the recursion, as
    TYPES.md 2.7 proposed, with no separate inference.
14. **Inline when known, instance when not** gives both folding and sharing:
    constants fold through families, and code for unknown values is
    compiled once per type.
15. **Two type systems must agree.** Ordinary March's checker infers from
    code and cannot see into an instance, so the prototype tells it the
    instance's result types, as march's lifted words already do.

## Not yet

- **Recursion's limits.** A family's first clause cannot apply the family,
  not yet defined, so the base case comes first; and it must be a guarded
  clause, since guarded clauses are compiled before the unguarded one, so a
  recursive guarded clause before an unguarded base has no types to go by
  (TYPES.md 2.7 would add a signature's outputs, or a pass over the base
  first). Instances calling each other in a cycle are not supported.
- **Ordinary March and later clauses.** The word ordinary March calls is the
  newest clause compiled for itself; a guarded clause added later, which
  has no code of its own, is not seen until another such clause is.
- **Content identities of types.** Built types are one number each within
  a session; the hash of a term, for storing types and for instances across
  sessions, is not computed yet.
- **Checking a vec's length at run time.** A word declaring a `3 i64 vec`
  input trusts what ordinary March gives it; the declared length should be
  checked on entry.
- **Tuples and records,** and map literals in typed bodies.
- **OR between contexts,** and nested headings and namespaces: one heading
  level for now.
- **Outputs of applied clauses.** A clause's declared outputs are checked
  when it is compiled for itself, not where it is applied.
- **Guards** look at inputs at most two down, are tested in the order
  defined, and must be words: no value patterns such as `0`.
- **Showing money.** Money shows as its cents.
- **Saving.** Typed words live in the session, not in a saved image.
- **Limits.** A body holds at most 64 judgments, 64 typed words and 256
  built types exist at once, and a string literal in a typed body may not
  hold a code hole. Errors carry no position in the source.

The prototype is in `seed/system.march`, under "Staged types, a
prototype", and its tests are in `tests/staged.rs`.
