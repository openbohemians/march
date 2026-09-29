use march_research::fast::{
    Context, Error, Executor, InputNode, Literal, Program, Value,
    store::{FrozenNode, FrozenValue, Store},
    stream,
};

fn compile(source: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let word = stream::compile(&mut p, source).unwrap();
    (p, word)
}

fn frozen(source: &str) -> FrozenValue {
    let (p, word) = compile(source);
    let mut e = Executor::new(&p);
    let out = e.start(word, &[], &Context::new(), 100_000).unwrap();
    e.freeze(out[0]).unwrap()
}

#[test]
fn store_format_v1_has_pinned_empty_and_nested_identity_vectors() {
    assert_eq!(
        Store::new().cid().to_string(),
        "c5e112f4ae3731ca0bff629efa1faa3899f6b38b7c2fe6f45ebc83e1f53974ee"
    );
    let store = Store::new()
        .with(&["demo", "result"], frozen("6 7 * ' dup tuple 2"))
        .unwrap();
    assert_eq!(
        store.cid().to_string(),
        "834c0e58fdcf53ddf4d3cc072b7f270efbb88e82c3010f526d055ae3540efa1d"
    );
}

#[test]
fn frozen_identity_matches_existing_value_identity_for_all_value_kinds() {
    for source in [
        "42",
        "true",
        "false",
        "unit",
        r#""é🙂""#,
        "' dup",
        "1 tuple 1",
        "1 2 pair",
        "1 2 tuple 2",
        "1 true unit tuple 3",
        r#""hi" 7 tuple 2 false tuple 2"#,
    ] {
        let (p, word) = compile(source);
        let mut e = Executor::new(&p);
        let out = e.start(word, &[], &Context::new(), 100_000).unwrap();
        let cid = e.content_id(out[0]).unwrap();
        assert_eq!(e.freeze(out[0]).unwrap().cid(), cid, "{source}");
    }
}

#[test]
fn writing_recursively_evaluates_data_and_detaches_it_from_executor_lifetime() {
    let store = Store::new();
    let next = {
        let (p, word) = compile(r#"6 7 * "a" "b" text-concat true tuple 2 tuple 2"#);
        let mut e = Executor::new(&p);
        let out = e.start(word, &[], &Context::new(), 1000).unwrap();
        e.store_put(&store, &["app", "result"], out[0]).unwrap()
    };
    assert!(store.is_empty());
    let data = next.get(&["app", "result"]).unwrap().unwrap();
    assert!(data.nodes().contains(&FrozenNode::Int(42)));
    assert!(data.nodes().contains(&FrozenNode::Text("ab".into())));
    let (p, word) = compile("dup");
    let mut e = Executor::new(&p);
    let input = data.to_input_graph(&p).unwrap();
    let out = e
        .start_graph(word, &input, &[data.root()], &Context::new(), 1000)
        .unwrap();
    assert_eq!(e.content_id(out[0]).unwrap(), data.cid());
}

#[test]
fn quotations_are_an_explicit_deferred_boundary_not_automatically_called() {
    let (p, word) = compile(": forever recur 0 1 ; ' forever 7 tuple 2");
    let mut e = Executor::new(&p);
    let out = e.start(word, &[], &Context::new(), 1000).unwrap();
    let next = e.store_put(&Store::new(), &["pending"], out[0]).unwrap();
    let data = next.get(&["pending"]).unwrap().unwrap();
    assert!(data.nodes().contains(&FrozenNode::Quote(
        p.cid(p.lookup("forever").unwrap()).unwrap()
    )));
    let before = e.stats().steps;
    let id = next.cid();
    assert_eq!(next.cid(), id);
    assert_eq!(
        e.stats().steps,
        before,
        "hashing stored state must not evaluate"
    );
}

#[test]
fn failed_or_divergent_fields_never_publish_a_store_update() {
    let original = Store::new().with(&["app", "answer"], frozen("42")).unwrap();
    let before = original.cid();
    for (source, expected) in [
        ("7 9223372036854775807 1 + pair", Error::Overflow),
        (": forever 7 recur 0 1 pair ; forever", Error::Budget),
    ] {
        let (p, word) = compile(source);
        let mut e = Executor::new(&p);
        let out = e.start(word, &[], &Context::new(), 1000).unwrap();
        assert_eq!(
            e.store_put(&original, &["app", "answer"], out[0]),
            Err(expected)
        );
        assert_eq!(original.cid(), before);
        assert_eq!(
            original.get(&["app", "answer"]).unwrap().unwrap().nodes(),
            &[FrozenNode::Int(42)]
        );
    }
}

#[test]
fn paths_are_exact_components_and_namespace_conflicts_are_nondestructive() {
    let value = frozen("1");
    let a = Store::new().with(&["a.b"], value.clone()).unwrap();
    let b = a.with(&["a", "b"], value.clone()).unwrap();
    assert_eq!(b.len(), 2);
    assert_eq!(b.get(&["a.b"]).unwrap(), b.get(&["a", "b"]).unwrap());
    assert_ne!(a.cid(), b.cid());
    assert!(b.with(&["a"], value.clone()).is_err());
    assert!(b.with(&["a.b", "c"], value.clone()).is_err());
    assert!(b.without(&["a"]).is_err());
    assert!(b.get(&["a.b", "c"]).is_err());
    assert!(b.get(&["missing", "c"]).unwrap().is_none());
    assert_eq!(b.namespace(&[]).unwrap(), Some(&b));
    assert_eq!(
        b.namespace(&["a"])
            .unwrap()
            .unwrap()
            .names()
            .collect::<Vec<_>>(),
        ["b"]
    );
    for path in [vec![], vec![""], vec!["x"; 65]] {
        assert!(b.with(&path, value.clone()).is_err());
    }
    let huge = "x".repeat(4097);
    assert!(b.with(&[&huge], value.clone()).is_err());
    let n = "x".repeat(4096);
    assert!(b.with(&vec![n.as_str(); 17], value).is_err());
}

#[test]
fn invalid_write_path_does_not_demand_the_value() {
    let old = Store::new().with(&["a", "b"], frozen("1")).unwrap();
    let (p, word) = compile(": spin recur 0 1 ; spin");
    let mut e = Executor::new(&p);
    let out = e.start(word, &[], &Context::new(), 1000).unwrap();
    let steps = e.stats().steps;
    assert!(matches!(
        e.store_put(&old, &["a"], out[0]),
        Err(Error::Store(_))
    ));
    assert_eq!(e.stats().steps, steps);
}

#[test]
fn namespace_identities_ignore_update_history_and_prune_empty_parents() {
    let one = frozen("1");
    let two = frozen("2");
    let empty = Store::new();
    let a = empty
        .with(&["ns", "one"], one.clone())
        .unwrap()
        .with(&["ns", "two"], two.clone())
        .unwrap();
    let b = empty
        .with(&["noise", "x"], one.clone())
        .unwrap()
        .with(&["ns", "two"], two)
        .unwrap()
        .with(&["ns", "one"], one)
        .unwrap()
        .without(&["noise", "x"])
        .unwrap();
    assert_eq!(a.cid(), b.cid());
    assert_eq!(a, b);
    assert_eq!(a.without(&["missing"]).unwrap().cid(), a.cid());
    let old_id = a.cid();
    let edited = a.with(&["ns", "one"], frozen("3")).unwrap();
    assert_ne!(edited.cid(), old_id);
    assert_eq!(a.cid(), old_id);
    assert_eq!(
        a.without(&["ns", "one"])
            .unwrap()
            .without(&["ns", "two"])
            .unwrap(),
        empty
    );
}

#[test]
fn identities_distinguish_types_structure_exact_text_and_quotes() {
    let mut ids = std::collections::HashSet::new();
    for s in [
        "1",
        "true",
        r#""1""#,
        "unit",
        "1 tuple 1",
        "1 2 pair",
        "2 1 pair",
        "' dup",
        "' swap",
        r#""é""#,
        r#""é""#,
    ] {
        assert!(
            ids.insert(Store::new().with(&["x"], frozen(s)).unwrap().cid()),
            "{s}"
        );
    }
}

#[test]
fn sharing_is_preserved_without_becoming_part_of_value_identity() {
    // Import distinct graphs so compiler common-subexpression sharing cannot
    // turn the deliberately duplicated representation into a shared one.
    let (p, word) = compile("dup");
    let export = |nodes: &[InputNode]| {
        let mut e = Executor::new(&p);
        let h = e
            .start_graph(word, nodes, &[nodes.len() - 1], &Context::new(), 1000)
            .unwrap()[0];
        e.freeze(h).unwrap()
    };
    let shared = export(&[
        InputNode::Scalar(Literal::Int(7)),
        InputNode::Tuple(vec![0]),
        InputNode::Pair(1, 1),
    ]);
    let copied = export(&[
        InputNode::Scalar(Literal::Int(7)),
        InputNode::Tuple(vec![0]),
        InputNode::Scalar(Literal::Int(7)),
        InputNode::Tuple(vec![2]),
        InputNode::Pair(1, 3),
    ]);
    assert_eq!(shared.cid(), copied.cid());
    assert!(shared.nodes().len() < copied.nodes().len());
    assert_eq!(
        Store::new().with(&["x"], shared).unwrap().cid(),
        Store::new().with(&["x"], copied).unwrap().cid()
    );
}

#[test]
fn deeply_nested_frozen_data_uses_iterative_traversal_and_flat_storage() {
    let (p, word) = compile("dup");
    let mut input = vec![InputNode::Scalar(Literal::Int(7))];
    for i in 0..20_000 {
        input.push(InputNode::Tuple(vec![i]));
    }
    let mut e = Executor::new(&p);
    let out = e
        .start_graph(word, &input, &[input.len() - 1], &Context::new(), 200_000)
        .unwrap();
    let value = e.freeze(out[0]).unwrap();
    assert_eq!(value.nodes().len(), input.len());
    assert_eq!(value.cid(), e.content_id(out[0]).unwrap());
    drop(value); // no recursively owned tuple tree to overflow the Rust stack
}

#[test]
fn frozen_quotes_resolve_by_cid_across_programs_not_local_word_numbers() {
    let (a, word) = compile(": answer 42 ; ' answer");
    let mut e = Executor::new(&a);
    let h = e.start(word, &[], &Context::new(), 1000).unwrap()[0];
    let value = e.freeze(h).unwrap();
    let (b, invoke) = compile(": unrelated 99 ; : answer 42 ; apply 0 1");
    assert_ne!(a.lookup("answer"), b.lookup("answer"));
    let input = value.to_input_graph(&b).unwrap();
    let mut other = Executor::new(&b);
    let out = other
        .start_graph(invoke, &input, &[value.root()], &Context::new(), 1000)
        .unwrap();
    assert_eq!(other.force(out[0]).unwrap(), Value::Int(42));
    let (missing, _) = compile(": answer 99 ; 0");
    assert!(matches!(
        value.to_input_graph(&missing),
        Err(Error::Store(_))
    ));
}

#[test]
fn stale_handles_budget_and_export_storage_limits_fail_without_publishing() {
    let (p, word) = compile("1 2 pair");
    let mut e = Executor::new(&p);
    let old = e.start(word, &[], &Context::new(), 1000).unwrap()[0];
    let fresh = e.start(word, &[], &Context::new(), 1000).unwrap()[0];
    assert!(matches!(e.freeze(old), Err(Error::StaleHandle)));
    e.tuple_field_limit = 1;
    assert_eq!(
        e.store_put(&Store::new(), &["x"], fresh),
        Err(Error::StorageLimit)
    );
    e.tuple_field_limit = 100;
    let h = e.start(word, &[], &Context::new(), 0).unwrap()[0];
    assert!(matches!(e.freeze(h), Err(Error::Budget)));
    let h = e.start(word, &[], &Context::new(), 1000).unwrap()[0];
    assert!(e.freeze(h).is_ok());
}

#[test]
fn text_export_is_budgeted_and_bounded() {
    let (p, word) = compile("dup");
    let input = [InputNode::Text("x".repeat(4096))];
    let mut e = Executor::new(&p);
    let h = e
        .start_graph(word, &input, &[0], &Context::new(), 100)
        .unwrap()[0];
    assert!(matches!(e.freeze(h), Err(Error::Budget)));
    let h = e
        .start_graph(word, &input, &[0], &Context::new(), 10_000)
        .unwrap()[0];
    e.text_byte_limit = 100;
    assert!(matches!(e.freeze(h), Err(Error::StorageLimit)));
}

#[test]
fn freeze_demands_only_the_selected_result_and_resolves_its_context_now() {
    let (p, word) = compile(": spin recur 0 1 ; ctx rate 2 * spin");
    let mut snapshots = Vec::new();
    for n in [3, 4] {
        let context = [("rate".to_string(), Literal::Int(n))]
            .into_iter()
            .collect();
        let mut e = Executor::new(&p);
        let out = e.start(word, &[], &context, 1000).unwrap();
        let stored = e.store_put(&Store::new(), &["total"], out[0]).unwrap();
        assert_eq!(
            stored.get(&["total"]).unwrap().unwrap().nodes(),
            &[FrozenNode::Int(n * 2)]
        );
        snapshots.push(stored);
    }
    assert_ne!(snapshots[0].cid(), snapshots[1].cid());
}

#[test]
fn exponential_logical_tuple_tree_is_exported_as_a_small_shared_dag() {
    let (p, word) = compile("dup");
    let mut nodes = vec![InputNode::Scalar(Literal::Int(7))];
    for i in 0..60 {
        nodes.push(InputNode::Pair(i, i));
    }
    let mut e = Executor::new(&p);
    let h = e
        .start_graph(word, &nodes, &[60], &Context::new(), 2000)
        .unwrap()[0];
    let value = e.freeze(h).unwrap();
    assert_eq!(value.nodes().len(), 61);
    assert_eq!(value.cid(), e.content_id(h).unwrap());
}
