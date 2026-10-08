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
1 i64 . 1 i64 . + .
```

- **`.` applies.** Everything is a value or a symbol until `.` applies it.
  `.` is the only special word, and types and operations share one
  namespace. (`.` is free: the surface language prints with `print`.)
- **A dot at the end of a word applies it** (Thomas, 2026-10-08): `sq.` is
  `sq .`, so `1 i64. 1 i64. +.` reads more easily. By that convention no
  name ends in a dot, and the reader enforces it. Each dot applies once:
  `x 100 i64 vec..` builds a type and annotates `x` with it.
- **A type is a value until it is applied, and applying a type annotates.**
  - `1 i64 .` annotates 1 as an i64.
  - `100 i64 vec .` builds a type: `i64` unapplied is a type value, and
    `vec` makes "array of 100 i64" from it.
  - `x 100 i64 vec . .` builds that type and annotates `x` with it.
- **Compile stages evaluate the type part and erase it.** When the type
  stage meets `+ .` with two i64s, it picks integer addition and leaves a
  plain call behind. After lowering, `.` is an ordinary call instruction
  and the types are gone, so cells stay bare and fast.
- **Programmers will not normally write this form,** as most do not write
  System March. How many stages there are is still to be discovered.

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
- **Fixed keys make a record:** `{ "port" 8080 "host" "x" }`, each field
  typed.
- **Genuinely variable mixtures need sums,** such as "i64 or string". Sums
  are explicit (declared), carry a runtime tag (the alternative's index,
  like a Rust enum's discriminant), and a family applied to one compiles to
  a branch on the tag. Inside a container the tags are stored per element,
  which also gives stable identities.
- **Sums come later,** at the first real need, most likely JSON or errors.
  Until then, failure and absence use traps, sentinels, or a value with a
  flag, as `text>integer` does.
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
  < i64 i64 -- i64 >    integer add
  < f64 f64 -- f64 >    float add
  < a ary a -- a ary >  lift: add to each element ("a" is a type variable)
  ```

  Lifting stops being a special case of the checker and becomes a clause.
- **Families are open:** other code, such as a money module, may add
  clauses, scoped by namespaces and modules.
- **The most specific clause wins; a tie is an error** for now. ("The
  clause defined last" was considered and left open.)
- **Every clause has the same stack effect,** as SURFACE.md decided, but
  clauses chosen by type may differ in their result types: `+` gives an
  integer for integers and a float for floats.

### 2.9 Choices at compile time and at run time

- A choice made at compile time, by type, may differ in result type.
- A choice made at run time, by value (`if`, a value pattern, a guard),
  must give the same type in every alternative, or a declared sum.

### 2.10 Literals

Literals take their type from context, with no implicit promotion
(SURFACE.md). At a compile stage a literal is undecided: its type is a
**literal type** (`lit#` is a placeholder name). Thomas: it is "just another
type signature `< f64 lit# >`", so literal handling is clauses, open like
any family.

- **Each type declares once how a literal becomes it:** f64 converts it to
  a float, money converts it exactly. One general rule covers every family:
  a clause that wants a T and finds a literal converts it with T's
  conversion. A specific clause can still override.
- **Two literal types, integer and decimal.** An integer literal can become
  i64, f64 or money; a decimal literal can become f64 or money, not i64.
  A decimal literal keeps its exact digits until its type is known, so
  `price 1.10 +` adds exactly one dollar ten, never via a binary float.
- **`1 1 +`** matches a clause on two literals and folds to the literal 2 at
  compile time. A literal defaults to i64 only when a concrete
  representation is needed: stored, printed, or never given a type.

March7's checker already does a narrow version: it rewrites an integer
literal as a float when a float clause needs it.

### 2.11 Roles

Money is a distinct type with the representation of a precise number type
(a "newtype"): it cannot be mixed with plain numbers by accident, and costs
nothing at run time. March has no precise number type yet (see
LAZY-NUMBERS.md).

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
  a runtime value in a type.
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
  construct of its own.
- In the explicit form a bracket builds its type and applies it:
  `x < 100 i64 vec >` lowers to `x 100 i64 vec . .`.

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

- **A definition is data in the lowered form:** a quoted body, a name, and
  `def` applied: `[ 1 + . ] inc def .`. `name : … ;` is the surface's
  spelling of it.
- **A heading sets a mode** for the definitions after it, as a stack
  machine's state: `## < i64 -- i64 >` makes them clauses for those types,
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
- **No match is no word.** When no clause matches, by types at compile time
  or by guards at run time, it is the same error as an undefined word. The
  name is a word's outermost context; types and guards narrow it.

## 3. Open

1. **The stages:** how many, and what each lowers.
2. **Representation:** how type values are stored (as content-addressed
   March data, so that type equality is identity?), and how compiled code
   and the display reach them.
3. **Records** in detail, and the surface syntax for tuples, records, value
   patterns and guards.
4. **Literal type names** (`lit#`, and the integer and decimal types).
5. **Precise numbers** for money: a decimal type, or rationals.
6. **Declared sums,** when the first need comes.
7. **Recursion by syntax alone?** Thomas suspects a simpler way than ghosts,
   by syntactic analysis. Part of it already is syntactic: a clause that
   does not call its word, directly or through a cycle of words, is a base
   case, so base clauses can be typed first and the others checked against
   them.

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
7. **Sums,** when needed.

Each step would come with tests and a note in march7/docs, as the checker's
slices did.
