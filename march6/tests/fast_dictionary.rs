use march_research::fast::{Context, Executor, Program, Value, stream};
use std::collections::BTreeMap;

#[test]
fn persistent_bindings_and_flags_leave_earlier_snapshots_unchanged() {
    let original = stream::seed().unwrap();
    let read = original.lookup("stream.word").unwrap();
    let find = original.lookup("stream.find").unwrap();
    let mut a = original.clone();
    a.bind("read-next", read).unwrap();
    a.mark_immediate("read-next").unwrap();
    let before = a.to_image(read).unwrap();
    let mut b = a.clone();
    b.bind("read-next", find).unwrap();
    b.bind("命名空间.word", read).unwrap();
    b.mark_immediate("命名空间.word").unwrap();
    assert_eq!(original.lookup("read-next"), None);
    assert_eq!(a.lookup("read-next"), Some(read));
    assert!(a.is_immediate("read-next"));
    assert_eq!(a.lookup("命名空间.word"), None);
    assert_eq!(b.lookup("read-next"), Some(find));
    assert!(!b.is_immediate("read-next"));
    assert!(b.is_immediate("命名空间.word"));
    assert_eq!(a.to_image(read).unwrap(), before);
}

#[test]
fn retained_dictionary_versions_match_an_independent_ordered_map_model() {
    let mut p = stream::seed().unwrap();
    let a = p.lookup("dup").unwrap();
    let b = p.lookup("swap").unwrap();
    let mut model = BTreeMap::new();
    let mut snapshots = Vec::new();
    for step in 0..256 {
        let name = format!("ns.word-{}", step * 97 % 73);
        let id = if step % 2 == 0 { a } else { b };
        p.bind(&name, id).unwrap();
        model.insert(name, id);
        if step % 16 == 0 {
            snapshots.push((p.clone(), model.clone()));
        }
    }
    for (snapshot, expected) in snapshots {
        for i in 0..73 {
            let name = format!("ns.word-{i}");
            assert_eq!(snapshot.lookup(&name), expected.get(&name).copied());
        }
    }
}

#[test]
fn images_ignore_hash_seed_insertion_order_and_rebinding_history() {
    let mut expected = None;
    for pass in 0..6 {
        let mut p = stream::seed().unwrap();
        let word = p.lookup("stream.word").unwrap();
        let other = p.lookup("stream.find").unwrap();
        let mut indices: Vec<_> = (0..100).collect();
        indices.rotate_left(pass * 13);
        if pass % 2 == 1 {
            indices.reverse();
        }
        for i in indices {
            let name = format!("namespace.word-{i:03}");
            p.bind(&name, other).unwrap();
            p.mark_immediate(&name).unwrap();
            p.bind(&name, word).unwrap();
            if i % 3 == 0 {
                p.mark_immediate(&name).unwrap();
            }
        }
        let bytes = p.to_image(word).unwrap();
        if let Some(expected) = &expected {
            assert_eq!(&bytes, expected);
        } else {
            expected = Some(bytes.clone());
        }
        let (loaded, entry) = Program::from_image(&bytes).unwrap();
        assert_eq!(loaded.to_image(entry).unwrap(), bytes);
        assert!(loaded.is_immediate("namespace.word-003"));
        assert!(!loaded.is_immediate("namespace.word-004"));
    }
}

#[test]
fn rebinding_in_one_snapshot_does_not_retarget_compiled_dependencies() {
    let mut a = stream::seed().unwrap();
    let old = stream::compile(&mut a, ": value 7 ; : retained value ; retained").unwrap();
    let bytes = a.to_image(old).unwrap();
    let mut b = a.clone();
    let new = stream::compile(&mut b, ": value 9 ; retained value").unwrap();
    assert_eq!(
        Executor::new(&a)
            .run(old, &[], &Context::new(), 1000)
            .unwrap(),
        [Value::Int(7)]
    );
    assert_eq!(
        Executor::new(&b)
            .run(new, &[], &Context::new(), 1000)
            .unwrap(),
        [Value::Int(7), Value::Int(9)]
    );
    assert_eq!(a.to_image(old).unwrap(), bytes);
}

#[test]
fn failed_compilation_discards_dictionary_and_flag_changes_atomically() {
    let mut p = stream::seed().unwrap();
    let root = stream::compile(&mut p, "7").unwrap();
    let before = p.to_image(root).unwrap();
    assert!(stream::compile(&mut p, ": helper stream.word ; immediate unknown-word").is_err());
    assert!(p.lookup("helper").is_none());
    assert!(!p.is_immediate("helper"));
    assert_eq!(p.to_image(root).unwrap(), before);
}
