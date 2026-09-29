use march_research::fast::{
    Context, Error, Executor, Program,
    stack::{Machine, Value},
    store::{FrozenNode, Store},
    stream,
};
use std::sync::Arc;

fn compile(source: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, source).unwrap();
    (p, w)
}
fn run(source: &str) -> Result<(Vec<Value>, Store), Error> {
    let (p, w) = compile(source);
    Machine::new(&p).run(w, &[], &Context::new(), 100_000, &Store::new())
}
fn fields(value: &Value) -> &[Value] {
    let Value::Tuple(t) = value else {
        panic!("expected tuple")
    };
    t.fields()
}

#[test]
fn tuple_construction_projection_and_unpacking_preserve_stack_order() {
    assert_eq!(
        run("99 1 true unit tuple 3 untuple 3").unwrap().0,
        [
            Value::Int(99),
            Value::Int(1),
            Value::Bool(true),
            Value::Unit
        ]
    );
    assert_eq!(
        run("tuple 0 tuple-length 10 20 pair dup first swap second")
            .unwrap()
            .0,
        [Value::Int(0), Value::Int(10), Value::Int(20)]
    );
    assert_eq!(
        run("1 2 pair 1 2 tuple 2 eq?").unwrap().0,
        [Value::Bool(true)]
    );
    assert_eq!(run("1 2 3 tuple 3 2 nth").unwrap().0, [Value::Int(3)]);
}

#[test]
fn dup_shares_tuple_and_text_storage() {
    let (p, w) = compile(r#""a large string" dup 1 2 tuple 2 dup"#);
    let mut vm = Machine::new(&p);
    let (v, _) = vm
        .run(w, &[], &Context::new(), 1000, &Store::new())
        .unwrap();
    let (Value::Text(a), Value::Text(b)) = (&v[0], &v[1]) else {
        panic!()
    };
    assert!(Arc::ptr_eq(a, b));
    let (Value::Tuple(a), Value::Tuple(b)) = (&v[2], &v[3]) else {
        panic!()
    };
    assert!(a.shares_storage(b));
    assert_eq!(vm.stats().tuple_nodes, 1);
    assert_eq!(vm.stats().text_bytes_allocated, 0);
    assert_eq!(vm.stats().frozen_nodes, 0); // Ordinary execution performs no value hashing/export.
}

#[test]
fn tuple_update_copies_only_outer_fields_and_preserves_original() {
    let (v, _) = run(r#""shared" 1 pair 10 pair dup 1 20 tuple-set"#).unwrap();
    assert_eq!(fields(&v[0])[1], Value::Int(10));
    assert_eq!(fields(&v[1])[1], Value::Int(20));
    let (Value::Tuple(a), Value::Tuple(b)) = (&fields(&v[0])[0], &fields(&v[1])[0]) else {
        panic!()
    };
    assert!(a.shares_storage(b));
}

#[test]
fn values_outlive_machine_and_program_and_survive_reset() {
    let value = {
        let (mut p, w) = compile(r#""alive" 7 pair"#);
        let other = stream::compile(&mut p, "1").unwrap();
        let mut vm = Machine::new(&p);
        let (mut v, _) = vm
            .run(w, &[], &Context::new(), 1000, &Store::new())
            .unwrap();
        vm.run(other, &[], &Context::new(), 1000, &Store::new())
            .unwrap();
        v.pop().unwrap()
    };
    assert_eq!(
        fields(&value),
        &[Value::Text("alive".into()), Value::Int(7)]
    );
}

#[test]
fn text_operations_preserve_utf8_and_byte_slice_semantics() {
    let (v,_) = run(r#""é🙂" dup text-bytes swap text-chars "ab" "cd" text-concat "é🙂" 2 6 text-slice "a" "b" lt?"#).unwrap();
    assert_eq!(
        v,
        [
            Value::Int(6),
            Value::Int(2),
            Value::Text("abcd".into()),
            Value::Text("🙂".into()),
            Value::Bool(true)
        ]
    );
    for source in [
        r#""é" 1 2 text-slice"#,
        r#""a" 2 1 text-slice"#,
        r#""a" 0 9 text-slice"#,
        r#""a" -1 0 text-slice"#,
    ] {
        assert!(matches!(run(source), Err(Error::Type(_))));
    }
}

#[test]
fn equality_is_structural_and_cross_types_are_false() {
    assert_eq!(
        run(r#"1 "x" pair 1 "x" tuple 2 eq? 1 true eq? 1 tuple 1 1 eq? 1 2 pair 1 3 pair eq?"#)
            .unwrap()
            .0,
        [
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false)
        ]
    );
    assert!(matches!(run("1 2 pair 1 2 pair lt?"), Err(Error::Type(_))));
}

#[test]
fn aggregates_remain_strict_even_for_discarded_fields() {
    for source in [
        "7 9223372036854775807 1 + pair first",
        "1 2 pair 0 9223372036854775807 1 + tuple-set second",
        "9223372036854775807 1 + tuple 1 drop",
    ] {
        assert_eq!(run(source), Err(Error::Overflow));
    }
    assert!(run("[ 9223372036854775807 1 + ] 7 pair second").is_ok());
}

#[test]
fn aggregate_errors_are_checked() {
    for source in [
        "1 first",
        "tuple 0 first",
        "1 tuple 1 second",
        "1 tuple 1 -1 nth",
        "1 tuple 1 true nth",
        "1 tuple 1 1 nth",
        "1 tuple 1 2 3 tuple-set",
        "1 tuple 1 untuple 2",
        "true text-bytes",
    ] {
        assert!(run(source).is_err(), "{source}");
    }
}

#[test]
fn frozen_identity_matches_existing_format_for_every_value_kind() {
    for body in [
        "42",
        "true",
        "false",
        "unit",
        r#""é🙂""#,
        "' dup",
        "tuple 0",
        "1 tuple 1",
        "1 2 pair",
        "1 true unit tuple 3",
        r#""x" 7 pair false pair"#,
    ] {
        let (mut p, w) = compile(body);
        let store_word = stream::compile(&mut p, &format!("{body} \"v\" store.put")).unwrap();
        let mut graph = Executor::new(&p);
        let out = graph.start(w, &[], &Context::new(), 10000).unwrap();
        let expected = graph.freeze(out[0]).unwrap();
        let (_, s) = Machine::new(&p)
            .run(store_word, &[], &Context::new(), 10000, &Store::new())
            .unwrap();
        assert_eq!(
            s.get(&["v"]).unwrap().unwrap().cid(),
            expected.cid(),
            "{body}: actual {:?}; expected {:?}",
            s.get(&["v"]).unwrap().unwrap(),
            expected
        );
    }
}

#[test]
fn nested_namespace_roundtrip_retains_quotes_and_shared_fields() {
    let (v,s)=run(r#": never recur 0 1 ; ' never "é" pair dup pair "ns" "data" pair store.put "ns" "data" pair store.get"#).unwrap();
    assert!(s.get(&["ns", "data"]).unwrap().is_some());
    let (Value::Tuple(a), Value::Tuple(b)) = (&fields(&v[0])[0], &fields(&v[0])[1]) else {
        panic!()
    };
    assert!(a.shares_storage(b));
    let frozen = s.get(&["ns", "data"]).unwrap().unwrap();
    assert_eq!(frozen.nodes().len(), 4); // code, text, shared pair, outer pair
    let Value::Text(text) = &a.fields()[1] else {
        panic!()
    };
    let FrozenNode::Text(stored) = &frozen.nodes()[1] else {
        panic!()
    };
    assert!(Arc::ptr_eq(text, stored));
}

#[test]
fn missing_nested_quotation_code_is_not_rebound_on_import() {
    let (_, s) = run(r#": unique-code 123456789 ; ' unique-code tuple 1 "v" store.put"#).unwrap();
    let (p, w) = compile(r#""v" store.get"#);
    assert!(matches!(
        Machine::new(&p).run(w, &[], &Context::new(), 10000, &s),
        Err(Error::Store(_))
    ));
}

#[test]
fn deep_values_drop_compare_and_roundtrip_without_host_recursion() {
    let (p, w) = compile(
        r#"
        : zero 0 eq? ; : yes drop true ; : done drop unit ;
        : step 1 - recur 1 1 tuple 1 ; family deep 1 1 zero done yes step ;
        20000 deep dup "v" store.put "v" store.get
    "#,
    );
    let (v, s) = Machine::new(&p)
        .run(w, &[], &Context::new(), 3_000_000, &Store::new())
        .unwrap();
    assert_eq!(s.get(&["v"]).unwrap().unwrap().nodes().len(), 20001);
    assert_eq!(v[0], v[1]);
    assert!(format!("{:?}", v[0]).len() < 1000); // Debug output is depth/node bounded.
    drop(v);
    drop(s); // Iterative destruction, including failure cleanup, is essential.
}

#[test]
fn exponentially_large_logical_trees_remain_shared_data() {
    let (p, w) = compile(
        r#"
        : zero 0 eq? ; : yes drop true ; : done drop unit ;
        : step 1 - recur 1 1 dup pair ; family tree 1 1 zero done yes step ;
        80 tree dup "v" store.put "v" store.get eq?
    "#,
    );
    let (v, s) = Machine::new(&p)
        .run(w, &[], &Context::new(), 100_000, &Store::new())
        .unwrap();
    assert_eq!(v, [Value::Bool(true)]);
    assert_eq!(s.get(&["v"]).unwrap().unwrap().nodes().len(), 82);
}

#[test]
fn tuple_text_and_import_limits_abort_without_publishing_state() {
    let (_, initial) = run(r#"1 2 3 tuple 3 "v" store.put"#).unwrap();
    let before = initial.cid();
    for source in [
        r#"7 "x" store.put 1 2 3 tuple 3"#,
        r#"7 "x" store.put "v" store.get"#,
    ] {
        let (p, w) = compile(source);
        let mut vm = Machine::new(&p);
        vm.tuple_field_limit = 2;
        assert_eq!(
            vm.run(w, &[], &Context::new(), 10000, &initial),
            Err(Error::StorageLimit)
        );
        assert_eq!(initial.cid(), before);
        assert!(initial.get(&["x"]).unwrap().is_none());
    }
    let (p, w) = compile(r#""abc" "def" text-concat"#);
    let mut vm = Machine::new(&p);
    vm.text_byte_limit = 5;
    assert_eq!(
        vm.run(w, &[], &Context::new(), 10000, &initial),
        Err(Error::StorageLimit)
    );
}

#[test]
fn every_fuel_cut_during_export_import_and_equality_is_clean() {
    let (p, w) = compile(r#"1 "é" pair dup pair dup "v" store.put "v" store.get eq?"#);
    let initial = Store::new();
    let mut vm = Machine::new(&p);
    let mut succeeded = false;
    for budget in 0..300 {
        match vm.run(w, &[], &Context::new(), budget, &initial) {
            Ok((v, _)) => {
                assert_eq!(v, [Value::Bool(true)]);
                succeeded = true;
            }
            Err(e) => assert_eq!(e, Error::Budget),
        }
        assert!(initial.is_empty());
    }
    assert!(succeeded);
}
