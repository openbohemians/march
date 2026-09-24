use march_research::{
    Atom, Bindings, Cid, Clause, CollectionError, Image, Node, ReduceError, Reducer,
    SpecializationCache, Store,
    seed::{Seed, Syntax, resume},
};

const WORK: usize = 20_000_000;

fn field(store: &Store, record: Cid, name: &str) -> Cid {
    let Some(Node::Record(fields)) = store.get(record) else {
        panic!("not a record")
    };
    fields.iter().find(|(key, _)| key == name).unwrap().1
}

#[test]
fn roots_keep_shared_data_and_all_dormant_code_without_evaluation() {
    let mut s = Store::new();
    let one = s.intern(Node::Const(Atom::Int(1)));
    let max = s.intern(Node::Const(Atom::Int(i64::MAX)));
    let bad = s.intern(Node::Add(max, one));
    let quote = s.intern(Node::Quote {
        params: 0,
        body: bad,
    });
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let family = s.intern(Node::Family {
        parameters: 0,
        clauses: vec![Clause {
            guard: yes,
            body: quote,
        }],
    });
    let shared = s.intern(Node::Pair(one, one));
    let root = s.intern(Node::Record(vec![
        ("code".into(), family),
        ("data".into(), shared),
    ]));
    let extra_root = s.intern(Node::Const(Atom::Text("host-held".into())));
    let garbage = s.intern(Node::Const(Atom::Int(999)));
    let image = Image::from_store(&s, &[root, extra_root]).unwrap();
    let (_, expected) = Image::parse(image.as_bytes()).unwrap();
    let stats = s.collect(&[root, extra_root, root]).unwrap();
    assert_eq!(stats.reclaimed, 1);
    assert_eq!(stats.retained, expected.len());
    assert!(s.get(garbage).is_none());
    for cid in [one, max, bad, quote, yes, family, shared, extra_root] {
        assert!(s.get(cid).is_some());
    }
    assert_eq!(Image::from_store(&s, &[root, extra_root]).unwrap(), image);
    assert_eq!(s.collect(&[root, extra_root]).unwrap().reclaimed, 0);
    // Freed content can be interned again with the same identity.
    assert_eq!(s.intern(Node::Const(Atom::Int(999))), garbage);
    let total = s.len();
    assert_eq!(s.collect(&[]).unwrap().reclaimed, total);
    assert!(s.is_empty());
}

#[test]
fn missing_root_or_reachable_edge_fails_before_deleting_anything() {
    let mut s = Store::new();
    let good = s.intern(Node::Const(Atom::Int(7)));
    let absent = Cid::digest(b"absent", b"node");
    let malformed = s.intern(Node::Pair(good, absent));
    for roots in [vec![good, absent], vec![malformed]] {
        assert_eq!(s.collect(&roots), Err(CollectionError::MissingNode(absent)));
        assert_eq!(s.len(), 2);
        assert!(s.get(good).is_some() && s.get(malformed).is_some());
    }
    // Unreachable malformed history is removable; only the live graph matters.
    assert_eq!(s.collect(&[good]).unwrap().reclaimed, 1);
}

#[test]
fn deep_shared_graph_collection_is_iterative() {
    let mut s = Store::new();
    let leaf = s.intern(Node::Const(Atom::Unit));
    let mut root = leaf;
    for _ in 0..30_000 {
        root = s.intern(Node::Pair(root, leaf));
    }
    let stats = s.collect(&[root]).unwrap();
    assert_eq!(stats.retained, 30_001);
    assert_eq!(stats.traced_edges, 60_000);
    assert_eq!(s.collect(&[leaf]).unwrap().reclaimed, 30_000);
}

#[test]
fn specialization_cache_and_bindings_can_supply_host_roots() {
    let mut s = Store::new();
    let input = s.intern(Node::Hole("x".into()));
    let one = s.intern(Node::Const(Atom::Int(1)));
    let program = s.intern(Node::Add(input, one));
    let seven = s.intern(Node::Const(Atom::Int(7)));
    let mut bindings = Bindings::new();
    bindings.insert("x", seven);
    let mut cache = SpecializationCache::new();
    let first = cache.specialize(&mut s, program, &bindings, WORK).unwrap();
    let roots: Vec<_> = [program]
        .into_iter()
        .chain(cache.roots())
        .chain(bindings.0.values().copied())
        .collect();
    s.collect(&roots).unwrap();
    let second = cache.specialize(&mut s, program, &bindings, WORK).unwrap();
    assert!(second.cache_hit);
    assert_eq!(first.residual, second.residual);
    assert_eq!(s.get(second.residual), Some(&Node::Const(Atom::Int(8))));
}

#[test]
fn staged_residual_and_external_bindings_survive_collection() {
    let mut s = Store::new();
    let x = s.intern(Node::Hole("x".into()));
    let one = s.intern(Node::Const(Atom::Int(1)));
    let expression = s.intern(Node::Add(x, one));
    let staged = Reducer::with_budget(&mut s, &Bindings::new(), WORK)
        .run(expression)
        .unwrap()
        .root;
    let forty_one = s.intern(Node::Const(Atom::Int(41)));
    s.collect(&[staged, forty_one]).unwrap();
    let mut runtime = Bindings::new();
    runtime.insert("x", forty_one);
    let result = Reducer::with_budget(&mut s, &runtime, WORK)
        .run(staged)
        .unwrap()
        .root;
    assert_eq!(s.get(result), Some(&Node::Const(Atom::Int(42))));
}

fn setup(source: &str, syntax: Syntax) -> (Store, Seed, Cid) {
    let mut s = Store::new();
    let seed = Seed::build(&mut s, syntax);
    let source = s.intern(Node::Const(Atom::Text(source.into())));
    let state = seed.state(&mut s, source);
    (s, seed, state)
}

#[test]
fn collection_at_every_token_boundary_preserves_exact_final_images() {
    for (source, syntax) in [
        (
            "end : quote ; ; begin : quote : ; square begin ( dup * ) end 9223372036854775807 1 + drop 7 square",
            Syntax::NameFirst,
        ),
        (
            ": square dup * ; : fourth square square ; 9223372036854775807 1 + drop 2 fourth",
            Syntax::Forth,
        ),
    ] {
        let (mut s, seed, initial) = setup(source, syntax);
        let initial_image = Image::from_store(&s, &[seed.runner, initial]).unwrap();
        let final_state = resume(&mut s, seed.runner, initial, u32::MAX, WORK)
            .unwrap()
            .root;
        let expected = Image::from_store(&s, &[seed.runner, final_state]).unwrap();
        for boundary in 0..=source.split_whitespace().count() as u32 {
            let (_, mut s) = Image::parse(initial_image.as_bytes()).unwrap();
            let paused = resume(&mut s, seed.runner, initial, boundary, WORK)
                .unwrap()
                .root;
            let before = Image::from_store(&s, &[seed.runner, paused]).unwrap();
            s.collect(&[seed.runner, paused]).unwrap();
            assert_eq!(
                Image::from_store(&s, &[seed.runner, paused]).unwrap(),
                before
            );
            let finished = resume(&mut s, seed.runner, paused, u32::MAX, WORK)
                .unwrap()
                .root;
            s.collect(&[seed.runner, finished]).unwrap();
            assert_eq!(
                Image::from_store(&s, &[seed.runner, finished]).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn collection_does_not_force_pending_overflow_or_hide_demanded_errors() {
    let (mut s, seed, initial) = setup("9223372036854775807 1 +", Syntax::NameFirst);
    let paused = resume(&mut s, seed.runner, initial, 3, WORK).unwrap().root;
    s.collect(&[seed.runner, paused]).unwrap();
    assert_eq!(
        resume(&mut s, seed.runner, paused, 1, WORK),
        Err(ReduceError::IntegerOverflow("add"))
    );
}

#[test]
fn batched_collection_reclaims_history_without_image_codec_work() {
    let source = format!("square : ( dup * ) ; {}", "2 square drop ".repeat(64));
    let (mut s, seed, initial) = setup(&source, Syntax::NameFirst);
    let initial_image = Image::from_store(&s, &[seed.runner, initial]).unwrap();
    let direct = resume(&mut s, seed.runner, initial, 1000, WORK)
        .unwrap()
        .root;
    let uncollected = s.len();
    let expected = Image::from_store(&s, &[seed.runner, direct]).unwrap();
    let (_, mut s) = Image::parse(initial_image.as_bytes()).unwrap();
    let mut state = initial;
    let mut peak = s.len();
    let mut reclaimed = 0;
    let mut epochs = 0;
    loop {
        let next = resume(&mut s, seed.runner, state, 16, WORK).unwrap().root;
        peak = peak.max(s.len());
        let stats = s.collect(&[seed.runner, next]).unwrap();
        reclaimed += stats.reclaimed;
        epochs += 1;
        assert_eq!(
            s.get(field(&s, next, "error")),
            Some(&Node::Const(Atom::Unit))
        );
        if next == state {
            break;
        }
        state = next;
        assert!(epochs < 20);
    }
    assert_eq!(
        Image::from_store(&s, &[seed.runner, state]).unwrap(),
        expected
    );
    assert!(reclaimed > 0 && peak * 4 < uncollected);
    eprintln!(
        "in-memory collection: epochs={epochs} uncollected={uncollected} peak={peak} retained={} reclaimed={reclaimed}",
        s.len()
    );
}
