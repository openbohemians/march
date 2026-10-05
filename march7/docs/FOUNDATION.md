# March7 foundation contract

Status: **first-slice implementation authorized**, 2026-09-28. This records
agreed direction and proposed details. Thomas authorized the definition-to-execution
slice after clarifying the FORTH model and independent definition compilation.
Unimplemented later-layer details remain proposals, not completed features.
March7 stays in this repository alongside the earlier implementations.

## 1. Purpose and agreed direction

Build the smallest practical March that can build the next layer of March.
The foundation is a programmable system layer, not a high-level host runtime
with low-level access added later. March itself participates from the beginning.

Agreed requirements:

- FORTH-style, input-driven self-extension and bootstrap.
- Content-addressed definitions; source names separate from code identity.
- Strict stack execution, with explicit laziness above the foundation.
- An accessible systems layer beneath typed, immutable, context-oriented March.
- An interpreter for interactive development and native compilation eventually,
  sharing language semantics. A JIT is optional, not a prerequisite.
- Static memory management wherever justified, with runtime reclamation when
  analysis cannot establish safe lifetimes. Ordinary programs must not depend on
  the analysis being complete.
- Reuse earlier code, designs and tests deliberately. No INet revival, SQLite
  requirement, or compatibility obligation for experimental images.

Everything more specific below is a **proposal for review**, not a previously
approved user requirement. Performance claims require measurements.

## 2. Architecture and responsibility

| Layer | Responsibility | Implementation target |
|---|---|---|
| Machine seed | Execute operations; provide bounded memory, host calls, code sealing/linking, and image loading | Small Rust kernel initially |
| System March | Working-memory representations, input handling, dictionary, interpreter/compiler, code builders and defining words | March on the seed |
| Checked March | Stack types, typed quotations, context families, immutable interfaces and memory analysis | March compiler/library code |
| Libraries | Persistent namespaces/state, collections, explicit laziness and system interfaces | March, with declared native services where justified |

One engine executes both compiler and application words. The compiler may
construct graphs for analysis later; graph construction or evaluation is not a
prerequisite for executing or sealing system definitions.

Start with a checked Rust interpreter over predecoded operations and resolved
word references. March4's assembly kernel is a design and implementation
candidate to study, not a performance result or an obligation to port verbatim.
Native compilation must consume the same definitions without introducing a
second language. Optimized executable code is derived, replaceable material.

No new host handler merely because a new source construct is introduced.
New host mechanisms require an explicit contract and a reason they should not
be March words. Count hidden loader and boot services in the kernel inventory.

### The compiler in FORTH terms

`WORD` consumes input; dictionary lookup finds an execution token and immediate
flag. `STATE` chooses execution or compilation. `:` begins a definition, `,`
appends its representation, and `;` finishes it. `HERE` is the next free address
in the current definition's output. These operations are March words using
ordinary working memory, not Rust syntax handlers or compiler-state objects.

The CAS addition at completion is to encode dependency identities, calculate the
definition's CID and obtain its executable form. The term **seal** below means
only this immutable completion operation. A failed attempt discards unfinished
output; it does not roll back every definition or a global application store.

An ordinary input stream is processed in order: immediate words can consume
input and change subsequent compilation. Already identified definitions can be
compiled with private stacks/output memory and a fixed view of their dependencies
and compiler words. Independent compilations can eventually run concurrently;
shared-input consumption and compile-time side effects require explicit ordering.
Do not split arbitrary FORTH source at punctuation in a host parser to obtain
those independent definitions. Failed replacements leave prior bindings usable.

## 3. Machine contract

### Cells, execution and failures

- A system cell is 64 bits. Its interpretation belongs to the operation using it;
  the seed does not supply host Text, Tuple or other application value variants.
- There is one operand stack per execution and a private control stack. Immediate
  words use that same operand stack, not a separate compiler evaluator.
- Instructions execute in sequence. Calls resume their caller; explicit tail
  calls reuse the continuation. Do not assume every terminal call is optimizable.
- A quotation's runtime representation is an execution token for linked code.
  `execute` checks token validity, but System March does not require a static
  input/output signature. Checked March must establish the quotation's contract.
- Execution tokens are not arbitrary machine addresses. A memory address cannot
  be used to jump into mutable bytes.
- Invalid tokens, bounds violations, invalid operations and exhausted resource
  limits produce defined errors, not host undefined behavior. An error stops the
  invocation; it does not promise to undo earlier workspace writes or I/O.
- Development execution has fuel, stack and memory limits. Resource accounting
  may differ between backends; those limits are not an optimization-equivalence
  test based on identical instruction counts.

### Arithmetic and memory

System arithmetic is explicitly unsigned and wrapping modulo 2^64. Unsigned
division/remainder by zero trap. Shift counts outside 0–63 trap. Comparisons
produce 0 or 1; conditional branches treat zero as false. Checked signed
application arithmetic has separate semantics and must not silently inherit
wrapping behavior. These choices do not redefine March6's primitives.

Memory addresses are pairs `(region, byte-offset)`, not packed host pointers.
Byte accesses are bounded; cell accesses are bounded, 8-byte aligned and use
little-endian encoding. Region arithmetic checks overflow before access. Released
region identifiers must not become valid references to new allocations.

Region identifiers and execution tokens can occupy cells, but use distinct host
tables and validation paths. This prevents accidental host-pointer dereference;
it is **not** an unforgeable capability system or a sandbox for hostile System
March. Foreign-memory/platform access needs a declared adapter rather than
unrestricted access to Rust objects.

## 4. Seed inventory and host boundary

This is the proposed closed inventory by operation family. Exact encodings and
host-call stack signatures are implementation specifications to pin in tests;
adding an operation outside this list requires review.

| Family | Included mechanisms |
|---|---|
| Stack | `dup`, `drop`, `swap`, `over`, `rot`; `mark` and `gather`, which collect the cells above a marked depth into a persistent vector (added 2026-10-04 for array literals) |
| Vectors | persistent sequences of cells (`merkle_champ::Sequence`, content-defined since 2026-10-05): length, element, append in place, update to a new version (added 2026-10-04) |
| Strings | UTF-8 text from a region's bytes; for strings and arrays, joining, slicing and equality of contents (added 2026-10-05; docs/STRINGS.md) |
| Integer | wrapping add/subtract/multiply; unsigned divide/remainder; equality and unsigned less-than; bitwise and/or/xor/not; logical shifts |
| Float | IEEE-754 binary64 add/subtract/multiply/divide, equality, less-than, integer conversion both ways, with canonical NaN (added 2026-10-03; docs/NUMBERS.md) |
| Control | full-cell literal, resolved call, return, local branch/zero-branch, quotation reference, indirect execute, local self-call, explicit tail-call forms, trap |
| Working memory | create/release region, region size, byte/cell load/store, cell load/store in the working region without a region operand (added 2026-10-04: every `get` and `put`, so compiling the system takes a third fewer steps) |
| Code | seal validated canonical bytes; obtain canonical bytes by CID; resolve/link CID to execution token; recover a token's code CID |
| Content | hash bytes; publish/retrieve immutable blobs by verified CID |
| Host I/O | read/write byte buffers using host-supplied endpoint handles; explicit status/count results |
| Boot/image | load/save a versioned canonical image and invoke its entry CID; supply initial input/I/O handles |

Host calls are declared imports with stable semantic identities, explicit stack
contracts and failure behavior. They cannot parse March syntax or recognize
dictionary names. Canonical bytes and CIDs cross these interfaces through
bounded buffers, not newly introduced high-level host values. Working memory
belongs to the machine, so validation cannot race concurrent mutation in the
initial single-threaded seed.

Use an existing SHA-256 implementation and, when persistent map integration is
needed, the existing Merkle-CHAMP library behind a documented adapter. Neither
cryptography nor a map rewrite is a bootstrap prerequisite. The bootstrap
dictionary itself can be a simple March memory structure.

Generic byte I/O and memory permit WORD, number conversion, string reading,
dictionary lookup, builder growth and branch fixups to be March code. There are
no `family-guard`, `tuple-syntax`, or `emit-text` host kernels. Source spellings
such as `:` and `;` are dictionary entries.

## 5. Definition identity, sealing and linking

Keep three objects separate:

1. **Unfinished definition:** mutable System March working data, including unresolved local
   branch fixups. It is not executable or published.
2. **Definition:** immutable canonical bytes and a CID. Its sequence records
   primitive references, literals, dependency CIDs, quotations and local control
   operands. Calling code and pushing a quotation are distinguishable.
3. **Executable:** validated, linked code with process-local call targets.
   Interpreter instructions and eventual native code are derived from definitions.

The definition domain includes a format/semantic version. Names, inferred
optimization graphs and process addresses are excluded. Primitive identity names
specified behavior; implementations may change without changing that behavior.
A semantic change requires a new identity, not reuse of a convenient numeric ID.
Compact primitive numbers are only an encoding within a specified domain.

Sealing copies and validates canonical bytes; it does not execute the definition
or infer a high-level signature. It rejects malformed operands and invalid local
branch targets. Linking verifies dependency identities, availability and kinds
before producing executable references. No partial definition is installed as a
successful dictionary binding: that publication order is the responsibility of
March's defining words, not a host-owned dictionary. Limits apply to image size
and linking work too.

Ordinary compiled calls do not hash or resolve names at runtime. Redefinition
changes future lookup, not previously sealed callers. Explicit dynamic lookup
is possible as March code, but is not the default calling convention.

Self-recursion uses a local self-call operand, avoiding a self-referential CID.
Mutually recursive groups need a separate canonical group/member design; they
are not required for the first milestone. Inferred signatures are derived data;
future metadata that changes dispatch or meaning must enter semantic identity
explicitly, rather than being attached invisibly.

## 6. Working memory, immutable publication and reclamation

Compiler cursor, mode, dictionary and unfinished definitions live in working
memory. They do not cause persistent-store updates or hashing on every step.

Publishing code or data takes an immutable snapshot. Subsequent writes through
old working aliases cannot change it. Initial publication copies into private
immutable storage; later zero-copy publication needs an actual proof or enforced
transfer of writable authority. CAS does not supply that proof by itself.

An image is a serialization of explicit durable representations, not a dump of
arbitrary cells. It contains reachable definitions/data, namespace bindings,
immediate flags, semantic-domain information and an entry CID. No raw region IDs,
execution tokens, open I/O handles or return-stack addresses survive as live
references. System March encodes its dictionary metadata and reconstructs its
working structures; the host does not secretly reinstall language bindings.

**Temporary bootstrap policy:** System March allocates inside bounded regions,
using a bump pointer initially. Whole-region release is available. Scratch regions
can be released after copying durable results out; the live dictionary region
lasts for its session. Repeated compilation within a session may accumulate
workspace until its limit. This is intentionally not a production REPL memory
solution, not static memory analysis, and not the permanent collector.

**Long-term policy:** prove placement, lifetimes and safe reuse where possible;
otherwise use runtime reclamation for managed application values. Failure to
prove a lifetime must not make an otherwise valid ordinary program unusable.
Manual System March region release retains explicit caller obligations; a future
collector cannot make arbitrary misuse of system memory correct.

Tracing, reference counting or a hybrid remains an engineering choice owned by
the lead developer, based on representative throughput, memory use and pause
measurements. Native code and interpreter code must interoperate under the same
value/lifetime contract. Both may benefit from static analysis. Retain cumulative
allocation counters as well as peak/live accounting where implemented.

## 7. Trust, types and contexts

The first System March is trusted low-level code. Bounds checks protect host
memory; they do not establish type safety, immutable-object discipline, or
isolation between arbitrary words. Do not advertise an ordinary-language safety
guarantee until its checker and trusted-library boundary exist.

Checked March supplies stack-effect inference and typed quotations above this
machine. The intended checker is March code, not another permanent Rust language
implementation. Advisory checks can assist development but are not a substitute
for enforcing the advertised safe interface.

Context-family syntax and construction belong in March. Start from ordered
read-only guards: preserve the caller's argument stack, evaluate against a
consistent state, and reject prohibited effects rather than performing and
discarding them. Guard-private temporary allocation may be permitted; mutation
of caller-visible memory, store publication and external I/O must not escape.
The authority/checking mechanism requires design before guards are called safe.

Module-level context grouping, error redispatch and specialization remain later
language work. A typed quotation and a small guarded family are the first
post-bootstrap integration slice, so those goals cannot be deferred indefinitely.

## 8. Reuse and migration

| Existing asset | Treatment |
|---|---|
| [March2 input](../../march2/src/input.rs) and [defining words](../../march2/src/forth.rs) | Adapt input-consuming/immediate-word behavior into March. Do not copy host-recursive execution or incomplete type tracking. |
| Earlier March2 contexts (`archive/march2/main:src/main.rs`) | Preserve test cases and dispatch/isolation lessons; discarded guard writes are not today's proposed semantics. |
| [March4 kernel](../../march4/kernel/x86-64/vm.asm), [linker](../../march4/src/loader.c), [workspace/store design](../../march4/docs/design/DESIGN-OVERVIEW.md) | Reuse the architecture and inspect primitive algorithms. Do not inherit addresses in persistent code, unchecked loader assumptions or unimplemented reclamation. |
| [March6 stream seed](../../march6/src/fast/stream-seed.march) and [bootstrap tests](../../march6/docs/BOOTSTRAP.md) | Retarget behavior and acceptance cases; remove old evaluator/compiler-state-handle dependencies. |
| March6 definitions, strict runtime and store | Preserve canonical-identity tests and strict semantic tests as references. Reuse suitable hashing/store code through adapters; do not make tagged Rust values the new system substrate. |

March6 stays runnable as a reference. No bulk deletion, nested repository,
automatic migration of old images or transplantation of the whole compiler.
New code must identify what it reuses, adapts or replaces and why.

## 9. First acceptance milestone

### Bounded generation-zero assembler

The one-time assembler is a build tool, not a second March compiler. Its input
is a flat instruction listing: instructions with literal operands, labels,
CID references and explicitly encoded data bytes. Labels provide local fixups
and references between seed definitions; they are not a March dictionary.
It has no March source reader, immediate-word execution, macros, type inference,
context selection or knowledge of language-level dictionary layouts.

Its output is the smallest generation-zero image containing the outer
interpreter and the seed definitions needed to build the next layer. Subsequent
language layers are compiled from March source by March. Extending the assembler
input format requires foundation-contract review, just like adding a seed
operation; implementation convenience is not permission to grow its language.

After the first successful boot and self-rebuild, the assembler's only role is
reproducing generation zero. Normal boot, interactive compilation and self-rebuild
must not invoke it. Keep its entry point out of the normal runtime binary, and
test boot and rebuild with the assembler unavailable. Shared, syntax-independent
encoding/validation routines are allowed; a hidden source compiler is not.

Difficulty in the March-written compiler is resolved in March, or by proposing
a general seed mechanism for contract review, never through an assembler shortcut.
Record the assembler's source line count and host dependencies at each milestone;
review growth explicitly. Line count is a warning signal, not proof that its
responsibility boundary is sound.

**Status: frozen (2026-09-30).** The self-rebuild milestone is met: generation 0,
assembled from `seed/system.asm`, compiles `seed/system.march`, and generations
1-3 were byte-identical; since tail calls (2026-10-04), generations 2 and 3
are, because generation 0's frozen `;` emits no tail calls (docs/REBUILD.md). The listing now only reproduces
generation 0. `tests/rebuild.rs` pins the SHA-256 of the assembled image
(`b9b9c609…`), so any change to the listing or the assembler that alters it
fails until the pinned value is changed on purpose. The system source depends
on nine of the listing's words: `: ; immediate STATE ! c, ' -- recur`.

### Acceptance sequence

1. A one-time assembler produces the initial system image. Record its entire
   host dependency boundary and line count under the restrictions above.
2. Boot that image into the seed. A March-written outer interpreter reads words,
   tries numbers before dictionary lookup, and executes or compiles them.
   Decimal overflow is an error, not fallback to a numeric-looking name.
3. March supplies `:`/`;`, immediate words, literal emission and postponed
   compilation. Demonstrate a new compiling word that reads input, computes a
   value, emits a definition and executes the result through the same engine.
4. Save code, bindings and flags. A fresh process reloads them and repeats the
   extension without the assembler, March6 evaluator, or native flag repair.
5. Rebuild the system interpreter/compiler with itself for two further
   generations. With identical inputs, compare canonical definitions, image
   bytes and behavior; exclude nondeterministic diagnostic metadata from images.
6. Repeat normal boot, compilation and self-rebuild with the generation-zero
   assembler unavailable. Fail the test if any path depends on that tool or
   delegates March source interpretation to another host compiler.

Negative tests cover corrupt/missing dependencies, invalid branches/tokens,
stack and memory limits, released regions, publication aliasing, arithmetic
failures and fuel-limited nontermination. Redefinition must preserve old callers.
All potentially nonterminating development tests also get process timeouts.

Measure cold load/link, compilation and warmed execution separately, including
calls, branches/loops, memory access, source compilation and self-rebuild. Report
workspace/allocation use alongside time. No native-code comparison may erase the
work being measured. Bootstrap success proves this layer, not full self-hosting.

## 10. Review decisions and work deliberately deferred

Approve or amend these concrete choices before coding:

1. Checked Rust seed; 64-bit cells; private control stack; resolved execution
   tokens rather than arbitrary executable addresses.
2. Region-plus-offset working memory, alignment/arithmetic/error rules, and the
   explicitly temporary bump/release bootstrap policy.
3. Seed inventory and native service boundary, including sealing/linking and
   the one-time assembler; no syntax-specific host kernels.
4. Separate canonical definitions, linked executables and image metadata; local
   self-recursion first; no experimental-image compatibility.
5. Bootstrap acceptance gate and subsequent typed/contextual integration slice.

Before the first persisted image, pin exact instruction encodings, primitive
semantic identifiers, host-call signatures and golden canonical byte/CID vectors.
These specify this contract; they must not quietly broaden the kernel.

Deferred, not abandoned: production reclamation and static lifetime analysis,
native code generation, optional JIT, recursive groups, full type/context system,
general object/image serialization and large-data collections. Do not let these
replace the bootstrap milestone with another unbounded research program.

### Design provenance

This consolidates the [Codex first pass](../../march6/docs/FOUNDATION-CODEX.md),
[Claude first pass](../../march6/docs/FOUNDATION-CLAUDE.md),
[comparison](../../march6/docs/FOUNDATION-COMPARISON.md), and
[Claude's clarified-requirements revision](../../march6/docs/FOUNDATION-CLAUDE-REVISED.md),
plus Thomas's subsequent interpreter/native and memory-management requirements.
The first passes shared prior evidence; the revision was not blind. Their
agreement supports this proposal but is not a substitute for the acceptance tests.
