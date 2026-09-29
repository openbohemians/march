use march_research::fast::{
    Context, Error, Executor, Literal, Op, Program, Value,
    store::{FrozenNode, Store},
    stream,
};

fn compile(s: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, s).unwrap();
    (p, w)
}
fn run(s: &str) -> Result<(Vec<Value>, Store), Error> {
    let (p, w) = compile(s);
    Executor::new(&p).run_with_store(w, &[], &Context::new(), 100_000, &Store::new())
}
fn integer(store: &Store, path: &[&str]) -> i64 {
    match store.get(path).unwrap().unwrap().nodes().last().unwrap() {
        FrozenNode::Int(n) => *n,
        value => panic!("expected integer, got {value:?}"),
    }
}

#[test]
fn writes_complete_even_with_an_empty_stack() {
    let (values, state) = run(r#"42 "answer" store.put"#).unwrap();
    assert!(values.is_empty());
    assert_eq!(integer(&state, &["answer"]), 42);
}

#[test]
fn compiler_execution_rejects_runtime_store_effects_atomically() {
    for source in [
        r#": wrong dup drop 1 "x" store.put ; immediate wrong"#,
        r#": wrong dup "x" store.put ; immediate wrong"#,
        r#": wrong dup drop "x" store.get drop ; immediate wrong"#,
        r#": reader "x" store.get ; : wrong ' reader apply 0 1 stream.emit-literal ; immediate wrong"#,
    ] {
        let mut p = stream::seed().unwrap();
        let before = p.len();
        let error = stream::compile(&mut p, source).unwrap_err();
        assert!(
            format!("{error:?}").contains("runtime store operations"),
            "{error:?}"
        );
        assert_eq!(p.len(), before);
        assert!(p.lookup("wrong").is_none());
    }
}

#[test]
fn delayed_read_observes_its_snapshot_not_the_latest_state() {
    let (p, w) = compile(r#"10 "x" store.put "x" store.get 20 "x" store.put "x" store.get"#);
    let mut e = Executor::new(&p);
    let out = e
        .start_with_store(w, &[], &Context::new(), 10000, &Store::new())
        .unwrap();
    let state = e.finish_state().unwrap();
    assert_eq!(integer(&state, &["x"]), 20);
    // Intentionally observe in reverse order, after all writes have completed.
    assert_eq!(e.force(out[1]).unwrap(), Value::Int(20));
    assert_eq!(e.force(out[0]).unwrap(), Value::Int(10));
    assert_eq!(e.finish_state().unwrap().cid(), state.cid());
}

#[test]
fn dynamic_readers_and_their_wrappers_are_not_merged_across_snapshots() {
    let (p, w) = compile(
        r#"
      : read-x "x" store.get ; : dynamic-reader ' read-x apply 0 1 ;
      : wrapper dynamic-reader ;
      10 "x" store.put wrapper 20 "x" store.put wrapper
    "#,
    );
    assert!(p.effects(p.lookup("wrapper").unwrap()).unwrap().dynamic);
    let mut e = Executor::new(&p);
    let out = e
        .start_with_store(w, &[], &Context::new(), 10000, &Store::new())
        .unwrap();
    e.finish_state().unwrap();
    assert_eq!(e.force(out[1]).unwrap(), Value::Int(20));
    assert_eq!(e.force(out[0]).unwrap(), Value::Int(10));
}

#[test]
fn dropped_call_results_keep_required_writes_and_repeated_calls_are_distinct() {
    let (values, state) = run(r#"
      : increment "x" store.get 1 + "x" store.put 99 ;
      0 "x" store.put increment drop increment drop "x" store.get
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(2)]);
    assert_eq!(integer(&state, &["x"]), 2);
}

#[test]
fn unchosen_family_bodies_do_not_write_or_evaluate_failing_work() {
    let (values, state) = run(r#"
      : positive 0 gt? ; : always drop true ;
      : selected drop 7 "x" store.put 17 ;
      : unselected drop 9223372036854775807 1 + "x" store.put 27 ;
      family choose 1 1 positive selected always unselected ;
      3 choose drop
    "#)
    .unwrap();
    assert!(values.is_empty());
    assert_eq!(integer(&state, &["x"]), 7);
}

#[test]
fn selecting_a_value_does_not_erase_prior_executed_words_writes() {
    let (values, state) = run(r#"
      : a 1 "x" store.put 10 ; : b 2 "x" store.put 20 ;
      true a b select
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(10)]);
    assert_eq!(integer(&state, &["x"]), 2);
}

#[test]
fn unused_pure_calculations_and_unused_reads_remain_lazy() {
    let (values, state) = run(r#"
      : spin recur 0 1 ;
      spin drop 9223372036854775807 1 + drop "missing" store.get drop
      7 "x" store.put 42
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(42)]);
    assert_eq!(integer(&state, &["x"]), 7);
}

#[test]
fn failed_write_never_returns_partial_invocation_state() {
    let (_, initial) = run(r#"10 "x" store.put"#).unwrap();
    let before = initial.cid();
    let (p, w) = compile(r#"20 "x" store.put 7 9223372036854775807 1 + pair "bad" store.put"#);
    let mut e = Executor::new(&p);
    assert_eq!(
        e.run_with_store(w, &[], &Context::new(), 10000, &initial),
        Err(Error::Overflow)
    );
    assert_eq!(e.finish_state(), Err(Error::Overflow));
    assert_eq!(initial.cid(), before);
    assert_eq!(integer(&initial, &["x"]), 10);
}

#[test]
fn tuple_paths_and_closed_quotations_cross_the_store_boundary() {
    let (p, w) = compile(
        r#": spin recur 0 1 ; ' spin "ns" "code" tuple 2 store.put "ns" "code" tuple 2 store.get"#,
    );
    let (values, state) = Executor::new(&p)
        .run_with_store(w, &[], &Context::new(), 10000, &Store::new())
        .unwrap();
    assert_eq!(
        values,
        [Value::Quote(p.cid(p.lookup("spin").unwrap()).unwrap())]
    );
    assert!(state.get(&["ns", "code"]).unwrap().is_some());
}

#[test]
fn stateful_recursion_preserves_sequential_updates() {
    let (values, state) = run(r#"
      : zero? 0 eq? ; : yes drop true ;
      : done drop ;
      : step dup "last" store.put 1 - recur 1 0 ;
      family countdown 1 0 zero? done yes step ;
      50 countdown "last" store.get
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(1)]);
    assert_eq!(integer(&state, &["last"]), 1);
}

#[test]
fn zero_argument_state_recursion_is_not_mistaken_for_a_pure_demand_cycle() {
    let (values, state) = run(r#"
      : stop? "n" store.get 0 eq? ; : yes true ; : done ;
      : step "n" store.get 1 - "n" store.put recur 0 0 ;
      family go 0 0 stop? done yes step ;
      20 "n" store.put go "n" store.get
    "#)
    .unwrap();
    assert_eq!(values, [Value::Int(0)]);
    assert_eq!(integer(&state, &["n"]), 0);
}

#[test]
fn overwritten_writes_still_evaluate_and_validate_their_values() {
    assert_eq!(
        run(r#"9223372036854775807 1 + "x" store.put 42 "x" store.put"#),
        Err(Error::Overflow)
    );
}

#[test]
fn invalid_write_paths_fail_before_demanding_the_value() {
    assert!(matches!(
        run(r#"9223372036854775807 1 + "" store.put"#),
        Err(Error::Store(_))
    ));
    assert!(matches!(
        run(r#"1 "x" store.put 9223372036854775807 1 + "x" "child" pair store.put"#),
        Err(Error::Store(_))
    ));
}

#[test]
fn frozen_imports_and_partial_state_work_respect_resource_limits() {
    let (_, initial) = run(r#""abcdefgh" "x" store.put"#).unwrap();
    let (p, w) = compile(r#""x" store.get"#);
    let mut e = Executor::new(&p);
    e.text_byte_limit = 4;
    assert_eq!(
        e.run_with_store(w, &[], &Context::new(), 10000, &initial),
        Err(Error::StorageLimit)
    );
    assert_eq!(e.finish_state(), Err(Error::StorageLimit));

    let (p, w) = compile(r#"1 2 pair "x" store.put "x" store.get "y" store.put"#);
    let mut succeeded = false;
    for budget in 0..250 {
        let mut e = Executor::new(&p);
        match e.run_with_store(w, &[], &Context::new(), budget, &Store::new()) {
            Ok((_, state)) => {
                assert_eq!(state.get(&["x"]).unwrap(), state.get(&["y"]).unwrap());
                succeeded = true;
            }
            Err(error) => {
                assert_eq!(error, Error::Budget);
                assert_eq!(e.finish_state(), Err(Error::Budget));
            }
        }
    }
    assert!(succeeded);
}

#[test]
fn malformed_family_bodies_return_errors_instead_of_panicking() {
    let mut p = Program::default();
    let guard = p
        .add_word(0, vec![Op::Const(Literal::Bool(true))], vec![0])
        .unwrap();
    let body = p.add_word(0, vec![], vec![]).unwrap();
    assert!(
        p.add_word(
            0,
            vec![Op::Dispatch {
                clauses: vec![(guard, body), (guard, usize::MAX)],
                arguments: vec![],
            }],
            vec![]
        )
        .is_err()
    );
}

#[test]
fn failed_state_initialization_and_failed_run_into_leave_no_partial_result() {
    let (p, w) = compile(r#"1 "x" store.put 7"#);
    let mut e = Executor::new(&p);
    assert!(matches!(
        e.start_with_store(w, &[], &Context::new(), 0, &Store::new()),
        Err(Error::Budget)
    ));
    assert_eq!(e.finish_state(), Err(Error::Budget));
    let (p, w) = compile(r#"9223372036854775807 1 + "x" store.put 7"#);
    let mut e = Executor::new(&p);
    let mut outputs = vec![Value::Int(99)];
    assert_eq!(
        e.run_into(w, &[], &Context::new(), 1000, &mut outputs),
        Err(Error::Overflow)
    );
    assert!(outputs.is_empty());
}

#[test]
fn pure_selected_branch_can_return_unchanged_state() {
    let (_, state) = run(r#"
      : yes true ; : no false ; : plain 7 ; : writer 8 "x" store.put 9 ;
      family choose 0 1 yes plain no writer ; choose drop
    "#)
    .unwrap();
    assert!(state.is_empty());
}

#[test]
fn state_writing_guards_are_rejected() {
    let mut p = stream::seed().unwrap();
    assert!(
        stream::compile(
            &mut p,
            r#": guard 1 "x" store.put true ; : body 7 ; family bad 0 1 guard body ;"#
        )
        .is_err()
    );
    assert!(p.lookup("bad").is_none());
}

#[test]
fn dynamic_apply_keeps_its_nonwriting_contract() {
    let (values, _) = run(r#": spin recur 0 1 ; ' spin apply 0 1 drop 7"#).unwrap();
    assert_eq!(values, [Value::Int(7)]);
    let result = run(r#": writer 1 "x" store.put 7 ; ' writer apply 0 1"#);
    assert!(matches!(result, Err(Error::Type(_))));
    let (values, state) = run(r#": writer 1 "x" store.put 7 ; ' writer call drop"#).unwrap();
    assert!(values.is_empty());
    assert_eq!(integer(&state, &["x"]), 1);
}

#[test]
fn stored_values_import_as_ready_data_with_existing_value_identity() {
    let (p, w) = compile(r#"6 7 * "é" true tuple 3 "data" store.put "data" store.get"#);
    let mut e = Executor::new(&p);
    let out = e
        .start_with_store(w, &[], &Context::new(), 10000, &Store::new())
        .unwrap();
    let state = e.finish_state().unwrap();
    assert_eq!(
        e.content_id(out[0]).unwrap(),
        state.get(&["data"]).unwrap().unwrap().cid()
    );
    assert_eq!(e.collect(&out, 10000), Err(Error::CollectionBusy));
}

#[test]
fn source_and_image_reload_preserve_effect_summaries_and_behavior() {
    let (p, w) = compile(r#": writer 42 "x" store.put ; writer "x" store.get"#);
    assert!(p.effects(w).unwrap().writes);
    assert!(p.effects(w).unwrap().reads);
    let bytes = p.to_image(w).unwrap();
    let (q, v) = Program::from_image(&bytes).unwrap();
    assert_eq!(p.cid(w).unwrap(), q.cid(v).unwrap());
    assert_eq!(p.effects(w).unwrap(), q.effects(v).unwrap());
    let (values, state) = Executor::new(&q)
        .run_with_store(v, &[], &Context::new(), 10000, &Store::new())
        .unwrap();
    assert_eq!(values, [Value::Int(42)]);
    assert_eq!(integer(&state, &["x"]), 42);
}

#[test]
fn divergent_writes_are_bounded_and_reset_starts_a_fresh_transaction() {
    let (mut p, w) = compile(r#": forever 7 recur 0 1 pair ; forever "x" store.put"#);
    let good = stream::compile(&mut p, r#"8 "x" store.put"#).unwrap();
    let mut e = Executor::new(&p);
    assert!(matches!(
        e.run_with_store(w, &[], &Context::new(), 1000, &Store::new()),
        Err(Error::Budget)
    ));
    let (_, state) = e
        .run_with_store(good, &[], &Context::new(), 1000, &Store::new())
        .unwrap();
    assert_eq!(integer(&state, &["x"]), 8);
}
