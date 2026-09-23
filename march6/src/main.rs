use march_research::inet::{Schedule, Value as InetValue};
use march_research::lower;
use march_research::memory::{self, InputIdentities, Instr, Ownership};
use march_research::template::Template;
use march_research::{
    Atom, Bindings, Clause, DEFAULT_REDUCTION_BUDGET, Image, Node, Reducer, SpecializationCache,
    Store,
};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("demo") {
        eprintln!("usage: march-research demo");
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
        "reductions   compile steps={} visited={} rewrites={}; runtime steps={} visited={} rewrites={}",
        compiled.stats.steps,
        compiled.stats.visited,
        compiled.stats.rewritten,
        result.stats.steps,
        result.stats.visited,
        result.stats.rewritten
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
