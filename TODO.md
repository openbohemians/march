# TODO

What is to be done in March, march8 first, in order. The design docs
describe each item; this is the list. What march8 has not built at all is
march8/docs/MACHINE.md, "Not yet".

## Bugs

- **A known array of types shows as values in messages.**
  `max' ( i64 i64 ) clauses#` says `( a value a value ) (2 type vec)`:
  `value_text` (march8/src/stage.rs) has no type names to show.
  (2026-10-21)

## Next: Low Stack running

Low Stack first; High Stack is dressing on top of it. March is to build
itself in time: do in March what can be, and keep the Rust a stone's
throw from it (Thomas, 2026-10-21).

- **The rest of Low Stack** (SYMBOLIC-FORTH.md 7), its steps 4 strings,
  unsealed as sequences now are (6.14), 5 letters as places, and 7 the
  litmus test with no exceptions. Each the way steps 2 and 3 were
  (2026-10-21): the reader writes the form as Low Stack does, the stage
  stops reading it, and a core word gives it its meaning. `str#` is not
  a word yet.

## Then

- **Traps at run time,** and their handlers, `trap`, `kind` and `why`
  (SYMBOLIC-FORTH.md 6.29). `fail` while compiling is built.
- **`width#`'s test and `quote-power`'s cases in March,** as the identity
  search now is (SYMBOLIC-FORTH.md 6.6, 22).
- **A `fold` over a quotation,** so March walks one past about sixty
  elements, depth's bound (SYMBOLIC-FORTH.md 6.6).
- **One thing, a sequence:** a quotation and an array as one, a kind
  saying what `!` makes of it (SYMBOLIC-FORTH.md 6.30; Thomas,
  2026-10-21). After Low Stack, since it changes what a value is; its
  open questions are there, a number's elements, quotations at run time,
  and their effects.

## To think on

- **List edits by arrows,** from SYMBOL (Thomas, 2026-10-09). Its ASCII
  pairs point the way the elements move: `+>` and `<+` add an element at
  the front and the end, `->` and `<-` remove a suffix and a prefix if
  there, `<->` both, `~>` and `<~` interleave, `><` concatenates and `<>`
  pairs, though `<>` is now `dip` (2026-10-15) and `><` joins a
  signature's two brackets (2026-10-20). A typeable system, but it must
  be checked against `<` and `>` as brackets (a word that only contains
  them is no bracket), and against what March's words already say
  (`insert`, `remove`, `concat`). Arrows as names need a word to be what
  is between spaces, so taking them keeps brackets apart for good
  (Thomas, 2026-10-21); a word of brackets alone read as brackets (6.8)
  would still leave every arrow with a `+`, `-` or `~`.
- **The symbol for the verb `map`, and a map form,** `{| … |}` perhaps,
  as `[| … |]` is the quotation's (SYMBOLIC-FORTH.md, the end of 6).
- **Brackets touching words,** `<string>` (SYMBOLIC-FORTH.md 6.8; Thomas,
  2026-10-20: "let's just stick with spacing", to revisit). Taking the
  arrows above would settle it.
