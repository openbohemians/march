use march_research::fast::{
    Context, Error, Executor, InputNode, Literal, Program, Value, source, stream,
};

fn compile(s: &str) -> (Program, usize) {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, s).unwrap();
    (p, w)
}
fn eval(s: &str) -> Result<Vec<Value>, Error> {
    let (p, w) = compile(s);
    Executor::new(&p).run(w, &[], &Context::new(), 100_000)
}
fn fields(v: Value) -> Vec<march_research::fast::Handle> {
    match v {
        Value::Pair(a, b) => vec![a, b],
        Value::Tuple(v) => v,
        Value::Unit => vec![],
        _ => panic!("tuple expected"),
    }
}

#[test]
fn standard_text_literals_unicode_and_all_five_escapes() {
    assert_eq!(
        eval(r#""george washington" "" "é🙂" "\"\\\n\r\t""#).unwrap(),
        [
            Value::Text("george washington".into()),
            Value::Text("".into()),
            Value::Text("é🙂".into()),
            Value::Text("\"\\\n\r\t".into())
        ]
    );
    assert_eq!(
        eval(r#""42" 42"#).unwrap(),
        [Value::Text("42".into()), Value::Int(42)]
    );
    assert_eq!(eval("\"a\nb\"").unwrap(), [Value::Text("a\nb".into())]);
}

#[test]
fn malformed_text_is_atomic_and_quote_lookup_remains_separate() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, "7").unwrap();
    let before = p.to_image(w).unwrap();
    for s in [r#""unterminated"#, r#""\q""#, r#""x"tail"#, "\"ends\\"] {
        assert!(stream::compile(&mut p, s).is_err(), "{s}");
        assert_eq!(p.to_image(w).unwrap(), before);
    }
    assert_eq!(
        eval(r#": "42" 99 ; "42" ' "42" call"#).unwrap(),
        [Value::Text("42".into()), Value::Int(99)]
    );
    assert_eq!(
        eval("-- \"unterminated ignored\n7").unwrap(),
        [Value::Int(7)]
    );
}

#[test]
fn strings_are_decoded_after_word_not_by_a_new_lexer() {
    let mut p = stream::seed().unwrap();
    // Install an interpreter that deliberately chooses dictionary lookup.
    // Merely rebinding a helper cannot change already-compiled CID references.
    stream::compile(
        &mut p,
        r#": "hi" 7 ; : stream.interpret stream.word stream.find stream.execute ;"#,
    )
    .unwrap();
    let w = stream::compile(&mut p, r#""hi""#).unwrap();
    assert_eq!(
        Executor::new(&p)
            .run(w, &[], &Context::new(), 1000)
            .unwrap(),
        [Value::Int(7)]
    );
}

#[test]
fn text_operations_are_byte_explicit_and_exact() {
    assert_eq!(
        eval(r#""é🙂" dup text-bytes swap text-chars "ab" "cd" text-concat "é🙂" 2 6 text-slice"#)
            .unwrap(),
        [
            Value::Int(6),
            Value::Int(2),
            Value::Text("abcd".into()),
            Value::Text("🙂".into())
        ]
    );
    for s in [
        r#""é" 1 2 text-slice"#,
        r#""abc" 2 1 text-slice"#,
        r#""a" 0 9 text-slice"#,
    ] {
        assert!(eval(s).is_err());
    }
    assert_eq!(
        eval(r#""a" "b" lt? "same" "same" eq? "é" "é" eq?"#).unwrap(),
        [Value::Bool(true), Value::Bool(true), Value::Bool(false)]
    );
    assert_ne!(
        Value::Text("".into()).scalar_cid(),
        Value::Unit.scalar_cid()
    );
}

#[test]
fn tuple_arity_metadata_and_selective_demand() {
    let (p, w) =
        compile(": spin recur 0 1 ; 7 9223372036854775807 1 + spin tuple 3 dup tuple-length");
    let mut e = Executor::new(&p);
    let roots = e.start(w, &[], &Context::new(), 1000).unwrap();
    assert_eq!(e.force(roots[1]).unwrap(), Value::Int(3));
    assert_eq!(e.stats().primitive_ops, 0);
    let f = fields(e.force(roots[0]).unwrap());
    assert_eq!(e.force(f[0]).unwrap(), Value::Int(7));
    assert_eq!(e.force(f[1]), Err(Error::Overflow));
    let ops = e.stats().primitive_ops;
    assert_eq!(e.force(f[1]), Err(Error::Overflow));
    assert_eq!(e.stats().primitive_ops, ops);
    assert!(matches!(e.force(f[2]), Err(Error::Cycle | Error::Budget)));
}

#[test]
fn index_type_range_and_container_error_order_do_not_force_fields() {
    assert_eq!(
        eval("ctx missing tuple 1 0 nth"),
        Err(Error::MissingContext("missing".into()))
    );
    assert_eq!(eval("7 ctx index nth"), Err(Error::Type("expected tuple")));
    for s in [
        "ctx missing tuple 1 -1 nth",
        "ctx missing tuple 1 true nth",
        "ctx missing tuple 1 1 nth",
    ] {
        assert!(matches!(eval(s), Err(Error::Type(_) | Error::Output(_))));
    }
}

#[test]
fn empty_tuple_is_unit_singleton_is_not_its_field_pair_is_two_tuple() {
    assert_eq!(
        eval("tuple 0 unit eq? 7 tuple 1 7 eq? unit 7 eq? tuple 0 7 eq?").unwrap(),
        [
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false)
        ]
    );
    assert_eq!(
        eval("1 2 pair 1 2 tuple 2 eq?").unwrap(),
        [Value::Bool(true)]
    );
    let (p, w) = compile("unit tuple 0 1 2 pair 1 2 tuple 2 7 7 tuple 1");
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 10000).unwrap();
    assert_eq!(e.content_id(h[0]).unwrap(), e.content_id(h[1]).unwrap());
    assert_eq!(e.content_id(h[2]).unwrap(), e.content_id(h[3]).unwrap());
    assert_ne!(e.content_id(h[4]).unwrap(), e.content_id(h[5]).unwrap());
}

#[test]
fn cross_type_equality_is_false_in_both_orders_and_execution_paths() {
    let (mut p, _) = compile(": same eq? ; : quoted 7 ;");
    let values = [
        "1",
        "true",
        "unit",
        "\"a\"",
        "' quoted",
        "7 tuple 1",
        "1 2 pair",
    ];
    for (i, a) in values.iter().enumerate() {
        for (j, b) in values.iter().enumerate() {
            let w = stream::compile(&mut p, &format!("{a} {b} same")).unwrap();
            let expected = vec![Value::Bool(i == j)];
            assert_eq!(
                Executor::new(&p)
                    .run(w, &[], &Context::new(), 10000)
                    .unwrap(),
                expected
            );
            let mut e = Executor::new(&p);
            let h = e.start(w, &[], &Context::new(), 10000).unwrap();
            assert_eq!(e.force(h[0]).unwrap(), expected[0]);
        }
    }
}

#[test]
fn mixed_tuple_fields_short_circuit_but_equality_still_demands_operands() {
    assert_eq!(
        eval("1 ctx absent pair true ctx absent pair eq?").unwrap(),
        [Value::Bool(false)]
    );
    assert_eq!(
        eval("\"a\" tuple 1 1 tuple 1 eq?").unwrap(),
        [Value::Bool(false)]
    );
    for s in ["1 ctx absent eq?", "ctx absent true eq?"] {
        assert_eq!(eval(s), Err(Error::MissingContext("absent".into())));
    }
    assert_eq!(
        eval("9223372036854775807 1 + true eq?"),
        Err(Error::Overflow)
    );
    for s in ["1 true lt?", "\"a\" 1 lt?", "1 true +"] {
        assert!(matches!(eval(s), Err(Error::Type(_))));
    }
}

#[test]
fn tuple_equality_short_circuits_and_never_skips_a_shared_failure() {
    assert_eq!(
        eval("1 ctx missing pair 2 ctx missing pair eq?").unwrap(),
        [Value::Bool(false)]
    );
    assert_eq!(
        eval("1 ctx missing pair 1 7 pair eq?"),
        Err(Error::MissingContext("missing".into()))
    );
    assert_eq!(
        eval("1 ctx missing pair dup eq?"),
        Err(Error::MissingContext("missing".into()))
    );
    assert_eq!(
        eval("1 2 pair 3 tuple 1 pair 1 2 pair 3 tuple 1 pair eq?").unwrap(),
        [Value::Bool(true)]
    );
    assert_eq!(
        eval("1 2 pair 3 pair 1 9 pair ctx missing pair eq?").unwrap(),
        [Value::Bool(false)]
    );
    assert_eq!(
        eval("1 2 pair 1 3 pair eq? 7 pair false 7 pair eq?").unwrap(),
        [Value::Bool(true)]
    );
}

#[test]
fn functional_replacement_shares_other_fields_and_does_not_demand_replacement() {
    let (p, w) = compile("7 ctx missing 9 tuple 3 dup 1 42 tuple-set");
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 10000).unwrap();
    let a = fields(e.force(h[0]).unwrap());
    let b = fields(e.force(h[1]).unwrap());
    assert_eq!(a[0], b[0]);
    assert_eq!(a[2], b[2]);
    assert_eq!(e.force(a[1]), Err(Error::MissingContext("missing".into())));
    assert_eq!(e.force(b[1]).unwrap(), Value::Int(42));
    assert_eq!(
        eval("1 2 pair 1 ctx absent tuple-set first").unwrap(),
        [Value::Int(1)]
    );
}

#[test]
fn unpack_is_fixed_arity_and_does_not_demand_unused_fields() {
    assert_eq!(
        eval("1 ctx missing 3 tuple 3 untuple 3 swap drop").unwrap(),
        [Value::Int(1), Value::Int(3)]
    );
    assert!(matches!(
        eval("1 2 3 tuple 3 untuple 2"),
        Err(Error::Arity {
            expected: 2,
            actual: 3
        })
    ));
    // Zero-result unpack has no observable result and therefore no demand.
    assert_eq!(eval("ctx missing untuple 0 7").unwrap(), [Value::Int(7)]);
    let (p, w) = compile("7 dup dup tuple 3");
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    let f = fields(e.force(h[0]).unwrap());
    assert_eq!(f[0], f[1]);
    assert_eq!(f[1], f[2]);
}

#[test]
fn data_images_and_cross_reader_identity_ignore_local_text_ids() {
    let src = r#": message "a\nb" ; "x" message 3 tuple 3"#;
    let (p, w) = compile(src);
    let mut q = Program::new();
    q.intern_text("unreachable history").unwrap();
    let qw = source::compile(&mut q, src).unwrap();
    assert_eq!(p.cid(w), q.cid(qw));
    let bytes = p.to_image(w).unwrap();
    let (r, rw) = Program::from_image(&bytes).unwrap();
    assert_eq!(r.cid(rw), p.cid(w));
    assert_eq!(r.to_image(rw).unwrap(), bytes);
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    let cid = e.content_id(h[0]).unwrap();
    let mut e = Executor::new(&r);
    let h = e.start(rw, &[], &Context::new(), 1000).unwrap();
    assert_eq!(cid, e.content_id(h[0]).unwrap());
}

#[test]
fn computed_text_can_be_emitted_but_runtime_tuples_cannot() {
    let (p, _) = compile(
        r#": greeting: stream.word stream.begin "hel" "lo" text-concat stream.emit-literal stream.end ; immediate greeting: hello : other "hello" ;"#,
    );
    assert_eq!(
        p.cid(p.lookup("hello").unwrap()),
        p.cid(p.lookup("other").unwrap())
    );
    let mut p = stream::seed().unwrap();
    assert!(stream::compile(&mut p, ": bad 1 2 pair stream.emit-literal ; immediate bad").is_err());
}

#[test]
fn collection_traces_tuple_fields_and_reclaims_unreachable_runtime_text() {
    let (p, w) =
        compile(r#""a" "b" text-concat "c" "d" text-concat 7 tuple 3 "unused" "text" text-concat"#);
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 10000).unwrap();
    let old = fields(e.force(h[0]).unwrap());
    e.force(old[0]).unwrap();
    e.force(old[1]).unwrap();
    e.force(h[1]).unwrap();
    assert_eq!(e.storage().text_bytes, 14);
    let roots = e.collect(&[h[0]], 10000).unwrap();
    assert_eq!(e.storage().text_bytes, 4);
    assert_eq!(e.storage().tuple_fields, 3);
    assert_eq!(e.force(old[0]), Err(Error::StaleHandle));
    let f = fields(e.force(roots[0]).unwrap());
    assert_eq!(e.force(f[0]).unwrap(), Value::Text("ab".into()));
    e.collect(&[], 10000).unwrap();
    assert_eq!(e.storage().text_bytes, 0);
    assert_eq!(e.storage().tuple_fields, 0);
}

#[test]
fn text_and_tuple_storage_caps_abort_cleanly_and_reset() {
    let (p, w) = compile(r#""abcd" "efgh" text-concat"#);
    let mut e = Executor::new(&p);
    e.text_byte_limit = 7;
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    assert_eq!(e.force(h[0]), Err(Error::StorageLimit));
    assert_eq!(e.collect(&h, 1000), Err(Error::StorageLimit));
    e.text_byte_limit = 8;
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    assert_eq!(e.force(h[0]).unwrap(), Value::Text("abcdefgh".into()));
    let (p, w) = compile("1 2 3 tuple 3");
    let mut e = Executor::new(&p);
    e.tuple_field_limit = 2;
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    assert_eq!(e.force(h[0]), Err(Error::StorageLimit));
}

#[test]
fn structured_host_inputs_and_text_context_are_validated() {
    let (mut p, w) = compile("dup tuple-length swap 0 nth");
    let t = p.intern_text("host").unwrap();
    let mut e = Executor::new(&p);
    let h = e
        .start_graph(
            w,
            &[
                InputNode::Scalar(Literal::Text(t)),
                InputNode::Text("runtime".into()),
                InputNode::Tuple(vec![0, 1, 0]),
            ],
            &[2],
            &Context::new(),
            1000,
        )
        .unwrap();
    assert_eq!(e.force(h[0]).unwrap(), Value::Int(3));
    assert_eq!(e.force(h[1]).unwrap(), Value::Text("host".into()));
    assert!(
        e.start_graph(w, &[InputNode::Tuple(vec![0])], &[0], &Context::new(), 1000)
            .is_err()
    );
    let (mut p, w) = compile("ctx label");
    let t = p.intern_text("context").unwrap();
    let c = Context::from([("label".into(), Literal::Text(t))]);
    assert_eq!(
        Executor::new(&p).run(w, &[], &c, 1000).unwrap(),
        [Value::Text("context".into())]
    );
    assert!(p.context_cid(&c).is_ok());
    assert!(
        p.context_cid(&Context::from([("x".into(), Literal::Text(usize::MAX))]))
            .is_err()
    );
}

#[test]
fn infinite_tuple_hash_and_equality_are_bounded() {
    let (p, w) = compile(": forever 7 recur 0 1 pair ; forever");
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 300).unwrap();
    assert!(matches!(
        e.content_id(h[0]),
        Err(Error::Budget | Error::Cycle)
    ));
    let (p, w) = compile(": forever 7 recur 0 1 pair ; forever dup eq?");
    assert!(matches!(
        Executor::new(&p).run(w, &[], &Context::new(), 300),
        Err(Error::Budget | Error::Cycle)
    ));
}

#[test]
fn content_cid_of_text_matches_host_scalar_cid() {
    let (p, w) = compile(r#""ab" "a" "b" text-concat"#);
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    let a = e.content_id(h[0]).unwrap();
    assert_eq!(a, e.content_id(h[1]).unwrap());
    assert_eq!(Some(a), Value::Text("ab".into()).scalar_cid());
}

#[test]
fn collection_failure_is_atomic_for_tuple_and_text_arenas() {
    let (p, w) = compile(r#""a" "b" text-concat 7 8 tuple 3"#);
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 10000).unwrap();
    let f = fields(e.force(h[0]).unwrap());
    e.force(f[0]).unwrap();
    let before = e.storage();
    assert_eq!(e.collect(&h, 0), Err(Error::Budget));
    assert_eq!(e.storage(), before);
    assert_eq!(e.force(f[0]).unwrap(), Value::Text("ab".into()));
    let cid = e.content_id(h[0]).unwrap();
    let roots = e.collect(&h, 10000).unwrap();
    assert_eq!(e.content_id(roots[0]).unwrap(), cid);
}

#[test]
fn pending_tuple_and_runtime_text_input_survive_collection_before_demand() {
    let (p, w) = compile("dup 0 nth swap tuple-length");
    let mut e = Executor::new(&p);
    let h = e
        .start_graph(
            w,
            &[InputNode::Text("keep".into()), InputNode::Tuple(vec![0])],
            &[1],
            &Context::new(),
            10000,
        )
        .unwrap();
    let roots = e.collect(&h, 10000).unwrap();
    assert_eq!(e.force(roots[0]).unwrap(), Value::Text("keep".into()));
    assert_eq!(e.force(roots[1]).unwrap(), Value::Int(1));
}

#[test]
fn malformed_text_image_is_rejected_even_with_a_matching_record_hash() {
    let mut record = vec![2];
    record.extend_from_slice(&1u64.to_le_bytes());
    record.extend_from_slice(&[1, 4]);
    record.extend_from_slice(&1u64.to_le_bytes());
    record.push(255);
    let cid = march_research::Cid::digest(b"march-definition-v1", &record);
    let mut bytes = b"MARCHF05".to_vec();
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&cid.0);
    bytes.extend_from_slice(&(record.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&record);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&cid.0);
    assert!(Program::from_image(&bytes).is_err());
}

#[test]
fn text_literals_intern_exact_bytes_and_ignore_spelling_history() {
    let mut a = Program::new();
    let first = a.intern_text("line\n").unwrap();
    assert_eq!(a.intern_text("line\n").unwrap(), first);
    let x = source::compile(&mut a, "\"line\n\"").unwrap();
    let mut b = Program::new();
    b.intern_text("unrelated").unwrap();
    let y = source::compile(&mut b, r#""line\n""#).unwrap();
    assert_eq!(a.cid(x), b.cid(y));
    assert_eq!(a.to_image(x).unwrap(), b.to_image(y).unwrap());
}

#[test]
fn text_comparison_and_content_hash_obey_observation_budget() {
    let (p, w) = compile(&format!("\"{}\" dup eq?", "a".repeat(1000)));
    assert_eq!(
        Executor::new(&p).run(w, &[], &Context::new(), 100),
        Err(Error::Budget)
    );
    let (p, w) = compile(&format!("\"{}\"", "a".repeat(1000)));
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 100).unwrap();
    assert_eq!(e.content_id(h[0]), Err(Error::Budget));
    assert_eq!(e.force(h[0]), Err(Error::Budget));
}
