# Surface March: the language people write

Status: design record, 2026-10-03, decided with Thomas in conversation. Apart
from quotations and floats, none of this is built yet. Each section says whether
it is decided, a maybe, or open.

Example programs written against this design, and what they found, are in
[surface/](surface/README.md).

## Two tracks, and lowering

March has two tracks:

- **Surface March** is the language people write: name-first definitions,
  headings, contexts, families such as `+`, and typed conversion with `!`.
- **System March** is FORTH: prefix `: name … ;`, raw memory `!` and `@`,
  FORTH's compile-time brackets (exported as `[[ ]]`, to be renamed; see Open
  questions), and explicit machine words (`u+`, `f+`). A module opts into it
  explicitly; it is the "unsafe" track. `seed/system.march` is written in it.

Surface March **lowers** to System March, which compiles to canonical code
(docs/FIRST-SLICE.md). Erlang works the same way: records, comprehensions and
guards are gone by the time its compiler reaches Core Erlang, so later passes
stay small. March already lowers in two places:

- Quotations consumed by `if`, `while` and `times` become plain branches
  (docs/QUOTATIONS.md).
- The checker reads canonical bytes, not source, so it is exact whatever the
  surface looks like (docs/CHECKER.md).

Three rules follow:

1. **Every surface form lowers to System March that could be written by
   hand.** If the surface could do something the system track cannot express,
   the system track would stop being a language.
2. **Source positions stay out of canonical code.** Positions in the bytes
   would make identical words hash differently, so they live in a side table
   keyed by CID and instruction index. Errors are reported in surface terms
   from that table.
3. **Lowering is visible.** A `lower` word (proposed) shows the System March
   form of a surface definition.

The bottom of `system.march` is compiled by the frozen generation-zero listing,
which has no checker, so it names its operations explicitly. Everything above
it can use surface features.

## Definitions: `name : body ;`

Decided. One binding form covers everything that has a name:

```
sqr        : dup * ;                   -- a word
+          : … ;                       -- symbolic names work the same way
math       : { sqr : dup * ; } ;       -- a namespace
config     : { port : 8080 ; host : "example.org" } ;   -- data
price      : Money ;                   -- a role: a word that pushes a type
adjustment : price -> price ;          -- a named signature
positive?  : 0 gt? ;                   -- a guard, usable on = lines
```

- **The reader looks one token ahead.** Before resolving a word, it saves the
  input offset, reads the next word, and restores the offset. If the next word
  is `:`, the current word is the name being defined. Restoring the offset means
  words that parse their own input (`'`, `s"`, `--`) see it unchanged.
- **Prefix `:` is not surface syntax.** With both forms allowed,
  `init : main run ;` could mean "call `init`, then define `main`" or "define
  `init`". So surface code has only the name-first form, and the system track
  keeps FORTH's prefix form. The same split applies to `!` and `@`.
- **`}` ends the last entry,** so the `;` before it is optional.
- **Entries are words.** `port : 8080 ;` is a word that pushes 8080. Words are
  pure and content-addressed, so an entry with no inputs can be evaluated once
  and stored as its value. Code and data are one kind of store entry, as the
  dictionary and the store are one (decided 2026-09-26).

**Rejected: a colon glued to the name** (`sqr: dup * ;`). It needs no
lookahead, but symbolic names read badly (`,:`, `+:`), `::` is ambiguous, and
no name could end in a colon. Also rejected: prefix definitions inside maps
(`{ : port 8080 ; }`), which wrap every entry in delimiters, s-expression style.

## Namespaces and literate files

Decided ("lean in"). Namespaces are written as Markdown headings:

```
# shape
= int ;
= f64 ;
double : dup + ;         -- two clauses: int and f64
square : dup * ;
= str ;
double : dup concat ;    -- a third clause of double

# math
sqr : dup * ;            -- the heading reset contexts: unconstrained
## trig                  -- math.trig
= f64 ;
sin : … ;
```

- **Headings are words.** `#`, `##` and `###` read the next token as a name,
  so March stays token-based.
- **A heading of depth n closes every open section at depth n or deeper,** as
  in Markdown. `## trig` under `# math` is `math.trig`. A section ends at the
  next heading or the end of the file.
- **Headings are the literate spelling of `name : { … } ;`.** Both produce
  identical store entries, so a file can use either, section by section.
- **Every heading resets the context.**
- **Two kinds of file:**
  - A plain `.march` file: headings are words and prose goes in `--` comments.
  - A literate `.md` file: the loader feeds headings and fenced `march` code
    blocks to the interpreter and skips the prose. The document's outline is
    the module's structure, and the file renders as documentation.

## Contexts

Type signatures are one kind of context, a compile-time one. Contexts are
sticky `=` lines placed before definitions. The marker was `?` until
2026-10-03, when Thomas chose `=`: contexts scope the definitions after them,
like headings, and AsciiDoc writes headings with `=`. The rules are decided; how a line
describes its inputs (the type stack below) is proposed, 2026-10-03, pending a
fine-tooth-comb review.

| Rule | Meaning |
|---|---|
| An `=` line is AND | `= int positive? ;` is a type and a guard; every guard must hold |
| Consecutive `=` lines are OR | Each alternative compiles to its own clause of the family, and the body is checked for each |
| Sticky | The definitions after a run of `=` lines belong to it; the next `=` after a definition starts a new context |
| Reset | `= ;` clears the context; each module and each heading starts with none |
| Nesting is depth | `==` nests inside the current `=`, `===` inside that. A definition's context is the AND of levels 1 to k. A new line at depth n replaces that level and clears deeper ones; `== ;` resets only level 2 |
| Inputs only | Contexts select a clause by its inputs. Outputs may appear as an obligation (`= int -> int ;`) but never select. Return-type polymorphism is rejected |

No context at all means the effect and types are inferred. A context with OR
lines is static duck typing over a declared set.

### The type stack

Types are first-class, so the words on an `=` line run at compile time, and
each type pushes an input slot. The line builds the inputs from the bottom up:

```
= int str ;                 -- two inputs: an int under a str
= int positive? str ;       -- a guard tests the slots on top where it is written
= int str positive? ;       -- error: positive? has no clause for str
= int int lt? ;             -- a guard over two slots: first < second
= int positive? even? ;     -- two guards on the int: AND
= positive? ;               -- the slot's type comes from what the guard accepts
```

- **Guards look, they don't consume.** That is the one way an `=` line differs
  from ordinary code, and it is what keeps a line an AND. Read as plain code,
  `positive? even?` would apply `even?` to a flag.
- **A misplaced guard is a compile error,** because it is resolved against
  the slot it sees.
- **The compiler tells types from guards by what they leave:** a type or a
  flag.
- **Names for contexts are ordinary bindings.** A role is a word that pushes a
  type, `price : Money ;`, used as `= price ;`. A guard is any word that
  returns a flag. There is no separate `context` definer.
- **Outputs follow `->`,** which marks where the inputs end: `= int -> int ;`,
  `adjustment : price -> price ;`. FORTH's `--` cannot serve, since it starts a
  comment. With `->`, the `< >` brackets may not be needed for types at all.

### Lowering

An `=` line produces no code; it sets the compiler's current context. Each
definition under it lowers to one clause word per alternative, plus an entry in
the family's clause table:

```
= int ;
= f64 ;
double : dup + ;
```

lowers to roughly this (the `|` names are illustrative; the table holds clause
CIDs, so clause words need no names):

```
: double|int  dup + ;     -- compiled for an int: + is the checked integer +
: double|f64  dup f+ ;    -- compiled for an f64: + becomes f+
' double|int int clause double
' double|f64 f64 clause double
```

- **Monomorphization happens here.** One source body becomes one clause per
  alternative, each checked and compiled with its own types.
- **Calls resolve at compile time.** `3 double` compiles a direct call to the
  int clause's CID, with no dispatch at run time.
- **Guards add run-time code.** A type's entry becomes a guard chain, tested in
  declaration order, ending at the unguarded clause. Guards get copies of their
  slots: for `= int positive? str ;` the copy is `over positive?`.
- **Types resolve only at compile time, for now.** Cells carry no type
  (docs/NUMBERS.md), so a value of unknown type cannot be tested at run time;
  that is a compile error until values carry tags. Guards test values, so they
  work at run time today.
- **Identity:** a caller embeds the CID of the clause it resolved to. Adding an
  f64 clause changes no int caller; adding a guarded int clause changes int
  callers, whose behaviour did change.

## Names, strings and printing

Provisional choices, 2026-10-03, so examples are consistent:

- **Qualified names are dotted:** `math.sqr` is one token. Numbers such as
  `1.5` are recognized before words, so they do not clash.
- **Printing is `print`.** FORTH's `.` is retired from the surface language.
- **Strings are `"…"`,** on both tracks. The reader recognizes a token
  starting with `"` by its first character, as it does numbers, and reads raw
  input to the closing `"`. Only `seed/system.march` keeps FORTH's `s" text"`,
  because the frozen generation 0 reads it. If a track ever cannot have
  `"…"`, the fallback is `" text "` with the spaces enforced.
- **Every clause of a family has the same stack effect,** so the counting
  checker knows a family's effect without types. The examples will show
  whether that is too strict.

## Brackets

| Brackets | Meaning | Status |
|---|---|---|
| `[ ]` | Quotations, completed by their consumer | Built |
| `( )` | Arrays and tuples: an array when it can be, a tuple when it must | Decided 2026-09-25 |
| `{ }` | Maps: namespaces and data | Decided |
| `< >` | Types, type patterns and signatures | Decided; may be unnecessary with the type stack and `->` |

FORTH's compile-time brackets, which run words while a definition is being
compiled, are System March only. Surface code computes constants with pure
entries instead: `twelve : 3 4 * ;` can be evaluated once.

## Types and numbers

Decided:

- **No implicit promotion between values.** Literals take their type from
  context, so `x 1 +` works for a float `x`. Conversions are explicit.
- **`!` converts the top value to the type of the value below:** `5.0 5 !`
  leaves `5.0 5.0`. Raw memory store and fetch move to the system track, as
  `!!`/`@@` or `store`/`fetch`, renamed together with module scoping because
  `!` is one of the words the bootstrap borrows.
- **`+` is a family,** resolved by type at compile time. The `u`-words and
  `f`-words become its clause bodies on the system track.
- **Roles are aliases,** erased after checking. Distinct types are declared on
  purpose. Units come later.
- **Types are first-class,** at compile time first.

Numbers are described in docs/NUMBERS.md.

## Locals: a maybe

Not decided. If March gets named locals, the spelling would be `4 :x`: the
value comes first and the name after, in stack order, as in FORTH's
`8080 constant port`. `x` then pushes the value.

If adopted:

- **Storage:** slots in the word's scratch-stack frame, which is discarded on
  return, exactly a local's lifetime. The checker already tracks the scratch
  depth. It needs one new primitive, a read at an index from the frame's base;
  scratch peek (35) reads only the top.
- **Slips:** `4 : x` is an error in surface code, since a number cannot be a
  name being defined.
- **Quotations:** an inlined quotation shares the word's frame and sees its
  locals. A materialized one is its own word, so the compiler would capture
  the local's value when sealing it, or require such quotations to be inlined.

## Pattern matching by inverses: a proposal

From Daniel Ehrenberg, "Pattern matching in concatenative programming
languages" (MICS 2009), on Factor's `undo` and `case`. Running code backwards
takes apart what it built: undoing a constructor leaves its fields, undoing a
literal checks equality, and undoing `dup` checks that the top two values are
equal and keeps one. Inverses compose, `[ f g ]` undone is `g` undone then `f`
undone, so they are derived from code. Conditionals and recursion have none.

Proposed for March:

- **An `=` line describes the inputs by a computation that could have produced
  them.** `= int str ;` is a type check, `= 0 ;` checks for zero, and `= dup ;`
  checks for two equal values. The tests run on copies, so contexts only
  select, and the body sees its inputs unchanged.
- **`match` takes values apart inside a body.** It is completed by its
  consumer, like `if`, over pairs of quotations, a pattern and an action:
  ```
  sum : [ nil ] [ 0 ] [ cons ] [ sum + ] match ;
  ```
  Each pattern's inverse is inlined as tests that branch to the next pair on
  failure, so no exceptions are needed.
- **Inverses are derived from canonical code** and remembered for the session,
  as effects are.
- **Constructor tests at run time need tagged values,** like dispatch on
  type. When the type is known at compile time they cost nothing.

The paper's showcase is unit conversion: `1 inches [ cm ] undo` converts with
inverses derived from the unit definitions, and converting inches to seconds
fails to match.

## Planned constructs

From the lineage research (doc/lineage/CONTROL-FLOW-COMPREHENSIONS.md §5), in
order: families resolved statically with runtime dispatch only when needed;
one lifting rule for arrays; comprehensions with a runtime depth marker and an
explicit `_` pull; loop sugar. Their surface syntax is not yet designed.

## Open questions

- **Constants:** whether `port : 8080 ;` is a word that pushes or a stored
  value. For pure entries with no inputs the two converge. This is load-bearing:
  roles such as `price : Money ;` work only if the compiler evaluates pure
  entries with no inputs at compile time.
- **Map values:** whether `config` used as a word pushes the map, and whether
  `config.port` reaches inside it.
- **Lookup:** local names, then enclosing namespaces, then imports? From
  outside, dotted names (`math.sqr`) are the provisional choice.
- **Computed and non-name keys,** perhaps a pair word such as `"k" 10 =>`.
- **`;` between entries:** recommended to stay required, for readability and
  clearer errors, though the lookahead could do without it.
- **OR at outer `=` levels:** expand into one clause per combination, or
  allow OR only at the innermost level so expansion stays predictable.
- **Checked by default** for user code.
- **Guard order:** declaration order within a type, then the unguarded clause,
  is assumed. Guards cannot be compared at compile time, so ambiguity cannot be
  detected for them.
- **Type variables and composite types.** Proposed, 2026-10-03: a type is
  written like the value it describes, as a quotation type already is.
  Variables are marked by a suffix (Thomas's spelling): `a'` is one value of
  some type, `a*` any number of values of any types (a row variable, "the
  rest of the stack"), and `t*` after a type is any number of that type.
  ```
  = a' b' -> b' a' ;                    swap : … ;
  = a* [ a* -> b* ] -> b* ;             call : … ;
  = a* x' [ a* -> b* ] -> b* x' ;       dip  : … ;
  = ( a'* ) [ a' -> b' ] -> ( b'* ) ;   map  : … ;
  ```
  A word's signature is an `=` line before it. `name : a' -> a' ;` would
  instead define `name` as a signature.
  - `[ a* -> b* ]` is a quotation type, `( int* )` an array of ints,
    `( int str )` a tuple: the array-or-tuple rule for `( )` literals shows up
    in the types. `( a* )` is a sequence with any contents.
  - `{ port : int ; host : str }` is a map type. A namespace's interface is
    then the map type of its entries' signatures, which fits march5's interface
    identity (the hash of the exported surface).
  - Variables are needed mainly for higher-order signatures (checker slice 3).
    Names stay in signatures, as FORTH's stack comments always had them, and
    never in bodies.
  - The suffix avoids the one-space hazard of ML's `'a` against FORTH's tick
    (`' a`), and a backtick would break Markdown inline code. The rule applies
    only in signatures and on `=` lines, so `*`, `u*` and `f*` cannot appear
    there; nothing else in `system.march` ends in `*` or `'`.
- **Open families and coherence:** whether another module can add a clause to
  `+`. If it can, which clauses are visible decides what a call resolves to.
- **Recursion between clauses:** code identities cannot form cycles, so
  clauses that call each other need group identities or one combined
  definition (lineage research §5.8).
- **Contexts across prose:** in a literate file, whether a context carries
  from one fenced block to the next, past prose a reader may not connect it to.
- **Self-delimiting brackets** (proposed): `[ ] ( ) { }` end a word and are
  tokens on their own, so `[[` reads as `[ [` and `{port : 8080}` needs no
  inner spaces. No word name in `system.march` contains these characters
  except the brackets themselves. `< >` cannot work this way, because `>` is
  part of names such as `>r` and `i>f`, so nested types close with `> >`. The
  compile-time brackets then need a new name, perhaps Factor's `<< >>`, which
  means parse-time evaluation there; March spells shifts `lshift` and
  `rshift`, so `<<` and `>>` are free. Not `[: ;]`, which is the standard
  FORTH spelling for quotations (Forth-2012).
