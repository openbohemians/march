# Control flow, comprehensions and polymorphism across the March lineages

Written 2026-09-30 for Thomas, to recover earlier work before Checked March is
designed. Four research agents each read one part of the history: march1,
march3 and the pre-March notes; march2 (both interpreters); march4; and
march5. They read the source and docs, including deleted docs and archive
branches, and built and ran old code in scratch copies. Claims marked
**verified** were reproduced by running the code. Everything else is cited
from the sources. Nothing in the repository was changed.

Labels: **built** (implemented and exercised), **broken** (implemented but
fails), **design** (prose only).

## 1. Where the good ideas are

| Topic | Richest source |
|---|---|
| Array processing, comprehension notation | The pre-March `ref/old-notes/euler.mh` sketches (2018-2022), and march4 |
| Guarded families as control flow | march2 interpreter A (worked), march5 (ordered dispatch) |
| Loops | march2 A and march4 |
| Elementwise `+` | march2 A's `[*]` family, which works (verified) |
| Overload resolution and specialization | march4, and march5's lessons |
| Floats | Nowhere. No lineage ever implemented a float |

## 2. Control flow

### 2.1 Quotations completed by their consumer (march4, built, then broken)

In march4, `(` captured *tokens*, not code. The immediate word that consumed
the quotation (`if`, `times`) compiled it against the caller's type stack and
inlined it as branches. Commit 1b00635 explains why: "Quotation type inference
needs to be deferred until the consuming immediate word (if/map/each/times)
provides context". So `( 3 < )` resolved its polymorphic `<` from the caller's
types.

- `if` compiled `[0branch] true… [branch] false…`. Branch offsets were computed
  from the already-compiled branch sizes, so the hashed bytes never needed
  patching (compiler.c 2256-2281).
- `times` chose its form by *how many* quotations were pending: one is a
  counted loop, two is an until loop (`0 ( dup 3 = ) ( 1 + ) times` gives 3,
  verified). Overloading `times` by type had failed first, because quotations
  never appeared on the type stack.
- Failures to avoid:
  - Branch effects were never checked for equality.
  - A loop's effect was never applied to the type stack.
  - "Design B" (c16fa9f) moved quotations onto a global stack with no position
    in the word, which broke everything that had worked the week before.
    For example, `3 5 < ( 42 ) ( 99 ) if` gave 42 before Design B and an error
    after (verified).

### 2.2 Guarded families as the conditional (march1 design, march2 A built, march5 built)

- **march2 A** is the one lineage where it fully worked. Guards ran on an O(1)
  fork of the persistent stack, with state writes going to a discarded overlay.
  Recursion by pattern matching worked with no `if` at all (verified):
  ```
  = i64 -> i64 ; ? dup 0 eq ; : cd ;
  = i64 -> i64 ; : cd 1 - cd ;
  ```
  But a guard added nothing to specificity, so the guarded variant had to be
  defined before its fallback (verified).
- **march5** turned families into an ordered `DISPATCH` node:
  - It tried each candidate's type key, then its pure guards.
  - If the candidate's *body* failed ("deopt"), dispatch also fell through to
    the next candidate. That is backtracking among clauses.
  - Build time pruned the candidates by the known stack types, and emitted a
    `DISPATCH` only when more than one remained.
  - Its bugs are lessons:
    - Candidates were ordered by name, not by declaration (verified).
    - Guards could run after an effect (verified in 7 of 16 builds).
    - Candidates with multiple results failed.
- **march1 and march3** designed *sticky headers*: `? cond ;` or `= sig`
  applies to every following `:` definition, so a family is written as
  consecutive blocks. march2 sketched grouped `CASE.` / `CONTEXT.` blocks.
- The unifying principle, from march2's DESIGN.md: "Static types are context
  guards too, but they are compile time guards", and "Guards check type
  constraints at compile time when possible, runtime otherwise". Guard
  elimination when a guard is statically known appears in march2 and march5.

### 2.3 Errors re-enter the same family (march1 design, march2 A built)

`raise T` pushed an error value, restored the word's original arguments, and
dispatched the *same word* again. A handler is just the variant whose first
input is the error type (`= DivideByZero i64 i64 -> i64 ;`), selected through
a subtype chain (`% Overflow < MathError ;`, verified). Flaws:

- The stack consumed before the raise was not rolled back.
- Specificity ignored the subtype depth, so `MathError` could beat
  `DivideByZero`.
- The archived test passed by accident.

march1 designed the same idea and never got it working.

### 2.4 Loops

- **march2 A, built:**
  - `[q] n #do` with `i0` for the innermost index.
  - A negative count gives negative indices.
  - `n #[ body ]` sugar.
  - A -1/0 flag used as the count gives "when" for free:
    `[ 42 ] 5 3 gt #do` printed 42 (verified).
- **march4, built:** a counted `times` kept its counter on the return stack,
  FORTH `DO` style, with `i0` to read it. There was no `i1`, and `i0` inside a
  called word returned a return address.
- **Pre-March notes, design:**
  - `1000 { (r) … }` runs a block N times, with `(r)` for the index.
  - `{ + | drop }` is a two-armed conditional block, true and false arms
    separated by `|`.
  - `times` was an adverb consuming the *next word*
    (`A B C 2 times rot` gives `B C A`).
- **Tail calls:** march2's design was "Tail recursion only", compiled as a
  jump. march2 A had no tail calls and died at depth 1000 (verified). march7
  now has tail calls and `recur`.
- **Recursion and identity:** march5 could not express recursion at all,
  because a word's CID cannot contain itself. A "self-call" silently bound to
  the previous version of the word (verified). march4's specialization cache
  recursed forever and segfaulted (verified).

## 3. Comprehensions and collection construction

### 3.1 Array processing in the pre-March notes (design, 2018-2022)

`euler.mh` solves Project Euler #1 several ways across 12 revisions:

```
1000 iota (r)
(r) 3 multiple
 ~  5 multiple or
index
( + ) reduce
```

- Scalar words (`mod`, `not`, `or`) apply to a whole `iota` array, producing
  boolean masks.
- `index` compresses the array by the mask, and `reduce` folds it.
- That is the APL elementwise model, stated years before march1.

The same file also tries:

- **Fan-out ("plex").** `( ( 3 multiple ) ( 5 multiple ) ) || or` applies each
  quotation to the same input.
- **Subject reuse without `dup`.** `(r)` with `~` as a ditto ("same as the line
  above"), and `|>` lines that each re-push a captured value:
  ```
  |> 3 multiple
  |> 5 multiple or
  |> index
  ```
- **Quoted operators.** `+'` and `#' +` quote an operator to pass to `reduce`.
- **An indentation form.** A higher-order word takes an indented block as its
  argument.

### 3.2 Adaptive literals: array when it can be, tuple when it must (march2 A, built)

`{ 1 2 3 }` was an array and `{ 1 "x" 3 }` a tuple, decided by content. This is
exactly the `( )` rule decided on 2026-09-25 (march6/docs/ARRAY-FAMILIES.md
§1a). march2 also converted arrays to persistent vectors when they were stored
in state.

### 3.3 Natural comprehensions and the explicit `_` pull (march2 design, march4 built)

- **march2 B's tests designed brackets that consume from outside:**
  `10 20 [ + ]` gives `[ 30 ]`, and `[ 10 20 + ]` also gives `[ 30 ]`.
- **march4 built "natural comprehensions".** `[` marks the stack depth, the
  body runs on the real stack, and `]` gathers what it left:
  `[ 1 2 + 10 20 * ]` gives `[ 3 200 ]` (verified).
- **The implicit pull (`1 1 [ + ]`) was rejected** in favour of an explicit
  `_`, which copies values from below the marker: `5 7 [ _ _ + ]` gives
  `[ 12 ]` (verified).
  - DESIGN-ARRAYS.md: "too confusing and has too strange edge cases. Being
    explicit with `_` seems a much better choice."
  - Pulled values are dropped at `]`.
  - The pull order came out reversed (`10 3 [ _ _ ]` gives `[ 3 10 ]`), which
    the docs never mention.
- **The limit was static counting.** march4 counted elements at compile time,
  so a loop or conditional inside `[ ]` produced wrong lengths
  (`[ 3 ( i0 ) times ]` has length 0, verified). The rejection of the implicit
  pull was also never enforced.

### 3.4 One lifting family broadcasts every operator (march2 A, built, verified)

When no variant of `+` matched, march2 A tried a `[*]` family, with
`current_word` set to the failed operator. Inside its body, the token `[*]`
re-dispatched *the operator being lifted*. So one pair of definitions lifted
every family operator elementwise:

```
= {a} a -> {a} ;
: [*] over array.len #[ over i0 @ over [*] rot i0 ! swap ] drop ;
= {a} {a} -> {a} ;
: [*] dup array.len #[ over i0 @ over i0 @ [*] rot i0 ! swap ] drop ;
```

| Input | Result |
|---|---|
| `{ 1 2 3 } 10 +` | `{11 12 13}` |
| `{ 1 2 3 } { 3 4 5 } +` | `{4 6 8}` |
| `{ 1 2 3 } { 3 4 5 } *` | `{3 8 15}` |
| Nested arrays | Elementwise recursively |
| A user word `inc` | Lifted too |

The archived tests failed only because the REPL read one line at a time, so
multi-line definitions broke. The earlier march2 digest's "never
demonstrated" is wrong. Limits:

- No rank polymorphism: a nested array plus a scalar failed.
- Mismatched lengths truncated silently, unless a guard compared them.
- Built-in non-family words such as `eq` never reached `[*]`.

### 3.5 map and each (design)

- march4's QUOTATIONS.md: `array ( 2 * ) map` would be inlined into a loop,
  with "No quotation object created".
- march4's PLAN-TYPES.md: `map : (Vec α, Thunk[α—β]) —> Vec β`, with a C demo
  that applied a thunk elementwise "using an internal scratch stack".
- Neither was built.

## 4. Polymorphism, for `1 1 +`, `1.0 1.0 +` and array `+`

- **march2 A, built.** Runtime multi-dispatch over per-value types, with type
  variables, `{a}` array types, specificity by count of concrete inputs, and
  first-defined order on ties. Users extended `+` directly:
  `= str str -> str ; : + ++ ;` made `"ab" "cd" +` give `abcd` (verified).
- **march4, built.** Summed specificity scores at compile time. The C version
  scored exact 100, type variable 80, unknown 50, `any` 10. The OCaml version
  raised "Ambiguous overload" on ties and had a real function type for
  quotations (`TFunc`), which the C rewrite dropped.
- **march4's "Design B", built but limited.** Per-call-site specialization,
  cached by name and concrete input types. Its failures give the rules:
  - Infer outputs rather than trusting declared signatures.
  - Record the cache entry before compiling, so recursion terminates.
  - Give each instance an identity from the generic word plus the type vector.
    Two instances with identical bytes collapsed into one blob, keeping the
    first one's signature.
  - The generic word never had an identity of its own.
- **march5, built.** Static pruning by stack type at build time. A runtime
  `DISPATCH` was emitted only when that left more than one candidate. Its
  **primitive identity collision** is the key warning:
  - Primitives with the same signature shared one CID, so `add_i64(2,3)`
    actually ran `eq_i64` (verified).
  - Every member of a family needs its own identity. Behaviour must never
    depend on a name table.
- **Pre-March notes, design.**
  - Overloads of a name keep a fixed arity, which keeps dispatch decidable on
    untagged cells.
  - Two equalities: `=` (numeric, `1 1.0` equal) and `==` (strict).
  - An integer type lattice refined by sign.
- **Numbers.** march1 tested a same-type-only arithmetic rule, so i64 plus a
  rational is an error. No lineage designed numeric promotion. `f64` appears
  only as enum entries, a fake test overload in march4, and an unimplemented
  overload in march5.

## 5. What this suggests for march7 and Checked March

Ranked by how directly it applies:

1. **Families with static resolution first and runtime dispatch only when
   needed.** This is march5's pruning plus march2's "types are compile-time
   guards". Each member has its own identity (march5's warning). An explicit
   declaration order and an ambiguity error (march4's OCaml version) replace
   silent first-defined wins.
2. **One lifting rule instead of per-operator array versions.** This is
   march2's `[*]`, resolved statically: an array clause applies the element
   type's clause, chosen at compile time, inside a loop. Add a length check,
   scalar extension, and later rank polymorphism.
3. **Quotations completed by their consumer.** `if`, loops and `map` type-check
   a quotation in the caller's context and inline it into march7's branches
   (march4). Keep each quotation's position in the word, and require equal
   branch effects.
4. **Comprehensions with a runtime depth marker.** `[` or `(` records the data
   stack depth on the scratch stack, and the closing bracket gathers what is
   above it at run time. That removes march4's static-count limit, so loops
   and conditionals work inside, and `_` becomes a pick relative to the marker
   in a deliberate order. The scratch stack's per-frame discard keeps the
   marker local.
5. **Subject reuse from the pre-March notes.** `|>` or ditto lines can map onto
   the scratch stack: capture at the start, re-push per line, drop at the end,
   with zero net effect. That gives readable fan-out without `dup` and `over`.
6. **Cheap loop sugar.** A flag as a count ("when"), `n #[ body ]`, and loop
   indices kept on the scratch stack, with `i0` and `i1` for nesting and no
   access across calls.
7. **Errors as re-dispatch into the same family.** Handlers are more specific
   variants chosen by subtype depth, with the stack reset to entry depth first.
   This needs rethinking before adoption.
8. **Recursion needs explicit identity handling.** Self-calls stay local
   (`recur`). Recursion between family members needs either group identities
   or compiling a family into one definition (march5, march4).

## 6. Corrections to the earlier digests

- **march2.md** says `[*]` broadcasting was "never demonstrated". It works
  (§3.4). The archived tests fail only because of test bugs and one-line REPL
  input.
- **march2 A's immediate words** (`::`) ran on the live data stack and compiled
  nothing, so test_immediate's passes were accidental.
- **march4.md:**
  - It notes quotations "partly broken". The cause is specific: Design B's
    global quotation stack (§2.1).
  - Nested quotations never worked in either build.
- **march1:**
  - Its v0.2.0 commit and tag messages claim error re-dispatch works; it never
    did.
  - Its `if` and `then` were stubs, and they could not appear inside a
    definition.
