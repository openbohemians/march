# Representations and sources

Status: a study, 2026-10-25. Nothing here is decided but section 10's
verdicts and section 11's reading, agreed 2026-10-26 (Thomas: "Yes, good
analysis."). Thomas proposed both halves: that a value is held the
simplest way first and moved to another where what is done to it needs
one, and that what the compiler cannot know comes in only through
channels in the state, which it can trace.

Two questions, then: how a value is held, and what of it is known while
compiling. The second answers much of the first.

## 1. Today

march8 chooses nothing. Every array is a merkle-champ sequence, the
content-defined tree that replaced merkle-vector, and so is every tuple
and record, which are arrays as the program runs (TYPES.md 2.11). Every
string is a sequence of its bytes, hashed as it is made so that equal text
is one handle. Every map is a CHAMP map. Only the tail an array grows
while it is gathered is flat.

Since 2026-10-25 a version made by an edit is charged what it adds, not
its whole length (MACHINE.md, "What it found", 10), so the trees cost
what they use.

Measured 2026-10-26, nanoseconds an element, the sequence against Rust's
`Vec` of u64, then the same in march8, where its own loop costs about
130 an element:

| | 10,000: `Vec` | sequence | 1,000,000: `Vec` | sequence | march8, 1,000,000 |
|---|---|---|---|---|---|
| made | 0.1 | 6.7 | 0.2 | 6.4 | 10, `range` |
| walked | 0.1 | 0.5 | 0.4 | 1.1 | |
| read by index | 0.1 | 8.8 | 0.3 | 22 | 127, `sum` |
| mapped | 0.2 | 7.5 | 0.5 | 8.2 | 159, `[ 2 * ] map` |
| an element set | 0.3 | 3,029 | 0.6 | 4,339 | no word yet |
| appended | 0.6 | 1,283 | 0.6 | 2,041 | 2,347, `-1 insert` |

Reading and mapping, the tree is a tenth of march8's time or less, the
loop being the rest; compiled to native code, it would be most of it. An
append or a set is a thousand times a `Vec`'s, the tree making a leaf
again each time, and is nearly all of march8's: an array that grows, as
`lower`'s tokens do, pays it on every element. So an append to an array
nothing else holds now goes onto the flat tail the machine keeps for
`map`, joined to the tree once, where the array is next needed whole: a
million appends take 197 ns each, not 2,347, the loop's 133 nearly all of
it (2026-10-26). `lower`'s are not alone, the record holding the array it
appends to (4). The idea of choosing is older: TYPES.md, open question 4,
says a string's structure is "a representation chosen by use, not a
type", and MACHINE.md's "Not yet" has "Representations chosen by use"
(2026-10-09). Neither says how.

## 2. Simplest first

Thomas (2026-10-25): "You should always assume the simplest first, vec#
-- and here is where we need efects. if a word acts on that vec that it
does not know, or is considered highly inefficent (that's more subtle
optimization), then it retargets the type as a type of ary# that can
efficently handle the operation, e.g. a large vec (the subtley is if is
small vs large) does not insert well, so that retages to a linked-list or
a merkle-sequence (depending on how else it gets used -- what all the
effects are)."

So a value is made flat, and a word applied to it that the flat form does
badly moves it to a form that does it well. The type the program sees does
not change, `< i64 ary >` is still `< i64 ary >`: the form is the
compiler's choice, beneath the type, as a literal's kind `str#` settles to
the type `string` (TYPES.md, open question 4).

| Form | What it is | Good at |
|---|---|---|
| `vec#` | flat cells, one after another | reading by index, length, walking, appending in place |
| `list#` | linked cells | adding and taking at the front |
| `tree#` | a merkle-champ sequence | versions that share, edits in the middle, content identity |

The names are placeholders. `seq#` is taken: it is the sequence literal's
kind, `( … )` being `[ … ] seq# !`. Section 10 weighs every form, `list#`
not among those worth having.

## 3. What decides: what is done to a value

| Done to the value | `vec#` | Moved to |
|---|---|---|
| read by index, length, walked | best | — |
| changed, the old version used no more | best: in place, an append amortized O(1) | — |
| changed, the old version still used after | a whole copy each time | small: stays; large: `tree#` |
| an insert or remove in the middle, large | O(n) each | `tree#` |
| added to and taken from at the front only | O(n) each | `list#` |
| stored in the global store, or compared by identity | hashed each time | `tree#`, whose identity is there already |

Small or large is Thomas's subtlety. A length known while compiling, `< 3
i64 vec >`, settles it; a bound would; otherwise the program checks as it
runs, or the compiler assumes large.

**What is done, written in the lowered type** (2026-10-26). Thomas: "the
idea that a sequence tagged as appends is one that needs to be an
appendable optimized concrete type. Sure any array can be appended to via
copying it, and for a small array that's fine. So maybe this isn't
needed, I just thought it might be an easy way for complier to track the
capabilities in the program flow." In straight code the stage needs no
record of it: it evaluates each word where it is applied, so it sees the
value made and every word applied to it, and moves the value at the word
that needs another form (2). A record is needed where a value's form must
be settled before the code that uses it runs: a loop's accumulator, a
recursive word's input, a value put in the store or in a structure. There
the stage would run the body, see what it does to the value, `push`
appending, `insert` inserting in the middle, `at` reading by index, and
write it in the value's lowered type, `[ … ] seq# ! appends# !`, as it
settles a loop's types now by running the body. Which word does what
would be facts in core.march. A program never needs to write one, and may
to force it (Thomas: "not necessary for the programmer to do unless they
wish to interject and force it"). The form comes of what is done and the
size together, by this table. It waits on a second form, every array
being a tree today (1); a property, what a value is that its
representation cannot show, is the language's, TYPES.md open question 17.

## 4. Sharing

Most of the table turns on one thing: whether the old version is used
after the change. `lower` (march8/compiler/lower.march) shows how subtle
that is. A token is added as

```
rd toks' rd.toks { spelling: text line: row } push put
```

which reads the array out of the record, appends to it and puts it back.
When the append happens, the record still holds the old array, so it is
shared, and a flat `vec#` would copy every token so far, on every token:
worse than the tree it has now. But the record is used no more after the
`put`, so a rule could see the pattern, a field read, changed and put
back, the record dead after, and change the field in place. March writes
rewrite rules in March already (SYMBOLIC-FORTH.md 6.25), so the rule
could be one. Then the array is never shared and stays flat.

So the compiler must know, while compiling, where a value is copied
(`dup`, `over`, `pick`, a name used twice, a value held by a structure and
on the stack) and where it is last used. That is what freeing needs too
(memory management, 2.0): this is where representation and memory meet,
and the one analysis serves both.

### Freeing by counting

Thomas (2026-10-25): "if immutable is anything really shared? each can
present it has it's own copy. and we have to know if it is in order to
know whnen to free it -- we need to know all refs are done with it. … I was
thinking a drop unlinks a link in a list or a tree, thus removing itself
from the path of oweneship. which is exactly deletion for the store.
freeing memory."

In meaning nothing is shared: a value cannot change, so no holder can
tell a shared value from a copy. Freeing is where sharing shows, since a
value may go only when its last holder is done with it. A holder is a
link: a place on the stack, a bracket's letter, an element of an array, a
field of a record, a path in the store, a literal in code. A `drop`
removes one, and a value with none left is deleted, its own links going
with it. Only their number need be kept, a count of holders. A value
holds only values made before it, so links never make a loop, and a
count that reaches nothing is always right: nothing need ever search for
what can no longer be reached.

Thomas: "We were trying to do compile time counting. Comptime analysis
should make some of it obvious. Perhaps it is the ones never dupped."

A value never copied has one holder all its life, and the compiler sees
where it ends, at its `drop` or the word that uses it up. It is freed
there, and nothing is counted as the program runs. More widely, the
compiler can count copies as it carries types, wherever the count at a
point is the same on every path to it:

| A value | Its count |
|---|---|
| never copied | known: freed at its `drop`, or where it is used up |
| copied, the copy only read and gone, `dup length` | known: one holder still |
| copied among places on the stack and letters, each dropped | known: freed at the last |
| held by a record's field, its key known while compiling | known, as the record's is: a field is a place too |
| read out of an array at a position known only as it runs | kept as it runs |
| copied a number of times a loop decides | kept as it runs |
| kept by one clause a guard chooses and not by another | kept as it runs, unless each clause frees its own |
| under a path in the store | kept as it runs: it lives until the path is set again |

This was March 4's design (docs/design/MEMORY-MANAGEMENT.md, on the
branch archive/march4/preserved/docs-2026-09-23): the stack of types
while compiling as the holders, a value freed where it leaves the last
of them. It stalled on the C compiler's shared state, not on the idea.
march6 added two corrections: a place in a loop makes a new value on
every pass, so a value is named by its pass, not only by where it is
made; and each word needs, for each input, what it does to it, uses it
up, keeps it in what it leaves, or only reads it. That is section 5's
need too.

**Measured, 2026-10-25.** `lower` lowering itself, the machine counting
every value's holders exactly, the stack's and scratch stack's places,
array elements, map entries and literals in code, by an instrument since
removed. Records are arrays as the program runs, so a field is an
element.

| Values | Objects | Bytes charged |
|---|---|---|
| never copied | 40,599 (36%) | 605 KB (4%) |
| copied among places on the stack only, by `over`, `pick`, `dup` | 34,696 (30%) | 2.1 MB (12%) |
| read out of a record while it holds them | 25,478 (22%) | 12.6 MB (75%) |
| held by a version and the one it was made from, by `put` or `insert` | 12,346 (11%) | 1.4 MB (9%) |
| read out of a larger array, or a string equal to one a structure holds | 953 (1%) | 37 KB (0.2%) |
| literals in code | 150 | 2 KB |

The rule alone, the ones never copied, frees a third of the values but
only 4% of the bytes. With copies among the stack's places, two thirds
and 16%. Three quarters of the bytes are one pattern, the one above: the
token array read out of the reader's record while the record still holds
it. Its key is known while compiling, so it is counted while compiling
too, if a record's fields count as places. Then all but 0.2% of the bytes
are counted while compiling, in this program. What is left is what an
index read at run time reaches, and strings, which are one handle for
each text.

Freed exactly, the run's peak would be 258 KB, not 16.8 MB, 65 times
less, and 1.9 KB would be left at the end. The instrument counted as the
program ran, so whether a count is known while compiling is the first
table's reading of each case, not measured; a guard or a loop can make a
stack copy's count depend on the run.

The record read is also where changing in place turns. A field taken
out, the record giving it up, rather than read, and the token array is
never shared: freed by its count, and changed in place by the rule
above.

**Built, 2026-10-25,** counted as the program runs first (Thomas: "If it
is simple enough, yes build the freeing", and "yes build it this way and
see how it goes"). Which cells are values the stage knows by their types,
and the machine does not, so the stage says: a copy of one `hold`s it, a
drop of one `release`s it, and a container made says which of its cells
are values, `holds`; every other primitive lets go of what it takes, as a
drop would, so all counting is `dup`'s and `drop`'s (march8/docs/MACHINE.md,
"Code"). A value nothing else holds is edited in place. `lower` on itself
peaks at 258 KB of values, as measured above, and 2 KB is left at the end.
It runs 37% slower, 0.92 s, not 0.67, most of it the pattern above: each
version of the token array made while the record still holds it holds its
thousand strings once more, 25 million holds in all.

**Taking the field, refused, 2026-10-26.** Two ways to take it were put
to Thomas. The compiler taking it itself, where it can show that nothing
reads the record between the read and the `put`, nor copies what was
read: "that sounds fragile", slow, with nothing to say why, where a
change nearby defeats it, and wrong where the showing is. And a word for
it, `change`,
`rd toks' tok [ push ] change`, the field taken out, changed and put back:
"The fact that you are making an esoteric word that a programmer has to
use for speed and *safety* just to work with a record is a big fat *red
flag*." So the line stays as it is written, and the cost goes beneath it.

The cost is not a copy. A version made while the record holds the old one
shares its tree; the pass is the counting, each version holding every
element it shares, and each let go when the old version is freed. Counted
by node instead, a leaf holding its elements once and a version holding
the nodes it makes, a version costs what it changes, whoever holds the one
it was made from. merkle-champ's nodes are counted already, as Rust's
`Arc`s; the machine needs a way to walk a version's nodes with their
counts, a change to the crate.

## 5. Two ways

Thomas: "Of couse there are two ways to handle that and they can
intertwine. make it the resulting retarget type always or convert it from
one ot the other where certain effects hold, but don't in other areas."

1. **Chosen where it is made.** The value is made in the form everything
   later done to it wants. This needs each word's effect to say what it
   does to each of its inputs, so the compiler sees, where a value is made,
   what the code it reaches will do.
2. **Moved where it is used.** An `insert` applied to a `vec#` not known
   small moves it to `tree#`, and its result is a `tree#`. A move is O(n),
   once.

They meet in loops. The stage joins a loop's types to one that holds on
every pass, as it settles a record literal in a `fold` (MACHINE.md). The
same join would carry a move made inside a loop to before it: the
accumulator is made `tree#` once, and every pass after finds it so. That is
the first way's result where it matters most, without seeing the whole
program.

**Leaning:** the second way first, with the loop's join; the first once
words say what they do to their inputs. Moving back, `tree#` to `vec#`
where only reads follow, is the subtle part, worth it only for many reads.

## 6. Sources

Thomas (2026-10-25): "I think the key to understanding dependencies with
the code is source tracing. where are the undecidable inputs coming in
from the. The key is to make all outside inputs a channel in the state."

Every word is a function of the stack and the state, the global store.
An input from outside, standard input, a file, the environment, the
clock, the network, is then a channel in the state, and reading it is a
read of the state: an effect, as `io.stdin.read` already is, a read of the
`io` domain, which is never folded (MACHINE.md, "Effects").

A channel is read at one of two times:

- **While compiling:** `api-key: [ "API_KEY" env ] ! ;`. `[ … ] !` runs
  code while compiling (SYMBOLIC-FORTH.md 6.32), so the key is known, and
  what follows folds on it. Thomas wrote `]| … |[` for it.
- **Live:** `api-key: "API_KEY" env ;` reads it each time `api-key` is
  applied. Ideally its value would follow the variable, a change trapped
  as an event, which is event-driven programming, beside the point here.

`env` is no word yet.

Nothing else is undecided. Values never change, and a quotation is
consumed while compiling, so no code is chosen as a value at run time
(MACHINE.md, "Quotations at run time"). Given its channels, a run is
determined.

### Where the undecided acts

Thomas: "Those guards are where branching occurs, and that is the primary
source of indermitism. Are there other places where that enters the
system?"

It enters only through channels, and through one source no program
declares, the machine. Where it acts:

| Where | How it acts |
|---|---|
| a guard | which clause runs: a guard on a value read is a choice at run time (MACHINE.md) |
| a clause chosen by type | a union split as the program runs: `"12" i64!` is `i64 nil or` |
| a loop or recursion | how many times: a `fold` over an array read in, recursion until a guard holds, a branch on every pass |
| a trap | whether an operation fails, a key missing, a division by zero, an overflow, a parse; a handler is a branch taken by failing |
| a size, a position, a key | no branch, but the shape is undecided: an array's length, the element `at` reads, the key `put` sets; these choose a form, small or large, and whether a value is shared |
| the machine | fuel, memory and their limits, and the clock if one is read: inputs outside the program's state |
| *the order of changes* | *not yet*: live channels would bring changes in an order of their own |

The first four are branching; the fifth is not, and matters most to
section 3; the sixth is outside the program. A seventh is closed by
design: which code runs is never a value, where a language with code as
data at run time has it everywhere (SYMBOLIC-FORTH.md 6.30).

## 7. Tracing them

The stage traces two sources now: a value is known, a literal or what
folds from literals, or it is `Run`, on the machine's stack (stage.rs,
`Val`). Tracing splits `Run` by what it depends on: which channels, or
none, through data, a result depending on its inputs, and through control,
a value made in a clause that a guard chose depending on what the guard
read. It would give:

- **Decision points.** Each guard, split, loop and handler on a channel's
  value is one, and the code between two is straight, what it does to each
  value known. A form can be chosen for each stretch, the areas "where
  certain effects hold" of the second way.
- **Sizes.** A length traced to its source is known, bounded, or a
  channel's, which settles small or large.
- **A live word's reach.** When a channel changes, only what depends on
  it need be found again, on demand.

## 8. Open

1. Small or large: where the line is, and whether a run-time check is
   worth its cost.
2. `list#`: whether front-only use is common enough to want it, or
   `tree#` serves. `tree#` serves (section 10).
3. Moving back to `vec#`, and when it pays.
4. Strings: flat by default too, hashed only when compared or stored,
   rather than as each is made.
5. Maps: a small one as a flat sorted `vec#` of pairs. Yes (section 10),
   below a size to be measured.
6. Identity of a flat value: hashed when asked for, or kept.
7. Channels: their names under `io` (`io.env`, `io.clock`), and what a
   live channel's change does.
8. Counts across threads: changed atomically where two threads hold a
   value, or values passed between threads only through State, the
   store counting them there.
9. A merge's rule for each type, and identities for a sequence's elements
   (section 11).

## 9. Order

1. Each word's effect on its inputs: what it does to each, read, append,
   insert, change, keep a copy, beside the effect rows' domains.
2. Copies and last uses, while compiling, a record's fields counted as
   places (section 4): freeing where a count is known, and a count kept
   as the program runs only where it is not.
3. `vec#` the default, moved to `tree#` where used, loops joining
   (section 5, the second way).
4. Sources traced: `Run` split by channel (section 7).
5. Chosen where made (the first way), and moving back.

Freeing came first, counted as the program runs (2026-10-25, section 4);
2 makes most of that counting go, and counting by node (section 4,
2026-10-26) makes what is left cost what changes, not what is shared.

## 10. Forms worth having

Thomas (2026-10-26): "We should also do ourselves a favor and clearly
enumerate the types of sequences and mappings possible that we should
support at minimum." He named the vector, the linked list, the doubly
linked list, the skip list, the rope, RRB and the merkle sequence, and:
"a small array will be better off as a copy", and "the ideal is for the
compiler to optimize deciding which to use when."

Two properties sort them. Whether versions can share a form's parts: a
value never changes, so a part two versions share is safe in both. And
whether the form is canonical, the same content the same shape, whatever
order the edits came in, so that equality is a comparison of hashes and a
merge passes over what is equal (section 11).

Sequences:

| Form | Good at | Against it | Verdict |
|---|---|---|---|
| Vector, `vec#` | reading by index, walking; a small array; in place where one thing holds it | changed while shared, a copy of n | **yes**: small arrays, and those one thing holds |
| Merkle sequence, `tree#` | versions that share; canonical, so equality by hash, diff, merge, the store | hashing as it changes | **yes**: march8's arrays now; the shared, the large and the stored |
| Rope | long text: joining, slicing and inserting in O(log n), lines counted | overhead on short strings | **yes, later**: a tree of text, measured in bytes, characters and lines |
| RRB vector | the fastest tree to index in working memory | not canonical: its shape depends on its history, so no equality by hash or merge | **not yet**: only if the merkle sequence is too slow where it runs hot |
| Linked list | the front in O(1), even shared | reading by index O(n), scattered in memory | **no**: the tree gives the front in O(log n), and the rest |
| Doubly linked list | a splice at a node in O(1), changed in place | each node points both ways, so no two versions share one, and the loops defeat counting | **no** |
| Skip list | changes in place made at once by many | randomized, so not canonical; versions do what it is for | **no** |
| Slice, a window on its parent | slicing in O(1) | keeps its whole parent | **yes**: `lower` slices strings at every step |
| Range | `n range` without its n cells | | **maybe**: an array made lazily, as lazy numbers are |

Maps:

| Form | Good at | Verdict |
|---|---|---|
| Record | keys known while compiling, read by position | **yes**: built (TYPES.md 2.11) |
| Small map, pairs in a `vec#` | a dozen entries or so, where a scan beats hashing | **yes**, its size measured (open 5) |
| CHAMP | versions that share; canonical | **yes**: march8's maps now |
| Hash table | a large map one thing holds, in a hot loop | **maybe**, if measured: not canonical, and copied whole when shared |
| Ordered merkle map, a prolly tree | keys in order, ranges of keys; canonical and mergeable | **maybe**: where order matters, State perhaps |
| Trie of paths | everything under a namespace, `core.…` | **worth considering** for State's keys |
| Skip list, red–black tree, AVL tree | | **no**: B-trees and tries do better |

A set is a map to nothing, merkle-champ's `ChampSet`.

So the compiler chooses among few: a vector where an array is small or
held once, the merkle sequence where it is large and shared, CHAMP for a
map shared or large, pairs for a small one, and later a rope for long
text, each moved to another as what is done to it needs (sections 3 and
5).

## 11. Transactions

Thomas (2026-10-26): "But there has to be transactional safety. We can't
have an operation that fails because something changed in the midst of
that operation taking place." And: "I was thinking about concurrency and
wondering if our merkle tree lends itself to CRDT style *transactional*
memory."

**Within a run** nothing changes under an operation. Every value is a
version, never changed once anything else can see it: an edit is made in
place only where the value is alone, so no one sees it happen. A handler
holds its inputs, so what it would put back is not alone and is not
changed in place, and the session holds its stack while a run may fail
(march8/tests/freeing.rs).

**Across threads** the same rule gives each a snapshot. A transaction
reads a version of State, a root, works on it, and that version cannot
change under it. What is left is the commit, when another has committed
since. Software transactional memory, Haskell's and Clojure's, runs the
transaction again, which is the failure Thomas rules out. A merge
instead: three ways, the version begun from, this one and the other.

**The merkle trees lend themselves to it.** merkle-champ is canonical, so
the equal parts of two versions have equal hashes and a merge passes over
them: it costs what changed, not the size, as Noms and Dolt merge prolly
trees.

- **Maps.** Changes to different keys merge of themselves. The same key
  changed both ways needs a rule, by the value's type: counters add, sets
  join, a register keeps both or the later. These are CRDT semantics, and
  with a rule for each type a commit cannot fail.
- **Sequences.** Positions shift under inserts made at once, so a merge by
  position holds only where the edits do not overlap. A sequence CRDT, RGA,
  Fugue or Yjs's, gives each element an identity of its own. To be
  designed (open 9).
