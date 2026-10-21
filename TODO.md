# TODO

What is to be done in March, march8 first, in order. The design docs
describe each item; this is the list. What march8 has not built at all is
march8/docs/MACHINE.md, "Not yet".

## Bugs

- **A one-letter symbol in a bracket is a variable.** `< a' >` reads as
  `< a >`, a type variable, so `ff: < a' > [ drop 1 ] ; 7 ff` leaves 1:
  the reader gives `a'` and `a` the same token, a name that does not
  apply, and in a bracket the stage takes any one-letter name as a
  variable. The token should say a symbol was written. (2026-10-21)
- **A known array of types shows as values in messages.**
  `max' ( i64 i64 ) clauses#` says `( a value a value ) (2 type vec)`:
  `value_text` (march8/src/stage.rs) has no type names to show.
  (2026-10-21)

## Next

- **`width#`'s test and `quote-power`'s cases in March,** as the identity
  search now is (doc/design/SYMBOLIC-FORTH.md 6.6, 22).

## Then

- **Traps at run time,** and their handlers, `trap`, `kind` and `why`
  (SYMBOLIC-FORTH.md 6.29). `fail` while compiling is built.
- **The rest of Low Stack** (SYMBOLIC-FORTH.md 7), its steps 2 a
  signature's variables, 3 brackets, 4 strings, 5 letters as places, and
  7 the litmus test with no exceptions.
- **A `fold` over a quotation,** so March walks one past about sixty
  elements, depth's bound (SYMBOLIC-FORTH.md 6.6).

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
