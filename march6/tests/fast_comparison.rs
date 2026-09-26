use march_research::fast::{
    Context, Error, Executor, Program, Value, definition::Definition, source, stream,
};

fn check_both_readers(text: &str, expected: &[Value]) {
    let mut host = Program::new();
    let h = source::compile(&mut host, text).unwrap();
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, text).unwrap();
    assert_eq!(host.cid(h), p.cid(w));
    for (program, word) in [(&host, h), (&p, w)] {
        assert_eq!(
            Executor::new(program)
                .run(word, &[], &Context::new(), 10000)
                .unwrap(),
            expected,
            "{text}"
        );
        let mut e = Executor::new(program);
        let roots = e.start(word, &[], &Context::new(), 10000).unwrap();
        let observed: Vec<_> = roots.into_iter().map(|h| e.force(h).unwrap()).collect();
        assert_eq!(observed, expected, "{text}");
    }
}

#[test]
fn all_five_postfix_predicates_cover_less_equal_greater() {
    for (a, b, order) in [
        ("-1", "0", -1),
        ("7", "7", 0),
        ("9223372036854775807", "-9223372036854775808", 1),
        (r#""apple""#, r#""pear""#, -1),
        (r#""é""#, r#""é""#, 0),
        (r#""é""#, r#""z""#, 1),
    ] {
        for (word, expected) in [
            ("eq?", order == 0),
            ("lt?", order < 0),
            ("gt?", order > 0),
            ("lte?", order <= 0),
            ("gte?", order >= 0),
        ] {
            check_both_readers(&format!("{a} {b} {word}"), &[Value::Bool(expected)]);
        }
    }
}

#[test]
fn renamed_ordering_stays_type_strict_and_preserves_demand_errors() {
    check_both_readers("1 true eq?", &[Value::Bool(false)]);
    check_both_readers("1 2 pair 1 2 tuple 2 eq?", &[Value::Bool(true)]);
    for word in ["lt?", "gt?", "lte?", "gte?"] {
        for operands in ["1 true", "true 1", "true false", "unit unit", "1 \"1\""] {
            let mut p = stream::seed().unwrap();
            let w = stream::compile(&mut p, &format!("{operands} {word}")).unwrap();
            assert!(matches!(
                Executor::new(&p).run(w, &[], &Context::new(), 10000),
                Err(Error::Type(_))
            ));
        }
    }
    for word in ["eq?", "lt?", "gt?", "lte?", "gte?"] {
        let mut p = stream::seed().unwrap();
        let w = stream::compile(&mut p, &format!("ctx absent 0 {word}")).unwrap();
        assert_eq!(
            Executor::new(&p).run(w, &[], &Context::new(), 10000),
            Err(Error::MissingContext("absent".into()))
        );
    }
}

#[test]
fn surface_names_do_not_change_primitive_identity_and_other_predicates_are_composed() {
    let mut p = stream::seed().unwrap();
    for (name, semantic) in [("eq?", "="), ("lt?", "<")] {
        let original = p
            .add_definition(Definition::Primitive(semantic.into()))
            .unwrap();
        assert_eq!(p.cid(p.lookup(name).unwrap()), p.cid(original));
    }
    for (name, body) in [
        ("gt?", "swap lt?"),
        ("gte?", "lt? false true select"),
        ("lte?", "gt? false true select"),
    ] {
        let word = p.lookup(name).unwrap();
        assert!(matches!(
            p.definition(word).unwrap(),
            Some(Definition::Sequence(_))
        ));
        stream::compile(&mut p, &format!(": rebuilt {body} ;")).unwrap();
        assert_eq!(p.cid(word), p.cid(p.lookup("rebuilt").unwrap()));
    }
}

#[test]
fn symbolic_names_are_not_seed_aliases_but_remain_user_definable() {
    let mut p = stream::seed().unwrap();
    for name in ["=", "<", ">", "<=", ">="] {
        assert!(p.lookup(name).is_none());
        assert!(stream::compile(&mut p, &format!("1 2 {name}")).is_err());
    }
    let w = stream::compile(&mut p, ": = eq? ; : < lt? ; 1 1 = 1 2 <").unwrap();
    assert_eq!(
        Executor::new(&p)
            .run(w, &[], &Context::new(), 10000)
            .unwrap(),
        [Value::Bool(true), Value::Bool(true)]
    );
}

#[test]
fn comparison_dictionary_and_compositions_survive_images_and_rebinding() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, "1 1 eq? 1 2 lt? 2 1 gt? 2 2 gte? 1 2 lte?").unwrap();
    let bytes = p.to_image(w).unwrap();
    let (mut q, loaded) = Program::from_image(&bytes).unwrap();
    assert_eq!(bytes, q.to_image(loaded).unwrap());
    assert_eq!(
        Executor::new(&q)
            .run(loaded, &[], &Context::new(), 10000)
            .unwrap(),
        vec![Value::Bool(true); 5]
    );
    // Existing compositions keep their original dependency CIDs.
    let w = stream::compile(&mut q, ": lt? drop drop false ; 1 2 lt? 2 1 gt?").unwrap();
    assert_eq!(
        Executor::new(&q)
            .run(w, &[], &Context::new(), 10000)
            .unwrap(),
        [Value::Bool(false), Value::Bool(true)]
    );
}
