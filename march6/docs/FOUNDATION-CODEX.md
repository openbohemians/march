# March foundation — independent Codex proposal

2026-09-28. **Proposal, not an adopted architecture or implementation request.**
First pass frozen before reading Claude's response. This is independent of his
new proposal, not blind to our shared history: I participated in the preceding
design discussion and consulted the existing Claude-authored lineage audit.
The source checks below are my own; I did not rerun the historical executables.

## Objective

Build the smallest practical March that can build the next layer of March.
Keep low-level systems programming accessible beneath typed, immutable,
context-oriented application programming. Execution is strict; explicit laziness
is a later library/compiler facility, not the foundation's scheduling model.
Do not restart the repository or require compatibility with experimental images.

## Proposed foundation

Use an abstract cell-stack machine with immutable, content-addressed definitions
and separately linked executable code. Compiler words and application words run
on the **same engine**. No graph inference/evaluation prerequisite for executing
or sealing system code.

The provisional system model is 64-bit cells, a data stack, and a private control
stack. System cells are not yet safe high-level values. Calls, return, indirect
execution, literals, conditional branches, integer/bit operations, byte/cell
memory access, and explicit host calls supply the mechanism. Start with bounded
stacks, execution fuel, and checked memory access for development; bounds checks
alone do not establish language-level type or lifetime safety.

Use explicit wrapping unsigned machine arithmetic; checked signed application
arithmetic must retain separate semantics. Specify shifts, division failures,
alignment and byte order, rather than inheriting host accidents. This is a
candidate contract requiring agreement, not a change to existing `+`.

Do not settle assembly versus Rust by philosophy. My first-backend preference
is a small checked Rust executor over linked numeric operations/local word
handles, using March4's code/link distinction. Compare it with March4's assembly
kernel before accepting that choice; retain the latter as a concrete candidate,
not merely inspiration. Neither has earned a speed claim for this architecture.

## Identities and executable code

Canonical definitions contain an ordered, tagged sequence of literals,
dependency identities, quotation references and local control operands. Names,
runtime addresses and inferred optimization graphs are not their identity.
Primitive identities include a semantic domain/version; compact primitive IDs
are an encoding choice, not permission to change primitive behavior silently.

A mutable builder emits a definition. Sealing fixes its bytes and CID; linking
validates and resolves dependencies into executable references. Ordinary calls
then perform neither dictionary lookup nor hashing. Mutable work buffers, stack
slots and allocator metadata do not need CIDs. Names are separate bindings;
rebinding never changes an already sealed caller.

Local branch targets must denote validated instruction boundaries. A local
self-call operand avoids a self-referential hash; general mutually recursive
groups need a separate identity design and are outside the first milestone.
Code identity does not certify safety or erase host/environment dependencies.

## Host boundary and March layers

| Layer | Initial responsibility | Intended destination |
|---|---|---|
| Host seed | Execute linked operations; provide memory regions, byte I/O, hashing, checked loading and explicit host imports | Small documented platform substrate; replaceable implementations |
| System March | Input cursor/WORD, numbers-first interpretation, dictionary/mode, builders, `:`/`;`, immediate words, branch fixups | March from the first bootstrap milestone |
| Checked March | Stack-effect inference, typed quotations, context-family construction, immutable data interfaces | March libraries/compiler words over the same machine |
| Application libraries | Persistent namespaces/store, explicit laziness, higher collections | March code, with measured native helpers where justified |

The seed loader/encoder may initially be host services; their exact responsibilities
must be listed, not hidden behind “self-hosted.” A one-time assembler installs the
first system definitions, but must not parse subsequent March source. Source
syntax belongs to dictionary words consuming input. No native special case for
each new language construct.

Provide region allocation/release first, not a bespoke allocator. My initial
preference is offsets into owned memory regions rather than arbitrary host
pointers; native interoperation then uses checked adapters. Raw platform access
remains an explicitly trusted extension. This choice needs comparison with
March4's pointer model before implementation.

Trusted system words may mutate construction storage but must relinquish writable
aliases when publishing immutable objects. Reclamation and alias discipline are
real obligations, not properties supplied by CAS. Never expose arbitrary writes
into Rust-managed immutable objects. Before claiming a safe application layer,
specify its verification and trusted-library boundary; a restricted dictionary
alone cannot secure an arbitrary-memory machine.

Types and contexts are not postponed into oblivion: the first layer after the
bootstrap must exercise typed quotation application and a guarded family.
Ordered, read-only guards are the current behavioral starting point. Compilation
mode can initially be explicit state; a later context-based replacement must
demonstrate equivalence, not require a second evaluator.

## Reuse decisions grounded in code

| Existing work | Decision and evidence |
|---|---|
| March2 bootstrap FORTH | Adapt input-consuming/immediate compilation from [forth.rs](../../march2/src/forth.rs) (`native_colon`, `native_semicolon`) and [input.rs](../../march2/src/input.rs). Do not inherit host-recursive `XT::Compiled` execution or incomplete type tracking. |
| March2 earlier contexts | Preserve cases and selection/isolation lessons from `archive/march2/main:src/main.rs`, `eval_token` and `eval_constraint`. Its discarded write overlay differs from today's write-rejecting guards; do not silently merge those semantics. |
| March4 kernel/linker | Adopt stored-code versus executable-code separation from [types.h](../../march4/src/types.h), [loader.c](../../march4/src/loader.c); evaluate direct reuse/adaptation of [vm.asm](../../march4/kernel/x86-64/vm.asm) and primitives after contract review. Do not assume unchecked cells or executable trampolines are required. |
| March4 memory/compiler | Do not claim reclamation exists: [free.asm](../../march4/kernel/x86-64/free.asm) is a stub. Revisit specialization design, but do not transplant the C compiler as the permanent language implementation. |
| March6 | Preserve definition/CID tests, stream-extension examples, strict behavior tests and Merkle-CHAMP store. Adapt [stream-seed.march](../src/fast/stream-seed.march)'s behavior; its current families/compiler-state handles depend on the old engine. [definition.rs](../src/fast/definition.rs) still lowers definitions into graphs: separate that dependency rather than rename it a foundation. Existing Rust data ownership is an adapter/reference, not a raw-cell ABI. |

## First acceptance milestone

Boot the system layer; use a March-defined compiling word to consume input,
perform arithmetic and emit another word. Save canonical definitions **and
bindings/immediate metadata**. In a fresh process, load them and repeat without
the host assembler, old evaluator, or host reinstatement of missing flags.
Rebuild the system layer with itself twice; compare canonical identities and
behavior. This demonstrates that layer's bootstrap, not full self-hosting.

Also test unresolved/corrupt dependencies, redefinition preserving old callers,
stack/memory bounds, branch validation and fuel-limited nontermination. Measure
compile/link cost separately from warmed calls, loops and compiler execution.
Do not delete the old path until these gates and a small typed/contextual slice
pass on the new one.

## Decisions still requiring agreement

Backend and cell/address ABI; primitive semantic table; host-import authority;
safe publication/reclamation discipline; exact type/context metadata in identity;
recursive-group identity; which existing host services remain intentionally
native. Independence/agreement is evidence, not proof: validate shared assumptions
with the milestone before expanding the feature list.
