use march_research::{
    Atom, Bindings, Cid, Image, Node, ReduceError, Reducer, Store,
    seed::{Seed, Syntax, resume},
};

const WORK: usize = 20_000_000;

fn field(s: &Store, record: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = s.get(record) else {
        panic!("expected record: {}", s.format(record))
    };
    fields
        .iter()
        .find(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .1
}
fn value(s: &Store, state: Cid) -> Cid {
    let stack = field(s, state, "stack");
    let Some(Node::Pair(cell, _)) = s.get(stack) else {
        panic!("no stack result")
    };
    field(s, *cell, "value")
}
fn assert_ok(s: &Store, state: Cid) {
    let error = field(s, state, "error");
    assert_eq!(
        s.get(error),
        Some(&Node::Const(Atom::Unit)),
        "{} at {}",
        s.format(error),
        s.format(field(s, state, "token"))
    );
}
fn setup(source: &str, syntax: Syntax) -> (Store, Seed, Cid) {
    let mut s = Store::new();
    let seed = Seed::build(&mut s, syntax);
    let source = s.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut s, source);
    (s, seed, state)
}
fn evaluate(source: &str, syntax: Syntax) -> (Store, Cid) {
    let (mut s, seed, state) = setup(source, syntax);
    let result = resume(&mut s, seed.runner, state, 10_000, WORK)
        .unwrap()
        .root;
    assert_ok(&s, result);
    (s, result)
}
fn named(s: &Store, state: Cid, name: &str) -> Cid {
    field(s, field(s, field(s, state, "dictionary"), name), "value")
}

#[test]
fn name_first_square_has_the_hand_built_cid_and_evaluates_to_49() {
    let (mut s, state) = evaluate("square : ( dup * ) ; 7 square", Syntax::NameFirst);
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(49))));
    let p = s.intern(Node::Param(0));
    let body = s.intern(Node::Mul(p, p));
    let expected = s.intern(Node::Quote { params: 1, body });
    assert_eq!(named(&s, state, "square"), expected);
}

#[test]
fn alternate_forth_seed_produces_the_same_square() {
    let (a, ar) = evaluate("square : ( dup * ) ; 7 square", Syntax::NameFirst);
    let (b, br) = evaluate(": square dup * ; 7 square", Syntax::Forth);
    assert_eq!(named(&a, ar, "square"), named(&b, br, "square"));
    assert_eq!(value(&a, ar), value(&b, br));
}

#[test]
fn construction_constants_and_word_composition_work() {
    let (s, state) = evaluate(
        "answer : 6 7 * ; square : ( dup * ) ; twice : ( 2 * ) ; answer square twice",
        Syntax::NameFirst,
    );
    assert_eq!(
        s.get(named(&s, state, "answer")),
        Some(&Node::Const(Atom::Int(42)))
    );
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(3528))));
    let (s, state) = evaluate(
        "square : ( dup * ) ; fourth : ( square square ) ; 3 fourth",
        Syntax::NameFirst,
    );
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(81))));
}

#[test]
fn source_can_alias_parsing_words_and_use_them_in_the_same_file() {
    let (s, state) = evaluate(
        "end : quote ; ; begin : quote : ; square begin ( dup * ) end 7 square",
        Syntax::NameFirst,
    );
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(49))));
    assert_eq!(named(&s, state, "end"), named(&s, state, ";"));
    assert_eq!(named(&s, state, "begin"), named(&s, state, ":"));
}

#[test]
fn every_source_token_boundary_resumes_from_image_roots_only() {
    let source = "answer : 6 7 * ; square : ( dup * ) ; answer square";
    let (mut s, seed, state) = setup(source, Syntax::NameFirst);
    let direct = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
    assert_ok(&s, direct);
    let expected = Image::from_store(&s, &[seed.runner, direct]).unwrap();
    for boundary in 0..=source.split_whitespace().count() {
        let partial = resume(&mut s, seed.runner, state, boundary as u32, WORK)
            .unwrap()
            .root;
        let image = Image::from_store(&s, &[seed.runner, partial]).unwrap();
        let (image, mut loaded) = Image::parse(image.as_bytes()).unwrap();
        let resumed = resume(&mut loaded, image.roots[0], image.roots[1], 100, WORK)
            .unwrap()
            .root;
        assert_eq!(resumed, direct, "boundary {boundary}");
        assert_eq!(
            Image::from_store(&loaded, &[image.roots[0], resumed])
                .unwrap()
                .cid(),
            expected.cid()
        );
    }
}

#[test]
fn errors_are_explicit_reader_states() {
    for source in [
        "missing",
        "+",
        "x : ;",
        "x : 1 2 ;",
        "x : ( ) ;",
        "x : ( dup ) ;",
        "x : ( ( 1 ) ) ;",
        "x : 1",
        ")",
        ";",
        "quote",
        "quote missing",
    ] {
        let (mut s, seed, state) = setup(source, Syntax::NameFirst);
        let result = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
        assert!(
            matches!(
                s.get(field(&s, result, "error")),
                Some(Node::Const(Atom::Text(_)))
            ),
            "source {source}"
        );
    }
}

#[test]
fn quotations_run_in_construction_contexts_and_keep_outer_stacks_separate() {
    let (s, state) = evaluate(
        "9 square : ( dup * ) ; result : 7 square ; result",
        Syntax::NameFirst,
    );
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(49))));
    assert_eq!(
        s.get(named(&s, state, "result")),
        Some(&Node::Const(Atom::Int(49)))
    );
    let Some(Node::Pair(_, tail)) = s.get(field(&s, state, "stack")) else {
        panic!()
    };
    let Some(Node::Pair(cell, end)) = s.get(*tail) else {
        panic!()
    };
    assert_eq!(
        s.get(field(&s, *cell, "value")),
        Some(&Node::Const(Atom::Int(9)))
    );
    assert_eq!(s.get(*end), Some(&Node::Const(Atom::Unit)));
}

#[test]
fn explicit_input_wiring_matches_forth_stack_order() {
    for (source, expected) in [
        ("sum : ( + ) ; 4 5 sum", 9),
        ("keep-top : ( swap drop ) ; 10 20 keep-top", 20),
        ("replace : ( drop 3 ) ; 20 replace", 3),
        ("one : ( 1 ) ; one", 1),
        ("sum : ( + ) ; increment : ( 1 sum ) ; 8 increment", 9),
        ("f : ( 1 swap + ) ; 8 f", 9),
    ] {
        let (s, state) = evaluate(source, Syntax::NameFirst);
        assert_eq!(
            s.get(value(&s, state)),
            Some(&Node::Const(Atom::Int(expected))),
            "{source}"
        );
    }
}

#[test]
fn rebinding_preserves_static_links_and_code_aliases() {
    let (s, state) = evaluate(
        "n : 1 ; add-n : ( n + ) ; n : 2 ; alias : quote add-n ; 5 alias n",
        Syntax::NameFirst,
    );
    assert_eq!(
        s.get(named(&s, state, "n")),
        Some(&Node::Const(Atom::Int(2)))
    );
    assert_eq!(named(&s, state, "alias"), named(&s, state, "add-n"));
    let Some(Node::Pair(_, tail)) = s.get(field(&s, state, "stack")) else {
        panic!()
    };
    let Some(Node::Pair(cell, _)) = s.get(*tail) else {
        panic!()
    };
    assert_eq!(
        s.get(field(&s, *cell, "value")),
        Some(&Node::Const(Atom::Int(6)))
    );
}

#[test]
fn runtime_type_failures_do_not_accidentally_force_reflection() {
    for source in [
        "square : ( dup * ) ; quote square 2 +",
        "square : ( dup * ) ; quote square square",
        "square : ( dup * ) ; square",
    ] {
        let (mut s, seed, state) = setup(source, Syntax::NameFirst);
        let result = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
        assert!(matches!(
            s.get(field(&s, result, "error")),
            Some(Node::Const(Atom::Text(_)))
        ));
    }
    let (mut s, seed, state) = setup("9223372036854775807 1 +", Syntax::NameFirst);
    assert_eq!(
        resume(&mut s, seed.runner, state, 100, WORK),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn source_can_arrive_after_a_residual_seed_image_is_loaded() {
    let mut s = Store::new();
    let seed = Seed::build(&mut s, Syntax::NameFirst);
    let hole = s.intern(Node::Hole("source".into()));
    let state = seed.state(&mut s, hole);
    let partial = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
    let image = Image::from_store(&s, &[partial]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let source = loaded.intern(Node::Const(Atom::Text(
        "square : ( dup * ) ; 7 square".into(),
    )));
    let mut bindings = Bindings::new();
    bindings.insert("source", source);
    let result = Reducer::with_budget(&mut loaded, &bindings, WORK)
        .run(partial)
        .unwrap()
        .root;
    let (_, direct) = evaluate("square : ( dup * ) ; 7 square", Syntax::NameFirst);
    assert_eq!(result, direct);
}

#[test]
fn seed_and_final_image_are_independent_of_unreachable_store_history() {
    let source = "square : ( dup * ) ; 7 square";
    let (mut a, seed, initial) = setup(source, Syntax::NameFirst);
    let result = resume(&mut a, seed.runner, initial, 100, WORK)
        .unwrap()
        .root;
    let image = Image::from_store(&a, &[seed.runner, result]).unwrap();
    let mut b = Store::new();
    for n in (0..100).rev() {
        b.intern(Node::Const(Atom::Int(n)));
    }
    let other = Seed::build(&mut b, Syntax::NameFirst);
    assert_eq!(seed.runner, other.runner);
    assert_eq!(seed.dictionary, other.dictionary);
    let source = b.intern(Node::Const(Atom::Text(source.into())));
    let initial = other.state(&mut b, source);
    let result = resume(&mut b, other.runner, initial, 100, WORK)
        .unwrap()
        .root;
    assert_eq!(
        Image::from_store(&b, &[other.runner, result])
            .unwrap()
            .cid(),
        image.cid()
    );
}

#[test]
fn parser_aliases_and_pending_modes_survive_every_token_boundary() {
    let source = "end : quote ; ; begin : quote : ; square begin ( dup * ) end 7 square";
    let (mut s, seed, state) = setup(source, Syntax::NameFirst);
    let direct = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
    assert_ok(&s, direct);
    for boundary in 0..=source.split_whitespace().count() {
        let partial = resume(&mut s, seed.runner, state, boundary as u32, WORK)
            .unwrap()
            .root;
        let image = Image::from_store(&s, &[seed.runner, partial]).unwrap();
        let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
        let resumed = resume(&mut loaded, seed.runner, partial, 100, WORK)
            .unwrap()
            .root;
        assert_eq!(resumed, direct, "boundary {boundary}");
    }
}

#[test]
fn repeated_compiled_calls_have_linear_charged_work_in_this_fixed_dictionary() {
    let mut previous = 0;
    for count in [16, 32, 64] {
        let source = format!("square : ( dup * ) ; {}", "2 square drop ".repeat(count));
        let (mut s, seed, state) = setup(&source, Syntax::NameFirst);
        let result = resume(&mut s, seed.runner, state, 1000, WORK).unwrap();
        assert_ok(&s, result.root);
        let image = Image::from_store(&s, &[seed.runner, result.root]).unwrap();
        eprintln!(
            "seed calls={count} steps={} store-nodes={} live-image-bytes={}",
            result.stats.steps,
            s.len(),
            image.as_bytes().len()
        );
        if previous > 0 {
            assert!(result.stats.steps < previous * 3);
        }
        previous = result.stats.steps;
    }
}

#[test]
fn unicode_keys_are_exact() {
    let (s, state) = evaluate("é : 1 ; e\u{301} : 2 ; é e\u{301}", Syntax::NameFirst);
    assert_eq!(
        s.get(named(&s, state, "é")),
        Some(&Node::Const(Atom::Int(1)))
    );
    assert_eq!(
        s.get(named(&s, state, "e\u{301}")),
        Some(&Node::Const(Atom::Int(2)))
    );
}

#[test]
fn integer_literals_cannot_be_redefined_in_either_seed() {
    for numeral in [
        "7",
        "+7",
        "007",
        "-0",
        "9223372036854775807",
        "-9223372036854775808",
    ] {
        for syntax in [Syntax::NameFirst, Syntax::Forth] {
            let source = if syntax == Syntax::NameFirst {
                format!("{numeral} : 42 ;")
            } else {
                format!(": {numeral} 42 ;")
            };
            let (mut s, seed, state) = setup(&source, syntax);
            let result = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
            assert_eq!(
                s.get(field(&s, result, "error")),
                Some(&Node::Const(Atom::Text(
                    "integer literals cannot be definition names".into()
                )))
            );
        }
    }
    // Even an externally supplied dictionary entry cannot change a numeral's
    // interpretation. This is image policy, not a Rust token special case.
    let (mut s, seed, state) = setup("x : 42 ; 7", Syntax::NameFirst);
    let state = resume(&mut s, seed.runner, state, 4, WORK).unwrap().root;
    let dictionary = field(&s, state, "dictionary");
    let entry = field(&s, dictionary, "x");
    let key = s.intern(Node::Const(Atom::Text("7".into())));
    let dictionary = s.intern(Node::PutKey {
        record: dictionary,
        key,
        value: entry,
    });
    let state = s.intern(Node::Put {
        record: state,
        field: "dictionary".into(),
        value: dictionary,
    });
    let result = resume(&mut s, seed.runner, state, 100, WORK).unwrap().root;
    assert_ok(&s, result);
    assert_eq!(s.get(value(&s, result)), Some(&Node::Const(Atom::Int(7))));
}

#[test]
fn input_arity_limit_is_reported_before_reflection() {
    // This is the reachable state after enough symbolic input-consuming words;
    // initialize the counter near the boundary to keep the regression small.
    for (operation, inputs, exceeds) in [
        ("drop", 65534, false),
        ("+", 65534, true),
        ("dup", 65535, true),
        ("sum", 65535, true),
    ] {
        let source = format!("sum : ( + ) ; {operation}");
        let (mut s, seed, state) = setup(&source, Syntax::NameFirst);
        let state = resume(&mut s, seed.runner, state, 6, WORK).unwrap().root;
        let compile = s.intern(Node::Const(Atom::Text("compile".into())));
        let state = s.intern(Node::Put {
            record: state,
            field: "mode".into(),
            value: compile,
        });
        let count = s.intern(Node::Const(Atom::Int(inputs)));
        let state = s.intern(Node::Put {
            record: state,
            field: "inputs".into(),
            value: count,
        });
        let result = resume(&mut s, seed.runner, state, 1, WORK).unwrap().root;
        if exceeds {
            assert_eq!(
                s.get(field(&s, result, "error")),
                Some(&Node::Const(Atom::Text(
                    "quotation input arity exceeds 65535".into()
                )))
            );
        } else {
            assert_ok(&s, result);
            assert_eq!(
                s.get(field(&s, result, "inputs")),
                Some(&Node::Const(Atom::Int(65535)))
            );
        }
    }
}

#[test]
fn dead_symbolic_work_is_erased_but_the_evaluation_stack_is_strict() {
    let (s, state) = evaluate(
        "dead : ( 9223372036854775807 1 + drop 0 ) ; dead",
        Syntax::NameFirst,
    );
    assert_eq!(s.get(value(&s, state)), Some(&Node::Const(Atom::Int(0))));
    let (mut s, seed, state) = setup("9223372036854775807 1 + drop 0", Syntax::NameFirst);
    assert_eq!(
        resume(&mut s, seed.runner, state, 100, WORK),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn checkpoint_reload_discards_intermediate_history_with_explicit_roots() {
    let source = format!("square : ( dup * ) ; {}", "2 square drop ".repeat(64));
    let (mut s, seed, initial) = setup(&source, Syntax::NameFirst);
    let direct_run = resume(&mut s, seed.runner, initial, 1000, WORK).unwrap();
    let direct = direct_run.root;
    let uncollected_nodes = s.len();
    let expected = Image::from_store(&s, &[seed.runner, direct]).unwrap();
    let (_, mut s) = Image::parse(
        Image::from_store(&s, &[seed.runner, initial])
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let mut state = initial;
    let mut peak_nodes = s.len();
    let mut epochs = 0;
    let mut checkpoint_steps = 0;
    loop {
        let resumed = resume(&mut s, seed.runner, state, 16, WORK).unwrap();
        state = resumed.root;
        checkpoint_steps += resumed.stats.steps;
        assert_ok(&s, state);
        peak_nodes = peak_nodes.max(s.len());
        // These are all retained roots for THIS fixture. An application with
        // other live roots must include them before replacing its store.
        let snapshot = Image::from_store(&s, &[seed.runner, state]).unwrap();
        let (_, loaded) = Image::parse(snapshot.as_bytes()).unwrap();
        s = loaded;
        epochs += 1;
        if s.get(field(&s, state, "position")) == Some(&Node::Const(Atom::Int(source.len() as i64)))
        {
            break;
        }
        assert!(epochs < 100, "reader failed to make progress");
    }
    assert_eq!(state, direct);
    assert_eq!(
        Image::from_store(&s, &[seed.runner, state]).unwrap().cid(),
        expected.cid()
    );
    assert!(peak_nodes < uncollected_nodes);
    eprintln!(
        "seed checkpoints: epochs={epochs} uncollected-nodes={uncollected_nodes} peak-epoch-nodes={peak_nodes} retained-nodes={} direct-steps={} checkpoint-steps={checkpoint_steps}",
        s.len(),
        direct_run.stats.steps
    );
}
