use march_research::fast::{Context, Error, Executor, Program, Value, stream};

const MAKE_ANSWER: &str = r#"
    : answer-data "sequence" "int" 42 tuple 2 tuple 1 tuple 2 ;
    : make-answer answer-data stream.construct
        dup stream.last-cid "answer" swap stream.bind ; immediate
    make-answer
"#;

fn run(p: &Program, word: usize) -> Vec<Value> {
    Executor::new(p)
        .run(word, &[], &Context::new(), 10000)
        .unwrap()
}

fn roundtrip(p: &mut Program, cid: march_research::Cid) {
    let src = format!(
        r#"
        : roundtrip dup dup "{cid}" stream.describe stream.construct
            stream.last-cid "{cid}" eq? stream.emit-literal ; immediate
        roundtrip
    "#
    );
    let w = stream::compile(p, &src).unwrap();
    assert_eq!(run(p, w), [Value::Bool(true)]);
}

#[test]
fn every_seed_definition_roundtrips_through_march_data() {
    let mut p = stream::seed().unwrap();
    let cids: Vec<_> = (0..p.len()).map(|w| p.cid(w).unwrap()).collect();
    for cid in cids {
        roundtrip(&mut p, cid);
    }
}

#[test]
fn every_item_kind_and_ordered_family_roundtrips() {
    let mut p = stream::seed().unwrap();
    stream::compile(
        &mut p,
        r#"
        : empty ; : literals 7 true unit "é\n" [ 7 ] ;
        : context ctx absent ; : static [ 7 ] call ;
        : dynamic apply 1 1 ; : looping recur 1 1 ;
        : tuples tuple 3 untuple 3 ;
        : zero 0 eq? ; : base drop 7 ; : yes drop true ;
        : step 1 - recur 1 1 ; family down 1 1 zero base yes step ;
    "#,
    )
    .unwrap();
    for name in [
        "empty", "literals", "context", "static", "dynamic", "looping", "tuples", "down",
    ] {
        let cid = p.cid(p.lookup(name).unwrap()).unwrap();
        roundtrip(&mut p, cid);
    }
}

#[test]
fn march_constructs_binds_and_executes_new_code_with_source_identity() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, &format!("{MAKE_ANSWER} : expected 42 ; answer")).unwrap();
    assert_eq!(run(&p, w), [Value::Int(42)]);
    assert_eq!(
        p.cid(p.lookup("answer").unwrap()),
        p.cid(p.lookup("expected").unwrap())
    );
}

#[test]
fn tuple_operations_transform_a_definition_without_source_reparsing() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(
        &mut p,
        &format!(
            r#"
        {MAKE_ANSWER}
        : changed dup 1 nth 0 "int" 43 tuple 2 tuple-set 1 swap tuple-set ;
        : make-changed dup dup "answer" stream.cid-of stream.describe changed stream.construct
            dup stream.last-cid "answer43" swap stream.bind ; immediate
        make-changed : expected 43 ; answer answer43
    "#
        ),
    )
    .unwrap();
    assert_eq!(run(&p, w), [Value::Int(42), Value::Int(43)]);
    assert_eq!(
        p.cid(p.lookup("answer43").unwrap()),
        p.cid(p.lookup("expected").unwrap())
    );
}

#[test]
fn constructed_code_is_visible_to_new_state_before_executor_snapshot_knows_it() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(
        &mut p,
        r#"
        : build-two
            "sequence" "int" 19 tuple 2 tuple 1 tuple 2 stream.construct
            dup dup stream.last-cid stream.describe stream.construct
            dup stream.last-cid "nineteen" swap stream.bind ; immediate
        build-two nineteen
    "#,
    )
    .unwrap();
    assert_eq!(run(&p, w), [Value::Int(19)]);
}

#[test]
fn construct_accepts_existing_quotes_as_explicit_code_references() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(
        &mut p,
        r#"
        : square dup * ;
        : make-wrapper "sequence" "word" ' square tuple 2 tuple 1 tuple 2
            stream.construct dup stream.last-cid "wrapper" swap stream.bind ; immediate
        make-wrapper 8 wrapper
        : query dup ' square stream.describe tuple-length stream.emit-literal ; immediate query
    "#,
    )
    .unwrap();
    assert_eq!(run(&p, w), [Value::Int(64), Value::Int(2)]);
}

#[test]
fn malformed_descriptors_and_unknown_cids_do_not_publish_partial_state() {
    let mut p = stream::seed().unwrap();
    let root = stream::compile(&mut p, "7").unwrap();
    let before = p.to_image(root).unwrap();
    for descriptor in [
        "unit",
        "7",
        r#""unknown" unit tuple 2"#,
        r#""primitive" "bogus" tuple 2"#,
        r#""kernel" "bogus" tuple 2"#,
        r#""sequence" "int" true tuple 2 tuple 1 tuple 2"#,
        r#""sequence" "word" "no-cid" tuple 2 tuple 1 tuple 2"#,
        r#""sequence" "apply" -1 1 tuple 3 tuple 1 tuple 2"#,
        r#""sequence" "tuple" 4097 tuple 2 tuple 1 tuple 2"#,
        r#""sequence" "word" "0000000000000000000000000000000000000000000000000000000000000000" tuple 2 tuple 1 tuple 2"#,
        r#""sequence" "unit" 9 tuple 2 tuple 1 tuple 2"#,
        // Even interning an earlier literal must not affect the original Program.
        r#""sequence" "text" "new interned text" tuple 2 "bad" tuple 1 tuple 2 tuple 2"#,
    ] {
        let src = format!(": bad {descriptor} stream.construct ; immediate bad");
        assert!(stream::compile(&mut p, &src).is_err(), "{descriptor}");
        assert_eq!(p.to_image(root).unwrap(), before);
    }
    assert!(stream::compile(&mut p, ": bad dup tuple 1 stream.construct ; immediate bad").is_err());
    assert_eq!(p.to_image(root).unwrap(), before);
}

#[test]
fn constructing_demands_only_its_descriptor_and_bounds_divergence() {
    let mut p = stream::seed().unwrap();
    stream::compile(
        &mut p,
        r#"
        : spin 1 + recur 1 1 ;
        : bad "sequence" "int" 0 spin tuple 2 tuple 1 tuple 2 stream.construct ; immediate
        : good ctx absent drop "sequence" unit tuple 2 stream.construct ; immediate
    "#,
    )
    .unwrap();
    let w = stream::compile(&mut p, "good 7").unwrap();
    assert_eq!(run(&p, w), [Value::Int(7)]);
    let before = p.to_image(w).unwrap();
    let limits = stream::Limits {
        fuel: 10000,
        ..Default::default()
    };
    assert_eq!(
        stream::compile_with_limits(&mut p, "bad", &limits),
        Err(Error::Budget)
    );
    assert_eq!(p.to_image(w).unwrap(), before);
}

#[test]
fn excessive_descriptor_nesting_and_new_code_over_limit_are_atomic() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(
        &mut p,
        r#"
        : too-deep unit tuple 1 tuple 1 tuple 1 tuple 1 tuple 1 stream.construct ; immediate
        : too-many "sequence" "int" 123456789 tuple 2 tuple 1 tuple 2 stream.construct ; immediate
    "#,
    )
    .unwrap();
    let before = p.to_image(w).unwrap();
    assert!(
        matches!(stream::compile(&mut p, "too-deep"), Err(Error::Compiler(message)) if message.contains("nesting"))
    );
    assert_eq!(p.to_image(w).unwrap(), before);
    let limits = stream::Limits {
        code_words: p.len(),
        ..Default::default()
    };
    assert_eq!(
        stream::compile_with_limits(&mut p, "too-many", &limits),
        Err(Error::StorageLimit)
    );
    assert_eq!(p.to_image(w).unwrap(), before);
}

#[test]
fn reflection_survives_images_and_extends_without_host_seed_reassembly() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, &format!("{MAKE_ANSWER} answer")).unwrap();
    let bytes = p.to_image(w).unwrap();
    let (mut q, loaded) = Program::from_image(&bytes).unwrap();
    assert_eq!(q.to_image(loaded).unwrap(), bytes);
    assert_eq!(run(&q, loaded), [Value::Int(42)]);
    let w = stream::compile(&mut q, "make-answer answer").unwrap();
    assert_eq!(run(&q, w), [Value::Int(42)]);
}
