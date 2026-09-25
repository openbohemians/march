//! Independent adversarial checks of the conventional engine's sharing layers,
//! cycle rule, failure isolation, budgets, and the tail-loop derivation.
//!
//! Layers, as audited: L1 = construction-time hash-consing within one word;
//! L2 = one suspended instance computed once for every consumer; L3 =
//! independently constructed equal calls, NOT merged (characterized here,
//! not required).
use march_research::fast::{Context, Error, Executor, Literal, Op, Program, Value, source};

const BUDGET: usize = 100_000;

fn compile(text: &str) -> (Program, usize) {
    let mut program = Program::new();
    let word = source::compile(&mut program, text).unwrap();
    (program, word)
}

fn ints(values: Vec<Value>) -> Vec<i64> {
    values
        .into_iter()
        .map(|v| match v {
            Value::Int(n) => n,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect()
}

/// Integer results of source with no inputs, plus the primitive-op count.
fn evaluate(text: &str) -> (Vec<i64>, usize) {
    let (program, word) = compile(text);
    let mut executor = Executor::new(&program);
    let values = executor.run(word, &[], &Context::new(), BUDGET).unwrap();
    (ints(values), executor.stats().primitive_ops)
}

fn boolean(text: &str) -> bool {
    let (program, word) = compile(text);
    match Executor::new(&program)
        .run(word, &[], &Context::new(), BUDGET)
        .unwrap()
        .as_slice()
    {
        [Value::Bool(b)] => *b,
        other => panic!("expected one Boolean, got {other:?}"),
    }
}

fn failure(text: &str, budget: usize) -> Error {
    let (program, word) = compile(text);
    Executor::new(&program)
        .run(word, &[], &Context::new(), budget)
        .unwrap_err()
}

// ------------------------------------------------------------------ L1

#[test]
fn l1_repeated_identical_call_costs_nothing_extra_regardless_of_body_size() {
    // The seed's `identical_calls_share_one_demand`, restated for this engine.
    for size in [4, 64, 512] {
        let n = size as i64;
        let big = format!(": big {}; ", "1 + ".repeat(size));
        let (once, ops_once) = evaluate(&format!("{big}5 big"));
        let (twice, ops_twice) = evaluate(&format!("{big}5 big 5 big +"));
        let (different, ops_different) = evaluate(&format!("{big}5 big 6 big +"));
        assert_eq!(once, [5 + n]);
        assert_eq!(twice, [2 * (5 + n)]);
        assert_eq!(different, [11 + 2 * n]);
        assert_eq!(ops_once, size);
        assert_eq!(ops_twice, size + 1, "identical call must be merged");
        assert_eq!(ops_different, 2 * size + 1, "different argument recomputes");
    }
}

#[test]
fn l1_shuffles_are_erased_from_code_identity_but_arithmetic_is_not() {
    assert!(boolean("[ dup * ] [ dup * ] ="));
    assert!(boolean("[ dup * ] [ dup dup * swap drop ] ="));
    assert!(!boolean("[ dup * ] [ dup * 0 + ] ="));
    assert!(!boolean("[ dup * ] [ dup + ] ="));
    // The same holds for named words: equal bodies are one word, so equal
    // code values compare equal across names.
    assert!(boolean(": a dup * ; : b dup dup * swap drop ; ' a ' b ="));
}

#[test]
fn l1_equal_definitions_collapse_to_one_word_and_their_calls_merge() {
    // Two names, one body: the calls are the same op and are hash-consed.
    let (values, ops) = evaluate(": square dup * ; : a 7 square ; : b 7 square ; a b +");
    assert_eq!(values, [98]);
    assert_eq!(
        ops, 2,
        "a and b are one word; one call, one multiply, one add"
    );
    let (program, _) = compile(": square dup * ; : a 7 square ; : b 7 square ;");
    assert_eq!(program.lookup("a"), program.lookup("b"));
    // A different constant keeps them apart.
    let (values, ops) = evaluate(": square dup * ; : a 7 square ; : b 8 square ; a b +");
    assert_eq!(values, [113]);
    assert_eq!(ops, 3);
}

#[test]
fn l1_merging_never_crosses_a_context_read_or_an_argument() {
    // Two reads of the same context key are one op (pure under a fixed
    // context); two uses of one argument are one cell; but distinct
    // arguments with equal values are distinct.
    let (program, word) = compile(": twice ctx n ctx n + ; twice");
    let context = Context::from([("n".to_string(), Literal::Int(21))]);
    let mut executor = Executor::new(&program);
    assert_eq!(
        ints(executor.run(word, &[], &context, BUDGET).unwrap()),
        [42]
    );
    assert_eq!(executor.stats().primitive_ops, 1);

    let (program, word) = compile(": square dup * ; square swap square +");
    let mut executor = Executor::new(&program);
    let args = [Literal::Int(3), Literal::Int(3)];
    assert_eq!(
        ints(executor.run(word, &args, &Context::new(), BUDGET).unwrap()),
        [18]
    );
    assert_eq!(
        executor.stats().primitive_ops,
        3,
        "equal argument VALUES in different cells are not merged"
    );
}

// ------------------------------------------------------------------ L2

#[test]
fn l2_one_instance_serves_a_fan_of_consumers_across_call_boundaries() {
    let (values, ops) = evaluate(": square dup * ; 3 4 * square dup dup + +");
    assert_eq!(values, [432]);
    assert_eq!(ops, 4, "one 3*4, one square, two additions");
    // A shared argument demanded inside two different callee instances is
    // still one cell: the caller's.
    let (values, ops) = evaluate(": inc 1 + ; : dec 1 - ; 6 7 * dup inc swap dec +");
    assert_eq!(values, [84]);
    assert_eq!(
        ops, 4,
        "one 6*7, one increment, one decrement, one addition"
    );
}

#[test]
fn l2_pair_fields_are_lazy_shared_and_fail_independently() {
    let (program, word) = compile(": p 9223372036854775807 1 + 7 pair ; p");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let Value::Pair(bad, good) = executor.force(handle).unwrap() else {
        panic!("pair")
    };
    assert_eq!(executor.stats().primitive_ops, 0);
    assert_eq!(executor.force(good), Ok(Value::Int(7)));
    assert_eq!(executor.force(bad), Err(Error::Overflow));
    assert_eq!(executor.force(good), Ok(Value::Int(7)));
    assert_eq!(executor.force(bad), Err(Error::Overflow));
    assert_eq!(
        executor.stats().primitive_ops,
        1,
        "the failure is memoized, not retried"
    );
    assert_eq!(executor.content_id(handle), Err(Error::Overflow));
    assert_eq!(executor.force(good), Ok(Value::Int(7)));
}

#[test]
fn l2_multiple_outputs_of_one_call_share_work_and_isolate_failure() {
    let (program, word) = compile(": both dup * 9223372036854775807 1 + ; 7 both");
    let mut executor = Executor::new(&program);
    let outputs = executor.start(word, &[], &Context::new(), BUDGET).unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(executor.force(outputs[0]), Ok(Value::Int(49)));
    assert_eq!(executor.force(outputs[1]), Err(Error::Overflow));
    assert_eq!(executor.force(outputs[0]), Ok(Value::Int(49)));
    assert_eq!(executor.stats().primitive_ops, 2);
}

// ------------------------------------------------------------------ L3

#[test]
fn l3_independent_equal_calls_from_different_words_are_recomputed() {
    // Characterization, not a requirement: the reference merges these by
    // canonical identity after substitution; this engine does not.
    let (values, ops) = evaluate(": big 1 + 1 + 1 + 1 + ; : f 5 big ; : g 5 big 0 + ; f g +");
    assert_eq!(values, [18]);
    assert_eq!(ops, 4 + 4 + 1 + 1, "big runs once per distinct call op");
}

// ---------------------------------------------------------------- cycle

#[test]
fn cycle_same_argument_cell_is_a_cycle_but_equal_recomputed_value_is_fuel() {
    assert_eq!(failure(": loop recur 1 1 ; 7 loop", BUDGET), Error::Cycle);
    assert_eq!(
        failure(": loop 0 + recur 1 1 ; 7 loop", 2_000),
        Error::Budget
    );
    // Through a stack shuffle the alias is still the same cell.
    assert_eq!(
        failure(": loop dup drop recur 1 1 ; 7 loop", BUDGET),
        Error::Cycle
    );
    // A cycle is an error outcome, never a cached value: a fresh run works.
    let (program, word) = compile(": choose 0 = 7 3 select ; choose");
    let mut executor = Executor::new(&program);
    assert_eq!(
        ints(
            executor
                .run(word, &[Literal::Int(0)], &Context::new(), BUDGET)
                .unwrap()
        ),
        [7]
    );
}

#[test]
fn cycle_is_not_confused_with_deep_recursion_and_retention_is_linear() {
    // The generic evaluator, reached through a dynamic application, must not
    // report a cycle for a genuinely deep descent.
    let (program, word) = compile(&tail_source("' down apply 1 1"));
    let mut peaks = Vec::new();
    for depth in [100i64, 1_000, 4_000] {
        let mut executor = Executor::new(&program);
        assert_eq!(
            ints(
                executor
                    .run(word, &[Literal::Int(depth)], &Context::new(), BUDGET)
                    .unwrap()
            ),
            [0]
        );
        peaks.push(executor.stats().peak_frames);
    }
    assert!(
        peaks[1] > peaks[0] && peaks[2] > peaks[1],
        "no TCO today: {peaks:?}"
    );
    assert!(
        peaks[2] < 60 * 4_000,
        "retention should stay linear: {peaks:?}"
    );
}

// --------------------------------------------------------------- budget

#[test]
fn budget_abort_is_sticky_for_every_handle_and_a_fresh_run_recovers() {
    let (program, word) = compile(": from dup 1 + recur 1 1 pair ; 0 from");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), 40).unwrap()[0];
    assert_eq!(executor.content_id(handle), Err(Error::Budget));
    assert_eq!(executor.force(handle), Err(Error::Budget));
    let handle = executor.start(word, &[], &Context::new(), 40).unwrap()[0];
    let Value::Pair(first, rest) = executor.force(handle).unwrap() else {
        panic!("pair")
    };
    assert_eq!(executor.force(first), Ok(Value::Int(0)));
    let Value::Pair(second, _) = executor.force(rest).unwrap() else {
        panic!("pair")
    };
    assert_eq!(executor.force(second), Ok(Value::Int(1)));
}

#[test]
fn budget_is_path_dependent_between_the_scalar_plan_and_the_generic_engine() {
    // Same word, same answer; the derived plan and the task engine charge
    // different amounts. Recorded so nobody reads fuel as a cost model.
    let (program, direct) = compile(": w 1 + 2 * 3 - ; w");
    let (dynamic_program, dynamic) = compile(": w 1 + 2 * 3 - ; ' w apply 1 1");
    assert!(program.is_fast(direct).unwrap());
    assert!(!dynamic_program.is_fast(dynamic).unwrap());
    let minimal = |program: &Program, word: usize| {
        (1..200)
            .find(|&budget| {
                Executor::new(program)
                    .run(word, &[Literal::Int(5)], &Context::new(), budget)
                    .is_ok()
            })
            .unwrap()
    };
    let fast = minimal(&program, direct);
    let generic = minimal(&dynamic_program, dynamic);
    for (program, word) in [(&program, direct), (&dynamic_program, dynamic)] {
        assert_eq!(
            ints(
                Executor::new(program)
                    .run(word, &[Literal::Int(5)], &Context::new(), BUDGET)
                    .unwrap()
            ),
            [9]
        );
    }
    assert!(fast < generic, "plan {fast} vs generic {generic}");
}

// -------------------------------------------------------- application

#[test]
fn application_arity_is_checked_only_when_demanded() {
    assert_eq!(
        failure("1 2 [ dup * ] apply 2 1", BUDGET),
        Error::Arity {
            expected: 1,
            actual: 2
        }
    );
    assert_eq!(
        failure("3 [ dup * ] apply 1 2", BUDGET),
        Error::OutputArity {
            expected: 2,
            actual: 1
        }
    );
    assert_eq!(
        failure("3 7 apply 1 1", BUDGET),
        Error::Type("application needs closed quotation")
    );
    // Mis-arity or non-code applications that are never demanded cost nothing.
    for text in [
        "true 7 3 [ dup * ] apply 1 2 drop select",
        "true 7 3 [ dup * ] apply 1 1 select",
        "true 7 3 9 apply 1 1 select",
    ] {
        let (values, ops) = evaluate(text);
        assert_eq!(values, [7], "{text}");
        assert_eq!(ops, 0, "{text}");
    }
}

#[test]
fn a_raw_call_bundle_as_output_is_a_type_error_not_a_panic() {
    let mut program = Program::new();
    let seven = program
        .add_word(0, vec![Op::Const(Literal::Int(7))], vec![0])
        .unwrap();
    let bundle = program
        .add_word(
            0,
            vec![Op::Call {
                word: seven,
                arguments: vec![],
            }],
            vec![0],
        )
        .unwrap();
    assert_eq!(
        Executor::new(&program).run(bundle, &[], &Context::new(), BUDGET),
        Err(Error::Type("internal call bundle must be projected"))
    );
}

#[test]
fn a_helper_word_called_from_a_clause_body_recurs_into_itself_not_the_family() {
    // `helper` recurs into itself with an unchanged argument: Cycle. It must
    // not silently re-enter the family with a different meaning.
    let text = ": zero 0 = ; : always drop true ; : base dup drop ; : helper recur 1 1 ; \
                : step helper ; family down 1 1 zero base always step ; 3 down";
    assert_eq!(failure(text, BUDGET), Error::Cycle);
}

// ------------------------------------------------------------ tail loop

fn tail_source(entry: &str) -> String {
    format!(
        ": zero 0 = ; : always drop true ; : base dup drop ; : step 1 - recur 1 1 ; \
         family down 1 1 zero base always step ; {entry}"
    )
}

#[test]
fn tail_loop_and_generic_engine_agree_on_values_errors_and_cycles() {
    let (program, direct) = compile(&tail_source("down"));
    let (generic_program, generic) = compile(&tail_source("' down apply 1 1"));
    assert!(program.is_tail_loop(direct).unwrap());
    assert!(!generic_program.is_tail_loop(generic).unwrap());
    for arg in [0, 1, 5, 1_000, i64::MIN + 3] {
        let expected = if arg >= 0 {
            Ok(vec![Value::Int(0)])
        } else {
            Err(Error::Overflow)
        };
        assert_eq!(
            Executor::new(&program).run(direct, &[Literal::Int(arg)], &Context::new(), BUDGET),
            expected,
            "tail loop, argument {arg}"
        );
        assert_eq!(
            Executor::new(&generic_program).run(
                generic,
                &[Literal::Int(arg)],
                &Context::new(),
                BUDGET
            ),
            expected,
            "generic engine, argument {arg}"
        );
    }
    // Unchanged-argument recursion cycles under both derivations.
    let text = ": zero 0 = ; : always drop true ; : base dup drop ; : same recur 1 1 ; \
                family stuck 1 1 zero base always same ;";
    assert_eq!(failure(&format!("{text} 3 stuck"), BUDGET), Error::Cycle);
    assert_eq!(
        failure(&format!("{text} 3 ' stuck apply 1 1"), BUDGET),
        Error::Cycle
    );
    // Neither derivation runs the unselected recursive argument: with the
    // base clause selected first, an overflowing step argument is never built.
    let text = ": zero 0 = ; : always drop true ; : base dup drop ; \
                : step 9223372036854775807 + recur 1 1 ; \
                family safe 1 1 zero base always step ;";
    for entry in ["0 safe", "0 ' safe apply 1 1"] {
        let (values, ops) = evaluate(&format!("{text} {entry}"));
        assert_eq!(values, [0]);
        assert!(ops <= 1, "{entry}: {ops} primitive ops");
    }
}

#[test]
fn tail_loop_workspace_is_constant_while_the_generic_engine_retains_frames() {
    let (program, direct) = compile(&tail_source("down"));
    let (generic_program, generic) = compile(&tail_source("' down apply 1 1"));
    let mut small = Executor::new(&program);
    small
        .run(direct, &[Literal::Int(10)], &Context::new(), BUDGET)
        .unwrap();
    let mut large = Executor::new(&program);
    large
        .run(direct, &[Literal::Int(10_000)], &Context::new(), BUDGET)
        .unwrap();
    assert_eq!(small.stats().peak_frames, large.stats().peak_frames);
    assert_eq!(small.stats().peak_cells, large.stats().peak_cells);
    assert_eq!(large.stats().tail_iterations, 10_001);
    let mut retained = Executor::new(&generic_program);
    retained
        .run(generic, &[Literal::Int(1_000)], &Context::new(), BUDGET)
        .unwrap();
    assert!(retained.stats().peak_frames > 1_000);
}
