# TODO

What is to be done in March, march8 first, in order. The design docs
describe each item; this is the list. What march8 has not built at all is
march8/docs/MACHINE.md, "Not yet".

## Bugs

None known.

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
  `[ … ] !` running a quotation while compiling, `io.file.embed` gone,
  and a body reaching below its first check, `ff: < i64 >. + ;`, where
  its effect is sure; and a check tested as the program runs, where it
  cannot be decided while compiling, a guard in any check among them
  (2026-10-26). To come: such a body whose effect is not sure, a family
  of clauses of different lengths in it, still refused where it is
  applied; and a map left by `[ … ] !`, read back, for data read while
  compiling.
- **Where a trap happened,** reported from a table beside the code, since
  a position in the code would change a word's identity with its layout.
- **`width#`'s test in March,** as the identity search and `power#` now
  are (SYMBOLIC-FORTH.md 6.6, 22; `power#` built 2026-10-26). Not small:
  a word handed code is known by the code on the stack where it is
  applied, which March code does not see, and a word that only moves
  values by its clauses, through the words they apply.
- **The rest of records** (TYPES.md 2.11). Built 2026-10-24: a map of
  types a record type, `;` in braces making an entry, a map literal of
  symbols undecided while its values are alike, a record where one is
  wanted and a map else, fields read by `at` and by a path from a
  bracket's name, `p.x`, or from a word that takes nothing, `cfg.port`
  (2026-10-26), and set by `put`, `r n' 5 put`; a record forgetting
  into a map where one is wanted, and so `at` with a key known only as
  the program runs (2026-10-26). To come: an entry by `;` of anything but
  a type; a record opened
  into places where a word that takes it is applied, for named inputs,
  `{ term: 72 rate: 0.2 } payment` (SYMBOLIC-FORTH.md 6.31); equality of
  records; a known array of map literals left undecided; and a map
  literal of alike values settled as the record a variable stands for,
  `labels { depth: 1 index: 2 line: 3 } push`, now refused unless
  checked, `< lab >.`.
- **The rest of types of their own** (TYPES.md 2.11). Built 2026-10-24:
  `own`, named by `;`, a subset of its representation forgotten where the
  representation's word is applied, a clause of its own keeping it;
  `!` of an array of the representation, `( 1.0 2.0 ) < 2 celsius vec >
  !`, and a minor warning where one word forgets two together, until a
  units system (2026-10-26). To come: a type made of itself, `tree: {
  left: < tree > ; … } ;`, an error for now.
- **The rest of properties** (TYPES.md 3.17). Built 2026-10-26: `prop`
  with a test or none, named by `;`, `i64 ary sorted` one type in any
  order, a check claiming and `!` testing, while compiling or as the
  program runs, clauses chosen by them, kept and forgotten as types of
  their own are; the core's `sorted`, by `ordered`, left by `sort`, and
  `add`, which keeps it. To come: a property inside a type, `i64 ary
  sorted ary`, refused by `!`; `lt` of strings, for `add` and `!` of a
  sorted string array; and the form a sorted array added to is held in,
  a tree kept in order (REPRESENTATIONS.md 3).
- **How a clause is chosen,** TYPES.md 2.8's rule (Thomas, 2026-10-17):
  slot by slot from the top, a value before a type before a union before
  a variable, a subset before the type it is in, replacing march8's
  scores. A name's clauses are one family since 2026-10-24, the
  namespace's own, its imports' in their order and the core's, the first
  found where two fit as well (SYMBOLIC-FORTH.md 16, 17), so it now
  decides across namespaces too. Thomas: "this might be hard to reason
  about … we can always revisit."
- **Kinds as types,** made where defined, `err.file.notfound: { error#:
  "file not found" } trap ;`, named by `;`, a handler taking its kind
  and every kind beneath it, the core's under `err`, the compiler's and the
  machine's moved there, `fail` with its message on top
  (SYMBOLIC-FORTH.md 6.29, decided 2026-10-22). The machine's are named
  as now, under `err`, `err.mismatch`, `err.arithmetic`, `err.io`, the
  symbol kinds retiring (2026-10-24). A failure's data beyond its message
  is to be designed.
- **Equality of tuples and records.** Arrays' is built (2026-10-26), a
  rule in core.march, element by element and only as long as each other.
  A tuple's elements differ in type, so a rule walks them by a known
  position, `1` to its length, which unfolds only once a family applied
  again on a known number does (march8/docs/MACHINE.md, "Not yet"); today
  it becomes an instance, the number a cell. By content identity later,
  once the store holds values canonically, floats among them, 0.0 and
  -0.0 one value and NaN one (Thomas: "state in the merkle-champ must be
  *canonical* for proper comparison").
- **The first compiler in March: `lower`** (2026-10-24), High Stack in
  and Low Stack out, printed, checked against `march8 lower` on the core
  and the examples, a fixed point as system.march's was. It needs no
  bytes, no file written and no code handed to the machine; its tables
  are a record handed along, as a fold hands its value, until the store
  holds them under the paths that name its words. Begun 2026-10-25,
  march8/compiler/lower.march: the reader's first pass, its words,
  strings, comments, brackets, numbers, names, labels and bodies, and
  then a bracket's names and its `_`s as letters, which lowers the whole
  core in 0.6 s as `march8 lower` does, to the byte, the examples too,
  and itself, a fixed point: lowered, it lowers itself to itself. For that
  the machine's limits were raised to 64 MB and a million objects, while
  nothing was freed. To come, though no program here uses them yet: a
  string's holes, `[| … |]`, a quotation's power and the symbol table's
  escapes.
## 2.0

Memory (Thomas, 2026-09-25). The model is built and its rules hold now,
for every feature (doc/design/MEMORY.md, 1 and 3); what is left waits
until a program needs it.

- **The rest of freeing** (MEMORY.md 5). Built 2026-10-25: a count of
  holders, a copy holding a value once more and a drop once less, the
  stage saying which cells are values by their types, every other
  primitive letting go of what it takes, and an edit of a value nothing
  else holds in place: `lower` on itself peaks at 258 KB of values, not
  16.8 MB, and runs 37% slower. To come: a version of a value held
  elsewhere sharing its holds by node, not a pass over its values, which
  `lower`'s token arrays need, being most of that time, and merkle-champ
  a way to walk a version's nodes with their counts (a field taken, by
  the compiler or by a word a programmer writes, was refused 2026-10-26,
  REPRESENTATIONS.md 4); reads needing no count; counting while
  compiling, a copy only read needing no count; what a trap throws away,
  by a table beside the code; and counts changed atomically, once there
  are threads.

## To think on

- **What is done to a value in its lowered type,** `appends#`, written
  where its form must be settled early: a loop's accumulator, a recursive
  input, a stored value (REPRESENTATIONS.md 3, Thomas, 2026-10-26). It
  waits on a second form for arrays.
- **A letter's field, `b.line`** (SYMBOLIC-FORTH.md 6.16, Thomas,
  2026-10-26), as a bracket name's `rd.line` is. A one-letter word is
  refused and a one-letter namespace is not, `b.ff: 1 ;`, so the two
  would clash; refusing one-letter namespaces too would settle it. To
  decide.
- **Representations and sources** (doc/design/REPRESENTATIONS.md,
  2026-10-25). A value held the simplest way first, `vec#`, and moved
  where what is done to it needs another, `tree#` or `list#` (Thomas);
  sharing decides most of it, the analysis freeing needs too. And what is
  undecided comes in only through channels in the state, traced to the
  guards, splits, loops, traps and sizes where it acts (Thomas). Freeing
  by a count of holders, kept while compiling where it can be (Thomas):
  measured on `lower`, all but 0.2% of its bytes, a record's fields
  counted as places, and its peak 258 KB, not 16.8 MB; counted as the
  program runs since 2026-10-25 (Next, above). The forms worth having,
  sequences and maps, each with its verdict, and transactions, merged
  rather than run again, merkle-champ's canonical trees lending themselves
  to it (sections 10 and 11, agreed 2026-10-26).
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
