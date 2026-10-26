# Memory

Status: proposed 2026-10-26. The model march8 has built since 2026-10-25,
the rules that keep it whole, and what is left. Thomas asked whether
"memeory management might be so fundamental that creating a language with
out it being worked out first is a fools errand." The answer here is that
the language decides what memory management can be, and March's
decisions already allow the simplest kind; what is left is speed.

Memory is 2.0 (Thomas, 2026-09-25): section 5 waits until a program needs
it. The rules of section 3 hold now, for every feature, the last a goal
until counting by node is built (5).

## 1. The model

Every value that is not a number is counted: how many holders it has. A
holder is a link, a place on the stack, a bracket's letter, an element of
an array, a field of a record, a path in the store, a literal in code
(REPRESENTATIONS.md 4).

- **Counted by the code.** The stage knows by its type which cells are
  values, so it says: a copy `hold`s one, a drop `release`s it, and a
  container made says which of its cells are values, `holds`
  (march8/docs/MACHINE.md, "Code").
- **Freed at the last.** A value with no holder left is freed, and what it
  holds is let go of with it.
- **Changed in place when alone.** An edit of a value with one holder
  changes it; else it makes a version that shares what it does not change.
  No one can tell: a value cannot change, so a share and a copy are one.
- **Always right as the program runs.** The compiler may remove counts it
  can show change nothing, but never frees on its own say-so. A count it
  does not remove costs a hold and a release, never a wrong answer.

This is how Lean 4 and Koka manage memory: Ullrich and de Moura,
"Counting Immutable Beans", 2019; Reinking, Xie, de Moura and Leijen,
"Perceus", 2021. Both count every value, change in place when alone, and
let the compiler remove what it can.

Built 2026-10-25 (commit 73eaedf): `lower` on itself peaks at 258 KB, not
16.8 MB, and runs 37% slower, most of it one pattern (5).

## 2. Why counting is enough

Counting fails on a loop of links: two values that hold each other keep
their counts above nothing when no one else holds them. Languages with
loops add a collector that searches for what is unreachable (Python), or
links that do not count, which the programmer must place (Swift).

March has no loops. A value never changes, so it holds only values made
before it. A word that applies itself does so by its name, a symbol looked
up (SYMBOLIC-FORTH.md 6.16), not by a link to its own code. So a count that
reaches nothing is always right, and nothing ever searches.

## 3. The rules

What keeps section 2 true. A feature that breaks one needs its own answer
before it is built.

| Rule | Where decided |
|---|---|
| A value never changes; changing in place is the machine's, only when alone | REPRESENTATIONS.md 4 (Thomas: "if immutable is anything really shared?") |
| A value links only to values; a place is reached by its path, and a path is a name, not a link | SYMBOLIC-FORTH.md 6.16 |
| No closures: a quotation holds only what was known where it was made | SYMBOLIC-FORTH.md 6.14 |
| Code is data only while compiling, so no code is made of values as the program runs | SYMBOLIC-FORTH.md 6.30 |
| No word for speed or safety: a program never says anything about memory | REPRESENTATIONS.md 4 (Thomas, 2026-10-26) |
| A value has no identity but its content, held canonically | TYPES.md 2.11 (Thomas: "*canonical* for proper comparison") |
| A count the compiler misses costs a constant, never a pass over a value | here |

The last answers Thomas's "that sounds fragile", of the compiler taking a
record's field (REPRESENTATIONS.md 4): slow, with nothing to say why,
where a change nearby defeats it. Where the compiler cannot show a
value is alone, the program pays a hold and a release, or a version
sharing its tree, a path of nodes. So a change nearby costs little, not a
copy. Two things follow: a flat form, which a shared edit copies whole,
only below a measured size (REPRESENTATIONS.md 8, open 1); and counting by
node, since one case breaks the rule today (5).

## 4. Where it would break

What would need a rule, when it comes:

- **Laziness.** A lazy value that refers to itself, a stream defined by
  itself, or a real's digits made on demand (LAZY-NUMBERS.md), is a loop.
  Proposed: it refers to itself through a name, as a word does, never by
  a link.
- **Threads.** A count changed by two threads at once must be changed
  atomically, which costs more. Lean marks a value when it reaches
  another thread and counts only those atomically (REPRESENTATIONS.md 8,
  open 8).
- **Traps.** A trap throws away what is on the stack between it and its
  handler, and each value must be let go of: by a table beside the code,
  as where a trap happened is to be (TODO).

Outside things, files and sockets, are no new case: a count frees one at
its last holder, at a point known in the program, which a collector does
not.

## 5. What is left

None of these changes the model. Each is a speedup or a leak closed,
built alone, and checked against the counts as the program runs: the
same output, the same peak.

| Piece | What it touches | What it is for |
|---|---|---|
| Counting by node | arrays and maps in the machine; merkle-champ, a way to walk a version's nodes with their counts | a version holding its elements once more, a pass over them: most of the 37%, and quadratic where a record holds an array pushed to (0.42 s at 10,000 strings, 6.1 s at 40,000) |
| Reads need no count | a summary for each word, of what it does to each input, uses it up, keeps it or only reads it, from the primitives up | speed: a hold and a release removed where an input is only read, as Lean's borrowed inputs are |
| Counts known while compiling | the stage carrying a count beside each type | speed: a value never copied freed at its drop, nothing counted (REPRESENTATIONS.md 4's table) |
| What a trap throws away | a table beside the code | leaks where a trap is handled |
| Atomic counts | values that reach another thread | threads, when there are any |
| Native code | nothing: `hold` and `release` are ops, compiled as any other | |

## 6. Not March 4

March 4 decided every free while compiling (docs/design/MEMORY-MANAGEMENT.md,
on the branch archive/march4/preserved/docs-2026-09-23). Nothing could be
freed until the analysis was whole and right, and it stalled. Here the
counts as the program runs are the model, and what the compiler shows only
removes some: each piece of section 5 is a speedup, and none of them is
needed for a program to run.
