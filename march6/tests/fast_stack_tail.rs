use march_research::fast::{
    Context, Error, Literal, Program,
    definition::{Definition, Item},
    stack::{Machine, Stats, Value},
    store::Store,
    stream,
};

fn compile(source: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, source).unwrap();
    (p, w)
}
const DOWN: &str = ": zero 0 eq? ; : yes drop true ; : done drop 0 ; : step 1 - recur 1 1 ; family down 1 1 zero done yes step ; down";
fn execute(source: &str, optimized: bool) -> (Result<(Vec<Value>, Store), Error>, Stats) {
    let (p, w) = compile(source);
    let mut vm = Machine::new(&p);
    vm.tail_call_optimization = optimized;
    let result = vm.run(w, &[], &Context::new(), 100_000, &Store::new());
    (result, vm.stats().clone())
}

#[test]
fn hundred_thousand_tail_iterations_have_constant_control_depth() {
    let (p, w) = compile(DOWN);
    let mut vm = Machine::new(&p);
    vm.continuation_limit = 16;
    let mut peak = None;
    for n in [10, 1000, 100_000] {
        let (values, _) = vm
            .run(
                w,
                &[Literal::Int(n)],
                &Context::new(),
                10_000_000,
                &Store::new(),
            )
            .unwrap();
        assert_eq!(values, [Value::Int(0)]);
        let now = vm.stats().peak_continuations;
        assert!(now <= 16);
        if let Some(earlier) = peak {
            assert_eq!(now, earlier);
        } else {
            peak = Some(now);
        }
        assert!(vm.stats().tail_calls >= n as usize);
        assert_eq!(vm.stats().tuple_nodes, 0);
    }
    vm.tail_call_optimization = false;
    assert_eq!(
        vm.run(
            w,
            &[Literal::Int(1000)],
            &Context::new(),
            100_000,
            &Store::new()
        ),
        Err(Error::StorageLimit)
    );
}

#[test]
fn static_tail_call_chains_reuse_returns() {
    let mut p = Program::default();
    let mut w = p
        .add_definition(Definition::Sequence(vec![Item::Literal(Literal::Int(42))]))
        .unwrap();
    for _ in 0..2000 {
        w = p
            .add_definition(Definition::Sequence(vec![Item::Word(w)]))
            .unwrap();
    }
    let mut vm = Machine::new(&p);
    vm.continuation_limit = 4;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 100_000, &Store::new())
            .unwrap()
            .0,
        [Value::Int(42)]
    );
    assert_eq!(vm.stats().tail_calls, 2000);
    assert!(vm.stats().peak_continuations <= 2);
}

#[test]
fn dynamic_tail_application_and_static_call_keep_prefixes() {
    let (mut p, _) = compile(DOWN);
    let apply = stream::compile(&mut p, "99 100000 ' down apply 1 1").unwrap();
    let call = stream::compile(&mut p, "99 100000 ' down call").unwrap();
    let stored = stream::compile(
        &mut p,
        r#"' down "f" store.put 99 100000 "f" store.get apply 1 1"#,
    )
    .unwrap();
    for w in [apply, call, stored] {
        let mut vm = Machine::new(&p);
        vm.continuation_limit = 16;
        assert_eq!(
            vm.run(w, &[], &Context::new(), 10_000_000, &Store::new())
                .unwrap()
                .0,
            [Value::Int(99), Value::Int(0)]
        );
    }
}

#[test]
fn zero_and_multiple_output_tail_loops_preserve_store_and_stack() {
    for source in [
        r#": zero 0 eq? ; : yes drop true ; : done drop ; : step dup "x" store.put 1 - recur 1 0 ; family down 1 0 zero done yes step ; 999 10000 down "x" store.get"#,
        r#": zero 0 eq? ; : yes drop true ; : done drop 10 20 ; : step dup "x" store.put 1 - recur 1 2 ; family down 1 2 zero done yes step ; 999 10000 down "x" store.get"#,
    ] {
        let (p, w) = compile(source);
        let mut vm = Machine::new(&p);
        vm.continuation_limit = 16;
        let (values, store) = vm
            .run(w, &[], &Context::new(), 2_000_000, &Store::new())
            .unwrap();
        assert_eq!(values.first(), Some(&Value::Int(999)));
        assert_eq!(values.last(), Some(&Value::Int(1)));
        if values.len() == 4 {
            assert_eq!(&values[1..3], &[Value::Int(10), Value::Int(20)]);
        } else {
            assert_eq!(values.len(), 2);
        }
        assert!(store.get(&["x"]).unwrap().is_some());
        assert!(vm.stats().peak_continuations <= 16);
    }
}

#[test]
fn non_tail_work_is_not_discarded() {
    let source = ": zero 0 eq? ; : yes drop true ; : done drop 0 ; : step 1 - recur 1 1 1 + ; family count 1 1 zero done yes step ; 100 count";
    assert_eq!(execute(source, true).0.unwrap().0, [Value::Int(100)]);
    let (p, w) = compile(source);
    let mut vm = Machine::new(&p);
    vm.continuation_limit = 16;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 100_000, &Store::new()),
        Err(Error::StorageLimit)
    );
}

#[test]
fn optimized_and_baseline_execution_agree_on_results_stores_and_errors() {
    for source in [
        "99 7 [ dup * ] apply 1 1 3 +",
        "[ 7 ] apply 0 2",
        ": bad dup recur 0 1 ; 7 bad",
        ": bad 9223372036854775807 1 + ; bad",
        ": no drop false ; : body drop 7 ; family f 1 1 no body ; 3 f",
        ": malformed drop 7 ; : body drop 9 ; family f 1 1 malformed body ; 3 f",
        ": no drop false ; : yes drop true ; : a drop 7 ; : b 2 * ; family f 1 1 no a yes b ; 9 3 f 1 +",
        r#": writer 1 "x" store.put 7 ; ' writer apply 0 1 drop "x" store.get"#,
        r#": guard [ 1 "x" store.put true ] apply 0 1 ; : body 7 ; family f 0 1 guard body ; f"#,
        r#": zero 0 eq? ; : yes drop true ; : done drop ; : step dup "x" store.put 1 - recur 1 0 ; family down 1 0 zero done yes step ; 50 down "x" store.get"#,
    ] {
        let (optimized, a) = execute(source, true);
        let (baseline, b) = execute(source, false);
        assert_eq!(optimized, baseline, "{source}");
        assert_eq!(a.calls, b.calls, "{source}");
        assert_eq!(a.tuple_nodes, b.tuple_nodes, "{source}");
        assert_eq!(a.tuple_fields, b.tuple_fields, "{source}");
        assert_eq!(a.text_bytes_allocated, b.text_bytes_allocated, "{source}");
    }
}

#[test]
fn tail_loops_keep_cumulative_allocation_statistics_and_limits() {
    let source = ": zero 0 eq? ; : yes drop true ; : done drop ; : step dup dup pair drop 1 - recur 1 0 ; family down 1 0 zero done yes step ; 5 down";
    let (p, w) = compile(source);
    for optimized in [false, true] {
        let mut vm = Machine::new(&p);
        vm.tail_call_optimization = optimized;
        vm.tuple_field_limit = 10;
        assert!(
            vm.run(w, &[], &Context::new(), 10000, &Store::new())
                .is_ok()
        );
        assert_eq!(vm.stats().tuple_nodes, 5);
        assert_eq!(vm.stats().tuple_fields, 10);
        vm.tuple_field_limit = 8;
        assert_eq!(
            vm.run(w, &[], &Context::new(), 10000, &Store::new()),
            Err(Error::StorageLimit)
        );
    }
}

#[test]
fn nonterminating_tail_calls_stop_at_fuel_and_reset_cleanly() {
    let (mut p, w) = compile(": spin recur 0 0 ; spin");
    let good = stream::compile(&mut p, "42").unwrap();
    let mut vm = Machine::new(&p);
    vm.continuation_limit = 4;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 100_000, &Store::new()),
        Err(Error::Budget)
    );
    assert!(vm.stats().peak_continuations <= 4);
    assert_eq!(
        vm.run(good, &[], &Context::new(), 1000, &Store::new())
            .unwrap()
            .0,
        [Value::Int(42)]
    );
}
