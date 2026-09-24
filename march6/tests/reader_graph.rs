//! B0b witness, not the language seed: a graph-defined token-frequency reader.
//! The host constructs and invokes graphs, but never iterates over source tokens.
use march_research::{Atom, Bindings, Cid, Clause, Image, Node, Reducer, Store};

fn int(s: &mut Store, n: i64) -> Cid {
    s.intern(Node::Const(Atom::Int(n)))
}
fn text(s: &mut Store, t: &str) -> Cid {
    s.intern(Node::Const(Atom::Text(t.into())))
}
fn get(s: &mut Store, record: Cid, field: &str) -> Cid {
    s.intern(Node::Get {
        record,
        field: field.into(),
    })
}
fn record(s: &mut Store, fields: &[(&str, Cid)]) -> Cid {
    s.intern(Node::Record(
        fields.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    ))
}
fn run(s: &mut Store, root: Cid) -> Cid {
    Reducer::with_budget(s, &Bindings::new(), 20_000_000)
        .run(root)
        .unwrap()
        .root
}

// Explicit parameters: immutable state and number of tokens allowed this epoch.
// EOF is detected by a guard; otherwise the body computes the successor state.
fn reader(s: &mut Store) -> Cid {
    let state = s.intern(Node::Param(0));
    let allowance = s.intern(Node::Param(1));
    let zero = int(s, 0);
    let one = int(s, 1);
    let minus_one = int(s, -1);
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let no = s.intern(Node::Const(Atom::Bool(false)));
    let input = get(s, state, "text");
    let position = get(s, state, "position");
    let dictionary = get(s, state, "dictionary");
    let count = get(s, state, "count");
    let next = s.intern(Node::NextToken {
        text: input,
        position,
    });
    let found = get(s, next, "found");
    let key = get(s, next, "token");
    let position = get(s, next, "position");
    let entry = s.intern(Node::Lookup {
        record: dictionary,
        key,
    });
    let present = get(s, entry, "found");
    let previous = get(s, entry, "value");
    let previous = s.intern(Node::If {
        condition: present,
        when_true: previous,
        when_false: zero,
    });
    let value = s.intern(Node::Add(previous, one));
    let updated = s.intern(Node::PutKey {
        record: dictionary,
        key,
        value,
    });
    let increment = s.intern(Node::Add(count, one));
    let successor = record(
        s,
        &[
            ("text", input),
            ("position", position),
            ("dictionary", updated),
            ("count", increment),
        ],
    );
    let eof = record(
        s,
        &[
            ("text", input),
            ("position", position),
            ("dictionary", dictionary),
            ("count", count),
        ],
    );
    let stop = s.intern(Node::Eq(allowance, zero));
    let exhausted = s.intern(Node::Eq(found, no));
    let remaining = s.intern(Node::Add(allowance, minus_one));
    let recur = s.intern(Node::Recur(vec![successor, remaining]));
    s.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: stop,
                body: state,
            },
            Clause {
                guard: exhausted,
                body: eof,
            },
            Clause {
                guard: yes,
                body: recur,
            },
        ],
    })
}

fn initial(s: &mut Store, source: &str) -> Cid {
    let input = text(s, source);
    let zero = int(s, 0);
    let dictionary = s.intern(Node::Record(vec![]));
    record(
        s,
        &[
            ("text", input),
            ("position", zero),
            ("dictionary", dictionary),
            ("count", zero),
        ],
    )
}
fn read(s: &mut Store, family: Cid, state: Cid, tokens: i64) -> Cid {
    let allowance = int(s, tokens);
    let call = s.intern(Node::Dispatch {
        family,
        arguments: vec![state, allowance],
    });
    run(s, call)
}

#[test]
fn a_guarded_graph_reads_and_updates_an_immutable_dictionary() {
    let mut s = Store::new();
    let family = reader(&mut s);
    let state = initial(&mut s, "square : ( dup * ) ; square  \n");
    let result = read(&mut s, family, state, 100);
    let count = get(&mut s, result, "count");
    assert_eq!(run(&mut s, count), int(&mut s, 8));
    let dict = get(&mut s, result, "dictionary");
    let key = text(&mut s, "square");
    let lookup = s.intern(Node::Lookup { record: dict, key });
    let count = get(&mut s, lookup, "value");
    assert_eq!(run(&mut s, count), int(&mut s, 2));
    let original = get(&mut s, state, "dictionary");
    assert_eq!(run(&mut s, original), s.intern(Node::Record(vec![])));
}

#[test]
fn every_token_boundary_can_be_saved_reloaded_and_resumed() {
    let mut s = Store::new();
    let family = reader(&mut s);
    let state = initial(&mut s, "α : ( dup * ) ; α\n");
    let direct = read(&mut s, family, state, 100);
    let expected = Image::from_store(&s, &[family, direct]).unwrap();
    for boundary in 0..=8 {
        let partial = read(&mut s, family, state, boundary);
        let image = Image::from_store(&s, &[family, partial]).unwrap();
        let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
        let resumed = read(&mut loaded, family, partial, 100);
        assert_eq!(resumed, direct, "boundary {boundary}");
        assert_eq!(
            Image::from_store(&loaded, &[family, resumed])
                .unwrap()
                .cid(),
            expected.cid()
        );
    }
}

#[test]
fn reader_residualizes_before_source_is_available() {
    let mut s = Store::new();
    let family = reader(&mut s);
    let source = s.intern(Node::Hole("source".into()));
    let zero = int(&mut s, 0);
    let dictionary = s.intern(Node::Record(vec![]));
    let state = record(
        &mut s,
        &[
            ("text", source),
            ("position", zero),
            ("dictionary", dictionary),
            ("count", zero),
        ],
    );
    let partial = read(&mut s, family, state, 100);
    let image = Image::from_store(&s, &[partial]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let input = text(&mut loaded, "a b a");
    let mut bindings = Bindings::new();
    bindings.0.insert("source".into(), input);
    let staged = Reducer::with_budget(&mut loaded, &bindings, 1_000_000)
        .run(partial)
        .unwrap()
        .root;
    let state = initial(&mut s, "a b a");
    assert_eq!(staged, read(&mut s, family, state, 100));
}

#[test]
fn dictionary_held_code_survives_an_image_and_invokes_without_capture() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let product = s.intern(Node::Mul(p, p));
    let square = s.intern(Node::Quote {
        params: 1,
        body: product,
    });
    let dictionary = s.intern(Node::Record(vec![]));
    let name = text(&mut s, "square");
    let bind = s.intern(Node::PutKey {
        record: dictionary,
        key: name,
        value: square,
    });
    let dict = run(&mut s, bind);
    let image = Image::from_store(&s, &[dict]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let name = text(&mut loaded, "square");
    let position = int(&mut loaded, 0);
    let next = loaded.intern(Node::NextToken {
        text: name,
        position,
    });
    let name = get(&mut loaded, next, "token");
    let entry = loaded.intern(Node::Lookup {
        record: dict,
        key: name,
    });
    let function = get(&mut loaded, entry, "value");
    let seven = text(&mut loaded, "7");
    let parsed = loaded.intern(Node::ParseInt(seven));
    let argument = get(&mut loaded, parsed, "value");
    let call = loaded.intern(Node::Apply {
        function,
        arguments: vec![argument],
    });
    assert_eq!(run(&mut loaded, call), int(&mut loaded, 49));
}

#[test]
fn ten_thousand_tokens_do_not_use_native_recursive_calls() {
    let mut s = Store::new();
    let family = reader(&mut s);
    let state = initial(&mut s, &"x ".repeat(10_000));
    let result = read(&mut s, family, state, 10_001);
    let count = get(&mut s, result, "count");
    assert_eq!(run(&mut s, count), int(&mut s, 10_000));
}
