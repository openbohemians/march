//! Small differential checks for the conventional spike, not full equivalence.
use march_research::fast::{Binary, Context, Executor, Literal, Op, Program, Value};
use march_research::{Atom, Bindings, Node, Reducer, Store};

#[test]
fn conditional_arithmetic_agrees_with_reference_and_checked_rust() {
    let mut program = Program::new();
    let word = program
        .add_word(
            2,
            vec![
                Op::Arg(0),
                Op::Arg(1),
                Op::Const(Literal::Int(3)),
                Op::Binary(Binary::Mul, 0, 2),
                Op::Const(Literal::Int(1)),
                Op::Const(Literal::Int(2)),
                Op::Binary(Binary::Add, 3, 4),
                Op::Binary(Binary::Add, 3, 5),
                Op::Select {
                    condition: 1,
                    when_true: 6,
                    when_false: 7,
                },
            ],
            vec![8],
        )
        .unwrap();
    let context = Context::new();
    let mut executor = Executor::new(&program);
    for x in -50i64..50 {
        for condition in [false, true] {
            let result = executor
                .run(
                    word,
                    &[Literal::Int(x), Literal::Bool(condition)],
                    &context,
                    1000,
                )
                .unwrap();
            let expected = x
                .checked_mul(3)
                .unwrap()
                .checked_add(if condition { 1 } else { 2 })
                .unwrap();
            assert_eq!(result, vec![Value::Int(expected)]);
            assert_eq!(
                executor.stats().primitive_ops,
                2,
                "only selected addition executes"
            );

            let mut store = Store::new();
            let input = store.intern(Node::Const(Atom::Int(x)));
            let three = store.intern(Node::Const(Atom::Int(3)));
            let one = store.intern(Node::Const(Atom::Int(1)));
            let two = store.intern(Node::Const(Atom::Int(2)));
            let shared = store.intern(Node::Mul(input, three));
            let when_true = store.intern(Node::Add(shared, one));
            let when_false = store.intern(Node::Add(shared, two));
            let condition = store.intern(Node::Const(Atom::Bool(condition)));
            let root = store.intern(Node::If {
                condition,
                when_true,
                when_false,
            });
            let reduced = Reducer::with_budget(&mut store, &Bindings::new(), 1000)
                .run(root)
                .unwrap();
            assert_eq!(
                store.get(reduced.root),
                Some(&Node::Const(Atom::Int(expected)))
            );
        }
    }
}

#[test]
fn unused_overflow_and_shared_call_are_lazy() {
    let mut program = Program::new();
    let square = program
        .add_word(1, vec![Op::Arg(0), Op::Binary(Binary::Mul, 0, 0)], vec![1])
        .unwrap();
    let word = program
        .add_word(
            1,
            vec![
                Op::Arg(0),
                Op::Call {
                    word: square,
                    arguments: vec![0],
                },
                Op::Project { call: 1, output: 0 },
                Op::Binary(Binary::Add, 2, 2),
                Op::Const(Literal::Int(i64::MAX)),
                Op::Const(Literal::Int(1)),
                Op::Binary(Binary::Add, 4, 5),
            ],
            vec![3],
        )
        .unwrap();
    let mut executor = Executor::new(&program);
    assert_eq!(
        executor
            .run(word, &[Literal::Int(7)], &Context::new(), 1000)
            .unwrap(),
        vec![Value::Int(98)]
    );
    assert_eq!(
        executor.stats().primitive_ops,
        2,
        "one multiplication, one addition, no overflow evaluation"
    );
}

#[test]
fn factored_discard_does_not_force_faulting_argument() {
    // This must remain true if the static scalar plan starts crossing calls.
    let mut program = Program::new();
    let word = march_research::fast::source::compile(
        &mut program,
        ": ignore drop 7 ; 9223372036854775807 1 + ignore",
    )
    .unwrap();
    let mut executor = Executor::new(&program);
    assert_eq!(
        executor.run(word, &[], &Context::new(), 1000).unwrap(),
        vec![Value::Int(7)]
    );
    assert_eq!(executor.stats().primitive_ops, 0);
    let roots = executor.start(word, &[], &Context::new(), 1000).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(executor.force(roots[0]).unwrap(), Value::Int(7));
    assert_eq!(executor.stats().primitive_ops, 0);
}

#[test]
fn source_calls_match_explicit_demand_for_values_and_overflow() {
    let mut program = Program::new();
    let word = march_research::fast::source::compile(
        &mut program,
        ": square dup * ; : quad square square ; quad",
    )
    .unwrap();
    let mut executor = Executor::new(&program);
    for x in [-1_000, -63, -1, 0, 1, 7, 63, 1_000, i64::MAX] {
        let fast = executor.run(word, &[Literal::Int(x)], &Context::new(), 1000);
        let roots = executor
            .start(word, &[Literal::Int(x)], &Context::new(), 1000)
            .unwrap();
        let demanded = executor.force(roots[0]).map(|value| vec![value]);
        assert_eq!(fast, demanded, "source factoring for x={x}");
        if let Some(expected) = x.checked_mul(x).and_then(|s| s.checked_mul(s)) {
            assert_eq!(fast.unwrap(), vec![Value::Int(expected)]);
        } else {
            assert!(fast.is_err());
        }
    }
}
