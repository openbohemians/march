# Symbolic Forth

Status: design note, 2026-10-13, from a conversation with Thomas. A first
account of March's lowest form; a precise specification is to come.
March8 reads and writes it: `march8 lower` writes any March in Symbolic
Forth, `--symbolic` reads it, and march8/tests/symbolic.rs is its litmus
test (section 5). Revised 2026-10-14: one bracket, a quotation; six
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
pattern words found in lookup (section 25).

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
  sequence, `[ … ] seq# !`; and **`;` defines**, binding the name below
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
sq [ i64 . ] seq# ! [ i64 . ] seq# ! [ dup . * . ] ;
fib [ n var# . ] seq# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
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
`(string*)` or `{sym:str}`, which a pattern word (6.25) would read
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
  March8 does not have this yet: there `( … )` is sealed, and `5 ( 1 + )`
  is an error.
- **`{ … }`** is a map literal, `[ … ] map# !`: a set of pairs (section
  1), undecided, and taking from below, in the same way (Thomas,
  2026-10-14).
- **`< … >`** is code run while compiling, whose values are a type or a
  signature, `< string i64 map k >`. `--` divides inputs from outputs,
  and first in a definition's quotation it is the clause's context. It
  needs no kind of its own: code run where it stands whose values are
  gathered is a sequence, so `< … >` is `[ … ] seq# !`, a sequence of
  types.
- **`"…"`** is a string, with holes of code: `"\_ apples"`. In the lowest
  form text is a quoted symbol, `'…'`, which a type structures, and a
  string with holes is a quotation of its pieces made text,
  `[ '1 + 2 = ' 1 int# ! 2 int# ! + . ] str# !`, as `seq# !` makes a
  sequence of one (6.15).
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

A signature in the lowest form is two sequences, inputs then outputs, and
needs no divider: `< a* -- b* >` is `[ a* ] [ b* ]` (Thomas, 2026-10-14),
each a sequence of types, `[ a* ] seq# ! [ b* ] seq# !`. One sequence
gives the inputs alone, and the stage finds the outputs; `[ n var# . ]
seq# ! [ ] seq# !` declares that there are none. `;` takes a clause's
signature before its body, so no bracket is read by where it stands:

```
sq [ dup . * . ] ;                         -- name, body
fib [ n var# . ] seq# ! [ … ] ;             -- name, inputs, body
sq [ i64 . ] seq# ! [ i64 . ] seq# !
   [ dup . * . ] ;                         -- name, inputs, outputs, body
```

`;` tells these apart by type, since a name is a symbol and a
sequence is not. And with no divider, `--` begins a comment everywhere,
inside a quotation as well; High Stack keeps it as the divider in a
signature's bracket (6.9).

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
| `a` | `a var# .` | one lowercase letter is a variable |
| `[ 2 + ]` | `[ 2 int# ! + . ]` | a quotation |
| `[ 2 + ].` | `[ 2 int# ! + . ] .` | a dot at a word's end applies once more |
| `2 i64!` | `2 i64 . !` | `!` structures by a type |
| `positive?` | `positive ?` | `?` tests |
| `10`, `1.5`, `1:2` | `10 int# !`, `1.5 dec# !`, `1:2 ratio# !` | a number's spelling gives its kind |
| `< i64 -- i64 >` | `[ i64 . ] seq# ! [ i64 . ] seq# !` | a signature is two sequences |
| `[\| sq math.pi \|]` | `[ sq math.pi ]` | the lowest form inline |
| `( 1 2 )` | `[ 1 int# ! 2 int# ! ] seq# !` | a sequence literal |
| `5 ( _ 1 )` | `5 [ a var# . 1 int# ! ] seq# !` | `_`, an argument, is a letter |
| `"\_ apples"` | `[ a var# . ' apples' ] str# !` | a string with an argument |
| `dup_2`, `dup₂` | `dup_2 .`, `dup 2 int# ! width# . .` in lookup | a width, by the word, a pattern word of the core (6.25) |
| `+^4`, `+⁴` | `+^4 .`, `+ 4 int# ! power# . .` in lookup | a power, by the word's kind, likewise |
| `[ dup + ]^3` | `[ dup . + . ] 3 int# ! power# .` | a quotation's power, a quotation |

A capital marks nothing (6.27): `Snake` is a name and applies, so
`Foo: [ 1 2 + ] ; Foo` runs to 3, and a symbol is quoted, `Snake'`.

```
fib: < n > [ dup 1 -+ fib swap 2 -+ fib + ] ;
fib [ n var# . ] seq# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
```

March8 reads it so (2026-10-15), and, until signatures are sequences,
lowers it to `fib < n > [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ .
fib . + . ] ;`.

A surface may come above the intermediate form, as SURFACE.md's headings
were to be. How many stages there are is still to be discovered; Symbolic
Forth is the one at the bottom.

## 5. The litmus test

What cannot be said in Symbolic Forth is not March. Every notation above
it must lower to it, mechanically, and mean the same.

March8 runs the test. `march8 lower` writes March as Symbolic Forth,
keeping its lines, and `--symbolic` reads it. march8/tests/symbolic.rs
lowers the core vocabulary and the examples, reads them back to the same
tokens, and runs the examples lowered to the same results. Code as data
is this form: what a program builds as code, it builds in Symbolic Forth.

## 6. Toward a specification

Settled (Thomas, 2026-10-13):

1. **Text** is a symbol that may hold white space, written quoted, `'this
   is symbols with spaces in it'`, and a type structures it into a
   string. An ordered set of code points would do too, but that is the
   long way round. March8 reads `'…'` as a symbol (28), which `string !`
   makes a string, but lowering still writes strings as strings (section
   7, step 4).
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
8. **Signatures are two sequences**, inputs then outputs, `[ a* -- b* ]`
   being `[ a* ] [ b* ]`, each a sequence of types, `seq# !`; one gives
   the inputs alone. `;` takes them before the body (section 3).
9. **Comments** run from `--` to the line's end, everywhere in the
   lowest form, since no divider is needed in a signature. High Stack
   keeps `--` as the divider in a signature's bracket, `< i64 -- i64 >`,
   FORTH's stack comment, and a comment elsewhere (Thomas, 2026-10-15:
   minor, to reconsider later). A line of three or more `-` and nothing
   else opens a block comment, and a line of them ending in a period,
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
    until `seq# !`.
11. **Variables are given by a word**, `n var# .`, as types are, so no rule
    of spelling is left in the lowest form; one lowercase letter is a
    variable only above it. A body's letter is given by the same word,
    `b var# .` (section 7).
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

    March8 lowers `_` as `_ .`, a word, until letters are built.

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
    scratch stack. `march8 lower` still writes `( … )` and `"…"` as they
    are, until `seq#` and `str# !` are the lowest form's.

    No bracket is sealed (Thomas, 2026-10-17): a quotation and a literal
    alike take what their code needs, so `[ + ]` takes two and `1 2 ( + )`
    is `( 3 )`, and concatenation stays composition. A `_` makes an order
    easy, `[ _ foo _ bar ]` for `[ swap foo swap bar ]` when `foo` leaves
    one value; it does not declare what a bracket takes, so `( _ _ + )` is
    `( + )`. Sealing every bracket, each taking only its `_`s, would put
    them in every word that takes an input, `sq: [ _ dup * ] ;`. March8's
    literals are still sealed (section 3).

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
    filled by matching (section 2), so in `at: < a ary i64 -- a > [ … ]`
    the signature's `a` and the body's are two variables. In Low Stack a
    body's letter is `b var# .` (section 7). Not built yet.

    Names can be an editor's (Thomas, 2026-10-17, an idea). A letter being
    only a place, a word may carry a key naming its places, and an editor
    show the names where the letters are, `qty price *` for `b a *`, while
    the source holds the letters. Names are then presentation, as Unison
    keeps names apart from code and de Bruijn indices keep a term
    nameless: renaming changes no code, and a name never clashes with a
    word. Where the key is kept waits for an editor (2026-10-19); a
    signature's labels are the likely place, `< qty: i64 price: f64 >`.
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
    As `( … )` is in march8, its code reaches only its arguments and what
    it makes, for now.

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
    settles to i64.

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
    `math`'s that `map` runs still finds `math.sq`; a bare name defined by
    a namespace's code is that namespace's. There a namespace is the words
    whose paths it begins, not yet a map, so `math .` and `at` on it are
    still to come.

17. **Import and export** (Thomas): a namespace's map has two keys of
    March's own, `import#` and `export#`, named so as not to take words a
    program wants, `csv.export`; a trailing `#` marks March's own names,
    as in `int#` and `seq#`. Setting them is a definition of data:

    ```
    import#: [| math.* io.file.read |] ;     -- each local by its last part
    import#: [| circle math.pi |] map#! ;    -- or named, as pairs
    export#: [| pi e sin cos |] ;
    ```

    `import#` makes names local, each by its last part or by the name
    paired with it, and `math.*` makes local all that `math` exports. A
    bare name is looked up locally, then among the imports, then in the
    core. A path is absolute, so it means the same anywhere. An import
    that meets a local name or another import is an error, settled by
    naming it. A namespace has one import, which holds for all of it
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
    severe. Not yet: the pairs that name an import, which wait for
    `map#`, and interfaces by hash, which wait for the store.

    **What `#` means** (Thomas, 2026-10-19): nothing to the reader. It is
    part of the name, as any character is, and a habit of naming, not a
    rule: a name ending in `#` is March's own, so a program keeps the
    plain word, `csv.export` beside `export#`. March's own are of three
    kinds: the literal kinds, `int#`, `dec#`, `ratio#`, `str#`, `seq#` and
    `map#` (TYPES.md 2.10); the words that say what a spelling said,
    `var#`, `power#`, `width#` and `pattern#` (section 7); and a
    namespace's keys, `import#` and `export#`. A vocabulary stays plain,
    the core's types and verbs among it, `i64`, `string`, `map`, `dup`:
    those are words a program uses, not machinery it lowers to. Nothing
    holds a program to the habit, as nothing holds it to a case (27).
18. **`;` defines**, a meta-op (Thomas). Through all of the above one
    word stayed constant, `def`, applied in every program, so it is named
    for what it is, FORTH's `;`: `sq: [ dup * ] ;` is `: sq dup * ;`
    turned postfix, the label's colon opening and `;` closing. It binds the
    name below to the value on top, with a signature between them if
    there is one, code and data alike, and adds a clause to the word of
    that name, as `def` did. So a second `;` on a constant's name adds a
    clause rather than replacing the value: replacing an entry is the
    global store's write, for the store's design. Built in march8
    (2026-10-15), with the signature before the body, in one bracket or
    in two, inputs then outputs; a body that begins with a signature is
    refused, saying where it goes.

19. **Symbols applied at run time** (Thomas). `.` applies a symbol known
    while compiling, or one its type bounds: a union of symbols, compiled
    as a switch among known words, or a named signature, `adjustment:
    < price -- price >` (Thomas, 2026-09-26), whose words are compiled at
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
    types it meets: that is why the stage must be there.

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
    joined by an operation. The word's stack effect gives the kind:

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
    literal's is. A type's power is its product, so in a signature
    `i64^3` is three i64s, as ℝ³ is. n is written, a number in the source.

    A power is a quotation, which the word `power#` makes from a word or
    a quotation and the count, so a quotation's power is the quotation of
    the power, `[ q ]^n` being `[ q^n ]` by q's kind: `[ dup + ]^3` is
    `[ dup + dup + dup + ]`, and `[ + ]^3` is `[ + + ]`, as `+^3` is. The
    superscript changes what a thing is, not whether it runs: a name still
    applies, and a quotation is still a value, for `.` to apply or `map`
    to take, `( 1 [ dup + ]^3 . )` being `( 8 )` and `( 1 2 3 ) [ dup + ]^3
    map` being `( 8 16 24 )`. `power#` needs the effect of what it takes,
    so the stage expands it, from known code only, as `.` applies only
    known code (4). Where the effect is not known while compiling, for a
    family whose clauses take different numbers of values or a symbol
    applied at run time (19), it is an error, and the words are written
    out.

    This replaces `w*n`, n copies in a row (march7/docs/SURFACE.md,
    "Copies and operands"): `^` says all of it, a value's copies being
    `pi dup^2`, since `dup` is how one gets a copy. `*` stays only in a
    signature, with no count, `t*` any number of t and `a*` the rest of
    the stack: the regular expression's star, which mathematics writes as
    Σ*. Built in march8 (2026-10-15).
22. **A subscript is a width,** a spelling as the power is (Thomas). `w_n`,
    written `wₙ`, is w with each value it works on a group of n, and
    lowers as `^n` does, `=_2` being `= 2 int# ! width# . .`, so no word
    needs defining at each width: `=_2` and `dup_2` are FORTH's 2DUP, `~_2`
    its 2SWAP, `over_2` 2OVER and `rot_2` 2ROT, and `dip_2`, `keep_2` and
    `bi_2` are Factor's 2dip, 2keep and 2bi. The stage widens a word as it
    powers one: inside `w_n`, w's own words act on groups, and a quotation
    handed to w is not widened but runs once, as written, on the group's
    values. So `keep_2` is 2keep, though `keep`'s own `over [ . ] dip` moves
    a pair past a quotation. An operation acts lane by lane, the first of
    each group with the first, as vectors add, so `1 2 10 20 +_2` is `11
    22`; the stack words agree, 2DUP being dup in each lane. `width#` leaves
    a quotation, as `power#` does, and the width comes before the power,
    `dup_2^3` being `dup_2` cubed, which is `dup` cubed two wide; so it
    lowers with the power first, `dup 3 power# . 2 width# .`, the power
    taken by the kind of the word itself, as `+_2^3` sums three pairs,
    though `+_2` takes four values. So `==` and `~~` go. Built in march8
    (2026-10-15), where a group's values at run time move by `pick`, by
    rolls, or through the scratch stack, `dup_2` being `over over`.
23. **`fmt` writes them as mathematics does,** by LaTeX's marks, `^` up
    and `_` down: a name's trailing `^` and digits as superscript digits,
    `+^4` as `+⁴`, and its trailing `_` and digits as subscript digits,
    `dup_2` as `dup₂`; only digits, and only at a name's end, so
    `file_read` stays as it is. The reader reads them back, `₂` as `_2`
    and `⁴` as `^4`, as it is to read `⟨ ⟩` as `< >`
    (march8/docs/MACHINE.md), and the ASCII spelling is the one kept, the
    one typed, the width first, `dup_2^3` as `dup₂³`. March8's escape `\_2`
    writes `₂`, and is the same spelling. Built in march8 (2026-10-15).
24. **No name ends in a power or a width** (Thomas). The reader takes
    `^n` and `_n` off, as the core's pattern words are to in lookup (25),
    so a word named `dup_2` could never be reached, and
    defining one is an error that says what the spelling means. FORTH
    lets a word redefine a number, searching its dictionary first; here
    what a spelling lowers to would then hang on what happens to be
    defined, and one definition would change every use unseen. A
    specific meaning is a clause instead, of `power#` or `width#`, for
    that word and that count: a family chooses its most specific clause,
    so the clause wins over the general rule, as FORTH's dictionary wins
    over its numbers, while the reader still lowers by spelling alone.
    The identities need it, since the stage cannot know that `+`'s is 0:

    ```
    power#: < +' 0 > [ drop drop [ 0 ] ] ;          -- +^0 is 0
    power#: < *' 0 > [ drop drop [ 1 ] ] ;          -- ⋅^0 is 1
    width#: < dup' 2 > [ drop drop [ over over ] ] ;
    ```

    The first two are in march8's core.march, which matches a quoted word
    as a value, as it does a number or a symbol.
25. **Recognizers: March defines its spelling rules** (Thomas). The rules
    that read High Stack (26), `^n` and `_n` among them, can be March code,
    recognizers, as FORTH names them, each taking a word's text and giving
    what it lowers to, and a program may add its own: `log_2` can be a
    spelling of its own, base 2's log, more specific than the width, where
    24 refuses it as a name. A definition still cannot change how text
    reads, only a spelling rule, which says that it is one, so reading hangs
    on spelling rules alone and the litmus test holds: `march8 lower` still
    writes any program in Low Stack. In FORTH a recognizer is a word that
    takes a token's text and says how to interpret or compile it, by which
    Gforth reads its numbers; Lisp's kin is the reader macro, Factor's the
    parsing word. With them March reads itself by its own words, a step up
    in kind. Very powerful, and an easy way to win an obscure-code contest:
    not to be used lightly (Thomas).

    A recognizer is a word whose name is a pattern (Thomas, 2026-10-16):
    a regular expression between slashes, which matches a whole word, then
    a signature and a body, as any word has. A name is a pattern that
    matches only itself; a pattern names a set of words, an indexed family,
    as math writes log_b. What the pattern's groups capture goes on the
    stack, each a symbol, in the order written, after the word's inputs, as
    if written where the word stands: the word is an ordinary word with
    inputs its name supplies, which `int# !` or `i64 !` makes numbers (7).

    ```
    logs./log_([0-9]+)/: < f64 sym -- f64 > [ i64 ! log-base ] ;
    import#: [| logs.* |] ;
    1024 log_2                       -- 1024 '2' i64 ! log-base
    ```

    The signature states what the body sees, the captures last; to its
    user `log_2` is `< f64 -- f64 >`, one input fewer for each group.
    Without a signature the effect is found, as any word's is, for each
    word the pattern matches. Clauses of one pattern are a family. The
    captures are on the stack, not marks, `$1`, since a mark would want a
    spelling in Low Stack, and `dup` serves a capture used twice; after the
    inputs, not beneath them, since that is where the word stands,
    `x log_2` reading as `x 2 log-base`, as the core's own `log₂'` reads as
    `log 2 width#`, and beneath them the stage would reach under its user's
    values. A group that may not match must still leave something, so the
    effect is fixed: `nil` perhaps, of `sym nil or`, or such groups refused.

    A pattern word is an entry of its namespace, as any word is, and
    reaches where that namespace is imported (17; Thomas, 2026-10-15), and
    in its own namespace, as a word is used where it is defined. It holds
    for the whole namespace, wherever it is written, as an import does
    (17), so no text reads one way before it and another after; its
    slashes say it is a pattern, so it is found before the rest is read. It
    reads a word as typed, without its marks, `log_2` for `log₂'`, and
    before the core's own spelling rules, so a program's `log_2` is not the
    core's width, as a bare name is looked up among the imports before the
    core (17). It reads a word the core reads as a name, `12kg` among them,
    but not a number, `;` or `_`. A word that two patterns match, of one
    namespace or of two, is an error (Thomas), so their order never counts;
    so is a word a pattern matches that is also a local name or an import,
    settled by naming it (17). A pattern word comes with `logs.*`, as any
    exported word does.

    In FORTH a recognizer runs anything while reading. Here only the
    pattern reads, choosing the word and its captures; the body runs where
    the word stands, as any word's does, or with `'` the word is passed,
    `log_2'`, as a name is. As a pattern word, the core's width would be
    `/(.+)_([0-9]+)/: [ int# ! width# . . ] ;`, the word widened, then
    applied, with no signature, since its effect is the widened word's.

    The slashes are a spelling of High Stack. In Low Stack `;` takes a
    pattern where it takes a name, the symbol made a pattern by `pattern#
    !`, as `int# !` makes a number (section 7); so `;`'s first slot
    widens from one name to a set of names, and no slot changes what it
    means. A pattern matches a word, which has no spaces, so it needs none;
    a `/` in it is written `\/`.

    This replaces a first form (2026-10-15), a namespace's clauses of
    `recognize#`, the pattern in the signature's slot and a template whose
    holes, `_`, took the captures,
    `logs.recognize#: < 'log_([0-9]+)' > [ _ int# ! log-base ] ;`. It copied `import#` and `export#`, but those are
    settings of a namespace, what it sees and what it shows, while a
    recognizer is vocabulary; and under its key `< >` held a pattern, not a
    signature. A key may give meaning to its value, as `import#` does to its
    list, but should not change what a slot means, each such exception a
    cost, though one may be made on purpose where it keeps a program from
    harm (Thomas, 2026-10-16). A map from pattern to template, one
    `recognize#` to a namespace, as the import pairs will be, mended the
    slot but kept the key, and is dropped with it.

    Built in march8 (2026-10-16), by Rust's `regex` crate, which matches
    in linear time (Thomas: March wants regular expressions anyway, if only
    as a module). March may come to have a pattern syntax of its own, a
    regex then an optional module, which March itself never needs (Thomas,
    2026-10-16). A source's patterns are known before it runs, and the
    core's text is read by the core's rules alone. A pattern word's code is
    its captures, then its word, the pattern's path, applied, so its
    signature, family and effect are a word's; a symbol becomes a number
    only where it is known, so a clause typed `sym` is checked where it is
    applied, not where defined. Each group must capture in every match: an
    optional group is refused, for now. Only `*` imports a pattern word;
    an export names it by its pattern, `export#: [| /log_([0-9]+)/ |]`.
    March8 learns a body's namespace only at `;`, so the stage, not the
    reader, reads a word by a pattern, when it reaches the word in its
    code's namespace, as it looks up a name. So `march8 lower` still
    writes the core's reading, and writes the pattern's name as a symbol,
    which `;` reads back. Lowering a program that uses pattern words writes
    each word a pattern claims as a word (section 7), which march8 can do
    once the core's width and power are found in lookup (below). A captured name, `(.+)`, is a
    symbol applied in the body's namespace, not where the word was
    written, as a passed `sq'` is (16); that is to be settled when
    namespaces become values. The cost, measured for the first form,
    whose matching this keeps: about 25 ns a word where patterns are
    known, and none elsewhere.

    **The core's width and power are found in lookup** (Thomas,
    2026-10-19; not built). They move from the reader into the core as
    pattern words, the width `/(.+)_([0-9]+)/: [ int# ! width# . . ] ;`
    and the power likewise. Then no word needs its namespace to be
    lowered: `dup_2` lowers as `dup_2 .` and `log_2` as `log_2 .`, as
    `dup` lowers as `dup .`, and which word each is, is found where any
    name's is, in lookup, in the namespace its code is written in. A
    program's `log_2` wins over the core's width by the order every name is
    found in, local, then imports, then the core (17), with no rule of its
    own. While the reader kept them, it had to choose between the width and
    a program's pattern before lookup, and only the namespace could say:
    the seam was the core's spelling rules in the reader, not pattern
    words (Thomas: "isn't that true of any word?"). It costs two things.
    Lowered code shows less, `dup_2 .` where it showed `dup 2 int# !
    width# . .`, which the core's pattern makes of it in lookup and which
    can still be written. And lookup does more, though it tries the core's
    patterns only where nothing else is found, so a word that is found pays
    nothing. A quotation's power, `[ dup + ]^3`, stays a spelling of the
    reader: it is on a bracket, which no pattern reads. Subscript and
    superscript digits still read as `_2` and `^2` (23), as `\times`
    reads as `×`. And 24 holds: no name ends in a width or a power, so
    only a pattern, which says it is one, claims `log_2`. March8 matches a
    pattern against the word as written in High Stack, which Symbolic
    Forth does not keep, and so the syllogism does not yet run lowered;
    matching the symbol `.` applies ends that.
26. **High Stack and Low Stack** (Thomas, to try). The surface March is
    written in, today the intermediate form of section 4, is High Stack;
    Symbolic Forth, the lowest form, is Low Stack.
27. **A capital marks nothing** (Thomas, 2026-10-17). Words are
    case-sensitive, so a capital need not mark a symbol: `Snake` is a name
    like any and applies, and `Snake.` goes. A symbol is quoted where it
    is used, `Snake'` or `'Snake'`, or given by a word that leaves it,
    `Snake: Snake' ;`, as a Lisp keyword evaluates to itself. A namespace
    of symbols, a state machine or facts and rules, imports the old rule
    as a pattern word (25) whose capture is the word itself,
    `/([A-Z][A-Za-z0-9]*)/: < sym -- sym > [ ] ;`, and there
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
    examples/syllogism.march reads its capitals by the pattern word.
    `lower` does not yet write what a pattern word reads (25, open), so
    the syllogism is not run lowered.
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

Open: for pattern words (25), what a group that may not match leaves,
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
word, `2 i64 !` being `2 i64 . !` (section 2). `;` binds the name below to
the value on top, with a signature between them if there is one, inputs
then outputs, each a sequence (section 3, 6.18); the name is a symbol, a
path among them, or a pattern, which `;` tells from a symbol by its type.

**The core's words it leans on.** What the higher forms spell, Low Stack
says with words of the core, which lowering writes (section 4): `int#`,
`dec#` and `ratio#` make a number of a symbol (6.7); `seq#` and `map#`
make data of a quotation (6.10), a signature being two sequences (6.8);
`str#` makes text of one (6.15); `var#` makes a variable of a name, in
a signature a type filled by matching, in a body an input by its place
(6.11, 6.14); `power#` and `width#` make a power and a
width (6.21, 6.22); `pattern#` makes a pattern of a symbol (below); and
`import#` and `export#` are a namespace's keys (6.17). Each ends in `#`,
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
  /log_([0-9]+)/: < f64 sym -- f64 > [ i64 ! log-base ] ;
  log_([0-9]+) pattern# ! [ f64 . sym . ] seq# ! [ f64 . ] seq# !
     [ i64 . ! log-base . ] ;
  ```

  A word the pattern claims is written as any word is, `log_2 .`: it is a
  symbol until `.` looks it up, and the pattern matches it there, in
  lookup, as a path is walked there, so the reader still has no rule. The
  core's width and power are pattern words too (6.25), so `dup_2` is
  `dup_2 .`, and no word's lowering needs its namespace. So
  a program that uses pattern words lowers word for word, the syllogism
  among them: `Socrates Is Human fact` is `Socrates . Is . Human . fact
  .`, its capitals read by the pattern word of 6.27 where they are looked
  up.

**The order march8 is to reach it in**, each step keeping the litmus test
(section 5) passing:

1. **Numbers as symbols.** `--symbolic` reads `10` as a symbol, and
   `lower` writes `10 int# !`, `1.5 dec# !` and `1:2 ratio# !`;
   `convert`, the word `!` is read as today, goes (6.5). Built
   2026-10-19 (6.7).
2. **A signature's variables** lower to `a var# .`, and a bare letter in a
   Low Stack signature is a value to match, as any symbol is.
3. **Brackets**, the largest step: `( … )` and `{ … }` unsealed (6.14)
   and lowered to `seq# !` and `map# !`, and `< … >` lowered to two
   sequences, which `;` takes (6.8).
4. **Strings**, as `[ … ] str# !` of `'…'` pieces and code (6.15).
5. **Letters as places**, built in High Stack (6.14), lowering to
   `b var# .`, with each `_` lowered to its letter.
6. **Pattern words**, as above, matched in lookup against the symbol `.`
   applies, not the word as written, and the core's width and power moved
   there from the reader as pattern words of the core, so lowering needs
   no namespace (6.25); then the syllogism runs lowered.
7. **The litmus test with no exceptions:** the core and every example
   lower, read back to the same tokens, and run the same.
