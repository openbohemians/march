# Bootstrap: FORTH's self-extension on an immutable substrate

Status: B0a reflection, B0b reader foundation, and the B0c executable seed
subset implemented; the complete B0 gate remains open. Drafted by @march-claude
from the archived March 1–5
lineages (see `../VERSIONS.md`), updated with the provisional surface choices.

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
| March 6 | staged CAS reducer, INet backend | executable B0c seed; parsing aliases demonstrated, full bootstrap still open |

This is structural, not accidental.  FORTH bootstrapping relies on mutation:
the dictionary, `STATE`, `HERE`, and the input buffer, all changed by
immediate words.  Each move toward immutability and content addressing left
the bootstrap with no obvious home, so it was deferred.  Unless the
reconciliation is designed deliberately, March 6 will repeat the pattern.

## The reconciliation

March 6 has the core ingredients: guarded families, explicit state records,
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

- **FORTH-like composition, with provisional name-first definitions.** March 3
  weighed bare identifiers as symbols with explicit `.` execution; its
  EXAMPLE.md judged the noise not worth it. Current working spellings are
  recorded in `SYNTAX.md`; historical spellings are not binding requirements.
- **Guards are pure.**  March 5 required it, and March 6 enforces it
  syntactically.
- **Names are outside object identity.**  A separate index maps names to CIDs
  (March 5).
- **Tail-recursion-only is not needed.**  March 2 adopted it to avoid native
  stack growth.  The implemented work-list reducer removes that reason.
- **Context is explicit.**  Code values are closed; context crosses into code
  only as a parameter (March 6).

## Still open

- The working seed uses `name : expression ;` and `( ... )` quotations.
  `quote name` provisionally retrieves held code. Final spelling, optional
  named inputs, and surface context guards remain open (see `SYNTAX.md`).
- How far user-defined grammar words go: reading N tokens, reading to a
  sentinel, or also rewriting the token stream.
- The format context: whether parse traces are stored so text round-trips
  without loss, and if so, where they live relative to semantic CIDs.
  Presumably beside them, as names are.
- Broader multi-modal contexts from March 1: type checking, documentation, and
  profiling as reduction under other contexts.  This is a promising
  unification, but it comes after the bootstrap gate.
- Remote or cross-image lookup by CID.  `Intern` deliberately cannot forge a
  reference from CID-shaped data.  Later lookup should require an explicitly
  supplied resolver/import capability; its authorized result is then an
  ordinary held value.  Dictionaries and namespaces consequently double as
  explicit grants of reachable code.

## Proposed gate: B0, a self-defined colon

Goal: falsify, or earn, the claim that March's surface syntax can live
entirely in a content-addressed seed image above a syntax-free nucleus.

### Candidates from March 5 for the next checkpoint

These are design inputs from the archived exploration, not inherited language
requirements.  Their fit must be tested against the current reducer.

- **Effect-token pool:** keep symbolic token wires indexed by effect domain
  beside the compiler's symbolic data stack.  The seed can thread those wires
  through generated code and return the final tokens.  This could keep source
  stack notation compact while preserving explicit effects.  An empty pool is
  sufficient for the first pure bootstrap; read/write distinctions and optional
  effects need separate semantics and tests.
- **Multiple results:** prefer existing pairs/unit and projections as the
  first experiment for words with multiple outputs.  Retain the B0.1 scalar
  result convention for `square`.  Record stack effects in dictionary entries
  so composition knows which projections to construct.
- **Namespaces:** resolve source names through the reader's dictionary and
  embed the resulting held code values in definitions.  This supports static
  linkage without deciding whether all future namespaces must be compile-time
  only.  Interface contracts, provider lockfiles, and remote import resolution
  remain later work.
- **Rules in images:** investigate versioned rule artifacts as a route to
  deriving reducer identity from content.  User-defined rewrite rules raise
  additional validation and semantic questions.  The B0 gate can establish
  FORTH-style syntax extension with the present fixed primitive reducer.

### Progress: B0a reflection

The first checkpoint is implemented.  `Intern(description)` consumes ordinary
record/list values, iteratively constructs the described semantic graph, and
returns it only when the existing validator proves it is a closed quotation or
non-empty family with pure guards and no captured effect capability.  It is
strict but staged: an unknown description remains a residual `Intern`, while a
malformed ground description is an error.  Construction work is budgeted and
failed attempts roll back their newly inserted nodes.  The schema and tests are
documented in `REFLECTION.md`.

No surface spelling occurs in this mechanism.  B0a proves the construction
needed by `;`; it does not yet prove the outer interpreter, alternate syntax,
self-extension, or split/reload image laws in B0.1–B0.5.

### Progress: B0b reader foundation

`NextToken`, `ParseInt`, `Lookup`, and `PutKey` are implemented, staged,
reflectable, and image-serializable. A graph-defined reader counts token
frequencies in an immutable dictionary; it survives every token-boundary
save/reload of the tested input with the same final image CID. A separate
dictionary-held-code test retrieves and invokes `square` after reload.
See `READER.md` for contracts, tests, and resource limits.

This is not the seed compiler: the frequency reader assigns no language
meaning to tokens. Complete syntax, self-extension, streamed input, and broad
linear scaling remain to be demonstrated.

### Progress: B0c executable seed

The hand-assembled seed now reads `square : ( dup * ) ; 7 square`, constructs
the exact hand-built square quotation CID, and returns `49`. An alternate
FORTH seed reads `: square dup * ; 7 square` to the same code CID and result.
All token interpretation, symbolic-stack construction, dictionary updates,
and syntax handlers execute as image graphs through the unchanged nucleus.
The `eval` and `eval-forth` commands expose this experiment; see `SEED.md`.

Source can alias parsing handlers and use them immediately, including aliases
for the binding and ending words. This is a narrow B0.3 witness, not yet
source-defined arbitrary parsing behavior or a self-hosted seed assembler.
Constants, scalar-result quotation calls/composition, input-wire inference,
rebinding with static linkage, and `quote` retrieval are implemented.

| Gate | Evidence so far |
| --- | --- |
| B0.1 | Exact square CID and `49`; computed constant `42` |
| B0.2 | Two seed surfaces, unchanged nucleus |
| B0.3 | Source-defined parsing aliases; broader handler composition open |
| B0.4 | Tested image identity across reload and unrelated store history |
| B0.5 | Every tested token boundary with complete source retained; streamed chunks open |

Nested quotations, richer value/stack effects, surface guards and recursive
definitions, arbitrary grammar extensions, and broad scaling remain open.

### Next proposed gate: source bootstrap fixed point

Independent review proposes writing the seed's state-transforming handlers
in March source, compiling that source with the stage-zero seed, and comparing
the resulting runner CID with the assembled runner CID. Exact equality would
make the Rust assembler a verified bootstrap recipe rather than the only
maintainable form of the compiler. This is a proposed B1 target, not a passed
gate or an implemented feature.

It requires source-level record/code-description operations, general stack
effects, recursion, and guarded state transformers. It should exercise the
context-oriented model by expressing the compiler's own modes in March.
Before broadening that surface, the strict-evaluation/lazy-quotation factoring
counterexample in `MODEL.md` needs a demand-policy resolution and inline versus
factored value/error comparisons.
The in-memory Store currently retains transient reduction history; explicit
checkpoint/reload measurements in `SEED.md` provide a baseline while persistent
dictionaries and transient-vs-persistent storage remain research questions.

### Nucleus capabilities (Rust)

The nucleus must stay free of syntax: no knowledge of `:` `;` `[` `]` or any
other word.  It needs only:

1. The existing reducer, guarded families, records, quotations, and images.
2. Text values, a byte-cursor whitespace tokenizer step, optional checked
   decimal conversion, and dynamic record-key lookup/update (B0b). Text in and
   tokens out; the seed supplies interpretation and dictionary policy.
3. **Reflection.**  Code descriptions must be ordinary values, and a
   primitive must intern a description into a code CID.  This is the key new
   capability, since `;` must *construct* a definition.  My recommendation is
   descriptions as plain records plus one intern primitive, not
   quasi-quotation, but the choice is yours.  Interning must run the existing
   closed-code and guard-purity checks.
4. The implemented work-list reducer.  The outer interpreter recurses once per
   token, so **B0 depends on stack-safe iteration**.  The R0 regressions now
   cover 10,000 guarded calls, linear lazy-carried-state workloads, and
   depth-20,000 structural traversals; an explicit work budget, rather than a
   native-stack limit, bounds execution.

### Seed image (March)

Initially written as hand-built graphs. The executable subset is in `seed.rs`;
expressing this assembler in source remains a later self-hosting step:

- **The outer interpreter**: a guarded family `step(state)` with clauses:
  - at end of input: return the state;
  - the next token names a word whose definition is enabled in the current
    mode (the immediate case): apply it to the state;
  - the mode is compiling: compile the token's word into the symbolic stack;
  - otherwise (interpreting): execute the word.
- **Name-first binding and `:`**: preserve the proposed binding name; enter
  expression construction/evaluation when `:` follows. The seed must resolve
  lookahead and existing-name redefinition; no Rust syntax case may do it.
- **`;`**: bind the constructed value in a new dictionary record. A constant
  expression binds data, while a quotation expression binds validated code.
- **`(` and `)`**: nest and close a quotation under construction. Closing
  turns its symbolic stack into a Quote/Family description and interns it.
- **Stack shuffles** (`dup`, `drop`, `swap`) compile to wiring on the
  symbolic stack, not to nodes.  This is March 5's builder insight:
  "maintain a stack of producer indices; shuffles become wiring".

### Pass conditions

- **B0.1 Equivalence.** Reading `square : ( dup * ) ;` binds `square` to
  exactly the CID of the hand-built `Quote{1, Mul(Param 0, Param 0)}`;
  `7 square` then yields `49`. `answer : 6 7 * ;` binds the data value `42`.
- **B0.2 No syntax in the nucleus.**  A test builds a *different* seed image
  using conventional `: square dup * ;`, and the same nucleus reads it to
  the same CID. If swapping the syntax
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
