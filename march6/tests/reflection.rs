use march_research::{
    Atom, Bindings, Cid, Clause, Image, Node, ReduceError, Reducer, ReflectError, Store,
};

fn int(store: &mut Store, value: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn text(store: &mut Store, value: &str) -> Cid {
    store.intern(Node::Const(Atom::Text(value.into())))
}

fn description(store: &mut Store, operation: &str, fields: Vec<(&str, Cid)>) -> Cid {
    let operation = text(store, operation);
    let mut record = vec![("op".into(), operation)];
    record.extend(fields.into_iter().map(|(name, value)| (name.into(), value)));
    store.intern(Node::Record(record))
}

fn list(store: &mut Store, values: impl IntoIterator<Item = Cid>) -> Cid {
    let values = values.into_iter().collect::<Vec<_>>();
    let mut tail = store.intern(Node::Const(Atom::Unit));
    for value in values.into_iter().rev() {
        tail = store.intern(Node::Pair(value, tail));
    }
    tail
}

fn param_description(store: &mut Store, index: i64) -> Cid {
    let index = int(store, index);
    description(store, "param", vec![("index", index)])
}

fn quote_description(store: &mut Store, parameters: i64, body: Cid) -> Cid {
    let parameters = int(store, parameters);
    description(
        store,
        "quote",
        vec![("parameters", parameters), ("body", body)],
    )
}

fn record_description(store: &mut Store, fields: Vec<(&str, Cid)>) -> Cid {
    let mut entries = Vec::with_capacity(fields.len());
    for (name, value) in fields {
        let name = text(store, name);
        entries.push(store.intern(Node::Pair(name, value)));
    }
    let fields = list(store, entries);
    description(store, "record", vec![("fields", fields)])
}

fn reflect(store: &mut Store, description: Cid) -> Result<Cid, ReduceError> {
    let root = store.intern(Node::Intern(description));
    Reducer::with_budget(store, &Bindings::new(), 1_000_000)
        .run(root)
        .map(|reduction| reduction.root)
}

#[test]
fn ordinary_values_intern_the_exact_hand_built_square_quote() {
    let mut store = Store::new();
    let parameter = param_description(&mut store, 0);
    let multiply = description(
        &mut store,
        "multiply",
        vec![("right", parameter), ("left", parameter)],
    );
    let described = quote_description(&mut store, 1, multiply);

    let parameter_node = store.intern(Node::Param(0));
    let product = store.intern(Node::Mul(parameter_node, parameter_node));
    let expected = store.intern(Node::Quote {
        params: 1,
        body: product,
    });

    assert_eq!(reflect(&mut store, described).unwrap(), expected);
}

#[test]
fn description_records_are_computable_and_field_order_is_not_semantic() {
    let mut store = Store::new();
    let parameter = param_description(&mut store, 0);
    let add = description(
        &mut store,
        "add",
        vec![("right", parameter), ("left", parameter)],
    );
    let zero = int(&mut store, 0);
    let initial = quote_description(&mut store, 0, add);
    let computed = store.intern(Node::Put {
        record: initial,
        field: "parameters".into(),
        value: zero,
    });

    assert_eq!(
        reflect(&mut store, computed),
        Err(ReduceError::ParameterOutOfRange {
            index: 0,
            arguments: 0,
        })
    );

    let one = int(&mut store, 1);
    let computed = store.intern(Node::Put {
        record: initial,
        field: "parameters".into(),
        value: one,
    });
    let reflected = reflect(&mut store, computed).unwrap();
    assert!(matches!(
        store.get(reflected),
        Some(Node::Quote { params: 1, .. })
    ));
}

#[test]
fn embed_uses_a_graph_edge_and_preserves_existing_code_identity() {
    let mut store = Store::new();
    let parameter = store.intern(Node::Param(0));
    let identity = store.intern(Node::Quote {
        params: 1,
        body: parameter,
    });
    let embedded = description(&mut store, "embed", vec![("value", identity)]);

    assert_eq!(reflect(&mut store, embedded).unwrap(), identity);
}

#[test]
fn scope_invalid_reflected_code_is_rejected_and_construction_is_rolled_back() {
    let mut store = Store::new();
    let parameter = param_description(&mut store, 1);
    let described = quote_description(&mut store, 1, parameter);
    let root = store.intern(Node::Intern(described));
    let before = store.len();

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), 10_000).run(root),
        Err(ReduceError::ParameterOutOfRange {
            index: 1,
            arguments: 1,
        })
    );
    assert_eq!(store.len(), before);
}

#[test]
fn an_existing_open_quote_cannot_cross_the_description_boundary() {
    let mut store = Store::new();
    let hole = store.intern(Node::Hole("ambient".into()));
    let open_quote = store.intern(Node::Quote {
        params: 0,
        body: hole,
    });
    let embedded = description(&mut store, "embed", vec![("value", open_quote)]);
    let root = store.intern(Node::Intern(embedded));

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), 10_000).run(root),
        Err(ReduceError::OpenCodeValue("ambient".into()))
    );
}

#[test]
fn reflection_cannot_place_effects_in_family_guards() {
    let mut store = Store::new();
    let token = param_description(&mut store, 0);
    let message_value = text(&mut store, "guard");
    let message = description(&mut store, "text", vec![("value", message_value)]);
    let emit = description(
        &mut store,
        "emit",
        vec![("token", token), ("message", message)],
    );
    let body = param_description(&mut store, 0);
    let clause = store.intern(Node::Pair(emit, body));
    let clauses = list(&mut store, [clause]);
    let parameters = int(&mut store, 1);
    let family = description(
        &mut store,
        "family",
        vec![("parameters", parameters), ("clauses", clauses)],
    );
    let root = store.intern(Node::Intern(family));
    let before = store.len();

    assert!(matches!(
        Reducer::with_budget(&mut store, &Bindings::new(), 10_000).run(root),
        Err(ReduceError::ImpureGuard(_))
    ));
    assert_eq!(store.len(), before);
}

#[test]
fn unknown_operations_fail_without_leaving_reflected_nodes() {
    let mut store = Store::new();
    let described = description(&mut store, "colon", vec![]);
    let root = store.intern(Node::Intern(described));
    let before = store.len();

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), 10_000).run(root),
        Err(ReduceError::Reflection(ReflectError::UnknownOperation(
            "colon".into()
        )))
    );
    assert_eq!(store.len(), before);
}

#[test]
fn description_schema_rejects_missing_extra_and_wrongly_typed_fields() {
    let mut store = Store::new();
    let extra_value = int(&mut store, 1);
    let extra = description(&mut store, "unit", vec![("extra", extra_value)]);
    let described = quote_description(&mut store, 0, extra);
    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Reflection(ReflectError::UnexpectedFields {
            operation: "unit".into(),
            fields: vec!["extra".into()],
        }))
    );

    let parameters = int(&mut store, 0);
    let missing = description(&mut store, "quote", vec![("parameters", parameters)]);
    assert_eq!(
        reflect(&mut store, missing),
        Err(ReduceError::Reflection(ReflectError::MissingField {
            operation: "quote".into(),
            field: "body".into(),
        }))
    );

    let wrong_type = text(&mut store, "one");
    let parameter = description(&mut store, "param", vec![("index", wrong_type)]);
    let described = quote_description(&mut store, 1, parameter);
    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Reflection(ReflectError::ExpectedInteger {
            field: "index".into(),
            value: wrong_type,
        }))
    );
}

#[test]
fn a_non_code_description_is_rejected() {
    let mut store = Store::new();
    let value = int(&mut store, 42);
    let described = description(&mut store, "int", vec![("value", value)]);

    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Type(
            "closure validation requires a quotation or family"
        ))
    );
}

#[test]
fn an_unknown_description_residualizes_for_a_later_epoch() {
    let mut store = Store::new();
    let later = store.intern(Node::Hole("description".into()));
    let root = store.intern(Node::Intern(later));

    let residual = Reducer::with_budget(&mut store, &Bindings::new(), 10_000)
        .run(root)
        .unwrap()
        .root;
    assert_eq!(store.get(residual), Some(&Node::Intern(later)));
}

#[test]
fn suspended_reflection_survives_an_image_epoch_boundary() {
    let mut store = Store::new();
    let later = store.intern(Node::Hole("description".into()));
    let root = store.intern(Node::Intern(later));
    let image = Image::from_store(&store, &[root]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();

    let parameter = param_description(&mut loaded, 0);
    let multiply = description(
        &mut loaded,
        "multiply",
        vec![("left", parameter), ("right", parameter)],
    );
    let described = quote_description(&mut loaded, 1, multiply);
    let mut bindings = Bindings::new();
    bindings.insert("description", described);
    let staged = Reducer::with_budget(&mut loaded, &bindings, 100_000)
        .run(root)
        .unwrap()
        .root;

    let parameter = loaded.intern(Node::Param(0));
    let product = loaded.intern(Node::Mul(parameter, parameter));
    let direct = loaded.intern(Node::Quote {
        params: 1,
        body: product,
    });
    assert_eq!(staged, direct);
}

#[test]
fn data_and_effect_values_cannot_be_embedded_as_code_authority() {
    let mut store = Store::new();
    let trace = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let described = description(&mut store, "embed", vec![("value", trace)]);

    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Reflection(ReflectError::ExpectedCodeValue(
            trace
        )))
    );
}

#[test]
fn arities_and_parameter_indices_must_fit_the_canonical_u16_range() {
    for invalid in [-1, 65_536] {
        let mut store = Store::new();
        let parameter = param_description(&mut store, invalid);
        let described = quote_description(&mut store, 1, parameter);
        assert_eq!(
            reflect(&mut store, described),
            Err(ReduceError::Reflection(ReflectError::IntegerOutOfRange {
                field: "index".into(),
                value: invalid,
            }))
        );
    }
}

#[test]
fn improper_argument_lists_and_duplicate_target_fields_are_errors() {
    let mut store = Store::new();
    let parameter = store.intern(Node::Param(0));
    let identity = store.intern(Node::Quote {
        params: 1,
        body: parameter,
    });
    let function = description(&mut store, "embed", vec![("value", identity)]);
    let improper = int(&mut store, 7);
    let apply = description(
        &mut store,
        "apply",
        vec![("function", function), ("arguments", improper)],
    );
    let described = quote_description(&mut store, 0, apply);
    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Reflection(ReflectError::ExpectedList(
            improper
        )))
    );

    let name = text(&mut store, "same");
    let unit = description(&mut store, "unit", vec![]);
    let entry = store.intern(Node::Pair(name, unit));
    let fields = list(&mut store, [entry, entry]);
    let record = description(&mut store, "record", vec![("fields", fields)]);
    let described = quote_description(&mut store, 0, record);
    assert_eq!(
        reflect(&mut store, described),
        Err(ReduceError::Reflection(ReflectError::DuplicateRecordField(
            "same".into()
        )))
    );
}

#[test]
fn empty_reflected_families_are_rejected_atomically() {
    let mut store = Store::new();
    let parameters = int(&mut store, 0);
    let clauses = list(&mut store, []);
    let described = description(
        &mut store,
        "family",
        vec![("parameters", parameters), ("clauses", clauses)],
    );
    let root = store.intern(Node::Intern(described));
    let before = store.len();

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), 10_000).run(root),
        Err(ReduceError::EmptyFamily)
    );
    assert_eq!(store.len(), before);
}

#[test]
fn reflected_family_order_has_exact_semantic_identity() {
    let mut store = Store::new();
    let truth_value = store.intern(Node::Const(Atom::Bool(true)));
    let truth = description(&mut store, "bool", vec![("value", truth_value)]);
    let ten_value = int(&mut store, 10);
    let ten = description(&mut store, "int", vec![("value", ten_value)]);
    let twenty_value = int(&mut store, 20);
    let twenty = description(&mut store, "int", vec![("value", twenty_value)]);
    let first = store.intern(Node::Pair(truth, ten));
    let second = store.intern(Node::Pair(truth, twenty));
    let clauses = list(&mut store, [first, second]);
    let parameters = int(&mut store, 0);
    let described = description(
        &mut store,
        "family",
        vec![("parameters", parameters), ("clauses", clauses)],
    );

    let expected = store.intern(Node::Family {
        parameters: 0,
        clauses: vec![
            Clause {
                guard: truth_value,
                body: ten_value,
            },
            Clause {
                guard: truth_value,
                body: twenty_value,
            },
        ],
    });
    assert_eq!(reflect(&mut store, described).unwrap(), expected);

    let dispatch = store.intern(Node::Dispatch {
        family: expected,
        arguments: vec![],
    });
    let result = Reducer::with_budget(&mut store, &Bindings::new(), 10_000)
        .run(dispatch)
        .unwrap()
        .root;
    assert_eq!(result, ten_value);
}

#[test]
fn every_reflectable_node_shape_maps_to_its_exact_semantic_cid() {
    let mut store = Store::new();

    let p0_description = param_description(&mut store, 0);
    let p1_description = param_description(&mut store, 1);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));

    let one_value = int(&mut store, 1);
    let one_description = description(&mut store, "int", vec![("value", one_value)]);
    let two_value = int(&mut store, 2);
    let two_description = description(&mut store, "int", vec![("value", two_value)]);
    let truth_value = store.intern(Node::Const(Atom::Bool(true)));
    let truth_description = description(&mut store, "bool", vec![("value", truth_value)]);
    let message_value = text(&mut store, "message");
    let message_description = description(&mut store, "text", vec![("value", message_value)]);
    let unit_value = store.intern(Node::Const(Atom::Unit));
    let unit_description = description(&mut store, "unit", vec![]);

    let add = store.intern(Node::Add(p0, one_value));
    let add_description = description(
        &mut store,
        "add",
        vec![("left", p0_description), ("right", one_description)],
    );
    let multiply = store.intern(Node::Mul(add, two_value));
    let multiply_description = description(
        &mut store,
        "multiply",
        vec![("left", add_description), ("right", two_description)],
    );
    let equal = store.intern(Node::Eq(multiply, p1));
    let equal_description = description(
        &mut store,
        "equal",
        vec![("left", multiply_description), ("right", p1_description)],
    );
    let pair = store.intern(Node::Pair(add, unit_value));
    let pair_description = description(
        &mut store,
        "pair",
        vec![("first", add_description), ("second", unit_description)],
    );
    let first = store.intern(Node::First(pair));
    let first_description = description(&mut store, "first", vec![("pair", pair_description)]);
    let second = store.intern(Node::Second(pair));
    let second_description = description(&mut store, "second", vec![("pair", pair_description)]);
    let conditional = store.intern(Node::If {
        condition: equal,
        when_true: first,
        when_false: second,
    });
    let conditional_description = description(
        &mut store,
        "if",
        vec![
            ("condition", equal_description),
            ("true", first_description),
            ("false", second_description),
        ],
    );

    let inner_record = store.intern(Node::Record(vec![
        ("message".into(), message_value),
        ("value".into(), conditional),
    ]));
    let inner_record_description = record_description(
        &mut store,
        vec![
            ("message", message_description),
            ("value", conditional_description),
        ],
    );
    let get = store.intern(Node::Get {
        record: inner_record,
        field: "value".into(),
    });
    let value_field = text(&mut store, "value");
    let get_description = description(
        &mut store,
        "get",
        vec![("record", inner_record_description), ("field", value_field)],
    );
    let put = store.intern(Node::Put {
        record: inner_record,
        field: "value".into(),
        value: multiply,
    });
    let put_description = description(
        &mut store,
        "put",
        vec![
            ("record", inner_record_description),
            ("field", value_field),
            ("value", multiply_description),
        ],
    );

    let identity = store.intern(Node::Quote {
        params: 1,
        body: p0,
    });
    let identity_description = quote_description(&mut store, 1, p0_description);
    let apply = store.intern(Node::Apply {
        function: identity,
        arguments: vec![get],
    });
    let apply_arguments = list(&mut store, [get_description]);
    let apply_description = description(
        &mut store,
        "apply",
        vec![
            ("function", identity_description),
            ("arguments", apply_arguments),
        ],
    );
    let emit = store.intern(Node::Emit {
        token: p1,
        message: message_value,
    });
    let emit_description = description(
        &mut store,
        "emit",
        vec![("token", p1_description), ("message", message_description)],
    );

    let recur = store.intern(Node::Recur(vec![p0]));
    let recur_arguments = list(&mut store, [p0_description]);
    let recur_description = description(&mut store, "recur", vec![("arguments", recur_arguments)]);
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause {
            guard: truth_value,
            body: recur,
        }],
    });
    let clause = store.intern(Node::Pair(truth_description, recur_description));
    let clauses = list(&mut store, [clause]);
    let one_parameter = int(&mut store, 1);
    let family_description = description(
        &mut store,
        "family",
        vec![("parameters", one_parameter), ("clauses", clauses)],
    );
    let dispatch = store.intern(Node::Dispatch {
        family,
        arguments: vec![p0],
    });
    let dispatch_arguments = list(&mut store, [p0_description]);
    let dispatch_description = description(
        &mut store,
        "dispatch",
        vec![
            ("family", family_description),
            ("arguments", dispatch_arguments),
        ],
    );
    let nested_intern = store.intern(Node::Intern(p0));
    let nested_intern_description =
        description(&mut store, "intern", vec![("description", p0_description)]);

    let body = store.intern(Node::Record(vec![
        ("apply".into(), apply),
        ("dispatch".into(), dispatch),
        ("emit".into(), emit),
        ("family".into(), family),
        ("get".into(), get),
        ("intern".into(), nested_intern),
        ("put".into(), put),
        ("quote".into(), identity),
    ]));
    let body_description = record_description(
        &mut store,
        vec![
            ("apply", apply_description),
            ("dispatch", dispatch_description),
            ("emit", emit_description),
            ("family", family_description),
            ("get", get_description),
            ("intern", nested_intern_description),
            ("put", put_description),
            ("quote", identity_description),
        ],
    );
    let expected = store.intern(Node::Quote { params: 2, body });
    let described = quote_description(&mut store, 2, body_description);

    assert_eq!(reflect(&mut store, described).unwrap(), expected);
}

#[test]
fn deep_descriptions_use_the_explicit_work_list() {
    const DEPTH: usize = 4_000;
    let mut store = Store::new();
    let mut body = param_description(&mut store, 0);
    let unit = description(&mut store, "unit", vec![]);
    for _ in 0..DEPTH {
        body = description(&mut store, "pair", vec![("first", body), ("second", unit)]);
    }
    let described = quote_description(&mut store, 1, body);
    let reflected = reflect(&mut store, described).unwrap();

    let Some(Node::Quote { mut body, .. }) = store.get(reflected).cloned() else {
        panic!("reflection did not return a quotation");
    };
    for _ in 0..DEPTH {
        let Some(Node::Pair(first, _)) = store.get(body) else {
            panic!("reflected body ended early");
        };
        body = *first;
    }
    assert_eq!(store.get(body), Some(&Node::Param(0)));
}
