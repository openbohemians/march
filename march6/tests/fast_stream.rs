use march_research::fast::{Context, Error, Executor, Literal, Op, Program, Value, stream};

fn run(p: &Program, w: usize, args: &[Literal]) -> Vec<Value> {
    Executor::new(p)
        .run(w, args, &Context::new(), 100_000)
        .unwrap()
}
fn evaluate(text: &str) -> Vec<Value> {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, text).unwrap();
    run(&p, w, &[])
}

#[test]
fn number_recognition_precedes_dictionary_including_immediate_numeric_names() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(
        &mut p,
        ": 42 999 ; : -3 777 ; : +12 888 ; 42 -3 +12 ' 42 call",
    )
    .unwrap();
    assert_eq!(
        run(&p, w, &[]),
        [
            Value::Int(42),
            Value::Int(-3),
            Value::Int(12),
            Value::Int(999)
        ]
    );
    assert_eq!(run(&p, p.lookup("42").unwrap(), &[]), [Value::Int(999)]);
    // If lookup came first, this would swallow the trailing 7 as a comment.
    assert_eq!(
        evaluate(": 42 stream.skip-line ; immediate 42 7"),
        [Value::Int(42), Value::Int(7)]
    );
}

#[test]
fn overflow_is_reported_before_lookup_not_reinterpreted_as_a_dictionary_word() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": 9223372036854775808 7 ;").unwrap();
    assert_eq!(
        stream::compile(&mut p, "9223372036854775808"),
        Err(Error::Overflow)
    );
    assert_eq!(
        evaluate("-9223372036854775808 9223372036854775807"),
        [Value::Int(i64::MIN), Value::Int(i64::MAX)]
    );
}

#[test]
fn user_defining_words_consume_input_and_compute_with_the_normal_vm() {
    assert_eq!(
        evaluate(include_str!("../examples/fast/compiler.march")),
        [Value::Int(49), Value::Int(42), Value::Int(42)]
    );
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": answer: stream.word stream.begin 20 22 + stream.emit-literal stream.end ; immediate answer: answer : literal-answer 42 ;").unwrap();
    assert_eq!(
        p.cid(p.lookup("answer").unwrap()),
        p.cid(p.lookup("literal-answer").unwrap())
    );
    let mut e = Executor::new(&p);
    assert_eq!(
        e.run(p.lookup("answer").unwrap(), &[], &Context::new(), 1000)
            .unwrap(),
        [Value::Int(42)]
    );
    assert_eq!(e.stats().primitive_ops, 0);
}

#[test]
fn punctuation_comments_and_runtime_primitives_are_dictionary_words() {
    assert_eq!(evaluate(": : 7 ; :"), [Value::Int(7)]);
    assert_eq!(evaluate(": -- 5 ; --"), [Value::Int(5)]);
    assert_eq!(evaluate(": dup drop 77 ; 8 dup"), [Value::Int(77)]);
    // Raw characters are not lexed ahead of the comment word.
    assert_eq!(evaluate("7 -- ) ] [ : ; bad syntax\n8 +"), [Value::Int(15)]);
    assert_eq!(evaluate("7 ( [ ] : ; ignored) 8 +"), [Value::Int(15)]);
    assert_eq!(
        evaluate(": skip! 33 stream.read-until ; immediate 7 skip! [ : invalid! 8 +"),
        [Value::Int(15)]
    );
}

#[test]
fn word_is_whitespace_delimited_not_punctuation_lexed() {
    for bad in [
        ":square dup * ;",
        "7[ dup * ] call",
        ": square dup *;",
        "--comment",
    ] {
        assert!(
            stream::compile(&mut stream::seed().unwrap(), bad).is_err(),
            "{bad}"
        );
    }
    assert_eq!(evaluate(": a:b 9 ; a:b"), [Value::Int(9)]);
    assert_eq!(evaluate(": λ 7 ;\n\tλ\u{2003}3 +"), [Value::Int(10)]);
}

#[test]
fn dictionary_entries_can_emit_compilation_behavior_inside_another_word() {
    assert_eq!(
        evaluate(": forty-two 40 2 + stream.emit-literal ; immediate : answer forty-two ; answer"),
        [Value::Int(42)]
    );
    assert_eq!(
        evaluate(
            ": next-number stream.word stream.number dup stream.number-value stream.emit-literal ; immediate : answer next-number 42 ; answer"
        ),
        [Value::Int(42)]
    );
}

#[test]
fn compiler_words_can_select_behavior_using_explicit_compilation_context() {
    let source = ": compiling? drop ctx compiling ; : always drop true ;
        : in-definition 7 stream.emit-literal ; : outside 8 stream.emit-literal ;
        family mode-value 1 1 compiling? in-definition always outside ; immediate
        mode-value : inner mode-value ; inner";
    assert_eq!(evaluate(source), [Value::Int(8), Value::Int(7)]);
}

#[test]
fn lazy_quotations_context_and_recursion_keep_runtime_semantics() {
    for (text, expected) in [
        ("ctx missing drop 7", 7),
        ("true 7 ctx missing select", 7),
        ("7 [ dup * ] call", 49),
        ("7 true [ dup * ] [ 1 + ] select apply 1 1", 49),
        (": loop recur 0 1 ; true 7 loop select", 7),
        (": from dup 1 + recur 1 1 pair ; 10 from second first", 11),
    ] {
        assert_eq!(evaluate(text), [Value::Int(expected)], "{text}");
    }
    let fib = ": zero 0 eq? ; : one 1 eq? ; : always drop true ; : base dup drop ;
        : step dup 1 - recur 1 1 swap 2 - recur 1 1 + ;
        family fib 1 1 zero base one base always step ; 10 fib";
    assert_eq!(evaluate(fib), [Value::Int(55)]);
}

#[test]
fn word_interfaces_and_compiled_dependencies_are_preserved() {
    let mut p = stream::seed().unwrap();
    let w = stream::compile(&mut p, ": subtract - ; subtract").unwrap();
    assert_eq!(p.signature(w), Ok((2, 1)));
    assert_eq!(
        run(&p, w, &[Literal::Int(10), Literal::Int(3)]),
        [Value::Int(7)]
    );
    let w = stream::compile(&mut p, ": f 1 ; : saved f ; : f 2 ; saved f").unwrap();
    assert_eq!(run(&p, w, &[]), [Value::Int(1), Value::Int(2)]);
}

#[test]
fn saved_seed_interprets_further_source_without_reinitializing_native_syntax() {
    let mut p = stream::seed().unwrap();
    let e = stream::compile(
        &mut p,
        ": define stream.word stream.begin ; immediate define square dup * ; 7 square",
    )
    .unwrap();
    let bytes = p.to_image(e).unwrap();
    assert_eq!(&bytes[..8], b"MARCHF05");
    let (mut q, qe) = Program::from_image(&bytes).unwrap();
    assert_eq!(q.to_image(qe).unwrap(), bytes);
    assert_eq!(run(&q, qe, &[]), [Value::Int(49)]);
    let w = stream::compile(&mut q, "define cube dup dup * * ; 3 cube").unwrap();
    assert_eq!(run(&q, w, &[]), [Value::Int(27)]);
    assert!(q.is_immediate("define"));
    let mut r = stream::seed().unwrap();
    let re = stream::compile(
        &mut r,
        ": define stream.word stream.begin ; immediate define square dup * ; 7 square",
    )
    .unwrap();
    assert_eq!(r.to_image(re).unwrap(), bytes);
}

#[test]
fn malformed_source_and_failed_execution_leave_the_original_dictionary_unchanged() {
    let mut p = stream::seed().unwrap();
    let e = stream::compile(&mut p, ": keep 13 ;").unwrap();
    let before = p.to_image(e).unwrap();
    for source in [
        ": temporary 7 ; unknown",
        ": unfinished 7",
        ";",
        "]",
        "[ 7 ;",
        ": nested : inner 7 ; ;",
        "( no close",
        "' absent",
        "immediate",
        ": wrong 7 ; immediate",
        ": not-state drop 7 ; immediate not-state",
        "family bad 1 1 missing missing ;",
        "recur -1 1",
        "apply 1 -1",
    ] {
        assert!(stream::compile(&mut p, source).is_err(), "{source}");
        assert_eq!(p.to_image(e).unwrap(), before, "{source}");
    }
}

#[test]
fn compiler_fuel_states_nesting_and_nested_execution_are_bounded() {
    let p = stream::seed().unwrap();
    let nested = stream::Limits {
        kernel_depth: 8,
        ..Default::default()
    };
    assert_eq!(
        stream::compile_with_limits(
            &mut p.clone(),
            ": again stream.execute ; immediate again",
            &nested
        ),
        Err(Error::StorageLimit)
    );
    for limits in [
        stream::Limits {
            fuel: 0,
            ..Default::default()
        },
        stream::Limits {
            states: 1,
            ..Default::default()
        },
        stream::Limits {
            cells: 1,
            ..Default::default()
        },
        stream::Limits {
            source_bytes: 1,
            ..Default::default()
        },
        stream::Limits {
            code_words: 1,
            ..Default::default()
        },
        stream::Limits {
            kernel_depth: 1,
            ..Default::default()
        },
    ] {
        assert!(stream::compile_with_limits(&mut p.clone(), ": f 7 ; f", &limits).is_err());
    }
    let limits = stream::Limits {
        fuel: 1000,
        ..Default::default()
    };
    assert!(
        stream::compile_with_limits(
            &mut p.clone(),
            ": loop dup pair recur 1 1 ; immediate loop",
            &limits
        )
        .is_err()
    );
    let limits = stream::Limits {
        nesting: 2,
        ..Default::default()
    };
    assert!(stream::compile_with_limits(&mut p.clone(), "[ [ [ 7 ] ] ]", &limits).is_err());
}

#[test]
fn compiler_state_cannot_be_forged_by_an_integer_or_used_without_a_session() {
    // Two reads from the same explicit state see the same next word. A hidden
    // mutable input cursor would read 7 then 9 and fail this assertion.
    assert_eq!(
        evaluate(
            ": twice-read dup stream.word stream.number stream.number-value
        swap stream.word stream.number dup stream.number-value stream.emit-literal
        swap stream.emit-literal ; immediate twice-read 7 9"
        ),
        [Value::Int(7), Value::Int(7), Value::Int(9)]
    );
    let p = stream::seed().unwrap();
    assert!(matches!(
        Executor::new(&p).run(
            p.lookup("stream.word").unwrap(),
            &[Literal::Int(0)],
            &Context::new(),
            1000
        ),
        Err(Error::Compiler(_))
    ));
    assert!(
        stream::compile(
            &mut p.clone(),
            ": forged drop 0 stream.word ; immediate forged"
        )
        .is_err()
    );
    let mut p = Program::new();
    assert!(
        p.add_word(
            0,
            vec![Op::Kernel {
                primitive: stream::Primitive::Word,
                arguments: vec![]
            }],
            vec![0]
        )
        .is_err()
    );
}

#[test]
fn host_reader_images_use_current_format_and_loading_executes_no_compiler_code() {
    let mut old = Program::new();
    let w = march_research::fast::source::compile(&mut old, "7").unwrap();
    let bytes = old.to_image(w).unwrap();
    assert_eq!(&bytes[..8], b"MARCHF05");
    assert_eq!(
        {
            let (p, e) = Program::from_image(&bytes).unwrap();
            p.to_image(e).unwrap()
        },
        bytes
    );
    let mut p = stream::seed().unwrap();
    let e = stream::compile(&mut p, ": stream.interpret recur 1 1 ; 7").unwrap();
    let (mut q, qe) = Program::from_image(&p.to_image(e).unwrap()).unwrap();
    assert_eq!(run(&q, qe, &[]), [Value::Int(7)]);
    assert!(stream::compile(&mut q, "7").is_err());
}

#[test]
fn corrupted_stream_images_and_immediate_metadata_are_rejected() {
    let mut p = stream::seed().unwrap();
    let e = stream::compile(&mut p, "7").unwrap();
    let bytes = p.to_image(e).unwrap();
    for n in [0, 7, 8, 16, bytes.len() / 2, bytes.len() - 1] {
        assert!(Program::from_image(&bytes[..n]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Program::from_image(&trailing).is_err());
    let mut wrong_version = bytes;
    wrong_version[..8].copy_from_slice(b"MARCHF02");
    assert!(Program::from_image(&wrong_version).is_err());
    let wrong = p.lookup("+").unwrap();
    p.bind("bad-immediate", wrong).unwrap();
    assert!(p.mark_immediate("bad-immediate").is_err());
}

#[test]
fn cli_extends_an_image_through_the_saved_march_interpreter() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let path = std::env::temp_dir().join(format!(
        "march-stream-{}-{}.mimg",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut p = stream::seed().unwrap();
    let e = stream::compile(&mut p, ": define stream.word stream.begin ; immediate").unwrap();
    fs::write(&path, p.to_image(e).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_march-fast"))
        .arg("--load-image")
        .arg(&path)
        .args(["--extend", "define square dup * ; 9 square"])
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "[Int(81)]");
}
