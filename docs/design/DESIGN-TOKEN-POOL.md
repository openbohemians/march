# DESIGN-V.md -- On Token Pool

Keep a small “token pool” right next to your stack while building/lowering, and thread handles from that pool into effectful nodes. When tokens are disabled (e.g., production build with logs/tests stripped), the builder can substitute pure replacements.

Here’s a crisp way to structure it and avoid foot-guns later.

1) Representing the pool

Domains & permissions (extensible)

    // Rust-y pseudocode (same idea maps to Zig/C)
    #[derive(Clone, Copy, Hash, Eq, PartialEq)]
    enum Domain { State, Io, Fs, Net, Metric, Test }

    #[derive(Clone, Copy, Hash, Eq, PartialEq)]
    enum Perm { R, W }               // start with single-token by collapsing both
    type Tid = u64;                  // transaction id (0 when unused)

    #[derive(Clone, Copy, Hash, Eq, PartialEq)]
    struct TokenKey { dom: Domain, perm: Perm, tid: Tid }  // extensible

Token handles (linearity vs duplicability)

    enum TokenHandle {
      Single(NodeInput),            // single linear token (baseline)
      RToken(NodeInput),            // duplicable read
      WToken(NodeInput, Tid),       // linear write w/ TID
    }

Token pool alongside the builder’s stack

    struct TokenPool {
      map: HashMap<TokenKey, TokenHandle>,
      mode: Mode,                   // Dev | Prod
    }

    enum Mode { Dev, Prod }

2) Builder API you call from lowerings

Keep tiny helpers that make intent obvious and keep the linearity rules in one place:

    impl TokenPool {
      fn get_read(&mut self, dom: Domain) -> Option<NodeInput> {
        match self.map.get(&TokenKey{dom, perm: Perm::R, tid: 0}) {
          Some(TokenHandle::RToken(n)) => Some(*n),
          Some(TokenHandle::Single(n)) => Some(*n), // baseline: single token serializes reads
          _ => None
        }
      }

      fn get_write(&mut self, dom: Domain) -> Option<(NodeInput, Tid)> {
        match self.map.remove(&TokenKey{dom, perm: Perm::W, tid: 0}) {
          Some(TokenHandle::WToken(n, tid)) => Some((n, tid)),
          Some(TokenHandle::Single(n))      => Some((n, 0)),     // baseline
          _ => None
        }
      }

      fn put_write(&mut self, dom: Domain, tok: NodeInput, tid: Tid) {
        let key = TokenKey{dom, perm: Perm::W, tid};
        assert!(self.map.insert(key, TokenHandle::WToken(tok, tid)).is_none(),
                "WToken must remain linear");
      }

      fn clone_read(&mut self, dom: Domain, rtok: NodeInput) -> NodeInput {
        // Emit a DUP node in the net (or just reuse handle if your IR treats RToken as value)
        emit_dup(rtok)
      }
    }

Usage in a lowering:

    // Lower STORE(key, val) in State domain
    if let Some((w, tid)) = pool.get_write(Domain::State) {
        let w2 = emit_STORE(key, val, w, tid);   // node consumes and returns token
        pool.put_write(Domain::State, w2, tid);
    } else if (pool.mode == Mode::Prod && is_optional_effect(Effect::StateW)) {
        // e.g., metrics/logs/tests → elide to pure no-op or buffer in RAM
        emit_ERASE(val);
    } else {
        error!("missing state write token for required STORE");
    }

This pattern gives you:
- linearity for WTokens (remove then put back),
- duplication for RTokens, and
- a clean Prod-mode elision for optional effects (logs/tests/metric).

3) Integrate with effect rows (types)

- Each word/quotation carries an effect row (`{}`, `{io.R}`, `{state.W}`, …).
- During typing, unify rows; during lowering, consult rows to ask the pool for the right token.
- If an effect is required (e.g., `state.W` for your DB update) and the pool can’t supply it, fail compilation—don’t silently erase.
- For optional effects (logs/tests/metrics) guard with `mode==Prod` and emit `ERASE` or `TRACE` sink.

This keeps semantics correct and makes “Prod removes noise” safe.

4) Upgrading to split tokens & TIDs

Your key already anticipates it. The changes are mechanical:

- When you enter a transaction, install a `WToken(State, tid)` and an `RToken(State, epoch)` in the pool:

      pool.map.insert(TokenKey{dom: State, perm: W, tid}, TokenHandle::WToken(n_w, tid));
      pool.map.insert(TokenKey{dom: State, perm: R, tid: 0}, TokenHandle::RToken(n_r));

- `get_read(State)` may prefer the RToken; `STORE` requires WToken (with correct TID).
- Reads can run concurrently (builder emits `DUP` for RToken as needed).
- Writes stay linear because `get_write` removes the entry until you `put_write` it back.

5) Guardrails & verification

- Verify pass after building: walk the graph and check that every `STORE/IOWRITE/...` consumes exactly one WToken edge and produces one; that no path duplicates WToken edges; and that RToken edges either come from a known duplicable source or are properly DUP’d. (This pass is short and catches regressions.)
- Domain independence: your key supports multiple domains; letting `{io, state}` advance independently is as simple as carrying both in the pool.
- SSA friendliness: if your mid-IR is SSA, represent tokens as SSA values just like any other argument; the pool simply maps domain → current SSA value.

6) A tiny Zig-flavored sketch (single token baseline)

    const Domain = enum { state, io };
    const TokenKey = struct { dom: Domain };

    const TokenHandle = struct { node: NodeInput }; // baseline: single token is linear

    const TokenPool = struct {
        map: std.AutoHashMap(TokenKey, TokenHandle),
        mode: enum { dev, prod },

        pub fn getWrite(self: *TokenPool, dom: Domain) ?NodeInput {
            if (self.map.remove(TokenKey{ .dom = dom })) |h| return h.node;
            return null;
        }
        pub fn putWrite(self: *TokenPool, dom: Domain, n: NodeInput) void {
            std.debug.assert(!self.map.contains(TokenKey{ .dom = dom }));
            self.map.put(.{ .dom = dom }, .{ .node = n }) catch unreachable;
        }
    };

    // lowering STORE
    if (pool.getWrite(.state)) |w| {
        const w2 = emitStore(key, val, w); // consumes w, returns new token
        pool.putWrite(.state, w2);
    } else if (pool.mode == .prod and optional_effect) {
        emitErase(val);
    } else {
        fail("missing state token");
    }

7) Subtle but important distinctions

- Only optional effects should be erasable. Logs/tests/metrics → OK. Core semantics (like your language “global variables”) must require a token or compilation should fail.
- Don’t hide the token behind a singleton. Treat it as a first-class IR value (SSA or node input). The pool is just a builder-time convenience to fetch the current handle.
- Don’t conflate stack values with tokens. Keep them separate tracks: the value stack for data; the pool/hashmap for capability flow. (If you later adopt a FRAME, you can attach token ports to it, but still keep the pool abstraction.)

8) Why this design scales

- Adding new domains? Extend `Domain`.
- Switching from single-token → R/W → TIDs? Your `TokenKey` and `TokenHandle` already account for it.
- Moving to checkpoints/MVCC later? The pool passes the right `WToken(TID, epoch)`; the backend implements semantics without perturbing front end codegen.

Bottom line: Your `HashMap<Domain, NodeInput>` idea is the right backbone. Wrap it in a tiny API that enforces linearity (W) vs duplicability (R), gate optional effects on `Mode::Prod`, and treat tokens as real IR values so a later verifier can check you. This keeps today simple and tomorrow extensible.

