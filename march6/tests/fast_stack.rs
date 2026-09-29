use march_research::fast::{
    Context, Error, Literal, Program,
    stack::{Machine, Value},
    store::{FrozenNode, Store},
    stream,
};

fn compile(source: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, source).unwrap();
    (p, w)
}
fn run(source: &str) -> Result<(Vec<Value>, Store), Error> {
    let (p, w) = compile(source);
    Machine::new(&p).run(w, &[], &Context::new(), 100_000, &Store::new())
}
fn stored(s: &Store, key: &str) -> i64 {
    match s.get(&[key]).unwrap().unwrap().nodes().last().unwrap() {
        FrozenNode::Int(n) => *n,
        _ => panic!("expected integer"),
    }
}

#[test]
fn actual_stack_composition_and_row_prefixes() {
    assert_eq!(
        run(": square dup * ; 99 7 square 2 + swap").unwrap().0,
        [Value::Int(51), Value::Int(99)]
    );
    assert_eq!(run("1 2 over swap + +").unwrap().0, [Value::Int(4)]);
    assert_eq!(
        run("3 2 gt? 2 2 gte? 2 3 lte?").unwrap().0,
        [Value::Bool(true), Value::Bool(true), Value::Bool(true)]
    );
}

#[test]
fn drop_does_not_cancel_execution_or_errors() {
    for source in [
        "9223372036854775807 1 + drop 7",
        ": fail 9223372036854775807 1 + ; fail drop 7",
        "true 7 9223372036854775807 1 + select",
    ] {
        assert_eq!(run(source), Err(Error::Overflow));
    }
    assert!(matches!(
        run(r#""missing" store.get drop"#),
        Err(Error::Store(_))
    ));
}

#[test]
fn writes_and_reads_follow_program_order() {
    let (values, s) = run(r#"
        : bump "x" store.get 1 + "x" store.put ;
        10 "x" store.put "x" store.get bump bump "x" store.get
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(10), Value::Int(12)]);
    assert_eq!(stored(&s, "x"), 12);
}

#[test]
fn dynamic_writers_run_even_when_results_are_dropped() {
    for source in [
        r#"[ 5 "x" store.put 1 ] apply 0 1 drop"#,
        r#": w [ 5 "x" store.put 1 ] apply 0 1 drop ; w"#,
        r#"[ 5 "x" store.put 1 ] "q" store.put "q" store.get apply 0 1 drop"#,
        r#"[ 5 "x" store.put 1 ] call drop"#,
    ] {
        let (values, s) = run(source).unwrap();
        assert!(values.is_empty());
        assert_eq!(stored(&s, "x"), 5);
    }
}

#[test]
fn quotations_defer_bodies_until_called() {
    let (values, _) = run("[ 9223372036854775807 1 + ] drop 7").unwrap();
    assert_eq!(values, [Value::Int(7)]);
    assert_eq!(
        run("[ 9223372036854775807 1 + ] call"),
        Err(Error::Overflow)
    );
}

#[test]
fn selected_families_use_forked_guard_inputs_and_skip_other_bodies() {
    let (values, s) = run(r#"
        : positive 0 gt? ; : yes drop true ;
        : selected 2 * 7 "x" store.put ;
        : fail drop 9223372036854775807 1 + ;
        family choose 1 1 positive selected yes fail ;
        99 3 choose
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(99), Value::Int(6)]);
    assert_eq!(stored(&s, "x"), 7);
}

#[test]
fn guards_can_read_but_cannot_write_through_dynamic_calls() {
    let (values, _) = run(r#"
        : enabled "mode" store.get ; : body 42 ;
        family f 0 1 enabled body ; true "mode" store.put f
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(42)]);
    assert!(matches!(
        run(r#"
        : guard [ 1 "x" store.put true ] apply 0 1 ; : body 7 ;
        family f 0 1 guard body ; f
    "#),
        Err(Error::Type("guards cannot write state"))
    ));
}

#[test]
fn recursion_has_explicit_bounded_continuations() {
    let source = r#"
        : zero 0 eq? ; : yes drop true ; : done drop ;
        : step dup "x" store.put 1 - recur 1 0 ;
        family down 1 0 zero done yes step ; 1000 down
    "#;
    let (p, w) = compile(source);
    let mut vm = Machine::new(&p);
    let (values, s) = vm
        .run(w, &[], &Context::new(), 200_000, &Store::new())
        .unwrap();
    assert!(values.is_empty());
    assert_eq!(stored(&s, "x"), 1);
    assert!(vm.stats().peak_continuations < 20);
    vm.continuation_limit = 20;
    assert!(
        vm.run(w, &[], &Context::new(), 200_000, &Store::new())
            .is_ok()
    );
    vm.tail_call_optimization = false;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 200_000, &Store::new()),
        Err(Error::StorageLimit)
    );
    let (p, w) = compile(": loop recur 0 1 ; loop drop");
    assert_eq!(
        Machine::new(&p).run(w, &[], &Context::new(), 100, &Store::new()),
        Err(Error::Budget)
    );
}

#[test]
fn failure_does_not_publish_state_and_machine_can_be_reused() {
    let (_, initial) = run(r#"10 "x" store.put"#).unwrap();
    let (mut p, bad) = compile(r#"20 "x" store.put 9223372036854775807 1 + drop"#);
    let good = stream::compile(&mut p, r#""x" store.get"#).unwrap();
    let mut vm = Machine::new(&p);
    assert_eq!(
        vm.run(bad, &[], &Context::new(), 1000, &initial),
        Err(Error::Overflow)
    );
    assert_eq!(
        vm.run(good, &[], &Context::new(), 1000, &initial)
            .unwrap()
            .0,
        [Value::Int(10)]
    );
    assert_eq!(stored(&initial, "x"), 10);
}

#[test]
fn contexts_apply_contracts_and_unsupported_features_are_checked() {
    let (p, w) = compile("ctx offset +");
    assert_eq!(
        Machine::new(&p)
            .run(
                w,
                &[Literal::Int(3)],
                &Context::from([("offset".into(), Literal::Int(4))]),
                1000,
                &Store::new()
            )
            .unwrap()
            .0,
        [Value::Int(7)]
    );
    assert!(matches!(
        run("[ 7 ] apply 0 2"),
        Err(Error::OutputArity { .. })
    ));
    assert!(run("1 2 pair drop").unwrap().0.is_empty());
    assert!(run("1 2 tuple 2 drop").unwrap().0.is_empty());
}

#[test]
fn strict_cli_is_explicit_and_rejects_unversioned_images() {
    let bin = env!("CARGO_BIN_EXE_march-fast");
    let ok = std::process::Command::new(bin)
        .args(["--stack", "--eval", "7 dup *"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    assert_eq!(String::from_utf8(ok.stdout).unwrap().trim(), "[Int(49)]");
    let bad = std::process::Command::new(bin)
        .args(["--stack", "--eval", "9223372036854775807 1 + drop"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let image = std::process::Command::new(bin)
        .args(["--stack", "--load-image", "missing.image"])
        .output()
        .unwrap();
    assert!(!image.status.success());
    assert!(
        String::from_utf8(image.stderr)
            .unwrap()
            .contains("identity must be versioned")
    );
}

#[test]
fn fuel_cuts_and_value_limits_fail_cleanly() {
    let (p, w) = compile(r#"7 "x" store.put "x" store.get"#);
    let initial = Store::new();
    let mut succeeded = false;
    for fuel in 0..100 {
        let mut vm = Machine::new(&p);
        match vm.run(w, &[], &Context::new(), fuel, &initial) {
            Ok((values, state)) => {
                assert_eq!(values, [Value::Int(7)]);
                assert_eq!(stored(&state, "x"), 7);
                succeeded = true;
            }
            Err(e) => assert_eq!(e, Error::Budget),
        }
        assert!(initial.is_empty());
    }
    assert!(succeeded);
    let (p, w) = compile("1 dup dup dup");
    let mut vm = Machine::new(&p);
    vm.stack_limit = 2;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 1000, &initial),
        Err(Error::StorageLimit)
    );
    let (p, w) = compile(r#""oversized" drop"#);
    let mut vm = Machine::new(&p);
    vm.text_byte_limit = 4;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 1000, &initial),
        Err(Error::StorageLimit)
    );
}

#[test]
fn scalar_results_match_existing_engine_without_using_its_execution_graph() {
    for source in [
        "dup *",
        ": square dup * ; square square",
        "dup 3 lt? swap 2 + 17 select",
    ] {
        let (p, w) = compile(source);
        let mut stack = Machine::new(&p);
        let mut graph = march_research::fast::Executor::new(&p);
        for n in -20..20 {
            let args = [Literal::Int(n)];
            let context = Context::new();
            assert_eq!(
                stack
                    .run(w, &args, &context, 10000, &Store::new())
                    .unwrap()
                    .0,
                graph
                    .run(w, &args, &context, 10000)
                    .unwrap()
                    .into_iter()
                    .map(|v| match v {
                        march_research::fast::Value::Int(n) => Value::Int(n),
                        march_research::fast::Value::Bool(b) => Value::Bool(b),
                        _ => panic!("unexpected scalar fixture result"),
                    })
                    .collect::<Vec<_>>()
            );
        }
    }
}
