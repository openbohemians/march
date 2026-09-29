# Foundation designs — comparison

2026-09-28. Codex comparison of two frozen first passes; not an adopted design.
No implementation is authorized by this note. Claude has been invited to review
the comparison and send his own assessment. His response is pending.

**Brief clarification:** After this comparison, Thomas pointed out that Claude
had not received the full preceding “Chuck Moore way” conversation. The original
brief mentioned bottom-up construction and systems access, but omitted his
explicit endorsement of building the systems layer first and his abstract
stack-assembler-with-CAS framing. Codex had that context. Consequently, the
differences below may reflect unequal requirements context, not disagreement
about the same goal. With Thomas's approval, the missing conversation has been
sent to Claude for a revised pass. Preserve both originals; label the revision
as clarified-requirements work, not another blind analysis. Specific cell layouts,
memory schemes, host languages and primitive inventories remain proposals.

## Independent records

- [Codex first pass](FOUNDATION-CODEX.md): SHA-256
  `8d719c5d5f4824374840b8d31239812b48382dc12baa4e0f960357d309bf4a48`.
- [Claude first pass](FOUNDATION-CLAUDE.md): SHA-256
  `fd603f8e5bce1eb36690c221f7dcae522bbcf3098cb00a012298af193c65ed5f`.

Both authors froze before reading the other's new proposal. Both knew earlier
project discussions and used Claude's existing lineage audit. Agreement is
therefore convergence from partially shared evidence, not two independent proofs.
Keep the first passes unchanged; record corrections here or in a successor design.

## Where the proposals agree

1. The host should supply execution mechanisms, not a growing set of syntax
   handlers. March should implement its own input-driven interpreter, defining
   words and eventually type/context machinery.
2. Compiler words and application words must execute on the same strict stack
   machine. The old graph evaluator/lowering must not be required to bootstrap it.
3. Definitions have canonical content identities; source names are bindings.
   Dependency references must preserve the selected code, not silently follow
   subsequent name changes. Mutable construction and immutable published code
   are distinct states.
4. Preserve March6's useful tests, stream-interpreter work, CAS and store assets;
   use March2/March4 evidence rather than either restoring or discarding them
   wholesale. SQLite and INets are not foundation requirements.
5. The first success is a strict self-rebuild from an image, not another feature
   checklist. A fresh process must run the rebuilt interpreter without its old
   source compiler. Syntax extension must genuinely emit code in March.

These are a strong common brief. They do not yet specify the machine.

## Architectural disagreements

| Question | Codex first pass | Claude first pass | Consequence |
|---|---|---|---|
| Lowest programming layer | Cells, memory regions, linked operations; checked high-level values above it | Rust-tagged Int/Bool/Text/Bytes/Tuple/Quote/Code plus store | Is March building its data/runtime foundations, or extending a host-provided high-level runtime? |
| Existing executor | Preserve behavioral assets; evaluate new linked representation and March4 assembly | Reuse `stack.rs` and `Value` as the seed | Raw-cell and tagged-value representations are not interchangeable implementation details. |
| Working state | Transient compiler/build buffers distinct from published state | Cursor, definition builder and mode all in persistent store | Store unification is convenient, but may conflate scratch work with the user's durable-state model and incur per-step allocation/hashing. Requires measurement and a semantic decision. |
| Guards | Start from current read-only guards, rejecting writes | Discard a guard's returned store, as in March2 A | Writing then reading inside a guard behaves differently; rollback of a map cannot undo external effects. |
| Memory responsibility | Region access/lifetime and publication contract at foundation | Reuse host-owned values; postpone lower-level memory model | Bootstrap can arrive earlier with host values, but the intended systems layer may remain deferred. |

The important choice is not merely Rust versus assembly. Both can implement
either a low-level substrate or a high-level host runtime. Neither representation
has an established performance advantage for the proposed full system.

## Corrections and shared gaps

- **Primitive identity:** an implementation may change while preserving meaning;
  changing overflow, effects or evaluation behavior requires a new semantic
  identity. Stable numeric IDs alone do not settle this.
- **Calls and signatures:** “every primitive has a fixed stack effect” needs a
  qualification for execution of a dynamically selected quotation. Removing
  `Apply` counts requires typed signatures, runtime contracts, or explicitly
  unchecked system execution—not just removal of the fields.
- **Recursion:** removing host `Recur` still needs a self-call mechanism.
  General recursive-group identity is not solved by hashing dependency CIDs.
- **Guard isolation:** specify restoration of the argument stack as well as
  state. Neither discarded snapshots nor read-only maps alone isolate arbitrary
  memory writes or I/O; guard authority/effect restrictions need a contract.
- **Safety:** advisory types do not establish a safe language. Conversely,
  region bounds do not prove type or lifetime safety. Define what is trusted
  before exposing low-level facilities under immutable interfaces.
- **Reuse accuracy:** current `stack.rs` has strict baseline measurements and
  conditional tail-return reuse, not unrestricted “free tail calls.” March4's
  specialization strategy is separable from its failures to persist definitions;
  reject accidental identity coupling without ruling out derived specialization.
- **Bootstrap completeness:** save dictionary bindings and immediate flags, not
  only code blobs. A byte-identical image fixed point alone does not demonstrate
  usable self-extension, correct semantics, or full self-hosting.
- **Smallness:** “about 40 primitives” and “minimal cell machine” both need an
  explicit inventory and dependency check. Count hidden host services as well as
  exposed instruction names. Fewer primitives is not automatically less complexity.

## Recommended resolution order

1. Agree on the lowest programmable layer and its memory/value contract. My
   current lean remains the cell/system layer, because accessible systems
   programming is part of the user's foundation goal. Claude's tagged runtime
   remains a useful reference/adapter; this preference is not consensus.
2. Decide transient compiler state versus published store, and the trust/effect
   boundary. Do not assume all state must be persistent or all mutation public.
3. Enumerate seed mechanisms, including dynamic calls, self-recursion, linking,
   memory release and host I/O. Specify which are intentionally native and which
   move to March. Choose the first backend against that contract and old code.
4. Adopt a combined acceptance gate: March defines a compiling word, emits and
   executes code, saves/reloads it with metadata, and rebuilds the interpreter
   for two further generations on the same engine. Include bounded failure tests
   and separate compile/link/runtime measurements.

Do not average the drafts into a larger seed containing both machines. Settle
these choices with the user, then write one agreed design and migration plan.
