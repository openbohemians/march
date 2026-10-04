# First slice: concrete interfaces and limitations

2026-09-28. This records implementation details of the authorized initial slice,
not a declaration that every foundation milestone is complete.

## FORTH working memory

Each machine initially owns a zeroed 1 MiB working region, **virtual region 1**.
This is the seed's boot convention, not a process address or a new language
“session” object. Region 1 is recreated on boot and cannot be released. An address
is two stack cells, `(region, byte-offset)`; it occupies two cells when stored.
Other region handles contain a slot and generation. Freed slots are reused with
a new generation, so stale handles remain invalid for reads, writes and release.
The checked interpreter uses the low 32 bits for the slot and high 32 bits for
the generation; words must treat the handle as opaque. Offsets still have their
own full cell. A slot is permanently retired rather than wrapping its generation.
Code tokens use a distinct table, with zero invalid. Neither kind of handle is
serialized as a live reference.

This intentionally simple convention locates FORTH's ordinary compiler variables
without adding a host-owned compiler-state object or named-variable service.
Changing it later requires updating the seed and its boot contract explicitly.

`seed/system.asm` defines the working layout, using byte offsets:

| Offset | Meaning |
|---|---|
| 0 | `STATE`: executing or compiling |
| 8 | `LATEST`: newest dictionary entry offset |
| 16 | `HERE`: next byte in the current definition's output region |
| 24 | Current output region, or zero |
| 32 | Next free dictionary byte, initially 4096 |
| 40–72 | Input region/length/cursor and current word boundaries |
| 80–96 | Name of the unfinished definition |
| 128–440 | Private scratch for byte copies, lookup, number conversion and image data |

`STATE`, `HERE` and `LATEST` are words returning addresses. `@`/`!` use cell
addresses; `c@`/`c!` use byte addresses. The seed's private `get`/`put` helpers
access offsets in region 1. Its dictionary entries are ordinary March memory:
previous-entry offset, executable token, immediate flag, name length, name bytes,
then alignment padding. The Rust host knows none of those fields. (This is the
frozen generation-0 listing's layout. The system rebuilt from
`seed/system.march` adds a bucket link before the name, which then starts at
offset 40, and finds names through a hashed dictionary; see docs/REBUILD.md.)

`:` calls `word begin`. `begin` allocates a bounded 64 KiB output region and
records the name. `c,` emits one byte; `,` emits eight little-endian bytes even
when the output position is not cell-aligned. `compile,` writes a call operand
using the target word's CID, not its executable token. `literal` emits a literal.
`;` appends RETURN, completes/links the definition, and installs the new binding
only after its name and fields have been written. Then it releases the output
region. Unlike JonesForth's hidden-entry approach, this slice links the new entry
last; both keep unfinished definitions out of ordinary lookup.

Ordinary source calls pin dependencies at lookup time. The current source
interface is ordered, not an implementation of general concurrent compilation.
Independent machines booting from an unchanged image provide independent stacks,
dictionary memory and output regions. Dependency scheduling remains future work.

## Host interfaces, without Rust layouts

The private `Instruction` enum is only executable machinery. Canonical code
uses explicit bytes; it is not a dump of a Rust enum or struct. Multi-byte integer
operands use little endian. The code domain is `march7/code/v1\0`; the data domain
is `march7/data/v1\0`. A CID is SHA-256(domain followed by bytes).

| Opcode | Operand / behavior |
|---|---|
| 0 | Return |
| 1 | Eight-byte cell literal |
| 2 | One-byte primitive ID (explicit fixed discriminants in `Primitive`) |
| 3, 4 | 32-byte code CID: call / push execution token |
| 5, 6 | Four-byte absolute instruction index: branch / zero-branch |
| 7 | Call the current definition recursively |
| 8 | 32-byte data CID: push read-only region, offset zero, byte length |
| 9 | 32-byte code CID: tail call |
| 10 | Eight-byte float literal (IEEE-754 binary64 bits); runs like opcode 1 (docs/NUMBERS.md) |

The literal-42-plus-return bytes and their independently calculated CID are
pinned in a golden test, and another test pins every primitive ID. Decoding
rejects unknown opcodes/primitive IDs, truncated operands and invalid branch
targets. The linker checks dependency kinds and
resolves code CIDs to local tokens. Published data is copied into immutable
storage and exposed as read-only regions; writes trap. Code publication checks
decoding and dependency existence/kinds before insertion into the CAS. Rejected
code cannot contaminate subsequent image exports. Publication tracks stored byte
size incrementally; duplicate publication does not charge those bytes again.

Selected generic primitive contracts (leftmost item is deeper on the stack):

| Primitive | Stack behavior |
|---|---|
| `load8/load64` | `(region offset -- cell)` |
| `store8/store64` | `(cell region offset --)` |
| `work-load/work-store` | `(offset -- cell)`, `(cell offset --)`; cells in working region 1 (44, 45) |
| `region-new/free/size` | `(bytes -- region)`, `(region --)`, `(region -- bytes)` |
| `execute` | `(… token -- …)`; no system-level signature inference |
| `seal` | `(region offset length -- token)`; complete canonical code bytes |
| `code-cid` | `(token destination-region offset --)`; copy 32 CID bytes |
| `resolve` | `(cid-region offset -- token)` |
| `publish` | `(region offset length -- readonly-region)` |
| `blob-cid` | `(readonly-region destination-region offset --)` |
| `blob-read` | `(cid-region offset -- readonly-region 0 length)` |
| `trap` | `(error-number --)`; stop the invocation |

Primitive IDs and behavior are fixed within this experimental domain; code changes
that change meaning require a new domain/identity. Wrapping arithmetic is not a
promise about future Checked March's arithmetic. The system dictionary names
these operations `u+`, `u-`, `u*`, `u/`, `umod`, `ult?`, with no
signed-looking aliases. Negative literals denote two's-complement cell bits;
division and ordering still interpret those bits as unsigned. The CLI prints
unsigned cells. Primitive wire identities and semantics are unchanged by these
dictionary spellings. Unsupported larger-platform
features (general host imports, terminal I/O words) have not been hidden in the
assembler to compensate.

## Images and boot

The host image has a magic/version, entry CID, opaque data-root CID and a
CID-sorted table of code/data blobs. Lengths are explicit; hashes, duplicates,
dependency kinds and trailing bytes are checked. The host format has no dictionary
fields. The data root is a byte blob written/read by March.

Boot is called with `(data-region 0 length)` and returns three executable tokens:
evaluate input, recover after an error, export dictionary data. An empty initial
data root selects March's generation-zero dictionary initializer; a saved root
selects March's dictionary reader. Runtime token numbers are reconstructed from
CIDs. No native code reinstalls flags or name bindings.

The driver passes an immutable input buffer as `(region length)` to the evaluate
word. It saves the caller's operand cells before that request. On failure it
restores those cells and calls the March recovery word, which releases unfinished
output and resets compilation mode. The driver owns no dictionary rollback.
Earlier completed definitions and arbitrary memory/I/O effects are not undone.
This is a narrow adapter, not a general CATCH/THROW facility. Cleanup failure is
terminal for that driver; hostile System March is outside the safety claim.

March serializes dictionary records oldest first: name length (u64), immediate
flag (u64), code CID (32 bytes), then name bytes. Export preserves shadowed
bindings; reload rebuilds working links and tokens. The working operand stack,
input buffers and unfinished definitions are not image contents. Recompiling a
word can add another dictionary record even when its CID is unchanged.

## Bounded resources and honest bootstrap status

Current caps: 16 MiB live region bytes and 16 MiB encoded CAS/image scale,
65,536 operand cells, 16,384 continuations, 65,536 instructions per definition,
one million linked instructions, 256 dependency-link depth, and 100,000 region
slots (including reserved/retired slots) and CAS objects, respectively. Released
region slots are recycled with stale-handle protection, not counted toward a
lifetime input limit. Dictionary workspace
is 1 MiB; names are 1–255 bytes. These are development limits, not total RSS
accounting. Fuel counts instructions; hashing/linking can cost more than one
simple arithmetic instruction. Nontermination tests additionally use process
timeouts. Counters retain cumulative allocation and peak live region bytes.

This slice proves defining/compiling words, persistent CIDs, failure isolation,
fresh-process reload and absence of the assembler from normal execution. Two
compiler words (`:` and `literal`) rebuild from March source with the same CIDs.
The whole interpreter is still supplied as a generation-zero instruction listing.
Converting it into self-rebuilding March source, controlling dictionary-history
growth and proving a whole-image rebuild fixed point are still work to do.

## Reuse and generation-zero boundary

- JonesForth's `WORD`/dictionary/`STATE`/`,`/colon-definition mechanism directly
  informed the March instruction sequences. March changes numbers-first lookup,
  canonical dependency operands and persistent identity; it does not reuse the
  i386 instructions or pretend source syntax is a Rust lexer.
- March4's persistent-code/executable-code distinction informs the linker. No
  executable trampolines or SQLite dependency were copied.
- March6 supplies behavior and test precedents for CID stability, early binding,
  numeric precedence, self-extension and failed publication. Its graph evaluator
  and tagged value representation are not dependencies of this crate.
- The SHA-256 dependency is reused. Merkle-CHAMP integration is not needed for
  this first dictionary and has not been fabricated or reimplemented.

The assembler accepts `word/end`, local labels, `lit/prim/call/quote/ref/branch/
zero/recur/tail/ret`, hex `data` and the two image root declarations. It has no
macros, March syntax, compiler-word execution or dictionary builder. Dictionary
construction is executable March code in the listing, analogous to JonesForth's
initial word bodies. The normal runtime library does not include this module.
Record formatted assembler and seed-tool line counts with the checkpoint, not
compressed source counts. Runtime execution/reload is tested with an empty PATH
and a temporary directory containing only images.

## Initial checkpoint

17 integration tests pass in debug and release; all-target Clippy with warnings
denied and formatting checks pass. Formatted generation-zero tool size:
`tools/assembler.rs` 162 lines plus `tools/seed.rs` 16 lines. The March instruction
listing is 1,396 lines. These counts are recorded to make growth visible, not to
claim that line count proves minimality.

Two local, unpinned release runs of `examples/bench.rs` (50,000 warm invocations)
observed roughly 271–277 microseconds for load/validate/link/boot, 56–58
microseconds for compiling and looking up `square`, and 25.1–25.2 nanoseconds per
warmed call. Checksum: 41,665,416,675,000. This includes invocation dispatch and
stack operations, excludes source compilation from the warm loop, and establishes
no application-wide performance claim or comparison with native compilation.

## First review fixes

The suite now has 22 integration tests and two machine unit tests. New regressions
cover rejected code leaving saved images unchanged, 100,100 successive input
evaluations, stale-handle rejection after repeated slot reuse, retiring a slot
at generation exhaustion, unsigned system spellings, publication byte accounting,
and every primitive wire ID. The initial benchmark numbers above predate these
fixes; they are not fresh measurements.

All CAS objects are still retained on export. Valid but unused objects can remain
after later failures (for example a linker resource limit); the publication check
prevents invalid dependencies, not all unused-object growth. Reachability-based
image pruning and dictionary compaction remain separate work.
