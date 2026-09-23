# Bootstrap: FORTH's self-extension on an immutable substrate

Status: principle plus a proposed falsification gate.  Drafted by @march-claude
from a reading of the archived March 1–5 lineages (see `../VERSIONS.md`).

## The principle

March is meant to be a *system*, not one more language feature.  Its central
inheritance from FORTH is that language, compiler, interpreter, and persistent
environment are not separate programs.  A tiny kernel exists; everything else,
including the compiler, is words in the dictionary.  Immediate words run during
compilation, so users extend the compiler the same way they write programs.
The running system can be saved and reloaded as an image.

March keeps that unity and changes the substrate: content-addressed immutable
graphs instead of threaded code, and explicit context values instead of mutable
interpreter state.  The claim to test is that nothing essential about FORTH's
self-extension is lost in that change.

## Five threads that recur in every lineage

These ideas survived every rewrite.  They are the vision; the rest is noise
from the exploration.

1. **One program, many contexts.**
   - March 1 called it "multi-modal execution": the same code run as runtime,
     type checker, optimizer, documentation, profiler, or debugger
     (`archive/march1/main:README.md`).
   - March 4 narrowed it to runtime, compile, and *format* contexts
     (`archive/march4/main:docs/design/SYNTAX.md`).
   - March 6 formalizes it as state-indexed reduction, where the context is an
     explicit immutable input to one reducer (`MODEL.md`).
2. **Context-oriented programming.**
   - Behavior is selected by guards over context rather than written as
     control flow.  Examples: March 2's `?` guard lines before `:`
     (`archive/march2/bootstrap-forth:DESIGN.md`), March 4's `@` guards,
     March 5's guard quotations and overloads
     (`archive/march5/main:docs/planning/PLAN-GUARDS.md`), and March 6's
     guarded families.
   - March 4 states the broad form: *"no AST, just tokens interpreted in
     different contexts (compile-time vs runtime).  Immediate words ARE the
     compiler."* (`archive/march4/main:docs/planning/PLAN-SELF-HOSTING.md`).
3. **Everything is a word, including `:` and `;`.**
   - Achieved once: March 2's `bootstrap-forth` branch, about 500 lines of
     Rust.  Its PROGRESS.md lists "no hardcoded parsing" as the main win over
     its first parse/eval attempt.
   - March 4 wanted the same thing, and its CORE-ARCHITECTURE.md describes how
     to get there.
4. **Text is one serialization of the stored program.**
   - The store is the true source.  Syntax must round-trip, parsing words
     amount to user-defined grammars, and other syntaxes (such as
     s-expressions) are equally valid views (`SYNTAX.md`; March 5's
     "code is in a database; ultimately a DB-connected GUI").
5. **Content identity and explicit, immutable state.**
   - Present in every version.  March 5 adds that names stay outside object
     identity and map to CIDs through a separate index.

## The recurring failure

The bootstrap thread was lost every time the substrate matured:

| Lineage | Substrate | Bootstrap status |
| --- | --- | --- |
| March 2 `bootstrap-forth` | mutable FORTH interpreter | achieved: `:` `;` are immediate words |
| March 4 | C compiler + x86 VM, CIDs in SQLite | lost: `:` `;` `[` `]` are tokenizer special cases |
| March 5 | CID objects, mini-inets | absent: programs built through CLI and YAML |
| March 6 | staged CAS reducer, INet backend | absent so far: no surface or seed image |

This is structural, not accidental.  FORTH bootstrapping relies on mutation:
the dictionary, `STATE`, `HERE`, and the input buffer, all changed by
immediate words.  Each move toward immutability and content addressing left
the bootstrap with no obvious home, so it was deferred.  Unless the
reconciliation is designed deliberately, March 6 will repeat the pattern.

## The reconciliation

March 6 now has every ingredient: guarded families, explicit state records,
lazy quotations, closed code values, and canonical images.  The proposal:

- **The compiler state is an explicit immutable value.**  It is a record
  holding:
  - the dictionary root;
  - the input text and position;
  - the mode (interpreting or compiling);
  - the symbolic stack of the definition under construction;
  - the "sticky" declarations that apply to every following definition:
    March 2's `=` signature and `?` context.
- **Reading a token is a pure transition** from one compiler state to the
  next.  Nothing mutates in place.
- **"Immediate" is a context guard, not a flag.**  An immediate word is an
  ordinary guarded family whose guard reads the compiler state's mode.  This
  is MODEL.md's "definition enabled in a construction context" made literal,
  and it means context-oriented programming and FORTH bootstrapping are the
  same mechanism.
- **Parsing is reduction under a construction context.**  A source file plus
  a seed image reduces to a new image.  Because the result is
  content-addressed, reproducibility is checkable: build twice and compare
  the image CIDs.
- **Definitions accumulate into sealed families.**
  - A `?` context line followed by `:` adds a clause in declaration order.
    That produces a new family CID, and the dictionary rebinds the name to
    it.
  - Consistent with guarded families: order is semantic, names are outside
    CIDs, and the name→CID map lives in the image root.

## Decisions already settled by the history

- **Traditional FORTH surface, not "symbols all the way down".**  March 3
  weighed bare identifiers as symbols with explicit `.` execution; its
  EXAMPLE.md judged the noise not worth it.
- **Guards are pure.**  March 5 required it, and March 6 enforces it
  syntactically.
- **Names are outside object identity.**  A separate index maps names to CIDs
  (March 5).
- **Tail-recursion-only is not needed.**  March 2 adopted it to avoid native
  stack growth.  The planned work-list reducer removes that reason.
- **Context is explicit.**  Code values are closed; context crosses into code
  only as a parameter (March 6).

## Still open

- Quotation delimiters: `( )` in March 2, `[ ]` in March 4.  Word definition:
  `: … ;` versus the Logo-style `to … end`.
- How far user-defined grammar words go: reading N tokens, reading to a
  sentinel, or also rewriting the token stream.
- The format context: whether parse traces are stored so text round-trips
  without loss, and if so, where they live relative to semantic CIDs.
  Presumably beside them, as names are.
- Broader multi-modal contexts from March 1: type checking, documentation, and
  profiling as reduction under other contexts.  This is a promising
  unification, but it comes after the bootstrap gate.

## Proposed gate: B0, a self-defined colon

Goal: falsify, or earn, the claim that March's surface syntax can live
entirely in a content-addressed seed image above a syntax-free nucleus.

### Nucleus capabilities (Rust)

The nucleus must stay free of syntax: no knowledge of `:` `;` `[` `]` or any
other word.  It needs only:

1. The existing reducer, guarded families, records, quotations, and images.
2. Text values and a primitive tokenizer step, of the form
   `(text, position) -> (token, position')`, split on whitespace.  Text in and
   tokens out; no meaning attached.
3. **Reflection.**  Code descriptions must be ordinary values, and a
   primitive must intern a description into a code CID.  This is the key new
   capability, since `;` must *construct* a definition.  My recommendation is
   descriptions as plain records plus one intern primitive, not
   quasi-quotation, but the choice is yours.  Interning must run the existing
   closed-code and guard-purity checks.
4. A work-list reducer, or at least a configurable depth limit well above
   the input length.  The outer interpreter recurses once per token, so
   **B0 depends on the iterative reducer**.  Today's depth limit of 64 would
   stop a 64-token source file.

### Seed image (March)

Written as hand-built graphs at first, since no parser exists yet to read it:

- **The outer interpreter**: a guarded family `step(state)` with clauses:
  - at end of input: return the state;
  - the next token names a word whose definition is enabled in the current
    mode (the immediate case): apply it to the state;
  - the mode is compiling: compile the token's word into the symbolic stack;
  - otherwise (interpreting): execute the word.
- **`:`**: read the next token as the name; mode becomes compiling; start an
  empty symbolic stack.
- **`;`**: turn the symbolic stack into a Quote or Family description, intern
  it, bind the name in a new dictionary record, and set the mode back to
  interpreting.
- **`[` and `]`**: nest and close a quotation under construction.
- **Stack shuffles** (`dup`, `drop`, `swap`) compile to wiring on the
  symbolic stack, not to nodes.  This is March 5's builder insight:
  "maintain a stack of producer indices; shuffles become wiring".

### Pass conditions

- **B0.1 Equivalence.**  Reading `: square dup * ;` binds `square` to exactly
  the CID of the hand-built `Quote{1, Mul(Param 0, Param 0)}`.
- **B0.2 No syntax in the nucleus.**  A test builds a *different* seed image
  in which the defining word is spelled `to … end`, and the same nucleus
  reads `to square dup * end` to the same CID.  If swapping the syntax
  requires any nucleus change, B0 fails.
- **B0.3 Self-extension.**  A source file defines a new parsing word, then
  uses it later in the same file (for example, defining `to`/`end` in terms
  of `:`/`;`).  This is the FORTH property itself.
- **B0.4 Determinism.**  Seed image plus source gives the same image CID
  across two runs, across insertion orders, and when resumed from a saved
  and reloaded intermediate image.
- **B0.5 Staging.**  Reading is reduction.  Splitting the source at any token
  boundary into two epochs (read part one, save, reload, read part two) gives
  the same image CID as reading it all at once.

### Failure criteria

Stop and rethink if any of these survive a serious attempt:

- the nucleus needs a syntax-specific case to make B0.1 or B0.2 pass;
- the outer interpreter needs hidden mutable state that is not in the
  compiler-state record;
- reading N tokens costs superlinear reductions or image growth (for
  example, re-interning the whole dictionary per token instead of sharing
  structure);
- reflection cannot be expressed without letting open or impure code into
  interned values.

Out of scope for B0: types and signatures (`=` may be parsed and stored, but
not checked); `?` contexts beyond showing that clauses accumulate in order;
format and round-trip; and the INet backend.  B0 is only about who owns the
syntax.
