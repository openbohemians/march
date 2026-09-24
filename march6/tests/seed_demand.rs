use march_research::{
    Atom, Cid, Image, Node, ReduceError, Store,
    seed::{Seed, Syntax, resume},
};

const WORK: usize = 20_000_000;
const OVERFLOW: &str = "9223372036854775807 1 +";

fn field(store: &Store, record: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = store.get(record) else {
        panic!("expected record: {}", store.format(record));
    };
    fields.iter().find(|(key, _)| key == name).unwrap().1
}

fn setup(source: &str) -> (Store, Seed, Cid) {
    let mut store = Store::new();
    let seed = Seed::build(&mut store, Syntax::NameFirst);
    let source = store.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut store, source);
    (store, seed, state)
}

fn check_ok(store: &Store, state: Cid) {
    assert_eq!(
        store.get(field(store, state, "error")),
        Some(&Node::Const(Atom::Unit))
    );
}

fn cells(store: &Store, state: Cid) -> Vec<Cid> {
    let mut stack = field(store, state, "stack");
    let mut result = Vec::new();
    while let Some(Node::Pair(head, tail)) = store.get(stack) {
        result.push(*head);
        stack = *tail;
    }
    assert_eq!(store.get(stack), Some(&Node::Const(Atom::Unit)));
    result
}

fn values(source: &str) -> Vec<i64> {
    let (mut store, seed, state) = setup(source);
    let result = resume(&mut store, seed.runner, state, u32::MAX, WORK).unwrap();
    check_ok(&store, result.root);
    cells(&store, result.root)
        .iter()
        .map(|cell| {
            assert_eq!(
                store.get(field(&store, *cell, "kind")),
                Some(&Node::Const(Atom::Text("int".into())))
            );
            let Some(Node::Const(Atom::Int(value))) = store.get(field(&store, *cell, "value"))
            else {
                panic!("expected observed integer");
            };
            *value
        })
        .collect()
}

fn overflow(source: &str) {
    let (mut store, seed, state) = setup(source);
    assert_eq!(
        resume(&mut store, seed.runner, state, u32::MAX, WORK),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn discarded_calls_and_unused_arguments_are_not_demanded() {
    for source in [
        format!("{OVERFLOW} drop 1"),
        format!("bad : ( {OVERFLOW} ) ; bad drop 1"),
        format!("ignore : ( drop 1 ) ; {OVERFLOW} ignore"),
        format!("ignore : ( drop 1 ) ; bad : ( {OVERFLOW} ) ; bad ignore"),
        format!("second : ( swap drop ) ; {OVERFLOW} 1 second"),
    ] {
        assert_eq!(values(&source), vec![1], "{source}");
    }
}

#[test]
fn demanded_overflow_matches_inline_and_factored_code() {
    for source in [
        OVERFLOW.to_owned(),
        format!("bad : ( {OVERFLOW} ) ; bad"),
        "plus : ( + ) ; 9223372036854775807 1 plus".into(),
        format!("id : ( dup drop ) ; {OVERFLOW} id"),
        format!("{OVERFLOW} dup drop"),
    ] {
        overflow(&source);
    }
}

#[test]
fn eof_observes_the_whole_stack_including_its_tail() {
    assert_eq!(values("2 3 + 4 5 *"), vec![20, 5]);
    overflow(&format!("{OVERFLOW} 7"));
    overflow(&format!("7 {OVERFLOW}"));
    assert_eq!(values(&format!("{OVERFLOW} 7 swap drop")), vec![7]);
}

#[test]
fn binding_observes_constants_but_not_the_saved_outer_stack() {
    overflow(&format!("bad : {OVERFLOW} ; 0"));
    assert_eq!(values(&format!("good : {OVERFLOW} drop 3 ; good")), vec![3]);
    assert_eq!(
        values(&format!("{OVERFLOW} good : 3 4 + ; drop good")),
        vec![7]
    );
    let (mut store, seed, state) = setup("answer : 6 7 * ; answer");
    let state = resume(&mut store, seed.runner, state, 6, WORK)
        .unwrap()
        .root;
    let answer = field(&store, field(&store, state, "dictionary"), "answer");
    assert_eq!(
        store.get(field(&store, answer, "kind")),
        Some(&Node::Const(Atom::Text("int".into())))
    );
    assert_eq!(
        store.get(field(&store, answer, "value")),
        Some(&Node::Const(Atom::Int(42)))
    );
}

#[test]
fn code_and_parser_values_remain_dormant_at_eof() {
    for (source, kind) in [
        (format!("( {OVERFLOW} )"), "code"),
        (format!("bad : ( {OVERFLOW} ) ; quote bad"), "code"),
        ("quote +".into(), "handler"),
    ] {
        let (mut store, seed, state) = setup(&source);
        let state = resume(&mut store, seed.runner, state, u32::MAX, WORK)
            .unwrap()
            .root;
        check_ok(&store, state);
        let cell = cells(&store, state)[0];
        assert_eq!(
            store.get(field(&store, cell, "kind")),
            Some(&Node::Const(Atom::Text(kind.into())))
        );
    }
}

#[test]
fn quoted_integer_cells_work_with_primitives_and_calls() {
    assert_eq!(values("a : 7 ; quote a 2 *"), vec![14]);
    assert_eq!(
        values("a : 7 ; square : ( dup * ) ; quote a square"),
        vec![49]
    );
}

#[test]
fn pause_and_reload_do_not_observe_pending_overflow() {
    let source = format!("{OVERFLOW} drop 0");
    let (mut store, seed, initial) = setup(&source);
    let state = resume(&mut store, seed.runner, initial, 3, WORK)
        .unwrap()
        .root;
    check_ok(&store, state);
    let pending = cells(&store, state)[0];
    assert_eq!(
        store.get(field(&store, pending, "kind")),
        Some(&Node::Const(Atom::Text("expr".into())))
    );
    assert_eq!(
        resume(&mut store, seed.runner, state, 0, WORK)
            .unwrap()
            .root,
        state
    );
    let image = Image::from_store(&store, &[seed.runner, state]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let resumed = resume(&mut loaded, seed.runner, state, u32::MAX, WORK)
        .unwrap()
        .root;
    let direct = resume(&mut store, seed.runner, initial, u32::MAX, WORK)
        .unwrap()
        .root;
    assert_eq!(direct, resumed);
    assert_eq!(
        Image::from_store(&store, &[seed.runner, direct])
            .unwrap()
            .cid(),
        Image::from_store(&loaded, &[seed.runner, resumed])
            .unwrap()
            .cid()
    );
}

#[test]
fn last_token_pause_defers_observation_until_eof() {
    let (mut store, seed, state) = setup(OVERFLOW);
    let state = resume(&mut store, seed.runner, state, 3, WORK)
        .unwrap()
        .root;
    let image = Image::from_store(&store, &[seed.runner, state]).unwrap();
    let (_, mut store) = Image::parse(image.as_bytes()).unwrap();
    assert_eq!(
        resume(&mut store, seed.runner, state, 1, WORK),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn malformed_or_incomplete_reader_does_not_observe_pending_work() {
    for source in [
        format!("{OVERFLOW} missing"),
        format!("{OVERFLOW} quote"),
        format!("answer : {OVERFLOW}"),
        format!("( {OVERFLOW}"),
    ] {
        let (mut store, seed, state) = setup(&source);
        let state = resume(&mut store, seed.runner, state, u32::MAX, WORK)
            .unwrap()
            .root;
        assert!(matches!(
            store.get(field(&store, state, "error")),
            Some(Node::Const(Atom::Text(_)))
        ));
        assert_eq!(
            resume(&mut store, seed.runner, state, 1, WORK)
                .unwrap()
                .root,
            state
        );
    }
}

#[test]
fn duplicated_pending_graphs_share_demand_in_one_observation_epoch() {
    fn run(copies: usize) -> usize {
        let source = format!("1 {}{}", "1 + ".repeat(64), "dup ".repeat(copies - 1));
        let (mut store, seed, state) = setup(&source);
        let tokens = source.split_whitespace().count() as u32;
        let state = resume(&mut store, seed.runner, state, tokens, WORK)
            .unwrap()
            .root;
        let pending = cells(&store, state);
        assert_eq!(pending.len(), copies);
        assert!(pending.iter().all(|cell| *cell == pending[0]));
        // Discard the reducer memo along with unreachable history before observing.
        let image = Image::from_store(&store, &[seed.runner, state]).unwrap();
        let (_, mut store) = Image::parse(image.as_bytes()).unwrap();
        let result = resume(&mut store, seed.runner, state, 1, WORK).unwrap();
        for cell in cells(&store, result.root) {
            assert_eq!(
                store.get(field(&store, cell, "value")),
                Some(&Node::Const(Atom::Int(65)))
            );
        }
        result.stats.steps
    }
    let one = run(1);
    let sixteen = run(16);
    eprintln!("seed shared observation steps: one={one}, sixteen={sixteen}");
    assert!(
        sixteen < one * 3,
        "demand should share the deep expression, not repeat it sixteen times"
    );
}

#[test]
fn generated_fragment_extraction_preserves_values_and_error_classes() {
    fn outcome(source: &str, syntax: Syntax) -> Result<Vec<Cid>, ReduceError> {
        let mut store = Store::new();
        let seed = Seed::build(&mut store, syntax);
        let text = store.intern(Node::Const(Atom::Text(source.into())));
        let state = seed.state(&mut store, text);
        let state = resume(&mut store, seed.runner, state, u32::MAX, WORK)?.root;
        check_ok(&store, state);
        Ok(cells(&store, state)
            .iter()
            .map(|cell| field(&store, *cell, "value"))
            .collect())
    }
    let mut random = 0x8729_55ab_u64;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        random as usize
    };
    let literals = [
        "0",
        "1",
        "-1",
        "2",
        "9223372036854775807",
        "-9223372036854775808",
    ];
    let mut successes = 0;
    let mut failures = 0;
    for _ in 0..36 {
        let mut depth = 0;
        let mut inputs = 0;
        let mut body = String::new();
        for _ in 0..8 {
            let (word, needed, produced) = match next() % 6 {
                0 => (literals[next() % literals.len()], 0, 1),
                1 => ("dup", 1, 2),
                2 => ("drop", 1, 0),
                3 => ("swap", 2, 2),
                4 => ("+", 2, 1),
                _ => ("*", 2, 1),
            };
            if depth < needed {
                inputs += needed - depth;
                depth = needed;
            }
            depth = depth - needed + produced;
            body.push_str(word);
            body.push(' ');
        }
        while depth > 1 {
            body.push_str("+ ");
            depth -= 1;
        }
        if depth == 0 {
            body.push_str("0 ");
        }
        // The sentinel checks preservation of stack cells outside the interface.
        let mut prefix = String::from("17 ");
        for _ in 0..inputs {
            prefix.push_str(literals[next() % literals.len()]);
            prefix.push(' ');
        }
        for suffix in ["", "drop 3", "dup *"] {
            for syntax in [Syntax::NameFirst, Syntax::Forth] {
                let definition = match syntax {
                    Syntax::NameFirst => format!("fragment : ( {body}) ;"),
                    Syntax::Forth => format!(": fragment {body};"),
                };
                let inline = format!("{prefix}{body}{suffix}");
                let factored = format!("{definition} {prefix}fragment {suffix}");
                let expected = outcome(&inline, syntax);
                match &expected {
                    Ok(_) => successes += 1,
                    Err(ReduceError::IntegerOverflow(_)) => failures += 1,
                    Err(error) => panic!("unexpected {error:?}: {inline}"),
                }
                assert_eq!(
                    outcome(&factored, syntax),
                    expected,
                    "{syntax:?}: {inline} versus {factored}"
                );
            }
        }
    }
    assert!(successes > 0 && failures > 0);
}
