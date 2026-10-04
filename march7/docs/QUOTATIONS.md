# Quotations and the control flow that consumes them

Status: built 2026-09-30, step 2 of the agreed Checked March order ("quotations
completed by their consumer"; doc/lineage/CONTROL-FLOW-COMPREHENSIONS.md §2.1).
Everything is March code in `seed/system.march`.

## The idea

`[ ... ]` is a quotation literal, as decided on 2026-09-25. Its body compiles
into a buffer of its own and stays **pending**, anchored where it appeared.
What happens next depends on the next word:

- **A consumer** (`if`, `while`, `times`) inlines the pending bodies into the
  definition as ordinary branches. There is no quotation object at run time,
  and the checker sees plain branches.
- **Anything else**, a number, a call or `;`, first **materializes** each
  pending body: it is sealed as an anonymous word and a quotation of it is
  emitted where the literal stood. `[ dup u* ] call` works this way, and the
  checker follows the quotation's effect through `call`.

This is march4's design, which worked until its "Design B" moved quotations to
a global stack with no position. Here a pending quotation records its nesting
depth, and a consumer or materialization touches only quotations at its own
depth, so quotations cannot drift.

## Words

| Form | Meaning |
|---|---|
| `c [ t ] [ f ] if` | Inline both arms: `t` when `c` is true, else `f` |
| `c [ t ] if` | Inline one arm, run when `c` is true |
| `c if … else … then` | FORTH's form, used when no quotation is pending |
| `[ c ] [ body ] while` | Loop while `c` leaves true |
| `cycle … while … repeat` | FORTH's form, used when nothing is pending |
| `n [ body ] times` | Run `body` n times with the index 0..n-1 as `i0` |
| `i0`, `i1` | The innermost and next loop index (compile-only) |
| `[[ … ]]` | FORTH's compile-time brackets, renamed from `[ ]` |

`if` and `while` choose their form by how many quotations are pending, as
march4's `times` did. `times` keeps its limit and index on the scratch stack,
so the body may use the scratch stack too, and `i0`/`i1` read the indices
without disturbing it.

## Rules and errors

The checker applies its usual rules to the inlined code:

- The arms of an `if` must have the same effect, or checked mode rejects the
  definition (`[ 1 ] [ ] if` traps 103).
- A loop body must leave the depth unchanged.

| Trap | Meaning |
|---|---|
| 15 | `exit` or `recur` inside a quotation that is sealed as its own word, where they would mean that word rather than the definition |
| 16 | Quotations nested more than 32 deep |
| 17 | `]` without `[` |
| 18 | More than 64 quotations pending |
| 19 | The wrong number of quotations for a consumer (`[ a ] [ b ] [ c ] if`) |
| 20 | `;` inside an open quotation |

An error inside a quotation abandons it: `recover` frees every open and pending
quotation buffer along with the definition.

**`exit` and `recur` in arms** (2026-10-04). They mean the definition they are
written in, so a quotation may contain them when a consumer inlines it, which
is how recursive words are usually written:

```
: f dup 0 eq? [ drop 1 ] [ dup 1 - recur * ] if ;
```

Compiling one inside an open quotation sets a bit for its nesting level
(working memory 872). At `]` the bit becomes a flag on the pending entry (a
byte at 880 + i). Sealing a flagged quotation as its own word traps 15, as
does `[ recur ]` outside a definition. Inlining a flagged quotation into the
definition is fine; inlining it into an enclosing quotation flags that one,
since it may still be sealed.

## Mechanics

- **Working memory** (region 1):
  - 800: the nesting depth.
  - 808: the pending count.
  - 1024 + 24i: pending entries (buffer, length, depth).
  - 2560 + 24d: saved outer builders (buffer, offset, `STATE`).
- **`[`** saves the outer builder and opens a new one. It is flagged
  *transparent* (dictionary flag bit 2), meaning it does not materialize
  pending quotations. That lets two literals stand side by side for `if`.
- **`]`** restores the outer builder. Compiling, it leaves the body pending.
  Interpreting (`[ 1 2 u+ ] call` at top level), it seals the body at once and
  leaves its token.
- **`interpret`** materializes pending quotations before compiling a number or
  word, or before running an immediate word that is not transparent. The
  consumers and `--` are transparent.
- **Inlining** copies the body's instructions and shifts its branch targets by
  the instruction index where the body lands.

The system source itself uses FORTH's forms. The quotation-aware words are
defined as `qif`, `qwhile`, `qexit` and `qrecur` and exported under the public
names, so compiling the system never depends on them.

## Tests

`tests/quotations.rs` has seven tests:

- one- and two-armed `if`, and the FORTH form;
- `times` with nested indices, and `while`;
- first-class quotations, their anchoring and nesting;
- the checker on inlined branches;
- every error, and recovery after one;
- the renamed compile-time brackets;
- `recur` and `exit` in inlined arms, nested arms and loop bodies, and the
  trap when such a quotation is sealed.

## Next

- `map` and `each` over arrays, when arrays exist.
- Typed quotation parameters, so a word that calls a quotation it was given
  stops being a dynamic call (checker slice 3).
