TITLE: Mini Interaction-Net ABI (Effect Tokens, Quotations, Guards, Store, TXN)

VERSION: 0.1

OVERVIEW
- Programs compile to a graph of AGENT instances (atoms) with fixed arity ports.
- Evaluation is graph-rewriting on ACTIVE PAIRS (principal-port interactions).
- Effects are serialized via TOKENS. Pure code does not mention tokens.
- This ABI defines agent names, ports, invariants, and rewrite sketches.

---------------------------------------------------------------------------
TYPE KEYS & EFFECT ROWS (compile-time concepts)

TypeKey      := hash of (arg ground types, result ground types, effect mask)
EffectMask   := bitset over domains {io, fs, net, state, time, ...} × {R,W}
TokenDemand  := per domain lattice ⊥ < R < W (compile-time)

Notes:
- Pure quotations have empty effect row ({}). They do not take any token ports.
- Effectful quotations require corresponding token ports per domain.

---------------------------------------------------------------------------
TOKENS (runtime concepts)

We support two designs; pick one and keep names stable.

A) SINGLE LINEAR TOKEN (SIMPLE)
  TOKEN         : a unique linear permission to perform any effect.

B) SPLIT TOKENS WITH R/W (RECOMMENDED)
  RTOKEN[d]     : duplicable read permission for domain d (snapshots allowed).
  WTOKEN[d,T,E] : linear write permission for domain d, carrying:
                  - T = TID (transaction id, opaque)
                  - E = epoch / timestamp (logical time)
  Laws:
    - SplitR: WTOKEN[d,T,E] -> (WTOKEN[d,T,E], RTOKEN[d,E])
    - JoinR:  (RTOKEN[d,E], ... RTOKEN[d,E]) -> RTOKEN[d,E]   (structural)
    - No JoinW: there is at most one WTOKEN per domain+TID.
    - No Upgrade: RTOKEN cannot become WTOKEN.

---------------------------------------------------------------------------
CORE STRUCTURAL AGENTS

VAL(c)        ; value literal
  Ports: out, k
  Rewrites: forward c to out, continue via k.

PAIR          ; build pair (x,y)
  Ports: x, y, out, k
  Rewrites: bundle x,y to out, continue via k.

UNPAIR        ; split pair
  Ports: in, x, y, k
  Rewrites: take pair from in, send to x,y, continue via k.

SHUFFLE(p)    ; optional general stack permuter (e.g., DUP/SWAP special cases)
  Ports: inputs..., outputs..., k
  Rewrites: permutes wires per pattern p.

FRAME         ; optional explicit stack frame root (see notes)
  Ports: cont(principal), top, rest
  Notes: Useful for analysis; may be optimized away in codegen.

---------------------------------------------------------------------------
QUOTATIONS & CALL/DISPATCH

QUOTE(qid)
  Meaning: push a quotation-id (qid) as a value.
  Ports: out, k

APPLY(qid@TypeKey)   ; devirtualized/specialized call
  Ports: in_args..., out_vals..., k, [optional tokens by effect row]
  Rewrites: inline / jump to quotation qid specialized to TypeKey.

EXEC_GENERIC         ; indirect call
  Ports: qid_in, in_args..., out_vals..., k, [optional tokens], dict?
  Rewrites: pairs with QTABLE to route to APPLY.

QTABLE
  Ports: qid_in, out_apply
  Rewrites: matches qid to an APPLY(qid@*) subnet. (Implementation-defined jump table.)

TAG(TypeKey)
  Meaning: attach a runtime type key (only emitted if analysis incomplete).
  Ports: val_in, key_out, k

---------------------------------------------------------------------------
BRANCHING, GUARDS, DEOPT

IF
  Ports: cond, k_true, k_false
  Rewrite: when cond is boolean literal, forward to chosen continuation.

GUARD(TypeKey, PathId)
  Ports: key_in, k_match, k_else
  Rewrite: if key_in == TypeKey -> k_match else k_else.

GUARDCTX(pred-id)
  Meaning: runtime context guard (may read state).
  Ports: [RTOKEN[state] or WTOKEN[state]], args..., k_match, k_else
  Rewrite: evaluates predicate; if true -> k_match else -> k_else.
  Notes: needs RTOKEN or WTOKEN depending on predicate’s effect row.

DEOPT
  Ports: in..., k
  Meaning: jump to generic fallback path (e.g., unspecialized EXEC/IF).

MERGE
  Ports: in1, in2, ..., k
  Meaning: multi-way join; first-arriving branch continues via k.

---------------------------------------------------------------------------
EFFECTFUL STORE / IO (STATE DOMAIN SHOWN; OTHERS ANALOGOUS)

LOAD(key)
  Ports (R/W token design):
    - R/W TOKEN: (RTOKEN[state] | WTOKEN[state,T,E])
    - key_in
    - val_out
    - token_out (same token)
    - k
  Rewrite: retrieves value for key at the token’s snapshot (E). Pure w.r.t data,
           effectful w.r.t token (permission).

STORE(key)
  Ports (W token required):
    - WTOKEN[state,T,E] in
    - key_in
    - val_in
    - WTOKEN[state,T,E] out
    - k
  Rewrite: stages a new version under TID T. Visibility after COMMIT.
           No read output; returns updated token.

CAS(key)  ; compare-and-swap (optional)
  Ports:
    - WTOKEN[state,T,E] in/out
    - key_in, expected_in, desired_in
    - ok_out (bool)
    - k

SCAN(range) (optional)
  Ports:
    - R/W TOKEN in/out
    - range_in
    - iter_out (iterator handle)
    - k

PRINT / READLINE / NETSEND / NETRECV / FILE ops
  Ports: corresponding domain tokens in/out, args..., k
  Notes: Each side-effecting op is serialized by its domain token.

---------------------------------------------------------------------------
TRANSACTIONS (MVCC-FRIENDLY)

TXN_BEGIN(domain=state)
  Ports:
    - WTOKEN[state, T_new, E_snap] out
    - RTOKEN[state, E_snap] out
    - k
  Semantics: start a txn with snapshot epoch E_snap; produce both tokens.

TXN_COMMIT
  Ports:
    - WTOKEN[state,T,E] in
    - ok_out (bool)
    - WTOKEN[state,T’,E’] out? (optional: next-epoch token)
    - k
  Semantics: validate & publish staged writes for T. ok_out=true on success.
             May bump epoch.

TXN_ABORT
  Ports:
    - WTOKEN[state,T,E] in
    - k
  Semantics: discard staged writes for T.

Notes:
- With SQLite: implement BEGIN/COMMIT/ROLLBACK (WAL recommended).
- TID, E are opaque in the net; runtime maps them to SQLite txns and snapshots.

---------------------------------------------------------------------------
CONTEXTUAL DISPATCH (RUNTIME STATE CONDITIONS)

DICTCALL(method-id)          ; for abstract/interface methods
  Ports:
    - dict_in
    - args...
    - result...
    - k
  Rewrite: routes to method APPLY bound in dict.

PACK(tyid) / UNPACK          ; existential packaging
  Ports:
    - PACK: val_in, dict_in → pack_out, k
    - UNPACK: pack_in → val_out, dict_out, k

GUARDCTX (defined above)     ; combine with DICTCALL or word overloads.

Overloaded word lowering pattern:
  word_X:
    GUARDCTX(pred1) → APPLY(X_impl1)
    GUARDCTX(pred2) → APPLY(X_impl2)
    else            → APPLY(X_default)

---------------------------------------------------------------------------
REWRITE SKETCHES (ILLUSTRATIVE, ENGINE-SPECIFIC)

1) Devirtualized call:
  (args..., [tokens]) — APPLY(qid@TypeKey) • k
  ⇒ (inlines or jumps to qid body specialized to TypeKey), then k.

2) Indirect call:
  qid — QTABLE • EXEC_GENERIC(args..., [tokens]) — k
  ⇒ QTABLE resolves qid ⇒ APPLY(qid@TypeKey?) ⇒ k

3) Guarded specialization:
  TAG(tk) — GUARD(TypeKey=tk0)
    match → APPLY(specialized)
    else  → DEOPT → EXEC_GENERIC

4) LOAD/STORE (R/W token form):
  RTOKEN[state,E] + key — LOAD → (val, RTOKEN[state,E])
  WTOKEN[state,T,E] + key + val — STORE → WTOKEN[state,T,E]

5) Transactions:
  TXN_BEGIN → WTOKEN[state,T,E] + RTOKEN[state,E]
  ... perform STORE/LOAD within T ...
  WTOKEN[state,T,E] — TXN_COMMIT → ok + (maybe WTOKEN[state,T’,E’])

---------------------------------------------------------------------------
INVARIANTS

- Linearity:
  - WTOKEN[d,*,*] is linear: at most one live instance per (d,TID).
  - RTOKEN[d,*] is duplicable. No RTOKEN→WTOKEN upgrade.
- Purity:
  - Pure agents (ADD, MUL, PAIR, UNPAIR, QUOTE, APPLY of pure qid) have no token ports.
- Effects:
  - Any agent that touches a domain must consume and produce that domain’s token.
- Guards:
  - GUARD / GUARDCTX must have a DEOPT or else-branch to preserve correctness.
- Quotation IDs:
  - qid is opaque; only QTABLE/EXEC/APPLY interpret it.

---------------------------------------------------------------------------
PORT ORDERING CONVENTION (RECOMMENDED)

For agents with mixed ports, use:
  [principal | data inputs..., data outputs..., token inputs..., token outputs..., continuation(s)]

Examples:
  LOAD: [principal | key_in, val_out, token_in, token_out, k]
  STORE: [principal | key_in, val_in, token_in, token_out, k]
  APPLY: [principal | args..., rets..., tokens_in..., tokens_out..., k]

Keep this consistent for codegen and peephole passes.

---------------------------------------------------------------------------
NOTES ON FRAME (OPTIONAL)

If you use FRAME explicitly:
  - DUP/SWAP/PUSH/POP/ROT compile to small rewires over FRAME.
  - Analysis is simpler (stack rows are explicit).
  - A later pass can erase FRAME when shapes are statically known.

---------------------------------------------------------------------------

## APPENDIX I: Durability & Checkpointing (0.1)

DurabilityFlag := V | B | D  ; Volatile, Buffered, Durable

STORE(key, D=inherit)
  Ports:
    - WTOKEN[state, T, E, Dtok?] in
    - key_in
    - val_in
    - WTOKEN[state, T, E, Dtok?] out
    - k
  Semantics:
    - EffectiveD = (D == inherit) ? Dtok : D
    - If EffectiveD == V: update RAM cache only (dirty=false), no SQLite.
    - If EffectiveD == B: update RAM cache (dirty=true), enqueue key for checkpoint.
    - If EffectiveD == D: open/ensure txn T, write-through to SQLite now (prepared stmt), update cache, dirty=false.

LOAD(key)
  Ports:
    - (RTOKEN[state, E, ...] | WTOKEN[state, T, E, ...]) in
    - key_in
    - val_out
    - same token out
    - k
  Semantics:
    - Hit RAM cache first (prefer newest value).
    - If miss and durability enabled: read from SQLite snapshot (WAL), then cache.

CHECKPOINT
  Ports:
    - WTOKEN[state, T, E, ...] in/out    ; or a dedicated admin token
    - k
  Semantics:
    - Begin SQLite txn (IMMEDIATE).
    - Flush all keys with dirty=true and D=B using prepared batch writes.
    - Commit; clear dirty flags; bump durable epoch E' (optional).
    - Token out carries updated epoch if used.

SYNC(key)  ; optional
  Ports:
    - WTOKEN[state, T, E, ...] in/out
    - key_in
    - k
  Semantics:
    - If key is dirty and D=B: write it now in a single-key txn; clear dirty.

CTX_PERSIST(D)  ; dynamic scope switch (optional sugar)
  Ports:
    - token/state/context in/out
    - k
  Semantics:
    - Sets default DurabilityFlag for nested STOREs (until scope ends).


## APPENDIX II: Constraint Agents (0.1)

ConstraintBundle := map varid → (lb:int64 = -∞, ub:int64 = +∞)   -- types can't be this limited, shoudl be all numbers

ASSERT_LB(varid, k)
  Ports: bundle_in, bundle_out, k_cont
  Semantics: bundle[varid].lb := max(bundle[varid].lb, k)

ASSERT_UB(varid, k)
  Ports: bundle_in, bundle_out, k_cont
  Semantics: bundle[varid].ub := min(bundle[varid].ub, k)

CSET_MERGE
  Ports: bundle_a, bundle_b, bundle_out, k_cont
  Semantics: pointwise meet; if any var has lb>ub → FAIL

FAIL
  Ports: k_cont
  Semantics: abort current path (or route to DEOPT)

OBS_READ(varid, key)
  Ports: RTOKEN[state,epoch] in/out, key_in, bundle_in/out, k_cont
  Semantics: read state[key] = v; emit ASSERT_LB/UB per configured predicate

SPECIALIZE_MATCH(sigkey)
  Ports: bundle_in, key_in(TypeKey), k_fast, k_slow
  Semantics: if bundle entails sigkey → k_fast else k_slow

EPOCHCHECK(min_epoch)
  Ports: RTOKEN[state,epoch] in/out, k_ok, k_deopt
  Semantics: if epoch ≥ min_epoch → k_ok else k_deopt


## APPENDIX III: Testing, Tracing, and Logging Agents (0.1)

DOMAINS
- io        : for user-visible logs/prints
- test      : (optional) for test event streams independent of io
- metrics   : (optional) for in-memory counters/gauges

CORE UTILITIES

TEE
  Ports: val_in, val_out, tap_out, k
  Semantics: duplicates value; tap_out feeds probes/loggers; val_out continues computation.
  Effects: pure (no tokens required).

ERASE
  Ports: in..., k
  Semantics: discards its inputs and continues; used to strip logs in release.

ASSERT / EXPECT

EXPECT_EQ
  Ports: actual_in, expected_in, RTOKEN[state]? (optional if reading state), 
         IOTOKEN(io)? (if printing immediately),
         TESTTOKEN(test)? (if emitting event),
         k
  Semantics:
    - Compare actual vs expected.
    - On pass: optionally emit TestEvent(pass) to TESTTOKEN or nothing.
    - On fail: emit TestEvent(fail, diff) and, if IOTOKEN present, also LOG a human message.
  Effects:
    - If only TESTTOKEN is used, does not require IOTOKEN.
    - If printing user-visible output, requires IOTOKEN(io).

EXPECT_PRED(pred-id)
  Ports: val_in, [additional args], IOTOKEN?/TESTTOKEN?, k
  Semantics: applies predicate; behaves like EXPECT_EQ on pass/fail.

ASSERT_EQ / ASSERT_PRED
  Same as EXPECT_*, but on fail route to FAIL or DEOPT as configured.

LOGGING

LOG
  Ports: IOTOKEN(io) in/out, msg_in (str), payload_in (any, optional), k
  Semantics: formats and writes a line to stdout/stderr. Sequenced by IOTOKEN.

LOGF(level)
  Ports: IOTOKEN(io) in/out, fmt_in (str), args_tuple_in, k
  Semantics: printf-style structured log. May be guarded by LOG_LEVEL.

TRACE(tag)
  Ports: TESTTOKEN(test) in/out, payload_in, k
  Semantics: append (tag, payload, timestamp?) to test event stream (in-memory). No user IO.

METRICS

COUNTER_ADD(tag, delta)
  Ports: METOKEN(metrics) in/out, k
  Semantics: increments counter[tag] by delta in-memory.

GAUGE_SET(tag, value)
  Ports: METOKEN(metrics) in/out, k
  Semantics: sets gauge[tag] = value in-memory.

CHECKPOINT_METRICS (optional)
  Ports: METOKEN(metrics) in/out, IOTOKEN(io)? in/out, k
  Semantics: flush metrics to log or backend on demand.

PROBING & SNAPSHOTS

PROBE(tag)
  Ports: val_in, TESTTOKEN(test)? in/out, k, val_out
  Semantics: forwards val to val_out; optionally emits (tag,val) to test stream.
  Effects: pure if no TESTTOKEN attached.

SNAPSHOT_STATE
  Ports: RTOKEN[state] in/out, TESTTOKEN(test)? in/out, key_range?, k
  Semantics: reads a selection of keys; emits (key,val) events to test stream.

CONFIG & GUARDS

LOG_LEVEL(level)
  Ports: k_on, k_off
  Semantics: compile/runtime guard. If current level >= level → k_on else k_off.

TEST_MODE(on_off)
  Ports: k_on, k_off
  Semantics: enables/disables tests. In release builds rewrite test/log nodes to ERASE.

REWRITE SUGGESTIONS

- In release:
  * LOG, TRACE, PROBE → ERASE (if configured)
  * EXPECT_* → ERASE (or keep as ASSERT_* only)
  * COUNTER_ADD/GAUGE_SET may persist (low overhead) or ERASE

- In debug:
  * LOG/TRACE sequenced by IOTOKEN/TESTTOKEN
  * EXPECT_* never alters value flow; only produces events/prints


### Practical wiring tips

* Put a TEE before critical transforms: val → TEE → (val to compute) + (tap to LOG/TRACE).
* Keep EXPECT_* after a computation node and before side effects you test; this keeps causality clear.=
* For high-volume logs, prefer TRACE into an in-memory ring buffer (no io token), and flush periodically with a single LOG or on error.
* Use LOG_LEVEL/TEST_MODE to rewrite logs/asserts to ERASE at build time (or via GUARD at runtime).

### How this interacts with tokens & specialization

* Logging that prints uses the io token; it naturally serializes with any other IO.
* Test probes that don’t print can run without tokens (pure) or with a dedicated test token if you want ordering.
* GUARD/DEOPT specialization is unaffected; at hot sites you can strip or hoist logs with a peephole pass.

---------------------------------------------------------------------------
END OF SPEC

