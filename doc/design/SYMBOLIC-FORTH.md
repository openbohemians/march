# Symbolic Forth

Status: design note, 2026-10-13, from a conversation with Thomas. A first
account of March's lowest form; a precise specification is to come.
March8 reads and writes it: `march8 lower` writes any March in Symbolic
Forth, `--symbolic` reads it, as march8 reads a file ending in `.s4`, and
march8/tests/symbolic.rs is its litmus test (section 5). Revised 2026-10-14: one bracket, a quotation; six
meta-ops; numbers as symbols; signatures as two sequences; holes as sugar;
text made by `string !`; namespaces as maps, with imports and exports;
and no rules of spelling. March8 does not have these yet. Revised
2026-10-15: powers and widths as spellings, recognizers, spelling rules
defined in March, and the names High Stack and Low Stack (section 6).
Revised 2026-10-16: a recognizer is a word whose name is a pattern, its
captures on the stack (section 25); `_` marks a bracket's arguments, so
March has no closures (section 14). Revised 2026-10-17: no bracket is
sealed, and a letter names a bracket's input by its place from the top
(section 14); a capital marks nothing, and a symbol taken as text is a
string (sections 27 and 28). Revised 2026-10-18: why types are words
(section 2). Revised 2026-10-19: the whole of Low Stack, and the order in
which march8 is to reach it (section 7); text as a literal kind, `str#`,
its encoding a type and its structure a representation (section 15);
`[| … |]` for `<| … |>` (section 3); what `#` means (section 17);
numbers as symbols, built (section 7); and the core's width and power as
pattern words found in lookup (section 25). Revised 2026-10-20: a
subscript is an index, which a word's family gives a meaning, and the core
gives one only to a word that moves values or is handed code, a width
(section 22); code means what it means where it was written, each name it
applies expanded to its path there, a symbol is global text, and the core
is a namespace, `core`, which every namespace finds last (section 16); a
quotation is a sequence, and a word's clauses and its effect are data
(6.6); and a recognizer is a rewrite rule, matched where `.` applies a
name, the core's power and width being rules of the core's, so the
syllogism runs lowered (section 25); a signature's two brackets touch,
`< i64 >< i64 >`, so `--` is a comment everywhere (6.8, 6.9); a clause as
data is what `;` takes, inputs, outputs and body (6.6); and a guard in a
bracket is always tested, `[ … ] ?` (section 2). All built. Designed, not
built: failing, `fail`, and handling a trap by a clause of the same word
(6.29). Revised 2026-10-21: `fail` while compiling, and the identity
search in March (6.21, 6.29); a signature is a quotation made one,
`[ … ] sig# !`, as a sequence is made by `seq# !` (6.8); and Low Stack's
steps 2 and 3, each High Stack form read as Low Stack writes it (section
7), and literals unsealed (6.14). Built. Code is data only while
compiling, so a quotation is a sequence then, and at run time only code
(6.30). Revised 2026-10-22: Low Stack's step 4, a string read as a
quotation of its text and code made text, `[ … ] str# !`, and unsealed
as a sequence is (6.15, section 7). Built. And step 5, letters as places,
`_` read as the letter of its place (6.14, section 7). Built. And step 7,
the litmus test with no exceptions: the core runs lowered, and every
example, and compiles to the same code (sections 5 and 7). Failing at
run time and handlers are built (6.29). Decided: a trap's kind is a type,
made where it is defined, `err.file.notfound`, which waits on defining
types (6.29). Revised 2026-10-23: a bracket names its inputs,
`[ rate term | … ]`, read as the letters of their places, seen by a
string or a literal inside it and not by a quotation, and a caller names
a word's inputs by passing a record (6.31). Built, but for records. And
a kind is named by `;`, as a type of its own is, with no `ns#` (6.29).
Revised 2026-10-24: a body is bare, between the label's colon and `;`,
as FORTH's is, so a bracket in it is always a value; a signature is a
check applied, `< i64 >.`, the first a body's inputs and the last what it
leaves, so `><` retires; and `[ … ] !` runs a quotation while compiling
(6.32). Built in march8 (2026-10-24). And a superscript, as a subscript, is a
convention, its meaning the word's family's, the core giving defaults by
kind where they are useful (6.21, 22).

## 1. What it is

Everything in Symbolic Forth is a symbol, a quotation, or a meta-op
(Thomas, 2026-10-13; the one bracket and the meta-ops, 2026-10-14):

- **A symbol** is a name, `dup`, `Snake`; a number, `2`; or, quoted, a
  symbol with spaces in it, `'this is one symbol'`, which is what text is
  before a type structures it. A symbol means nothing until it is applied
  or structured, and a number is no exception: `2` is the symbol `'2'`,
  and no rule of spelling reads it (Thomas, 2026-10-14). `2 i64 !` makes
  it an i64; `2 int# !` an integer literal, whose type what takes it
  decides (TYPES.md 2.10).
- **A quotation**, `[ … ]`, the one bracket: an ordered set of symbols,
  held, which `.` applies. Nothing here is a data structure until a type
  makes it one: `[ … ] seq# !` is a sequence, and a map is no fundamental
  of its own, but a set of pairs read as keys and values, `[ … ] map# !`.
  A bracket is always a quotation, so its kind is never left to what
  takes it (Thomas, 2026-10-14). That matters, since the kinds differ in
  how their code runs: a quotation is held, and runs when applied, perhaps
  many times, leaving its values loose, while a sequence is made once,
  where it stands, its values gathered into one. A number may wait for
  what takes it, since its type changes only how it is held.
- **The meta-ops**: `[` and `]` mark a quotation; **`.` applies** the
  value on top, a symbol the word its name is defined as, a quotation
  evaluated; **`?` tests**, applying the value on top as a test, which
  must leave one bool, `x 0 int# ! lt ?`, and in a signature is a guard
  (section 3); **`!` structures** the value below by the type on top, a
  number symbol into a concrete i64, `2 i64 !`, or a quotation into a
  sequence, `[ … ] seq# !`, and a quotation on top it runs while
  compiling, `[ … ] !` (6.32); and **`;` defines**, binding the name below
  to the value on top, `sq [ dup . * . ] ;`. `.` is the store's one read,
  and `;` its one write.

Besides these the reader knows only white space, the quotes of a symbol
with spaces in it, `'…'`, a quote inside doubled, `'it''s'`, and comments,
from `--` to the line's end. A meta-op stands alone between white space,
so inside a symbol `.`, `?`, `!` and `;` are characters. No rule of
spelling is left in reading: a name means what is done with it, never how it is written
(Thomas, 2026-10-14). The one place spelling counts is lookup: `.` applies
a dotted name by its path, `math.pi .` (6.16).

A side note on the equivalence: every meta-op but `.` could be a
primitive word applied, `?` as `test .`, `!` as `convert .`, `;` as
`def .`, and `]` as `close-bracket .` (a word that, as FORTH's immediate
words do, runs even while a set is being gathered). A primitive is needed
either way, so the lowest level takes these few as meta-ops; what can be said is the same (Thomas,
2026-10-14). `.` cannot be a word applied: applying it would need
application already.

```
1 i64 ! 1 i64 ! + .
sq [ dup . * . ] ;
sq [ i64 . ] sig# ! [ i64 . ] sig# ! [ dup . * . ] ;
fib [ n var# . ] sig# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
Closed Open door . Close door . Lock door . Locked eq? .
```

Nothing applies unless a dot says so: `3 int# ! dup .` is 3 and 3, and
`3 int# ! dup` leaves 3 and the symbol `dup`. A definition is data and
`;`: a symbol, what it names, and between them a signature if it has one
(section 3), `sq [ dup . * . ] ;`, which is FORTH's `: sq dup * ;` turned
postfix.

Its nearest kin are Joy, where a program is a list and `i` applies one,
and FORTH's own words; the difference is that every name is a symbol, and
`.` is the only application.

## 2. Types

The only base things are literals and symbols (Thomas, 2026-10-12).
Everything else is structure, and words build it, types included: `i64 .`
is the type i64, the word `i64` applied, and `string . i64 . map .` a map
from strings to i64s. A type that bottoms out might show as `i64'`, were
it ever shown, but it never is, since every type is given by a word: even
`i64` might one day be `64 int .`, as `string` might be `utf8 str .`. A
type variable is given by a word too, `n var# .` (Thomas, 2026-10-14),
where above the lowest form one lowercase letter is one in a signature,
and in a body names an input by its place (section 14).

Types are words because a type is a construction (Thomas, 2026-10-18).
Were types symbols, only the simplest would be one, `string` or `i64`;
the rest are built, and built by no word they would be elaborate symbols,
`(string*)` or `{sym:str}`, which a rule (6.25) would rewrite
anyway: a word again. So a base type is a primitive word, as a primitive
operation is, and every other type is made by words from those. A type's
written form is its construction, `string . i64 . map .`, as code is
data.

Weighed on the way and left: a base type as a symbol structured, `i64
type# !`, which ends in a word, `type#`, all the same; a meta-op looking
names up among types kept apart from words, `i64 $`, so that a type and
a verb might share a name, at the cost of a second namespace, a seventh
meta-op, and a prime on every symbol to match in a signature; and
prototypes, a type written as an example of its values, `( "" )`, which a
signature already reads as a value to match. Prototypes survive as the
tuple type, `( i64 string )`, a sequence of types read position by
position, as a signature reads its slots. That a type takes a name a verb
might want, `mail`, is left to naming: a capital marks nothing (6.27), so
`Mail` is another name. In the core only `map` stings, the type and the
verb, so the verb is to have a symbol of its own, and types stay
lowercase (Thomas, 2026-10-19). Which symbol is open: APL's each, `¨`,
which `\map;` would write, or Mathematica's `/@` in ASCII. `or`, and
`and` and `not` to come, are no clash: a union is `or` over membership,
one meaning at two levels.

So types need no spelling of their own, and types and words are one
namespace. In a signature a type must be given, `i64 .`, not named: there
a symbol left bare is a value to match, `Open`, so a bare `i64` would be
read by its spelling, or by what happens to be defined. A test is marked
too, `positive ?`, a guard (6.20). Each role has its mark:

```
[ i64 . ] seq# !                                 -- applied: a type
[ Open ] seq# !                                  -- bare: a value to match
[ i64 . positive ? ] seq# !                      -- tested: a guard
[ i64 . [ 2 int# ! mod . zero ? ] ? ] seq# !     -- a quotation as a test
```

A type is data, and is not called. Structuring a value by it is the
meta-op `!`: `2 i64 . !`, or, since `!` takes a type's symbol too,
`2 i64 !`, where a symbol can only name a type. A type converts a symbol
by the clauses under its name, a string by the number it writes, `"42" i64
!` being 42 or nil; a number is the same act, `42 i64 !` reading the
symbol `42`, so parsing is converting. A class checks, `x num !`; a
sequence of types gives several values their types, `x y [ i64 . f64 . ]
seq# ! !`. Which mark calls and which converts is only a choice: `!`
would serve as well for calling, a command (Thomas).

## 3. Structure

The lowest form has one bracket (section 1). March8 still reads four,
which become spellings above it (decided 2026-10-14, not built):

- **`[ … ]`** is the quotation itself: its words, unevaluated.
- **`( … )`** is a sequence literal, `[ … ] seq# !`: what its code
  leaves, ordered, repeats allowed, and undecided, like a number, until
  something takes it as a tuple, a vec or an ary (Thomas, 2026-10-14;
  TYPES.md 2.10). Its code takes what it needs from below, and the
  sequence gathers what it leaves: `1 2 [ + . ] seq# !` is `( 3 )`. How
  many it takes and leaves is the code's effect, which the stage knows
  while compiling (Thomas, 2026-10-14). `[ + ] seq# !` takes nothing: it
  holds the symbol `+`, since nothing applies without a dot. A `_` in it
  is one of its arguments, taken where it stands (6.14).
  Built in march8 (2026-10-21): `5 ( 1 + )` is `( 6 )`.
- **`{ … }`** is a map literal, `[ … ] map# !`: a set of pairs (section
  1), undecided, and taking from below, in the same way (Thomas,
  2026-10-14).
- **`< … >`** is code run while compiling, whose values are a type or a
  signature, `< string i64 map k >`. Applied, `< a a gt? >.`, it checks
  the values on top where it is applied, and a clause's signature is the
  check its body begins with, its outputs the one it ends with, `max: <
  a a gt? >. drop < a >. ;` (6.32). It needs no kind of its own: code run where it
  stands whose values are gathered is a sequence, so `< … >` is
  `[ … ] seq# !`, a sequence of types.
- **`"…"`** is a string, with holes of code: `"\_ apples"`. In the lowest
  form text is a quoted symbol, `'…'`, which a type structures, and a
  string with holes is a quotation of its pieces made text,
  `[ '1 + 2 = ' 1 int# ! 2 int# ! + . ] str# !`, as `seq# !` makes a
  sequence of one (6.15).
  Built in march8 (2026-10-22): `5 "\[ 1 + ]"` is "6".
- **`[| … |]`** is the lowest form's quotation written inline, a pure
  container of symbols (Thomas, 2026-10-14): its contents are read as
  Symbolic Forth, so nothing applies without a dot, a number is a symbol,
  and a capital marks nothing. It lowers word for word, `[| math.pi
  math.e |]` being `[ math.pi math.e ]`, and is the intermediate form's
  escape from its own rules of spelling, as Lisp's `quote` is. It writes
  lists of names, `[| math.* io.file.read |]` (6.17), and code as data
  as it will be stored. It was `<| … |>` until 2026-10-19 (Thomas): what
  it makes is a quotation, written `[ ]`, where `< >` is code run while
  compiling, and the `|` alone says the contents are Low Stack, as
  Template Haskell quotes code in `[| … |]`; `<| |>` read as pipes in F#,
  Elm and Elixir. A map form, `{| … |}` perhaps, is to come. Built in
  march8 (2026-10-15), as `[| … |]` on 2026-10-19, closed by `|]` alone.

A signature in the lowest form is a check in the body, a quotation made a
signature and applied, `[ a* ] sig# ! .`, and needs no divider (6.32):
`;` takes a name and a body, and the body's first check is the clause's
inputs, and its last, after that, its outputs. One gives the inputs
alone, and the stage finds the outputs; `[ n var# . ] sig# ! . … [ ]
sig# ! .` declares that there are none:

```
sq [ dup . * . ] ;                                   -- name, body
fib [ [ n var# . ] sig# ! . … ] ;                    -- inputs, body
sq [ [ i64 . ] sig# ! . dup . * . [ i64 . ] sig# ! . ] ;   -- inputs, body, outputs
```

With no divider, `--` begins a comment everywhere, inside a quotation
and a bracket as well (6.9).

Until 2026-10-24 the whole signature came before the body, two brackets,
`[ a* ] sig# ! [ b* ] sig# ! [ … ] ;`, which High Stack wrote touching,
`< i64 >< i64 > [ … ]`, `;` telling them apart by type (Thomas,
2026-10-20: "I like the full signature up front"). Written in the body,
`foo: [ < i64 i64 > … < i64 > ] ;`, was set aside then: a bracket would
be read by where it stands, and in Low Stack `;` would have to find that
shape at both ends of the body; and `foo: < i64 i64 > [ … ] < i64 > ;`
split the signature around the body. Both are so now, by Thomas's
reading of a signature as a check, which holds where it is applied, and
the outputs could not be checked at the start (6.32): `;` finds a check
at each end, a bracket made by `sig#` and applied.

A signature has two kinds of hole. Values its code takes from below must
be known while compiling, since a type depends on no value known only at
run time (TYPES.md 2.4), so `T [ ary . ] seq# !` needs T a type by then.
Type variables, `n var# .`, stay open: each use fills them by matching,
which is what makes a word generic (TYPES.md 3.14).

Inside each, the code is Symbolic Forth too.

## 4. The intermediate form

March8 reads an intermediate form over Symbolic Forth (TYPES.md 2.2). It
marks the rarer things: code applies words far more often than it passes
symbols, so a name applies by itself and a symbol is spelled as one
(DOTS.md). It lowers to Symbolic Forth by spelling alone, and the spelling
rules live here, not below:

| Intermediate | Symbolic Forth | |
|---|---|---|
| `dup` | `dup .` | a name applies |
| `Snake` | `Snake .` | a capital marks nothing (6.27) |
| `+'` | `+` | a prime marks a word passed |
| `fib:` | `fib` | a label is a symbol |
| `sq: dup * ;` | `sq [ dup . * . ] ;` | a label's body runs to its `;`, a quotation (6.32) |
| `a` | `a var# .` | one lowercase letter is a variable |
| `[ 2 + ]` | `[ 2 int# ! + . ]` | a quotation |
| `[ 2 + ].` | `[ 2 int# ! + . ] .` | a dot at a word's end applies once more |
| `2 i64!` | `2 i64 . !` | `!` structures by a type |
| `positive?` | `positive ?` | `?` tests |
| `10`, `1.5`, `1:2` | `10 int# !`, `1.5 dec# !`, `1:2 ratio# !` | a number's spelling gives its kind |
| `< i64 >.` | `[ i64 . ] sig# ! .` | a signature applied is a check (6.32) |
| `[\| sq math.pi \|]` | `[ sq math.pi ]` | the lowest form inline |
| `( 1 2 )` | `[ 1 int# ! 2 int# ! ] seq# !` | a sequence literal |
| `{ a: 1 }` | `[ a 1 int# ! ] map# !` | a map literal |
| `5 ( _ 1 )` | `5 [ a var# . 1 int# ! ] seq# !` | `_`, an argument, is a letter |
| `"\_ apples"` | `[ a var# . ' apples' ] str# !` | a string with an argument |
| `dup_2`, `dup₂` | `dup_2 .` | a name, its subscript an index; for `dup`, the core's width, by a rule (6.22, 6.25) |
| `+^4`, `+⁴` | `+^4 .` | a power, by the word's kind, a rule of the core's (6.21, 6.25) |
| `[ dup + ]^3` | `[ dup . + . ] 3 int# ! power# .` | a quotation's power, a quotation |

A capital marks nothing (6.27): `Snake` is a name and applies, so
`Foo: 1 2 + ; Foo` runs to 3, and a symbol is quoted, `Snake'`.

```
fib: < n >. dup 1 -+ fib swap 2 -+ fib + ;
fib [ [ n var# . ] sig# ! . dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
```

March8 reads it so (2026-10-24), as it read `fib: < n > [ … ] ;` from
2026-10-15.

A surface may come above the intermediate form, as SURFACE.md's headings
were to be. How many stages there are is still to be discovered; Symbolic
Forth is the one at the bottom.

## 5. The litmus test

What cannot be said in Symbolic Forth is not March. Every notation above
it must lower to it, mechanically, and mean the same.

March8 runs the test. `march8 lower` writes March as Symbolic Forth,
keeping its lines, and `--symbolic` reads it. A file of it ends in `.s4`
(Thomas, 2026-10-21: "fitting a low level language, it is a bit of an
assembly its own way, just an abstract one"), symbolic Forth as `.4th`
is Forth, and one of High Stack in `.march`; march8 reads each by its
ending. march8/tests/symbolic.rs
lowers the core vocabulary and the examples, reads them back to the same
tokens, and runs them lowered, the core too: on the core read as Symbolic
Forth, each example compiles to the code it does in High Stack, the
core's words with it, since a word is known by its code, and does what it
did, the summary run as a `.s4` file on its input (2026-10-22). Code as
data is this form: what a program builds as code, it builds in Symbolic
Forth.

## 6. Toward a specification

Settled (Thomas, 2026-10-13):

1. **Text** is a symbol that may hold white space, written quoted, `'this
   is symbols with spaces in it'`, and a type structures it into a
   string. An ordered set of code points would do too, but that is the
   long way round. March8 reads `'…'` as a symbol (28), which `str# !`
   makes text, `'ab' str# !`, as `10 int# !` makes a number, and a string
   as a quotation of its text and code, which `str# !` makes text too
   (section 7, step 4).
2. **Maps** are ordered sets of pairs, not a fundamental.
3. **`!`** structures a value by a type. It was `convert .` first, a
   convenience over a primitive word; it is now a meta-op (5). March8
   reads it as `! .`, as it reads `;` (2026-10-19), and has no word
   `convert`.

Built since:

4. **Symbols at run time** (2026-10-13). A symbol is a value while a
   program runs too, its name, which march8 keeps as an interned string,
   so symbols compare and key maps by identity, and a family applies itself
   to them: `3 dup` leaves 3 and the symbol `dup`, and `"Open" sym!` makes
   a symbol from data. Applying one is choosing a word, which is done while
   compiling, so `.` applies a known symbol only.

Settled (Thomas, 2026-10-14), not built:

5. **The meta-ops**, `[`, `]`, `.` and `!` (section 1). Each but `.`
   could be a primitive word applied; since a primitive is needed either
   way, the lowest level takes them as meta-ops, and `convert` goes.
   Built in march8 for `!` (2026-10-19): the primitive under it is named
   `!`, as the one under `;` is named `;`, so nothing names it but the
   meta-op.
6. **One bracket, a quotation.** `[ … ]` is always a quotation, and
   `seq# !` or `map# !` makes data of one, so a bracket's kind is never
   left to what takes it, since the kind decides when and how its code
   runs (section 1). The quotation is the more fundamental: a sequence is
   a quotation run and gathered, and once run its code is gone. Joy has
   one thing too, a list, which `i` applies.

   **A quotation is taken apart as a sequence** (2026-10-20), so March
   can read code, as it must to do in March what march8 did in Rust: the
   identity search, `width#`'s and `quote-power`'s (21, 22; Thomas: build
   in March, not Rust). Its elements are what Low Stack holds, each a
   symbol or a quotation: `[ swap + ]` is `[| swap . + . |]`, four; `10`
   is three, `10 int# !`; a quotation inside is one, whole. Running one
   splices them in where it is applied, Thomas: "isn't running `[ swap +
   ] .` the same as removing the `[` and `]` and executing as if `swap .
   + .` were on the symbolic stack?" It is: each symbol lands on the
   stack, and each meta-op acts on what is there. Taken apart, nothing
   runs, so a meta-op is held, as the symbol of its name, `'.'` (Thomas:
   "I suppose that's fine"), and put back, it acts again. It is the core's
   families that take a quotation, `length`, `at`, `slice`, `concat` and
   `insert`, and so `first`, `last`, `rest` and `most`, rather than Joy's
   `uncons` and `cons`. Its names were expanded where it was written
   (16), so a name taken out is its path, `math.dbl'`, and quotations put
   together mean what each meant. Only while compiling, as a quotation has no value at run
   time; and a family applied again to one unfolds there, its guards
   decided by the values, so a word walks a quotation by recursion.
   Built in march8 (2026-10-20). A bracket in one is three elements,
   its quotation, its kind and `!`, `[ … ] seq# !` (section 7, step 3).
   A string is three too, `[ … ] str# !`, its text a symbol (step 4).
   `each` over one is unrolled, its quotation applied to each element in
   turn, so `fold` walks one past depth's bound of about sixty, as
   `shuffles#` does (2026-10-26). Not yet: a quotation holding the symbol
   `'.'` pushed, which would be one with the meta-op taken out. At run
   time a quotation is code only, never taken apart (30).

   **A word's clauses are data too** (2026-10-20; Thomas: "that seems
   reasonable"), and its effect, so March reads a word as it reads a
   quotation. `w ins clauses#` gives the clauses of the word w that
   dispatch would choose for inputs of the types ins gives, a type or
   code that leaves them, `< i64 i64 >` or `[ i64 i64 ]`, in the order it
   would try them: those with guards first, as defined, then the best
   without. Dispatch is the stage's, so March asks it rather than writing
   it twice. Each clause is a quotation of what `;` takes after the name,
   each bracket as the code written in it: its inputs, its outputs if it
   gives them, and its body (Thomas, 2026-10-20). So `max' < i64 i64 >
   clauses#` gives two, the first `[ [ a a gt ? ] [ a ] [ drop ] ]`; a
   type given alone is written by its name, `[ i64 . ]`; a clause with no
   signature is its body alone, `[ [ + ] ]`; and a primitive has no
   clauses, its code being the machine's. Brackets lowered (section 7,
   step 3), a clause still holds each as the code in it, which `sig# !`
   makes what `;` takes. `w effect#` gives how many
   values a word or a quotation takes and leaves, `max' effect#` being 2
   and 1, as the stage knows them. Both run while compiling. Built in
   march8 (2026-10-20); a clause of two, its signature with `--` and its
   body, until `><` (8). Given values as well as types, `unit' [ i64+' ]
   clauses#`, a value pattern for another value is not chosen
   (2026-10-21), so March asks whether a fact is stated. The identity
   search is March over them (21, built 2026-10-21), with `fail` (29).
   Next: `width#`'s test and `quote-power`'s cases (22).
7. **Numbers are symbols.** No rule of spelling reads them in the lowest
   form: `10 i64 !` gives the type, and `10 int# !`, `1.5 dec# !` and
   `1:2 ratio# !` the literal kind, undecided until something takes it,
   which is how the intermediate form writes its numbers. So no
   interpretation is hidden: only symbols, and their application and
   structuring. Built in march8 (2026-10-19): `--symbolic` reads `10` as
   the symbol it is, `'10'` on the stack, and `lower` writes each number
   with its kind. Reading `10 int# !`, march8 folds it into the literal
   `10` is in the intermediate form, `!` done early: the stage would make
   the same literal while compiling, and made at once it is one wherever
   a literal is looked for, a count or a value to match. A symbol not of
   the kind, `1.5 int# !`, is left for `!`, which says why. Inside `[| …
   |]` a number is a symbol too (3).
8. **Signatures are two**, inputs then outputs, `[ a* ] [ b* ]`, each a
   quotation made a signature, `sig# !`; one gives the inputs alone.
   `;` takes them before the body (section 3). They were sequences,
   `seq# !`, until 2026-10-21 (Thomas chose `sig#`): a sequence's code
   runs where it stands, its letters places and its tests run, where a
   signature's runs over types, its letters type variables and its tests
   guards, so which it is must be said where the quotation is made, as a
   bracket's kind always is (6.6), not left to `;`. High Stack writes them as
   two brackets touching, `< a a gt? >< a >`, so `><` is `>` then `<`
   and needs no meaning of its own (Thomas, 2026-10-20, from the bowtie
   `⋈`, which it looks like; the glyph stays out of source). An empty
   bracket gives no inputs, `< >< i64 >`. Built in march8 (2026-10-20),
   the core and every test moved to it. Since 2026-10-24 a signature is a
   check applied, `< i64 >.`, a body's first its inputs and its last what
   it leaves, `sq: < i64 >. dup * < i64 >. ;`, so `><` retires (32),
   which march8 says where it is written; built that day, the core and
   every test moved to it. A bracket still stands apart,
   `< string >`, not `<string>`: a word is what is between spaces
   (Thomas, 2026-10-20: "let's just stick with spacing", to revisit).
   Brackets that touch words were weighed and set aside: lined-up cases
   read as columns either way, and patterns, `/([0-9]+)kg/`, and names,
   `<#`, `>string`, would need rules of their own. If nested data comes
   to need it, a narrower step is open: a word of brackets alone is those
   brackets, `]]` as `] ]`, of which `><` would be one case.
9. **Comments** run from `--` to the line's end, everywhere, since no
   divider is needed in a signature: in High Stack too, inside a bracket
   as well (2026-10-20). Until then High Stack kept `--` as the divider
   in a signature's bracket, `< i64 -- i64 >`, FORTH's stack comment
   (Thomas, 2026-10-15: minor, to reconsider later); march8 says so when a
   bracket a `--` begins a comment in is never closed. A line of three or
   more `-` and nothing else opens a block comment, and a line of them ending in a period,
   `---.`, closes it (Thomas), so a block reads as a ruled box:

   ```
   -------------------------
   Write block comments here
   -------------------------.
   ```

   Built in march8 (2026-10-15).
10. **`seq#` and `map#`**, the sequence and map literals: what a
    quotation's code leaves, collected, `[ … ] seq# !`, which `( … )`
    writes, or paired, `[ … ] map# !`, which `{ … }` writes; their code
    takes what it needs from below, and they are undecided until taken,
    as `int#` is. A sequence that waits to be made is a quotation, held
    until `seq# !`. A signature is made by `sig# !`, which `< … >`
    writes, its code run over types (8).
11. **Variables are given by a word**, `n var# .`, as types are, so no rule
    of spelling is left in the lowest form; one lowercase letter is a
    variable only above it. A body's letter is given by the same word,
    `b var# .` (section 7). Built in march8 for signatures (2026-10-21):
    the reader writes every letter as `a var# .`, so a letter's symbol,
    `a'`, is the symbol in a signature too, a value to match. And for code
    (2026-10-22), where a letter is an input by its place (14).
12. **Quoting.** A quote inside a quoted symbol is doubled, `'it''s'`,
    the one rule the reader needs to find where `'…'` ends. A symbol holds
    its characters as they are, newlines among them. Escapes, `\n;` and
    `\times;`, are spellings of the intermediate form, decoded by its
    reader, so the lowest form has none, and a symbol and the string made
    from it hold the same text.
13. **A pull takes.** `_` takes a value from below, and a copy is `dup`,
    as everywhere. March8's pulls copied, read from the literal's mark
    (`mark-pick`), so that one in a loop read the same value each pass,
    `10 ( xs [ _ + ] each )`. A `_` belongs to the bracket it is written in
    (14), so in that loop it is the element; a value from outside is
    written in, `( xs [ 10 + ] each )`, or reached on the stack, and
    `mark-pick` goes.
14. **`_` marks a bracket's arguments** (Thomas, 2026-10-16). A bracket
    with k `_`s takes k values from below when it is applied, and each `_`
    puts the next of them where it stands, the first written the deepest:
    `[ _ _ + _ / ]` is an anonymous word of three inputs, and
    `1 2 3 [ _ _ + _ / ] .` is `1 2 + 3 /`. A quotation is applied by `.`
    or by what takes it, so its `_`s are filled then; a sequence, a map and
    a string are applied where they stand, so theirs are filled there:
    `5 ( _ 1 )` is `( 5 1 )`, and `1 2 "\_ and \_"` is "1 and 2". A word's
    body is a quotation, so its `_`s are the word's inputs. A `_` belongs
    to the innermost bracket it is written in, so `( xs [ _ 1 + ] each )`
    adds 1 to each element, the element being the inner quotation's
    argument. The arguments are taken first, so the code may still reach
    below them: `[ _ + ]` applied is `+`. Scala's `_ + _` and Clojure's
    `#(+ %1 %2)` are the kin.

    Nothing is captured: a quotation holds no value from where it was
    made, so March has no closures. A value from outside reaches a
    quotation on the stack, as in Joy, or is written in when it is known.
    No closures means no values from outside, not no names: a name in a
    quotation means what it means where the quotation was written (16),
    settled while compiling (Thomas, 2026-10-20).
    The stage moves the arguments while compiling, a value known only at
    run time through the scratch stack, as `dip`'s is. In the lowest form a
    `_` is a letter (Thomas, 2026-10-19): lowering counts a bracket's `_`s
    and writes each as the letter of its place, the first written the
    deepest, so `[ _ _ / ]` is `[ b a / ]`, by spelling alone, since a
    bracket uses `_`s or letters, never both. So Low Stack has one way to
    name an input, and `_` is High Stack's:

    ```
    5 ( _ 1 )                5 [ a var# . 1 int# ! ] seq# !
    1 2 "\_ and \_"          1 2 [ b var# . ' and ' a var# . ] str# !
    [ _ 2 / ]                [ a var# . 2 int# ! / . ]
    ```

    Built in march8 (2026-10-22): the reader counts a bracket's `_`s and
    writes each as its letter, and refuses a bracket with both, and `_` in
    a signature, whose letters are types.

    This replaces holes filled first (2026-10-14): a bracket's values
    written into its code where it stood, a hole in a nested bracket the
    outermost's, lowered by Joy's `cons` and `compose`. A quotation made
    where a value was known only at run time then held it, a closure,
    which every word that moves or takes a quotation would have to carry
    (Thomas: "Aren't they just anonymous words? The `_` are the
    arguments?"). Since this is sugar, another surface may choose
    otherwise with no change below.

    Built in march8 (2026-10-16): a quotation, a word's body, a literal
    and a string each take their arguments, and `mark-pick` is gone. A
    run-time argument used first never moves; one used later waits on the
    scratch stack. `march8 lower` writes `( … )` as `[ … ] seq# !`
    (2026-10-21), and `"…"` as `[ … ] str# !` (2026-10-22).

    No bracket is sealed (Thomas, 2026-10-17): a quotation and a literal
    alike take what their code needs, so `[ + ]` takes two and `1 2 ( + )`
    is `( 3 )`, and concatenation stays composition. A `_` makes an order
    easy, `[ _ foo _ bar ]` for `[ swap foo swap bar ]` when `foo` leaves
    one value; it does not declare what a bracket takes, so `( _ _ + )` is
    `( + )`. Sealing every bracket, each taking only its `_`s, would put
    them in every word that takes an input, `sq: [ _ dup * ] ;`. Built in
    march8 for sequences and maps (2026-10-21): a literal is its code
    applied, as `.` applies it, and what it leaves kept together, so
    its elements are what its code left, from as deep as it reached:
    `5 ( 7 over )` is `( 5 7 5 )`, and `0 ( xs [ + ] each )` a sum, `( 0 )`
    when xs is empty. And for strings (2026-10-22), whose text is among
    what their code leaves, so a hole reaches the text before it:
    `5 "\[ 1 + ]"` is "6", and `"a\[ drop 1 ]"` is "1".

    **A letter names an input by its place** (Thomas, 2026-10-17). In a
    bracket one lowercase letter is the input at its place from the top,
    `a` the top, `b` the next, and so on, and written again it is a copy:
    `[ b a / ]` is `/`, `[ a b / ]` is `swap /`, and `[ b a b + ]` needs
    no `swap` or `dup`. Letters written in the stack's order give the
    plain word, as a stack picture `( b a -- )` reads. A bracket takes as
    many inputs as its highest letter, as q's `{x+y}` does, and drops
    those it does not name, so `[ c ]` takes three and keeps the deepest.
    A letter means the same place in every bracket, then, and code pasted
    from one into another still works, which counting from the deepest
    would not give (Thomas). `x`, `y` and `z` are `a`, `b` and `c`, as
    math names its first three, and every other letter names its own
    place, so `w` is the twenty-third (Thomas, 2026-10-19): the backward
    run, `w` down to `n` as `d` to `m`, would be a rule no one remembers,
    and past three inputs letters from `a` serve. No other letter names
    two places. Math's `i`, `j`, `k` and `n` lie deep, so
    `[ n 2 * ]` takes more than a dozen inputs; the stage refuses it where
    it is given fewer, saying which place `n` is.

    Letters and `_`s count from opposite ends: a `_` is the next in
    written order, the first the deepest, and a letter a place from the
    top, so `[ _ _ / ]` and `[ b a / ]` are both `/`. Each has its use,
    `_` in a string, whose written order is its reading order,
    `1 2 "\_ and \_"`, and letters in code; a bracket uses one or the
    other, and a string is one bracket. In a string a letter is written in
    a hole, `5 "\[ a ] is \[ a ]"` being "5 is 5": `\a` would clash with
    the escapes, `\n;` being a newline while `n` is a place, and `\( a )`
    would be a sequence written into the string. Letters belong to the innermost bracket they are written in, as
    `_`s do, so a bracket means the same wherever it is pasted, and
    nothing is captured. In a signature a letter is still a type variable,
    filled by matching (section 2), so in `at: < a ary i64 >. … < a >. ;`
    the signature's `a` and the body's are two variables. In Low Stack a
    body's letter is `b var# .` (section 7).

    Built in march8 (2026-10-22). A bracket takes its inputs as it is
    applied, so its code may reach below them, and the stage gives each
    letter its input where it is written, the input of the bracket it is
    written in. The letters a quotation or a body begins with are a
    shuffle, done on the stack: each input copied, or moved where it is
    named last, and those in order already never moving, so `[ b a b + ]`
    compiles as `over +`, `[ a b / ]` as `swap /` and `[ c a ]` as
    `swap drop`. Those named once its code has begun wait, those known
    only at run time on the scratch stack, the one used last at the
    bottom, each taken from there where it is used last if it is on top
    and copied before; so letters in the stack's order, as `_`s are,
    compile as the plain words do, and every `_` compiles as it did. A
    literal's and a string's letters all wait, its elements being what its
    code leaves. A bracket given fewer inputs than its deepest letter
    names says which: "`n`, at 1:3, is the 14th input from the top". A
    quotation's power applies its code each time, `[ a 1 + ]^3`, and
    `compose` each quotation that names its inputs, `[ … ] .`, so each
    time its letters are its own and composing is composition still.
    `concat` joins tokens, code taken as data being Low Stack's (6.30), so
    `[ a ] [ a ] concat` is `[ a a ]`, a `dup`.

    Names can be an editor's (Thomas, 2026-10-17, an idea). A letter being
    only a place, a word may carry a key naming its places, and an editor
    show the names where the letters are, `qty price *` for `b a *`, while
    the source holds the letters. Names are then presentation, as Unison
    keeps names apart from code and de Bruijn indices keep a term
    nameless: renaming changes no code, and a name never clashes with a
    word. Where the key is kept waits for an editor (2026-10-19); a
    signature's labels are the likely place, `< qty: i64 price: f64 >`.
    Names in the source came first (31, 2026-10-23): a bracket names its
    inputs, `[ qty price | qty price * ]`, and the reader writes them as
    letters, so Low Stack holds places only, as this would have.
15. **Strings with holes.** `str# !` makes text of a quotation, as
    `seq# !` makes a sequence: it runs the code where it stands and writes
    each value the code leaves, in order, into one string, `'…'` symbols as
    their text and other values as `>string` writes them. It is an
    emitter: one pass into one buffer, with no sequence made first and no
    chain of joins; pieces known while compiling fold. A sequence made a
    string is another thing, the sequence written, `( 1 2 )`. Built in
    march8 (2026-10-16): `"…"` and `[ … ] string !` join their pieces in
    one buffer, the machine's `text-join`, where march8 had joined them
    with `concat` one at a time, copying the growing string at each piece.
    `[ … ] string !` was retired for `str# !` (Thomas, 2026-10-22): a
    bracket's kind is said by a kind, as `seq#` says a sequence's, and
    `string` is a type.
    Its code is not sealed (14): it reaches what `.`'s would, and all it
    leaves from as deep as it reached is written. `"…"` is read as one
    quotation, its text a symbol for each run of it and its holes' code
    between (section 7, step 4), so a hole is code written in, which may
    leave any number of values and reach the text before it. Built in
    march8 (2026-10-22), where until then each hole had left one value
    and reached only its string's arguments and what it made.

    Text is a literal kind, `str#`, as a number is (Thomas, 2026-10-19):
    strings differ along two axes, their encoding, UTF-8 or ASCII, and
    their structure, from an array of code points to a tree, as an RRB
    vector or a rope is, the two ends probably enough. So `"…"` is
    undecided until something takes it, as `10` is, and `str# !` makes it
    so. The two axes differ in kind (agreed 2026-10-19, TEXT.md 1):
    - **Encoding changes which texts a type holds.** Every ASCII text is
      UTF-8 byte for byte, so `ascii` is a refinement of `string`, its
      bytes the same, as `bool` is of i64 (TYPES.md 2.16): nothing
      converts, and an `ascii` clause is chosen before a `string` one
      (TYPES.md 2.8). TEXT.md's flag for a fast path becomes the type.
      Other encodings, UTF-16 or Latin-1, are conversions at input and
      output, so UTF-8 stays the only encoding inside.
    - **Structure changes only what an operation costs,** so it is a
      representation, not a type: a flat array of bytes when small or
      local, as march8's strings are, a tree when large or stored, as
      TEXT.md's sequence in merkle-champ is, chosen by use as tuples' is
      (march8/docs/MACHINE.md, "Representations chosen by use"). A word
      does not say which.

    `str#` settles to `string` where nothing asks for more, as `int#`
    settles to i64. So `str#` makes text and `string` is the type text has,
    what a signature names and a value has at run time, as `int#` makes a
    number and i64 is a type (Thomas, 2026-10-22: "why do we need
    `string`?"): `str# !` makes text of a symbol, `'ab' str# !`, as of a
    quotation, and `[ … ] string !` was retired (above). In march8 `str# !`
    gives a string at once, there being no other text type yet.

16. **Namespaces** are maps all the way down (Thomas): a namespace's
    entries are words, data, or more namespaces, and a dot inside a name
    divides the path, `math.pi` (TYPES.md 3.12). The reader keeps a dotted
    name whole, and until applied it is a plain symbol, passed as `Open`
    is; `.` reads the path, walking the maps to the entry at its end. That
    is the one place spelling counts, and it is lookup's, not reading's. A
    number's dot is never a path, since a number is structured, never
    applied. What it means is explicit, `math . pi at . .`, the map, its
    entry, applied, and `math.pi .` names that. So a namespace is a value:
    `math .` leaves the map, to pass, merge or compare, and in a
    merkle-champ store it has an identity, its content hash, which can pin
    an import (march5). `;` with a dotted name writes at that path,
    making maps on the way, `math.pi [ … ] ;`. Code and data are entries
    alike, the dictionary and the global store being one (2026-09-26), so a
    variable is an entry. Built in march8 (2026-10-15), but for a
    namespace as a value: `;` defines at a path, and a word's names are
    looked up in its namespace where they are written, so a quotation of
    `math`'s that `map` runs still finds `math.sq`. There a namespace is the words
    whose paths it begins, not yet a map, so `math .` and `at` on it are
    still to come.

    **A name means what it means where it was written** (Thomas,
    2026-10-20). Thomas: "if I write `[ 1 2 + ]` in a program, I almost
    certainly want `+` to mean what it means where I wrote it. It would be
    hard to reason about if I passed that quote to another word which was
    defined in another namespace where `+` means something else." So code
    is **expanded** where it is written: once a source is read, each name
    it applies, `+ .`, becomes its path there, `math.+` if `math` defines
    one, else its import's or the core's, and a name found nowhere is the
    namespace's own, defined later or never. Code then means the same
    wherever it runs, a quotation handed to a word of another namespace
    among it. It is no closure: no value is captured, only which word a
    name means (14). Clojure's syntax-quote does the same, writing `map` as
    `clojure.core/map`, and Unison keeps code resolved and shows it by short
    names. Expanding is a walk over code that rewrites names, so March can
    do it itself, once a quotation can be taken apart (6).

    **A symbol is global** (Thomas, 2026-10-20): "symbols are global, not
    namespaced, hence one of the reasons one might do `Human: Human' ;` if
    they want/need to 'spec' acceptable symbols a module recognizes." Only
    a name applied where it is written is expanded, which the code shows by
    its `.`, so no lookup decides it (Thomas: "`dbl'` means `dbl'`, `dbl'
    .` means `math.dbl`"). `dbl'` is `dbl'` everywhere, compared as text,
    and applied from the stack it is a path from the root: `dbl' map`
    inside `math` applies the root's `dbl`, and `[ dbl ] map`, or `math.dbl'
    map`, applies `math.dbl`. That reverses what was first built the same
    morning, a symbol carrying its namespace while compiling: march8 kept
    it on each symbol, and on each quotation by its place in memory, state
    March would have nowhere to keep (Thomas: "how does it track that? A
    symbol is a symbol after all"). A rule's capture is a symbol too, which
    the rule reads as a name where it was applied by `name#` (25).

    **The meta-ops are no names** (Thomas: "the meta-ops might be the one
    thing you don't ever have to namespace -- they are universals. They
    can't be overriden or changed."). The reader knows them and nothing
    looks them up, so expanding leaves them as they are, and defining one,
    `!: … ;` or `';' … ;`, is an error.

    So that a namespace's own word counts wherever its source defines it,
    as its import does (17), a source's definitions are known before it
    is expanded, and its `import#` and `export#` set. A definition's
    signature and body are expanded in the namespace of the name it
    defines, and a bare name defined is the root's, as a symbol is. A name
    a rule matches is expanded to a path where it was written,
    `math.dbl^2`, so the path says where the rule was applied (25). Each
    source is expanded once, so a word a later source defines in a
    namespace leaves code already expanded as it was. Built in march8
    (2026-10-20).

    **The core is a namespace, `core`, which every namespace imports**
    (Thomas, 2026-10-20: "ultimately certain core words won't be directly
    available. To get them the coder would have to import them, making
    that potentially 'unsafe' decision where they can muck about in the
    deeper internals"; 2026-10-24: "core is just the auto imported module,
    importeted into all other namespaces"). A program is written into the
    root, not into the core, and the root begins, in essence, as

    ```
    { import#: [| core.* |] ; }
    ```

    an import being a sequence, since imports are ordered (Thomas: "imports
    are ordered so it has to be a sequence of pairs, not a map"), and
    `core.*` making local, each by its last part, what the core exports
    (17); what it does not export, its
    internals, a namespace reaches by importing it by its path, a decision
    written where it is made, as Go's `unsafe` is a package imported by
    name. So `+` is `core.+`, a path like any other. **A name's clauses
    are one family**, wherever they are found: the namespace's own, then
    its imports' in their order, the core's last (Thomas: "imported
    modules are looked up after the local definitions"). The clause that
    fits best is chosen, as in any family (TYPES.md 2.8), and where two fit
    as well, the one found first, Thomas: "you can't match by just name,
    the types are necessary". So a program's `+: < celsius celsius >. …
    ;` takes two celsius, `2 3 +` is the core's, a literal's own kind
    fitting it best, and the clause's own `+` of two f64s is the core's
    too. A program overrides a core clause at the same signature, which it
    is found first for; a clause of its own that fits less well, `+: < a a
    >.` or one with no signature, does not hide the core's. Thomas
    (2026-10-24): "My only concern is that this might be hard to reason
    about. But let's go with it, we can always revisit if it proves too
    problematic." Order alone was weighed, the first word with a clause
    that fits, and left: `1 2 +` would be celsius in a program with a
    celsius `+`, and where an import stands would change what a literal
    means. Nothing is added to the core, since a program does not write the
    core's words (Thomas: "one should not write core methods"): the core's
    own words still mean the core's, `sum` a sum by `core.+`, so a sum of
    celsius forgets into f64 unless the program's `sum` has a clause of
    its own. `dup: 7 ;` at the root is the program's `dup`, taking fewer
    values, which wins (TYPES.md 3.13), `over` still meaning the core's
    and `core.dup` reaching it. A source applies its own word wherever it is written, its own
    definition included, so `i64: < i64 f64 or > ;` is an error that
    names `core.i64`. Expanded code keeps its paths, and what shows a name
    shows the shortest one the root finds the word by, `dup` for
    `core.dup`: a message, a name taken out of a quotation, `[ swap + ]
    first` being `swap'`, and a capture `name#` reads (25), so `-+^0`
    still meets `< -+' 0 >`. Built in march8 (2026-10-20), the core
    exporting all of its words, and a name's clauses one family since
    2026-10-24, chosen by march8's scores until 2.8's rule is built, a
    name expanded to the first word found. Not yet: which words the core
    exports and how importing the rest is marked, the core a subset in
    time, the words of the system imported by hand, and how namespaces are
    organized, all to be worked out (Thomas). The typed primitives,
    `i64+`, `ary-at`, `quote-at`, `string-concat`, are the natural
    internals; the words lowered code applies, `int#`, `power#`, `name#`,
    stay exported, since lowered code names no import. Nor yet showing
    expanded code by its shortest names, which nothing shows yet, `lower`
    and `fmt` writing source as it was typed.

    **To decide: a letter's field** (Thomas, 2026-10-26). In a bracket
    that names `rd`, `rd.line` reads a field, a path's head found as a
    name before a namespace (TYPES.md 2.11): "that is a parameter
    (essentially a local variable) combined with a literal name `.line`.
    That use had not occurred to me." A letter could do the same, "`emit:
    < parse string >. b a b.line emit-at ;`", but now does not: "no word
    `b.line`: there is no namespace `b`". And "that presents a problem
    because the no namespace could have a one letter name (or at least
    not accessed like this if it did". A one-letter word is refused, `b:
    1 ;` being "`b` is a variable, one letter, and names no word", but a
    one-letter namespace is not, `b.ff: 1 ;` defining `b.ff`. Refusing
    one-letter namespaces too, a namespace being a name and a one-letter
    name a letter, would make a path headed by one letter always a
    letter's field, read as a bracket name's is; the cost, no namespace
    `x` or `m`. Thomas: "at the very least we note is as something that
    needs a decision."

17. **Import and export** (Thomas): a namespace's map has two keys of
    March's own, `import#` and `export#`, named so as not to take words a
    program wants, `csv.export`; a trailing `#` marks March's own names,
    as in `int#` and `seq#`. Setting them is a definition of data:

    ```
    import#: [| math.* io.file.read |] ;     -- each local by its last part
    export#: [| pi e sin cos |] ;
    ```

    `import#` makes names local, each by its last part or by a name
    paired with it, `circle` for `math.pi`, and `math.*` makes local all
    that `math` exports. Imports are ordered, so they are a sequence, of
    pairs where they name (Thomas, 2026-10-24), how a pair is written in
    it to be settled. A bare name is looked up locally, then among the
    imports in their order, the core's last, which every namespace imports
    (16), and the clauses it finds are one family, the clause that fits
    best chosen, the one found first where two fit as well (Thomas,
    2026-10-24). A path is absolute, so it means the same anywhere. A namespace has one import, which holds for all of it
    wherever it is written, as if first, and is written first by
    convention, not by rule (Thomas, 2026-10-15); a second is an error. A
    word's body is compiled when it is applied, not where it is written,
    so its names cannot hang on where an import stands, as FORTH's search
    order does, compiling as it reads. Code that wants other imports is a
    child namespace with its own. `export#` holds the names that may be
    reached from outside,
    and with none, all may. It holds for paths as well as imports:
    reaching an unexported entry from outside compiles, with a serious
    warning, so a word is private but not locked out. A namespace's
    children see its unexported entries, so tests in `math.test` need no
    warning. A namespace's interface is the hash of its exported entries,
    so a change to a private word changes nothing an importer pinned. The
    lineage: march2's `<` imports and `>` exports, never built past whole
    namespaces; march5's exports, interface hashes and lookup order, which
    held namespaces to compile time, as `.` applying known symbols only
    still does (doc/lineage). So a module is a namespace, its map with
    its imports and exports, and needs no construct of its own (Thomas).
    Built in march8 (2026-10-15): imports and exports by lists, `[| … |]`,
    a source's imports run first, two finds of a name are an error, and
    reaching an unexported word from outside its namespace warns as
    severe; and since 2026-10-24, the imports in their order, a name's
    clauses one family. Not yet: the pairs that name an import; and
    interfaces by hash, which wait for the store.

    **What `#` means** (Thomas, 2026-10-19): nothing to the reader. It is
    part of the name, as any character is, and a habit of naming, not a
    rule: a name ending in `#` is March's own, so a program keeps the
    plain word, `csv.export` beside `export#`. March's own are of three
    kinds: the literal kinds, `int#`, `dec#`, `ratio#`, `str#`, `seq#`,
    `map#` and `unit#` (TYPES.md 2.10, 6.21); the words that say what a
    spelling said, `var#`, `power#`, `width#` and `pattern#` (section 7),
    `identity#`, which finds an identity (6.21), and `name#`, which reads
    a symbol as a name where a rule's name was applied (25), and
    `clauses#` and `effect#`, which read a word (6); and a namespace's
    keys, `import#` and `export#`. A vocabulary stays plain,
    the core's types and verbs among it, `i64`, `string`, `map`, `dup`:
    those are words a program uses, not machinery it lowers to. Nothing
    holds a program to the habit, as nothing holds it to a case (27).
18. **`;` defines**, a meta-op (Thomas). Through all of the above one
    word stayed constant, `def`, applied in every program, so it is named
    for what it is, FORTH's `;`: `sq: [ dup * ] ;` is `: sq dup * ;`
    turned postfix, the label's colon opening and `;` closing, and since
    2026-10-24 `sq: dup * ;` is, the colon opening the body as FORTH's
    does (32). It binds the
    name below to the value on top, with a signature between them if
    there is one, code and data alike, and adds a clause to the word of
    that name, as `def` did. A second `;` with a new signature adds a
    clause, and one with the same signature replaces it, as march8 does,
    so a name's entry is one, its family (32); writing an entry as the
    program runs is the global store's write, for the store's design. Built in march8
    (2026-10-15), with the signature before the body, its inputs, then its
    outputs if it gives them, each a bracket (8; one bracket divided by
    `--` until 2026-10-20); since 2026-10-24 the signature is in the
    body, its first check, and its outputs its last (32), and a signature
    before the body is refused, saying where it goes (section 3).

19. **Symbols applied at run time** (Thomas). `.` applies a symbol known
    while compiling, or one its type bounds: a union of symbols, compiled
    as a switch among known words, or a named signature, `adjustment:
    < price >< price >` (Thomas, 2026-09-26), whose words are compiled at
    that shape into a table, FORTH's `execute` with the check done first.
    Any other symbol is data, and runs by immediate mode, through a word
    of the system, as ANS FORTH's `EVALUATE` runs text. In immediate mode
    every value is concrete, so the stage chooses the clause, checks it
    and compiles it for the stack it finds, then runs it; the code
    compiled before checks the stack handed back, or names the shape it
    expects. The compilation is cached, by the symbol and the types it
    met, so immediate mode is March's JIT, with no machinery of its own
    (Thomas). A program that evaluates carries the stage with it, as
    FORTH carries its outer interpreter, and one that does not, does not.
    The language stays static, and the dynamism is FORTH's own, on
    request. Unlike FORTH's, a word is a family whose clause is chosen by
    type, so a word named at run time may have no body compiled for the
    types it meets: that is why the stage must be there. It is the one
    way data becomes code at run time, compiled whole by the stage; a
    quotation is never edited at run time (30).

    A future idea (Thomas): a pure word's results may be cached too, by a
    memoization modifier, which is easy in context-oriented code, where
    the inputs already choose a clause; and the system could choose what
    to memoize by profiling as it runs.

20. **`?` tests**, a sixth meta-op (Thomas): it applies the value on top
    as a test, which must leave one bool. When it runs depends on where
    its inputs are: in code they are on the stack, so `x 0 int# ! lt ?`
    leaves a bool then; in a signature the code runs while compiling,
    over types, so the test is held as a guard on the clause's inputs,
    looking at them without taking them, and is decided at run time,
    unless its inputs are known, when march8 already decides it while
    compiling. So `?` puts the context into play at run time (Thomas).
    Test words lose the `?` from their names, `positive`, `zero`, `lt`,
    and the intermediate form keeps today's spelling, `positive?` being
    `positive ?` as `[ 2 + ].` is `[ 2 + ] .`. `.` still applies one, leaving
    a bool unchecked. This takes away march8's last rule of spelling, a
    name ending in `?`, which must leave a bool and, applied in a
    signature, is held as a guard, as if quoted. Built in march8
    (2026-10-15): a name ending in `?` is refused, and the primitives'
    tests lost theirs, `i64lt`.

Settled (Thomas, 2026-10-15):

21. **Powers.** `w^n`, written `wⁿ`, is the word's n-th power as
    mathematics takes it for that kind of thing: n copies of something,
    joined by an operation. A superscript is a convention, as a subscript
    is (22), and not one transform for every word (Thomas, 2026-10-24:
    "general conventions, and not strict mathmatical transforms", since
    a power does not apply usefully in the same way to every word): what
    `w^n` means is w's family's, and these are the core's, by kind, for a
    word that gives none. A family gives its own by a clause of the core's
    `power#` (24), for its word and any count, as a combinator's count of
    quotations would be, `cleave^3` for Factor's `tri` (COMBINATORS.md).
    The word's stack effect gives the kind:

    | kind | effect | `w^n` | so |
    |---|---|---|---|
    | value | `-- a` | n copies, multiplied | `pi^2` is π², `pi pi ⋅` |
    | function | `a -- a` | n applications, composed | `sqrt^2` is `sqrt sqrt`, a fourth root |
    | operation | k values, one left | n operands, joined by it | `+^4` is `+ + +` |

    An operation is counted by its operands, as x³ is three x's and two
    ⋅'s, so one taking k values is applied (n−1)/(k−1) times, which must
    be whole. Any other word, a stack word as `swap` or `dup`, is applied
    n times, as a function is, so `swap^2` and `rot^3` do nothing, as a
    permutation's power may be the identity; and a word that leaves
    nothing takes n operands, `print^3`. `^1` is x¹, the word once, or for
    an operation its one operand left as it is; `^0` is x⁰, the identity:
    nothing, for a function, and for an operation the value its fold
    starts from, `+^0` being 0 and `⋅^0` 1, its type from context, as a
    literal's is. The identity depends on the operation, not on the type
    alone (Thomas, 2026-10-20): `+` and `⋅` share a type and not an
    identity. A value's power is its copies multiplied, so its `^0` is
    `⋅^0`, `three^0` being 1. A type's power is its product, so in a
    signature `i64^3` is three i64s, as ℝ³ is. n is written, a number in
    the source.

    **An identity is a literal, `unit#`** (Thomas, 2026-10-20). `+^0`
    has no inputs, but `+` does, and its identity's type is the one `+` is
    used at: so it is decided where the value is used, as a literal's is,
    and as Haskell's `mempty` is. `+^0` is `+' unit# !`, the identity of
    `+`, its type to come, and a value's `three^0` is `*' unit# !`; where
    nothing says, it is i64, as `0` is. So `5 max^0 max` asks for `max`'s
    identity at whatever 5 is, and `concat^0` alone is no error.

    **The identities come from the primitives** (Thomas: "since
    everything is ultimately built of primitives, can't we analyze the
    primitives to determine the ^0? a sort of ^0 algebra"). The facts are
    March's, in march8's core.march: `unit` gives each primitive
    operation's, `i64+' unit` being 0, the products' 1 and
    `string-concat`'s the empty string; and `least` and `greatest` give an
    order's ends, `i64' least` the smallest i64, `f64' greatest` ∞ and
    `bool' greatest` true. A type converts a `unit#` to itself as it does
    a literal, `i64: < unit# >. [ i64 i64 ] identity# < i64 >. ;`, and
    `identity#` finds which fact holds: it follows the operation at that type to the
    primitive it applies there, through a word that applies one operation,
    `add: + ;`, or flips it, `[ swap + ]`, and a family's clause for
    that type; or to a choice of the greater of two, by `gt` or `lt`, whose
    identity is the type's least value, the lesser's its greatest. So `max`
    at i64 has the smallest i64, `min` at f64 ∞, and `and`, `min` at bool,
    true, with no fact of its own. Any other word has none, `-+` and `avg`
    among them, since an e with f(e, x) = x for every x is an equation,
    which for most words has no solution: avg(e, x) = x makes e be x. APL
    does the same: `+/⍬` is 0, `×/⍬` 1, and a function of one's own
    reduced over nothing is a DOMAIN ERROR. An unbounded type has no ends,
    so `max`'s identity at int#, a literal not yet typed, is an error. A
    clause of the core's `unit`, `core.unit: < int#-' > … ;`, adds a fact,
    and one of the core's `power#` still gives a word a 0th power of its
    own (24).

    Built 2026-10-20. The facts are March, and `identity#`'s search was
    Rust, in the stage, until a word's clauses were data March can read
    (16; Thomas: "we have to ultimately implement this in March itself").
    A first build that day had the facts and the search both in Rust, and
    decided an identity's type where `^0` was written, so `max^0` and
    `concat^0` were errors.

    The search is March since 2026-10-21, in core.march, over `clauses#`
    and `effect#` (6): `u ins identity#`, u the identity, whose operation
    `quote !` gives as code, `+' unit# !` being `[ + ]`, and ins the types
    it is for, `[ i64 i64 ]`, which is how a type converts one, `i64: <
    unit# >. [ i64 i64 ] identity# < i64 >. ;`. A word has the fact
    `unit` states for it, if one does; else its one clause for those
    types, followed; else two that pick one of two values by `gt` or `lt`,
    only moving them, which it shows by applying each to two symbols,
    `left' right'`. Code that applies one word, or swaps first, is
    followed into the word. Whether a fact is stated is `clauses#` given a
    value, `unit' [ int#-' ] clauses#` being empty, so the facts are only
    facts, and where none holds, `fail` says which word, at which type, and
    which clause would state one, as the Rust did. The stage keeps no
    search of its own. The search's words end in `#`, as March's own (17),
    so `choice` and `end` stay a program's. For it, `eq?` is false for a
    symbol and a quotation, as code's elements are compared.

    A power is a quotation, which the word `power#` makes from a word or
    a quotation and the count, so a quotation's power is the quotation of
    the power, `[ q ]^n` being `[ q^n ]` by q's kind: `[ dup + ]^3` is
    `[ dup + dup + dup + ]`, and `[ + ]^3` is `[ + + ]`, as `+^3` is. The
    superscript changes what a thing is, not whether it runs: a name still
    applies, and a quotation is still a value, for `.` to apply or `map`
    to take, `( 1 [ dup + ]^3 . )` being `( 8 )` and `( 1 2 3 ) [ dup + ]^3
    map` being `( 8 16 24 )`. `power#` is March, in core.march, as the
    search for an identity is (2026-10-26): it reads the effect of what it
    takes, `effect#`, known of known code only, as `.` applies only known
    code (4), and builds the quotation, a word applied by its symbol,
    `+' 3 power#` being `[ +' . +' . ]`. Where the effect is not known
    while compiling, for a family whose clauses take different numbers of
    values or a symbol applied at run time (19), it is an error, and the
    words are written out.

    This replaces `w*n`, n copies in a row (march7/docs/SURFACE.md,
    "Copies and operands"): `^` says all of it, a value's copies being
    `pi dup^2`, since `dup` is how one gets a copy. `*` stays only in a
    signature, with no count, `t*` any number of t and `a*` the rest of
    the stack: the regular expression's star, which mathematics writes as
    Σ*. Built in march8 (2026-10-15). A word's power is a rule of the
    core's (25), which rewrites `dbl^2` where it is applied as `dbl 2 int#
    ! power# .`, so `dbl` is the one found there, and `power#` makes its
    power. Built 2026-10-20.
22. **A subscript is an index** (Thomas, 2026-10-20). In mathematics a
    subscript picks a member of a family, log_b, x_i, a_n, and the family
    says what its index means. So in March `w_n`, written `wₙ`, is a name,
    and what it means is its family's. A superscript is the same, a
    convention whose meaning is the family's, the core giving defaults by
    kind (21; Thomas, 2026-10-24). Until then it was held to be unlike a
    subscript, fⁿ being f iterated whatever f is, one rule for every word;
    but that rule was four meanings by kind already, and mathematics' own
    superscript is a convention: sin²x squares the value, f² in dynamics
    composes, f⁻¹ inverts and sin⁻¹ is arcsin, ℝ³ is a product and f⁽ⁿ⁾
    a derivative. A program gives `log_2` a meaning by a
    rule, its base (25), or `x_1` one by defining it, as any name.

    Where no family gives a subscript a meaning, the core gives one, by a
    rule of its own (25), to any word that moves values or is handed code:
    a width, each value it moves a group of n, the group moved as one
    value. So `1 2 dup_2` is `1 2 1 2`, FORTH's 2DUP, the pair
    copied, not `1 1 2 2`, each value copied. `swap_2` is 2SWAP, `over_2`
    2OVER, `rot_2` 2ROT and `drop_2` 2DROP, and `=_2` and `~_2` likewise;
    `dip_2`, `keep_2` and `bi_2` are Factor's 2dip, 2keep and 2bi. Inside
    `w_n`, w's own words move groups, but a quotation handed to w is not
    widened: it runs once, as written, on the group's values. So `keep_2`
    is 2keep, though `keep`'s own `over [ . ] dip` moves a pair past a
    quotation. At run time a group's values move by `pick`, by rolls, or
    through the scratch stack, `dup_2` being `over over`. The width comes
    before the power, `dup_2^3` being `dup_2` cubed. So `==` and `~~` go.
    A program's word made of them has one too, by what it does, not by a
    list: `tuck: swap over ;` gives `tuck_2` (Thomas, 2026-10-20: "if
    it is general then maybe we make it general after all").

    A word that computes has no width: `+_2` and `cube_2` are no words,
    unless a program defines them. A first rule (2026-10-15) gave every
    word one, an operation acting lane by lane, `1 2 10 20 +_2` being
    `11 22`, as vectors add. But a
    vector does that better; it is not what one guesses, Thomas first
    reading `+_3` as three operands, which is `+^3`; and it made a
    subscript a width everywhere, where mathematics makes it an index
    (Thomas: "is this truly generalizable?"). Moving a group as one value
    is what stack words do, and FORTH and Factor give only those a width.
    Built in march8 (2026-10-15, as a rule of every word; as an index,
    2026-10-20, first for a list of the core's words, then for any word
    that moves values or is handed code).
23. **`fmt` writes them as mathematics does,** by LaTeX's marks, `^` up
    and `_` down: a name's trailing `^` and digits as superscript digits,
    `+^4` as `+⁴`, and its trailing `_` and digits as subscript digits,
    `dup_2` as `dup₂`; only digits, and only at a name's end, so
    `file_read` stays as it is. The reader reads them back, `₂` as `_2`
    and `⁴` as `^4`, as it is to read `⟨ ⟩` as `< >`
    (march8/docs/MACHINE.md), and the ASCII spelling is the one kept, the
    one typed, the subscript first, `dup_2^3` as `dup₂³`. March8's escape `\_2`
    writes `₂`, and is the same spelling. Built in march8 (2026-10-15).
24. **No name ends in a power** (Thomas). The core's rule rewrites `^n`
    (25), and comes last, so a word named `sq^2` would come before it, as
    FORTH lets a word redefine a number, searching its dictionary first.
    Here what a spelling means would then hang on what happens to be
    defined, and one definition would change every use unseen, so defining
    one is an error that says what the spelling means. A specific meaning
    is a clause instead, of the core's `power#`, by its path (16), for
    that word and that count: a family chooses its most specific clause,
    so the clause wins over the general rule, as FORTH's dictionary wins
    over its numbers. An identity the primitives do not give is one (21):

    ```
    core.power#: < -+' 0 >. drop drop [ 0 ] ;    -- -+^0 is 0
    ```

    `power#` matches a quoted word as a value, as it does a number or a
    symbol. The core's own identities were such clauses until they came
    from the primitives (21, 2026-10-20). A subscript is part of a name,
    an index (22, 2026-10-20), so `x_1` and `log_2` may be defined, and a
    program's `dup_2` comes before the core's, as a program's `dup` would
    (17).
25. **Recognizers: March defines its spelling rules** (Thomas). The rules
    that read High Stack (26) can be March code, recognizers, as FORTH
    names them, and a program may add its own: `log_b` can be a family of
    its own, base b's log, `log_2` among them. A definition still cannot
    change how text reads, only a spelling rule, which says that it is
    one, so the litmus test holds: `march8 lower` still writes any program
    in Low Stack. In FORTH a recognizer is a word that takes a token's
    text and says how to interpret or compile it, by which Gforth reads
    its numbers; Lisp's kin is the reader macro, Factor's the parsing
    word. With them March reads itself by its own words, a step up in
    kind. Very powerful, and an easy way to win an obscure-code contest:
    not to be used lightly (Thomas).

    **A recognizer is a rewrite rule** (Thomas, 2026-10-20: "recognizers
    should be implemented as defining rewrite rules"). Its name is a
    pattern, a regular expression between slashes that matches a whole
    name, and its value is code. Where a name the pattern matches is
    applied, the rule puts in its place what the pattern's groups capture,
    each a symbol, in the order written, then the rule's code:

    ```
    logs./log_([0-9]+)/: i64 ! log-base ;
    import#: [| logs.* |] ;
    1024 log_2                       -- 1024 '2' i64 ! log-base
    ```

    A rule is a spelling, not a word, so it has no signature and one
    meaning: a second definition of a pattern is an error, and so is a
    signature. Its effect is the rewrite's, found where the name is
    applied, so to its user `log_2` is `< f64 >. … < f64 >.`. More meanings
    are a family's, which the rule's code applies:

    ```
    times: < i64 sym >. int# ! * < i64 >. ;
    times: < f64 sym >. f64 ! * < f64 >. ;
    /times([0-9]+)/: times ;         -- 5 times3 is 15, 2.5 times2 5.0
    ```

    Each part means what it means where it was written (16). A capture is
    a symbol, global text, and `name#` reads one as a name where the name
    was applied, its path: a name a rule matches is expanded to a path
    there, `math.dbl^2`, so the path says where. The core's power and width
    rules read their word so, `[ [ name# ] dip int# ! power# . ]`, and
    `dbl^2` in `math` squares `math.dbl`; a rule whose capture is a count,
    `times3`, has no need of it. Clojure's macros read a symbol so, with
    `resolve`, while they expand. The rule's own
    names are found where the rule was defined, as a word's are, so `sq7`,
    in a namespace that imports `ops./sq([0-9]+)/: int# ! sq ;`,
    squares by `ops.sq`: hygiene, as Scheme's macros keep it. The captures
    go on the stack, not into marks, `$1`, since a mark would want a
    spelling in Low Stack, and `dup` serves a capture used twice; and they
    go where the name stands, after the values before it, so `x log_2` is
    `x '2' i64 ! log-base`. Each group must capture in every match, so the
    effect is fixed: an optional group is refused, for now.

    A rule holds in its own namespace and where that namespace is imported
    by `*` (17), for the whole namespace, wherever it is written, as an
    import does, so no name means one thing before it and another after;
    before its code is defined, a name it matches is an error. An export
    names it by its pattern, `export#: [| /log_([0-9]+)/ |]`. A name two
    rules match, of one namespace or of two, is an error (Thomas), so
    their order never counts; so is a name a rule matches that is also
    defined there or imported, settled by naming it (17). The core has
    rules too, which come last, as the core's names are found last:

    ```
    /(.+)\^([0-9]+)/: int# ! power# . ;
    /(.+)_([0-9]+)/: int# ! width# ;
    ```

    So `.` applying a name looks first for a rule of its namespace or of
    the namespaces it imports, then for the name, locally, among the
    imports and in the core, and last for a rule of the core. `dbl^2` is
    `dbl 2 int# ! power# .`, `power#` leaving a quotation, which `.`
    applies (21); `dup_2` is `dup 2 int# ! width#`, `width#` applying the
    word its symbol names at that width, if it moves values or is handed
    code, and otherwise saying there is no such word (22). A program's `dup_2`, or a
    rule of its own that matches it, comes before the core's.

    A rule is matched where the name is applied, by `.`, against the name
    without its marks, `log_2` for `log₂`; with `'` the name is passed,
    `log_2'`, and rewritten where `.` applies it later. So High Stack and
    Low Stack match alike, and nothing runs while reading: in FORTH a
    recognizer runs anything then, and here a pattern only matches. A
    number is no name, nor `;` or `_`; `12kg` is a name, which applies, so
    a rule may give it a meaning. In Low Stack `;` takes a pattern where
    it takes a name, the symbol made a pattern by `pattern# !`, as `int#
    !` makes a number (section 7); so `;`'s first slot widens from one
    name to a set of names, and no slot changes what it means. A pattern
    matches a name, which has no spaces, so it needs none; a `/` in it is
    written `\/`.

    Built in march8 (2026-10-20), by Rust's `regex` crate, which matches
    in linear time (Thomas: March wants regular expressions anyway, if only
    as a module). March may come to have a pattern syntax of its own, a
    regex then an optional module, which March itself never needs (Thomas,
    2026-10-16). The syllogism runs lowered, its capitals rewritten where
    `.` applies them (27). The cost, measured for the first form, whose
    matching this keeps: about 25 ns a name where a namespace has rules,
    and none elsewhere; the core's rules are tried only where nothing else
    is found.

    **How it came to this.** A first form (2026-10-15) was a namespace's
    clauses of `recognize#`, the pattern in the signature's slot and a
    template whose holes, `_`, took the captures. It copied `import#` and
    `export#`, but those are settings of a namespace, what it sees and
    what it shows, while a recognizer is vocabulary; and under its key
    `< >` held a pattern, not a signature. A key may give meaning to its
    value, as `import#` does to its list, but should not change what a
    slot means, each such exception a cost, though one may be made on
    purpose where it keeps a program from harm (Thomas, 2026-10-16).

    A second (2026-10-16) made a recognizer a word whose name is a
    pattern, with a signature and clauses as any word has, its captures on
    the stack after its inputs. March8 matched it while compiling, against
    the word as typed in High Stack, which Low Stack does not keep, so the
    syllogism could not run lowered; and a captured name was looked up
    where the word's body applied it, not where it was written. Thomas:
    "the issue is how you are trying to implement them -- as quotes. Why
    not implement it as a rewrite rule [in] the low stack?" A rewrite puts
    the code where the name is, which is what a spelling rule says. It
    loses a pattern's clauses, which a family gives back.

    The core's power and width went the same way. A plan (2026-10-19) made
    them rules of the core, but a captured name, `dbl` of `dbl^2`, was then
    looked up inside the core's code, where `dbl` is not; march8 had that
    fault already, `width#` and `power#` being core words with bodies. So
    for a day (2026-10-20) lookup read a power itself, in Rust, and the
    core's widths were names. Once a name means what it means where it was
    written (16), a capture does too, and the plan works: the rules above
    replace that Rust. A subscript stays an index (22): the core's width
    is for a word that moves values or is handed code, and `+_2` is no
    word. Thomas: "If I write `1 2 +`, how does it handle `+`? So why
    would `+_2` be any different?" A name keeps its subscript and its power
    when lowered, `dup_2 .` where it was `dup 2 int# ! width# . .`, so
    lowering needs no namespace (Thomas: "isn't that true of any word?").
    A quotation's power, `[ dup + ]^3`, stays a spelling, `[ dup + ] 3
    int# ! power# .`: it is on a bracket, which lookup never sees.
    Subscript and superscript digits still read as `_2` and `^2` (23), as
    `\times` reads as `×`.
26. **High Stack and Low Stack** (Thomas, to try). The surface March is
    written in, today the intermediate form of section 4, is High Stack;
    Symbolic Forth, the lowest form, is Low Stack.
27. **A capital marks nothing** (Thomas, 2026-10-17). Words are
    case-sensitive, so a capital need not mark a symbol: `Snake` is a name
    like any and applies, and `Snake.` goes. A symbol is quoted where it
    is used, `Snake'` or `'Snake'`, or given by a word that leaves it,
    `Snake: Snake' ;`, as a Lisp keyword evaluates to itself. A namespace
    of symbols, a state machine or facts and rules, imports the old rule
    as a rule (25) whose capture is the name itself and whose code is
    none, `/([A-Z][A-Za-z0-9]*)/: ;`, and there
    `Socrates Is Human fact` reads without marks as before; a capitalized
    word defined in it clashes with the pattern, as among facts it should.
    In a signature a bare `Open` is applied and means what it leaves, its
    symbol if it is such a word; written `Open'` it needs no definition,
    and a reader tells it from a type by its spelling (TYPES.md 2.2). This
    takes a rule of spelling out of the intermediate form. Built in
    march8 (2026-10-17): `;` binds a symbol as it does code; the core
    defines none, reading and case styles being words of namespaces,
    `io.file.read` and `case.snake` (TYPES.md 3.12);
    examples/door.march defines its states and events, and
    examples/syllogism.march reads its capitals by the rule, lowered too,
    where `.` applies them (25, 2026-10-20).
28. **A symbol taken as text is a string** (Thomas, 2026-10-17). A symbol
    is text until a type structures it (section 1), so a word that takes a
    string takes a symbol and makes it one: `'hello' length` and
    `hello' length` are both 5. It holds for a symbol known only at run
    time, a `sym` input, as for one written. Values are not promoted
    (TYPES.md 2.10) because precision is lost and families are made
    ambiguous, and neither happens here: a symbol has no structure to
    lose, and a family's `sym` clause, the exact one, is chosen first. A
    symbol costs nothing either way: it is its name, interned, at run time
    as a string is, so only its type changes. It goes one way: a string used as a symbol does
    not become one, which would intern any text, and a symbol compared
    with a string is compared as a string, so a fact read as text from a
    file matches one written in code. Built in march8 (2026-10-17):
    `'…'` reads as a symbol, a quote in it doubled, and is shown so when
    it is no name, `'two words'`; a hole writes a symbol as its text.

    The other way is explicit: `sym !` makes a symbol of a string, at run
    time too (section 4, built 2026-10-13), and that is what dynamic
    symbols are for (Thomas: "are dynamic symbols essential?"): data
    choosing a clause by symbol, as the door's events read as text choose
    its transitions as it runs, `"Unlock Open" words [ sym! ] map`. For
    those the stage folds nothing; the clauses are tested at run time.
    Text that chooses no clause can stay a string, already a cell as a
    map's key, strings being interned.

29. **Failing, and traps** (Thomas, 2026-10-20: "it is just an ERROR
    context with the same word definitions as the word you want to catch
    that error in"; "yes, that is what I had in mind"). A word fails by
    `fail`, which takes one value, a kind alone or a tuple of a message
    and a kind, so that what lies below is never taken for a message:

    ```
    no-file' fail
    ( "can't read it" no-file' ) fail
    ```

    `fail` never returns, so nothing after it is reached and a body that
    ends in it leaves what its signature says. Code run while compiling
    that fails, a bracket's, a rule's, the identity search's (21), is an
    error where that code was applied. At run time failing is a trap.

    A handler is a clause of the same word, for its inputs and a trap on
    top. When anything inside the word fails, its inputs come back as
    they were when it was applied, the trap goes on top, and the word is
    applied again, so ordinary dispatch chooses the handler, which leaves
    what the word leaves: the word's effect is the same either way, and
    checked while compiling.

    ```
    size: < string >.                io.file.read length < i64 >. ;
    size: < string k trap >.         drop drop 0 < i64 >. ;
    size: < string no-file' trap >.  drop drop 0 < i64 >. ;
    ```

    A trap's kind is in its type (Thomas, 2026-10-22, and revised the same
    day: a kind is a type itself, below), as `i64 ary` gives
    an array's elements, so a handler takes a kind as any clause takes a
    case, with no test written. `trap` takes its kind as `ary` takes its
    elements' type, so a letter takes any kind, `k trap`, as `a ary` is
    any array; `trap` alone would take whatever is below it as its kind
    (Thomas). The most particular clause is chosen, `no-file' trap`
    before `k trap`, as `i64 ary` is before `a ary`. A trap's
    kind is known only when it fails, so the choice is made at run time,
    as a union's is: a trap is the union of its kinds. A guard still
    chooses by anything else, its message among them. A trap no handler
    takes goes on outward, as though the word had none. `kind` gives a trap's kind
    and `why` its message, or with none, its kind's name; where it failed
    is kept for the report. `fail` takes a trap too, and passes it on as
    it is. The nearest word applied with a handler catches; with none,
    the session reports it, as now. The machine's failures are traps
    too, overflow, division by zero, a file not read, no clause's guard
    holding, each of its kind; the fuel limit is the session's, never
    caught. This is Factor's `recover`, which restores the data stack and
    calls a quotation with the error, and ANS FORTH's `CATCH`, which
    restores its depth: with types, March restores the inputs themselves.
    The word is the try, as a Ruby method is with its own `rescue`.

    The error context is on the stack, as every context is, so it is no
    global state, and each task fails and handles on its own (Thomas:
    "Is error a system global state? maybe not, b/c of multi-tasking").
    It is a type only failing makes, `trap`, so a word that takes an
    error as data, a program's own type, is never a handler (Thomas: "just
    b/c a word accepts an error argument doesn't make the a trap"). One
    rule keeps it so: a clause that takes a trap may not leave one, and no
    store may hold one, so a trap lives only while a failure is handled.
    Only a word with a handler pays, keeping its inputs as it is entered,
    which, March's values being persistent, is keeping their cells; what
    was done before the failure stands.

    **A kind is a type, made where it is defined** (Thomas, 2026-10-22).
    `trap` takes a map, whose `error#` holds the kind's message, and makes
    it a type, so a signature names a kind as it names any type, with no
    `trap` written there, and `or` joins kinds as it joins types. `;`
    names it with the path it defines, as it names a type of its own
    (TYPES.md 2.11), so a kind knows its own name, and two kinds with one
    message are two. Thomas first asked for `ns#` to give it, "a primative
    the returns the current namespace as a symbol", written in the map;
    but the name wanted is always the one being defined, which `;` knows
    (Thomas, 2026-10-23), and `ns#` would give where it is written, the
    core's namespace or a helper's in a word that makes kinds. A kind's name is a word and a namespace both, the kinds
    beneath it in it, as march8 already lets a word and a namespace share
    a name; the hierarchy is the namespaces', not a symbol's spelling
    (Thomas: "that would make just longer symbol `err.file'` that's not a
    namespace. it has to be a word"). The core's kinds are under `err`:

    ```
    err.file:          { error#: "file error" } trap ;
    err.file.notfound: { error#: "file not found" } trap ;
    err.file.denied:   { error#: "permission denied" } trap ;

    size: < string err.file.notfound >.          drop drop 0 < i64 >. ;
    size: < string err.file >.                   drop drop -1 < i64 >. ;   -- any file error
    size: < string err.file err.arith or >.      … < i64 >. ;
    ```

    A kind takes the kinds beneath it, and the most particular is chosen,
    `err.file.notfound`, then `err.file`, then `err`, the root, beneath
    which is every kind, a program's too, so `err` takes any. A kind
    misspelt is no word, never a handler that silently takes nothing.
    `fail` takes a kind and, on top, a message of its own, `err.file "my
    error message" fail`: a string is the message only on top, so one
    below a kind is never taken for it, and no tuple is needed; with none,
    `why` gives the kind's. A program's kinds are defined as the core's
    are, and none is made as the program runs (Thomas: "I suppose in that
    case we could make errors on the fly as needed?"; not now, and one
    made under a defined kind would still be taken by its parent's
    handlers). What a kind holds is known while compiling (Thomas: "a trap
    is a map that knows it's name it then map space and a default
    message"):

    ```
    {
      error: < sym > ;
      message: < str > ;
      subspace: < sym trap map > ;
    } ;
    ```

    A failure carries less, its kind's name and its message, so what is
    beneath a kind is never made at run time: a handler knows the kinds it
    takes while compiling. Whatever else is the same each time a kind
    fails, `status: 404 ;`, the kind holds; what belongs to one failure,
    which file, travels with it, in its message for now, and as a value
    its handlers take once a kind can say what its failures carry, which
    is still to design. The compiler's and the machine's own kinds move
    under `err`, overflow and division by zero apart once the machine says
    which, named as they are now (2026-10-24, Thomas: "I agree"):
    `err.syntax`, `err.no-word`, `err.mismatch`, `err.literal`,
    `err.unknown`, `err.arithmetic`, `err.limit`, `err.effect` and
    `err.io`, and the symbol kinds, `no-file'`, retire. A kind is a type a program defines,
    so it waits on defining types, records among them (Thomas: "clearly we
    need to be able to define new types"). Decided, not built: march8's
    kinds are still symbols, `no-file' fail`, and `trap` is written in a
    signature, `k trap`.

    `fail` while compiling is built in march8 (2026-10-21). The machine's
    own kinds are named as its errors are, `syntax'`, `no-word'`,
    `mismatch'`, `literal'`, `unknown'`, `arithmetic'`, `limit'`,
    `effect'` and `io'`, so March code can fail as the compiler does; any
    other kind is the program's. Its message reads from the tuple while
    compiling because an array literal of known elements is now known
    there too (TYPES.md 2.10), and a symbol beside a string makes a tuple,
    `( string sym )`, not two strings. A clause that fails is defined as
    any other, checked only as far as `fail`.

    Failing at run time is built in march8 (2026-10-22). In code
    that may not run, a choice made at run time, a union's arm or a loop's
    body, `fail` is a trap, its kind and message known while compiling or
    only as the program runs; the session reports it, by its message or,
    with none, its kind. A path that fails leaves nothing, so the others
    give a choice its results, and one every path of which fails never
    returns. Code sure to fail fails while compiling, so where its kind is
    known only as the program runs, it is an error that says so. The
    machine's own traps carry their kinds too: no clause's guard holding
    is `no-word'`, arrays not as long `mismatch'`, and since 2026-10-24
    an element or a key not there `mismatch'` too, as they are while
    compiling. Where a trap happened is not reported yet: written into the
    code, a position would make a word's identity depend on how its source
    is laid out, so it wants a table beside the code.

    Handlers are built too (2026-10-22), with `kind`, `why`, and `fail` of
    a trap. A word is handled where clauses of its own take its inputs and
    a trap on top: the machine's `handle` keeps the inputs that are values
    at run time, and `unhandle` lets them go when the word is done; a trap
    before then puts the stack back with them on it and the trap on top,
    and goes on into the handlers' choice, compiled where the word is
    applied, a trap none of them takes failing again, outward. The word's
    results and the handlers' are joined as a choice's alternatives are,
    so the effect is checked while compiling, and a handler that leaves
    its trap, alone or in an array, is an error. Inside a handled word a
    failure is caught, so it fails as the program runs, arithmetic on
    known values among it; a handler runs only if something fails, so it
    is code that may not run. A recursion is handled at each application,
    the innermost catching first. Overflow and a file not read are traps
    of `arithmetic'` and `io'`; a missing file is `io'`, finer kinds to
    come when a handler needs one. A clause that takes a trap last is a
    handler only where its word's other clauses take the inputs;
    elsewhere it is a word applied to a trap, as `log: < k trap >. … ;`
    is in a handler. Not built: kinds as types (above), and where a
    trap happened.

30. **Code is data only while compiling** (Thomas, 2026-10-21: "we are
    headed into Lisp territory now. and guess what? I don't want to go
    there. That's why a quotation is not just a sequence"). While
    compiling, a quotation is a sequence (6): March takes it apart, builds
    it and reads it, as the identity search does and as March compiling
    itself will, which is staging, as macros are, not evaluation. At run
    time it is code only, passed and applied, never taken apart or
    edited, so every quotation a program applies is one the stage saw
    whole, and every effect is checked. `!` makes data of what a
    quotation's code leaves, a sequence by `seq# !`, and nothing at run
    time makes code of data
    again: an array is not a quotation, and `.` does not apply one, as
    spreading one is a word of its own, `vec-spread`.

    Code at run time is then what FORTH's is (Thomas: "interesting Forth
    itself seem to manage it"): an execution token, opaque, which
    `EXECUTE` applies, or text, which `EVALUATE` hands to the outer
    interpreter to compile whole. In March the first is a word of a known
    set, or one compiled at a named signature, a value whose type says its
    effect (19); the second is immediate mode (19), a symbol or text from
    data compiled whole by the stage, on request, and checked against the
    shape its caller declares, since March, unlike FORTH, checks effects.
    What March leaves out is Lisp's third, code as a list built at run
    time and applied: Joy's quotation, and Factor's, a sequence at run
    time, which needs `call( -- )` to be applied. Kitten's quotation, a
    typed function, is the nearest kin.

    Weighed first (2026-10-21) and set aside: a quotation and an array as
    one thing, a sequence, a kind saying what `!` makes of it. Any array
    would then be code, applying it a choice of words at run time, and
    its effect unknown while compiling. Weighed too and too far (Thomas):
    no quotations, every piece of code a named word. Low Stack is built on
    them, a word's body being one and every bracket made from one, and
    FORTH added `[: … :]` to be rid of naming every piece. What stands:
    `seq# !` is `.` with what it leaves kept together, so a literal's code
    reaches what `.`'s does (14). And `[ … ] !` runs a quotation while
    compiling, staging too, its values left known (32).
31. **A bracket names its inputs** (Thomas, 2026-10-23: "stack-base
    programming is not easy for humans or LLMs … The mind just does not
    think well in terms of a stack past a few postions", and "am I wasting
    our time building this on a foundaton of FORTH"). The foundation
    stays: a parser's tree ends at a stack machine anyway, as wasm's, the
    JVM's and CPython's do, and Low Stack, the stage and the rules are what
    March is built on. What changes is what a person holds. A run of words
    passing one value along reads as a pipe does, and needs nothing; a
    word juggling three or four values needs a stack comment,
    `( P r g g' )`, which is a programmer naming the values and tracking
    them by hand, work the stage, which knows every place, can do. So a
    bracket may name its inputs, before a lone `|`, in a signature's
    order, the deepest first:

    ```
    growth:  < f64 f64 >.     [ rate term | rate 1 + term ^ ] .  < f64 >. ;
    annuity: < f64 f64 f64 >. [ p r g | p r ⋅ g ⋅  g 1 -+  div ] .  < f64 >. ;
    payment: < f64 f64 f64 >.
      [ principal rate term | principal rate  rate term growth  annuity ] .
      < f64 >. ;
    ```

    The names are the bracket's inputs: it takes as many as it names, and
    drops those its code does not use (Thomas), where a bracket of letters
    takes as many as its deepest letter. A name is the input, copied each
    time it is written, as a letter is. Only names come before the bar, no
    number, symbol, label, path or bracket, and they differ; a bracket
    that names its inputs uses no other letter and no `_`, as a bracket
    uses `_`s or letters, never both. A name may be a letter,
    `[ x y | x y + ]`, and may be a word's name, which it hides in its
    bracket. A signature names nothing, its letters being types, and a
    string none, its text being what comes first.

    The names belong to the bracket (Thomas: labeling "becomes the local
    variable program that thwarts easy factoring. My solution was to
    contain it to a block"). A quotation inside it does not see them
    (Thomas: "let's say no, see how that fares"): one passed to `map` that
    used them would be a closure, which March has none of (14), and could
    not be lifted out alone. So a bracket that names its inputs is closed:
    from outside it is a stack word like any, and it lifts out whole into
    a word of its own, unchanged. A quotation inside it is given the value
    on the stack, and naming its enclosing bracket's name is refused,
    saying whose it is, unless it names it itself.

    A string, a sequence and a map inside it see its names (Thomas,
    2026-10-23, of refusing them: "that does feel too tight"). They are
    made where they stand and never carried off, as their `_`s are filled
    there (14), so nothing can be captured, and each name is passed in on
    the stack just before the bracket and taken there by a letter, as one
    would write it by hand:

    ```
    [ x y | ( x y ) ]                 [ x y | x y ( b a ) ]
    [ n | "n is \[ n ]" ]             [ n | n "n is \_" ]
    [ t r | { term: t rate: r } ]     [ t r | t r { term: b rate: a } ]
    ```

    Each name is passed once, the first used the deepest, and the
    bracket's own inputs, its `_`s, letters or names, lie below those
    passed in, so `9 5 [ n | "\_ and \[ n ]" ] .` is "9 and 5". A literal
    inside a literal is passed them through each. A quotation inside a
    literal still sees none, and a signature has no names in it, its
    letters being types. A record built of a word's names, the way its
    caller names them (below), needs nothing written twice.

    One bar, after the names, since `[ | rate term | … ]` was too near
    `[| … |]` (Thomas): Smalltalk's block, `[ :x :y | … ]`, without its
    colons. Weighed and set aside the same day: `:x` in a body, binding the
    value on top, Factor's `:>` and Kitten's `-> x;`. Names at the head
    only keep one form, the bar being the stack comment, made something the
    stage reads, and an intermediate value is named by a word of its own,
    `annuity`, or by a bracket applied in place, `[ p r g | … ] .`, which is
    given what it names. Set aside too: labels in the signature,
    `< principal: f64 … >`, so a signature stays the interface, its types,
    and the names stay the body's.

    Low Stack has no names (section 7): by spelling alone, the reader
    writes each name as the letter of its place and drops the bar, so
    `[ rate term | rate 1 + term ^ ]` is `[ b var# . 1 int# ! + . a var# .
    ^ . ]`, and Low Stack keeps one way to name an input. Where the code
    does not use the deepest name, its letter is written once and dropped,
    `c var# . drop .`, after the letters the code begins with, so the
    bracket takes all it names and the shuffle stays one. Names are then
    High Stack's, the presentation that the editor's names of 14 were to
    be, and a message names the letter, as it does for `_`.

    A word's inputs named by its caller are a record (Thomas, 2026-10-23:
    "Passing records -- of course"): `{ term: 72 rate: 0.2 principal: 50000
    } payment`, `payment` taking a record whose type is known where it is
    applied, so the stage can open it into places at no cost. A misspelled
    label is then an error, and two `f64`s are not swapped unseen. Set
    aside: values labeled on the stack, `72 :term 0.2 :rate 50000
    :principal payment`, since a label loose on the stack has no end, no
    word saying which values are whose. Waits on records (TYPES.md 2.11).

    Built in march8 (2026-10-23), names and not records: the reader writes
    a bracket's names as their letters before it writes its `_`s, those
    of a string or a literal passed names among them, so the stage reads
    neither, and a named bracket compiles as the plain words do,
    `[ x y | y x -+ ]` as `swap -+` and `[ x y z | y z + ]` as
    `rot drop +`, and a literal passed names as one passed them by hand. For the second the shuffle keeps the inputs a bracket's
    letters begin with in order already, `b a` in `b a c`, where it kept
    only those in place, at the bottom; every example compiles to the same
    code, word for word.
    A `|` is a bar only where High Stack reads it, so in Low Stack it is a
    symbol, as any name is.
32. **A body is bare** (Thomas, 2026-10-24: "It appears we have made a
    subtle faux pas"). Between the label's colon and `;` is the body, as
    between FORTH's `:` and `;`, and applying the name runs it: `sq: dup
    * ;` is `: sq dup * ;`. A bracket in a body is a value, whichever
    bracket it is, so each of these leaves what it holds:

    ```
    foo: ( 1 2 ) ;      -- the tuple
    bar: { a: 1 } ;     -- the map
    baz: [ dup * ] ;    -- the quotation
    sq: dup * ;         -- runs dup *
    ```

    Until then a body was a quotation, `sq: [ dup * ] ;`, and `;` treated
    it apart from every other value: a tuple was left where the name was
    applied and a quotation run, so a word leaving a tuple would have been
    `foo: [ ( … ) ] ;` (Thomas), and code with no bracket, `qux: … ;`, was
    to be computed while compiling, where it could take nothing from below.
    The brackets came up from Low Stack, where `;`, being postfix, takes
    the body as a value. In High Stack the label's colon opens it already,
    so `name: … ;` lowers to `name [ … ] ;`, and Low Stack does not change.
    What it settles, and what follows from it:

    - **A constant of several values** is a word that leaves them, `fbb:
      "FOO" "BAR" "BAZ" ;`, the colon marking where the body starts.
    - **A type is named as before.** A type is known only while compiling
      (TYPES.md 2.4), so a body that takes nothing and leaves a type is
      computed then, and `;` names what it leaves: `celsius: f64 own ;`,
      `meter: measure ;`, an alias, `temp: f64 ;`, a kind made by `trap`
      (29), and a record type, `point: { x: < f64 > ; y: < f64 > ; } ;`.
    - **A signature is a check, applied** (Thomas). A type bracket is a
      value as any bracket is, and `.` applied to it checks that the top
      of the stack holds those types where it is applied, a law of the
      code there; `< i64 >.` is `< i64 > .`, as `[ 2 + ].` is `[ 2 + ] .`.
      At a word's start that is what the word expects, so `;` takes the
      check a body begins with as the clause's inputs, which dispatch
      matches, a clause whose check fails passed over, and the check it
      ends with as what it leaves; elsewhere a check that fails is an
      error. Outputs cannot be checked at the start, which is why FORTH's
      `( n -- n )` was a comment, so the outputs' bracket touching the
      inputs', `><` (8), retires, and `><` is free again (TODO.md):

      ```
      sq:    < i64 >. dup * < i64 >. ;
      floor: < i64 >. ;
      max:   < a a gt? >. drop ;
      num:   < i64 f64 or money or int# or dec# or ratio# or > ;
      ```

      `floor`'s outputs are worked out, as a clause's are where it gives
      none, so its check at the end is left off; `num`'s bracket is not
      applied, so the body leaves the type, and `;` names it, as a record
      type's field is `x: < f64 > ;`. A check says only what it says
      (Thomas): `ff: < i64 >. + ;` takes two, as `ff: + ;` does, the top
      an `i64` and the value below typed where `ff` is applied, as a
      word's inputs are where it has no check. A letter is one type
      variable through a definition, `abs: < a >. … < a >. ;`. A check
      costs nothing where the types are known while compiling; where they
      are not, a `num`'s, it is tested as the program runs and fails as
      `fail` does (29), as dispatch tests a clause's first check now.
      `clauses#` gives a clause's checks as its inputs and outputs, as
      before (6). Weighed first and set aside the same day: the bracket a
      body begins with as its signature, with the outputs' if it touched,
      unless they were all of the body, which kept the aliases, nine, but
      lost the identity clauses, twenty-seven, `floor: < i64 >< i64 > [ ]
      ;` read as two types.
    - **A bracket names its inputs** as before, applied where it stands
      (Thomas): `growth: < f64 f64 >. [ rate term | rate 1 + term ^ ] .
      ;`. Names on the definition itself would say the same twice, so
      there are none (Thomas, 2026-10-24: "every definition with more
      than one argument is going to turn into that form"), a concern
      that holds for the bracket's names too, which COMBINATORS.md
      weighs.
    - **Code runs while compiling only when asked,** by `!` with a
      quotation on top, which until now it refused (section 7). `[ … ] !`
      runs the quotation once, while compiling, and leaves its values
      known:

      ```
      my-schema: [ "schemas/my.json" io.file.read ] ! ;
      validate:  [ "schemas/my.json" io.file.read ] ! check ;
      ```

      `!` makes data of what a quotation's code leaves (30): with a kind
      on top, data of that kind, `[ … ] seq# !`, and with the quotation
      itself on top, its values, made while compiling. So `!` faces `.`:
      `[ … ] .` runs the code where it is applied, each time, and
      `[ … ] !` runs it while compiling. The quotation takes nothing from
      below, since what is below is known only as the program runs, so
      `2 [ 1 + ] !` is refused; its effects run, a file read as Rust's
      `include_str!` reads one, the code's identity following what it
      read (TYPES.md 2.15); and each quotation runs once, kept by its
      identity. `io.file.embed` gives way to it, `[ "notes.txt"
      io.file.read ] !`. It is FORTH's `[ … ] LITERAL`, Factor's
      `<< … >>` and Zig's `comptime`, and macros come of it with no
      macro language: a quotation it leaves, applied, `[ … ] ! .`, is
      code made while compiling. Weighed first and set aside: `[[ … ]]`,
      as some FORTHs write it, for its doubled brackets (Thomas), which
      would have been two tokens of their own as well (8). It would read
      the reverse of FORTH's, in which `[ … ]` runs now and Gforth's
      `]] … [[` postpones code: March's `[ … ]` is the later, a
      quotation, as in Joy and Factor, and needs no `POSTPONE`.
    - **`;` defines within its scope** (Thomas): it makes an entry where
      it stands, and a scope gathers its entries, a namespace's or a
      map's braces, so a record type keeps its `;`s, `{ x: < f64 > ; y:
      < f64 > ; }`. A map's keys are unique and unordered, as a
      namespace's are, a name's entry being its family: a second `;` with
      a new signature adds a clause and one with the same signature
      replaces it (18), as `{ a: 1 a: 2 }` is `{ a' 2 }`. A map is not a
      sequence of pairs (Thomas): pairs in order, repeats allowed, are a
      sequence of tuples, `( ( a' 1 ) ( a' 2 ) )`.

    Built in march8 (2026-10-24), the core, the examples and every test
    moved to it. A label's body
    runs to the `;` that closes the label in its own bracket, so a label
    whose bracket closes first is a key, `{ a: 1 }`, and the reader
    writes the body as a quotation; input that ends inside a body is
    unfinished, so the REPL waits for its `;`. `;` takes a body's first
    check and its last as the clause's signature, and a body that is one
    type as that type; a definition that names its inputs is refused,
    saying a bracket does. A type applied anywhere else checks the values
    on top while compiling, settling a literal, `3 < f64 >.` being 3.0.
    A primitive's signature is written so too, `< i64 i64 >. < i64 >.`.
    `[ … ] !` compiles its quotation as a word of its own, with nothing
    below, and runs it on the stage's machine, which holds all the code
    sealed, then reads back what it left by its types, numbers, strings,
    symbols, and arrays and tuples of them; a value known as the code is
    compiled stays as it is, a quotation or a type too, so `[ … ] ! .`
    works; each runs once, kept by its code's identity, what it writes
    shown before what the program writes; and `io.file.embed` and
    `file-embed` are gone. A body reaching below its first check takes
    those values too, `ff: < i64 >. + ;` two, since dispatch counts a
    clause's inputs by its signature: `;` gives the signature a letter
    for each, one it does not use, where the body's effect is sure, and
    where it is not, a family of clauses of different lengths in it, the
    body is refused where it is applied, as before. A check that cannot
    be decided while compiling, a value's type known only as the program
    runs or a guard in any check, is tested then, a family of one clause,
    the check its signature and nothing its body, failing as `fail` does
    where it does not hold (built 2026-10-26). Not yet: a map it leaves,
    read back. `;` in braces makes an entry of the map around it, as a
    one-letter key may be, `{ x: < f64 > ; }`, a record type's field
    (built with records, 2026-10-24, TYPES.md 2.11); an entry of anything
    but a type is not built yet.

Open: for rules (25), what a group that may not match leaves,
refused for now, and how an import names one alone; and the symbol for
the verb `map` (section 2). To revisit: a map form, `{| … |}` perhaps, as
`[| … |]` is the quotation's (section 3), which `[| … |] map#!` says
already.

## 7. The whole of Low Stack

Settled (Thomas, 2026-10-19), gathered from sections 1 to 6: all that Low
Stack holds, so that what lowers to it, and what reads it, can be checked
against one list.

**What the reader knows.** White space; `'…'`, a symbol with spaces in
it, a quote inside doubled (6.12); comments, from `--` to the line's end,
and a block between ruled lines (6.9); and six meta-ops, each standing
alone: `[` and `]` mark a quotation, `.` applies, `?` tests, `!`
structures and `;` defines (section 1). Every other run of characters is
a symbol, a number among them (6.7). Nothing else is read: no number by
its digits, no capital or one-letter name, no escape or hole, no other
bracket, and no prime, label, slash or divider. A dotted name is a path only to `.`,
which looks it up (6.16), and a pattern matches a name there too (below),
so spelling counts in lookup alone.

**What the meta-ops do.** `.` applies the value on top: a symbol, the
word it names, a quotation, run. `?` applies it as a test, which leaves
one bool, and in a signature is a guard (6.20). `!` structures the value
below by the type on top, and a symbol on top names its type by its
word, `2 i64 !` being `2 i64 . !` (section 2). A quotation on top it
runs while compiling, once, leaving its values known, `[ 1 2 + ] !`
being 3 (6.32, decided 2026-10-24, not built), where it was refused
(Thomas, 2026-10-21: `[ 1 2 + ] !` "doesn't mean anything"). Anything
else on top is refused. What the quotation leaves is not taken as a
type for the value below, so `5 [ i64 ] !` is 5 and the symbol `i64`, not `5 i64 !`:
a bracket's kind is said where it is
made (6.6), and `[ i64 ] sig# !` makes a signature, which `!` takes, here
the type alone. In `[ … ] seq# !` the quotation is the value below, and
`seq#` the type on top. March8 refuses a quotation on top still. `;` binds the name below to
the value on top, with a signature between them if there is one, inputs
then outputs, each made by `sig# !` (section 3, 6.8, 6.18); the name is a symbol, a
path among them, or a pattern, which `;` tells from a symbol by its type.

**The core's words it leans on.** What the higher forms spell, Low Stack
says with words of the core, which lowering writes (section 4): `int#`,
`dec#` and `ratio#` make a number of a symbol (6.7); `seq#` and `map#`
make data of a quotation (6.10), and `sig#` a signature of one (6.8);
`str#` makes text of one, or of a symbol (6.15); `var#` makes a variable of a name, in
a signature a type filled by matching, in a body an input by its place
(6.11, 6.14); `power#` makes a power (6.21), `unit#` an identity and
`identity#` finds it at a type (6.21); `width#` applies a word at a width
(6.22); `pattern#` makes a pattern of a symbol (below), and `name#`
reads a rule's capture as a name where the rule was applied (6.16, 6.25);
`clauses#` and `effect#` read a word, its clauses as data and its effect
(6.6); and `import#` and `export#` are a namespace's keys (6.17). Each ends in `#`,
which marks March's own by habit and means nothing to the reader (6.17).
Types are words (section 2).

**Two spellings, settled** (Thomas, 2026-10-19):

- **A body's letter is `b var# .`**, the word that gives a signature's
  variable. A variable belongs to its bracket, and the bracket fills it:
  a signature by matching, so `a` there is a type its uses fill, and a
  body by place, so `b` there is the second input from the top (6.14).
  `[ b a / ]` is `[ b var# . a var# . / . ]`. A letter is not lowered to
  the stack moves it stands for: lowering is by spelling alone (section
  4), and where an input lies after `foo` in `[ b foo a bar ]` hangs on
  `foo`'s effect, which only the stage knows. The stage moves the inputs
  while compiling, as it does for `_`.
- **A pattern is a symbol made one, `pattern# !`**, as a number is made
  by `int# !`, and `;` takes it where it takes a name (6.25):

  ```
  /log_([0-9]+)/: i64 ! log-base ;
  log_([0-9]+) pattern# ! [ i64 . ! log-base . ] ;
  ```

  A name the rule matches is written as any name is, `log_2 .`: it is a
  symbol until `.` looks it up, and the rule matches it there, in lookup,
  as a path is walked there, so the reader still has no rule. A power and
  a subscript are the core's rules (6.25), so `dup_2` is `dup_2 .`, and no
  name's lowering needs its namespace. So a program that uses rules lowers
  word for word, the syllogism among them: `Socrates Is Human fact` is
  `Socrates . Is . Human . fact .`, its capitals rewritten by the rule of
  6.27 where `.` applies them. Built 2026-10-20.

**The order march8 is to reach it in**, each step keeping the litmus test
(section 5) passing:

1. **Numbers as symbols.** `--symbolic` reads `10` as a symbol, and
   `lower` writes `10 int# !`, `1.5 dec# !` and `1:2 ratio# !`;
   `convert`, the word `!` is read as today, goes (6.5). Built
   2026-10-19 (6.7).
2. **A signature's variables** lower to `a var# .`, and a bare letter in a
   Low Stack signature is a value to match, as any symbol is. Built
   2026-10-21, the first step done the way the rest are to be (Thomas,
   2026-10-21: Low Stack running first, High Stack dressing on top): the
   reader writes the form as Low Stack does, by spelling alone, every
   letter being `a var# .` in a body too; the stage no longer reads it,
   and the word that gives it a meaning is the core's, `var#`. A
   message writes it back as it was written. In a body `var#` said that
   places were not built until step 5.
3. **Brackets**, the largest step: `( … )` and `{ … }` unsealed (6.14)
   and lowered to `seq# !` and `map# !`, and `< … >` lowered to `sig#
   !`, which `;` takes (6.8). Lowered 2026-10-21, as step 2 was: the
   reader writes each bracket as `[`, and where it closes its kind and
   `!`, the kind where the bracket opened, which says it; the stage
   reads `[` alone, and `!` makes the quotation a sequence, a map or a
   signature, so a primitive's signature, a definition's parts and a
   message read the new form too, the message writing `( … )` back. The
   compiled code is the same, the core's and every example's. Unsealed
   the same day (6.14): a sequence's or a map's code reaches what `.`'s
   would, and the stage notes how deep, where its elements start; the
   compiled code is still the same, since nothing compiled before reached
   below.
4. **Strings**, as `[ … ] str# !` of `'…'` pieces and code (6.15).
   Built 2026-10-22, as steps 2 and 3 were: the reader writes `"…"` as a
   quotation of its text, each run a symbol, and its holes' code, `\_`
   being `_ .`, closed by `str#` and `!`, the kind where the string
   opened; the stage reads no string, and `!` makes the quotation text by
   `str#`, a kind as `seq#` is. Low Stack reads no `"`, and a message
   writes `"…"` back. Unsealed with it (6.14). The compiled code is the
   same, the core's and every example's.
5. **Letters as places**, built in High Stack (6.14), lowering to
   `b var# .`, with each `_` lowered to its letter. Built 2026-10-22, as
   steps 2 to 4 were: the reader writes each bracket's `_`s as their
   letters, and the stage reads no `_`. A letter in code is `a var# .`
   still, the stage giving it its place where it is written, since the
   place is its bracket's. The compiled code is the same, the core's and
   every example's, and a message names the letter, `\_` being `a`.
6. **Rules**, as above, matched in lookup against the symbol `.`
   applies, not the word as typed (6.25); then the syllogism runs
   lowered. Built 2026-10-20: a recognizer is a rewrite rule, the core's
   power and width are rules of the core's, and a name keeps its
   subscript and its power when lowered, so lowering needs no namespace
   (6.21, 6.22).
7. **The litmus test with no exceptions:** the core and every example
   lower, read back to the same tokens, and run the same. Built
   2026-10-22: nothing in the stage needed to change, since steps 2 to 5
   left it reading Low Stack alone. The core read lowered compiles every
   example to the same code, byte for byte, and all of march8's tests
   pass on it; the summary, which reads its input, runs lowered as a
   `.s4` file. Only a message's position in the core differs, since
   `lower` keeps a line's breaks but not its blank and comment lines.
