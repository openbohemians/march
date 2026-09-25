use march_research::fast::{Context, Executor, Literal, Program, Value, source};

fn evaluate(text: &str, args: &[Literal], context: &Context) -> Vec<Value> {
    let mut program = Program::new();
    let word = source::compile(&mut program, text).unwrap();
    Executor::new(&program)
        .run(word, args, context, 10_000)
        .unwrap()
}

fn integers(values: Vec<Value>) -> Vec<i64> {
    values
        .into_iter()
        .map(|v| match v {
            Value::Int(n) => n,
            _ => panic!("expected integer, got {v:?}"),
        })
        .collect()
}

#[test]
fn named_words_and_closed_quotations() {
    for text in [
        ": square dup * ; 7 square",
        "7 [ dup * ] call",
        ": square dup * ; 7 quote square call",
        ": square dup * ; 7 ' square call",
    ] {
        assert_eq!(integers(evaluate(text, &[], &Context::new())), [49]);
    }
}

#[test]
fn inferred_inputs_and_outputs_are_bottom_to_top() {
    let args = [Literal::Int(10), Literal::Int(3)];
    for text in ["-", ": subtract - ; subtract", "[ - ] call"] {
        assert_eq!(integers(evaluate(text, &args, &Context::new())), [7]);
    }
    assert_eq!(integers(evaluate("swap", &args, &Context::new())), [3, 10]);
    assert_eq!(
        integers(evaluate("over +", &args, &Context::new())),
        [10, 13]
    );
    assert_eq!(
        integers(evaluate(": both dup 1 + ; 8 both", &[], &Context::new())),
        [8, 9]
    );
}

#[test]
fn dropped_and_unselected_computations_are_not_forced() {
    for text in [
        "ctx missing drop 7",
        "true 7 ctx missing select",
        "false ctx missing 7 select",
        "7 ctx missing pair first",
        ": ignore drop 7 ; ctx missing ignore",
    ] {
        assert_eq!(integers(evaluate(text, &[], &Context::new())), [7]);
    }
}

#[test]
fn context_and_ordered_family_dispatch() {
    let text = ": enabled ctx enabled ; : always true ; : yes 7 ; : no 8 ; family choice 0 1 enabled yes always no ; choice";
    for (enabled, result) in [(true, 7), (false, 8)] {
        let context = Context::from([("enabled".to_string(), Literal::Bool(enabled))]);
        assert_eq!(integers(evaluate(text, &[], &context)), [result]);
    }
}

#[test]
fn comments_and_adjacent_delimiters() {
    assert_eq!(
        integers(evaluate(
            "( nested ( comment ) ) :square dup *; \\ comment\n7[ square ]call",
            &[],
            &Context::new()
        )),
        [49]
    );
}

#[test]
fn malformed_source_is_rejected() {
    for text in [
        "[ 1",
        ": missing 1",
        "( missing",
        "]",
        "unknown",
        "call",
        "family f 0 1 absent absent ;",
        ": true 7 ;",
        ": 42 7 ;",
        "family false 0 1 ;",
        "recur 18446744073709551615 1",
        "recur 0 18446744073709551615",
        "7 ctx missing [ dup ] dup select call",
    ] {
        assert!(
            source::compile(&mut Program::new(), text).is_err(),
            "{text}"
        );
    }
}

#[test]
fn quotations_do_not_capture_surrounding_stack() {
    let mut program = Program::new();
    let word = source::compile(&mut program, "7 [ + ] call").unwrap();
    assert_eq!(program.signature(word).unwrap(), (1, 1));
    let result = Executor::new(&program)
        .run(word, &[Literal::Int(3)], &Context::new(), 1000)
        .unwrap();
    assert_eq!(integers(result), [10]);
}

#[test]
fn recursive_context_family() {
    let text = "
        : zero 0 = ;
        : one 1 = ;
        : always drop true ;
        : base dup drop ;
        : step dup 1 - recur 1 1 swap 2 - recur 1 1 + ;
        family fib 1 1 zero base one base always step ;
        10 fib
    ";
    assert_eq!(integers(evaluate(text, &[], &Context::new())), [55]);
}

#[test]
fn same_word_equal_calls_are_hash_consed_at_compile_time() {
    for (text, expected_ops) in [
        (": square dup * ; 7 square dup +", 2),
        (": square dup * ; 7 square 7 square +", 2),
    ] {
        let mut program = Program::new();
        let word = source::compile(&mut program, text).unwrap();
        let mut executor = Executor::new(&program);
        assert_eq!(
            integers(executor.run(word, &[], &Context::new(), 1000).unwrap()),
            [98]
        );
        assert_eq!(executor.stats().primitive_ops, expected_ops);
    }
}

#[test]
fn multiple_outputs_share_a_call_but_unused_output_remains_lazy() {
    let mut program = Program::new();
    let word = source::compile(&mut program, ": both dup * dup ; 7 both +").unwrap();
    let mut executor = Executor::new(&program);
    assert_eq!(
        integers(executor.run(word, &[], &Context::new(), 1000).unwrap()),
        [98]
    );
    assert_eq!(executor.stats().primitive_ops, 2);
    assert_eq!(
        integers(evaluate(
            ": both 7 ctx absent ; both drop",
            &[],
            &Context::new()
        )),
        [7]
    );
}

#[test]
fn ignored_recursive_call_is_lazy_and_demanded_call_is_bounded() {
    let mut program = Program::new();
    let word = source::compile(&mut program, ": loop recur 0 1 ; true 7 loop select").unwrap();
    assert_eq!(
        integers(
            Executor::new(&program)
                .run(word, &[], &Context::new(), 1000)
                .unwrap()
        ),
        [7]
    );
    let looping = program.lookup("loop").unwrap();
    assert_eq!(
        Executor::new(&program).run(looping, &[], &Context::new(), 100),
        Err(march_research::fast::Error::Cycle)
    );
    let growing = source::compile(&mut program, ": grow 1 + recur 1 1 ; 0 grow").unwrap();
    assert_eq!(
        Executor::new(&program).run(growing, &[], &Context::new(), 100),
        Err(march_research::fast::Error::Budget)
    );
}

#[test]
fn source_size_and_nesting_are_bounded() {
    let oversized = " ".repeat(4 * 1024 * 1024 + 1);
    assert!(
        source::compile(&mut Program::new(), &oversized)
            .unwrap_err()
            .to_string()
            .contains("4 MiB")
    );
    let too_deep = format!("{}7{}", "[".repeat(129), "]".repeat(129));
    assert!(
        source::compile(&mut Program::new(), &too_deep)
            .unwrap_err()
            .to_string()
            .contains("nesting")
    );
    let allowed = format!("{}7{}", "[".repeat(128), "]".repeat(128));
    assert!(source::compile(&mut Program::new(), &allowed).is_ok());
}

#[test]
fn dictionary_rebinding_does_not_change_previously_compiled_calls() {
    assert_eq!(
        integers(evaluate(
            ": foo 1 ; : bar foo ; : foo 2 ; bar foo",
            &[],
            &Context::new()
        )),
        [1, 2]
    );
}

#[test]
fn a_fresh_context_cannot_reuse_an_old_invocations_memo() {
    let mut program = Program::new();
    let word = source::compile(&mut program, ": read ctx n ; read dup +").unwrap();
    let mut executor = Executor::new(&program);
    for n in [3, 7, 2] {
        let context = Context::from([("n".into(), Literal::Int(n))]);
        assert_eq!(
            integers(executor.run(word, &[], &context, 1000).unwrap()),
            [n * 2]
        );
    }
}

#[test]
fn family_ignores_later_guards_and_unchosen_bodies() {
    let text = ": yes drop true ; : unknown drop ctx absent ; : good drop 7 ; : bad drop ctx absent ; family choose 1 1 yes good unknown bad ; ctx missing choose";
    assert_eq!(integers(evaluate(text, &[], &Context::new())), [7]);
}

#[test]
fn dynamic_selected_quotes_and_lazy_arguments() {
    for (text, expected) in [
        ("7 true [ dup * ] [ 1 + ] select apply 1 1", 49),
        ("7 false [ dup * ] [ 1 + ] select apply 1 1", 8),
        ("ctx missing [ drop 7 ] apply 1 1", 7),
        ("[ 7 ctx missing ] apply 0 2 drop", 7),
    ] {
        assert_eq!(integers(evaluate(text, &[], &Context::new())), [expected]);
    }
}

#[test]
fn dynamic_quote_condition_and_signature_are_checked() {
    use march_research::fast::Error;
    let mut program = Program::new();
    let word =
        source::compile(&mut program, "7 ctx missing [ dup * ] dup select apply 1 1").unwrap();
    assert_eq!(
        Executor::new(&program).run(word, &[], &Context::new(), 1000),
        Err(Error::MissingContext("missing".into()))
    );
    for text in ["7 [ 8 ] apply 1 1", "[ 8 ] apply 0 2", "3 apply 0 1"] {
        let word = source::compile(&mut program, text).unwrap();
        assert!(
            Executor::new(&program)
                .run(word, &[], &Context::new(), 1000)
                .is_err()
        );
    }
}

#[test]
fn quotation_passed_as_context_value_or_explicit_argument() {
    let mut program = Program::new();
    source::compile(&mut program, ": square dup * ;").unwrap();
    let square = program.lookup("square").unwrap();
    let context_word = source::compile(&mut program, "7 ctx operation apply 1 1").unwrap();
    let context = Context::from([("operation".into(), Literal::Quote(square))]);
    assert_eq!(
        integers(
            Executor::new(&program)
                .run(context_word, &[], &context, 1000)
                .unwrap()
        ),
        [49]
    );
    let argument_word = source::compile(&mut program, "apply 1 1").unwrap();
    assert_eq!(
        integers(
            Executor::new(&program)
                .run(
                    argument_word,
                    &[Literal::Int(7), Literal::Quote(square)],
                    &Context::new(),
                    1000
                )
                .unwrap()
        ),
        [49]
    );
}
