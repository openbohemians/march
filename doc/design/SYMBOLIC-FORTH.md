# Symbolic Forth

Status: design note, 2026-10-13, from a conversation with Thomas. A first
account of March's lowest form; a precise specification is to come.
March8 reads and writes it: `march8 lower` writes any March in Symbolic
Forth, `--symbolic` reads it, and march8/tests/symbolic.rs is its litmus
test (section 5). Revised 2026-10-14: one bracket, a quotation; five
meta-ops; numbers as symbols; signatures as two sequences; holes as sugar;
text made by `string !`; namespaces as maps, with imports and exports;
and no rules of spelling. March8 does not have these yet.

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
  evaluated; and **`!` structures** the value below by the type on top, a
  number symbol into a concrete i64, `2 i64 !`, or a quotation into a
  sequence, `[ … ] seq# !`; and **`;` defines**, binding the name below
  to the value on top, `sq [ dup . * . ] ;`. `.` is the store's one read,
  and `;` its one write.

Besides these the reader knows only white space, the quotes of a symbol
with spaces in it, `'…'`, a quote inside doubled, `'it''s'`, and comments,
from `--` to the line's end. A meta-op stands alone between white space,
so inside a symbol `.`, `!` and `;` are characters. No rule of spelling is left
in reading: a name means what is done with it, never how it is written
(Thomas, 2026-10-14). The one place spelling counts is lookup: `.` applies
a dotted name by its path, `math.pi .` (6.16).

A side note on the equivalence: every meta-op but `.` could be a
primitive word applied, `!` as `convert .`, `;` as `def .`, and `]` as
`close-bracket .` (a word that, as FORTH's immediate words do, runs even
while a set is being gathered). A primitive is needed either way, so the lowest level takes
these few as meta-ops; what can be said is the same (Thomas,
2026-10-14). `.` cannot be a word applied: applying it would need
application already.

```
1 i64 ! 1 i64 ! + .
sq [ dup . * . ] ;
sq [ i64 . ] seq# ! [ i64 . ] seq# ! [ dup . * . ] ;
fib [ n var . ] seq# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
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
type variable is given by a word too, `n var .` (Thomas, 2026-10-14),
where above the lowest form one lowercase letter is one.

So types need no spelling of their own, and types and words are one
namespace. In a signature a type must be given, `i64 .`, not named: there
a symbol left bare is a value to match, `Open`, so a bare `i64` would be
read by its spelling, or by what happens to be defined.

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
  holds the symbol `+`, since nothing applies without a dot. A surface's
  `_` is sugar above this: a hole, filled before anything runs (6.14).
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
  `[ '1 + 2 = ' 1 int# ! 2 int# ! + . ] string !`, as `seq# !` makes a
  sequence of one (6.15).
- **`<| … |>`** is the lowest form's quotation written inline, a pure
  container of symbols (Thomas, 2026-10-14): its contents are read as
  Symbolic Forth, so nothing applies without a dot, a number is a symbol,
  and a capital marks nothing. It lowers word for word, `<| math.pi
  math.e |>` being `[ math.pi math.e ]`, and is the intermediate form's
  escape from its own rules of spelling, as Lisp's `quote` is. It writes
  lists of names, `<| math.* io.file.read |>` (6.17), and code as data
  as it will be stored. A map form, `{| … |}` perhaps, is to come.

A signature in the lowest form is two sequences, inputs then outputs, and
needs no divider: `< a* -- b* >` is `[ a* ] [ b* ]` (Thomas, 2026-10-14),
each a sequence of types, `[ a* ] seq# ! [ b* ] seq# !`. One sequence
gives the inputs alone, and the stage finds the outputs; `[ n var . ]
seq# ! [ ] seq# !` declares that there are none. `;` takes a clause's
signature before its body, so no bracket is read by where it stands:

```
sq [ dup . * . ] ;                         -- name, body
fib [ n var . ] seq# ! [ … ] ;             -- name, inputs, body
sq [ i64 . ] seq# ! [ i64 . ] seq# !
   [ dup . * . ] ;                         -- name, inputs, outputs, body
```

`;` tells these apart by type, since a name is a symbol and a
sequence is not. And with no divider, `--` begins a comment everywhere,
inside a quotation as well.

A signature has two kinds of hole. Values its code takes from below must
be known while compiling, since a type depends on no value known only at
run time (TYPES.md 2.4), so `T [ ary . ] seq# !` needs T a type by then.
Type variables, `n var .`, stay open: each use fills them by matching,
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
| `Snake` | `Snake` | a capital marks a symbol |
| `Snake.` | `Snake .` | a symbol applied: the word it names |
| `+'` | `+` | a prime marks a word passed |
| `fib:` | `fib` | a label is a symbol |
| `a` | `a var .` | one lowercase letter is a variable |
| `[ 2 + ]` | `[ 2 int# ! + . ]` | a quotation |
| `[ 2 + ].` | `[ 2 int# ! + . ] .` | a dot at a word's end applies once more |
| `2 i64!` | `2 i64 . !` | `!` structures by a type |
| `10`, `1.5`, `1:2` | `10 int# !`, `1.5 dec# !`, `1:2 ratio# !` | a number's spelling gives its kind |
| `< i64 -- i64 >` | `[ i64 . ] seq# ! [ i64 . ] seq# !` | a signature is two sequences |
| `<\| sq math.pi \|>` | `[ sq math.pi ]` | the lowest form inline |
| `( 1 2 )` | `[ 1 int# ! 2 int# ! ] seq# !` | a sequence literal |
| `5 ( _ 1 )` | `5 [ 1 int# ! ] cons . seq# !` | a hole, filled first |
| `"\_ apples"` | `[ ' apples' ] cons . string !` | a string with a hole |

A capital marks only that the reader adds no dot, so `Snake.` is what
`Snake .` is anywhere, the word of that name applied: march8 runs
`Foo: [ 1 2 + ] def Foo.` to 3, and `Open.`, with no word `Open`, is an
error.

```
fib: < n > [ dup 1 -+ fib swap 2 -+ fib + ] ;
fib [ n var . ] seq# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
```

Until these are built, march8 reads the old form,
`fib: [ < n > dup 1 -+ fib swap 2 -+ fib + ] def`, and lowers it to
`fib [ < n > dup . 1 -+ . fib . swap . 2 -+ . fib . + . ] def .`.

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
   long way round. March8 does not have it yet: there `'…'` is a raw
   string, and lowering writes strings as strings.
2. **Maps** are ordered sets of pairs, not a fundamental.
3. **`!`** structures a value by a type. It was `convert .` first, a
   convenience over a primitive word; it is now a meta-op (5). March8
   still reads it as `convert .`.

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
   structuring.
8. **Signatures are two sequences**, inputs then outputs, `[ a* -- b* ]`
   being `[ a* ] [ b* ]`, each a sequence of types, `seq# !`; one gives
   the inputs alone. `;` takes them before the body (section 3).
9. **Comments** run from `--` to the line's end, everywhere, since no
   divider is needed in a signature.
10. **`seq#` and `map#`**, the sequence and map literals: what a
    quotation's code leaves, collected, `[ … ] seq# !`, which `( … )`
    writes, or paired, `[ … ] map# !`, which `{ … }` writes; their code
    takes what it needs from below, and they are undecided until taken,
    as `int#` is. A sequence that waits to be made is a quotation, held
    until `seq# !`.
11. **Variables are given by a word**, `n var .`, as types are, so no rule
    of spelling is left in the lowest form; one lowercase letter is a
    variable only above it.
12. **Quoting.** A quote inside a quoted symbol is doubled, `'it''s'`,
    the one rule the reader needs to find where `'…'` ends. A symbol holds
    its characters as they are, newlines among them. Escapes, `\n;` and
    `\times;`, are spellings of the intermediate form, decoded by its
    reader, so the lowest form has none, and a symbol and the string made
    from it hold the same text.
13. **A pull takes.** `_` takes a value from below, and a copy is `dup`,
    as everywhere. March8's pulls copy, read from the literal's mark
    (`mark-pick`), so that one in a loop reads the same value each pass,
    `10 ( xs [ _ + ] each )`. A hole is filled first (14), so the value is
    written into the loop's code instead, `( xs [ 10 + ] each )`, and
    `mark-pick` goes.
14. **Holes are sugar.** The surface fills a bracket's holes first,
    before anything runs: `1 2 3 [ _ _ + _ / ]` is `[ 1 2 + 3 / ]`, so
    `[ 1 ]` (Thomas). The first `_` takes the deepest value. The lowest form
    has no holes. It needs only a way to put a value into a quotation, the
    ordinary sequence word that puts one at the front, `cons`, as Joy's
    does, `10 [ + . ] cons .` being `[ 10 + . ]`, and `compose`, which
    joins two quotations:

    ```
    5 ( _ 1 )                5 [ 1 int# ! ] cons . seq# !
    10 ( xs [ _ + ] each )   10 [ [ + . ] cons . xs . swap . each . ] seq# !
    1 2 "\_ and \_"          1 2 [ ] cons . [ ' and ' ] swap . compose . cons . string !
    ```

    The lowering builds from the last hole back to the first, and since the
    first `_` takes the deepest, the last hole's value is on top, so each
    `cons` finds its value there with no shuffling. A quotation that holds
    a value known only at run time is a closure: its code is known, so the
    stage compiles it, and `.` still applies known code only (4). March8
    has `compose`, of quotations known while compiling, but not `cons`.
    A hole in a nested bracket belongs to the outermost bracket, so the
    lowering fills it there, and an inner quotation keeps no holes for
    later; to watch (Thomas, 2026-10-14). Since this is sugar, another
    surface may choose otherwise with no change below.
15. **Strings with holes.** `string !` makes text of a quotation, as
    `seq# !` makes a sequence: it runs the code where it stands and writes
    each value the code leaves, in order, into one string, `'…'` symbols as
    their text and other values as `>string` writes them. It is an
    emitter: one pass into one buffer, with no sequence made first and no
    chain of joins; pieces known while compiling fold. A sequence made a
    string is another thing, the sequence written, `( 1 2 )`. March8 joins
    the pieces with `concat` one at a time, which copies the growing
    string at each piece.

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
    variable is an entry. March8 does not have it: there `def` refuses a
    dotted name.

17. **Import and export** (Thomas): a namespace's map has two keys of
    March's own, `import#` and `export#`, named so as not to take words a
    program wants, `csv.export`; a trailing `#` marks March's own names,
    as in `int#` and `seq#`. Setting them is a definition of data:

    ```
    import#: <| math.* io.file.read |> ;     -- each local by its last part
    import#: <| circle math.pi |> map#! ;    -- or named, as pairs
    export#: <| pi e sin cos |> ;
    ```

    `import#` makes names local, each by its last part or by the name
    paired with it, and `math.*` makes local all that `math` exports. A
    bare name is looked up locally, then among the imports, then in the
    core. A path is absolute, so it means the same anywhere. An import
    that meets a local name or another import is an error, settled by
    naming it. `export#` holds the names that may be reached from outside,
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
18. **`;` defines**, a meta-op (Thomas). Through all of the above one
    word stayed constant, `def`, applied in every program, so it is named
    for what it is, FORTH's `;`: `sq: [ dup * ] ;` is `: sq dup * ;`
    turned postfix, the label's colon opening and `;` closing. It binds the
    name below to the value on top, with a signature between them if
    there is one, code and data alike, and adds a clause to the word of
    that name, as `def` does. So a second `;` on a constant's name adds a
    clause rather than replacing the value: replacing an entry is the
    global store's write, for the store's design. March8 uses `;` only
    inside a string, closing a named escape, `\times;`, and still reads
    `def`.

Open:

19. **Symbols applied at run time.** A symbol known only as the program
    runs names a word that is not chosen yet. Applying one would take the
    words themselves at run time, a dictionary as FORTH has, or a choice
    among the words a symbol's type allows.
