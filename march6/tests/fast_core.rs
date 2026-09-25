//! Adversarial checks of the conventional executor's public core API.
use march_research::fast::{Binary, Context, Error, Executor, Literal, Op, Program, Value, source};

#[test]
fn malformed_code_is_rejected_before_registration() {
    let mut p = Program::new();
    let invalid = [
        (0, vec![Op::Arg(0)], vec![0]),
        (0, vec![Op::Binary(Binary::Add, 0, 0)], vec![0]),
        (0, vec![Op::Const(Literal::Int(1))], vec![1]),
        (0, vec![], vec![0]),
        (65536, vec![], vec![]),
    ];
    for (inputs, ops, outputs) in invalid {
        assert!(matches!(
            p.add_word(inputs, ops, outputs),
            Err(Error::InvalidCode(_))
        ));
        assert!(p.is_empty());
    }
    assert_eq!(
        p.add_word(0, vec![Op::Const(Literal::Quote(99))], vec![0]),
        Err(Error::UnknownWord(99))
    );
    let identity = p.add_word(1, vec![Op::Arg(0)], vec![0]).unwrap();
    assert_eq!(
        p.add_word(
            0,
            vec![Op::Call {
                word: identity,
                arguments: vec![]
            }],
            vec![0]
        ),
        Err(Error::Arity {
            expected: 1,
            actual: 0
        })
    );
    assert!(matches!(
        p.add_family(0, 1, vec![(identity, identity)]),
        Err(Error::InvalidCode(_))
    ));
    assert!(matches!(
        p.add_family(0, 1, vec![]),
        Err(Error::InvalidCode(_))
    ));
    assert_eq!(p.len(), 1);
}

#[test]
fn zero_output_words_do_not_force_dead_work() {
    let mut p = Program::new();
    let empty = p.add_word(0, vec![], vec![]).unwrap();
    let discarded = p
        .add_word(0, vec![Op::Context("missing".into())], vec![])
        .unwrap();
    let mut executor = Executor::new(&p);
    for word in [empty, discarded] {
        assert_eq!(executor.run(word, &[], &Context::new(), 0), Ok(vec![]));
        assert_eq!(executor.stats().primitive_ops, 0);
        assert!(
            executor
                .start(word, &[], &Context::new(), 0)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn deeply_recursive_countdown_uses_no_recursive_host_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut p = Program::new();
            let countdown = p
                .add_word(
                    1,
                    vec![
                        Op::Arg(0),
                        Op::Const(Literal::Int(0)),
                        Op::Binary(Binary::Eq, 0, 1),
                        Op::Const(Literal::Int(1)),
                        Op::Binary(Binary::Sub, 0, 3),
                        Op::Recur { arguments: vec![4] },
                        Op::Project { call: 5, output: 0 },
                        Op::Select {
                            condition: 2,
                            when_true: 1,
                            when_false: 6,
                        },
                    ],
                    vec![7],
                )
                .unwrap();
            let mut executor = Executor::new(&p);
            assert_eq!(
                executor.run(
                    countdown,
                    &[Literal::Int(10_000)],
                    &Context::new(),
                    1_000_000
                ),
                Ok(vec![Value::Int(0)])
            );
            assert_eq!(executor.stats().calls, 10_001);
            assert_eq!(
                executor.run(countdown, &[Literal::Int(-1)], &Context::new(), 100),
                Err(Error::Budget)
            );
            assert_eq!(executor.stats().steps, 100);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn failures_are_memoized_without_poisoning_independent_outputs() {
    let mut p = Program::new();
    let word = p
        .add_word(
            0,
            vec![
                Op::Const(Literal::Int(i64::MAX)),
                Op::Const(Literal::Int(1)),
                Op::Binary(Binary::Add, 0, 1),
                Op::Binary(Binary::Add, 2, 1),
                Op::Const(Literal::Int(7)),
            ],
            vec![3, 2, 4],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    let outputs = executor.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(executor.force(outputs[0]), Err(Error::Overflow));
    assert_eq!(executor.stats().primitive_ops, 1);
    for handle in [outputs[0], outputs[1], outputs[0]] {
        assert_eq!(executor.force(handle), Err(Error::Overflow));
        assert_eq!(executor.stats().primitive_ops, 1);
    }
    assert_eq!(executor.force(outputs[2]), Ok(Value::Int(7)));
}

#[test]
fn stale_and_foreign_pair_handles_are_rejected() {
    let mut p = Program::new();
    let word = p
        .add_word(
            0,
            vec![
                Op::Const(Literal::Int(7)),
                Op::Context("missing".into()),
                Op::Pair(0, 1),
            ],
            vec![2],
        )
        .unwrap();
    let mut first = Executor::new(&p);
    let pair = first.run(word, &[], &Context::new(), 1000).unwrap();
    let Value::Pair(left, right) = pair[0] else {
        panic!("expected pair")
    };
    assert_eq!(first.force(left), Ok(Value::Int(7)));
    let mut second = Executor::new(&p);
    second.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(second.force(left), Err(Error::StaleHandle));
    assert_eq!(second.force(right), Err(Error::StaleHandle));
    first.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(first.force(left), Err(Error::StaleHandle));
    assert_eq!(first.force(right), Err(Error::StaleHandle));
}

fn generic_run(p: &Program, word: usize) -> Result<Vec<Value>, Error> {
    let mut executor = Executor::new(p);
    let handles = executor.start(word, &[], &Context::new(), 1000)?;
    handles
        .into_iter()
        .map(|handle| executor.force(handle))
        .collect()
}

#[test]
fn fast_plan_preserves_operand_and_output_error_order() {
    for (outputs, expected) in [
        (vec![6], Error::Type("binary operand types")),
        (vec![7], Error::Overflow),
        (vec![3, 5], Error::Type("binary operand types")),
        (vec![5, 3], Error::Overflow),
    ] {
        let mut p = Program::new();
        let word = p
            .add_word(
                0,
                vec![
                    Op::Const(Literal::Bool(true)),
                    Op::Const(Literal::Int(1)),
                    Op::Const(Literal::Int(i64::MAX)),
                    Op::Binary(Binary::Add, 0, 1),
                    Op::Const(Literal::Int(7)),
                    Op::Binary(Binary::Add, 2, 1),
                    Op::Binary(Binary::Add, 3, 5),
                    Op::Binary(Binary::Add, 5, 3),
                ],
                outputs,
            )
            .unwrap();
        assert!(p.is_fast(word).unwrap());
        assert_eq!(
            Executor::new(&p).run(word, &[], &Context::new(), 1000),
            Err(expected.clone())
        );
        assert_eq!(generic_run(&p, word), Err(expected));
    }
}

#[test]
fn fast_plan_skips_dead_context_and_faults_and_shares_dependencies() {
    let mut p = Program::new();
    let word = p
        .add_word(
            0,
            vec![
                Op::Context("missing".into()),
                Op::Const(Literal::Int(i64::MAX)),
                Op::Const(Literal::Int(1)),
                Op::Binary(Binary::Add, 1, 2),
                Op::Const(Literal::Int(7)),
                Op::Binary(Binary::Mul, 4, 4),
                Op::Binary(Binary::Add, 5, 5),
            ],
            vec![6, 5],
        )
        .unwrap();
    assert!(p.is_fast(word).unwrap());
    let mut executor = Executor::new(&p);
    let expected = vec![Value::Int(98), Value::Int(49)];
    assert_eq!(
        executor.run(word, &[], &Context::new(), 1000),
        Ok(expected.clone())
    );
    assert_eq!(executor.stats().primitive_ops, 2);
    assert_eq!(generic_run(&p, word), Ok(expected));
}

#[test]
fn run_into_clears_old_results_when_evaluation_fails() {
    let mut p = Program::new();
    let fast = p
        .add_word(
            0,
            vec![
                Op::Const(Literal::Int(7)),
                Op::Const(Literal::Bool(true)),
                Op::Binary(Binary::Add, 0, 1),
            ],
            vec![0, 2],
        )
        .unwrap();
    let slow = p
        .add_word(
            0,
            vec![Op::Const(Literal::Int(7)), Op::Context("missing".into())],
            vec![0, 1],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    for word in [fast, slow] {
        let mut output = vec![Value::Int(999)];
        assert!(
            executor
                .run_into(word, &[], &Context::new(), 1000, &mut output)
                .is_err()
        );
        assert!(output.is_empty());
    }
}

#[test]
fn code_and_quotation_cids_are_independent_of_registration_order() {
    fn build(padding: bool) -> (Program, usize, usize, usize) {
        let mut p = Program::new();
        if padding {
            p.add_word(0, vec![Op::Const(Literal::Int(99))], vec![0])
                .unwrap();
        }
        let target = p
            .add_word(0, vec![Op::Const(Literal::Int(7))], vec![0])
            .unwrap();
        let quoted = p
            .add_word(0, vec![Op::Const(Literal::Quote(target))], vec![0])
            .unwrap();
        let caller = p
            .add_word(
                0,
                vec![
                    Op::Call {
                        word: target,
                        arguments: vec![],
                    },
                    Op::Project { call: 0, output: 0 },
                ],
                vec![1],
            )
            .unwrap();
        (p, target, quoted, caller)
    }
    let (a, at, aq, ac) = build(false);
    let (b, bt, bq, bc) = build(true);
    assert_ne!(at, bt);
    for (aw, bw) in [(at, bt), (aq, bq), (ac, bc)] {
        assert_eq!(a.cid(aw), b.cid(bw));
    }
    assert_eq!(
        Executor::new(&a).run(aq, &[], &Context::new(), 1000),
        Executor::new(&b).run(bq, &[], &Context::new(), 1000)
    );
    assert_eq!(
        a.context_cid(&Context::from([("q".into(), Literal::Quote(at))])),
        b.context_cid(&Context::from([("q".into(), Literal::Quote(bt))]))
    );
}

#[test]
fn reused_recursive_body_targets_its_selected_family() {
    let mut p = Program::new();
    let root = source::compile(
        &mut p,
        "
        : zero 0 = ; : always drop true ;
        : base-a drop 7 ; : base-b drop 9 ;
        : step 1 - recur 1 1 ;
        family a 1 1 zero base-a always step ;
        family b 1 1 zero base-b always step ;
        10 a 10 b
    ",
    )
    .unwrap();
    assert_eq!(
        Executor::new(&p).run(root, &[], &Context::new(), 10000),
        Ok(vec![Value::Int(7), Value::Int(9)])
    );
}

#[test]
fn recursive_argument_storage_is_bounded_and_preserves_abort_reason() {
    let mut p = Program::new();
    let word = p
        .add_word(
            4,
            vec![
                Op::Arg(0),
                Op::Const(Literal::Int(1)),
                Op::Binary(Binary::Add, 0, 1),
                Op::Recur {
                    arguments: vec![2; 4],
                },
                Op::Project { call: 3, output: 0 },
            ],
            vec![4],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    executor.argument_limit = 12;
    let handles = executor
        .start(word, &[Literal::Int(1); 4], &Context::new(), 1000)
        .unwrap();
    assert_eq!(executor.force(handles[0]), Err(Error::StorageLimit));
    assert!(executor.stats().peak_arguments <= 12);
    assert!(executor.stats().peak_cells < executor.cell_limit);
    assert_eq!(executor.force(handles[0]), Err(Error::StorageLimit));
}

#[test]
fn cell_budget_exhaustion_preserves_abort_reason() {
    let mut p = Program::new();
    let word = p
        .add_word(
            1,
            vec![
                Op::Arg(0),
                Op::Const(Literal::Int(1)),
                Op::Binary(Binary::Add, 0, 1),
                Op::Recur { arguments: vec![2] },
                Op::Project { call: 3, output: 0 },
            ],
            vec![4],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    executor.cell_limit = 16;
    let handles = executor
        .start(word, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap();
    assert_eq!(executor.force(handles[0]), Err(Error::StorageLimit));
    assert_eq!(executor.force(handles[0]), Err(Error::StorageLimit));
    assert!(executor.stats().peak_cells <= 16);
}

#[test]
fn unchanged_argument_recursion_is_a_cycle() {
    let mut p = Program::new();
    let word = p
        .add_word(
            1,
            vec![
                Op::Arg(0),
                Op::Recur { arguments: vec![0] },
                Op::Project { call: 1, output: 0 },
            ],
            vec![2],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    let handles = executor
        .start(word, &[Literal::Int(7)], &Context::new(), 1000)
        .unwrap();
    assert_eq!(executor.force(handles[0]), Err(Error::Cycle));
    assert_eq!(executor.force(handles[0]), Err(Error::Cycle));
    assert!(executor.stats().steps < 1000);
}

#[test]
fn recursion_can_demand_an_independent_output_without_a_cycle() {
    let mut p = Program::new();
    let word = p
        .add_word(
            0,
            vec![
                Op::Const(Literal::Int(7)),
                Op::Recur { arguments: vec![] },
                Op::Project { call: 1, output: 0 },
            ],
            vec![0, 2],
        )
        .unwrap();
    let mut executor = Executor::new(&p);
    let handles = executor.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(executor.force(handles[1]), Ok(Value::Int(7)));
    assert_eq!(executor.force(handles[0]), Ok(Value::Int(7)));
}

#[test]
fn context_supplied_dynamic_self_calls_detect_cycles_without_recur() {
    let mut p = Program::new();
    let dynamic = p
        .add_word(
            0,
            vec![
                Op::Context("self".into()),
                Op::Apply {
                    function: 0,
                    arguments: vec![],
                    outputs: 1,
                },
                Op::Project { call: 1, output: 0 },
            ],
            vec![2],
        )
        .unwrap();
    let wrapper = p
        .add_word(
            0,
            vec![
                Op::Call {
                    word: dynamic,
                    arguments: vec![],
                },
                Op::Project { call: 0, output: 0 },
            ],
            vec![1],
        )
        .unwrap();
    for word in [dynamic, wrapper] {
        let context = Context::from([("self".into(), Literal::Quote(word))]);
        let mut executor = Executor::new(&p);
        let handles = executor.start(word, &[], &context, 1000).unwrap();
        assert_eq!(executor.force(handles[0]), Err(Error::Cycle));
        assert_eq!(executor.force(handles[0]), Err(Error::Cycle));
        assert!(executor.stats().steps < 1000);
    }
}

#[test]
fn inlining_ignores_unused_arguments_and_outputs_but_shares_one_call() {
    for (text, expected, operations) in [
        (": ignore drop 7 ; ctx missing ignore", 7, 0),
        (": split dup * ctx missing ; 7 split drop", 49, 1),
        (": split dup * dup ctx missing ; 7 split drop +", 98, 2),
    ] {
        let mut p = Program::new();
        let word = source::compile(&mut p, text).unwrap();
        assert!(p.is_fast(word).unwrap(), "{text}");
        let mut executor = Executor::new(&p);
        assert_eq!(
            executor.run(word, &[], &Context::new(), 1000),
            Ok(vec![Value::Int(expected)])
        );
        assert_eq!(executor.stats().primitive_ops, operations);
        assert_eq!(generic_run(&p, word), Ok(vec![Value::Int(expected)]));
    }
}

#[test]
fn identical_calls_inside_one_body_share_canonical_work() {
    let mut p = Program::new();
    let word = source::compile(&mut p, ": square dup * ; 7 square 7 square +").unwrap();
    assert!(p.is_fast(word).unwrap());
    let mut executor = Executor::new(&p);
    assert_eq!(
        executor.run(word, &[], &Context::new(), 1000),
        Ok(vec![Value::Int(98)])
    );
    assert_eq!(executor.stats().primitive_ops, 2);
    let handles = executor.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(executor.force(handles[0]), Ok(Value::Int(98)));
    assert_eq!(executor.stats().primitive_ops, 2);
}

#[test]
fn different_wrappers_do_not_share_equal_runtime_results() {
    let mut p = Program::new();
    let word = source::compile(
        &mut p,
        ": square dup * ; : wrapped square ; 7 square 7 wrapped +",
    )
    .unwrap();
    assert!(p.is_fast(word).unwrap());
    let mut executor = Executor::new(&p);
    assert_eq!(
        executor.run(word, &[], &Context::new(), 1000),
        Ok(vec![Value::Int(98)])
    );
    assert_eq!(
        executor.stats().primitive_ops,
        3,
        "cross-wrapper runtime result canonicalization is not implemented"
    );
    let handles = executor.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(executor.force(handles[0]), Ok(Value::Int(98)));
    assert_eq!(executor.stats().primitive_ops, 3);
}

#[test]
fn deeply_nested_wrappers_inline_then_fall_back_at_the_scope_limit() {
    let mut p = Program::new();
    let mut word = p
        .add_word(0, vec![Op::Const(Literal::Int(7))], vec![0])
        .unwrap();
    for depth in 1..=1050 {
        word = p
            .add_word(
                0,
                vec![
                    Op::Call {
                        word,
                        arguments: vec![],
                    },
                    Op::Project { call: 0, output: 0 },
                ],
                vec![1],
            )
            .unwrap();
        if depth == 32 {
            assert!(p.is_fast(word).unwrap());
            assert_eq!(
                Executor::new(&p).run(word, &[], &Context::new(), 10),
                Ok(vec![Value::Int(7)])
            );
        }
    }
    assert!(
        !p.is_fast(word).unwrap(),
        "bounded inlining must fall back, not grow indefinitely"
    );
    let mut executor = Executor::new(&p);
    assert_eq!(
        executor.run(word, &[], &Context::new(), 100),
        Err(Error::Budget)
    );
    assert_eq!(executor.stats().steps, 100);
    assert_eq!(
        executor.run(word, &[], &Context::new(), 10000),
        Ok(vec![Value::Int(7)])
    );
    assert_eq!(executor.stats().fast_runs, 0);
    assert_eq!(executor.stats().calls, 1051);
}

#[test]
fn inlining_slot_limit_falls_back_without_changing_results() {
    let mut p = Program::new();
    let large = p
        .add_word(
            0,
            (7..16_392).map(|n| Op::Const(Literal::Int(n))).collect(),
            vec![0],
        )
        .unwrap();
    let wrapper = p
        .add_word(
            0,
            vec![
                Op::Call {
                    word: large,
                    arguments: vec![],
                },
                Op::Project { call: 0, output: 0 },
            ],
            vec![1],
        )
        .unwrap();
    assert!(!p.is_fast(large).unwrap());
    assert!(!p.is_fast(wrapper).unwrap());
    assert_eq!(
        Executor::new(&p).run(wrapper, &[], &Context::new(), 100),
        Ok(vec![Value::Int(7)])
    );
}
