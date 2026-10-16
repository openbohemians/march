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
captures on the stack (section 25).

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
  as it will be stored. A map form, `{| … |}` perhaps, is to come. Built
  in march8 (2026-10-15).

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
inside a quotation as well; High Stack keeps it as the divider in a
signature's bracket (6.9).

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
| `positive?` | `positive ?` | `?` tests |
| `10`, `1.5`, `1:2` | `10 int# !`, `1.5 dec# !`, `1:2 ratio# !` | a number's spelling gives its kind |
| `< i64 -- i64 >` | `[ i64 . ] seq# ! [ i64 . ] seq# !` | a signature is two sequences |
| `<\| sq math.pi \|>` | `[ sq math.pi ]` | the lowest form inline |
| `( 1 2 )` | `[ 1 int# ! 2 int# ! ] seq# !` | a sequence literal |
| `5 ( _ 1 )` | `5 [ 1 int# ! ] cons . seq# !` | a hole, filled first |
| `"\_ apples"` | `[ ' apples' ] cons . string !` | a string with a hole |
| `dup_2`, `dup₂` | `dup 2 int# ! width# . .` | a width, by the word |
| `+^4`, `+⁴` | `+ 4 int# ! power# . .` | a power, by the word's kind |
| `[ dup + ]^3` | `[ dup . + . ] 3 int# ! power# .` | a quotation's power, a quotation |

A capital marks only that the reader adds no dot, so `Snake.` is what
`Snake .` is anywhere, the word of that name applied: march8 runs
`Foo: [ 1 2 + ] ; Foo.` to 3, and `Open.`, with no word `Open`, is an
error.

```
fib: < n > [ dup 1 -+ fib swap 2 -+ fib + ] ;
fib [ n var . ] seq# ! [ dup . 1 int# ! -+ . fib . swap . 2 int# ! -+ . fib . + . ] ;
```

March8 reads it so (2026-10-15), and, until signatures are sequences,
lowers it to `fib < n > [ dup . 1 -+ . fib . swap . 2 -+ . fib . + . ] ;`.

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
    import#: <| math.* io.file.read |> ;     -- each local by its last part
    import#: <| circle math.pi |> map#! ;    -- or named, as pairs
    export#: <| pi e sin cos |> ;
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
    Built in march8 (2026-10-15): imports and exports by lists, `<| … |>`,
    a source's imports run first, two finds of a name are an error, and
    reaching an unexported word from outside its namespace warns as
    severe. Not yet: the pairs that name an import, which wait for
    `map#`, and interfaces by hash, which wait for the store.
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
    `positive ?` as `Snake.` is `Snake .`. `.` still applies one, leaving
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
    `^n` and `_n` off, so a word named `dup_2` could never be reached, and
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
    import#: <| logs.* |> ;
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
    pattern where it takes a name, the symbol made a pattern by a word, as
    `int# !` makes a number, its name to be settled; so `;`'s first slot
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
    an export names it by its pattern, `export#: <| /log_([0-9]+)/ |>`.
    March8 learns a body's namespace only at `;`, so the stage, not the
    reader, reads a word by a pattern, when it reaches the word in its
    code's namespace, as it looks up a name. So `march8 lower` still
    writes the core's reading, and writes the pattern's name as a symbol,
    which `;` reads back. Lowering a program that uses pattern words waits
    until a namespace's text is its own. A captured name, `(.+)`, is a
    symbol applied in the body's namespace, not where the word was
    written, as a passed `sq'` is (16); that is to be settled when
    namespaces become values. The cost, measured for the first form,
    whose matching this keeps: about 25 ns a word where patterns are
    known, and none elsewhere. The core's own `^n` and `_n` stay in the
    reader.
26. **High Stack and Low Stack** (Thomas, to try). The surface March is
    written in, today the intermediate form of section 4, is High Stack;
    Symbolic Forth, the lowest form, is Low Stack.

Open: a map form of `<| … |>`, `{| … |}` perhaps (section 3); which
bracket owns a nested hole (14), to watch; for pattern words (25), what
a group that may not match leaves, refused for now, the word that makes
a pattern in Low Stack, and how an import names one alone.
