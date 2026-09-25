use march_research::{
    Cid,
    fast::{
        Context, Error, Executor, Literal, Op, Program, Value,
        definition::{Definition, Item},
        stream,
    },
};

fn named_cid(p: &Program, name: &str) -> Cid {
    p.cid(p.lookup(name).unwrap()).unwrap()
}
fn run(p: &Program, w: usize, args: &[Literal]) -> Vec<Value> {
    Executor::new(p)
        .run(w, args, &Context::new(), 100_000)
        .unwrap()
}

#[test]
fn square_is_exactly_the_ordered_dependency_sequence() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": square dup * ; : renamed dup * ;").unwrap();
    let square = p.lookup("square").unwrap();
    let dup = p.lookup("dup").unwrap();
    let mul = p.lookup("*").unwrap();
    assert_eq!(
        p.definition(square).unwrap(),
        Some(&Definition::Sequence(vec![
            Item::Word(dup),
            Item::Word(mul)
        ]))
    );
    // Pin the canonical bytes independently: sequence tag, length, word tags,
    // dependency CIDs. No inputs, output slots, or register connections.
    let mut bytes = vec![2];
    bytes.extend_from_slice(&2u64.to_le_bytes());
    for word in [dup, mul] {
        bytes.push(0);
        bytes.extend_from_slice(&p.cid(word).unwrap().0);
    }
    assert_eq!(
        p.cid(square).unwrap(),
        Cid::digest(b"march-definition-v1", &bytes)
    );
    assert_eq!(named_cid(&p, "square"), named_cid(&p, "renamed"));
    assert_eq!(run(&p, square, &[Literal::Int(7)]), [Value::Int(49)]);
}

#[test]
fn no_semantic_normalization_or_source_spelling_in_identity() {
    let mut p = stream::seed().unwrap();
    stream::compile(
        &mut p,
        ": two 1 1 + ; : literal 2 ; : spelled +002 ; : noop ; : shuffled swap swap ;",
    )
    .unwrap();
    assert_ne!(named_cid(&p, "two"), named_cid(&p, "literal"));
    assert_eq!(named_cid(&p, "literal"), named_cid(&p, "spelled"));
    assert_ne!(named_cid(&p, "noop"), named_cid(&p, "shuffled"));
    assert_eq!(run(&p, p.lookup("two").unwrap(), &[]), [Value::Int(2)]);
}

#[test]
fn dependency_changes_propagate_but_rebinding_does_not_rewrite_old_code() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": leaf 1 ; : parent leaf ; : top parent ;").unwrap();
    let old = p.lookup("top").unwrap();
    let before = p.cid(old).unwrap();
    stream::compile(&mut p, ": leaf 2 ; : parent leaf ; : top parent ;").unwrap();
    assert_ne!(named_cid(&p, "top"), before);
    assert_eq!(run(&p, old, &[]), [Value::Int(1)]);
    assert_eq!(run(&p, p.lookup("top").unwrap(), &[]), [Value::Int(2)]);
}

#[test]
fn names_and_immediacy_are_image_metadata_not_definition_identity() {
    let mut p = stream::seed().unwrap();
    let entry = stream::compile(&mut p, ": custom stream.word ;").unwrap();
    let before = p.to_image(entry).unwrap();
    let cid = named_cid(&p, "custom");
    p.mark_immediate("custom").unwrap();
    p.bind("alias", p.lookup("custom").unwrap()).unwrap();
    assert_eq!(named_cid(&p, "alias"), cid);
    assert_eq!(named_cid(&p, "custom"), cid);
    assert!(!p.is_immediate("alias"));
    assert_ne!(p.to_image(entry).unwrap(), before);
}

#[test]
fn quote_kind_order_and_context_keys_are_meaning_bearing() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": a 1 ; : b 2 ; : ab a b ; : ba b a ; : quoted ' a ; : inline [ 1 ] ; : invoked a ; : x ctx x ; : y ctx y ;").unwrap();
    assert_ne!(named_cid(&p, "ab"), named_cid(&p, "ba"));
    assert_eq!(named_cid(&p, "quoted"), named_cid(&p, "inline"));
    assert_ne!(named_cid(&p, "quoted"), named_cid(&p, "invoked"));
    assert_ne!(named_cid(&p, "x"), named_cid(&p, "y"));
    let x = p.lookup("x").unwrap();
    let cid = p.cid(x).unwrap();
    for n in [1, 2] {
        assert_eq!(
            Executor::new(&p)
                .run(x, &[], &Context::from([("x".into(), Literal::Int(n))]), 100)
                .unwrap(),
            [Value::Int(n)]
        );
        assert_eq!(p.cid(x).unwrap(), cid);
    }
}

#[test]
fn recursion_is_symbolic_and_bounded_not_a_circular_cid() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": a recur 0 1 ; : b recur 0 1 ; : c recur 1 1 ;").unwrap();
    assert_eq!(named_cid(&p, "a"), named_cid(&p, "b"));
    assert_ne!(named_cid(&p, "a"), named_cid(&p, "c"));
    assert_eq!(
        p.definition(p.lookup("a").unwrap()).unwrap(),
        Some(&Definition::Sequence(vec![Item::Recur {
            inputs: 0,
            outputs: 1
        }]))
    );
    assert!(matches!(
        Executor::new(&p).run(p.lookup("a").unwrap(), &[], &Context::new(), 100),
        Err(Error::Cycle | Error::Budget)
    ));
}

#[test]
fn families_hash_ordered_clauses_and_derive_their_signature() {
    let mut p = stream::seed().unwrap();
    stream::compile(&mut p, ": yes drop true ; : no drop false ; : a drop 1 ; : b drop 2 ; family f 1 1 yes a no b ; family renamed 1 1 yes a no b ; family reordered 1 1 no b yes a ;").unwrap();
    let clauses = vec![
        (p.lookup("yes").unwrap(), p.lookup("a").unwrap()),
        (p.lookup("no").unwrap(), p.lookup("b").unwrap()),
    ];
    assert_eq!(
        p.definition(p.lookup("f").unwrap()).unwrap(),
        Some(&Definition::Family(clauses))
    );
    assert_eq!(named_cid(&p, "f"), named_cid(&p, "renamed"));
    assert_ne!(named_cid(&p, "f"), named_cid(&p, "reordered"));
    assert!(stream::compile(&mut p, "family bad 0 1 yes a ;").is_err());
}

#[test]
fn images_keep_even_unused_definition_references() {
    let mut p = stream::seed().unwrap();
    let entry = stream::compile(
        &mut p,
        ": hidden 7 ; : saved ' hidden drop ; : hidden 8 ; saved 9",
    )
    .unwrap();
    let bytes = p.to_image(entry).unwrap();
    assert_eq!(&bytes[..8], b"MARCHF05");
    let (loaded, entry2) = Program::from_image(&bytes).unwrap();
    assert_eq!(bytes, loaded.to_image(entry2).unwrap());
    assert_eq!(run(&loaded, entry2, &[]), [Value::Int(9)]);
    assert!(
        loaded
            .definition(loaded.lookup("saved").unwrap())
            .unwrap()
            .is_some()
    );
    assert_eq!(named_cid(&p, "saved"), named_cid(&loaded, "saved"));
}

#[test]
fn definition_images_are_independent_of_insertion_order() {
    let mut a = stream::seed().unwrap();
    let mut b = stream::seed().unwrap();
    let ae = stream::compile(&mut a, ": a 1 ; : b 2 ; a b +").unwrap();
    let be = stream::compile(&mut b, ": b 2 ; : a 1 ; a b +").unwrap();
    assert_eq!(a.to_image(ae).unwrap(), b.to_image(be).unwrap());
}

#[test]
fn march_rebuilds_its_seed_across_images_with_stable_definitions() {
    let mut p = stream::seed().unwrap();
    let seed = include_str!("../src/fast/stream-seed.march");
    // Each generation uses the currently installed March interpreter to
    // rebuild the next one, rather than running the native seed assembler.
    let mut previous = None;
    for _ in 0..3 {
        stream::compile(&mut p, seed).unwrap();
        // Rebind the defining-word entry points to their rebuilt definitions.
        for (name, implementation) in [
            (":", "seed.colon"),
            ("(", "seed.comment"),
            ("'", "seed.quote"),
            ("quote", "seed.quote"),
            ("ctx", "seed.context"),
            ("recur", "seed.recur"),
            ("apply", "seed.apply"),
            ("family", "seed.family"),
        ] {
            p.bind(name, p.lookup(implementation).unwrap()).unwrap();
            p.mark_immediate(name).unwrap();
        }
        let interpreter = p.lookup("stream.interpret").unwrap();
        let bytes = p.to_image(interpreter).unwrap();
        if let Some(previous) = previous {
            assert_eq!(bytes, previous);
        }
        previous = Some(bytes.clone());
        p = Program::from_image(&bytes).unwrap().0;
    }
    let entry = stream::compile(&mut p, ": square dup * ; : 42 999 ; 7 square 42").unwrap();
    assert_eq!(run(&p, entry, &[]), [Value::Int(49), Value::Int(42)]);
}

fn definition_image(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"MARCHF05".to_vec();
    bytes.extend_from_slice(&(records.len() as u64).to_le_bytes());
    let mut entry = None;
    for record in records {
        let cid = Cid::digest(b"march-definition-v1", record);
        bytes.extend_from_slice(&cid.0);
        bytes.extend_from_slice(&(record.len() as u64).to_le_bytes());
        bytes.extend_from_slice(record);
        entry = Some(cid);
    }
    bytes.extend_from_slice(&0u64.to_le_bytes()); // dictionary
    bytes.extend_from_slice(&0u64.to_le_bytes()); // immediate names
    bytes.extend_from_slice(&entry.unwrap().0);
    bytes
}

#[test]
fn rehashed_malformed_definitions_are_rejected_without_execution() {
    let mut sequence = vec![2];
    sequence.extend_from_slice(&1u64.to_le_bytes());
    let mut cases = vec![vec![255], vec![1, 255], vec![3, 0, 0, 0, 0, 0, 0, 0, 0]];
    // Unknown semantic primitive, even when the record's digest is correct.
    let mut unknown = vec![0];
    unknown.extend_from_slice(&3u64.to_le_bytes());
    unknown.extend_from_slice(b"wat");
    cases.push(unknown);
    for item in [
        vec![255],
        vec![3],
        vec![1, 1, 2],
        {
            let mut item = vec![0];
            item.extend_from_slice(&[0; 32]);
            item
        },
        {
            let mut item = vec![4];
            item.extend_from_slice(&u64::MAX.to_le_bytes());
            item.extend_from_slice(&1u64.to_le_bytes());
            item
        },
    ] {
        let mut record = sequence.clone();
        record.extend(item);
        cases.push(record);
    }
    for record in cases {
        assert!(Program::from_image(&definition_image(&[record])).is_err());
    }
    let empty = vec![2, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(Program::from_image(&definition_image(&[empty.clone(), empty.clone()])).is_err());
    let mut trailing = empty;
    trailing.push(0);
    assert!(Program::from_image(&definition_image(&[trailing])).is_err());
}

#[test]
fn definition_images_reject_truncation_corruption_and_wrong_record_domain() {
    let mut p = Program::new();
    let w = p
        .add_definition(Definition::Sequence(vec![Item::Literal(Literal::Int(7))]))
        .unwrap();
    let bytes = p.to_image(w).unwrap();
    assert_eq!(
        Program::from_image(&bytes).unwrap().0.to_image(0).unwrap(),
        bytes
    );
    for n in 0..bytes.len() {
        assert!(Program::from_image(&bytes[..n]).is_err());
    }
    for offset in [16, 48, 56, bytes.len() - 1] {
        let mut corrupted = bytes.clone();
        corrupted[offset] ^= 0xff;
        assert!(Program::from_image(&corrupted).is_err());
    }
    let mut wrong_domain = bytes;
    let wrong = Cid::digest(b"march-evaluator-graph-v1", &wrong_domain[56..75]);
    wrong_domain[16..48].copy_from_slice(&wrong.0);
    assert!(Program::from_image(&wrong_domain).is_err());
}

#[test]
fn graph_fixtures_cannot_be_saved_or_referenced_by_definitions() {
    let mut p = Program::new();
    let w = p
        .add_word(0, vec![Op::Const(Literal::Int(7))], vec![0])
        .unwrap();
    assert_eq!(run(&p, w, &[]), [Value::Int(7)]);
    assert!(p.to_image(w).is_err());
    for item in [Item::Word(w), Item::Literal(Literal::Quote(w))] {
        assert!(p.add_definition(Definition::Sequence(vec![item])).is_err());
    }
}
#[test]
fn all_obsolete_image_versions_are_rejected() {
    let mut p = Program::new();
    let w = p.add_definition(Definition::Sequence(vec![])).unwrap();
    for version in [b"MARCHF01", b"MARCHF02", b"MARCHF03", b"MARCHF04"] {
        let mut bytes = p.to_image(w).unwrap();
        bytes[..8].copy_from_slice(version);
        assert!(Program::from_image(&bytes).is_err());
    }
}

#[test]
fn rehashed_family_with_mismatched_clause_signatures_is_rejected_on_load() {
    let mut empty = vec![2];
    empty.extend_from_slice(&0u64.to_le_bytes());
    let mut yes = vec![2];
    yes.extend_from_slice(&1u64.to_le_bytes());
    yes.extend_from_slice(&[1, 1, 1]);
    let empty_cid = Cid::digest(b"march-definition-v1", &empty);
    let yes_cid = Cid::digest(b"march-definition-v1", &yes);
    let mut family = vec![3];
    family.extend_from_slice(&2u64.to_le_bytes());
    for cid in [yes_cid, empty_cid, yes_cid, yes_cid] {
        family.extend_from_slice(&cid.0);
    }
    assert!(Program::from_image(&definition_image(&[empty, yes, family])).is_err());
}
#[test]
fn host_and_stream_readers_agree_on_definition_identity_and_quote_equality() {
    let source = ": a dup * ; : b dup dup * swap drop ; ' a ' b =";
    let mut host = Program::new();
    let h = march_research::fast::source::compile(&mut host, source).unwrap();
    let mut p = stream::seed().unwrap();
    let s = stream::compile(&mut p, source).unwrap();
    assert_eq!(host.cid(h), p.cid(s));
    assert_eq!(named_cid(&host, "a"), named_cid(&p, "a"));
    assert_eq!(run(&host, h, &[]), [Value::Bool(false)]);
    assert_eq!(run(&p, s, &[]), [Value::Bool(false)]);
}
