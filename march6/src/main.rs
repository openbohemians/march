use march_research::inet::{Schedule, Value as InetValue};
use march_research::lower;
use march_research::memory::{self, InputIdentities, Instr, Ownership};
use march_research::seed::{self, Seed, Syntax};
use march_research::template::Template;
use march_research::{
    Atom, Bindings, Cid, Clause, DEFAULT_REDUCTION_BUDGET, Image, Node, Reducer,
    SpecializationCache, Store,
};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next();
    if matches!(command.as_deref(), Some("eval" | "eval-forth")) {
        let source = arguments
            .next()
            .ok_or("eval requires one quoted source argument")?;
        if arguments.next().is_some() {
            return Err("eval requires one quoted source argument".into());
        }
        let syntax = if command.as_deref() == Some("eval-forth") {
            Syntax::Forth
        } else {
            Syntax::NameFirst
        };
        return evaluate_source(&source, syntax);
    }
    if command.as_deref() != Some("demo") {
        eprintln!("usage: march-research demo | eval 'source' | eval-forth 'source'");
        std::process::exit(2);
    }

    let mut store = Store::new();
    let mode = store.intern(Node::Hole("mode".into()));
    let input = store.intern(Node::Hole("input".into()));
    let one = store.intern(Node::Const(Atom::Int(1)));
    let two = store.intern(Node::Const(Atom::Int(2)));
    let fast = store.intern(Node::Add(input, one));
    let generic = store.intern(Node::Mul(input, two));
    let program = store.intern(Node::If {
        condition: mode,
        when_true: fast,
        when_false: generic,
    });

    let truth = store.intern(Node::Const(Atom::Bool(true)));
    let mut compile_context = Bindings::new();
    compile_context.insert("mode", truth);
    let mut cache = SpecializationCache::new();
    let compiled = cache.specialize(
        &mut store,
        program,
        &compile_context,
        DEFAULT_REDUCTION_BUDGET,
    )?;
    let cached = cache.specialize(
        &mut store,
        program,
        &compile_context,
        DEFAULT_REDUCTION_BUDGET,
    )?;

    let forty_one = store.intern(Node::Const(Atom::Int(41)));
    let mut runtime = Bindings::new();
    runtime.insert("input", forty_one);
    let result = Reducer::new(&mut store, &runtime).run(compiled.residual)?;

    println!("source       {}", compiled.source);
    println!("context      {}", compiled.context);
    println!("reducer      {}", compiled.reducer);
    println!(
        "cache key    {}  second-hit={} entries={}",
        compiled.cache_key,
        cached.cache_hit,
        cache.len()
    );
    println!(
        "residual     {}  {}",
        compiled.residual,
        store.format(compiled.residual)
    );
    println!(
        "result       {}  {}",
        result.root,
        store.format(result.root)
    );
    println!(
        "reductions   compile steps={} visited={} rewrites={} peak-frames={}; runtime steps={} visited={} rewrites={} peak-frames={}",
        compiled.stats.steps,
        compiled.stats.visited,
        compiled.stats.rewritten,
        compiled.stats.peak_frames,
        result.stats.steps,
        result.stats.visited,
        result.stats.rewritten,
        result.stats.peak_frames,
    );
    let image = Image::from_store(&store, &[program])?;
    let (_, loaded) = Image::parse(image.as_bytes())?;
    println!(
        "image        {}  bytes={} nodes={} reloaded={}",
        image.cid(),
        image.as_bytes().len(),
        store.len(),
        loaded.len()
    );

    let parameter_flag = store.intern(Node::Param(0));
    let parameter_value = store.intern(Node::Param(1));
    let flag_is_true = store.intern(Node::Eq(parameter_flag, truth));
    let incremented = store.intern(Node::Add(parameter_value, one));
    let zero = store.intern(Node::Const(Atom::Int(0)));
    let guarded_family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: flag_is_true,
                body: incremented,
            },
            Clause {
                guard: truth,
                body: zero,
            },
        ],
    });
    let guarded_flag = store.intern(Node::Hole("guarded.flag".into()));
    let guarded_input = store.intern(Node::Hole("guarded.input".into()));
    let guarded_call = store.intern(Node::Dispatch {
        family: guarded_family,
        arguments: vec![guarded_flag, guarded_input],
    });
    let guarded_compile = Reducer::new(&mut store, &Bindings::new()).run(guarded_call)?;
    let guarded_residual_text = store.format(guarded_compile.root);
    let mut guarded_runtime = Bindings::new();
    guarded_runtime.insert("guarded.flag", truth);
    guarded_runtime.insert("guarded.input", forty_one);
    let guarded_result = Reducer::new(&mut store, &guarded_runtime).run(guarded_compile.root)?;
    println!("\nguarded reduction epoch");
    println!("residual     {guarded_residual_text}");
    println!(
        "result       {}  clauses-instantiated={}",
        store.format(guarded_result.root),
        guarded_result.stats.clauses_instantiated
    );

    let memory_program = vec![
        Instr::Int(1),
        Instr::Int(2),
        Instr::Pair,
        Instr::Int(3),
        Instr::Int(4),
        Instr::Pair,
        Instr::Pair,
        Instr::First,
        Instr::Drop,
        Instr::InputRef {
            index: 0,
            ownership: Ownership::Shared,
        },
        Instr::Drop,
    ];
    let comparison = memory::compare(&memory_program, &InputIdentities::new())?;
    println!("\nmemory plan");
    for (index, step) in comparison.plan.steps.iter().enumerate() {
        println!("  {index:02} {:?}  free={:?}", step.instruction, step.free);
    }
    println!(
        "memory       local={} exact-frees={} slots={} reuse={} fallback-inputs={} fallback-ops={} runtime-checks={}",
        comparison.plan.metrics.local_allocations,
        comparison.plan.metrics.exact_frees,
        comparison.plan.metrics.physical_slots,
        comparison.plan.metrics.reused_allocations,
        comparison.plan.metrics.fallback_inputs,
        comparison.plan.metrics.fallback_operations,
        comparison.plan.metrics.runtime_liveness_checks,
    );
    println!(
        "oracles      premature={} missed={} max-delay={} rc-time-mismatches={}",
        comparison.oracle.premature_frees.len(),
        comparison.oracle.missed_frees.len(),
        comparison.oracle.max_reclamation_delay,
        comparison.rc_oracle_mismatches.len()
    );
    println!(
        "RC baseline  increments={} decrements={} zero-tests={} frees={}",
        comparison.reference_counting.increments,
        comparison.reference_counting.decrements,
        comparison.reference_counting.zero_tests,
        comparison.reference_counting.frees
    );

    let mut repeated = Vec::new();
    for index in 0..100 {
        repeated.extend([
            Instr::Int(index),
            Instr::Int(index + 1),
            Instr::Pair,
            Instr::Drop,
        ]);
    }
    let repeated = memory::compare(&repeated, &InputIdentities::new())?;
    println!("\n100 sequential temporaries");
    println!(
        "planned      allocator-slots={} static-reuses={} runtime-checks={}",
        repeated.plan.metrics.physical_slots,
        repeated.plan.metrics.reused_allocations,
        repeated.plan.metrics.runtime_liveness_checks
    );
    println!("frame arena  retained-slots={}", repeated.frame_arena_slots);
    println!(
        "scope region retained-slots={} resets={}",
        repeated.scoped_regions.physical_slots, repeated.scoped_regions.resets
    );
    println!(
        "RC baseline  decrements={} zero-tests={}",
        repeated.reference_counting.decrements, repeated.reference_counting.zero_tests
    );
    println!(
        "tracing/32   peak-slots={} collections={} roots={} marked={} edges={} swept={}",
        repeated.tracing.peak_resident_locals,
        repeated.tracing.collections,
        repeated.tracing.roots_scanned,
        repeated.tracing.objects_traced,
        repeated.tracing.edges_scanned,
        repeated.tracing.objects_swept
    );

    let mut anchored = vec![Instr::Int(-1), Instr::Int(-2), Instr::Pair];
    for index in 0..100 {
        anchored.extend([
            Instr::Int(index),
            Instr::Int(index + 1),
            Instr::Pair,
            Instr::Drop,
        ]);
    }
    anchored.push(Instr::Drop);
    let anchored = memory::compare(&anchored, &InputIdentities::new())?;
    println!("\n100 temporaries beside one long-lived anchor");
    println!(
        "planned      allocator-slots={} static-reuses={}",
        anchored.plan.metrics.physical_slots, anchored.plan.metrics.reused_allocations
    );
    println!(
        "scope region retained-slots={} resets={}",
        anchored.scoped_regions.physical_slots, anchored.scoped_regions.resets
    );
    println!("frame arena  retained-slots={}", anchored.frame_arena_slots);
    println!(
        "RC baseline  decrements={} zero-tests={}",
        anchored.reference_counting.decrements, anchored.reference_counting.zero_tests
    );
    println!(
        "tracing/32   peak-slots={} collections={} roots={} marked={} edges={} swept={}",
        anchored.tracing.peak_resident_locals,
        anchored.tracing.collections,
        anchored.tracing.roots_scanned,
        anchored.tracing.objects_traced,
        anchored.tracing.edges_scanned,
        anchored.tracing.objects_swept
    );

    let loop_template =
        Template::compile(&[Instr::Int(1), Instr::Int(2), Instr::Pair, Instr::Drop])?;
    let loop_run = loop_template.run(10_000);
    println!("\n10,000 dynamic loop iterations from one static plan");
    println!(
        "template     dynamic-allocations={} exact-frees={} slots={} reuses={}",
        loop_run.dynamic_allocations,
        loop_run.exact_frees,
        loop_run.allocator_slots,
        loop_run.static_reuses
    );
    println!(
        "baselines    frame-arena={} iteration-arena={} naive-RC-zero-tests={}",
        loop_run.frame_arena_slots, loop_run.iteration_arena_slots, loop_run.rc_zero_tests
    );

    let y = store.intern(Node::Hole("y".into()));
    let y_plus_two = store.intern(Node::Add(y, two));
    let binary_program = store.intern(Node::Mul(fast, y_plus_two));
    let mut inet = lower::to_inet(&store, binary_program, "result")?;
    let inet_source = inet.snapshot_cid();
    inet.bind(&BTreeMap::from([("input".into(), InetValue::Int(40))]));
    let inet_compile = inet.reduce(100, Schedule::LowestWire)?;
    let inet_residual = inet.snapshot_cid();
    inet.bind(&BTreeMap::from([("y".into(), InetValue::Int(1))]));
    let inet_runtime = inet.reduce(100, Schedule::LowestWire)?;
    println!("\nport-level interaction net");
    println!("source       {inet_source}");
    println!(
        "residual     {inet_residual}  compile-rewrites={}",
        inet_compile.rewrites
    );
    println!("result       {:?}", inet.outputs().get("result"));
    println!(
        "runtime      rewrites={} deleted={} fresh-cells={} reused-cells={} peak-live={}",
        inet_runtime.rewrites,
        inet_runtime.deleted_agents,
        inet_runtime.fresh_agent_cells,
        inet_runtime.reused_agent_cells,
        inet_runtime.peak_live_agents
    );

    let mut guarded_inet = lower::to_inet(&store, guarded_call, "guarded-result")?;
    guarded_inet.bind(&BTreeMap::from([
        ("guarded.flag".into(), InetValue::Bool(true)),
        ("guarded.input".into(), InetValue::Int(41)),
    ]));
    let guarded_inet_stats = guarded_inet.reduce(100, Schedule::LowestWire)?;
    println!("\nguarded interaction-net call");
    println!(
        "result       {:?}  rewrites={} peak-live={} final-live={}",
        guarded_inet.outputs().get("guarded-result"),
        guarded_inet_stats.rewrites,
        guarded_inet_stats.peak_live_agents,
        guarded_inet.live_agents()
    );

    Ok(())
}

#[cfg(test)]
mod seed_driver_tests {
    use super::*;
    use march_research::ReduceError;

    fn setup(source: &str) -> (Store, Seed, Cid) {
        let mut store = Store::new();
        let seed = Seed::build(&mut store, Syntax::NameFirst);
        let source = store.intern(Node::Const(Atom::Text(source.into())));
        let state = seed.state(&mut store, source);
        (store, seed, state)
    }

    #[test]
    fn batched_driver_matches_direct_images_and_does_not_stop_at_last_token() {
        for source in [
            "",
            "1 2 +",
            "9223372036854775807 1 + drop 0",
            "( 9223372036854775807 1 + )",
        ] {
            for batch in [1, 3, 64] {
                let (mut store, seed, initial) = setup(source);
                let direct = seed::resume(&mut store, seed.runner, initial, u32::MAX, 20_000_000)
                    .unwrap()
                    .root;
                let expected = Image::from_store(&store, &[seed.runner, direct]).unwrap();
                let (state, stats) =
                    run_seed(&mut store, seed.runner, initial, batch, 20_000_000).unwrap();
                assert_eq!(
                    Image::from_store(&store, &[seed.runner, state]).unwrap(),
                    expected
                );
                assert!(stats.collections > 0);
            }
        }
    }

    #[test]
    fn batched_driver_rejects_zero_quota_and_preserves_the_total_work_limit() {
        let (mut store, seed, initial) = setup(&"1 drop ".repeat(100));
        assert!(run_seed(&mut store, seed.runner, initial, 0, 1000).is_err());
        // Each individual token fits; the entire program doesn't. Resetting
        // fuel every batch would incorrectly allow this run to succeed.
        seed::resume(&mut store, seed.runner, initial, 1, 1000).unwrap();
        let error = match run_seed(&mut store, seed.runner, initial, 1, 1000) {
            Err(error) => error,
            Ok(_) => panic!("driver reset fuel between epochs"),
        };
        assert!(matches!(
            error.downcast_ref::<ReduceError>(),
            Some(ReduceError::BudgetExhausted { .. })
        ));
    }
}

fn seed_field(store: &Store, record: Cid, field: &str) -> Result<Cid, Box<dyn std::error::Error>> {
    let Some(Node::Record(fields)) = store.get(record) else {
        return Err("seed returned an unresolved or malformed state".into());
    };
    fields
        .iter()
        .find(|(name, _)| name == field)
        .map(|(_, value)| *value)
        .ok_or_else(|| format!("seed state lacks field {field}").into())
}

#[derive(Default)]
struct SeedRunStats {
    steps: usize,
    collections: usize,
    reclaimed: usize,
    peak_nodes: usize,
}

// This adapter owns this store's live roots. It inspects reader status, never
// source tokens. A positive-quota resume of this seed advances the cursor or
// finishes EOF/error handling. A successful fixed point therefore certifies
// EOF observation, unlike merely reaching the last token's byte position.
fn run_seed(
    store: &mut Store,
    runner: Cid,
    mut state: Cid,
    token_batch: u32,
    work: usize,
) -> Result<(Cid, SeedRunStats), Box<dyn std::error::Error>> {
    if token_batch == 0 {
        return Err("seed token batch must be positive".into());
    }
    let mut stats = SeedRunStats::default();
    loop {
        let result = seed::resume(store, runner, state, token_batch, work - stats.steps)?;
        stats.steps += result.stats.steps;
        stats.peak_nodes = stats.peak_nodes.max(store.len());
        let complete = result.root == state;
        state = result.root;
        let collection = store.collect(&[runner, state])?;
        stats.collections += 1;
        stats.reclaimed += collection.reclaimed;
        let error = seed_field(store, state, "error")?;
        if store.get(error) != Some(&Node::Const(Atom::Unit)) {
            let token = seed_field(store, state, "token")?;
            return Err(format!("{} near {}", store.format(error), store.format(token)).into());
        }
        if complete {
            return Ok((state, stats));
        }
    }
}

fn evaluate_source(source: &str, syntax: Syntax) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();
    let seed = Seed::build(&mut store, syntax);
    let source = store.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut store, source);
    let (state, stats) = run_seed(&mut store, seed.runner, state, 64, 20_000_000)?;
    let mut stack = seed_field(&store, state, "stack")?;
    let mut rendered = Vec::new();
    while let Some(Node::Pair(cell, tail)) = store.get(stack) {
        let value = seed_field(&store, *cell, "value")?;
        rendered.push(match store.get(value) {
            Some(Node::Quote { params, .. }) => format!("quote/{params}:{}", value.short()),
            Some(Node::Family { .. }) => format!("handler:{}", value.short()),
            _ => store.format(value),
        });
        stack = *tail;
    }
    if store.get(stack) != Some(&Node::Const(Atom::Unit)) {
        return Err("seed returned a malformed stack".into());
    }
    println!("stack (top first): [{}]", rendered.join(", "));
    println!(
        "image: {}",
        Image::from_store(&store, &[seed.runner, state])?.cid()
    );
    println!("reduction steps: {}", stats.steps);
    println!(
        "collection: epochs={} reclaimed-nodes={} peak-store-nodes={} retained-nodes={}",
        stats.collections,
        stats.reclaimed,
        stats.peak_nodes,
        store.len()
    );
    Ok(())
}
