# The symbolic stack machine

Status: slices 1 to 3, 2026-10-08; the notation, 2026-10-12. March8 is
March's compiler written in Rust, as Thomas decided on 2026-10-08: build the
compiler in Rust now, and write it in March, bootstrapping like a real
FORTH, once March is mature. System March (march7) is frozen. This is the
symbolic stack machine (doc/design/TYPES.md 2.2). The staged-types
prototype (march7/docs/STAGED.md) is its specification, and its cases are
the tests. Slice 1 is the machine, types and families
chosen by types; slice 2 adds guards, the choice between clauses at run
time, `map`, and arithmetic lifted over arrays; slice 3 adds recursion,
instances and tail calls.

```
fee: < money >. 1.10 + < money >. ;
19.99 fee                          → 21.09, exactly: 2109 cents
sq: dup * ;
3 sq                               → 9, compiled as the literal 9
{ "k" ( 41 2 ) } "k" at 0 at 1 +        → 42
positive: 0 gt? ;
sign: < i64 positive? >. drop 1 ;
sign: < i64 >. drop 0 ;
x sign                             → 1 or 0, chosen at run time
5 sign                             → the literal 1, chosen now
( 1 2 3 ) 10 *                     → ( 10 20 30 ): it lifts
zero: 0 eq? ;
fact: < i64 zero? >. drop 1 ;
fact: < i64 >. dup 1 -+ fact * ;
10 fact                            → 3628800, by fact's i64 instance
```

## The notation

Words are separated by white space, as in FORTH. `--` starts a comment that
runs to the end of the line, except inside `< >`, where it divides inputs
from outputs. A line of three or more `-` and nothing else opens a block
comment, and a line of them ending in a period, `---.`, closes it
(Thomas, 2026-10-15). The notation was decided
on 2026-10-12 (Thomas), after a comparison of the same programs written
with a dot after every name applied and without (doc/design/DOTS.md): a
name applies, as in FORTH, in code and in a bracket alike, and a symbol or
a variable is spelled as one.

It is an **intermediate form** (Thomas, 2026-10-13). Below it is Symbolic
Forth, where everything is a symbol, `.` or `!` (TYPES.md 2.2,
doc/design/SYMBOLIC-FORTH.md):
every name a symbol until `.` applies it, a type given by applying its
word, `i64 .`, and converting `!`, a meta-op, which takes a type or a
type's symbol: `2 i64 !`. A number is a symbol there too, given its kind,
`10 int# !`, `1.5 dec# !`, `1:2 ratio# !`, which march8 folds into the
literal as it reads (2026-10-19). March8 reads the
intermediate form and lowers it by spelling alone: `fib` is `fib .`,
`Snake'` and `+'` are symbols, and `2 i64!` is `2 i64 . !`. `march8 lower`
writes any March in Symbolic Forth, Low Stack, and `--symbolic` reads it,
as march8 reads a file ending in `.s4`, where High Stack's end in
`.march` (2026-10-21); the litmus
test, `tests/symbolic.rs`, lowers the core vocabulary and the examples,
reads them back to the same tokens, and runs them lowered, the core too:
on the core read as Low Stack each example compiles to the same code and
does what it did (2026-10-22).

| Form | What it is |
|---|---|
| `1`, `-2` | an integer literal |
| `1.10` | a decimal literal: exact digits until its type is known |
| `1:2`, `12:30:15` | a ratio literal: whole numbers between colons, a sign only in front, kept as written (Thomas, 2026-10-13). Of two terms it is a number, `1:2` one half; of more, it is for what takes it to read |
| `"text"` | a string, with march7's escapes (`\n;`, `\#x2603;`, `\times;`) and holes (`\[ code ]`, `\_`) |
| `'text'` | a quoted symbol: its text as it is, spaces and newlines among it, a quote in it doubled, `'it''s'` (doc/design/SYMBOLIC-FORTH.md 6.12); a string wherever one is taken (6.28) |
| `name` | a name, applied: a primitive, a family, a constructor, or a type's name, which gives the type |
| `Name` | a capitalized name, a name like any: it applies (doc/design/SYMBOLIC-FORTH.md 6.27) |
| `a` | one lowercase letter: a variable, read as `a var# .` (doc/design/SYMBOLIC-FORTH.md 7), in code an input by its place from the top, `a` the top, and in a signature a type variable (14); it names no word, and its symbol, `a'`, is a value to match |
| `name:` | a label: the symbol `name`, written where a name is defined or a key given |
| `name'` | the symbol `name`, for a word passed rather than applied, `+' reduce`, or any symbol, `Snake'` |
| `\name` | a symbol table entry, in any word: `\times` is `×`, `x\_1` is `x₁` (core/symbols.txt; march7/docs/SURFACE.md) |
| `.` | applies the value on top, a word or a quotation; each dot at the end of a word applies once more: `[ 2 + ].` |
| `!` | converts the value below by the type on top, a meta-op, as `;` is, read as the primitive `!` applied: `2 i64!`, `x y < i64 f64 > !` (Thomas, 2026-10-13; no word `convert` since 2026-10-19) |
| `a.b` | one word: a dot inside a word is part of it, a number's point, `2.5`, or in a name a path in a namespace, `io.file.read` (TYPES.md 3.12), kept for namespaces and defining no word yet. Dots between names once chained them, `a.b.` for `a. b.` (2026-10-09), which left no spelling for a path (Thomas, 2026-10-10) |
| `[ … ]` | a quotation: its words, unevaluated. Every bracket is one, made data where it closes, as Low Stack writes it (doc/design/SYMBOLIC-FORTH.md 7): `( … )` is `[ … ] seq# !`, `{ … }` is `[ … ] map# !` and `< … >` is `[ … ] sig# !` |
| `( … )`, `{ … }` | an array and a map literal; inside an array literal, whatever the code leaves is collected |
| `_` | an input of the bracket it is written in, the first written the deepest, read as the letter of its place: `[ _ _ / ]` is `[ b a / ]` (Comprehensions) |
| `[ x y \| … ]` | a bracket naming its inputs, before a lone `\|`, the deepest first, each read as the letter of its place: `[ rate term \| rate 1 + term ^ ]` is `[ b 1 + a ^ ]` (doc/design/SYMBOLIC-FORTH.md 6.31; Comprehensions) |
| `< … >` | a bracket: code whose values are a type or a signature, a comprehension of types |

**Literals, symbols and words** (Thomas, 2026-10-12). Literals spell
themselves. A symbol is a pure literal, a name with no structure, written
with a prime, `Closed'`, or left by a word a module defines for it,
`Human: Human' ;`. Everything else
is structure, and structure is built by words, types included: `i64` is a
word giving the type, as `64 int` might build it one day, and `ary`, `vec`,
`map` and `or` build types from types, `string i64 map`. So types and words
are one namespace, not two spelled alike, and a name reads the same
everywhere. (For a day a bracket read names as types and code read them as
words, the bracket deciding which a name was; and capitals cannot serve
types and symbols both, since in a bracket one would be taken for the
other.) Until 2026-10-17 a capital marked a symbol, the rarer thing, since
code applies words far more often than it passes symbols. Now a capital
marks nothing (Thomas): words are case-sensitive, so a module defines the
symbols it needs as words that leave them, and they resolve as words do,
by namespace and import. A program made of symbols, a logic program's
atoms, reads its capitals as symbols by a rule whose capture is the name
and whose code is none, `/([A-Z][A-Za-z0-9]*)/: ;`, as
examples/syllogism.march does.

A symbol is text until a type structures it, so a word that takes a string
takes a symbol, known while compiling or only at run time: `hello' length`
is 5, a symbol and a string compare as strings, and a hole writes a symbol
as its text (doc/design/SYMBOLIC-FORTH.md 6.28). The exact clause comes
first, `< sym >` before `< string >`. It goes one way: a string becomes a
symbol only by `sym!`.

`map` and `or` are families (Thomas, 2026-10-16): each builds a type
from types, its constructor the clause whose signature is
`< type type >. < type >.`, chosen as any clause is, by what it takes, and
otherwise maps over an array or joins truths. No value is both a type and
an array or a truth, so `i64 true or` is no word. `nil` stands for its type
where a type is wanted, in any family: it matches `type`, so `i64 nil or`
is a union. A symbol is no type until applied: in Symbolic Forth
`string . i64 . or .`, not `string i64 or .`.

**`.` calls** what is on top: a **symbol**, the word its name is defined
as, or a **quotation**, evaluated: `[ 2 + ].`. Nothing else is called.

**A type is a value**, given by its name or built: `string i64 map
empty`. No bracket is needed for one in code. A type is data, and is not
called (Thomas, 2026-10-13); **`!` converts** by it:

- a **type** converts the value below to it: `2 i64!`, `x 100 i64 vec !`;
  parsing is converting, `"42" i64!` being 42, or nil for a string that
  writes no number;
- a **class** or a type with variables checks it: `x num!`;
- a **signature**, a bracket of more than one type, gives the values below
  those types, the deepest first: `x y < i64 f64 > !`.

**A bracket is code** (TYPES.md 2.2), read as any code is; what its code
leaves is the signature, by what each value is:

```
case: < string Camel >. … ;            a string, and the symbol Camel
at: < a ary i64 >. … < a >. ;          one letter is a type variable
ff: < a' >. … ;                        its symbol is a value: the symbol a
sign: < i64 positive? >. … ;           a word tested, by ?: a guard
even: < i64 [ 2 mod zero ] ? >. … ;    a quotation tested is one too
fib: < 0 >. … ;                        a value left: equal to it, as in
                                       `< 0 i64! >.` and `< true >.`
pair: < ( i64 string ) >. … ;          an array of types: a tuple type
```

`?` applies a test, which leaves one bool, and the compiler holds it to
that (doc/design/SYMBOLIC-FORTH.md 20): `positive?` is `positive ?`, and
`.` applies the word unchecked. In a bracket a test is a guard on the
inputs before it. A name left in a bracket,
unapplied, is a symbol, so no definition changes what a bracket means: a
symbol is never a type. A letter is a variable, which the reader writes
as Low Stack does, `a var# .`, and `var#` gives in a bracket the type
variable its letter names (2026-10-21); in a body a letter is to be an
input by its place (SYMBOLIC-FORTH.md 14), which is not built, and `var#`
says so. Reading a signature takes knowing
what a constructor takes, `< k i64 map k >` being a map and a key, as
reading any FORTH does. The code runs while compiling, so it leaves only
known values. Before a definition's body, a bracket is the clause's
context. Anywhere else it is a value, so `[ < f64 > ! ] map` converts each
element, as `[ f64! ] map` does.

**Namespaces** (doc/design/SYMBOLIC-FORTH.md 16, 17): a dot inside a name
divides a path, and `;` defines at one, `math.sq: dup * ;`. Each
source is **expanded** once it is read (2026-10-20): each name it applies
becomes its path where it was written, the first word found of it, in its
namespace, then among the namespace's imports in their order, the core's
last, and a name found nowhere is the namespace's own, so a quotation of
`math`'s that `map` runs still finds `math.sq`. The source's definitions are gathered first, and its
`import#` and `export#` set, so a namespace's own later word counts; a
definition's signature and body are expanded in its name's namespace, and
an entry in braces in its record's, `math.pt: { ab: < cc > ; } ;` finding
`math.cc`.
Lookup is then of a path, or of a bare name in the root. A symbol is
global text and is never expanded: `sq' map` in `math` applies the root's
`sq`, and `math.sq' map` or `[ sq ] map` applies `math.sq`; `Open'` equals
`Open'` wherever written. The meta-ops are no names, and defining one is
an error. A word of the program's root is reached from a namespace by
importing it. **The core is the namespace `core`**, which every namespace
imports, last: `+` is `core.+`. **A name's clauses are one family**
(2026-10-24): a word gathers the clauses of the families its namespace
finds of its name after it, its imports' in their order and the core's,
and the one that fits best is chosen, the one found first where two
namespaces' fit as well. So the program's `+: < cash cash >. … ;` takes
two cash, `1 2 +` and `2.0 sqrt 3.0 +` are the core's, and a program
writes nothing into the core; it overrides a core clause at the same
signature. Where none of a family's clauses fits, a primitive or a type
found after it is applied, so `dup: < string >. … ;` leaves `1 dup` the
core's. `dup: 7 ;` takes fewer values, so it is always the program's,
leaving the core's words, `over` among them, the core's `dup`. A path to
a word gathers from its namespace, `math.+`, where `math` defines one, the
core's `+` too, and where a
namespace finds a name in two of its imports and none of its own, the name
is expanded to its own path, `ns.ff`, which names no word, so the family
is gathered there. While core.march is read, a bare name
defined is the core's. Expanded code
keeps its paths; a message, a name taken out of a quotation, and
`name#`'s symbol show the shortest name the root finds the word by, `dup`
for `core.dup`. The core exports all of its words for now; which it keeps
as internals, reached by importing them by path, is to come. `import#: [| math.*
io.file.read |] ;` makes names local, each by its last part, or all that a
namespace exports; `math.export#: [| pi |] ;` says what may be reached from
outside, and reaching anything else warns as severe, though it compiles,
and a namespace's children see all of it. A namespace has one import,
which holds for all of it: a source's imports run first. A name found in
two places is the first's, the namespace's own before an import's, and a
path says which. Where none of the program's guards holds, the core's
clause is chosen, as in any family: `abs: < i64 [ 100 gt ] ? >. drop 0 ;`
leaves `-5 abs` 5.

**Recognizers** (doc/design/SYMBOLIC-FORTH.md 25) are rewrite rules: a
pattern, a regular expression between slashes that matches a whole name,
and code, `units./([0-9]+)kg/: int# ! 1000 * ;`. Where a name the
pattern matches is applied, the rule puts in its place its captures, each
a symbol, in order, then its code: `12kg` is `'12' int# ! 1000 *`. A rule
has no signature and one definition; its effect is the rewrite's, so to
its user `log_2` of `/log_([0-9]+)/: i64 ! log-base ;` is
`< f64 >. … < f64 >.`, and more meanings are a family's, which its code
applies. A capture is a symbol, global text, which the rule's code reads
as a name where the name was applied with `name#`; the rule's own names
mean what they mean where the rule was defined. A name a rule matches is
expanded to a path where it was written, `math.dbl^2`, so the path says
where: its namespace is the longest part before a dot that is one. When
`.` applies a name, the stage looks for a rule of that namespace, or of
one it imports by `*` that exports the rule by its pattern, then for the
name, then for a rule of the core: the power,
`/(.+)\^([0-9]+)/: [ name# ] dip int# ! power# . ;`, and the width,
`/(.+)_([0-9]+)/: [ name# ] dip int# ! width# ;`, which `width#` gives a word that
moves values or is handed code, `tuck_2` of a program's `tuck` too. A rule holds for
all of its namespace, and a name it matches before its code is defined is
an error. Each group must capture in every match, so the effect is fixed.
A name that two rules match, or that a rule matches and is also defined
or imported, is an error. A rule is matched against the symbol `.`
applies, so Symbolic Forth matches as March does, and the syllogism runs
lowered. A symbol becomes a number where it is known, so a family's
clause typed `sym` is checked where it is applied.

**A definition is data:** a name, its body, and `;` (Thomas,
2026-10-12; doc/design/SYMBOLIC-FORTH.md 18), FORTH's `: inc 1 + ;` with
its name a label, `inc: 1 + ;`. The body runs from the label to its `;`,
as FORTH's runs from `:` (6.32), and the reader writes it as a quotation,
`inc [ 1 + ] ;`, so a bracket in it is a value:

```
fib: < n >. dup 1 -+ fib swap 2 -+ fib + ;     code
primes: ( 2 3 5 7 ) ;                          a constant, the tuple
num: < i64 f64 or money or int# or dec# or ratio# or > ;    a type
```

A type bracket applied checks the values on top where it is applied, and
the check a body begins with is the clause's signature, its context
(TYPES.md 2.15), the types of its inputs; the one it ends with, after
that, the outputs it promises, `sq: < i64 >. dup * < i64 >. ;`
(doc/design/SYMBOLIC-FORTH.md 6.32). Until 2026-10-24 the two came before
the body, touching, `< i64 >< i64 > [ dup * ]`, which march8 now refuses,
saying where they go. `--` begins a comment everywhere, inside a bracket
too; one that leaves a bracket never closed is taken for the divider it
was until 2026-10-20, and the error says where the outputs go. Clauses of
one name are a family. No name ends in `?`, which tests, or in a power,
`^` and digits, which the core's rule rewrites where the name is applied;
a subscript, `_` and digits, is part of a name, an index, as in x₁. A body
that is one type, a type's name or a bracket, names it, `config: < string
i64 map > ;`, and a named class is called by its name in messages. A
definition names no inputs, a bracket in it does, `[ rate term | … ] .`;
and a label is closed by its `;`, so input that ends inside a body is
unfinished, and the REPL waits for the rest.

## Judgments

The machine runs code on a stack of **judgments** (TYPES.md
2.3). A judgment is a type, and the value when it is known at compile time:

| Known value | Of |
|---|---|
| an integer | an integer literal, an i64, or money in cents |
| digits and places | a decimal literal |
| terms | a ratio literal: as written, or in lowest terms once computed |
| a float, a string | an f64, a string |
| a type | a type, as a value |
| a name, a quotation | a word not yet applied, a quotation |
| nothing | a value at run time, on the machine's stack |
| pinned | a value at run time known to equal a value: a value pattern's input, in its clause |
| a known array | an array literal whose elements are all known, held as its elements (2026-10-21) |

**A known value has no code until a value at run time needs it.** When an
operation that runs takes it, or the word ends, the value is
**materialized**: its literal is emitted, under the values at run time
above it if there are any (by `swap`, `rot rot`, or the scratch stack). So:

- **constants fold through everything,** words, families and stack words
  alike: `3 sq` is the literal 9, and `1 2 swap` is two literals in the
  other order, with no swap;
- **stack words move judgments,** and emit code only for the values at run
  time among them: `dup` of a known value copies the judgment, and `swap`
  of a known value and a value at run time emits nothing;
- **the code holds only what must happen at run time;**
- **an array literal of known elements is known too:** `( 1 2 3 ) 2 at` is
  the literal 2, and `( ( 1 2 ) ( 3 4 ) ) 2 at 1 at` the literal 3. Its
  elements are gathered only where a value at run time is needed, each
  copy on its own, so `( 1 2 ) dup` builds two. A clause that wants it as
  another type takes its elements so, each as it would alone: `( "a" b'
  )`, a string and a symbol, is two strings where a `string ary` is
  wanted (TYPES.md 2.10). It is what lets March read data while
  compiling, as `fail` reads its message (below, "Errors").

The prototype emitted a placeholder for each literal where it appeared and
patched it once its type was known, keeping code order equal to judgment
order. Materializing late folds more, at the price of moving values at run
time now and then.

## Types as data

`src/types.rs`. A type is a term, a constructor and its arguments, in a
table where equal terms are one entry, so a type is a small number and
equal types are equal numbers.

| Type | Meaning |
|---|---|
| `i64`, `f64` | numbers |
| `money` | exact cents in an i64; a placeholder until March has precise numbers, then a type of its own over one (TYPES.md 2.11) |
| `string` | UTF-8 text |
| `int#`, `dec#`, `ratio#` | an integer, a decimal and a ratio literal, undecided (TYPES.md 2.10); the names are placeholders (TYPES.md 3.4) |
| `sym` | a symbol, a name not yet applied: at run time its name, an interned string, so symbols compare and key maps by identity |
| `type`, `quote` | a type, a quotation: values at compile time only |
| `a ary` | an array of any length |
| `3 a vec` | an array of exactly 3: the length is known at compile time |
| `k v map` | a map |
| `a` to `z` | type variables, in patterns |
| `atom` | in patterns, any type that is not a container: a number, a string, a literal. A class, not a type: it binds nothing, so `< atom atom >` takes two of different types |
| `a b or` | in patterns, a union: any of its types, `i64 f64 or`. Kept canonical, so order and repeats do not matter. It matches a little less well than one of its types alone, and better than `atom` |
| `num` | the numbers: `num: < i64 f64 or money or int# or dec# or ratio# or > ;`, in `core.march` |
| `nil` | the type with one value, `nil`: nothing is there, as in `i64 nil or` |
| `bool` | truth: 0 and 1, `false` and `true`, Iverson's brackets, so an i64 too (TYPES.md 2.16) |
| `( i64 string )` | a tuple: fixed positions, a type each. Positions of one type are a vec, so `( a b )` takes `( 1 2 )` |
| `tuple` | in patterns, any tuple: a class, as `atom` is |
| `{ x: f64 y: string }` | a record: fixed keys, a type each, a map of types (TYPES.md 2.11). Known by its fields, in the order of their keys |
| `record` | in patterns, any record: a class, as `tuple` is |
| `celsius` | a type of its own, `celsius: f64 own ;`: its representation's values, and a type no other is (TYPES.md 2.11) |

**An array literal is a vec:** `( 7 8 9 )` is a `3 i64 vec`, so its length
is a constant. A symbol beside a string makes a tuple, `( "a" b' )` being a
`( string sym )`: a symbol is text only where a string is taken
(doc/design/SYMBOLIC-FORTH.md 28), and a literal takes nothing. A vec is accepted where an array of the same elements is
wanted, forgetting its length, at any depth: a `string 2 i64 vec map` is a
`string i64 ary map`. It costs nothing, since both are the same at run time.

## Families

A family applied chooses its clause by the values on top (TYPES.md 2.8):

- **Matching.** Each clause's signature is matched against as many values
  as it has inputs, scoring:
  - 8 for each part of a pattern that is not a variable, 4 for an `ary`
    that a vec stands for, 2 for `atom`, and for a union 1 less than its
    best member;
  - for a literal: 8 for its own type, 4 for its default type, 2 for any
    other type it becomes;
  - nothing for a variable, which binds; the same letter must be the same
    type, and a variable bound to a literal takes a type the literal
    becomes.

  A clause with no signature matches anything, with no score.
- **The shortest clause wins, then the highest score; a tie is an error.**
  No clause matching is no word, the same error as a name with no
  definition (TYPES.md 2.15). Clauses of different lengths are below.
- **The chosen clause is evaluated where it is applied, on the caller's
  judgments** (TYPES.md 2.6). So `sq` is integer multiplication in one
  place, float multiplication in another, and a constant on a literal.
- **A signature settles the inputs:** each takes the type its pattern names,
  so literals convert; the body cannot reach below its inputs; and the
  outputs it promises are checked, a literal result taking the type
  promised.
- **A clause whose signature types all its inputs** is also compiled where
  it is defined, on values at run time of those types, so its errors show
  there. If it fails, it is not defined; unless it fails only for needing
  a value known while compiling, as `vec`'s length, or a word its source
  defines later, when it is checked where it is applied instead.
- **A clause with the same context as one before replaces it,** as a FORTH
  redefinition does; otherwise the two would always tie.

### Clauses of different lengths

A family's clauses need not take as many values as each other (Thomas,
TYPES.md 3.13):

```
step: < Up' >. drop 1 ;
step: < i64 Down' >. drop drop 2 ;

5 Up' step                         5 1: the shorter takes one value
5 Down' step                       2
```

(The first such family was `read`, `Stdin read` beside `"data.csv" File
read`; reading is by namespace now, `io.stdin.read` and `io.file.read`.)

- **When clauses of different lengths match, the shortest wins,** the most
  specific among those. A word reaches as little of the stack as it can,
  so a longer clause never changes what a call to a shorter one did. A
  clause loses a large score for each value it takes, before how well it
  matches, and one longer than the shortest that matches without guards,
  which always holds, is dropped: `< i64 >` beside `< i64 i64 >` is chosen
  for `2 3`, leaving 2.
- **Guarded clauses are tried shortest first,** each length in the order
  defined, so a shorter clause whose guard fails gives way to a longer:
  `5 Down' step` tests `Up'` against Down', known now, and drops it.
- **An application takes as many values as the longest clause that may be
  chosen,** which is what an instance of a recursive family takes; a
  clause's body still cannot reach below its own signature.
- **A choice at run time between clauses of different lengths must leave
  the stack as deep,** whichever is chosen; the values beneath the shorter
  clause's are left as they were, and merge with what the longer leaves
  there, as a union if they differ. Otherwise it is an error: ``clauses
  of `step` taking 1 and 2 values are chosen between at run time``.
- **Their effect,** for a guard's arity, is one only if all agree in what
  they add to the stack: the longest's inputs, and as many more or fewer
  outputs.
- **A clause shorter than another that both may match is warned of,**
  informatively: added, it may change what a call did. So is a longer
  clause added under a shorter one with no guards, which is chosen
  wherever both match.

### Guards: the choice at run time

A word tested in a signature is a **guard**, `< i64 positive? >`, and so
is a quotation tested there, `< i64 [ 2 mod zero ] ? >` (TYPES.md 2.2,
2.15; doc/design/SYMBOLIC-FORTH.md 20). A quotation left bare is refused,
saying to test it, since each role has its mark (SYMBOLIC-FORTH.md 2). It looks at the
inputs before it, as many as it takes, without taking them, and leaves a
`bool` (TYPES.md 2.16). How many it takes comes from its definition, by a
pass over its words: `positive?`, `[ 0 gt? ]`, takes one, and `lt?` takes
two, so `< i64 i64 lt? >` compares the two inputs. A test that leaves
anything but one bool is an error where it is applied.

When some clauses that match have guards, the choice has two phases:

1. **By types, now.** The literals among the inputs take the types of the
   best match, and the clauses are matched again.
2. **By guards, at run time.** For each guarded clause, in the order they
   were defined: a test of its guards on copies of its inputs, then its
   body. Last, the best clause with no guards, or, if there is none, the
   trap for no word, so that no clause whose guard holds is no word at run
   time, as at compile time.

- **A guard on known values is decided now.** One that fails drops its
  clause; one that holds makes its clause the choice, and the rest are never
  compiled. So `5 sign` is the literal 1.
- **Every alternative starts from the same judgments** and is compiled into
  code of its own. Its results then take the types all the alternatives
  share, literals taking the others' type as in an array literal, or else
  their union ("Unions", below; TYPES.md 2.9). Then the code is laid out: each test
  branches past its clause when it fails, and each clause jumps to the end.

```
x sign        dup 0 swap lt? 0branch L
               drop 1 branch END
            L: drop 0
          END:
```

This is if-free code (TYPES.md 2.12): a sign, a minimum, a partial
function, written as clauses.

### Value patterns

A value left in a signature, not taken as a `vec`'s length, is a **value
pattern**: the input must equal it (TYPES.md 2.12).

```
fib: < 0 >. ;
fib: < 1 >. ;
fib: < n >. dup 1 -+ fib swap 2 -+ fib + ;
```

- **It is a guard,** testing the input `eq?` the value, so it is chosen in
  the order defined, decided now on a known value, and tested at run time
  otherwise. `fib`'s instance tests `dup 0 eq` and `dup 1 eq` before its
  step, and `0 fib` is the literal 0.
- **An untyped value matches an input of any type it becomes,** compared in
  that type: `< 0 >` takes an i64, an f64 or money, and `< 0.5 >` an f64.
  The input keeps its own type. `< 0 i64! >` takes an i64 only, as `0 i64!`
  is an i64 in code.
- **Strings and symbols too:** `< "quit" >`, `< Closed' Open' >`. A symbol
  at run time is its name, an interned string, so the test is one `eq`:
  a state machine's table is chosen from while compiling on known states,
  and tested as it runs on states read from data, `"Open" sym!`.
- **Inside its clause the input is known to equal the value:** it is
  *pinned*, a value known at compile time whose cell is still on the
  machine's stack. Passed through, it costs nothing, so `fib`'s `< 0 > ;` is
  no code; copied, the copy is the known value; folded, its cell is dropped
  and the result is a constant, so `g: < 0 >. 1 + ;` compiles that
  clause to `drop 1`, and `< "quit" > "bye: " swap concat` to a drop and a
  constant string.
- Value patterns are inputs: one in a check in a body is tested there, as
  a guard is, at run time where it must be (2026-10-26); one among the
  outputs is an error.

### Effects

**A guard must not write** (Thomas, 2026-10-08). Otherwise its write would
happen or not as the choice is made, and on known values it would vanish
with a clause dropped at compile time. It may read, which makes it a choice
at run time.

So the stage works out effects as it works out types. Each primitive has a
set of effects, for each domain whether it reads and whether it writes, as
march5's effect rows; so far only `write`, which writes `io`, has any. The
stage adds up the effects of the primitives it emits, and an instance
records its effects beside its results' types, so the effects of a word
are found through everything it applies. A guard whose effects include a
write is refused, where its clause is defined if the clause's signature
types all its inputs, else where it is applied:

```
1:54: the guard `noisy?` writes io, and a guard must not write
  in `f`, defined at 1:48
```

A primitive with an effect is never folded: a read is a fact of the run, and
a write must happen then. march5 also had effect tokens at run time, to put
effects in order in its interaction nets; code here runs in order, so
nothing needs them yet.

### `map` and lifting

`xs q map` applies the quotation to each element. The quotation is
compiled once, on a value of the element type at run time, as the body of a
loop; the array, the new array and the index are on the scratch stack, read
with `scratch-at`. The quotation may read the values below its element,
`100 ( 1 2 3 ) [ over + ] map`, but must leave them as they were and leave
one value. A vec maps to a vec as long. `map` given two types builds a map
type instead.

Lifting is clauses in `core.march`, an array and a number in either
order for `+`, `-+`, `*` and `/*` ("Arrays as tensors"):

```
+: < a ary a >. swap [ over + ] map swap drop < a ary >. ;
```

So `( 1 2 3 ) 1 +` is `( 2 3 4 )`, a literal taking the elements' type, and
the prototype's original bug, `{ "k" ( 1 2 ) } "k" at 1 +`, lifts.

### Literals are clauses too

A clause defined under a type's name says how a literal becomes that type:

```
i64: < int# >. int#>i64 < i64 >. ;
money: < dec# >. dec#>money < money >. ;
```

Converting a literal by the type, `2 i64!`, runs this clause, and so does
a clause that wants the type and is given the literal. Applying the name
gives the type itself; its clauses are only its conversions. So literal handling is
an open family, as Thomas framed it ("just another type signature"). Two
literals fold by clauses on the literal types, `< int# int# >. … < int# >.`,
exactly, and stay literals, so `1 2 + 3.5 +` is the decimal 6.5. A
literal takes its default type, i64 or f64, only when it must be a value at
run time. Until then it prefers to become another literal kind, which keeps
it exact, over its default: `1 3 /*` is the ratio 1:3, so `1 3 /* 3 *` is
exactly 1.

A ratio, `1:2`, is one half: the f64 nearest it when it must be a value,
rounded once from the exact quotient, money when it is a whole number of
cents, never an i64 (`1:2 i64!` says to round). Its terms are kept as
written, so `12:30` is not `2:5`, and a ratio of more terms, `2026:10:13`,
reads: it is no number, but a word that takes it can read its terms, as a
time or a date might. Computed, a ratio is in lowest terms. Literal
division is exact, so `1 0 /*` is an error while compiling, and NaN takes
a float, `0.0 f64! 0.0 /*`.

### Loops: `each`, folds and reductions

`xs q each` applies `q` to each element in turn, from the first;
`each-right` from the last. The quotation takes its element and may change
the values below it, an accumulator, but must leave as many as there were,
of the same types, since the next turn starts from them: the loop's
**invariant**.

```
0 ( 1 2 3 ) [ + ] each          → 6
0 ( 1.5 2.5 ) [ + ] each        → 4.0: the literal 0 becomes an f64
```

The stage compiles the body once to see what it leaves, and throws that code
away. A value it changes becomes, before the loop, a value at run time of the
type it keeps, a literal taking the body's type as the ghost of a recursion
does; then the body is compiled again, until nothing changes. So the 0 above
is the i64 0, under the array, and the body is one `i64+`. The array and an
index are on the scratch stack.

The rest are words in `core.march`:

| Word | Means |
|---|---|
| `xs x q fold` | from the left: ((x e0) e1) e2 |
| `xs x q fold-right` | from the right: e0 (e1 (e2 x)), its quotation taking an element under the accumulator |
| `xs q reduce` | `fold` from the first element; `reduce-right`, `fold-right` from the last |
| `xs q scan` | the running reductions from the left: `( 1 2 3 ) +' scan` is `( 1 3 6 )`, a comprehension |
| `n q repeat` | `q` n times, as `each` over `n range` |
| `first`, `last`, `rest`, `most`, `slice` | parts of an array, and `slice` of a string, from element i to element j, both included |
| `xs v k insert`, `xs k remove` | v put in the gap after element k: `0 insert` prepends, `-1 insert` appends; element k taken out |

**Elements count from 1, gaps from 0** (Thomas, 2026-10-09). An array of n
has elements 1 to n, and -1 to -n from the end; there is no element 0. Its
n+1 gaps count from 0, before the first, each gap after the element of its
number, so gap -1 is after the last. So `( 5 6 7 ) 1 at` is 5 and `-1 at`
7; `2 -2 slice` drops the first and the last; `0 insert` prepends and
`-1 insert` appends, with no -0 needed; and `n range` is 1 to n. Counting
from 0, the gap before element k is k, so -1 cannot append (Python's
`insert(-1, x)`); counting elements from 1 makes the ends symmetric. A known
index is settled at compile time, and checked against a known length; one
known only at run time is settled by a few instructions.
| `xs q within` | `q` run with the array's last elements as its stack, as many as it takes, and what it leaves put in their place: `( 1 2 3 ) [ ~ ] within` is `( 1 3 2 )`; `[ 4 ] within` appends, `[ drop ] within` drops the last |
| `spread` | an array's elements on the stack: a vec's each a value; any array's, in a literal, a run |
| `sort` | an array in order: numbers by value, strings by text |
| `keys`, `values` | a map's keys and values, in one order, the map's own |

`within` is an adverb in J's sense (Thomas, 2026-10-09): it takes a word
that acts on the stack and makes it act on the end of an array, as Joy's
`infra` and Factor's `with-datastack` do. How many elements it takes comes
from the effect pass; only those are loaded, and the rest of the array is
kept by a slice and joined to what the word leaves, so it costs what the word
does, not the array's length. A vec's new length is known.

With `-+` on `( 1 2 3 )`, `fold` from 0 is −6 and `fold-right` is 2, as APL's
`-/`; for an associative word the two agree. A consumer takes a word as well
as a quotation, so `( 1 2 3 4 ) +' reduce` is APL's `+/`, the prime leaving
`+` unapplied. `q r compose` joins two quotations at
compile time; `n range` is the array 1 to n, a vec when n is known, and
`reverse` an array reversed.

### Comprehensions

Inside an array literal, whatever the code leaves is an element, as many as
it leaves: `(` marks the machine's stack and `)` gathers what is above the
mark (march7/docs/ARRAYS.md). So `each` and the literal together are map,
filter, flat-map and scan, with one loop:

```
( xs [ 10 * ] each )            map
( xs keep' each )                 filter: keep's clauses leave the element or nothing
( xs [ dup ] each )             each element twice
( 0 xs [ over + ] each )       scan: ( 0 1 3 6 ), reading the last collected
( 3 [ 7 ] repeat )               ( 7 7 7 )
```

- **A run** is a new kind of judgment: zero or more values of one type,
  counted only when the literal gathers them. A loop in a literal whose body
  leaves values above the loop's is collecting, and leaves a run; clauses
  chosen at run time that leave different numbers of values, the filter's
  one or none, leave a run. The none is Thomas's mirror, and here it costs
  nothing: the gather counts what is there. A literal with a run is an array
  of any length, `T ary`; without one, a vec, as before.
- **Nothing reaches beneath a run,** whose depth is not known: a run can only
  be gathered, so adding to a filter's result is an error. The elements
  before it become values at run time first, of the one element type.
- **A collecting body may read the values below its element,** which are
  elements already collected, of the element type, as a scan reads the last;
  it may not take them.
- **A letter is an input by its place** (doc/design/SYMBOLIC-FORTH.md
  14; Thomas, 2026-10-17; built 2026-10-22). In a bracket one lowercase
  letter is the input at its place from the top, `a` the top, `b` the
  next, and `x`, `y` and `z` the first three; written again it is a copy.
  `[ b a / ]` is `/`, `[ a b / ]` is `swap /`, and `[ b a b + ]` is
  `over +`. A bracket takes as many inputs as its deepest letter names,
  when it is applied, and drops those it names nowhere, so `[ c ]` takes
  three and keeps the deepest. A literal or a string is applied where it
  stands, and a quotation by `.` or by what takes it, so `[ b a + ]` is an
  anonymous word of two inputs, and a word's body takes the word's inputs.
  A letter in a nested bracket is that bracket's: `( xs [ a 10 + ] each )`
  adds 10 to each element. Nothing is captured, so there are no closures;
  a value from outside is written in or reached on the stack.
- **`_` is the letter of its place** (Thomas, 2026-10-16, 2026-10-19): a
  bracket with k `_`s takes k inputs, and each `_` puts the next where it
  stands, the first written the deepest, so the reader writes
  `[ _ _ + _ * ]` as `[ c b + a * ]` and `1 2 ( _ _ div )` is
  `( 1 2 div )`. A bracket uses `_`s or letters, never both.
- **A bracket may name its inputs** (doc/design/SYMBOLIC-FORTH.md 6.31;
  Thomas, 2026-10-23; built 2026-10-23), before a lone `|`, in a
  signature's order, the deepest first: `[ rate term | rate 1 + term ^ ]`.
  It takes all it names, those its code does not use dropped, and each
  name is the input, copied where written. The reader writes each as the
  letter of its place, and where the deepest is unused, writes its letter
  once and drops it, `c drop`, after the letters the code begins with. Only
  names come before the bar, and the bracket uses no other letter and no
  `_`. A quotation inside it does not see its names, so naming one there
  is refused, saying whose it is; the quotation is given the value on the
  stack. A string, a sequence or a map inside it, made where it stands,
  sees them: the reader passes each name in on the stack just before the
  bracket, once, the first used the deepest, and the bracket takes it by
  a letter, its own inputs lying below, so `[ n | "n is \[ n ]" ]` is
  `[ n | n "n is \_" ]`. A signature and a string name no inputs.
- **How the stage moves them.** The inputs are taken first, so the code may
  reach below them. The letters a quotation or a body begins with are a
  shuffle, done on the stack, each input copied, or moved where it is
  named last, those in order already never moving. The inputs named once
  the code has begun wait, those known only at run time on the scratch
  stack, as `dip`'s value does, the one used last at the bottom: each is
  taken from there where it is used last if it is on top, and copied
  before, so letters in the stack's order, as `_`s are, compile as the
  plain words do. A literal's and a string's letters all wait, since their
  elements are what their code leaves.

Outside a literal the rules stay strict: a loop keeps its shape, and clauses
chosen at run time leave as many values. A value that may be absent outside a
collection needs a union with nothing, `a or nil` (TYPES.md 3.6).

### Strings with holes

A hole, `\[ code ]`, writes the value of its code into a string, and `\_` an
input, short for `\[ _ ]` (march7/docs/STRINGS.md), as a letter in a hole
is, `5 "\[ a ] is \[ a ]"` being "5 is 5":

```
"1 + 2 = \[ 1 2 + ]"           "1 + 2 = 3", a constant
1 2 "\_ and \_"                 "1 and 2": the first input is the deepest
ff: "<\[ _ 1 + ]>" ;    5 ff is "<6>", 1.5 ff is "<2.5>"
"H\_2;O"                        "H₂O": a subscript's name and `;` follow
```

- **The reader** reads a hole's code with its own words, to a `]` where a
  word would start with the hole's own brackets closed, so text resumes right
  after it (`"\[ n ]apples"`), and strings nest. It writes a string as Low
  Stack does (2026-10-22; doc/design/SYMBOLIC-FORTH.md 7, step 4): a
  quotation of its text, each run a symbol, and its holes' code, `\_`
  being a letter, `a var# .`, made text by `str# !`, so `"1 + 2 = \[ 1 2 + ]"` is
  `[ '1 + 2 = ' 1 2 + ] str# !`.
- **The stage** runs the quotation's code where it stands and writes each
  value it leaves, by `>string`: a string's own text, a symbol's, or any
  other value as `show` writes it; and joins them in one buffer,
  `text-join`, not a chain of `concat`s.
  Pieces known while compiling join then, so on known values the string is
  one constant. A hole is code written in: it may leave any number of
  values, and reaches what `.`'s would, the text before it among them
  (14), so `5 "\[ 1 + ]"` is "6" and `"a\[ drop 1 ]"` is "1". What the
  code has left is written as soon as the code after reaches none of it,
  before the next text or letter, so a hole's value is written where it
  is made, not brought up to be written last; before code holding a
  quotation, whose reach `each` and its kin leave unknown, it waits.
- **`_` or a letter in a hole** is an input of the string, taken where
  the string stands, so a word whose body is such a string takes them.
- **`str# !` of a quotation** is text of the values its code leaves, in
  order, `1 2 [ b " and " a ] str# !` being `"1 and 2"`
  (doc/design/SYMBOLIC-FORTH.md 15), and of a symbol, text as it is,
  `'ab' str# !`, as `10 int# !` is a number. `string !` makes no text of a
  quotation, `string` being the type text has and not a kind (retired
  2026-10-22).
- **`seq# !`, `map# !` and `sig# !` of a quotation** make it an array
  literal, a map literal or a signature, which is how the reader writes
  `( … )`, `{ … }` and `< … >` (2026-10-21; doc/design/SYMBOLIC-FORTH.md
  7, step 3). A literal's code runs where it stands and reaches what
  `.`'s would, and its elements are what it leaves, from as deep as it
  reached (14): `1 2 3 ( + )` is `1 ( 5 )`, and `0 ( xs [ + ] each )` a
  sum. The stage notes the deepest value its code takes, and where that
  had a cell, `reach` lowers the mark by as many before the gather, so
  code that reaches nothing below compiles as it did. A signature's runs
  over types, so its
  letters are type variables and its tests guards, and in it `seq# !` is
  a tuple type. The three kinds are words, `seq#`, `map#` and `sig#`,
  as `int#` is, and no value has one.
- **`show`,** in `core.march`, writes a value as the display does, so it
  reads back: a string quoted, money with its cents, an array in
  parentheses, element by element, and a map in braces, sorted by key (Thomas,
  2026-10-08), as the display sorts it. `print` writes any value, by
  `>string`.

### Arrays as tensors

Scalars, vectors and matrices are tensors of rank 0, 1 and 2 (Thomas,
2026-10-09), and in March a tensor of rank n is arrays nested n deep. A vec
type gives its shape at compile time: `2 3 i64 vec vec` is a 2×3 matrix.

```
( ( 1 2 ) ( 3 4 ) ) 10 ⋅                 ( ( 10 20 ) ( 30 40 ) )
( ( 1 2 ) ( 3 4 ) ) ( 10 20 ) +          ( ( 11 22 ) ( 13 24 ) ): across each row
( 1 2 3 ) ( 4 5 6 ) dot                  32
( ( 1 2 ) ( 3 4 ) ) ( ( 5 6 ) ( 7 8 ) ) dot   ( ( 19 22 ) ( 43 50 ) ): the matrix product
( 1 2 ) ( 3 4 5 ) ×                      ( ( 3 4 5 ) ( 6 8 10 ) ): the outer product
( ( 1 2 3 ) ( 4 5 6 ) ) transpose        ( ( 1 4 ) ( 2 5 ) ( 3 6 ) ), a 3 2 i64 vec vec
```

- **Element by element.** `+`, `-+`, `*` and `/*` on two tensors of one shape
  pair their elements, by `xs ys q zip`, level by level. A tensor and one of
  lower rank, a number or a row, pair along the leading axis, recursively,
  so the lower broadcasts across the higher's trailing axes. A number goes
  into a tensor of any rank: `atom` in a pattern matches any type that is
  not a container ("Types as data"), so four clauses an operation cover every rank.
  A tensor and a row two or more ranks lower do not pair yet.
- **`×`, the outer or tensor product:** each element of A times all of B, so
  ranks add: two matrices make a tensor of rank 4. `xs ys q table` applies
  any word to every pair, as APL's `∘.` and Uiua's `⊞`.
- **`dot` contracts A's last index with B's first:** vector · vector is a
  number, matrix · vector a vector, matrix · matrix the matrix product. Its
  symbol is open: `⋅` multiplies element by element, as everything else
  works, and `⊙`, a dot in a circle, is the Hadamard product in mathematics.
- **`transpose`** swaps a matrix's two indices, written in March with `map`
  and `range`.
- **Shapes are checked at compile time** when they are known: lengths paired
  by `zip`, and so the contracted lengths of `dot`, a 2×3 by a 2×2 refused
  before anything runs; at run time otherwise. A clause keeps a vec's length
  when its signature asks for an array, so shapes reach the words that check
  them.

### Unions

A value whose type is decided only at run time has a union type, as
`i64 nil or` (TYPES.md 3.6). `nil` is a type with one value, `nil`, which
means nothing is there.

```
{ "a" 1 } "a" get               1, of i64 nil or
{ "a" 1 } "z" get               nil, of i64 nil or
port: < i64 >. 10 * ;
port: < nil >. drop 80 ;
m "z" get port                 80, by the clause the tag chooses
m "a" get 1 +                  no word `+` for nil: each type needs a clause
```

- **Made where clauses chosen at run time leave different types** (TYPES.md
  2.9). At the join each alternative's value is tagged with its type, and
  the result has their union. A choice made at compile time, a guard on
  known values, makes none: the value has its one type. Each union made is
  noted by an informative warning, unless the clauses' outputs name a union,
  as `get`'s do, `-- v nil or`.
- **Taken apart by clauses.** A family applied to a union value is applied
  to each of its types in code of its own, chosen by the tag, the last with
  no test, and the results merge, into a union again if they differ. It
  takes the union whole when one clause is best for every type: stack
  words, a type variable, `atom`, a union that holds it. A type with a
  better clause of its own is applied apart, so `>string` of a string is
  its text. A type with no clause is no word, found while compiling.
- **One cell.** A union value is a small array of its type's tag and its
  value (`union-make`, `union-tag`, `union-value`), so a value is still one
  cell on the stack. The tag is the type's own number, so a value of one
  union is a value of any union that holds its types, as it is. Arrays
  hold union values: `( 1 nil )` is a `2 i64 nil or vec`.

### Unions of values

`or` of symbols is the type of just those symbols, a subset of `sym`
(TYPES.md 3.6; built 2026-10-26), as a value pattern is the type of one
value. `or^7` joins seven, and `;` names it.

```
read: code' data' text' or^3 ;
kind: read own' top' or^3 ;
like: < kind >. code' eq ;
code' like                       true, decided now
frob' like                       no word `like` for frob'
tok: { kind: < kind > ; n: < i64 > ; } ;
```

- **An input of one is a guard,** a symbol tested to be one of them, as a
  value pattern's input is tested to equal its value: decided now on a
  known symbol, or on a value whose type is a union of values within it,
  and as the program runs otherwise, `eq` to each in turn.
- **Inside its clause the input is one of them,** its type the union, so it
  fills a field of that type, `{ kind: kd n: 1 } < tok >.`. A known symbol
  fills one too, and `put` sets one, `kind' text' put`; a symbol known only
  as the program runs, not through such an input, is refused while
  compiling.
- **Forgotten where a symbol is wanted,** matched a little less well, as a
  type of its own is its representation, and into a wider union of values.
  At run time it is a symbol.
- **Not yet:** a union of symbols and a type, `code' nil or`, refused; values
  of other types, numbers and strings, whose `or` means something else
  (`true false or` is truth); and a known symbol in a structure made as the
  program runs, `{ line: row kind: body' }`, which keeps only `sym`, so
  `lower` makes such a token by `tok-of`, whose input says its kind.

### Tuples

An array literal whose elements differ in type is a **tuple**, typed by
position (TYPES.md 2.5): `( 1 "a" )` is a `( i64 string )`. At run time it
is an array's cells; only its type differs.

```
( 1 "a" 2.5 ) 2 at               "a", a string
( 1 "a" ) spread                 1 "a", an i64 and a string
( 1 "a" ) [ show ] map          ( "1" "\"a\"" ), a 2 string vec
0 ( 1 "ab" ) [ show length + ] each     5
( 1 "a" ) ( 2 "b" ) concat       ( 1 "a" 2 "b" ), an i64 string or ary
```

- **Positions are precise and free.** `at` with an index known at compile
  time has its position's type, from either end; `length` is a constant;
  `spread`, `reverse` and `show` keep each position's type.
- **`map` and `each` are unrolled:** the quotation is compiled for each
  position on its type, so `map`'s results keep theirs, a tuple or a vec,
  and what `each` threads may change type from one position to the next.
- **A tuple forgets its positions where an array is wanted,** as a vec
  forgets its length: given to a clause that asks for `a ary`, as
  `concat`'s, or `at` with an index known only at run time, it becomes an
  array of the union of its types, each element tagged, with an
  informative warning. Only where the union fits: `sort` has no clause
  for `i64 string or ary`. A clause for `tuple` is preferred, so the
  forgetting is the last resort.
- **In brackets,** `( i64 string )` is a tuple type, and `tuple` any tuple.

### Records

A map literal whose keys are symbols is a **record**, or may be one
(TYPES.md 2.11): of values not alike, `{ ab: 1 cd: "x" }` is a `{ ab: i64
cd: string }`, shown as a map is, its type telling them apart; of values
alike it is undecided, a record where one is wanted, a map where a map
is, and a map with nothing to take it, `{ ab: 1 cd: 2 }` a `sym i64 map`.
A key written again replaces the one before. At run time a record is its
values in the order of its keys, as a tuple's cells are by position.

```
point: { x: < f64 > ; y: < f64 > ; } ;         a record type, named
norm: < point >. [ p | p.x dup * p.y dup * + sqrt ] . ;
{ x: 3 y: 4 } norm                              5.0, its integers floats
{ ab: 1 cd: "x" } cd' at                        "x", a string
```

- **A record type is a map of types:** `;` in braces makes an entry of
  the map around it, a field, `{ x: < f64 > ; }`; in a bracket a field's
  type is written bare, `< { x: f64 y: f64 } >`. Known by its fields, so
  two of the same fields are one type, and a name given one is an alias.
- **A clause that wants a record makes one** of an undecided literal,
  each value converted as it would be alone, at run time too, its cells
  already the record's; one of the wrong fields, missing, extra or of
  another type, is no word. Where a map is wanted the stage makes the
  map, `keys` taking it, from the record's cells.
- **A record forgets into a map where one is wanted** (2026-10-26), of
  its keys, symbols, and its values' type, or their union, each value
  tagged, as a tuple forgets into an array, with an informative warning:
  `{ x: 1 y: "a" } values` is `( "a" 1 )`, an `i64 string or ary`. So `at`
  with a key known only as the program runs reads the map, its value of
  that union.
- **A field is read by `at`** with its key known while compiling, `p x'
  at`, of its own type, as a tuple's position is; a path from a
  bracket's name is short for it, `p.x`, and `p.ab.cd` reads one inside
  another; so is one from a word that takes nothing, `cfg.port` for `cfg
  port' at`, its head found as any name is, where no word has the path
  (2026-10-26). Words do not chain, so `sort.length` is no path. A field
  the record lacks is an error while compiling.
- **A field is set by `put`,** its key known while compiling, `r n' 5
  put`: a new record of the same type, its cells the old one's but that
  one, the value taking the field's type, so a fold hands a record along,
  `( 4 5 6 ) { n: 0 last: 0 } [ … n' swap put … ] fold`. An undecided
  literal becomes a record, or, given a key it lacks, the map it is. An array
  of records holds them, `( { x: 1 y: "a" } { x: 2 y: "b" } ) [ y' at ]
  map`, and an array of undecided literals holds maps.

A record type is not made of itself yet: one named in its own fields,
`tree: { left: < tree > ; … } ;`, is an error, not code that applies
itself.

### Types of their own

`own` makes **a type of its own** of a representation, and `;` names it
with the path it defines (TYPES.md 2.11), so two over the same
representation are two types, and its values are its representation's,
costing nothing at run time:

```
celsius: f64 own ;
fahrenheit: f64 own ;
20.0 celsius !                       20.0, a celsius: `!` checks nothing
20.0 celsius ! sqrt                  4.47…, an f64: the f64 word forgets it
temp: celsius ;                      an alias, `celsius` named already
```

- **Named where it is defined.** One no `;` binds is an error wherever it
  is used, "a type of its own, named where it is defined", and `;` binding
  one named already is an alias. A namespace's is its path, `math.cc`.
- **A subset of its representation,** as `bool` is of i64: accepted
  wherever the representation is, matched a little less well than a
  clause of its own, and forgotten where the representation's word is
  applied, so `price sqrt` is a decimal. A word of any type, `dup` or
  `max`, keeps it.
- **A clause of its own keeps it**, the program's `+` one family with the
  core's: `+: < cash cash >. < f64 f64 > ! + cash ! < cash >. ;`, `< f64
  f64 > !` forgetting it, so its body's `+` is the core's. A literal takes
  its type from context, so `price 3 +` is that clause's, while `1 2 +`
  is the core's, a literal's own kind fitting it best.
- **Made by `!`** of a value of its representation, or of one that
  forgets into it, `20.0 sqrt celsius !`, and a literal becomes it as its
  representation, `{ x: 1 y: 2 } place !` where `place: point own ;`.
  Wanted by a clause, a value of the representation made as the program
  runs is no match: `!` says it is meant. So is an array of them, or any
  value holding them, by `!` of one of the representation's, `( 1.0 2.0 )
  < 2 celsius vec > !` as `[ celsius ! ] map` gives, each element its
  own: of the representation only, so `( 1 2 )` is no `2 celsius vec`,
  nor `( 1 0 )` a `2 bool vec`.

### Recursion and instances

A family is evaluated where it is applied, on the caller's judgments,
unless it is recursive. **Recursion is found as it happens:** when a family
is applied again, to the same types, inside its own application, the
compiler unwinds to the outer application, puts back its judgments and code,
and makes it a call to an **instance**:

- An instance is the family compiled once for its input types, as a word
  of its own, guards and all, and cached: every word applying `fact` to an
  i64 calls the same code, by its content identity. A literal input takes
  the type of the best match, or its default, first.
- Inside the instance, the family applied to those types again is a call
  to itself, `recur`.
- Recursion through other words is found the same way. Only recursion
  needs an instance; every other word is inlined, so folding is unchanged.
- Its inputs are values at run time, symbols among them: a rule that asks
  its own family again, `is: < x Mortal >. drop Human is ;`, is a
  word testing symbols as it runs. Known inputs do not unfold through
  recursion, numbers or symbols: `10 fact` calls the instance (TYPES.md
  3.16).
- A value known only while compiling, as a quotation, has no cell for an
  instance to take, so a family applied to one again **unfolds**, its
  guards decided by the values, until a clause finishes: so March walks
  a quotation (SYMBOLIC-FORTH.md 6.6). Depth bounds it, sixty-four words
  inside words.

**The ghost.** A recursive call's results have the types of the
alternatives that finish without recursing (TYPES.md 2.7). An alternative
that reaches the recursion before any has finished **waits**; once others
have finished, their results type the recursion and it is compiled again.
So the base case need not come first:

```
positive: 0 gt? ;
down: < i64 >. ;                        the base, with no guard
down: < i64 positive? >. 1 -+ down ;    tested first, compiled after
```

A literal result, as `fact`'s 1, takes the type of an input it can become,
so `fact` on an f64 multiplies floats; if the instance then leaves other
types than the ghost said, it is compiled again with the types it found. A
family that applies itself with no clause finishing without doing so has no
types for its results, an error where it is defined, unless its signature
promises outputs, a check last in its body, `< i64 >.`: then it is a loop.

**Tail calls.** A call followed only by a return, perhaps after branches,
is a tail call, and a call to itself one in its own frame, so recursion that
loops runs in constant space: a million levels of `sumto`.

```
march8 --code '5 fact.'      source:  5  tail fact-i64
                             fact-i64:
                               dup 0 eq 0branch L  drop 1  branch END
                             L: dup 1 i64- recur i64*
                           END:
```

## The core vocabulary

`core/core.march` defines `+`, `*` (its glyph `⋅`), `-`, which negates,
and `/`, the reciprocal (its glyph `÷`), with `-+` and `/*`, which subtract
and divide by negating and adding, and by the reciprocal and the product,
in one word until the compiler fuses the two (Thomas, 2026-10-09: so `-`
has one meaning, and an inverse is a word); `^`, power; `mod`, the comparisons, `length`,
`at`, `concat`, `slice`, `print`, the folds, reductions and `repeat` as families
of clauses over primitives, in March. Also there: `=` and `~` as `dup` and
`swap`; `<>` as `dip`, and over it `over`, `rot`, `keep`, which applies a
quotation to a value and keeps the value, and `bi`, which applies two to
one, `3 [ dup * ] [ 1 + ] bi` leaving 9 4, each of them also n wide,
`dup_2` and `keep_2` being FORTH's 2DUP and Factor's 2keep, the only
meaning the core gives a subscript, and given as well to a program's word
that only moves values or is handed code; `power#`, which makes a power, a
quotation, by the effect of its word, `effect#`, a word applied by its
symbol, `+' 3 power#` being `[ +' . +' . ]`, `+^0` and any operation's 0th
power being its identity, `+' unit# !`, a literal whose type comes where it
is used, which `quote !` makes code again, `[ + ]`, for `identity#` to find
which it is, and a value's `^0` being `⋅^0`; the conversions of `unit#` to each type, and the facts
they read, `unit` for each primitive operation (`i64+' unit` is 0,
`string-concat' unit` the empty string), and `least` and `greatest` for
each order's ends (`i64' least`, `f64' greatest` ∞, `bool' greatest`
true), and `identity#`, which finds which fact holds by reading the words
as data, `clauses#` and `effect#` (2026-10-21): it follows the operation
at the type to the primitive it applies there, through a word that applies
one operation or swaps first, or to two clauses keeping the greater or
the lesser of two values, by `gt` or `lt`, which only move them, and says
where it finds none;
and the two rules that rewrite a power and a width, `dbl^2` as `dbl 2
int# ! power# .` and `dup_2` as `dup 2 int# ! width#`, each word read by
`name#` where the name was applied
(doc/design/SYMBOLIC-FORTH.md 21, 22, 24, 25); the guards `zero?`, `positive?` and `negative?`, and `abs`, `min` and
`max` as clauses chosen by them, with no `if`; `sqrt`; `pow`, x to the y,
exact for two literals and checked for integers; and `divmod`, the quotient
and remainder, floored (`-7 3 divmod` is `-3 2`), with `div` and `mod` each
taking one of them, so the three agree, integers having no reciprocal; and
`bool`, with `true` and `false`, which comparisons leave and guards take,
and `not`, `and` and `or` as 1 - x, the minimum and the maximum (TYPES.md
2.16); `floor`, `ceil`, `round` and `trunc`, which narrow a float to an integer,
saying how it rounds (`round` halves away from zero), exact on a decimal
or a ratio literal, an error for a float with no i64; converting does not round, so
`2.5 i64!` is an error that names them. doc/design/WORDCHART.md
charts the words beside APL's and Uiua's. `order`, `<#`, gives the positions
that put an array in order, stable, APL's grade up `⍋` and R's `order` (not
"grade", which is too near "gradient"); `at` given an array of
positions gives the elements there; and `keep` the elements a mask of
bools keeps, APL's compress. So `sort-by`, a sort by any key, is five
words, `over swap map order at`: the keys made, ordered, and the array
read in their order; core keeps it, as long enough to be worth a name
(Thomas, 2026-10-09). From SYMBOL (~/Projects/axiomatic/symbol, Thomas's
expression language): `dorder`, `#>`, the descending order, stable, as APL's
`⍒`, which `order reverse` is not; `take`, an
array's or a string's first n elements or its last n when n is negative,
and `skip`, all but those (`drop` being the stack's); `thru`, the integers
from a to b, both included, counting down when a is the larger, its glyph
`‥` (two dots as one character, since `.` applies);
`sum` and `prod`, with their glyphs `∑` and `∏`, a fold from 0 and from 1;
and `⊤` and `⊥` for `true` and `false`. Text: `split` at a separator
(between characters, for ""), `lines`, `words` at white space, as
Haskell's are. A string converts to the number it writes, `"42" i64!`, by
clauses for strings under `i64` and `f64`: `i64 nil or`, the value tagged
at run time or nil, and on a known string the value or nil itself. An i64
converts to an f64, `12 i64! f64!`; an f64 to an i64 only by narrowing,
which says how it rounds. `le?`, `ge?` and `neq?`, with
`≤`, `≥` and `≠`, are "less, or equal" rather than "not greater", so that
NaN is at most and at least nothing. Arrays are equal element by
element, and only as long as each other, `( 1 2 ) ( 1 2 ) eq`, a rule in
core.march (2026-10-26); tuples and records are not yet. Reading is by the core's first
namespaces (TYPES.md 3.12): `io.stdin.read` is standard input's text and
`"data.csv" io.file.read` a file's, when the program runs, an effect, so a
filter starts `io.stdin.read lines`; read while compiling, `[ "notes.txt"
io.file.read ] !`, a file's text is known, so what follows folds on it
(TYPES.md 2.15; below, "Code"). The primitives beneath are `stdin-read` and
`file-read`. The
`case` namespace puts a string in a style: `case.lower` and `case.upper`,
element by element over arrays of strings too, and `"Hello World"
case.snake` is "hello_world", with `case.kebab`, `case.camel` and
`case.pascal`; imported, all of them or one alone, they are `lower`,
`snake` and the rest, and anyone may add one.
The primitives are in `src/prims.rs`: a name, a signature and the machine
operations, such as `i64+ "< i64 i64 >. < i64 >."`, which is checked addition.
The literal primitives (`int#+`, `dec#>money`, `ratio#/`) exist only at
compile time.
A primitive applied to values all known folds, unless it has an effect
(`write`) or makes an array, which has no known value here. It folds by
running: the stage runs the primitive's own operations on a machine of its
own and reads the results back, so a primitive has one meaning, compiled or
folded, as FORTH's immediate words run the same code as the program does
(the kernel shrink, 2026-10-14). Only the literal primitives, which have no
value at run time, fold by code of their own (`prims::fold`). A fold that
fails is an error while compiling: `i64+` on 9223372036854775807 and 1 has
no i64 result.

A few are handled by the stage itself, because their types are not a
signature's: the stack words, which move judgments, the values at run time
among them moving, only if their order changes, by rolls or the scratch
stack, whichever is shorter; `quote-length`, `quote-at`, `quote-slice`,
`quote-concat` and `quote-insert`, a quotation as a sequence while
compiling, its elements Low Stack's, a symbol or a quotation, a meta-op
the symbol of its name, `'.'`, which the core's
`length`, `at`, `slice`, `concat`, `insert`, `first` and `rest` take
(SYMBOLIC-FORTH.md 6.6), and `each`, unrolled, its quotation applied to
each element in turn, so `fold` walks one no deeper for its length; `clauses#`, `w ins clauses#`, the clauses of w
that dispatch would choose for inputs of the types ins gives, a type or
code that leaves them, `< i64 i64 >`, or values, `[ i64+' ]`, a value
pattern for another value not chosen, in the order it would try them, each
a quotation of what `;` takes after the name, each bracket as its code,
`[ [ a var# . a var# . gt ? ] [ a var# . ] [ drop . ] ]`, a primitive
having none, and `effect#`, how many values a word or a
quotation takes and leaves, so March reads a word as it reads a quotation
(SYMBOLIC-FORTH.md 6.6); `name#`, a symbol as a name where a rule's
name was applied, the word expanding finds there, by the shortest name the
root finds it by (SYMBOLIC-FORTH.md 16, 25); `width#`, which applies a
stack word or a combinator with each value a group, `dup`, `drop`, `swap`
and `dip` moving groups, and a word made only of them or handed code
widened in its body; `dip`,
which compiles its quotation with the value under it out of reach, on the
scratch stack if it is a value at run time and the quotation has code;
`vec-length`, a
constant; `vec-at`, which checks a literal index against the length;
`vec-concat`, whose length is the sum; the tuple words; and `empty`, whose
type is the type it is given.

## Code

The stage emits the machine's operations (march7/docs/ENGINE.md), and a
string literal as a data object and `text`. Shuffles across a value `dip`
sets aside become the one operation they come to, `[ dup ] dip swap` the
machine's `over` and `[ swap ] dip swap` its `rot`, so stack words defined
in March cost what primitives did. A final call becomes a tail call, and a
return ends the word. Code is content-addressed as in march7,
in its own domain, `march8/code/v1`.

The machine is march7's, with fifty-seven primitives added: `pick`; checked
signed `i64+`, `i64-`, `i64*`, `i64-quot` and `i64-rem`, which trap on
overflow and division by zero, and `i64-divmod`, floored; signed `i64lt?`; `i64>text` and `f64>text`;
`scratch-at`, which reads a loop's state; `range` and `reverse`;
`text-join`, which joins texts in one buffer; `reach`, which lowers an
array or map literal's mark by the cells its code reached below it;
`money>text` and
`string-show`; `sort-ints`, `sort-floats` and `sort-texts`; `f64-sqrt`,
`f64-pow` and `i64-pow`; `f64-floor`, `f64-ceil` and `f64-round`;
`vector-insert` and `vector-remove`; `union-make`, `union-tag` and
`union-value`; `grade-ints`, `grade-floats` and `grade-texts`, and their
`grade-down-` forms; `vector-keep`; `split`, `lines`, `words`, `lower` and
`upper`; `snake-case`, `kebab-case`, `camel-case` and `pascal-case`;
`symbol-show`; `parse-int` and `parse-float`, which leave a value and a
flag; `read-file` and `read-stdin`, whose failure is an `Io` error with the
system's reason; `handle` and `unhandle`, which keep a word's inputs
for its handlers while it runs (above, "Errors"); and `hold`, `release`
and `holds`, which count a value's holders (below).

**Freeing by counting** (doc/design/REPRESENTATIONS.md 4, built
2026-10-25; Thomas: "a drop unlinks a link in a list or a tree, thus
removing itself from the path of oweneship"). A value, a string, an array or a map, counts its holders:
the cells on the stacks that are it, the containers it is in and the
handlers that keep it. It is made with one. A copy holds it once more, and
a drop once less, and with none left it is freed, and each value it held
with one holder fewer, in turn. Since values never change, nothing holds
itself, so the count is exact. Which cells are values is known only while
compiling, by their types, so the stage says so: after a copy of one,
`dup`, `over`, `pick` or a scratch read, it emits `hold`, and for a drop of
one `release` in place of `drop`; after making a container whose cells are
values, `holds`, with a mask of which: bit i its element i, every bit every
element, and for a map 1 its keys and 2 its values, a union's set by its
member where it is made. Numbers cost nothing. Every other primitive lets go
of the values it takes, once done with them, as a drop would: `length`
once it has the length, `at` once it holds the element it read. So a value
never copied is never counted: it is made with its one holder and freed
where it is used up. `handle` takes a mask too, of its inputs, and holds
them while its body runs. The session holds its stack while a run may
fail, to put it back whole. `region-free` frees raw bytes, as before, and
lets go of a value.

An edit, `put`, `insert`, `remove`, `concat`, `slice`, of a value nothing
else holds changes it in place, the element it replaces let go of; of one
held elsewhere it makes a version that shares the rest, which holds each
value in it once more, a pass over them. On `lower` lowering itself, the
run's peak is 1.3 MB, the working memory's megabyte and 258 KB of values,
as REPRESENTATIONS.md's measure foretold, not 16.8 MB, and 2 KB is held at
the end, the literals, which are made once and kept. It takes 0.92 s, not
0.67, most of it 23,000 versions of token arrays still held by the record
they were read from, `rd.toks … push put`, each a pass over a thousand
strings. The tree is shared, not copied; the pass is the count, each
version holding every string it shares. Counted by node, a version would
hold only the nodes it makes (REPRESENTATIONS.md 4). A trap does not free
what it throws away, which only a table beside the code could say.

Branches are labels until the code is sealed, so each alternative of a
choice can be compiled on its own and laid out afterwards. The stage seals
code itself, hashing it into blobs, and the session publishes them, callees
first. `march8 --code SOURCE` prints the code a piece of source compiles to,
and the code of the words it calls.

**Running while compiling** (doc/design/SYMBOLIC-FORTH.md 6.32). `!` with a
quotation on top runs it while compiling, as FORTH's `[ … ] LITERAL` does,
and leaves its values known, so what follows folds on them: `[ "notes.txt"
io.file.read ] ! length` compiles to the length. The quotation takes
nothing from below, known only as the program runs, so `2 [ 1 + ] !` is
refused. Its code is compiled as a word of its own and run on the stage's
own machine, the one primitives on known values are folded on, which is
given all the code the stage seals; then what it left is read back by its
types, numbers, strings, symbols, and arrays and tuples of them. A map is
not read back yet. A value known as the code is compiled stays as it is,
a quotation or a type too, with no run if there is no code, so `[ … ] !
.` applies code made while compiling. Each quotation runs once, kept by
its code's identity, in at most ten million steps, and what it writes is
shown before what the program writes. A failure as it runs is an error
where it is applied, of the failure's kind: a file not read is an `Io`
error, as `io.file.embed`'s was until 2026-10-24, when `[ … ] !` took its
place.

## Sessions

`Session` (`src/lib.rs`) holds a machine, a stage and the types of the
values on the machine's stack. Each piece of source is compiled as one word
whose inputs are the stack, then run, so the types of the values on the
stack are always known, and the display writes every value by its type:
`{ "k" ( 1 2 ) } ( "a" "b" )` shows as written, which march7 could not.

```sh
cargo run --offline -- --eval 'fee: < money >. 1.10 + ; 19.99 fee'
# <1> 21.09
cargo run --offline -- --types --eval '{ "a" ( 1.5 ) } 2'
# <2> { "a" ( 1.5 ) } < string 1 f64 vec map > 2 < i64 >
```

With no arguments, `march8` is a REPL: each line is compiled and run, and
the stack shown after it. A line ending inside a bracket or a string goes on
to the next, and an error leaves the session as it was. Commands start with
`:`, which March does not use: `:types` shows each value's type after it, in
a bracket, so the line reads back as March; `:code SRC` shows the code SRC
compiles to; `:quit`. `march8 fmt` rewrites `\name` escapes as their symbols,
from stdin to stdout, in code and comments but not strings, and writes a
name's trailing `_` and digits as subscript digits and `^` and digits as
superscript digits, `dup_2` as `dup₂` and `+^4` as `+⁴`, before any marks
and in code only. The reader reads them back, so a name's spelling is the
one typed, `dup₂` and `dup\_2` both being `dup_2`
(doc/design/SYMBOLIC-FORTH.md 23).

## Errors

Errors are found while compiling, with the line and column of the word
being applied, not its `.`, since a name remembers where it was written, and
the words being applied:

```
1:15: no word `+` for i64 string
  in `f`, defined at 1:3
```

Their kinds: no word; a mismatch (types that disagree, clauses that tie,
values missing or out of reach, a type left unapplied); a literal that
cannot become its type; a value needed while compiling that is known only
at run time, as `vec`'s length; arithmetic that overflows or divides by zero,
found at compile time; an effect where none is allowed; a limit. An error
inside a core clause is reported where the clause was applied. Errors while
running are the machine's, overflow, a file not read, or a trap: the
machine's `trap` takes a message and a kind, each text, and fails with them,
`Error::Trap` (2026-10-22). A family none of whose clauses holds at run
time traps so, by `no-word`, saying which, as compiling does: ``no word
`door` for sym sym: no clause's guard holds``; arrays that `zip` or `keep`
pair and are not as long, or too short for `within`'s quotation, by
`mismatch`, as compiling would. Where a trap happened is not yet reported:
a position written in the code would make a word's identity depend on
where its source is laid out, so it wants a table beside the code.

**A trap is handled** by clauses of the word that take its inputs and a
trap on top, `size: < string io' trap >. drop drop 0 < i64 >. ;`, the
trap's kind in its type, or any kind, `k trap` (SYMBOLIC-FORTH.md 29,
2026-10-22). Where such a word is applied, the machine's `handle` keeps
the inputs that are values at run time, and `unhandle` lets them go when
the word is done. A trap before then, `Error::Trap` or the machine's own,
overflow (`arithmetic'`), a file not read (`io'`), or an element or a
key not there (`mismatch'`), but not the fuel
limit, puts the stacks back as they were, the inputs on them and the trap
on top, and goes on into the handlers' choice, compiled where the word is
applied; a trap none of them takes traps again, for a handler further
out. So the innermost handler catches first, a recursion's at each level.
The word's results and the handlers' are joined as a choice's
alternatives are, a different type making a union, and a handler that
leaves a trap, alone or in an array, is an error. `kind` and `why` read a
trap, its kind a symbol and its message a string, the kind's name if it
has none, and `fail` fails with it again. A clause that takes a trap last
is a handler only where clauses of its word take the inputs; elsewhere it
is a word applied to a trap, as a helper is in a handler.

**A program fails by `fail`** (doc/design/SYMBOLIC-FORTH.md 29), which takes
a kind, `no-file' fail`, or a message and a kind, `( "can't read it"
no-file' ) fail`, and never returns. While compiling it is an error of that
kind, the machine's own kinds by their names, `syntax'`, `no-word'`,
`mismatch'`, `literal'`, `unknown'`, `arithmetic'`, `limit'`, `effect'`
and `io'`, and any other the program's own, `Kind::Fail`. Its message is
the one given, or ``fails: `no-file` ``, said where `fail` is with the
words applied around it. A clause that fails is defined as any other, and
leaves whatever it must, since nothing after `fail` is reached; it is
checked where defined only as far as `fail`. Where the code may not run,
in a choice made at run time, a union's arm or a loop's body, it fails
only if it runs, a trap (2026-10-22), whose kind and message may be known
only then, a symbol, or a tuple `( string sym )` made as the program runs;
with no message, it is reported by its kind. A path that fails ends in its
trap and leaves nothing, so the other alternatives give the choice its
results; a choice all of whose alternatives fail never returns either.
Inside a handled word, every failure is the program's, a trap, so a
handler may catch it, arithmetic on known values among it. A handler runs
only if something fails, so its code is code that may not run. Elsewhere,
code that is sure to fail fails while compiling: where its kind is known
only as the program runs, a choice every clause of which fails or a
`fail` of a kind made then, that is an error that says so, and a loop's
quotation that always fails has no effect to compile the loop by. A
recursive word's instance is compiled for itself, as code that surely
runs, wherever it was first applied.

Warnings have three levels: informative, minor and severe (Thomas,
2026-10-09). The session collects them, and `march8` writes them to
standard error, `1:1: informative: …`. Two are built so far, both
informative: where a union is made (above, "Unions"), and where clauses of
different lengths both may match (above).

- **Informative:** what programs can and often do, with a clear downside,
  that could be avoided by writing it another way. A slot that may hold
  more than one type at run time costs a branch where it is used (TYPES.md
  3.6); many instances of one family cost code size.
- **Minor:** probably a mistake, but harmless as written: a value computed
  and then dropped, a name defined and never used.
- **Severe:** almost certainly a bug that still compiles: a clause that can
  never be chosen, since an earlier one always wins; a guard that is always
  true.

## What it found

1. **Most of the language fits in March.** Arithmetic, comparisons,
   containers, printing and literal conversion are families in
   `core.march`; Rust holds the primitives and the stage's rules.
2. **Late materialization folds more than placeholders did,** and needs no
   patching: constants pass through stack words and families, and only
   values at run time cost code.
3. **No match is no word, everywhere.** Errors the prototype reported as type
   mismatches (`< i64 f64 > +`) are no word here, since `+` has no clause for
   them, which is the design's rule.
4. **An array literal is a vec,** so lengths are constants wherever arrays
   are written out, and a vec is accepted as an array.
5. **Guards fold like everything else.** A guard is a word applied to
   copies of the inputs, so on known values it is decided at compile time
   with no extra mechanism, and the choice disappears.
6. **Compiling each alternative into code of its own** lets the results'
   types be joined before any code for them is fixed, so a literal in one
   clause adapts to the type another leaves (TYPES.md 2.7).
7. **Lifting needs no special case:** with `map`, it is ordinary clauses.
8. **Recursion is better found as it happens than from the text.** A test
   for a family's name in its bodies said `+` was recursive, since lifting
   applies `+` to the elements; applied to other types, that is not
   recursion. Finding the same types again during evaluation is exact, and
   finds recursion through other words too.
9. **Waiting alternatives free the order of clauses.** The prototype needed
   a guarded base first; here any alternative that finishes types the
   recursion.
10. **A real program runs** (2026-10-09). `examples/wordfreq.march` counts
    the words of a text and lists them by word and by count, in about 40
    lines: value patterns for separators, guards,
    recursion with a tail call, a comprehension collecting each word or
    nothing, `get`'s `i64 nil or` taken apart by clauses, `fold` and a
    string with holes. Writing it found:
    - **Missing words,** added: `put`, a map with a key set, and `empty`,
      an empty array or map of a type, `string i64 map empty`, since `( )`
      and `{ }` cannot say their types. `empty` is provisional.
    - **A literal string in a loop was made again on every pass,** its bytes
      chunked and hashed each time, which made the count quadratic: 4.6 s
      for 1,000 words. The machine now makes a string from read-only bytes
      once, by where they are: 0.2 s, 13 ms in a release build.
    - **The memory limit stops it at scale.** Nothing is freed yet, and each
      `put` is charged a whole map, though versions share their nodes, so
      20,000 words pass the 16 MB limit. With the limit raised they take
      0.3 s and agree with Python. Charging what a version adds, or freeing
      a version nothing else holds, is the memory work deferred to 2.0.
      Since 2026-10-25 a version is charged what it adds (Thomas): an
      array's, a string's or a map's made by an edit, `insert`, `remove`,
      `put`, `concat` or `slice`, is charged the nodes it does not share,
      about a kilobyte for the leaves at each seam and a branch for each
      level above them, and never more than it would be on its own, so
      5,000 appends are charged about 9 MB, not 100 MB. `insert` and
      `remove` are one splice. Freeing was still 2.0: about 8,000 versions
      kept filled the 16 MB. The limits were raised to 64 MB and a million
      objects on 2026-10-25, at Thomas's asking, so that `lower` lowered
      itself; a value no one holds is freed since that day too, so a
      version replaced is let go of, and an array's or a map's, nothing
      else holding it, changes in place (above, "Code"). The limits stay,
      a ceiling on what is held at once.
    - **Still wanted:** a way to read input, so the text is not a literal;
      and a word applied without its `.` is the easiest mistake to
      make, though the error names it. Wanted then and since built:
      `not` and `and` (`bool`, TYPES.md 2.16), `sort-by`, and `words` and
      `lower`, which the example now uses in place of its own splitting.

## Not yet

Bugs, what is next, and what is still to think on are in TODO.md, at the
repo's root.

- **The rest of bare bodies** (doc/design/SYMBOLIC-FORTH.md 6.32). Built
  2026-10-24: a body bare, `sq: dup * ;`, a bracket in it a value, a
  signature a check applied, `sq: < i64 >. dup * < i64 >. ;`, `[ … ] !`
  running a quotation while compiling, and a body reaching below its
  first check taking those values too, `ff: < i64 >. + ;` taking two, a
  letter each, where its effect is sure. Built 2026-10-26: a check that
  cannot be decided while compiling, a guard or a value pattern in it, a
  union's member, `"a" get < i64 >.`, or a symbol of a union of values,
  `< kind >.`, tested as the program runs, a family of one clause, the
  check its signature and nothing its body, chosen as a clause is and
  failing by a `mismatch` trap, "expected i64, found nil", where it does
  not hold. Not yet: a map left by `[ … ] !`, read back.

- **More of traps** (doc/design/SYMBOLIC-FORTH.md 29). Handlers are built
  (above, "Errors"); not yet are kinds as types, decided 2026-10-22,
  made where defined by `trap` from a map holding a message, named by `;`,
  `err.file.notfound` under `err.file`, a handler taking its kind
  and every kind beneath it, the core's kinds under `err` and the
  machine's failures among them, which waits on defining types; and where
  a trap happened. A kind is a symbol here still. A trap may not be stored; with no global
  store in march8 yet, nothing checks that.
- **Recursion on known numbers and symbols while compiling.** A family
  applied to itself on values that have cells becomes an instance on
  values at run time (above, "Recursion and instances"), even when they
  are known, so `5 fact` is a call, and a recursion over symbols alone has
  no types. With a value known only while compiling among its inputs, a
  type or a quotation, it unfolds instead, its guards decided by the
  values, which is what March's own work while compiling uses.
- **Loops in March, the kernel shrink** (2026-10-14). `map`, `zip`,
  `table` and `within` are about 470 lines of Rust, which could be words in
  `core.march` over `each`, the one loop the stage types. What was found:
  - **`map` is no comprehension.** In a literal, what a pass leaves is
    gathered on the data stack, between the next element and the values
    below the array, so `( 100 ( 1 2 3 ) [ over + ] each )` is a scan,
    `( 100 101 103 106 )`. Comprehensions stay as they are, the general
    case: each pass sees what the last left, and the literal takes all.
  - **`map` follows Joy's rule** (Thomas): each element in isolation, on
    the stack below the map, what the quotation consumes restored and its
    one result collected, so `100 ( 1 2 3 ) [ + ] map` would be `100 ( 101
    102 103 )`, as in Joy, where it is an error now. The stage restores by
    copying what the quotation takes, which it knows: `[ + ]` in isolation
    is `over +`. Lifting stays clauses over `map`, which is still needed
    for words whose meaning on an array is not their meaning on each
    element, `length`, `show`, `split`, as APL's each `¨` is.
  - **So `map` keeps its state off the data stack,** as FORTH keeps a
    loop's on the return stack, which here is the scratch stack. March
    says it with Joy's and Factor's `dip`, written `<>` (Thomas), as `=` is
    `dup`: `x [ q ] dip` runs q with x set aside, and puts x back on top
    (built 2026-10-15, with `over`, `rot`, `keep` and `bi` over it).
    It is `>r q r>` in one word, balanced by construction, so nothing can
    be left aside and no zero-net rule needs policing; Factor dropped `>r`
    and `r>` for it. While q compiles, x is out of reach, and natively it
    stays in a register. Arrows were the other candidates, `->` and `<-`
    moving and `=>` and `<=` copying (Thomas), but `->` and `<-` are wanted
    for list edits and `<=` reads as ≤. `<>` is a name to march8's reader,
    which takes only a lone `<` as a bracket; the list edits (TODO.md) wanted
    it for pairing, which will need another spelling. Thomas first named
    `dip` `_`, which now marks a bracket's arguments.
  - **Moving the loops waits on the machine code** (Thomas). The stage
    knows both stacks' depths everywhere, so natively both can be
    registers, and either way costs nothing; where the definitions live is
    the choice.
- **Deferred terms and rewriting clauses.** `range` and `reverse` build
  arrays. Since everything is a value until `.` applies it, applying a pure
  word could leave a term, "`reverse` of this array", made code only when
  something forces it, and a clause could match the term's form: `fold` on a
  `reverse` would be a loop backwards and `n range each` a counted loop, with
  no array built (Thomas, 2026-10-08). That is rewriting, as GHC's rules, in
  March itself, with no views in Rust. Only pure words may be deferred, which
  the effects say; a rule's two forms are trusted to be equal.
- **`#` for `count`,** as J's tally, perhaps also meaning `each` on a
  quotation (`xs [ f ] #`), the literal then deciding what is collected
  (Thomas, 2026-10-08, noted). It needs headings to start lines, as in
  Markdown, since SURFACE.md's headings are words that read what follows.
  For now the word is `count`, the same as `length`.
- **Array literals typed by what follows.** `( 2 4 )` settles at its `)`, to
  integers, so `( 6.0 8.0 ) ( 2 4 ) /*.` is no word; written `( 2.0 4.0 )` it
  works.
- **Glyphs:** APL's and Uiua's for these words are deferred. `/` is the
  reciprocal, so reduce is the word `reduce`.
- **Fusing `- +` and `/ *`.** The stage should compile a negation followed by
  an addition as a subtraction, and a reciprocal followed by a product as a
  division: as fast, exact for floats (`x ⋅ 1/y` can differ from `x / y` in
  the last bit), and with no overflow negating the least i64. Until then
  `-+` and `/*` are each one word.

- **Mutual recursion** between instances, which would make a cycle of
  content identities; and words used before they are defined.
- **Sharing large words** that are not recursive, as instances, to save code:
  everything else is inlined, which is fast and folds constants, but grows
  code. An instance keyed by its known inputs as well as its types, returning
  its known results as judgments, means exactly what inlining means, so the
  choice is only one of size and speed (2026-10-08).
- **Definitions' identities.** A CID names compiled code: an instance, or a
  piece of top-level code, with names, types and inlined words gone. A
  definition has no identity yet. Open: its signature and words as written,
  families resolved where it is used; pinned, as Unison, each name resolved
  when it is defined to the family as it is then, so a new clause makes new
  identities; or modules pinning what they import. To decide with images and
  modules (doc/design/TYPES.md, open question 2).
- **OR between whole contexts.** A union is one slot's: `< num num >` takes
  an i64 and an f64 together, and `< i64 i64 >` or `< f64 f64 >` as one
  context is not built.
- **Representations chosen by use** (2026-10-09). A tuple forgets its
  positions where a clause asks for an array, at that use, each time it
  runs, and twice for two copies; the forgetting changes its type. The
  type should say what a value is and the compiler choose how it is held,
  from what is done with it where it is made (indexed, appended, iterated,
  shared, as effects on values) and whether anything else can see it, which
  a stack language shows by `dup`, `over` and `pick`. A tuple used mostly as
  an array would then be built tagged, its positions still known, read with
  no test; and a flat array, a persistent tree, or an update in place chosen
  the same way. Some of it at run time, by size or by a check that a value
  is unshared, as Koka's Perceus does. A project of its own, beside
  freeing's counting while compiling (below).
- **The rest of freeing** (doc/design/REPRESENTATIONS.md 4). Built
  2026-10-25: a count of holders, the stage saying which cells are values,
  `hold`, `release` and `holds`. To come: a version of a value held
  elsewhere sharing its holds by node, not a pass over its values, which
  `lower`'s token arrays need (a field taken, by the compiler or by a word,
  was refused 2026-10-26: REPRESENTATIONS.md 4); counting while compiling,
  so that a copy only read needs no count; and what a trap throws away,
  freed by a table beside the code of which cells are values where it can
  happen.
- **`fmt` to mathematics** (Thomas, 2026-10-09). `march8 fmt` rewrites
  `\name` escapes as symbols; a "mathize" mode should also render what is
  typed as what mathematics writes: `*` as `⋅`, `/` as `÷`, and a bracket's
  `< … >` as the mathematical angle brackets `⟨ … ⟩` (U+27E8, U+27E9,
  LaTeX's `\langle` and `\rangle`). The reader then reads `⟨ ⟩` as `< >`,
  so either form reads back, and the way back is the same rewrite.
- **More of files.** Writing a file, which cannot be another
  `write` clause, since `write` takes one string and a family's clauses
  take as many; `string error or` from `file-read` once there is an error type;
  paths from the source's own place, not where march8 runs.
- **More of known arrays.** An array literal of known elements is known
  (2026-10-21), but words that make an array, `words`, `split`, `lines`,
  `range`, still make it at run time, so they do not fold on known values,
  but in `[ … ] !`, where the array they make is read back, known: a YAML
  file read into data while compiling needs maps read back too. A known array is built once for each copy, so `( 1 2 ) dup` builds
  two.
- **More of tuples, and records.** `rest`, `most`, `slice` and `concat` on
  tuples forget their positions, where they could make tuples; `eq?` has no
  clause for them. A literal with a run among mixed elements is still an
  error. An entry by `;` in braces of anything but a type; a
  record opened into places, for named inputs; equality of records; and
  an array of map literals undecided, which settles into maps.
- **More of unions.**
  A union made and taken apart at once is still tagged and untagged; the
  stage could carry each alternative on into the use instead. A recursive
  word applied to a union may not wait for its results' types inside the
  split. Tags are this session's type numbers, until types have identities.
  `text>integer` still leaves a value and a flag, not `i64 nil or`.
- **Effects in signatures,** declared and checked like outputs, for words
  whose bodies the compiler cannot see; and domains besides `io`, such as
  the global store, with primitives that read.
- **Quotations at run time:** a quotation is always consumed at compile
  time, by `.`, `;` or `map`.
- **Headings:** SURFACE.md's literate headings, as namespaces and contexts
  shared by several clauses, read by the reader.
- **Saving** a session as an image; tuples and records;
  namespaces; showing types.
