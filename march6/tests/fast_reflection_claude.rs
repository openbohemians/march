//! Independent edge cases for the reflection kernel words:
//! stream.cid-of / describe / construct / last-cid / bind.
//!
//! Stack conventions (compiler words are state -> state):
//!   cid-of    ( state name -- cid-text )
//!   describe  ( state cid-or-quote -- descriptor )
//!   construct ( state descriptor -- state' )
//!   last-cid  ( state -- cid-text )
//!   bind      ( state name cid-or-quote -- state' )
use march_research::fast::{Context, Error, Executor, Program, Value, stream};

fn seeded() -> Program {
    stream::seed().unwrap()
}

fn run(p: &Program, word: usize) -> Vec<Value> {
    Executor::new(p)
        .run(word, &[], &Context::new(), 100_000)
        .unwrap()
}

fn compile_run(p: &mut Program, source: &str) -> Vec<Value> {
    let word = stream::compile(p, source).unwrap();
    run(p, word)
}

fn cid_text(p: &Program, name: &str) -> String {
    p.cid(p.lookup(name).unwrap()).unwrap().to_string()
}

/// Image of a fixed probe entry: unchanged bytes and an unchanged word count
/// prove a failed compile published nothing.
fn snapshot(p: &mut Program) -> (usize, Vec<u8>) {
    let root = stream::compile(p, ": probe-entry 7 ; probe-entry").unwrap();
    (p.len(), p.to_image(root).unwrap())
}

fn assert_atomic_failure(p: &mut Program, source: &str) -> Error {
    let (len, image) = snapshot(p);
    let error = stream::compile(p, source).unwrap_err();
    // Guard against a test that fails for the wrong reason: a miscounted stack
    // shows up as a compiler-word signature error, not as the case under test.
    assert!(
        !matches!(&error, Error::Compiler(m) if m.contains("signature")),
        "stack mistake in test source: {source}"
    );
    assert_eq!(p.len(), len, "words were published by a failed compile");
    assert_eq!(snapshot(p), (len, image));
    error
}

const ANSWER: &str = r#""sequence" "int" 42 tuple 2 tuple 1 tuple 2"#;

// Small stack helpers written in March, used by the compiler words below.
const HELPERS: &str = r#"
    : rot tuple 3 dup 1 nth swap dup 2 nth swap 0 nth ;
    : w-item "word" swap tuple 2 ;
    : q-item "quote" swap tuple 2 ;
"#;

#[test]
fn cid_text_is_64_lowercase_hex_and_matches_the_host_cid() {
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        r#": square dup * ;
           : q dup "square" stream.cid-of stream.emit-literal ; immediate q"#,
    );
    let [Value::Text(text)] = values.as_slice() else {
        panic!("{values:?}")
    };
    assert_eq!(text.len(), 64);
    assert!(
        text.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(*text, cid_text(&p, "square"));
}

#[test]
fn describe_by_cid_text_and_by_quote_give_structurally_equal_data() {
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        r#": square dup * ;
           : q dup dup dup "square" stream.cid-of stream.describe
               swap ' square stream.describe eq? stream.emit-literal ; immediate q"#,
    );
    assert_eq!(values, [Value::Bool(true)]);
}

#[test]
fn constructing_an_existing_definition_returns_its_existing_cid() {
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        r#": square dup * ;
           : q dup dup dup dup "square" stream.cid-of stream.describe stream.construct
               stream.last-cid swap "square" stream.cid-of eq? stream.emit-literal ; immediate q"#,
    );
    assert_eq!(values, [Value::Bool(true)]);
}

#[test]
fn binding_a_second_name_aliases_the_same_word_without_copying_it() {
    let mut p = seeded();
    stream::compile(
        &mut p,
        r#": square dup * ;
           : alias dup "square" stream.cid-of "sq" swap stream.bind ; immediate alias"#,
    )
    .unwrap();
    assert_eq!(p.lookup("sq"), p.lookup("square"));
    assert_eq!(compile_run(&mut p, "5 sq"), [Value::Int(25)]);
}

#[test]
fn a_dropped_new_state_leaves_the_old_state_without_the_construction() {
    let mut p = seeded();
    // Construct and bind in a NEW state, drop it, then query the OLD state.
    let error = assert_atomic_failure(
        &mut p,
        &format!(
            r#": leak dup {ANSWER} stream.construct
                   dup stream.last-cid "answer" swap stream.bind drop
                   dup "answer" stream.cid-of stream.emit-literal ; immediate leak"#
        ),
    );
    assert!(
        matches!(&error, Error::Compiler(m) if m.contains("answer")),
        "{error:?}"
    );
    assert!(p.lookup("answer").is_none());
    // last-cid of a state that constructed nothing, even after a sibling
    // state constructed something.
    let error = assert_atomic_failure(
        &mut p,
        &format!(
            r#": stale dup {ANSWER} stream.construct drop dup stream.last-cid
                   stream.emit-literal ; immediate stale"#
        ),
    );
    assert!(
        matches!(&error, Error::Compiler(m) if m.contains("constructed")),
        "{error:?}"
    );
}

#[test]
fn code_built_inside_one_invocation_is_referenced_by_cid_as_word_and_quote() {
    // Snapshot skew: the running executor never sees `a`, yet `b` is built
    // from `a`'s CID both as a called word and as a quotation that is called.
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        &format!(
            r#"{HELPERS}
               : items tuple 2 dup 0 nth w-item swap dup 0 nth q-item swap
                   1 nth w-item "call" tuple 1 swap tuple 4 ;
               : build
                   dup "+" stream.cid-of swap
                   {ANSWER} stream.construct
                   dup stream.last-cid "a" swap stream.bind
                   dup "a" stream.cid-of
                   rot items "sequence" swap tuple 2 stream.construct
                   dup stream.last-cid "b" swap stream.bind ; immediate
               build a b"#
        ),
    );
    assert_eq!(values, [Value::Int(42), Value::Int(84)]);
}

#[test]
fn a_family_built_from_clause_cids_equals_the_source_family_and_recurs() {
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        r#": zero 0 eq? ; : base drop 0 ; : yes drop true ; : step 1 - recur 1 1 ;
           family down 1 1 zero base yes step ;
           : build2
               dup dup "zero" stream.cid-of swap "base" stream.cid-of tuple 2
               over dup "yes" stream.cid-of swap "step" stream.cid-of tuple 2
               tuple 2 "family" swap tuple 2 stream.construct
               dup stream.last-cid "down2" swap stream.bind ; immediate
           build2 5 down2"#,
    );
    assert_eq!(values, [Value::Int(0)]);
    assert_eq!(cid_text(&p, "down2"), cid_text(&p, "down"));
}

#[test]
fn compiler_states_cannot_become_descriptor_data() {
    let mut p = seeded();
    for descriptor in [
        // The state itself as the descriptor.
        "dup",
        // The state as a text literal item, and as a word reference.
        r#"dup "sequence" swap "text" swap tuple 2 tuple 1 tuple 2"#,
        r#"dup "sequence" swap "word" swap tuple 2 tuple 1 tuple 2"#,
        // The state as a family clause component.
        r#"dup dup tuple 2 tuple 1 "family" swap tuple 2"#,
    ] {
        let source = format!(": bad {descriptor} stream.construct ; immediate bad");
        let error = assert_atomic_failure(&mut p, &source);
        assert!(!matches!(error, Error::Budget), "{descriptor}: {error:?}");
    }
}

#[test]
fn malformed_cid_text_and_non_code_bindings_are_rejected_atomically() {
    let mut p = seeded();
    stream::compile(&mut p, ": square dup * ;").unwrap();
    for body in [
        // 63 hex digits.
        r#"dup dup "square" stream.cid-of 0 63 text-slice stream.describe tuple-length stream.emit-literal"#,
        // Non-hex text of the right length.
        r#"dup "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz" stream.describe tuple-length stream.emit-literal"#,
        // Binding a name to an integer, a Boolean, or an unknown CID.
        r#""n" 7 stream.bind"#,
        r#""n" true stream.bind"#,
        r#""n" "0000000000000000000000000000000000000000000000000000000000000000" stream.bind"#,
        // Looking up a missing name, when the result is demanded.
        r#"dup "missing-word" stream.cid-of stream.emit-literal"#,
    ] {
        let source = format!(": bad {body} ; immediate bad");
        assert_atomic_failure(&mut p, &source);
    }
    assert!(p.lookup("n").is_none());
}

#[test]
fn a_failure_after_a_successful_construct_publishes_nothing() {
    let mut p = seeded();
    assert_atomic_failure(
        &mut p,
        &format!(
            r#": late {ANSWER} stream.construct
                   dup stream.last-cid "answer" swap stream.bind
                   dup "nope" stream.cid-of stream.emit-literal ; immediate late"#
        ),
    );
    assert!(p.lookup("answer").is_none());
}

#[test]
fn rebinding_an_immediate_name_clears_immediacy() {
    let mut p = seeded();
    stream::compile(&mut p, ": imm dup drop ; immediate").unwrap();
    assert!(p.is_immediate("imm"));
    let values = compile_run(
        &mut p,
        &format!(
            r#": rebind {ANSWER} stream.construct
                   dup stream.last-cid "imm" swap stream.bind ; immediate
               rebind
               : user imm ;
               user"#
        ),
    );
    assert_eq!(values, [Value::Int(42)]);
    assert!(!p.is_immediate("imm"));
}

#[test]
fn a_bound_numeric_name_never_shadows_number_recognition() {
    let mut p = seeded();
    let values = compile_run(
        &mut p,
        r#": nine 9 ;
           : bind42 dup "nine" stream.cid-of "42" swap stream.bind ; immediate
           bind42
           42 ' 42 call"#,
    );
    assert_eq!(values, [Value::Int(42), Value::Int(9)]);
}

#[test]
fn unbound_constructions_do_not_enter_images_of_other_entries() {
    // Images carry the dictionary, so define everything first; running the
    // constructor afterwards adds an unbound word but no name.
    let mut p = seeded();
    let root = stream::compile(
        &mut p,
        r#": kept 5 ;
           : orphan "sequence" "int" 77 tuple 2 tuple 1 tuple 2 stream.construct ; immediate
           kept"#,
    )
    .unwrap();
    let image = p.to_image(root).unwrap();
    let words = p.len();
    stream::compile(&mut p, "orphan").unwrap();
    assert!(
        p.len() > words,
        "the constructed word exists in the Program"
    );
    assert_eq!(p.to_image(root).unwrap(), image, "but not in the image");
}

#[test]
fn unused_reflection_results_are_never_demanded() {
    // March is lazy: a lookup or description whose result is dropped does not
    // run, so it cannot fail. Demanding the same result does fail.
    let mut p = seeded();
    assert_eq!(
        compile_run(
            &mut p,
            r#": ignore dup "missing-word" stream.cid-of drop ; immediate ignore 7"#
        ),
        [Value::Int(7)]
    );
    assert_atomic_failure(
        &mut p,
        r#": demand dup "missing-word" stream.cid-of stream.emit-literal ; immediate demand"#,
    );
}

#[test]
fn reflected_words_survive_an_image_round_trip_with_identical_cids() {
    let mut p = seeded();
    let entry = stream::compile(
        &mut p,
        &format!(
            r#": square dup * ;
               : make {ANSWER} stream.construct dup stream.last-cid "answer" swap stream.bind
                   dup "square" stream.cid-of "sq" swap stream.bind ; immediate
               make
               : both answer sq ;
               both"#
        ),
    )
    .unwrap();
    assert_eq!(run(&p, entry), [Value::Int(1764)]);
    let bytes = p.to_image(entry).unwrap();
    let (mut q, loaded) = Program::from_image(&bytes).unwrap();
    assert_eq!(run(&q, loaded), [Value::Int(1764)]);
    assert_eq!(p.cid(entry).unwrap(), q.cid(loaded).unwrap());
    assert_eq!(cid_text(&q, "answer"), cid_text(&p, "answer"));
    assert_eq!(q.lookup("sq"), q.lookup("square"));
    // The loaded image still runs the reflection words on new source.
    assert_eq!(compile_run(&mut q, "make answer"), [Value::Int(42)]);
}
