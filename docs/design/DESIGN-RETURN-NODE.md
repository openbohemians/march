RIGHT WAY: Explicit RETURN node + multi-output support + effect sequencing

GOALS
  - Multi-result words (0..N)
  - Void (unit) words
  - Structural sequencing of side-effects (not metadata-only)
  - Single unambiguous entry point for evaluation

DESIGN SUMMARY

1) Introduce RETURN node kind
   - RETURN is always the root of a word graph.
   - Has:
       vals: [{cid, port}]  // returned value inputs in order
       deps: [{cid, port}]  // dependency inputs for effect sequencing
       outs: ["i64", "ptr", ...] // N result types
   - Output arity = outs.len().
   - Works for void (outs.len() = 0) and multi-result.

2) Distinguish value vs dependency wiring
   - vals drives the returned results
   - deps are traversed for side effects (ignored results)
   - This encodes sequencing structurally via edges, not via metadata

3) Upgrade Node representation to support multi-output
   - Add `outs: [TypeAtom]` to node canonical form
   - Non-RETURN nodes:
       * outs = results of prim/word
       * often length 1
   - RETURN:
       * outs length = vals length

4) Legacy compatibility
   - If `outs` is absent:
       interpret old `ty` field as outs=[ty]
   - Preserves all legacy nodes

5) Builder tracks effect frontier
   - Maintain frontier: HashMap<EffectCID, StackItem>
   - On apply_prim/apply_word with declared effects:
       frontier[e] = StackItem{cid: new_node_cid, port:0, ty:Unit}
   - frontier tracks the newest node per effect domain

6) Builder finish_word algorithm
   - Expect stack.len() == expected_result_count
   - vals = stack items (left-to-right)
   - deps = sorted, deduped frontier.values()
   - Create RETURN node:
       kind: "RETURN"
       vals: vals
       deps: deps
       outs: result types
       effects: [] (RETURN doesn’t declare new effects)
   - Store RETURN, set WordCanon.root = return_cid
   - Clear builder state

7) Evaluator semantics
   - Start at RETURN root
   - Evaluate all deps (to realize side effects)
   - Evaluate all vals (to produce return tuple)
   - Void (outs=[]) still traverses deps
   - Return empty tuple/unit if outs empty

8) Multi-result consumption
   - Existing StackItem model already supports (cid, port)
   - Consumers target ports 0..N-1 of RETURN
   - Intermediate multi-result nodes work the same way

9) Optional PACK node (not required)
   - Could help assemble multi-result values mid-graph
   - RETURN already covers final packing, so PACK can wait

10) Advantages
    - Single explicit root (RETURN)
    - Structural sequencing via dependency edges
    - Natural void words
    - First-class multi-result
    - No schema ambiguity
    - Backward compatible

11) Minimal schema shape change
    Node:
      kind: "RETURN" | "PRIM" | ...
      outs: [string]
      // Non-RETURN:
      in: [{cid, port}]
      // RETURN ONLY:
      vals: [{cid, port}]
      deps: [{cid, port}]
    WordCanon:
      root: cid
      params: [...]
      results: [...]
      effects: [...] // summary; sequencing uses deps edges

12) Test cases to validate
    - Void, pure (deps empty)
    - Void, effectful (deps non-empty)
    - Multi-result primitive (e.g., divmod)
    - Multi-result from disparate producers
    - Effect ordering enforced by deps
    - Legacy single-result nodes still load (outs synthesized)

KEY IDEA
  RETURN encodes the contract:
    what to return (vals),
    what must run (deps),
    and where callers begin.


