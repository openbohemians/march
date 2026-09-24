//! C0: pin the reference contract before altering aggregate or net semantics.
use march_research::{Atom, Bindings, Cid, Clause, Image, Node, ReduceError, Reducer, Store};

const WORK: usize = 1_000_000;
fn int(s: &mut Store, n: i64) -> Cid {
    s.intern(Node::Const(Atom::Int(n)))
}
fn run(s: &mut Store, root: Cid) -> Result<Cid, ReduceError> {
    Reducer::with_budget(s, &Bindings::new(), WORK)
        .run(root)
        .map(|r| r.root)
}
fn overflow(s: &mut Store) -> Cid {
    let max = int(s, i64::MAX);
    let one = int(s, 1);
    s.intern(Node::Add(max, one))
}

#[test]
fn arbitrary_guard_parameter_can_leave_an_earlier_computed_argument_unused() {
    let mut s = Store::new();
    let bad = overflow(&mut s);
    let zero = int(&mut s, 0);
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let p0 = s.intern(Node::Param(0));
    let p1 = s.intern(Node::Param(1));
    let family = s.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: p1,
                body: zero,
            },
            Clause {
                guard: yes,
                body: p0,
            },
        ],
    });
    let ignored = s.intern(Node::Dispatch {
        family,
        arguments: vec![bad, yes],
    });
    assert_eq!(run(&mut s, ignored), Ok(zero));
    let no = s.intern(Node::Const(Atom::Bool(false)));
    let demanded = s.intern(Node::Dispatch {
        family,
        arguments: vec![bad, no],
    });
    assert_eq!(
        run(&mut s, demanded),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn unknown_guard_blocks_later_clauses_but_not_an_independent_static_island() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let zero = int(&mut s, 0);
    let bad = overflow(&mut s);
    let family = s.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause {
                guard: p,
                body: zero,
            },
            Clause {
                guard: yes,
                body: bad,
            },
        ],
    });
    let flag = s.intern(Node::Hole("flag".into()));
    let call = s.intern(Node::Dispatch {
        family,
        arguments: vec![flag],
    });
    let six = int(&mut s, 6);
    let seven = int(&mut s, 7);
    let island = s.intern(Node::Mul(six, seven));
    let root = s.intern(Node::Add(call, island));
    let reduced = Reducer::with_budget(&mut s, &Bindings::new(), WORK)
        .run(root)
        .unwrap();
    assert_eq!(reduced.stats.clauses_instantiated, 0);
    let forty_two = int(&mut s, 42);
    assert_eq!(s.get(reduced.root), Some(&Node::Add(call, forty_two)));
    let image = Image::from_store(&s, &[reduced.root]).unwrap();
    let (_, mut s) = Image::parse(image.as_bytes()).unwrap();
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let mut facts = Bindings::new();
    facts.insert("flag", yes);
    let result = Reducer::with_budget(&mut s, &facts, WORK)
        .run(reduced.root)
        .unwrap();
    assert_eq!(result.root, forty_two);
}

#[test]
fn arithmetic_child_errors_precede_parent_type_checks() {
    let mut s = Store::new();
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let max = int(&mut s, i64::MAX);
    let two = int(&mut s, 2);
    let multiply_error = s.intern(Node::Mul(max, two));
    let root = s.intern(Node::Add(yes, multiply_error));
    assert_eq!(
        run(&mut s, root),
        Err(ReduceError::IntegerOverflow("multiply"))
    );
    let left_error = overflow(&mut s);
    let both = s.intern(Node::Add(left_error, multiply_error));
    assert_eq!(run(&mut s, both), Err(ReduceError::IntegerOverflow("add")));
}

#[test]
fn an_error_observed_with_unknown_inputs_is_not_a_permanent_memo_value() {
    let mut s = Store::new();
    let x = s.intern(Node::Hole("x".into()));
    let two = int(&mut s, 2);
    let left = s.intern(Node::Mul(x, two));
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let root = s.intern(Node::Add(left, yes));
    assert_eq!(
        run(&mut s, root),
        Err(ReduceError::Type("add expects two integers"))
    );
    let max = int(&mut s, i64::MAX);
    let mut facts = Bindings::new();
    facts.insert("x", max);
    assert_eq!(
        Reducer::with_budget(&mut s, &facts, WORK).run(root),
        Err(ReduceError::IntegerOverflow("multiply"))
    );
}

#[test]
fn strict_pair_and_record_projection_are_not_lazy_output_bundles() {
    let mut s = Store::new();
    let bad = overflow(&mut s);
    let seven = int(&mut s, 7);
    let pair = s.intern(Node::Pair(seven, bad));
    let first = s.intern(Node::First(pair));
    assert_eq!(run(&mut s, first), Err(ReduceError::IntegerOverflow("add")));
    let record = s.intern(Node::Record(vec![
        ("good".into(), seven),
        ("unused".into(), bad),
    ]));
    let get = s.intern(Node::Get {
        record,
        field: "good".into(),
    });
    assert_eq!(run(&mut s, get), Err(ReduceError::IntegerOverflow("add")));
}

#[test]
fn strict_containers_can_hold_closed_suspensions_without_forcing_them() {
    let mut s = Store::new();
    let bad = overflow(&mut s);
    let seven = int(&mut s, 7);
    let good_code = s.intern(Node::Quote {
        params: 0,
        body: seven,
    });
    let bad_code = s.intern(Node::Quote {
        params: 0,
        body: bad,
    });
    let pair = s.intern(Node::Pair(good_code, bad_code));
    assert_eq!(run(&mut s, pair), Ok(pair));
    let image = Image::from_store(&s, &[pair]).unwrap();
    s.collect(&[pair]).unwrap();
    let (_, mut s) = Image::parse(image.as_bytes()).unwrap();
    let first = s.intern(Node::First(pair));
    let force_good = s.intern(Node::Apply {
        function: first,
        arguments: vec![],
    });
    assert_eq!(run(&mut s, force_good), Ok(seven));
    let second = s.intern(Node::Second(pair));
    let force_bad = s.intern(Node::Apply {
        function: second,
        arguments: vec![],
    });
    assert_eq!(
        run(&mut s, force_bad),
        Err(ReduceError::IntegerOverflow("add"))
    );
    // This is a witness for closed suspensions, not a general implementation
    // of argument-capturing, multi-output words or lazy aggregate semantics.
}

#[test]
fn shared_code_with_internal_duplication_gets_fresh_explicit_arguments() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let square_body = s.intern(Node::Mul(p, p));
    let square = s.intern(Node::Quote {
        params: 1,
        body: square_body,
    });
    let three = int(&mut s, 3);
    let inner = s.intern(Node::Apply {
        function: square,
        arguments: vec![three],
    });
    let outer = s.intern(Node::Apply {
        function: square,
        arguments: vec![inner],
    });
    assert_eq!(run(&mut s, outer), Ok(int(&mut s, 81)));
    // Higher-order closed code: apply the supplied unary word twice. The
    // code argument is explicit, and no nested lexical closure is created.
    let f = s.intern(Node::Param(0));
    let x = s.intern(Node::Param(1));
    let once = s.intern(Node::Apply {
        function: f,
        arguments: vec![x],
    });
    let twice = s.intern(Node::Apply {
        function: f,
        arguments: vec![once],
    });
    let apply_twice = s.intern(Node::Quote {
        params: 2,
        body: twice,
    });
    let root = s.intern(Node::Apply {
        function: apply_twice,
        arguments: vec![square, three],
    });
    assert_eq!(run(&mut s, root), Ok(int(&mut s, 81)));
}

#[test]
fn church_two_two_and_its_partially_applied_code_are_closed_and_imageable() {
    fn description(s: &mut Store, op: &str, mut fields: Vec<(&str, Cid)>) -> Cid {
        let op = s.intern(Node::Const(Atom::Text(op.into())));
        fields.push(("op", op));
        s.intern(Node::Record(
            fields
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        ))
    }
    fn application(s: &mut Store, function: Cid, argument: Cid, nil: Cid) -> Cid {
        let args = s.intern(Node::Pair(argument, nil));
        description(
            s,
            "apply",
            vec![("function", function), ("arguments", args)],
        )
    }

    let mut s = Store::new();
    let zero = int(&mut s, 0);
    let one = int(&mut s, 1);
    let nil = s.intern(Node::Const(Atom::Unit));
    let f = s.intern(Node::Param(0));
    // two(f) explicitly constructs closed code x -> f(f(x)). `f` is embedded
    // as a held closed code value by Intern, not captured from a lexical scope.
    // The inner x is a reflection description, not an outer Param occurrence.
    let held_f = description(&mut s, "embed", vec![("value", f)]);
    let x = description(&mut s, "param", vec![("index", zero)]);
    let fx = application(&mut s, held_f, x, nil);
    let ffx = application(&mut s, held_f, fx, nil);
    let quote = description(&mut s, "quote", vec![("parameters", one), ("body", ffx)]);
    let construct = s.intern(Node::Intern(quote));
    let two = s.intern(Node::Quote {
        params: 1,
        body: construct,
    });
    let two_two_call = s.intern(Node::Apply {
        function: two,
        arguments: vec![two],
    });
    let two_two = run(&mut s, two_two_call).unwrap();
    assert!(matches!(
        s.get(two_two),
        Some(Node::Quote { params: 1, .. })
    ));

    let n = s.intern(Node::Param(0));
    let increment = s.intern(Node::Add(n, one));
    let increment = s.intern(Node::Quote {
        params: 1,
        body: increment,
    });
    let four_increments = s.intern(Node::Apply {
        function: two_two,
        arguments: vec![increment],
    });
    let four_increments = run(&mut s, four_increments).unwrap();
    assert!(matches!(
        s.get(four_increments),
        Some(Node::Quote { params: 1, .. })
    ));

    // No source, constructor, or environment root needs to survive: the
    // returned code carries all explicit code dependencies as graph edges.
    s.collect(&[four_increments]).unwrap();
    let image = Image::from_store(&s, &[four_increments]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let zero = int(&mut loaded, 0);
    let final_call = loaded.intern(Node::Apply {
        function: four_increments,
        arguments: vec![zero],
    });
    let result = run(&mut loaded, final_call).unwrap();
    assert_eq!(loaded.get(result), Some(&Node::Const(Atom::Int(4))));
}

#[test]
fn independent_output_templates_can_share_explicit_inputs_without_strict_packing() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let seven = int(&mut s, 7);
    let identity_output = s.intern(Node::Quote { params: 1, body: p });
    let constant_output = s.intern(Node::Quote {
        params: 1,
        body: seven,
    });
    // R1 candidate: a word owns output templates with a common explicit input
    // interface. Select code before applying; don't pack already-applied
    // expressions into today's strict Pair. This is not new seed syntax.
    let outputs = s.intern(Node::Pair(identity_output, constant_output));
    let input = overflow(&mut s);
    let selected = s.intern(Node::Second(outputs));
    let application = s.intern(Node::Apply {
        function: selected,
        arguments: vec![input],
    });
    assert_eq!(run(&mut s, application), Ok(seven));
    let selected = s.intern(Node::First(outputs));
    let application = s.intern(Node::Apply {
        function: selected,
        arguments: vec![input],
    });
    assert_eq!(
        run(&mut s, application),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn separate_output_templates_reuse_the_same_instantiated_recursive_work() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let zero = int(&mut s, 0);
    let one = int(&mut s, 1);
    let minus_one = int(&mut s, -1);
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let guard = s.intern(Node::Eq(p, zero));
    let next = s.intern(Node::Add(p, minus_one));
    let recurse = s.intern(Node::Recur(vec![next]));
    let sum = s.intern(Node::Add(p, recurse));
    let family = s.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause { guard, body: zero },
            Clause {
                guard: yes,
                body: sum,
            },
        ],
    });
    let shared = s.intern(Node::Dispatch {
        family,
        arguments: vec![p],
    });
    let output_a = s.intern(Node::Quote {
        params: 1,
        body: shared,
    });
    let increment = s.intern(Node::Add(shared, one));
    let output_b = s.intern(Node::Quote {
        params: 1,
        body: increment,
    });
    let three = int(&mut s, 3);
    let a = s.intern(Node::Apply {
        function: output_a,
        arguments: vec![three],
    });
    let b = s.intern(Node::Apply {
        function: output_b,
        arguments: vec![three],
    });
    let observe_both = s.intern(Node::Pair(a, b));
    let reduced = Reducer::with_budget(&mut s, &Bindings::new(), WORK)
        .run(observe_both)
        .unwrap();
    let six = int(&mut s, 6);
    let seven = int(&mut s, 7);
    assert_eq!(s.get(reduced.root), Some(&Node::Pair(six, seven)));
    // sum(3), sum(2), sum(1), sum(0), once each despite two distinct outputs.
    assert_eq!(reduced.stats.clauses_instantiated, 4);
}

#[test]
fn dynamic_first_consumer_example_has_the_correct_two_results() {
    for (condition, expected) in [(true, 85), (false, 126)] {
        let mut s = Store::new();
        let six = int(&mut s, 6);
        let seven = int(&mut s, 7);
        let shared = s.intern(Node::Mul(six, seven));
        let one = int(&mut s, 1);
        let two = int(&mut s, 2);
        let a = s.intern(Node::Add(shared, one));
        let b = s.intern(Node::Mul(shared, two));
        let condition = s.intern(Node::Const(Atom::Bool(condition)));
        let branch = s.intern(Node::If {
            condition,
            when_true: a,
            when_false: b,
        });
        let root = s.intern(Node::Add(branch, shared));
        assert_eq!(run(&mut s, root), Ok(int(&mut s, expected)));
    }
}
