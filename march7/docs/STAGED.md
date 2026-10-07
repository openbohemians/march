# Staged types: the first prototype

Status: prototype, built 2026-10-07, the "thin end-to-end" step of
doc/design/TYPES.md. It covers integers, floats, money, two kinds of
literal and `+`, through every stage, to show that the staged model works
and to find what the design has not thought of yet.

## What it does

```
typed fee < money > 1.10 + ;      1999 fee   → 2109
typed inc < f64 > 1 + ;           2.5 inc    → 3.5
typed two 1 1 + ;                 two        → 2, compiled as the literal 2
```

- **`typed name … ;`** takes a body in the surface form. **Stage 1** lowers
  it to the explicit form, in which `.` applies: every word that is not a
  number gets a `.` after it, so `1 1 +` becomes `1 1 + .`.
- **`explicit name … ;`** takes a body already in the explicit form:
  `1 i64 . 1 i64 . + .` annotates each literal before adding.
- **Stage 2, the type stage,** reads the explicit form with a stack of
  judgments beside it and leaves plain code behind, which is sealed and
  installed as any definition is.
- A body may start with its inputs' types, `< f64 money >`. These are
  judgments whose values are only known at run time.

Both forms give the same code: `typed a 1 1 + ;` and `explicit b 1 1 + . ;`
have one identity.

## Types and judgments

| Number | Type |
|---|---|
| 1 | i64 |
| 2 | f64 |
| 3 | money: exact cents, in an i64 |
| 4 | integer literal |
| 5 | decimal literal: digits and how many follow the point |
| 6 | a type, as a value: `i64`, `f64`, `money` |
| 7 | an operation, as a value: `+` |

A judgment holds a type, whether its value is known, the value, a decimal
literal's scale, and where its literal was emitted.

## How it works

- **A literal is emitted where it appears,** as a placeholder: a literal
  instruction with no value yet. The judgment records where. When the
  literal's type is settled, the placeholder's opcode and value are patched
  in place: an integer literal instruction for i64 and money, a float one for
  f64. So the code's stack always matches the judgments', and nothing ever
  has to be reordered.
- **`.` applies the top judgment.**
  - A type annotates the judgment below it: a literal converts to it, and
    anything else must already have it, since nothing converts implicitly.
  - An operation dispatches on the judgments below it.
- **`+`** works by its operands' types:
  - two literals emitted last fold into one literal, at compile time;
    integers add, and decimals align to the longer fraction, exactly;
  - a literal takes the other operand's type, by that type's conversion;
  - then both types must agree: f64 adds with `f+`, while i64 and money add
    with the checked `+`, which traps on overflow at run time.
- **Each type converts a literal once,** for every operation:
  - to i64: an integer literal as it is; a decimal literal is an error;
  - to f64: either kind of literal, as a float;
  - to money: an integer literal is whole units, times 100; a decimal
    literal must have at most two digits after the point, and becomes cents
    exactly, never through a float.
- **At `;`,** a literal no context typed defaults: an integer to i64, a
  decimal to f64. A type or an operation left unapplied is an error.

Errors are traps: 23 when no clause matches or types disagree, 40 for a word
the prototype does not know, 41 for a literal that cannot become its type.

## What it found

1. **Patching literals in place keeps lowering simple.** Code order always
   matches judgment order, so the stage never shuffles the stack. Folding
   only has to retract the last literals emitted.
2. **Stage 1 applies every word, so the surface can annotate but not build
   types.** `19.99 money` lowers to `19.99 money .` and annotates, but
   `100 i64 vec` would lower to `100 i64 . vec .` and apply `i64` to 100.
   Building a type in the surface needs either the explicit form or some
   syntax for it. This is a question for the design.
3. **Cell stores must be aligned,** and a literal's value is not, so a
   placeholder's value is written a byte at a time.

## Not yet

- **Words calling typed words.** There are no instances per input types
  yet, so a typed body calls no other words, and typed words are called from
  ordinary March, where the checker types their results from their code.
- **Type values as data.** The type numbers are fixed. Types as
  content-addressed values, with type constructors such as `vec`, come next.
- **Families as declarations.** `+` is hard-coded; clauses with signature
  patterns, lifting, and other operations come later.
- **Showing money.** Money shows as its cents.
- **Limits.** A body holds at most 64 judgments, and errors carry no
  position in the source.

The prototype is in `seed/system.march`, under "Staged types, a
prototype", and its tests are in `tests/staged.rs`.
