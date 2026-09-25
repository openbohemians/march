use march_research::fast::{Binary, Context, Error, Executor, Literal, Op, Program, Value, source};

const COUNTDOWN: &str = "
    : zero 0 = ; : always drop true ; : base drop 0 ;
    : step 1 - recur 1 1 ;
    family countdown 1 1 zero base always step ;";

fn generic(p: &Program, word: usize, args: &[Literal], budget: usize) -> Result<Vec<Value>, Error> {
    let mut e = Executor::new(p);
    let handles = e.start(word, args, &Context::new(), budget)?;
    handles.into_iter().map(|h| e.force(h)).collect()
}

#[test]
fn scalar_contextual_countdown_has_constant_workspace() {
    let mut p = Program::new();
    source::compile(&mut p, COUNTDOWN).unwrap();
    let word = p.lookup("countdown").unwrap();
    assert!(p.is_tail_loop(word).unwrap());
    let mut e = Executor::new(&p);
    e.cell_limit = 16;
    e.argument_limit = 2;
    for depth in [0, 100, 1_000, 10_000, 100_000] {
        assert_eq!(
            e.run(word, &[Literal::Int(depth)], &Context::new(), 2_000_000),
            Ok(vec![Value::Int(0)])
        );
        assert_eq!(e.stats().tail_iterations, depth as usize + 1);
        assert_eq!(e.stats().primitive_ops, depth as usize * 2 + 1);
        assert_eq!(e.stats().peak_cells, 0);
        assert_eq!(e.stats().peak_frames, 0);
        assert_eq!(e.stats().peak_registers, 3);
        assert_eq!(e.stats().peak_arguments, 1);
    }
}

#[test]
fn results_and_errors_match_selective_evaluator() {
    let mut p = Program::new();
    source::compile(&mut p, COUNTDOWN).unwrap();
    let word = p.lookup("countdown").unwrap();
    for arg in [
        Literal::Int(0),
        Literal::Int(1),
        Literal::Int(100),
        Literal::Bool(true),
        Literal::Unit,
        Literal::Int(i64::MIN),
    ] {
        assert_eq!(
            Executor::new(&p).run(word, &[arg], &Context::new(), 100_000),
            generic(&p, word, &[arg], 100_000)
        );
    }
}

#[test]
fn source_entry_and_images_keep_derived_loop_without_changing_identity() {
    let mut p = Program::new();
    let word = source::compile(&mut p, &format!("{COUNTDOWN} 100 countdown")).unwrap();
    assert!(p.is_tail_loop(word).unwrap());
    let image = p.to_image(word).unwrap();
    let (q, loaded) = Program::from_image(&image).unwrap();
    assert!(q.is_tail_loop(loaded).unwrap());
    assert_eq!(p.cid(word), q.cid(loaded));
    assert_eq!(image, q.to_image(loaded).unwrap());
    let mut e = Executor::new(&q);
    e.cell_limit = 16;
    assert_eq!(
        e.run(loaded, &[], &Context::new(), 100_000),
        Ok(vec![Value::Int(0)])
    );
}

#[test]
fn unselected_recursive_argument_is_never_evaluated() {
    let mut p = Program::new();
    source::compile(
        &mut p,
        ": zero 0 = ; : always drop true ; : base drop 7 ;
        : bad drop 9223372036854775807 1 + recur 1 1 ;
        family f 1 1 zero base always bad ;",
    )
    .unwrap();
    let word = p.lookup("f").unwrap();
    assert!(p.is_tail_loop(word).unwrap());
    for (arg, expected) in [(0, Ok(vec![Value::Int(7)])), (1, Err(Error::Overflow))] {
        assert_eq!(
            Executor::new(&p).run(word, &[Literal::Int(arg)], &Context::new(), 1000),
            expected
        );
        assert_eq!(generic(&p, word, &[Literal::Int(arg)], 1000), expected);
    }
}

#[test]
fn nonstrict_first_guard_falls_back_and_does_not_force_bad_input() {
    let mut p = Program::new();
    let word = source::compile(
        &mut p,
        ": yes drop true ; : base drop 7 ;
        : step 1 - recur 1 1 ; family f 1 1 yes base yes step ;
        9223372036854775807 1 + f",
    )
    .unwrap();
    assert!(!p.is_tail_loop(p.lookup("f").unwrap()).unwrap());
    assert!(!p.is_tail_loop(word).unwrap());
    assert_eq!(
        Executor::new(&p).run(word, &[], &Context::new(), 1000),
        Ok(vec![Value::Int(7)])
    );
}

#[test]
fn context_dependent_first_guard_keeps_lazy_fallback() {
    let mut p = Program::new();
    source::compile(
        &mut p,
        ": enabled drop ctx enabled ; : base drop 7 ;
        : yes drop true ; : step 1 - recur 1 1 ;
        family f 1 1 enabled base yes step ;",
    )
    .unwrap();
    assert!(!p.is_tail_loop(p.lookup("f").unwrap()).unwrap());
}

#[test]
fn same_argument_alias_cycles_but_equal_recomputed_value_does_not() {
    for (step, expected) in [
        ("recur 1 1", Error::Cycle),
        ("0 + recur 1 1", Error::Budget),
    ] {
        let mut p = Program::new();
        source::compile(
            &mut p,
            &format!(
                ": zero 0 = ; : base drop 0 ; : yes drop true ;
            : step {step} ; family f 1 1 zero base yes step ;"
            ),
        )
        .unwrap();
        let word = p.lookup("f").unwrap();
        assert!(p.is_tail_loop(word).unwrap());
        assert_eq!(
            Executor::new(&p).run(word, &[Literal::Int(1)], &Context::new(), 1000),
            Err(expected.clone())
        );
        assert_eq!(generic(&p, word, &[Literal::Int(1)], 1000), Err(expected));
    }
}

#[test]
fn budget_storage_reset_and_stale_handles_remain_bounded() {
    let mut p = Program::new();
    source::compile(&mut p, COUNTDOWN).unwrap();
    let word = p.lookup("countdown").unwrap();
    let mut e = Executor::new(&p);
    let h = e
        .start(word, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    for budget in 0..30 {
        assert_eq!(
            e.run(word, &[Literal::Int(-1)], &Context::new(), budget),
            Err(Error::Budget)
        );
        assert_eq!(e.stats().steps, budget);
    }
    assert_eq!(e.force(h), Err(Error::StaleHandle));
    e.cell_limit = 2;
    assert_eq!(
        e.run(word, &[Literal::Int(1)], &Context::new(), 1000),
        Err(Error::StorageLimit)
    );
    e.cell_limit = 3;
    e.argument_limit = 0;
    assert_eq!(
        e.run(word, &[Literal::Int(1)], &Context::new(), 1000),
        Err(Error::StorageLimit)
    );
    e.argument_limit = 1;
    assert_eq!(
        e.run(word, &[Literal::Int(1)], &Context::new(), 1000),
        Ok(vec![Value::Int(0)])
    );
}

#[test]
fn wrappers_do_not_discard_input_transformation() {
    let mut p = Program::new();
    let entry = source::compile(
        &mut p,
        &format!(
            "{COUNTDOWN}
        : wrapped 1 - countdown ; -9223372036854775808 wrapped"
        ),
    )
    .unwrap();
    assert!(p.is_tail_loop(p.lookup("wrapped").unwrap()).unwrap());
    assert!(!p.is_tail_loop(entry).unwrap());
    assert_eq!(
        Executor::new(&p).run(entry, &[], &Context::new(), 1000),
        Err(Error::Overflow)
    );
}

#[test]
fn no_matching_clause_and_guard_error_precedence_match() {
    for (guard, arg) in [("0 <", 1), ("0 =", i64::MIN)] {
        let mut p = Program::new();
        source::compile(
            &mut p,
            &format!(
                ": guard {guard} ;
            : step 1 - recur 1 1 ; family f 1 1 guard step ;"
            ),
        )
        .unwrap();
        let word = p.lookup("f").unwrap();
        assert!(p.is_tail_loop(word).unwrap());
        assert_eq!(
            Executor::new(&p).run(word, &[Literal::Int(arg)], &Context::new(), 1000),
            generic(&p, word, &[Literal::Int(arg)], 1000)
        );
    }
}

#[test]
fn later_guard_fault_precedes_recursive_body_fault() {
    let mut p = Program::new();
    source::compile(
        &mut p,
        ": zero 0 = ; : base drop 7 ;
        : badguard drop 9223372036854775807 1 + 0 = ;
        : badbody true + recur 1 1 ;
        family f 1 1 zero base badguard badbody ;",
    )
    .unwrap();
    let word = p.lookup("f").unwrap();
    assert!(p.is_tail_loop(word).unwrap());
    for (arg, expected) in [
        (Literal::Int(0), Ok(vec![Value::Int(7)])),
        (Literal::Int(1), Err(Error::Overflow)),
        (
            Literal::Bool(true),
            Err(Error::Type("binary operand types")),
        ),
    ] {
        assert_eq!(
            Executor::new(&p).run(word, &[arg], &Context::new(), 1000),
            expected
        );
        assert_eq!(generic(&p, word, &[arg], 1000), expected);
    }
}

#[test]
fn malformed_dispatch_projection_stays_checked_not_an_optimizer_panic() {
    let mut p = Program::new();
    let guard = p
        .add_word(1, vec![Op::Arg(0), Op::Binary(Binary::Eq, 0, 0)], vec![1])
        .unwrap();
    let empty = p.add_word(1, vec![], vec![]).unwrap();
    let word = p
        .add_word(
            1,
            vec![
                Op::Arg(0),
                Op::Dispatch {
                    clauses: vec![(guard, empty)],
                    arguments: vec![0],
                },
                Op::Project { call: 1, output: 0 },
            ],
            vec![2],
        )
        .unwrap();
    assert!(!p.is_tail_loop(word).unwrap());
    assert_eq!(
        Executor::new(&p).run(word, &[Literal::Int(0)], &Context::new(), 1000),
        Err(Error::Output(0))
    );
}

#[test]
fn multioutput_families_and_selective_observation_keep_general_evaluator() {
    let mut p = Program::new();
    source::compile(
        &mut p,
        ": zero 0 = ; : yes drop true ;
        : base drop 0 ctx missing ; : step 1 - recur 1 2 ;
        family f 1 2 zero base yes step ;",
    )
    .unwrap();
    let word = p.lookup("f").unwrap();
    assert!(!p.is_tail_loop(word).unwrap());
    let mut e = Executor::new(&p);
    let h = e
        .start(word, &[Literal::Int(2)], &Context::new(), 1000)
        .unwrap();
    assert_eq!(e.force(h[0]), Ok(Value::Int(0)));
    assert_eq!(e.force(h[1]), Err(Error::MissingContext("missing".into())));
    assert_eq!(e.force(h[0]), Ok(Value::Int(0)));
}

#[test]
fn oversized_tail_dispatch_falls_back_with_same_result() {
    let mut p = Program::new();
    source::compile(&mut p, COUNTDOWN).unwrap();
    let zero = p.lookup("zero").unwrap();
    let base = p.lookup("base").unwrap();
    let always = p.lookup("always").unwrap();
    let step = p.lookup("step").unwrap();
    let mut clauses = vec![(zero, base); 1024];
    clauses.push((always, step));
    let word = p.add_family(1, 1, clauses).unwrap();
    assert!(!p.is_tail_loop(word).unwrap());
    assert_eq!(
        Executor::new(&p).run(word, &[Literal::Int(0)], &Context::new(), 1000),
        Ok(vec![Value::Int(0)])
    );
}
