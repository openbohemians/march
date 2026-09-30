# The stack-effect checker

Status: slice 1, 2026-09-30. It infers stack effects: how many cells a word
takes and how many it leaves. Value types, typed quotation parameters and
family resolution come in later slices (see the end). It is written in March,
in `seed/system.march`, with no host support beyond existing primitives.

## Why it comes first

The lineage research (`doc/lineage/CONTROL-FLOW-COMPREHENSIONS.md`) found that
nearly every past failure came from not knowing stack effects statically:

- march4's `if` and loops mistracked the types they left.
- Its comprehensions miscounted their elements.
- march2's collection marker could not tell consumed values from collected
  ones.
- march4's `execute` crashed because it did not know what a quotation did.

Consumer-completed control flow, families, the array lifting rule and
comprehensions all depend on knowing effects.

## What it analyses: compiled code, not source

The checker walks a definition's canonical instruction bytes (FORMAT: literal,
primitive, call, quotation, branch, zero-branch, `recur`, data, tail call,
return). This has three consequences:

- It is exact whatever syntax or immediate words produced the code.
- The same analysis runs on the definition being built, whose bytes are in
  its output region at `;`, and on sealed code read back by its CID
  (`code-cid` then `blob-read`).
- A call's callee is found from the CID in the call operand (`resolve`), so
  the checker needs no name lookup.

## Effects are derived, lazily

A word's effect is computed from its code the first time it is needed
(`effect-of`) and remembered for the session in a table indexed by execution
token. Effects are never stored in images. After a reload they are simply
computed again from the code, so they cannot go stale. Code identities cannot
form cycles, so computing a callee's effect inside a caller's analysis always
terminates. Each analysis has its own region, so analyses nest.

## The rules

The analysis follows every path through the code, tracking the stack depth
relative to entry and the scratch-stack depth.

- **Primitives** have fixed effects. `trap` ends its path.
- **Calls** apply the callee's effect.
- **Branches.** A forward branch records its state at the target. Every way of
  reaching an instruction must agree on both depths; otherwise the branches are
  unbalanced. A backward branch must return to a loop header at the depth it
  was first reached, so a loop body leaves the depth unchanged.
- **Returns.** Every return, including `exit`, must leave the same depth.
- **Scratch stack.** A pop or peek must not reach below the word's own frame.
  What a word leaves there is discarded on return, so it never affects its
  caller.
- **Quotations.** `' sq` pushes a token carrying `sq`'s effect, so
  `' sq call` checks as `sq` would. Calling a quotation whose origin is unknown
  is a dynamic call.
- **Recursion** is solved from the base case. The first pass treats `recur` as
  a dead end, which yields the base case's effect. The second pass assumes that
  effect for `recur` and must reproduce it.

The result is an effect, *(inputs, outputs)*, or unknown with a reason:

| Reason | Meaning |
|---|---|
| 1 | Dynamic call: a quotation whose effect is unknown is called |
| 2 | Calls a word whose effect is unknown |
| 3 | Branches meet at different depths |
| 4 | A loop changes the depth |
| 5 | Returns leave different depths |
| 6 | Scratch pop below the word's frame |
| 7 | Never returns (for example recursion with no base case) |
| 8 | Recursion inconsistent with its base case |
| 10 | Limits |
| 11 | Invalid code |

## System March and checked mode

Unknown is not an error by default. System March legitimately contains words
the checker cannot pin down:

- `interpret` makes dynamic calls, and so `evaluate` is unknown too.
- The FORTH idiom `( -- n 1 | 0 )` returns different counts on different
  paths.
- `s"` leaves nothing when compiling and three cells when interpreting.

Such words get an unknown effect with a reason, and so do their callers.

`checked` turns on checked mode: `;` then analyses the definition before
sealing it and rejects it, trapping with 100 plus the reason, if its effect is
unknown. A rejected definition is not installed. `unchecked` turns it off.
Checking is off at boot, so rebuilding the system is unaffected. Whether user
code should be checked by default is a later decision.

## Public words

| Word | Effect | Meaning |
|---|---|---|
| `stack-effect` | `( token -- inputs outputs 1 )` or `( token -- reason 0 0 )` | A word's effect |
| `checked` | | `;` rejects unknown effects |
| `unchecked` | | Turns that off |

## Tests

`tests/checker.rs` has eight tests:

- straight-line words and calls;
- branches, loops, `exit`, trapping paths and the scratch stack;
- recursion from the base case, including a word whose recursive path takes
  more inputs than its base case, a word with no base case, and an
  inconsistent one;
- quotations;
- every unknown reason that user code can reach;
- effects of system words, computed from their code;
- checked mode, including that a rejected definition is not installed;
- effects recomputed after reloading a saved image.

## Cost

The checker adds about 230 lines to `seed/system.march`. That pushed compiling
the system over the driver's default budget, though the checker itself does not
run during a rebuild. The cost was the dictionary's linear lookup, now replaced
by a hashed dictionary (docs/REBUILD.md). Generation 1 compiles the system in
5.3 million steps (6.3 million after quotations were added).

## Next slices

1. **Types.** Add value types (i64, f64, arrays, quotations) to the same
   analysis. Literal and primitive types come from the code, and word types
   from signatures or inference. Families need the types *during* compilation,
   to pick a clause for `+`. The analysis can report the live state at the end
   of a partial definition, so the compiler can ask for the types at the
   current point. Forward branches that are not yet patched need care there.
2. **Consumer-completed control flow.** Done (docs/QUOTATIONS.md): `if`,
   `while` and `times` inline pending quotations, so the checker sees plain
   branches. `map` waits for arrays.
3. **Typed quotation parameters.** A word that calls a quotation it was given
   declares or infers that quotation's effect, so it stops being a dynamic
   call.
4. **Families** resolved by type at compile time, falling back to runtime
   dispatch only when a type is unknown.
