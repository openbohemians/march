# Staged types: the prototype

Status: prototype, doc/design/TYPES.md made concrete. Step 1, 2026-10-07:
integers, floats, money, two kinds of literal and `+`, through every stage.
Step 2, 2026-10-08: typed words applied in typed bodies and typed for each
use, brackets that annotate, `*`, `-`, `dup`, `drop` and `swap`.

## What it does

```
typed sq dup * ;                  a word for any type that has `*`
typed area < f64 > sq ;           2.5 area   → 6.25
typed nine 3 sq ;                 nine       → 9, compiled as the literal 9
typed fee < money > 1.10 + ;      1999 fee   → 2109
typed total 19.99 fee ;           total      → 2109: 19.99 becomes money
typed two 1 1 + ;                 two        → 2, compiled as the literal 2
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
  given: `19.99 fee` makes 19.99 money.

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
- **Stack words** move judgments as the code moves values:
  - `dup` copies a literal as a new literal, so each copy can take its own
    type, and `1.5 dup +` still folds;
  - `drop` takes back a literal emitted last, leaving no code;
  - `swap` is a run-time swap of the two judgments.
- **A word's stack effect** comes from a pass over its explicit form:
  numbers push, a name pushes a value that `.` applies, and a bracket needs
  as many values as it names.
- **At `;`,** a literal no context typed defaults, an integer to i64 and a
  decimal to f64. A type, operation or typed word left unapplied is an error.

Errors are traps: 23 when no clause matches, types disagree or values are
missing; 40 for a word the prototype does not know; 41 for a literal that
cannot become its type; 42 for typed words applied more than 16 deep.

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

## Not yet

- **Instances.** A typed word is inlined at each use. Compiling it once per
  set of input types, and calling that code, would share it, and would let
  ordinary March call a generic word.
- **Recursion.** A word is not known while it is being defined, so it cannot
  apply itself. With instances, it would call its own instance, typed by a
  ghost (TYPES.md 2.7).
- **Type values as data.** The type numbers are fixed. Types as
  content-addressed values, with constructors such as `vec`, come next.
- **Families as declarations.** The operations are built in; clauses with
  signature patterns, lifting, and value patterns for if-free bodies come
  later.
- **Showing money.** Money shows as its cents.
- **Saving.** Typed words live in the session, not in a saved image.
- **Limits.** A body holds at most 64 judgments, a bracket 8 types, and 64
  typed words exist at once. Errors carry no position in the source.

The prototype is in `seed/system.march`, under "Staged types, a
prototype", and its tests are in `tests/staged.rs`.
