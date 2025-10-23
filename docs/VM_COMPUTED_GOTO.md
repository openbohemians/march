# Computed Goto VM Design (Draft)

This document captures the intended shape of the GCC/Clang computed-goto inner interpreter for March.

## Goals

1. Replace the current assembly dispatcher with a portable C implementation while we rely on GCC/Clang extensions.
2. Keep colon definitions (user words, quotations) dispatching through a DOCOL-style helper.
3. Minimise per-call overhead; the only extra work per invocation should be a single indirect jump.
4. Provide a clear ABI so loader/compiler know how to lay out cell streams and entry records.

## Core Loop Overview

```c
typedef uint64_t cell_t;
typedef struct {
    cell_t *ip;        // instruction pointer (points into cell stream)
    int64_t ds[DS_MAX];
    int64_t *sp;       // data stack top
    cell_t *rs[RS_MAX];
    cell_t **rp;       // return stack top
} vm_state_t;

void vm_run(vm_state_t *vm, cell_t *entry);
```

- `vm_run` initialises `ip` with the entry array and executes until it sees `XT(0)` (EXIT) with empty return stack.
- Dispatch uses a tag table: `goto *tagtab[tag(cell)]`.
- Tags follow current encoding: `00` XT, `01` LIT, `10` LST, `110` LNT, `111` EXT.

## DOCOL Helper

Each user word/quotation gets a small entry record:

```c
typedef struct {
    cell_t *body;      // pointer to first cell
} entry_t;

static inline void docol(vm_state_t *vm, entry_t *entry) {
    *vm->rp++ = vm->ip;
    vm->ip = entry->body;
    goto *tagtab[tag(*vm->ip++)];
}
```

- The loader materialises an `entry_t` per CID and hands `entry->body` + `docol` address to the interpreter.
- The interpreter sees an XT cell that encodes a pointer to `entry_t`. The XT handler loads it and jumps to `docol`.
- `docol` re-enters the shared dispatch loop (no per-CID executable stubs).

## Loader Expectations

For each CID-backed definition the loader:

1. Decodes blobs into `cell_t[]` arrays (same as today).
2. Allocates an `entry_t` struct and stores the array pointer.
3. Places the address of `entry_t` into the dictionary.

Primitives continue to expose their machine-code addresses. To keep the XT handler uniform we can prefix primitive entries with a sentinel bit (e.g., lowest bit set = primitive) or we can dedicate another tag combination; details TBD.

## Tag Handlers

- **XT**: fetch pointer, branch to primitive or `docol`. If pointer is NULL, pop return stack; if stack empty, halt.
- **LIT**: push sign-extended literal.
- **LST**: push symbol ID (or resolve through symbol table).
- **LNT**: next N cells copied verbatim to stack.
- **EXT**: reserved (safe fallback halts or throws).

## Integration Notes

- Expose the interpreter through a C function that the existing runner can call.
- Provide C stubs for all assembly primitives (or reimplement primitives directly in C as part of migration).
- Keep the loader updates isolated so tests can switch between old assembly VM and new computed-goto VM via a build flag.

## Future Migration

When March self-hosts, the same structure can be coded in March itself:

- Generate jump tables as arrays of closures or using an equivalent of computed goto in the target backend.
- Maintain the `entry_t` concept so the rest of the runtime does not change.
- Remove the GCC/Clang requirement once the March compiler produces the dispatcher.

---

Open questions to track while prototyping:

- Exact representation of XT cells for primitives vs user words (sentinel bit? two separate tag values?).
- Ensure label alignment guarantees the low tag bits remain zero when using `&&label` (may require `-O2` or explicit alignment directives).
- Effect of tail-call optimisation on stack traces / dev-mode instrumentation.
- Whether `entry_t` needs type/effect metadata for dev-mode shadow stack.
