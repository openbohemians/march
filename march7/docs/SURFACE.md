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
sqr        : = * ;                     -- a word (= is dup)
+          : … ;                       -- symbolic names work the same way
math       : { sqr : = * ; } ;         -- a namespace
config     : { port : 8080 ; host : "example.org" } ;   -- data
price      : Money ;                   -- a role: a word that pushes a type
adjustment : < price -> price > ;      -- a named signature
positive?  : 0 gt? ;                   -- a guard, usable in patterns
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

## Headings: namespaces and contexts

Namespaces as headings were decided early ("lean in"). Contexts became headings
on 2026-10-03, replacing `?` and then `=` lines, to try: a name after the
marks opens a namespace, and `< … >` patterns open a context.

```
# shape

## < i64 > < f64 >
double : = + ;           -- two clauses: i64 and f64
square : = * ;

## < str >
double : = concat ;      -- a third clause of double

# math
sqr : = * ;              -- no context
## trig                  -- math.trig
### < f64 >
sin : … ;
```

- **Headings are words.** `#` to `######` read what follows: a name opens a
  namespace, and one or more `< … >` patterns open a context. March stays
  token-based.
- **A heading of depth n closes every open section at depth n or deeper,** as
  in Markdown. A section ends at the next such heading or the end of the file.
- **Namespaces nest into dotted names:** `## trig` under `# math` is
  `math.trig`. Headings are the literate spelling of `name : { … } ;`, and
  both produce identical store entries.
- **Contexts nest by AND.** A definition's context is the AND of every
  context heading enclosing it: `### < positive? >` under `## < i64 >` means
  i64 and positive.
- **Several patterns on one heading are OR.** They may continue on the
  following lines, since headings are words and newlines are whitespace:
  ```
  ## < i64 >
     < f64 >
  ```
  is `## < i64 > < f64 >`.
- **`< >` is a context with no constraint,** for unconstrained words that must
  come after constrained ones inside a section.
- **Spaces inside the brackets.** In Markdown, `<i64>` looks like an HTML tag
  and a renderer hides it; `< i64 >` is safe.

**Two kinds of file:**

- **A plain `.march` file:** headings are words, and prose goes in `--`
  comments.
- **A literate `.md` file:** the loader feeds March headings and fenced
  `march` code blocks to the interpreter and skips the prose, so the
  document's outline is the module's structure. Proposed, found while
  converting `surface/money.md`: a heading is March when its text is inline
  code (`` # `money` ``, `` ## `< i64 >` ``); other headings are prose. A
  prose heading opens nothing, but it closes March sections at its depth or
  deeper, so the rendered outline always shows where a context ends. A
  context heading's alternatives continue on the lines after it that start
  with `` `< ``.

## Contexts

Type signatures are one kind of context, a compile-time one.

| Rule | Meaning |
|---|---|
| A pattern is AND | `< i64 positive? >` is a type and a guard; every guard must hold |
| Several patterns are OR | Each alternative compiles to its own clause of the family, and the body is checked for each |
| Sections | A context lasts until the next heading of the same or shallower depth |
| Nesting is AND | A definition's context is the AND of the context headings enclosing it |
| Inputs only | Contexts select a clause by its inputs. Outputs may appear as an obligation (`< i64 -> i64 >`) but never select. Return-type polymorphism is rejected |

No context at all means the effect and types are inferred. A context with OR
patterns is static duck typing over a declared set.

### The type stack

Types are first-class, so the words in a pattern run at compile time, and
each type pushes an input slot. A pattern builds the inputs from the bottom up
(proposed, 2026-10-03, pending a fine-tooth-comb review):

```
## < i64 str >              -- two inputs: an i64 under a str
## < i64 positive? str >    -- a guard tests the slots on top where it is written
## < i64 str positive? >    -- error: positive? has no clause for str
## < i64 i64 lt? >          -- a guard over two slots: first < second
## < i64 positive? even? >  -- two guards on the i64: AND
## < positive? >            -- the slot's type comes from what the guard accepts
```

- **Guards look, they don't consume.** That is the one way a pattern differs
  from ordinary code, and it is what keeps a pattern an AND. Read as plain
  code, `positive? even?` would apply `even?` to a flag.
- **A misplaced guard is a compile error,** because it is resolved against
  the slot it sees.
- **The compiler tells types from guards by what they leave:** a type or a
  flag.
- **Names for contexts are ordinary bindings.** A role is a word that pushes a
  type, `price : Money ;`, used as `< price >`. A guard is any word that
  returns a flag. There is no separate `context` definer.
- **Outputs follow `->`,** which marks where the inputs end: `< i64 -> i64 >`.
  FORTH's `--` cannot serve, since it starts a comment.
- **`< >` is the pattern and signature literal,** beside `[ ]` for code,
  `( )` for sequences and `{ }` for maps. A named signature is
  `adjustment : < price -> price > ;`.

### Lowering

A context heading produces no code; it sets the compiler's current context.
Each definition under it lowers to one clause word per alternative, plus an
entry in the family's clause table:

```
## < i64 > < f64 >
double : = + ;
```

lowers to roughly this (the `|` names are illustrative; the table holds clause
CIDs, so clause words need no names):

```
: double|i64  dup + ;     -- compiled for an i64: + is the checked integer +
: double|f64  dup f+ ;    -- compiled for an f64: + becomes f+
' double|i64 i64 clause double
' double|f64 f64 clause double
```

- **Monomorphization happens here.** One source body becomes one clause per
  alternative, each checked and compiled with its own types.
- **Calls resolve at compile time.** `3 double` compiles a direct call to the
  i64 clause's CID, with no dispatch at run time.
- **Guards add run-time code.** A type's entry becomes a guard chain, tested in
  declaration order, ending at the unguarded clause. Guards get copies of their
  slots: for `< i64 positive? str >` the copy is `over positive?`.
- **Types resolve only at compile time, for now.** Cells carry no type
  (docs/NUMBERS.md), so a value of unknown type cannot be tested at run time;
  that is a compile error until values carry tags. Guards test values, so they
  work at run time today.
- **Identity:** a caller embeds the CID of the clause it resolved to. Adding an
  f64 clause changes no i64 caller; adding a guarded i64 clause changes i64
  callers, whose behaviour did change.

## Names, strings and printing

Provisional choices, 2026-10-03, so examples are consistent:

- **Qualified names are dotted:** `math.sqr` is one token. Numbers such as
  `1.5` are recognized before words, so they do not clash.
- **Printing is `print`.** FORTH's `.` is retired from the surface language.
- **`=` is `dup` and `~` is `swap`,** adopted 2026-10-03 to try. They are the
  two most common stack words, and as aliases they compile to the same code,
  so `swap` and `dup` stay valid. `=` reads as "the same again", and undoing
  `dup` checks that two values are equal, so in a pattern `=` means equal
  values. The risk is that readers, FORTH's included, see `=` as comparison;
  March spells comparison `eq?`. In literate prose, keep `~` inside backticks,
  since GitHub's Markdown treats `~text~` as strikethrough.
- **Strings are `"…"`,** on both tracks. The reader recognizes a token
  starting with `"` by its first character, as it does numbers, and reads
  input to the closing `"`. Only `seed/system.march` keeps FORTH's `s" text"`,
  because the frozen generation 0 reads it. If a track ever cannot have
  `"…"`, the fallback is `" text "` with the spaces enforced. Built
  2026-10-05 (docs/STRINGS.md).
- **Escapes in strings follow the symbols** (decided 2026-10-05): `\name;`
  is the symbol `\name` names in code, ended by `;` as in HTML's `&times;`,
  with `\n;`, `\t;` and `\r;` for control characters and `\#9731;` or
  `\#x2603;` for any code point. `\\` and `\"` need no `;`.
- **Holes are quotations:** `"\[ code ]"` writes the code's value into the
  string, and `_` in a hole takes the literal's next input, so a literal can
  consume the stack the way a comprehension does. `\_` is short for
  `\[ _ ]`. Erlang's `~` style was considered and passed over, since `\`
  already escapes in code.
- **Raw strings are `'…'`,** as in Ruby's single quotes but with no escapes
  at all; `'` followed by a space still quotes a word. Provisional (Thomas:
  "we can always revisit").
- **Every clause of a family has the same stack effect,** so the counting
  checker knows a family's effect without types. The examples will show
  whether that is too strict.

## Symbols: `\times` is `×`

Decided 2026-10-04 (Thomas: "even if they weren't formatted it still compiles;
`\times` is the same word as `×`"). Source may spell symbols with LaTeX-style
names, as Julia, Lean and Agda editors do.

- **One table,** `symbol-table` in `seed/system.march`: 142 entries covering
  arithmetic (`\times` ×, `\div` ÷, `\sqrt` √), relations (`\leq` ≤,
  `\neq` ≠), arrows (`\to` →), logic, sets, Greek letters, and subscripts and
  superscripts (`x\_1` is `x₁`, `x\^2` is `x²`).
- **The reader accepts both spellings.** A name is `_` or `^` with the
  character after it, or the longest run of ASCII letters, so `\infty` is not
  `\in` followed by `fty`. A word that is not found is looked up again with
  its escapes rewritten, so ordinary lookups cost nothing extra, and a
  definition's name is rewritten when it is installed. `\times` and `×` are
  one word: code compiles to the same bytes and the same identity either way.
  An unknown name is an unknown word.
- **`march7 fmt` rewrites the escapes** from stdin to stdout, in code and
  comments but never in strings (the code in a string's holes is code),
  reading the same table. Formatting is
  idempotent. Zed runs it on save.
- **Symbols mean nothing until defined.** The table only makes them typeable.
  Which symbols the core library defines, and as what (`× : * ;`,
  `≤ : lte? ;`, `→` wherever `->` is accepted), is still to be decided.

## Brackets

| Brackets | Meaning | Status |
|---|---|---|
| `[ ]` | Quotations, completed by their consumer | Built |
| `( )` | Arrays and tuples: an array when it can be, a tuple when it must | Decided 2026-09-25 |
| `{ }` | Maps: namespaces and data | Decided |
| `< >` | Patterns and signatures: context headings, named signatures | Decided 2026-10-03 |

FORTH's compile-time brackets, which run words while a definition is being
compiled, are System March only. Surface code computes constants with pure
entries instead: `twelve : 3 4 * ;` can be evaluated once.

## Types and numbers

Decided:

- **Type names are sized:** `i64` (the checked signed arithmetic), `u64` (the
  wrapping `u`-words) and `f64`, all on the same 64-bit cell. A friendlier
  name is a role away: `int : i64 ;`.
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

- **A pattern describes the inputs by a computation that could have produced
  them.** `< i64 str >` is a type check, `< 0 >` checks for zero, and
  `< i64 = >` checks for two equal ints, since `=` is `dup`. The tests run on
  copies, so contexts only select, and the body sees its inputs unchanged.
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
explicit `_` pull; loop sugar. Families, the lifting rule and `( … )` with a
runtime depth marker are built (docs/CHECKER.md, docs/ARRAYS.md); `_` and loop
sugar are not.

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
- **OR at outer headings:** expand into one clause per combination, or
  allow OR only at the innermost context so expansion stays predictable.
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
  ### < a' b' -> b' a' >
  swap : … ;
  ### < a* [ a* -> b* ] -> b* >
  call : … ;
  ### < ( a'* ) [ a' -> b' ] -> ( b'* ) >
  map : … ;
  ```
  A word's signature is a context heading over it, so a library of
  polymorphic words reads like an API reference, one heading per word.
  `name : < a' -> a' > ;` would instead define `name` as a signature. An
  inline form for one word is open.
  - `[ a* -> b* ]` is a quotation type, `( i64* )` an array of ints,
    `( i64 str )` a tuple: the array-or-tuple rule for `( )` literals shows up
    in the types. `( a* )` is a sequence with any contents.
  - `{ port : i64 ; host : str }` is a map type. A namespace's interface is
    then the map type of its entries' signatures, which fits march5's interface
    identity (the hash of the exported surface).
  - Variables are needed mainly for higher-order signatures (checker slice 3).
    Names stay in signatures, as FORTH's stack comments always had them, and
    never in bodies.
  - The suffix avoids the one-space hazard of ML's `'a` against FORTH's tick
    (`' a`), and a backtick would break Markdown inline code. The rule applies
    only inside `< >`, so `*`, `u*` and `f*` cannot appear there; nothing else in `system.march` ends in `*` or `'`.
- **Open families and coherence:** whether another module can add a clause to
  `+`. If it can, which clauses are visible decides what a call resolves to.
- **Recursion between clauses:** code identities cannot form cycles, so
  clauses that call each other need group identities or one combined
  definition (lineage research §5.8).
- **Self-delimiting brackets** (proposed): `[ ] ( ) { }` end a word and are
  tokens on their own, so `[[` reads as `[ [` and `{port : 8080}` needs no
  inner spaces. No word name in `system.march` contains these characters
  except the brackets themselves. `< >` cannot work this way, because `>` is
  part of names such as `>r` and `i>f`, so nested types close with `> >`. The
  compile-time brackets then need a new name, perhaps Factor's `<< >>`, which
  means parse-time evaluation there; March spells shifts `lshift` and
  `rshift`, so `<<` and `>>` are free. Not `[: ;]`, which is the standard
  FORTH spelling for quotations (Forth-2012).
