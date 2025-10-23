# March Development Mode & Debug Runtime

## Purpose

March’s production runtime keeps the execution stack lean: every cell is just an encoded literal or execution token, and the VM trusts the compiler’s static checks. During language bring‑up and user DSL development we need more visibility. Development Mode (“dev mode”) adds a shadow stack and instrumentation that mirror the production stack so we can:

- Detect type/guard mismatches at runtime before they reach production.
- Inspect quotation boundaries, effect annotations, and originating CIDs while stepping through code.
- Produce actionable stack traces when primitives or user words misbehave.

The instrumentation is opt‑in: the compiler and loader still emit the same production cells, and the VM executes them unchanged. Dev mode layers metadata alongside those cells without perturbing normal execution.

## Operating Modes

| Mode        | Stack layout                    | Guarantees                                 | Typical use                    |
|-------------|---------------------------------|---------------------------------------------|--------------------------------|
| Production  | Runtime stack only (cells)      | Minimal overhead, relies on static typing.  | Releases, benchmarks, FFI.     |
| Dev Mode    | Runtime stack + debug stack     | Runtime check for blob kind, signature, guard lineage, source span. | Debugging, DSL experimentation, compiler bring-up. |

Switching modes is a compile‑time or runtime flag (e.g. `march --dev` or `MARCH_DEV=1`). Tests can opt into dev mode to assert stack discipline.

## Shadow Stack Design

The debug stack is a simple array mirrored on push/pop with the runtime stack. Each entry tracks:

- **Blob kind** (`BLOB_*`) captured from the loader’s decode step.
- **Signature CID** (if available) for quoting stack effects.
- **Origin**: word/quotation name, namespace, and source hash (if stored in `defs`).
- **Runtime tag**: `XT`, `LIT`, `DATA`, `THUNK`, etc., giving the checker enough to reject mismatched consumers.
- **Optional metadata**: guard context, effect row, or user annotations once the type system propagates them.

Cells continue to store raw addresses or integers. When dev mode is enabled:

1. The loader records metadata when linking: alongside `cells[count++] = encode_xt(...)` it pushes a debug record containing blob kind and signature.
2. The VM’s stack operations (`push`, `pop`, `dup`, etc.) duplicate their actions on the debug stack when the flag is set.
3. Consumer words (e.g. `execute`, `map`) read the debug entry before using the value; mismatches raise descriptive errors.

Because the debug stack is separate, existing cell encodings and primitives do not change. Dev mode only allocates the side stack and toggles the mirror operations.

## Integration Points

### Loader (`src/loader.c`)

- After decoding each tag with `decode_tag_ex`, emit the production cell as today.
- If dev mode is active, emit a `debug_record_t` capturing `id_or_kind`, linked CID, and any decoded literal size. For quotations, keep the `sig_cid` instead of freeing it immediately.
- Register buffers for cleanup alongside `allocated_buffers`.

### VM / Stack API (`runtime` directory)

- Stack operations accept an optional `debug_stack_t*`.
- `encode_exit` and primitive dispatch remain unchanged; debug mode simply notes that a branch instruction consumed an `XT`.
- When raising errors, include both the runtime stack snapshot and the mirrored metadata.

### Compiler (`src/compiler.c`)

- No change to the primary emission path yet; user words still become runtime cells.
- When materializing quotations, store them as `BLOB_QUOTATION` and attach the signature ID so the loader can propagate it to the debug stack.
- Optional future work: emit development annotations (e.g. guard names, effect rows) into a side table keyed by CID for the loader to retrieve.

### Tooling & Tests

- Unit tests can toggle dev mode to assert misuses (e.g. pushing a literal where a quotation is required).
- CLI flag / environment variable controls instrumentation without recompiling.
- Stack dumps include both literal values and debug metadata for easier diagnosing.

## Error Handling Strategy

When the debug stack detects a mismatch:

1. Raise a descriptive error (word name, expected kind, actual kind, signature summary).
2. Provide a trace of the last N debug entries pulled from the shadow stack.
3. Optionally dump the raw runtime cells to help track VM-level issues.

Production builds strip this code or short‑circuit the checks so there is no overhead.

## Implementation Phases

1. **Scaffolding**: add a debug stack struct, loader instrumentation hooks, and runtime flag plumbing. Do not yet enforce checks.
2. **Metadata propagation**: keep quotation `sig_cid` and blob kind alive through loader, push entries alongside each cell in dev mode.
3. **Runtime checks**: teach primitives like `execute`, `call`, `map`, and guard transitions to verify debug entries.
4. **Enhanced diagnostics**: include guard/type inheritance lineage, effect rows, and source spans as the type system evolves.
5. **Optimization**: allow per-module filtering (e.g. only track words compiled with `--debug-stack`).

## Open Questions

- How should dev mode behave when interacting with the C FFI or host environment? (Potentially require explicit annotations.)
- Do we want to record effect bits and guard names at runtime, or leave them to tracing logs?
- Can we reuse the debug stack for deterministic replay tooling or crash dumps?

## Summary

Dev mode layers a metadata-rich debug stack on top of the existing runtime without changing production semantics. The compiler retains responsibility for static guarantees, while the debug stack provides guardrails and diagnostics during development, DSL experimentation, and compiler validation. Once the type system and loader agree on the metadata they exchange, enabling or disabling dev mode is simply a flag—giving March both the speed of a traditional FORTH core and the safety nets expected from a modern statically typed language.

