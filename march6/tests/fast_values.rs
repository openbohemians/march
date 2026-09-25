use march_research::fast::{Context, Error, Executor, Literal, Program, Value, source};

#[test]
fn recursive_lazy_pair_produces_only_requested_prefix() {
    let mut p = Program::new();
    let word = source::compile(
        &mut p,
        ": from dup 1 + recur 1 1 pair ; 10 from second first",
    )
    .unwrap();
    let mut e = Executor::new(&p);
    assert_eq!(
        e.run(word, &[], &Context::new(), 1000).unwrap(),
        vec![Value::Int(11)]
    );
    assert_eq!(e.stats().primitive_ops, 1);
    assert!(e.stats().peak_frames < 10);
}

#[test]
fn value_identity_is_independent_of_code_and_invocation_identity() {
    let mut p = Program::new();
    let a = source::compile(&mut p, "2 3 + true pair").unwrap();
    let b = source::compile(&mut p, "5 true pair").unwrap();
    assert_ne!(p.cid(a).unwrap(), p.cid(b).unwrap());
    let mut e = Executor::new(&p);
    let h = e.start(a, &[], &Context::new(), 1000).unwrap()[0];
    let id = e.content_id(h).unwrap();
    let n = e.stats().primitive_ops;
    assert_eq!(e.content_id(h).unwrap(), id);
    assert_eq!(e.stats().primitive_ops, n);
    let h = e.start(b, &[], &Context::new(), 1000).unwrap()[0];
    assert_eq!(e.content_id(h).unwrap(), id);
    assert_ne!(Value::Int(1).scalar_cid(), Value::Bool(true).scalar_cid());
}

#[test]
fn ordinary_observation_does_not_force_fields_but_content_identity_does() {
    let mut p = Program::new();
    let word = source::compile(&mut p, "7 ctx missing pair").unwrap();
    let mut e = Executor::new(&p);
    let h = e.start(word, &[], &Context::new(), 1000).unwrap()[0];
    let Value::Pair(a, _) = e.force(h).unwrap() else {
        panic!("pair")
    };
    assert_eq!(e.force(a).unwrap(), Value::Int(7));
    assert_eq!(
        e.content_id(h),
        Err(Error::MissingContext("missing".into()))
    );
}

#[test]
fn infinite_value_canonicalization_is_bounded_and_never_a_cached_language_error() {
    let mut p = Program::new();
    let word = source::compile(&mut p, ": from dup 1 + recur 1 1 pair ; from").unwrap();
    let mut e = Executor::new(&p);
    let h = e
        .start(word, &[Literal::Int(0)], &Context::new(), 100)
        .unwrap()[0];
    assert_eq!(e.content_id(h), Err(Error::Budget));
    let h = e
        .start(word, &[Literal::Int(0)], &Context::new(), 100)
        .unwrap()[0];
    let Value::Pair(first, _) = e.force(h).unwrap() else {
        panic!("pair")
    };
    assert_eq!(e.force(first).unwrap(), Value::Int(0));
}
