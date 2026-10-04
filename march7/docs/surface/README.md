# Surface March examples

Status: design probes, 2026-10-03. These programs are written in the surface
language described in [../SURFACE.md](../SURFACE.md), before any of it is
built. Their job is to find what the design gets wrong or leaves out. None of
them runs yet.

| File | Exercises |
|---|---|
| [money.md](money.md) | A literate module: a distinct type, units, inverses, family clauses, a role, contexts across prose |
| [calc.march](calc.march) | A recursive-descent evaluator: stack juggling without locals, literal patterns, mutual recursion, a variant type and `match` |
| [config.march](config.march) | Maps, map types, dotted access, building maps at run time |
| [stats.march](stats.march) | Arrays of floats, type variables, the lifting rule, literals typed by context, comprehensions |

They assume a library that does not exist yet: `2dup`, `-rot`, `/mod`,
`char-at`, `digit-value`, `>str`, `pad-left`, `concat`, `length`, `sqrt`,
`fold`, `map`, `each`, `merge`, `>map`, `undo` and `match`.

## The most serious findings

1. **Mutual recursion (F8, F10).** A parser's `expr`, `term` and `factor` call
   one another, and `Expr` refers to itself. Code identities cannot form
   cycles, so March cannot express either today. This is the biggest gap.
2. **Recursion inside an inlined arm (F9).** Calling a word from inside its own
   `if` or `match` arm is how most recursive code is written. It traps today
   (trap 15).
3. **Comprehensions that keep or drop (F15).** A comprehension body with a
   one-armed `if` leaves a varying number of values, which the counting
   checker rejects.
4. **Whether contexts take values apart (F4).** The examples keep running into
   it.

## Findings

**F1. Distinct types need a name in their identity.** Content addressing makes
two `distinct int` definitions identical. A distinct type is distinct by
name, so `Money : distinct int ;` must hash the name (with its module) into
the type. That makes it a nominal type in a structural store, which is fine
but should be deliberate.

**F2. `!` has two forms that disagree on direction.** `5.0 5 !` converts the
top to the type of the value below and keeps both. `5 Money !` converts the
value below to the type on top and consumes the type. Both are wanted:
`stats.march` needs `i0 f64 !` and falls back to `i>f`. Either one rule covers
both, or they get two spellings.

**F3. Inverses of integer arithmetic must check exactness.** Undoing `100 *` on
550 has no integer answer, so `550 cents [ dollars ] undo` fails. That is
correct, but it surprises anyone expecting 5 or 5.5. Floats have no exact
inverses at all, so float units need a decision: approximate inverses, or
none.

**F4. Contexts that take values apart would remove a lot of code.** In
`money.md`, `= cents cents ; + : + cents ;` would replace
`[ cents ] undo swap [ cents ] undo + cents`. In `calc.march`, every `apply`
clause starts with `drop` to discard the operator its literal pattern already
matched. The proposal keeps contexts selection-only so a sticky group's
bodies all see the same inputs. The examples suggest that taking values apart
is exactly what a group wants: every Money arithmetic clause wants ints. One
possible rule: a pattern that is a type only checks, while a pattern built by
a constructor, a unit or a literal also takes its value apart.

**F5. Open families need a rule against orphans.** `money.md` adds clauses to
`+`, `-`, `*`, `/` and `print`. That is fine because they are clauses for
Money, a type the module defines. A module adding `+` for two types it does
not own would make a call's meaning depend on which modules are loaded.
Proposed rule, Haskell's: a module may add clauses only for types it defines,
or to families it defines.

**F6. A family's name inside its own clause means the family.** The Money
`print` clause calls `print` on a string and an int. Those calls resolve by
type to other clauses. A call is recursion only when it resolves to the same
clause.

**F7. Contexts carry across prose.** In `money.md`, the `/` block is still
under `= Money int`, set two blocks and a paragraph earlier. A reader of the
rendered document can miss that. Options: contexts end at the end of a code
block in literate files, or the loader warns when a block starts under a
context set in an earlier block.

**F8. Mutual recursion between words.** `factor` calls `expr`, which calls
`term`, which calls `factor`. Each word's identity hashes the identities of
the words it calls, so a cycle has no identity. The usual fix is a group
identity: the words of a strongly connected group are hashed together, and a
call inside the group names a member of the group rather than a CID. It also
needs forward references in the surface language, since `factor` uses `expr`
before `expr` is defined.

**F9. Recursion inside inlined arms.** `eval` calls itself inside `match`
arms, and recursive words in general call themselves inside `if` arms. Trap
15 forbids `recur` in a quotation because a materialized quotation is its own
word. An arm that is inlined runs in the word itself, so recursion there is
safe. The rule should be: allowed when inlined, an error when materialized.

**F10. Recursive types.** `Expr : variant { … bin : Expr Expr str } ;` refers
to itself, the same cycle problem as F8 at the type level. Group identities
would serve here too.

**F11. Without locals, parsers juggle.** `number`, `term` and `expr` keep
three values in motion and use the scratch stack to park a fourth. It works,
and it is readable once you know the convention, but it is the strongest
evidence for locals so far. One catch for locals: a loop accumulator such as
`number`'s changes every iteration, so immutable locals would not help there.
A fold-style loop would.

**F12. Map entries with no inputs are values.** Entries are words, but
`endpoint` builds a map from stack values with `_`, which only makes sense if
an entry with no inputs is evaluated once when the map is built. The rule:
an entry with no inputs is a value, computed when the map is built (at compile
time when it can be); an entry with inputs is a word. The same rule makes a
map type a map whose values are types: `Config`'s `port : int ;` is an entry
whose value is the type `int`.

**F13. Dotted access falls out of retiring `.`.** A word starting with `.`
reads that entry of the map on top: `.port` is `( map -> value )`, and
`config.port` is `config .port`. The same rule covers namespaces, since
`math.sqr` calls the entry `sqr` of `math`.

**F14. A word's signature is an `=` line.** Found while writing: `swap : a'
b' -> b' a' ;` would define `swap` as a signature value. SURFACE.md is
corrected.

**F15. Comprehensions need effects with a varying count.** `positives` keeps
or drops each element. Its `if` arms leave different counts, which slice 1
rejects. In types it is natural: the body is `[ f64 -> f64* ]`, leaving any
number of floats, and the comprehension gathers them. The checker needs that
`*` to type comprehensions at all.

**F16. Computed keys may need no syntax.** `>map` over pairs covers keys that
are not names or are computed, with nothing new inside the braces.

**F17. No character literals.** `calc.march` compares one-character strings,
which reads fine. It may be all that is needed.

**F18. Repeated loop shapes want combinators.** `term` and `expr` are the same
loop with different operators and operands. A combinator taking both as
quotations needs typed quotation parameters and row variables, which the type
variables proposal covers.

**F19. Sticky contexts push toward a file order.** `config.march` needs no
`= ;` because its words with no context come first. `calc.march` cannot do
that: its plain words call the words that have contexts, words must be defined
before use, so it needs one `= ;`. Definition order and grouping by context
pull in different directions, but one reset per file is tolerable.

## What held up

- **Name-first definitions** read well throughout, including maps, map types
  and symbolic names.
- **The type stack on `=` lines** worked in every example, including literal
  patterns (`= int int "*" ;`).
- **Map types written like map values** needed no new syntax.
- **The lifting rule** made `variance` a one-liner: `dup mean - dup * mean`.
- **Literals typed by context** kept `double` free of conversions.
- **Signatures with type variables** (`fold`, `map`) read naturally.
