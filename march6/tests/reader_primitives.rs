use march_research::{Atom, Bindings, Cid, Clause, Image, Node, ReduceError, Reducer, Store};

fn atom(s: &mut Store, a: Atom) -> Cid {
    s.intern(Node::Const(a))
}
fn text(s: &mut Store, t: &str) -> Cid {
    atom(s, Atom::Text(t.into()))
}
fn int(s: &mut Store, n: i64) -> Cid {
    atom(s, Atom::Int(n))
}
fn run(s: &mut Store, root: Cid) -> Result<Cid, ReduceError> {
    Reducer::with_budget(s, &Bindings::new(), 100_000)
        .run(root)
        .map(|r| r.root)
}
fn field(s: &Store, r: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = s.get(r) else {
        panic!("not a record")
    };
    fields.iter().find(|(k, _)| k == name).unwrap().1
}
fn optional(s: &mut Store, root: Cid, expected: Option<i64>) {
    let result = run(s, root).unwrap();
    assert_eq!(
        s.get(field(s, result, "found")),
        Some(&Node::Const(Atom::Bool(expected.is_some())))
    );
    assert_eq!(
        s.get(field(s, result, "value")),
        Some(&Node::Const(expected.map_or(Atom::Unit, Atom::Int)))
    );
}

#[test]
fn decimal_contract_and_generated_integer_roundtrips() {
    let mut s = Store::new();
    for (input, expected) in [
        ("", None),
        ("+", None),
        ("-", None),
        (" 1", None),
        ("1 ", None),
        ("0x10", None),
        ("1_000", None),
        ("１２", None),
        ("1.0", None),
        ("--1", None),
        ("+0", Some(0)),
        ("-0", Some(0)),
        ("001", Some(1)),
        ("9223372036854775807", Some(i64::MAX)),
        ("-9223372036854775808", Some(i64::MIN)),
        ("9223372036854775808", None),
        ("-9223372036854775809", None),
    ] {
        let input = text(&mut s, input);
        let root = s.intern(Node::ParseInt(input));
        optional(&mut s, root, expected);
    }
    let mut bits = 17u64;
    for _ in 0..500 {
        bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = bits as i64;
        let input = text(&mut s, &value.to_string());
        let root = s.intern(Node::ParseInt(input));
        optional(&mut s, root, Some(value));
    }
}

#[test]
fn token_cursor_contract_including_utf8_and_non_ascii_whitespace() {
    let mut s = Store::new();
    let source = " \t\n\r\u{b}\u{c}α:β\u{a0}γ ; ";
    let input = text(&mut s, source);
    let zero = int(&mut s, 0);
    let first = s.intern(Node::NextToken {
        text: input,
        position: zero,
    });
    let result = run(&mut s, first).unwrap();
    assert_eq!(
        s.get(field(&s, result, "token")),
        Some(&Node::Const(Atom::Text("α:β\u{a0}γ".into())))
    );
    let position = field(&s, result, "position");
    let second = s.intern(Node::NextToken {
        text: input,
        position,
    });
    let second = run(&mut s, second).unwrap();
    assert_eq!(
        s.get(field(&s, second, "token")),
        Some(&Node::Const(Atom::Text(";".into())))
    );
    let position = field(&s, second, "position");
    let eof = s.intern(Node::NextToken {
        text: input,
        position,
    });
    let eof = run(&mut s, eof).unwrap();
    assert_eq!(
        s.get(field(&s, eof, "position")),
        Some(&Node::Const(Atom::Int(source.len() as i64)))
    );
    assert_eq!(
        s.get(field(&s, eof, "found")),
        Some(&Node::Const(Atom::Bool(false)))
    );
    for offset in -1..=(source.len() as i64 + 1) {
        let position = int(&mut s, offset);
        let node = s.intern(Node::NextToken {
            text: input,
            position,
        });
        let valid = usize::try_from(offset).is_ok_and(|i| source.is_char_boundary(i));
        assert_eq!(run(&mut s, node).is_ok(), valid, "cursor {offset}");
    }
}

#[test]
fn every_reader_operation_stages_each_input_subset_through_an_image() {
    for operation in 0..4 {
        let mut s = Store::new();
        let a = s.intern(Node::Hole("a".into()));
        let b = s.intern(Node::Hole("b".into()));
        let c = s.intern(Node::Hole("c".into()));
        let number = int(&mut s, 42);
        let record = s.intern(Node::Record(vec![("x".into(), number)]));
        let key = text(&mut s, "x");
        let input = text(&mut s, "42");
        let zero = int(&mut s, 0);
        let (root, args) = match operation {
            0 => (
                s.intern(Node::NextToken {
                    text: a,
                    position: b,
                }),
                vec![input, zero],
            ),
            1 => (s.intern(Node::ParseInt(a)), vec![input]),
            2 => (
                s.intern(Node::Lookup { record: a, key: b }),
                vec![record, key],
            ),
            _ => (
                s.intern(Node::PutKey {
                    record: a,
                    key: b,
                    value: c,
                }),
                vec![record, key, zero],
            ),
        };
        let full = Bindings(
            args.iter()
                .enumerate()
                .map(|(i, v)| (["a", "b", "c"][i].into(), *v))
                .collect(),
        );
        let expected = Reducer::with_budget(&mut s, &full, 100_000)
            .run(root)
            .unwrap()
            .root;
        for mask in 0..(1 << args.len()) {
            let mut early = Bindings::new();
            let mut late = Bindings::new();
            for (i, value) in args.iter().enumerate() {
                let target = if mask & (1 << i) != 0 {
                    &mut early
                } else {
                    &mut late
                };
                target.0.insert(["a", "b", "c"][i].into(), *value);
            }
            let partial = Reducer::with_budget(&mut s, &early, 100_000)
                .run(root)
                .unwrap()
                .root;
            let mut roots = vec![partial];
            roots.extend(late.0.values().copied());
            let image = Image::from_store(&s, &roots).unwrap();
            let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
            let resumed = Reducer::with_budget(&mut loaded, &late, 100_000)
                .run(partial)
                .unwrap()
                .root;
            assert_eq!(resumed, expected, "operation {operation}, mask {mask}");
        }
    }
}

#[test]
fn absent_key_and_present_unit_are_distinct_and_updates_are_canonical() {
    let mut s = Store::new();
    let unit = atom(&mut s, Atom::Unit);
    let record = s.intern(Node::Record(vec![("a".into(), unit)]));
    for (key, expected) in [("a", true), ("b", false)] {
        let key = text(&mut s, key);
        let lookup = s.intern(Node::Lookup { record, key });
        let result = run(&mut s, lookup).unwrap();
        assert_eq!(field(&s, result, "value"), unit);
        assert_eq!(
            s.get(field(&s, result, "found")),
            Some(&Node::Const(Atom::Bool(expected)))
        );
    }
    let b = text(&mut s, "b");
    let a = text(&mut s, "a");
    let number = int(&mut s, 3);
    let put = s.intern(Node::PutKey {
        record,
        key: b,
        value: number,
    });
    let put = s.intern(Node::PutKey {
        record: put,
        key: a,
        value: number,
    });
    let expected = s.intern(Node::Record(vec![
        ("b".into(), number),
        ("a".into(), number),
    ]));
    assert_eq!(run(&mut s, put).unwrap(), expected);
    assert_eq!(s.get(record), Some(&Node::Record(vec![("a".into(), unit)])));
}

#[test]
fn byte_work_consumes_budget_even_for_one_huge_token() {
    for parse in [false, true] {
        let mut s = Store::new();
        let input = text(&mut s, &"0".repeat(20_000));
        let position = int(&mut s, 0);
        let node = s.intern(if parse {
            Node::ParseInt(input)
        } else {
            Node::NextToken {
                text: input,
                position,
            }
        });
        assert_eq!(
            Reducer::with_budget(&mut s, &Bindings::new(), 1_000).run(node),
            Err(ReduceError::BudgetExhausted { limit: 1_000 })
        );
        assert!(run(&mut s, node).is_ok());
    }
}

fn description(s: &mut Store, op: &str, fields: &[(&str, Cid)]) -> Cid {
    let op = text(s, op);
    let mut fields: Vec<_> = fields.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    fields.push(("op".into(), op));
    s.intern(Node::Record(fields))
}

#[test]
fn reflected_reader_quotations_have_the_exact_hand_built_cids() {
    let mut s = Store::new();
    let mut params = Vec::new();
    let mut descriptions = Vec::new();
    for index in 0..3 {
        params.push(s.intern(Node::Param(index)));
        let index = int(&mut s, i64::from(index));
        descriptions.push(description(&mut s, "param", &[("index", index)]));
    }
    let (a, b, c) = (params[0], params[1], params[2]);
    let (ad, bd, cd) = (descriptions[0], descriptions[1], descriptions[2]);
    for (op, arity, fields, node) in [
        (
            "next-token",
            2,
            vec![("text", ad), ("position", bd)],
            Node::NextToken {
                text: a,
                position: b,
            },
        ),
        ("parse-int", 1, vec![("text", ad)], Node::ParseInt(a)),
        (
            "lookup",
            2,
            vec![("record", ad), ("key", bd)],
            Node::Lookup { record: a, key: b },
        ),
        (
            "put-key",
            3,
            vec![("record", ad), ("key", bd), ("value", cd)],
            Node::PutKey {
                record: a,
                key: b,
                value: c,
            },
        ),
    ] {
        let body = s.intern(node);
        let expected = s.intern(Node::Quote {
            params: arity,
            body,
        });
        let body = description(&mut s, op, &fields);
        let parameters = int(&mut s, i64::from(arity));
        let described = description(
            &mut s,
            "quote",
            &[("parameters", parameters), ("body", body)],
        );
        let root = s.intern(Node::Intern(described));
        assert_eq!(run(&mut s, root).unwrap(), expected, "{op}");
        let args = match op {
            "next-token" => vec![text(&mut s, "7"), int(&mut s, 0)],
            "parse-int" => vec![text(&mut s, "7")],
            "lookup" => vec![s.intern(Node::Record(vec![])), text(&mut s, "key")],
            _ => vec![
                s.intern(Node::Record(vec![])),
                text(&mut s, "key"),
                int(&mut s, 7),
            ],
        };
        let direct = match op {
            "next-token" => Node::NextToken {
                text: args[0],
                position: args[1],
            },
            "parse-int" => Node::ParseInt(args[0]),
            "lookup" => Node::Lookup {
                record: args[0],
                key: args[1],
            },
            _ => Node::PutKey {
                record: args[0],
                key: args[1],
                value: args[2],
            },
        };
        let direct = s.intern(direct);
        let expected_result = run(&mut s, direct).unwrap();
        let call = s.intern(Node::Apply {
            function: expected,
            arguments: args,
        });
        assert_eq!(run(&mut s, call).unwrap(), expected_result);
    }
}

#[test]
fn reader_guard_purity_rejects_update_and_accepts_conversion() {
    let mut s = Store::new();
    let param = s.intern(Node::Param(0));
    let body = int(&mut s, 1);
    let convert = s.intern(Node::ParseInt(param));
    let guard = s.intern(Node::Get {
        record: convert,
        field: "found".into(),
    });
    let family = s.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard, body }],
    });
    let argument = text(&mut s, "42");
    let call = s.intern(Node::Dispatch {
        family,
        arguments: vec![argument],
    });
    assert_eq!(run(&mut s, call).unwrap(), body);

    let key = text(&mut s, "key");
    let put = s.intern(Node::PutKey {
        record: param,
        key,
        value: body,
    });
    let guard = s.intern(Node::Eq(put, param));
    let family = s.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard, body }],
    });
    assert!(matches!(
        run(&mut s, family),
        Err(ReduceError::ImpureGuard(_))
    ));
}

#[test]
fn dynamic_keys_do_not_bypass_capability_linearity_or_closed_code() {
    let mut s = Store::new();
    let token = atom(&mut s, Atom::Trace(vec![]));
    let key = text(&mut s, "token");
    let dictionary = s.intern(Node::Record(vec![("token".into(), token)]));
    let lookup = s.intern(Node::Lookup {
        record: dictionary,
        key,
    });
    let capture = s.intern(Node::Quote {
        params: 0,
        body: lookup,
    });
    assert!(matches!(
        run(&mut s, capture),
        Err(ReduceError::LinearValueDuplicated(_))
    ));
    let value = s.intern(Node::Get {
        record: lookup,
        field: "value".into(),
    });
    let duplicate = s.intern(Node::Pair(value, value));
    assert!(matches!(
        run(&mut s, duplicate),
        Err(ReduceError::LinearValueDuplicated(_))
    ));
    let alias = text(&mut s, "alias");
    let duplicate = s.intern(Node::PutKey {
        record: dictionary,
        key: alias,
        value: token,
    });
    assert!(matches!(
        run(&mut s, duplicate),
        Err(ReduceError::LinearValueDuplicated(_))
    ));
}
