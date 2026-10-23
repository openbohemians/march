# Types

Status: design note, 2026-10-07, from a conversation with Thomas. Section 2
records decisions, section 3 what is still open, and section 4 proposes a
way to build it. A prototype is built in march7 (march7/docs/STAGED.md):
integers, floats, money, literal types, arithmetic and stack words through
both stages, typed words evaluated for each use, brackets that annotate,
types as data (`ary`, `vec`, `map`) with containers typed by their
structure, headings, families chosen by types and by guards, with type
variables, outputs, lifting and array literals, and instances, with
recursion typed by ghosts and tail calls. On 2026-10-08 Thomas decided to
build the compiler in Rust, as march8 (march8/docs/MACHINE.md), with System
March frozen, and to write it in March once March is mature. Its first
three slices build the symbolic stack machine for the explicit form, with
everything the prototype had: families, guards, lifting, recursion by ghosts,
instances and tail calls.

## 1. Why

March7's checker gives every value a type of one byte: i64, f64, arrays by
rank and element, a few kinds of map, strings (march7/docs/CHECKER.md). The
byte space is nearly full, and it cannot describe nesting or mixtures:

- **Display.** `{ "k" ( 1 2 ) }`, `( "a" 2 )` and `( ( "x" ) )` show raw
  cells, because the types of their elements are unknown.
- **Wrong answers.** `{ "k" ( 1 2 ) } "k" at 1 +` returns `4294967431`: the
  checker does not know the value is an array, so `+` takes its integer
  clause and adds 1 to the array's handle number, with no error.
- **Identity.** A container holding strings or containers is hashed by
  their handle numbers, which differ between sessions.

All three have one cause: at run time a value is an untagged 64-bit cell,
and only the compiler's types say what it is. march4 was the same ("fully
type-erased"). The fix is not to tag values but to make sure the compiler
can always tell.

## 2. Decisions

### 2.1 The compiler can almost always tell

Where it cannot, March falls back to a check at run time. Programmers may
write types for now; inference will make that rare.

### 2.2 Lowering in stages, with types in the code

Thomas: "The trick to this is lowering in stages. And the trick to that is
a first-class type system in the code itself."

`1 1 +` lowers to an explicit form in which every value has its type and
every application is marked:

```
1 i64 ! 1 i64 ! + .
```

This is **Symbolic Forth**, the lowest form (Thomas, 2026-10-13;
SYMBOLIC-FORTH.md): symbols, numbers among them, sets of them in one
bracket, `[ … ]`, and the meta-ops `[`, `]`, `.`, `?`, which tests, `!`
and `;`, which defines (2026-10-14).
Every name is a symbol until `.` applies it. `i64 .` is
the type: every type is given by a word, so types need no spelling of
their own (one that bottoms out might show as `i64'`, were it ever shown).
A type is data, and is not applied: converting is the meta-op `!`,
`i64 . !` or `i64 !` (`!` takes a type, or a type's symbol). It was
`convert .` at first, a word applied, and march8 read it so until
2026-10-19; it now reads `!` as the primitive `!` applied, as `;` is. A
bracket is a quotation, made data the same way, `[ … ] seq# !`, and so is
a number, which is
only a symbol there: `10 i64 !`, or `10 int# !` for a literal whose type
what takes it decides (2.10). A signature there is two sets, inputs then
outputs, `[ i64 . ] sig# ! [ i64 . ] sig# !`, so `--` is only a comment,
and a type variable is given by a word, `n var# .`: no rule of spelling is
left there. (First the form was `1 i64
. 1 i64 . + .`, when applying a type's name converted.) March8 reads an
intermediate form over it, in which this is `1 i64! 1 i64! +` (the last
bullet below).

- **`.` applies.** Everything is a value or a symbol until `.` applies it.
  `.` is the only special word, and types and operations share one
  namespace. (`.` is free: the language prints with `print`.)
- **A name applies; a symbol is spelled as one** (Thomas, 2026-10-12). The
  form above is what code means, every application marked; it is written
  with a name applying, as in FORTH, in code and in a bracket alike. The
  same programs written with a dot after each name applied and without
  showed code applying words far more often than it passes symbols: a CSV
  summary had 57 dots and needs 8 marks without them (DOTS.md, the
  comparison). So the mark goes on the rarer thing. A capitalized name was a
  symbol, `Snake`, `Closed`, as Haskell's constructors, Rust's variants and
  Elixir's atoms are capitalized, so programs of symbols, a state machine or
  facts and rules, read without marks: `Socrates Is Human fact`. Since
  2026-10-17 a capital marks nothing (Thomas): `Snake` is a word, and a
  namespace of symbols imports the old rule as a pattern word, so they read
  there as before (SYMBOLIC-FORTH.md 6.27). One
  lowercase letter is a variable, a type variable in a signature, an input
  by its place from the top in a body (SYMBOLIC-FORTH.md 6.14), and a logic
  program's variable, `x Is Mortal if x Is Human rule`, and names no word.
  `name:` labels what follows, a name being defined or a key, and `name'` is
  the symbol, where a word is passed rather than applied, `+' reduce`, or
  any symbol is written, `Snake'`. A dot at the end of a word applies once more, `[
  2 + ].`, and a `!` converts, `2 i64!`. (From 2026-10-08 a dot after a word applied it, `sq.`,
  every name being a symbol until then; Thomas's rule that a word of
  punctuation alone applies was weighed on the way, and holds here anyway.)
  A dot inside a word is part of it: a number's point, `2.5`, and in a name
  a path in a namespace, `io.file.read` (3.12). For a day dots between names
  chained them, `"data.csv" file-read.lines.` for `file-read. lines.`, as
  uniform call syntax reads (Nim's, D's); but then a path had no spelling,
  so the chain went (Thomas, 2026-10-10).
- **Types are built by words** (Thomas, 2026-10-12). The only base things
  are literals, unstructured until something structures them, and
  symbols, pure literals; everything else is structure, and words build
  it. A type's name gives the type, `i64`, which might one day be `64 int`
  as `string` might be `utf8 str`, and constructors build types from types,
  `string i64 map`. So types and words are one namespace, and words,
  symbols and literals are the three kinds of thing, each spelled its own
  way: a type's name reads the same in code and in a bracket. A type is a
  value like any other, `string i64 map empty`, and data, not called:
  `!` converts by it (Thomas, 2026-10-13), `2 i64!`, `x 100 i64 vec !`, by
  its clauses for literals. Parsing is converting: `"42" i64!` is 42, or
  nil, `i64 nil or`, by `i64`'s clause for strings. (From 2026-10-09 a type's name applied converted, `2 i64`, so a
  type as a value took a bracket, `< i64 >`, and in a bracket names stayed
  names, a dot applying a constructor, `< string i64 map. >`: code and
  brackets read differently, two notations where one would do.
  For a day before that, `i64.` was the type and `to` converted; this
  returns to it, with `!` for `to` and the reason it lacked.)
- **A bracket is code, a comprehension of types** (Thomas, 2026-10-09).
  `< … >` holds code like any other, read as all code is. What it leaves is
  the signature: a type, inputs of that type; a value, inputs equal to it,
  a literal (`0`), a converted one (`0 i64!`) or `true`; a quotation, a
  guard on the inputs before it; a word ending in `?` applied, a guard too,
  as such a word leaves one bool, which the compiler holds it to (Thomas,
  2026-10-12); a symbol, `< string Camel' >`, inputs equal to it; and a
  variable, a type variable. No definition elsewhere changes what a bracket
  means: types come from words and a symbol is marked, so a symbol is
  never a type (from 2026-10-09 `camel!` marked a symbol, and any other
  name left was a type's; to 2026-10-17 a capital marked one, and a bare
  `Camel` is now a word, meaning what it leaves). Reading a signature takes knowing what a
  constructor takes, `< k i64 map k >` being a map and a key, as reading
  FORTH does. Symbols need no declaring and are one namespace: a symbol has
  no definition, only clauses that match it, which belong to families. A
  bracket's code runs while compiling, so it leaves only known values. A
  bracket before a definition's body is the clause's context, and from
  2026-10-14 is never read inside it (SYMBOLIC-FORTH.md 3); anywhere else
  it is a value (2026-10-12), so `[ < f64 > ! ] map` converts each
  element: a type, or, of more than one type, a signature, by which `!`
  gives the values below their types, `x y < i64 f64 > !`. A quotation is
  a guard only tested, `< i64 [ 2 mod zero ] ? >`, each role having its
  mark (SYMBOLIC-FORTH.md 2; a bare one was a guard until 2026-10-20).
  Inputs and outputs are two brackets, touching, `< i64 >< i64 >`, so `--`
  is a comment inside a bracket too (SYMBOLIC-FORTH.md 6.8, 2026-10-20;
  `--` divided them until then).
- **Compile stages evaluate the type part and erase it.** When the type
  stage meets `+ .` with two i64s, it picks integer addition and leaves a
  plain call behind. After lowering, `.` is an ordinary call instruction
  and the types are gone, so cells stay bare and fast.
- **March8 reads an intermediate form** (Thomas, 2026-10-13). Names apply
  without a dot, a prime marks a symbol, one letter a variable, `name:` a
  label: all of it notation over Symbolic Forth, where everything is a
  number, a symbol, `.` or `!`. It lowers to it by spelling alone: `fib` is
  `fib .`, `Snake'`, `+'` and `fib:` are the symbols `Snake`, `+` and
  `fib`, and `2 i64!` is `2 i64 . !`. Both forms
  call with `.` and convert with `!`; one could as well call with `!`, a
  command, and the choice is only that (Thomas).
  Symbolic Forth is the litmus test of a notation: what cannot be said in
  it is not March. March8 runs it (march8/tests/symbolic.rs): the core
  vocabulary and the examples lower, read back the same, and run the same. The intermediate form reads one way in code and in
  brackets, so code is data there too. A surface may come above it, as
  SURFACE.md's headings were; how many stages there are is still to be
  discovered.

This follows The Little Typer: types are values (of type Type), and
annotation is an operator, Pie's `the`. Zig's `comptime`, where types are
compile-time values that are erased, is the closest working example.

Rejected: a prefix form, `i64 1`, in which `i64` is an immediate word that
reads the next token. It annotates only tokens in the source, not computed
values, and makes the meaning of a program depend on the token stream,
which breaks composition and staging.

### 2.3 Judgments

At a compile stage every stack item is one **judgment**: a term with its
type, and its value when that is known (a literal's, a constant length).
`dup` copies a judgment; `+ .` takes two and leaves one; a type is a
judgment whose type is Type.

### 2.4 No runtime values in types

Runtime values in types would make inference undecidable. So:

- **`i64 ary`** is an array of i64 of any length: March's persistent array,
  which grows and shrinks. The length is a runtime fact, never part of the
  type. (Pie's `List`.)
- **`100 i64 vec`** is an array of exactly 100, only when the length is a
  compile-time constant. (Pie's `Vec` with a known length; Rust's `[T; N]`.)
- The compile stage computes lengths when it can: `concat` of a 100 and a
  50 is a 150, and `( 5 [ i0 ] times )` has length 5. When a length depends
  on a runtime value, the result is the unsized type: the type forgets the
  length rather than mention the value.
- A property that depends on runtime values is a runtime check, never part
  of a type.

### 2.5 Mixed data

- **Fixed positions make a tuple:** `( "a" 2 )` is a string and an i64,
  as march7/docs/SURFACE.md already decided ("an array when it can be, a
  tuple when it must").
- **A tuple forgets its positions where an array is wanted** (Thomas,
  2026-10-09), as a vec forgets its length: `( 1 "a" )` given to `sort`,
  `map` or any clause that asks for `a ary` becomes an
  `i64 string or ary`, an array of the union of its types. So positions
  are precise and free while the tuple is a tuple, and every array word
  still works on it. Unlike a vec's, this forgetting costs: each element
  is tagged then, a union made, with its informative warning (3.6). Built
  in march8 (march8/docs/MACHINE.md, "Tuples"); records are not yet.
- **Fixed keys make a record:** `{ port: 8080 host: "x" }`, each field
  typed, its type a map of types, `{ port: < i64 > ; host: < string > ; }`
  (2.11). A record forgets its keys' types the same way where a map is
  wanted, into a map of the union of its values' types.
- **Genuinely variable mixtures need unions,** such as "i64 or string": a
  value whose type is one of several, decided only at run time, so it
  carries a tag, and a family applied to one compiles to a branch on the
  tag. Inside a container the tags are stored per element, which also gives
  stable identities. Planned as set-theoretic unions (3.6, 2026-10-09),
  in place of the declared sums first written here.
- **Unions come later,** at the first real need, most likely JSON, errors,
  or FORTH's words that leave a value or nothing. Until then, failure and
  absence use traps, sentinels, or a value with a flag, as `text>integer`
  does.
- **No `any` type for now.** Its tag would have to name any type at all,
  which brings back dynamic typing. It can come later if wanted.

### 2.6 How words get their types

A word's body stays symbolic. Each time it is applied, the type stage runs
the body on the caller's judgments and gets its result types and an
instance of compiled code, cached by the word's identity and its input
types. So `3 sq` and `2.5 sq` make an i64 and an f64 instance. This is
march4's "Design B" and march7's instances, which is what "inferring a
word's type" means in a staged system.

Signatures in `< >` are optional: a stated contract, or a word whose inputs
are not known when it is stored or passed as a quotation.

### 2.7 Recursion

A recursive call's result is a **ghost**: a type variable that collects
the constraints of its uses, such as "must work with `*` on an i64". A path
that does not recurse (a base clause, or a branch) gives the result type,
and the ghost resolves to it, because choices made at run time must agree
(2.9). The literal 1 of `fact`'s base adapts to the i64 of its step. Branches
split the ghost and their constraints recombine where they meet.

A signature is needed only when the ghost cannot be resolved: no
non-recursive path in reach, or uses that fit several clauses the base does
not decide between. A result type that grows with the recursion, such as an
array nested deeper on each call, would depend on a runtime value, and is
an error.

### 2.8 Families

- **Clauses are signature patterns:**

  ```
  < i64 i64 >< i64 >    integer add
  < f64 f64 >< f64 >    float add
  < a ary a >< a ary >    lift: add to each element ("a" is a type variable)
  ```

  Lifting stops being a special case of the checker and becomes a clause.
- **Classes in patterns** (Thomas, 2026-10-09). `atom` matches any type
  that is not a container: a number, a string, a literal, but not an array,
  a map, a quotation, a type or a name. It is a class, not a type: no value
  has it, it binds nothing, and a clause using it is specialized for each
  type that arrives. It lets one clause lift a number into a tensor of any
  rank (march8/docs/MACHINE.md, "Arrays as tensors"). A union is a class
  too: `i64 f64 or` in a pattern matches either (3.6), and core.march names
  the numbers `num` (built 2026-10-09).
- **Families are open:** other code, such as a money module, may add
  clauses, scoped by namespaces and modules.
- **The most specific clause wins; a tie is an error** for now. ("The
  clause defined last" was considered and left open.)
- **How a clause is chosen** (Thomas, 2026-10-17; not built). The
  shortest first (3.13); then, among clauses of one length, by the top
  slot, keeping those whose pattern there is most specific, then by the
  next slot down, and so on; clauses tied in every slot are an error. A
  pattern is more specific than one that includes it: a value, `< 1 >` or
  `< Open' >`, before a concrete type, `i64` or `sym`, before a union or
  a class, `num` or `atom`, before a variable (Thomas: "literals are most
  specific, 1 is 1 after all … then concrete types, like i64 … and then
  sum types … which are clearly less specific"); a bool is among the
  i64s and a symbol among the strings, so each comes before the type it
  is in. Patterns that both match and neither includes the other tie at
  their slot. An undecided literal given, `1`, which `i64` and `f64` both
  hold, takes its own kind, then its default type, so `1 2 +` is i64's.
  This replaces march8's scores, summed over the slots, by which one slot
  made up for another, `< i64 atom >` beating `< a i64 >`. It is CLOS's
  way, which orders methods argument by argument, the top here being the
  first, rather than Julia's, which calls such clauses ambiguous: deciding
  from the top reaches as little of the stack as it can, as the shortest
  does. Definition order was weighed and left: families are open, so the
  module loaded first would choose; an override, if wanted, is a mark on
  a clause.
- **Every clause has the same stack effect,** as SURFACE.md decided, but
  clauses chosen by type may differ in their result types: `+` gives an
  integer for integers and a float for floats. To be relaxed (3.13).

### 2.9 Choices at compile time and at run time

- A choice made at compile time, by type, may differ in result type.
- A choice made at run time, by value (`if`, a value pattern, a guard),
  whose alternatives give different types gives their union (3.6), with a
  informative warning (Thomas, 2026-10-09; it was an error unless the result
  was a declared sum). Built in march8 (march8/docs/MACHINE.md, "Unions").

### 2.10 Literals

Literals take their type from context, with no implicit promotion
(SURFACE.md). A symbol taken as a string, a value known only at run time
too, is no promotion: it is text until a type structures it
(SYMBOLIC-FORTH.md 6.28). At a compile stage a literal is undecided: its type is a
**literal type**, `int#`, `dec#`, `ratio#`, `str#`, `seq#`, `map#` or `unit#`, an operation's identity,
`+^0` (3.4; SYMBOLIC-FORTH.md 6.21). Thomas: it is "just another
type signature `< f64 lit# >`", so literal handling is clauses, open like
any family.

- **Each type declares once how a literal becomes it:** f64 converts it to
  a float, money converts it exactly. One general rule covers every family:
  a clause that wants a T and finds a literal converts it with T's
  conversion. A specific clause can still override.
- **Three literal types, integer, decimal and ratio.** An integer literal
  can become i64, f64 or money; a decimal literal can become f64 or money,
  not i64. A decimal literal keeps its exact digits until its type is known,
  so `price 1.10 +` adds exactly one dollar ten, never via a binary float.
- **A ratio literal, `1:2`** (Thomas, 2026-10-13; built 2026-10-14): one
  half, exact while a literal, an f64 rounded once or money when it must be
  a value. Its terms are kept as written, so a word that takes it can read
  them: `12:30` is not `2:5`, and `2026:10:13`, no number, could be a date.
  It is not a union of what it could become: a union is one of its members,
  and whoever takes it handles each; a literal is undecided, and whoever
  takes it decides. Two literals divide exactly, `1 3 /*` is 1:3: a literal
  prefers another literal kind to its default, since it keeps it exact.
- **A sequence literal, `seq#`** (Thomas, 2026-10-14; in part): what
  `( … )` collects, `[ … ] seq# !` in the lowest form. Ordered, repeats
  allowed, and undecided like a number: a clause that wants a tuple, a vec
  or an `i64 ary` makes it one, and with nothing to take it, it settles by
  its contents, a vec when its elements are alike, a tuple when not.
  `{ … }` is `[ … ] map# !`, a map literal, undecided the same way.
  Built in march8 (2026-10-21): an array literal of known elements is
  held as them while compiling, settled by its contents, and a clause
  that wants another type takes its elements as it would each alone, so
  `( "a" b' )`, a string and a symbol, is two strings where a `string
  ary` is wanted. A symbol and a string are not alike: the symbol is text
  only once taken as a string. Not yet: its numbers stay undecided, so
  `( 1 2 )` is still i64s where an `f64 ary` is wanted.
- **`1 1 +`** matches a clause on two literals and folds to the literal 2 at
  compile time. A literal defaults to i64 only when a concrete
  representation is needed: stored, printed, or never given a type.

March7's checker already does a narrow version: it rewrites an integer
literal as a float when a float clause needs it.

### 2.11 Defining types

Decided 2026-10-22 (Thomas: "clearly we need to be able to define new
types"), from what a trap's kind needs (SYMBOLIC-FORTH.md 6.29). A type is
a structure, data made of types, and a type of its own is a structure that
holds its name. This replaces what this section first said, that Money was
a "newtype" never mixed with plain numbers. There are four cases:

- **An alias** is another name for a type, and the same type: `temp: [ f64
  ] ;`, so `< temp >` is `< f64 >` and mixes with it freely. Built in
  march8, since a word that leaves a type serves in a bracket. A word that
  makes a type of another, `maybe: [ nil or ] ;`, so `i64 maybe`, is a
  word as `ary` and `or` are, not a type of its own (Thomas: "it's not
  really a type of it's own").
- **A record** is a map of types, as a tuple is a sequence of them (2.5),
  each field's type in a bracket, since a bare `f64` in a map would be
  applied as a value (Thomas: "for the definition of a record, that make
  complete sense to me"):

  ```
  point: [ { x: < f64 > ; y: < f64 > ; } ] ;
  ```

  A record is known by its content, as a tuple is, so two with the same
  fields are one type, and a name given it by a definition is an alias.
  A record is also how a caller names a word's inputs (SYMBOLIC-FORTH.md
  6.31; Thomas, 2026-10-23: "Passing records -- of course"): `{ term: 72
  rate: 0.2 principal: 50000 } payment`, a misspelled label an error, and
  the record's type known where `payment` is applied, so the stage can
  open it into places at no cost.
- **A type of its own** has a representation and a name, the path where
  it is defined, which `ns#` gives as a symbol (SYMBOLIC-FORTH.md 6.29),
  so no other type is the same: Money over a decimal, Celsius over f64. It
  costs nothing at run time, its values being its representation's. It is
  a subset of its representation, as `bool` is of i64 (2.16), so it is
  accepted wherever the representation is, matched less well than a
  clause of its own (Thomas: "if decimal can't accept money, it seems
  rather restrictive -- we would be deifning money version for everything
  or forcing he user to alwasy say `dec !` even for the simplist of math
  functions"). Where its representation's word is applied it forgets
  itself, as a vec forgets its length and a tuple its positions (2.5), so
  `price sqrt` is a decimal, and a clause of its own keeps it where it
  should:

  ```
  +: < money money >< money > [ < dec dec > ! + money ! ] ;
  *: < money dec >< money > [ < dec dec > ! * money ! ] ;
  ```

  The rule is the one `bool` follows already, `true true +` being 2, an
  i64, with dispatch's, the most particular clause chosen, and no list of
  the words that keep a type (Thomas: "We need a rule, not a list we have
  to maintain, just making specialized defintions also helps make that
  choice when it differs from the rule"). Forgetting is the safe default,
  never wrong, only less exact, where keeping by default can be wrong: a
  bool of 2, or money times money as money. A literal takes its type from
  context, so `price 3 +` takes money's `+`, and a decimal made as the
  program runs is converted, `money !`. Proposed, not decided: a minor
  warning where one word forgets two types of their own, dollars and
  euros, or Celsius and Fahrenheit, which a units system will check
  properly (Thomas: "we will get to units system, eventually. I
  definitely want that").
- **A kind**, a trap's, is a type of its own in a tree, its name a word
  and a namespace both, holding the kinds beneath it: like an enum
  (Thomas: "The Kind reminds me a bit of enums"), but open, since a
  program may define a kind beneath any, and nested, since a kind takes
  the kinds beneath it. What it holds is a record, known while compiling
  (SYMBOLIC-FORTH.md 6.29):

  ```
  { error: < sym > ; message: < str > ; subspace: < sym trap map > ; }
  ```

The word that makes a type of its own from a representation and a name is
still to name; `trap` is that word for kinds. Not built: records, types of
their own and kinds. march8's `money`, exact cents in an i64, is a
placeholder for a type of its own over a precise number, which March does
not have yet (LAZY-NUMBERS.md).

### 2.12 If-free word bodies, as an experiment

Word bodies in the surface language have no `if`. Conditions live in
dispatch: families and contexts, with value patterns (a clause for 0) and
guards. If this proves troublesome, `if` comes back. System March keeps its
branches, and lowering turns dispatch on values into branches.

- Bodies become straight-line code, and branching becomes declarative, so
  the compiler can check that every case is covered and that no clauses
  tie.
- A value pattern such as `0` is a constant in a pattern, matched by a
  check at run time, or at compile time when the value is known. It is not
  a runtime value in a type. Written (2026-10-08): a value left in a bracket,
  `< 0 >`, matches an input of any type the literal becomes, compared in
  that type; `< 0 i64! >` gives it a type, as `0 i64!` does in code. A
  section's context ANDs with it slot by slot, so under `## < i64 >`, `< 0 >`
  is an i64 equal to 0. `fib` needs no helper words:

  ```
  [ < 0 > ] fib def.
  [ < 1 > ] fib def.
  [ < n > dup. 1 -. fib. swap. 2 -. fib. +. ] fib def.
  ```
- Loops tend to become recursion over clauses, which tail calls make cheap.
  `map`, `fold`, `each` and `times` remain: they are consumers, not
  conditions.

### 2.13 Writing types in the surface

Decided 2026-10-07. Stage 1 applies every word, so in ordinary code a type
name annotates: `19.99 money` lowers to `19.99 money .`. Building a type
needs a bracket, as SURFACE.md assigns `< >` to signatures and patterns:

- **Inside `< … >`, words build a type** instead of annotating: `i64` is
  the type itself, so `< 100 i64 vec >` is "array of 100 i64".
- **A bracket of n types annotates the top n values:** `x < 100 i64 vec >`
  annotates `x`, and `x y < i64 f64 >` both. A signature at the start of a
  word, `< f64 money >`, is then the annotation of its inputs, not a
  construct of its own. (Since 2026-10-12 a type's name gives the type
  everywhere, so a type is built in code as in a bracket, `x 100 i64 vec
  .`, and a bracket is a value except before a definition's body, `!`
  annotating by it: `x y < i64 f64 > !` (2.2).)

Rejected: marking each type name, as in `100 <i64> <vec>`. It leaves unclear
where a type expression starts (is the 100 part of it?), and makes a second
set of names for types.

### 2.14 The type stage is an elaborator

The type stage does not check code after it is compiled, as march7's
checker does; it decides the types and emits the code in one pass, so what
was checked is what runs. Type theory calls this an elaborator: checking
and translation together, as Pie, The Little Typer's language, is
implemented.

### 2.15 Contexts, definitions and guards

Decided 2026-10-08, with march7/docs/SURFACE.md ("Contexts").

- **Terms.** A **clause** is one definition: a name, a context and a body.
  A **family** is the clauses that share a name: `+`, with an i64 clause, an
  f64 clause and so on. A **domain** is the clauses that share a context:
  everything defined on `< i64 >`. A **section** is the text under one
  heading; sections far apart in a program can define in the same domain,
  so the domain is the meaning and the section only the layout. A domain is
  not a Haskell type class, which is an interface; the definitions of a
  domain are nearer a Haskell instance.

- **A definition is data:** a name, what it names, and `def` applied
  (Thomas, 2026-10-12): `inc: [ 1 + ] def`, the name first as a label, as
  SURFACE.md's `name : … ;` put it, and `]` and `def` closing it as `;` did.
  The bracket says what is defined: `[ ]` code, `< >` a type, `num: < i64
  f64 or > def`. Code computed where it stands, with no bracket to mark
  where it starts, is refused: a word that takes a symbol could take the
  name below it. Until then the name came last, `[ 1 + . ] inc def .`.
- **A heading sets a mode** for the definitions after it, as a stack
  machine's state: `## < i64 >< i64 >` makes them clauses for those types,
  until the next heading. A heading always starts a section, even an empty
  one, so consecutive headings do not combine; how to write OR between
  patterns is deferred.
- **Lowering removes the mode:** each definition carries its section's
  context as its own signature, first in its quoted body, so the lowered form
  has no hidden state, though the definitions of a section share one
  context.
- **`--` separates inputs from outputs** inside a bracket, whose reader reads
  its own words. Outputs are an obligation, checked; they never select a
  clause.
- **Contexts are compile-time (types) or run-time (guards).** A clause is
  chosen in two phases: by types at compile time, keeping the clauses whose
  types match; then by guards at run time, tested in the order the clauses
  were defined, an unguarded clause last. A guard looks at its values and does
  not consume them. Clauses chosen between at run time return the same type
  (2.9).
- **A guard must not write** (Thomas, 2026-10-08). It may read, the time or
  a random number, say, which makes it a choice at run time. The compiler
  works out each word's effects as it works out types, from the primitives
  it applies: for each domain, whether it reads and whether it writes, the
  effect rows of march5. Its run-time effect tokens are not needed while
  code runs in order.
- **An effect runs while compiling only when asked** (Thomas, 2026-10-09).
  `file-read` reads a file when the program runs, even when its path is
  known; `file-embed` reads it while compiling, so what follows folds on its
  text:
  macros by staging, as Crystal's `read_file` or Rust's `include_str!`, a
  YAML file read into values, even types, with no macro language. The text
  is data in the code, so the code's identity follows the file.
- **No match is no word.** When no clause matches, by types at compile time
  or by guards at run time, it is the same error as an undefined word. The
  name is a word's outermost context; types and guards narrow it.

### 2.16 Truth and nothing

Decided 2026-10-09 (Thomas: "Iverson brackets it is").

- **`bool` is 0 and 1,** Iverson's brackets: [P] is 1 when P holds and 0
  when it does not, as APL has it and Knuth writes it, and as FORTH's -1
  did not. So a bool is an i64 too, a subset: any word that takes an i64
  takes it as it is, matched a little less well than an i64, so a clause
  for `bool` is more specific. `mask +' reduce` counts and `mask xs ⋅`
  zeroes. That is no promotion: the value does not change, it is a member
  of the larger set. A bool and an i64 join to an i64, and a variable bound
  to one widens to the other.
- **A type of its own,** as Haskell's `Bool` is: `true` and `false` are
  its values, it shows as `true` and `false`, comparisons and predicates
  leave it, and guards take it, or a literal 0 or 1. A count is not a
  truth, so FORTH's flag that was a number is an error.
- **`not`, `and` and `or`** are 1 - x, the minimum and the maximum. On
  values from 0 to 1 those are also Zadeh's fuzzy connectives, so fuzzy
  truth, when it comes, is an extension: its own type, with `decide` to
  make a bool where a guard must choose (3.9). The product and
  Łukasiewicz logics, which differ only in `and` and `or`, would be types of
  their own, the families choosing each one's algebra. An integer mapped
  to 0 to 1 (graphics' UNORM) is a cheap exact representation for them.
- **No truthiness.** `nil`, a 0 that is a number and the empty string are
  not false; only a bool is tested. Absence is taken apart by clauses.
- **One bit of 64:** a lone bool takes a cell, as an i64 does; an array of
  them may be packed, a bit each, as APL implementations do, a choice of
  representation by use (march8/docs/MACHINE.md, "Not yet").
- **Nothing is `nil`,** a type with one value (3.6). Not `none`, which in
  set-theoretic types is the empty set, the type with no values; not
  `null`, which is a member of every type, so that anything might be
  missing, where `nil` is a member only of its own and of a union that
  names it; and no `unit`, since a word that returns nothing leaves
  nothing.

## 3. Open

1. **The stages:** how many, and what each lowers.
2. **Representation:** how type values are stored (as content-addressed
   March data, so that type equality is identity?), and how compiled code
   and the display reach them.
3. **Records** in detail. A record's type is a map of types (2.11,
   2026-10-22); open still are a record value's surface and how a field is
   read, by `at` or by a path, whether a record is a namespace, a value of
   it defining what its type declares, and how a record is laid out at run
   time; and the surface syntax for tuples, value patterns and guards.
4. **Literal type names.** Settled (Thomas, 2026-10-19): `int#`, `dec#`,
   `ratio#`, `seq#` and `map#`, as written, and `str#` for text, which
   strings' encodings make undecided too (SYMBOLIC-FORTH.md 6.15): `str#`
   settles to `string`, UTF-8, as `int#` settles to i64, and `ascii` is a
   refinement of `string`, as `bool` is of i64 (2.16). A string's structure,
   flat or a tree, is a representation chosen by use, not a type (TEXT.md
   1). The placeholder `lit#` goes;
   should a word need to match any literal, that is a class, as `num` is.
5. **Precise numbers** for money: a decimal type, or rationals.
6. **Unions, set-theoretic** (proposed 2026-10-09, after Elixir's type
   system: Castagna, Duboc and Valim, "The Design Principles of the Elixir
   Type System", 2023). A type is a set of values; `or`, `and` and `not`
   are union, intersection and complement.
   - **Families are intersections already.** Elixir types a function of
     several clauses as the intersection of their arrows, which is a
     family. `atom` is a complement, everything that is not a container,
     and `number` would be a union: classes are sets, not a notion of
     their own.
   - **`or` has one meaning, union.** Whether a tag exists depends on what
     the stage knows about a value, not on the pattern. Known to be an i64,
     the clause is specialized and there is no tag; known only to be in
     `i64 or nil`, the code branches on the tag. A tag is materialized late,
     as everything else is: when a union-typed value is stored, or passes
     through code not specialized for its type.
   - **Clauses take unions apart,** with no `match`. Clauses on `< i64 >` and
     `< nil >` applied to an `i64 or nil` compile to a branch on the tag,
     each arm specialized for its type (splitting). A clause that does not
     match narrows the rest to `T and not i64`, so the stage can report a
     member no clause handles.
   - **Structural, not declared.** A tag comes from the member type's
     identity, so `i64 or nil` is one type in any order, anywhere, with no
     declaration. Names are aliases: `a maybe` for `a or nil`. Alternatives
     with one representation, Celsius and Fahrenheit, are told apart as
     distinct types (roles), not by position.
   - **`nil`, not `none`.** In set-theoretic types `none` is the empty set,
     the type with no values, so `i64 or none` is `i64`. The value that
     means "nothing here" needs a name and a type of its own; `nil` is
     Elixir's.
   - **An informative warning** (Thomas, 2026-10-09): a slot that may hold
     more than one concrete type at run time costs a branch wherever it is
     used, and a copy of the code for each type. Informative is for what
     programs can and often do, with a clear downside, that could be avoided
     by writing it another way (march8/docs/MACHINE.md, "Errors").
     Proposed: a signature that names the union (`-- i64 or nil`) states the
     intent and silences it. Many instances of one family, chosen at compile
     time, cost code size, and are informative too.
   - **Built** (2026-10-09, march8): unions in patterns, `or` and `num`,
     and `def` naming a type. A union matches as its best member does, a
     little less well, so a clause for one type beats one for a union,
     which beats `atom`. Then values of a union type: `nil`, `get`, unions
     made where run-time choices differ, families split by tag, a union
     value as one cell of tag and value, and the informative warning
     (march8/docs/MACHINE.md, "Unions").
   - **Not taken from Elixir:** gradual typing (`dynamic()`), which serves
     an existing dynamic language on a VM that tags every value; and full
     semantic subtyping, with complements of function types. March starts
     with unions and complements over base types and constructors, which
     are sets of type numbers.
7. **Recursion by syntax alone?** Thomas suspects a simpler way than ghosts,
   by syntactic analysis. Part of it already is syntactic: a clause that
   does not call its word, directly or through a cycle of words, is a base
   case, so base clauses can be typed first and the others checked against
   them.
8. **Ahead of time, with run-time staging later** (2026-10-09). Thomas
   wants compiling ahead of time, with the one part of a JIT that helps:
   pruning branches, and compiling for types once they are decided. The
   stage does both ahead of time with what is known then: known values
   fold, a guard on a known value is decided, each word gets an instance
   for each set of types, and unions split (6). Knowledge that exists only
   at run time could come later in two ways, neither speculative: compiling
   again with a profile of the branches and types that occurred, keyed by
   CID; or the stage as a word a program applies to a quotation and the
   values now known, its code cached by CID so that a later run loads it,
   as Julia compiles a method for its concrete argument types on the first
   call and caches the result. A JIT that prunes what it has not seen taken
   needs deoptimization to recover when wrong; staging prunes only what is
   known, so nothing is undone.
9. **Fuzzy truth** (2026-10-09; Thomas is reading on fuzzy logic first).
   - **`fbool`,** the numbers from 0 to 1, as f64 or UNORM, with `not`,
     `and` and `or` as for `bool` (2.16), which embeds in it. Fuzzy truths
     come mostly from membership functions, words that say how far a
     measurement belongs to a fuzzy set: `[ < f64 >< fbool. > 160.0 -. 30.0
     ÷. 0.0 1.0 clamp. ] tall def.`, 175 cm being 0.5 tall.
   - **Hedges,** Zadeh's modifiers: "very" is x², "somewhat" √x.
   - **`decide`** (Thomas's word) makes a bool from a fuzzy truth, for a
     guard: an α-cut, truth at least α, which underneath is the comparison
     `x α ge?`.
   - **α as an ambient setting,** not written at each decision, so that
     changing it changes every decision under it (Thomas). Lexical: a
     section's heading sets α for its definitions, decided at compile time,
     so decisions fold. Or dynamic: α in the global store, a quotation run
     with it changed, as Racket's `parameterize`, and nested contexts
     setting theirs relative to the enclosing one ("0.9 of the outer α").
     Reading the store is a read effect, which a guard may have. Which, or
     both, is open.
   - **Fuzzy families,** perhaps the most March-like: rather than deciding
     to choose one clause, every clause that matches applies to its degree
     and the results are blended, weighted by truth, as fuzzy control does
     (Mamdani; Takagi and Sugeno). No cliff at α.
   - **To read:** Zadeh, "Fuzzy Sets" (1965); α-cuts and the decomposition
     theorem, by which a fuzzy set is the stack of all its α-cuts, so
     sweeping α moves through a family of crisp sets; possibility theory
     (Zadeh 1978; Dubois and Prade), fuzzy logic's modal logic, whose
     possibility and necessity measures are as ◇ and □.
10. **`next`, a clause applying the one it overrides** (2026-10-09). Families
    are open, so a module can add a more specific clause; with `next` that
    clause could wrap the general one instead of replacing it, as a money
    module's `show` adding the currency to what the general `show` writes.
    It is OOP's `super` (Thomas), chosen by the family's order of specificity
    rather than a class hierarchy, and over all the inputs, not only the
    first: CLOS's `call-next-method`, Dylan's `next-method`, AspectJ's
    `proceed` around advice, which grew from CLOS's method combination.
    Open: the name, `next` or `super`; and which clause is next when
    guards decide at run time, the next by score that matches, its guards
    tested in turn, as a choice is now.
11. **A range type** (2026-10-09). `thru` builds an array; a range could be a
    value of its own instead, lazy: `length`, `at`, `each` and `sum` (in
    closed form) work on it with no array built, and it becomes one only
    where an array is wanted, as a tuple forgets its positions; so
    `n range each` is a counted loop (march8/docs/MACHINE.md, "Not yet",
    deferred terms). And, types being values, a range could be a type: a
    subrange, as Pascal's `0..100` and Ada's `range 0 .. 100`, so `bool` is
    the integers 0 through 1 and `fbool` the reals from 0 to 1 (3.9). A
    literal, `1..5` or the like, is open; SYMBOL's interval notation,
    `[1,2)`, is not to be had, `[` and `(` being March's brackets.
12. **Namespaces** (Thomas, 2026-10-10). Words gather in namespaces,
    `io.file.read`, `io.stdin.read`, `io.file.embed`, rather than a flat
    dictionary's `file-read`. A dot inside a name divides the path, and the
    name applies the word it names: `"data.csv" io.file.read`.
    The dot chain, `a.b.` for `a. b.`, was tried and left for this, the two
    being unable to share the dot (Thomas, 2026-10-10); the reader keeps a
    dotted name whole, and `def` refuses one until namespaces come. `.`
    divides, as headings already nest namespaces into dotted names,
    `math.trig` (march7/docs/SURFACE.md); `:`, the other candidate, ends a
    label (2.2). Importing
    says how much of the prefix a module needs, `import : ( io.file.* )` or the like, by words of
    a namespace's own that declare what it imports and what it exports, as
    march2 had (`<` imports, `>` exports, a namespace pushed on the stack:
    doc/lineage/march2.md). Collisions between imports are the programmer's
    to settle, by a longer prefix, as in Elixir. Symbols stay one global
    namespace (2.2). Then `file-read` and its kin move in. Built in march8
    (2026-10-17), the core's first namespaces: `io.stdin.read`,
    `io.file.read` and `io.file.embed`, and `case.lower`, `case.upper`,
    `case.snake`, `case.kebab`, `case.camel` and `case.pascal`, imported,
    all or one alone, as `lower` and the rest (Thomas). They
    replace a `read` and a `case` chosen by symbols, `"data.csv" File
    read` and `"Hello World" Snake case`: once a capital marked nothing
    (SYMBOLIC-FORTH.md 6.27), each symbol needed a word of the core's, for
    one family only (Thomas: "Do you think `"Hello World" Snake case` is
    really better than say `"Hello World" case.snake`?"). Settled for the
    lowest form (2026-10-14): namespaces are maps all the way down, and `.`
    reads a dotted name's path (SYMBOLIC-FORTH.md 6.16); importing and
    exporting are a namespace's keys `import#` and `export#` (6.17).
13. **Clauses of different arity** (Thomas, 2026-10-12; built
    2026-10-13). A family's clauses need not take as many values as each
    other: `5 Down' step` beside `5 Up' step`, `< i64 Down' >` taking two
    and `< Up' >` one (first `"data.csv" File read` beside `Stdin read`,
    reading by namespace since 3.12). When clauses of
    different lengths match, **the shortest wins**, the most specific among
    those: a word reaches as little of the stack as it can, and a longer
    clause never changes what a call to a shorter one did (adding a shorter
    clause can, which an informative warning says, as it says of a longer
    clause added under a shorter one with no guards, which is chosen
    wherever both match). Guarded clauses are tried shortest first. What
    constrains it is a choice made at run time, by guards or a union's
    tag: alternatives must agree in net effect, and the deeper slots merge
    as a union. march8/docs/MACHINE.md, "Clauses of different lengths".
14. **Types with holes** (2026-10-12). A bracket with type variables, named,
    is a type with holes: `opt: < a nil or > def`, `grid: < n m a vec vec
    > def`. Unapplied in a signature the holes match anything, `< opt
    >` being the class `a nil or`; applied to types they are filled, in the
    order they first appear, the order the expression would take them from
    a stack: `i64 opt`, `3 4 f64 grid`. Each use has fresh holes. That is
    Haskell's `type Opt a = Maybe a`, a substitution; a type computed, by
    arithmetic on a size or by cases, is code, a quotation that leaves a
    type, as Zig's comptime functions and Haskell's type families are. Not
    built: today `def` names such a bracket as a class.
15. **Facts and rules** (Thomas, 2026-10-12). Symbols and families are most
    of a logic language: `Socrates Is Human fact`, `x Is Mortal if x Is
    Human rule`, `Socrates Is Mortal ask`, triples, RDF's shape, each word
    of one arity, one lowercase letter a variable. Two ways to run it: rules
    as clauses of the verb's family, chosen while compiling, so a question
    on known symbols folds to its answer (and with truth from 0 to 1, `and`
    and `or` being min and max, the same engine is fuzzy, 3.9); or facts as
    data, an array of triples, rules applied until nothing new comes,
    Datalog, which gives every answer, `x Is Mortal all`. The first needs a
    word that makes a clause from symbols; the second known arrays in the
    stage. Both have symbols at run time and recursion on them (16).
16. **Recursion on symbols.** A family that applies itself is compiled as a
    word of its own, an instance, whose inputs are values at run time. A
    symbol is one now (2026-10-13), its name as an interned string, so a
    family recurses on symbols as on numbers, testing them as it runs:
    march8/examples/syllogism.march. Open: with its inputs known, a
    recursion could unfold while compiling, under the depth limit, so that
    `Socrates Mortal is` folds to true, as `10 fact` might to 3628800;
    where it stops, when a known recursion is too deep or too wide to
    unfold, is the question.

## 4. A way to build it (proposal)

Superseded on 2026-10-08: the compiler is built in Rust, in march8, and
System March is frozen. The steps below still describe the order of the
work.

The bootstrap does not need to change. System March stays FORTH, and
compiles itself with today's checker; the staged pipeline is written in
System March beside it, and compiles the surface language. So the
self-rebuild keeps working throughout, and the system's own checker can
adopt the new types later, or never.

1. **Type values.** Types as March data: the scalars, `ary`, `vec`, tuples
   and records, built by words and identified by content. A table gives
   each type a small number for the checker's stacks.
2. **Judgments.** A compile-stage evaluator, in March, over the explicit
   form (`1 i64 . + .`): a stack of judgments, `.` applying words and
   types, and plain bytecode as its residue.
3. **Families as clause declarations,** with signature patterns, type
   variables and the most-specific rule; lifting as a clause; literal types
   and per-type conversions.
4. **Instances, ghosts and recursion.**
5. **Tuples and records; value patterns and guards** for if-free bodies.
6. **Kinds at run time for identity:** where a container is built, the
   compiler tells the machine its elements' kinds, so identities hash
   contents rather than handles.
7. **Unions** (3.6), when needed.

Each step would come with tests and a note in march7/docs, as the checker's
slices did.
