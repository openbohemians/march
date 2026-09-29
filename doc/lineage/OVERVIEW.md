# March lineage overview: march1 to march6

Written 2026-09-26 for Thomas, to bring the five earlier lineages together
before deciding march6's direction. Each earlier lineage was read in full
(every document, the source, the archived branches listed in `VERSIONS.md`)
and built and tested in a scratch copy. The per-lineage digests, with file and
line citations, are beside this file: `march1.md`, `march2.md`, `march4.md`,
`march5.md`. March3 is summarized here directly (two design documents).
Key claims in each digest were spot-checked against the sources.

A recurring caution first: in every earlier lineage the documents claim more
than the code does. Status files are stale, "working" features are dead code,
and no lineage before march6 recorded a single performance measurement. The
digests separate "implemented and tested" from "designed only" throughout.

## 1. The lineages at a glance

| | Dates | Language | Execution model | Evaluation | State of the code |
|---|---|---|---|---|---|
| march1 | 2025-10-05 (one day) + an archived WIP session | Rust | Interpreter that re-parses a word's stored source text on every call | Strict; explicit `{ }` thunks, never implicit | Tests 19/31 at v0.2.0; working tree 15/31 (a regression makes user words uncallable) |
| march2 | 2025-10 | Rust | Two different interpreters (see below) | Strict | A: 45 assertions pass, several test files error. B: 49/49 `march2 test`, but false type errors and a broken database round trip |
| march3 | 2025-10-08 | none | Design only | Explicit execution (`foo .`) | Two documents |
| march4 | 2025-10 to 2025-11 | C + x86-64 asm | Strict FORTH stack VM, direct threading, compile-time types | Strict | Arrays working after late fixes; C unit tests no longer compile at HEAD; `( 42 ) execute ( 99 ) execute` segfaults |
| march5 | 2025-11 to 2026 | Rust | Recursive on-demand evaluator over a content-addressed node graph; the interaction-net code is an isolated scaffold never used to run a program | Inputs eager, untaken branches skipped, per-call caching | 63/63 tests pass; several real bugs reproduced by hand |
| march6 | 2026-09 | Rust | Words compiled once into data-flow graphs of slots; lazy demand cells; strict scalar fast paths and tail loops | Lazy by default | ~600 tests; measured; store and state sequencing built (uncommitted) |

## 2. Each lineage in brief

**march1.** A value stack plus a parallel type stack. Definitions are hashed
into lists of word hashes and stored in SQLite, but execution ignores the
hashes and re-parses the source text; the hash-based threaded runtime is never
called. `{ }` stores raw source with no environment and no memoization;
`force` re-runs it against whatever global state exists. The design's
predicate-guarded families were never wired up; a later type-signature
dispatch looks candidates up in a column that is never written, so it cannot
match. Two design ideas are worth keeping: every word as
`(State x -- State y)` threaded implicitly (the same idea as march6's state
chain), and per-definition read/write sets with an import-time state rename
map (only the rename table was built).

**march2.** Two interpreters described together in the documents.
Interpreter A (`archive/march2/main`, one 2,160-line file): line-at-a-time
evaluation with late binding by name, values carrying their types, runtime
multi-dispatch with type variables, a subtype hierarchy, and `?` guards. It is
the only lineage where context-oriented dispatch actually worked: guards run
on a forked stack with a throwaway state overlay, and `raise T` re-enters the
same family with an error-typed argument so an error-typed variant acts as the
handler. Interpreter B (`bootstrap-forth`): a four-day rewrite where `:` and
`;` are ordinary immediate words, with a namespace stack, content hashes over
word bodies, SQLite, and a test runner. It never reached guards, overloading,
loops, or recursion (words are copied in by value, so recursion fails). Rough
timings taken during this review: about 37 ns per loop iteration in A and
115 ns per word call in B.

**march3.** Design only. Bare identifiers push symbols; execution is
explicit (`foo .`), variables are explicit (`@` reads, `!` writes), a type
stack mirrors the data stack for checking and dispatch, and top-level sigil
operators declare imports, exports, state, signatures and context
constraints. The document ends by asking whether the symbol model is worth
its noise compared with a more traditional FORTH.

**march4.** The most machine-level lineage: a strict FORTH VM in hand-written
x86-64 assembly, 64-bit tagged code cells through one dispatch loop, untagged
data stack, all type checking at compile time. Code is stored in SQLite as
SHA-256 blobs with primitives by fixed ID and relinked on load. The only
deferral is at compile time: word bodies are kept as tokens and compiled per
call site with the caller's concrete types. Memory management was attempted
three ways at compile time and never freed anything (the runtime `free` is a
stub). The workspace-heap and global-store split, `freeze`, uniqueness
inference, namespaces, effects and I/O were design only. `_` pulls values from
below `[` into array literals.

**march5.** Words are stored as graphs of content-addressed nodes (CBOR) and
run from a root node on demand, re-reading nodes from SQLite as it goes.
Effect tokens are real: one linear token per effect domain orders side effects,
and the root's dependencies keep effects alive under on-demand evaluation. The
interaction net (`src/inet.rs`) is a four-test scaffold reached only by storage
commands; `docs/INET.md` says "IGNORE FOR NOW!". Namespaces, interfaces,
bindings and lockfiles are designed in depth but not implemented. The global
store is an in-process map persisted only as snapshots. Reproduced bugs
include primitives with equal signatures colliding on one identity, lost
namespace exports, and multi-result order disagreements.

## 3. Threads across the lineages

| Question | march1 | march2 | march4 | march5 | march6 |
|---|---|---|---|---|---|
| Strict or lazy | Strict; explicit thunks | Strict | Strict | Eager inputs, on-demand from the root | Lazy by default |
| Effects ordered by | Global mutation in token order | Execution order | Instruction order (no I/O existed) | Linear effect tokens | Implicit state chain alongside lazy values |
| Dynamic quotation call knows its stack effect | Never declared or inferred | Never checked | Designed (typed quotations `( _i64 _i64 + )` carry their signature); never checked; segfaults | Only static calls exist | Declared counts on `apply`; inferred for static `call` |
| Context-oriented (guarded) dispatch | Designed; never wired | **Worked** (interpreter A) | Type specialization only | Pure guards; dispatch node tries candidates in order | Guarded families implemented |
| Global store / namespaces | Alias table; state rename map designed | Namespace stack (B) | Dotted names only | Designed in depth; in-process map | CHAMP store with identities, built |
| Memory | Not addressed | Not addressed | Three compile-time attempts, nothing freed | Not addressed | Boundary collector, tail loops |
| Self-bootstrapping | No | `:` `;` as immediate words (B) | No | No | Stream nucleus, reflection, seed rebuilds itself |
| Performance measured | No | No (rough timings taken in this review) | No | No | Yes, systematically |

Observations:

1. **Lazy by default is new to march6.** Every earlier lineage executed in
   program order. That is why the questions now being worked on in march6
   (effects needing a separate chain, `drop` executing nothing, `apply`
   needing declared counts) never came up before.
2. **Dynamic quotation stack effects were designed but never implemented.**
   march4's `docs/design/QUOTATIONS.md` designed typed quotations: inputs
   declared with markers, as in `( _i64 _i64 + )`, and outputs inferred from
   the body, so the quotation value carries its full signature
   (`i64 i64 -> i64`) into its stored identity. The implementation never
   checked it (`execute` pops one value and a mismatch crashes). The design
   answers "what is this quotation's signature"; what remains is the
   application site when the quotation is only known at run time. With a real
   run-time stack (options B and C below) the carried signature is enough:
   `apply` reads it and pops that many values, with no counts written. In
   march6's statically wired graph, the application site's shape must be known
   at compile time, so the signature must flow as a type to the application
   site, or be checked at run time against a declared shape.
3. **Effect tokens were the one effect mechanism that was actually built**
   before march6 (march5), and march6's state chain is the same idea.
4. **Context-oriented dispatch has one working precedent**: march2's
   interpreter A, including guards on a forked stack and errors re-entering
   the family. It is worth mining for march6's families.
5. **Namespaces and the store were designed repeatedly** (march1, march2,
   march5) and first built in march6.
6. **The failed interaction-net attempt never ran a program as a net.** What
   march5 actually ran was an on-demand graph evaluator, closer in spirit to
   march6 than to interaction nets.

## 4. The decision in front of us

The central choice is the evaluation model, because the store and the
compiler are about to be unified on top of whichever one stands.

**A. Keep lazy by default (march6 as it is).** Effects run on the state chain
in program order; pure values stay lazy and shared. Remaining work: put
`apply` on the chain so dynamic code's writes run in order; infer `apply`'s
counts wherever the quotation's origin is known and require them only for
truly unknown code; document that dropped values are never computed.
Gains: sharing and unused work skipped for free, which is what the store,
staging and big-data plans lean on. Costs: a graph evaluator at run time and
a two-track mental model (lazy values, ordered effects).

**B. Strict stack execution with explicit deferral (march4's model, done
properly).** Words execute in order on a real stack; `drop` pops a computed
value; effects are ordered for free; laziness only through quotations and
explicit deferral, with shared cells where deferral is used. Costs: rewriting
the execution core, losing automatic sharing and skipping of unused work, and
the dynamic stack-effect problem remains (march4 never solved it either).

**C. Strict within a word, lazy across explicit boundaries.** Code executes
in order by default, and laziness is attached to specific constructs:
quotations, guarded family bodies, lazy data such as streams, and store
reads. This is closest to the original wording of `DIRECTION.md` ("shared
demand cells for genuinely deferred work"). It keeps effects simple and keeps
laziness where it pays, at the price of making laziness something the
programmer chooses.

My own reading, offered as input rather than a recommendation to act on
before you decide: A is working and measured, and its rough edges are
fixable; C is the most FORTH-like model that keeps the benefits you have been
counting on; B gives up the most. Whichever you choose, the dynamic
stack-effect question needs an answer. march4's typed-quotation design is the
starting point: under B or C it is essentially sufficient; under A it needs
the signature to flow to, or be checked at, the application site.
