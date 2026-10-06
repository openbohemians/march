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

## Slice 2: value types and families (2026-10-04)

**Types.** The same analysis now tracks value types: 0 unknown, 1 i64, 2 f64,
a byte each, by stack position in 64-byte blocks. Position p is the cell at
depth p - 32 relative to the word's entry, so a word's inputs (unknown unless
an instance gives them types) are 31, 30, and so on down, and it may grow 32
cells above its entry. A cell outside the block is lost (255): no clause
matches it, so a family call on it is an error (trap 23), never the i64
version applied to a float's bits. The first version kept only the top eight
slots in one cell, and a ninth value slid off unknown.

- Literals give i64 (opcode 1) or f64 (opcode 10). Integer arithmetic, logic
  and comparisons give i64; float arithmetic and `i>f` give f64; float
  comparisons and `f>i` give i64. `dup`, `swap`, `over` and `rot` carry types
  with their values.
- A word's output types are computed with its effect and remembered beside it
  (the table at working offset 712), so a call gives its outputs the callee's
  types.
- Where branches meet, types merge: a slot whose types disagree becomes
  unknown. A loop header's types are widened from its back edges, and the pass
  runs again until they are stable.
- `stack-types` reports a word's output types: `( token -- types outputs 1 )`
  or `( token -- reason 0 0 )`, types packed a byte each, the top first.

**Families.** `+ - * /` and `lt? gt? lte? gte?` are families (dictionary flag
bit 3). Each is the i64 version; a registry (working offset 728) holds their
f64 clauses: `f+`, `f-`, `f*`, `f/`, `flt?` and float `gt?`, `lte?`, `gte?`
that are false with a NaN. When a definition has compiled a family call, `;`
runs the typed analysis in resolving mode:

- At each family call it picks the clause whose input types match, applies
  that clause's effect and types, and records the choice. After the analysis
  the call is patched in place to the clause: a call's operand is a 32-byte
  identity either way.
- An integer literal just before the call takes its type from context:
  `x 1 +` with x an f64 becomes `x 1.0 +`, if nothing branches to the call.
- Inputs whose types are unknown keep the i64 version, as before families.
  Inputs that are all known but match no clause (`1 2.5 +`, where the 1 is not
  just before the call) trap 23, and the definition is not installed.

**Generic words.** A word whose code still calls a family word, or another
generic word, is generic: bit 40 of its effect says so, and `;` sets
dictionary flag bit 4 on it, so that compiling a call to it also triggers
resolution. A call to a generic word with an f64 among its inputs gets an
instance:

- The word's code is copied and resolved with the call's input types as the
  types its inputs start with, then sealed as an anonymous word. The call is
  patched to it.
- Instances are remembered by word and input types (working offset 3512), so
  the same call gets the same instance. Instances nest: `quad`, calling
  `double` twice, gets an instance whose calls go to `double`'s instance.
- A literal in the word takes the call's types: `: inc 1 + ;` on 2.5 adds 1.0.
- A known type that matches no clause in the instance is the caller's error
  (trap 23 at its `;`).

**Top level.** The interpreter keeps the types of the values it pushes in a
stack of its own, a byte each for the whole data stack (a 64 KB region whose
handle is at working offset 3576): literals give theirs, and each word run
there gives its outputs the types an analysis from its inputs' types finds
(remembered by word and input types). So `1.5 2.5 +`, `1.5 dup +` and
`2.5 double` typed at top level use the f64 clause or an instance, and an
integer literal just read converts to a float when a float clause needs it
(`2.5 1 +`). Recovery after an error forgets the types, with the stack.

Words run at top level are analysed the first time they run there, which
costs the system's own rebuild about a million steps once per session (the
analysis of `:` covers much of the compiler).

**Arrays** (docs/ARRAYS.md) add types by element type and rank: 3, 4 and 5
for arrays of i64, f64 and unknown elements, 3 more per rank up to 230, 247
for strings, 252 for mixed elements and 254 for the empty array. The checker
models `mark` and `gather`, so an array literal's element count and type are
exact, even when its count varies, and families lift over arrays, all the
way down nested ones. **Strings** are 253 (docs/STRINGS.md) and **maps** 231
to 246, by their keys' and values' kinds (docs/MAPS.md).

| Type | Meaning |
|---|---|
| 0, 1, 2 | unknown, i64, f64 |
| 3 to 230 | arrays, by element type and rank |
| 231 to 245 | maps: 231 + 5 × key kind + value kind |
| 246 | the empty map |
| 247 | an array of strings |
| 252 | an array of mixed elements |
| 253 | a string |
| 254 | the empty array |
| 255 | lost: outside the tracked window |

**Scratch types.** The types of the first eight scratch-stack slots are kept
too (field 128, a byte each), and merged where paths meet like the data
stack's (per-op tables 9 and 10). So `i0` is an i64, and `each` and `map`
know their array's element type.

**Empty collections join.** Where paths meet, an empty array meeting an array
of t becomes an array of t, and an empty map meeting a typed map becomes that
map, on the data stack and the scratch stack alike (since maps; it was the
scratch stack only), so a collection built in a loop keeps its type.

**Type errors.** Known types that cannot work are errors, recorded as a trap
number (field 208) and raised at `;`: 23 for a family call whose inputs match
no clause and do not lift, 26 for an array used as a condition. Resolving
mode raises them for every definition it analyses, which is any definition
with an array literal or a call to a family or generic word; checked mode
raises them for the rest.

**Instances keep their types.** An instance's effect and output types are
remembered from the analysis that made it, with the call's input types; its
code alone does not say what, for example, an array built from its inputs
holds.

## Next slices

1. **Quotation types and strings** as types, `!` conversion, and effects with
   a varying count for comprehensions.
2. **Consumer-completed control flow.** Done (docs/QUOTATIONS.md): `if`,
   `while` and `times` inline pending quotations, so the checker sees plain
   branches. `map` waits for arrays.
3. **Typed quotation parameters.** A word that calls a quotation it was given
   declares or infers that quotation's effect, so it stops being a dynamic
   call.
4. **Families** resolved by type at compile time: done for numbers (slice 2
   above); runtime dispatch on unknown types waits for tagged values.
