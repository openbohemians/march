# TODO

What is to be done in March, march8 first, in order. The design docs
describe each item; this is the list. What march8 has not built at all is
march8/docs/MACHINE.md, "Not yet".

## Bugs

- **A known array of types shows as values in messages.**
  `max' ( i64 i64 ) clauses#` says `( a value a value ) (2 type vec)`:
  `value_text` (march8/src/stage.rs) has no type names to show.
  (2026-10-21)
- **A missing key fails as a memory fault.** `{ a: 1 b: 2 } c' at` says
  `Error: Memory`, where it should trap, so a handler can catch it.
  (2026-10-23)

## Next

Low Stack runs (SYMBOLIC-FORTH.md 7, all its steps built 2026-10-22):
the core and every example run lowered, to the same code. High Stack is
dressing on top of it. March is to build itself in time: do in March
what can be, and keep the Rust a stone's throw from it (Thomas,
2026-10-21).

- **The rest of bare bodies** (SYMBOLIC-FORTH.md 6.32). Built
  2026-10-24: `name: … ;` lowered as `name [ … ] ;`, a bracket in a body
  a value, a signature a check applied, `sq: < i64 >. dup * < i64 >. ;`,
  the first a clause's inputs and the last what it leaves, `><`
  retired, the core, the examples, the tests and the docs moved to it,
  and `[ … ] !` running a quotation while compiling, `io.file.embed` gone.
  To come: a body reaching below its first check, `ff: < i64 >. + ;`,
  since a check says only what it says, where march8 says 1 is in reach;
  a check tested at run time, where a value's type is not known while
  compiling; a guard in a check that is not a body's first; and a map
  left by `[ … ] !`, read back, for data read while compiling.
- **Where a trap happened,** reported from a table beside the code, since
  a position in the code would change a word's identity with its layout.
- **`width#`'s test and `quote-power`'s cases in March,** as the identity
  search now is (SYMBOLIC-FORTH.md 6.6, 22).
- **A `fold` over a quotation,** so March walks one past about sixty
  elements, depth's bound (SYMBOLIC-FORTH.md 6.6).
- **Defining types** (TYPES.md 2.11, decided 2026-10-22): records as maps
  of types, and types of their own, each a subset of its representation
  that forgets itself where the representation's word is applied, a
  clause of its own keeping it, made by `own` and named by `;`,
  `celsius: f64 own ;`. A record value is written as a map is, and a
  field read by `at` or a path (decided 2026-10-23). `;` defines within
  its scope, so a record type's fields are its entries, where march8's
  `;` in braces defines into the namespace and refuses a one-letter name
  such as `x`, which a field reached by `p.x` could take (6.32). Still
  open: the warning where two types of their own are forgotten together. Kinds
  wait on it. A record is also how a caller
  names a word's inputs, `{ term: 72 rate: 0.2 } payment`, opened into
  places where the word is applied (SYMBOLIC-FORTH.md 6.31, decided
  2026-10-23).
- **Kinds as types,** made where defined, `err.file.notfound: { error#:
  "file not found" } trap ;`, named by `;`, a handler taking its kind
  and every kind beneath it, the core's under `err`, the compiler's and the
  machine's moved there, `fail` with its message on top
  (SYMBOLIC-FORTH.md 6.29, decided 2026-10-22). Their names are to be
  chosen, and a failure's data beyond its message is to be designed.

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
- **Combinators before names** (doc/design/COMBINATORS.md, 2026-10-24).
  Names would become every word's form (Thomas), since they are easier to
  think in, and a named bracket factors only whole. A pair used twice,
  where FORTH juggles most, `bi_2` does without them. The family by a
  rule: three words, `cleave`, `spread` and `apply`, a width by subscript
  (built, `bi_2`) and a count by superscript, `cleave^3` for `tri`, each
  word's own convention by a clause of `power#` (a superscript a
  convention, as a subscript is, decided 2026-10-24, SYMBOLIC-FORTH.md
  6.21).
- **The symbol for the verb `map`, and a map form,** `{| … |}` perhaps,
  as `[| … |]` is the quotation's (SYMBOLIC-FORTH.md, the end of 6).
- **Brackets touching words,** `<string>` (SYMBOLIC-FORTH.md 6.8; Thomas,
  2026-10-20: "let's just stick with spacing", to revisit). Taking the
  arrows above would settle it.
