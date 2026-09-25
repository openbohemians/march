use march_research::fast::{Context, Error, Executor, Handle, Literal, Op, Program, Value, source};

fn stream() -> (Program, usize) {
    let mut p = Program::new();
    let word = source::compile(&mut p, ": from dup 1 + recur 1 1 pair ; from").unwrap();
    (p, word)
}
fn advance(e: &mut Executor<'_>, cursor: Handle, n: i64) -> Handle {
    let Value::Pair(head, tail) = e.force(cursor).unwrap() else {
        panic!("pair")
    };
    assert_eq!(e.force(head), Ok(Value::Int(n)));
    tail
}

#[test]
fn streaming_prefix_is_reclaimed_and_storage_stays_bounded() {
    let (p, word) = stream();
    let mut e = Executor::new(&p);
    e.cell_limit = 512;
    e.argument_limit = 128;
    let mut cursor = e
        .start(word, &[Literal::Int(0)], &Context::new(), 2_000_000)
        .unwrap()[0];
    let mut plateau = None;
    for i in 0..10_000 {
        cursor = advance(&mut e, cursor, i);
        if i % 32 == 31 {
            let old = cursor;
            cursor = e.collect(&[cursor], 10_000).unwrap()[0];
            assert_eq!(e.force(old), Err(Error::StaleHandle));
            let s = e.storage();
            assert!(s.cells <= 16 && s.frames <= 2, "{s:?}");
            let size = (s.cells, s.frames, s.arguments, s.vector_bytes);
            assert_eq!(*plateau.get_or_insert(size), size);
        }
    }
    assert_eq!(e.stats().primitive_ops, 9_999);
    assert_eq!(e.stats().tail_iterations, 0);
    assert!(e.stats().peak_cells < 256);
    assert!(e.stats().collected_cells > 50_000);
}

#[test]
fn slow_consumer_pins_only_its_window_and_shares_evaluation() {
    let (p, word) = stream();
    let mut e = Executor::new(&p);
    e.cell_limit = 1024;
    let mut fast = e
        .start(word, &[Literal::Int(0)], &Context::new(), 1_000_000)
        .unwrap()[0];
    let mut slow = fast;
    for i in 0..1000 {
        fast = advance(&mut e, fast, i);
        if i >= 32 {
            slow = advance(&mut e, slow, i - 32);
        }
        if i % 16 == 15 {
            let roots = e.collect(&[fast, slow], 20_000).unwrap();
            fast = roots[0];
            slow = roots[1];
            assert!(e.storage().cells < 256, "{:?}", e.storage());
        }
    }
    assert_eq!(e.stats().primitive_ops, 999);
    let before = e.storage().cells;
    fast = e.collect(&[fast], 20_000).unwrap()[0];
    assert!(e.storage().cells < before / 2);
    assert_eq!(e.force(slow), Err(Error::StaleHandle));
    advance(&mut e, fast, 1000);
    assert_eq!(e.stats().primitive_ops, 1000);
}

#[test]
fn keeping_the_head_legitimately_retains_history_until_released() {
    let (p, word) = stream();
    let mut e = Executor::new(&p);
    let mut head = e
        .start(word, &[Literal::Int(0)], &Context::new(), 100_000)
        .unwrap()[0];
    let mut cursor = head;
    for i in 0..100 {
        cursor = advance(&mut e, cursor, i);
        let roots = e.collect(&[head, cursor], 20_000).unwrap();
        head = roots[0];
        cursor = roots[1];
    }
    assert!(e.storage().cells >= 100);
    advance(&mut e, head, 0);
    cursor = e.collect(&[cursor], 20_000).unwrap()[0];
    assert!(e.storage().cells <= 16);
    advance(&mut e, cursor, 100);
}

#[test]
fn collection_does_not_force_dormant_work_and_keeps_error_memos() {
    let mut p = Program::new();
    let w = source::compile(
        &mut p,
        ": loop recur 0 1 ;
        7 loop pair 9223372036854775807 1 +",
    )
    .unwrap();
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 1000).unwrap();
    assert_eq!(e.force(h[1]), Err(Error::Overflow));
    let ops = e.stats().primitive_ops;
    let h = e.collect(&[h[1], h[0], h[1]], 1000).unwrap();
    assert_eq!(h[0], h[2]);
    assert_eq!(e.stats().primitive_ops, ops);
    assert_eq!(e.force(h[0]), Err(Error::Overflow));
    let Value::Pair(a, _) = e.force(h[1]).unwrap() else {
        panic!("pair")
    };
    let a = e.collect(&[a], 1000).unwrap()[0];
    assert_eq!(e.force(a), Ok(Value::Int(7)));
    assert_eq!(e.storage().failures, 0);
    assert_eq!(e.stats().primitive_ops, ops);
}

#[test]
fn invalid_roots_and_collection_budget_are_atomic() {
    let (p, w) = stream();
    let mut a = Executor::new(&p);
    let mut b = Executor::new(&p);
    let h = a
        .start(w, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    let foreign = b
        .start(w, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    let storage = a.storage();
    let stats = a.stats().clone();
    assert_eq!(a.collect(&[h, foreign], 1000), Err(Error::StaleHandle));
    assert_eq!(a.collect(&[h], 0), Err(Error::Budget));
    assert_eq!(a.storage(), storage);
    assert_eq!(*a.stats(), stats);
    advance(&mut a, h, 0);
    for budget in 0..100 {
        let storage = a.storage();
        let stats = a.stats().clone();
        match a.collect(&[h], budget) {
            Err(Error::Budget) => {
                assert_eq!(a.storage(), storage);
                assert_eq!(*a.stats(), stats);
            }
            Ok(roots) => {
                advance(&mut a, roots[0], 0);
                return;
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("small heap should fit");
}

#[test]
fn detached_argument_alias_still_preserves_recursive_cycle_identity() {
    let mut p = Program::new();
    let w = p
        .add_word(
            1,
            vec![
                Op::Arg(0),
                Op::Recur { arguments: vec![0] },
                Op::Project { call: 1, output: 1 },
            ],
            vec![0, 2],
        )
        .unwrap();
    let mut e = Executor::new(&p);
    let h = e
        .start(w, &[Literal::Int(7)], &Context::new(), 1000)
        .unwrap();
    assert_eq!(e.force(h[0]), Ok(Value::Int(7)));
    let h = e.collect(&[h[1]], 1000).unwrap()[0];
    assert_eq!(e.force(h), Err(Error::Cycle));
    let h = e.collect(&[h], 1000).unwrap()[0];
    assert_eq!(e.force(h), Err(Error::Cycle));
    assert_eq!(e.storage().frames, 0);
}

#[test]
fn call_bundle_preserves_independent_outputs_and_value_identity() {
    let mut p = Program::new();
    let w = source::compile(&mut p, ": both dup * 7 ; 5 both pair").unwrap();
    let mut e = Executor::new(&p);
    let h = e.start(w, &[], &Context::new(), 1000).unwrap()[0];
    let Value::Pair(a, _) = e.force(h).unwrap() else {
        panic!("pair")
    };
    assert_eq!(e.force(a), Ok(Value::Int(25)));
    let h = e.collect(&[h], 1000).unwrap()[0];
    let cid = e.content_id(h).unwrap();
    let h = e.collect(&[h], 1000).unwrap()[0];
    assert_eq!(e.content_id(h), Ok(cid));
    assert_eq!(e.stats().primitive_ops, 1);
    let Value::Pair(a, b) = e.force(h).unwrap() else {
        panic!("pair")
    };
    assert_eq!(e.force(a), Ok(Value::Int(25)));
    assert_eq!(e.force(b), Ok(Value::Int(7)));
}

#[test]
fn empty_roots_release_runtime_storage_without_changing_code_images() {
    let (p, w) = stream();
    let image = p.to_image(w).unwrap();
    let mut e = Executor::new(&p);
    let h = e
        .start(w, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    advance(&mut e, h, 0);
    assert!(e.collect(&[], 1000).unwrap().is_empty());
    assert_eq!(e.storage().cells, 0);
    assert_eq!(e.storage().frames, 0);
    assert_eq!(e.storage().arguments, 0);
    assert_eq!(e.force(h), Err(Error::StaleHandle));
    assert_eq!(p.to_image(w).unwrap(), image);
}

#[test]
fn collection_cannot_revive_aborted_evaluation_or_refill_fuel() {
    let (p, w) = stream();
    let mut e = Executor::new(&p);
    let h = e.start(w, &[Literal::Int(0)], &Context::new(), 0).unwrap()[0];
    let h = e.collect(&[h], 1000).unwrap()[0];
    assert_eq!(e.force(h), Err(Error::Budget));
    let storage = e.storage();
    let stats = e.stats().clone();
    assert_eq!(e.collect(&[h], 1000), Err(Error::Budget));
    assert_eq!(e.storage(), storage);
    assert_eq!(*e.stats(), stats);
}

#[test]
fn every_output_observation_can_be_separated_by_collection() {
    for source_text in [
        ": both dup * dup 1 + ; 5 both",
        ": both dup * ctx missing ; 5 ' both apply 1 2",
        ": bad 9223372036854775807 1 + ; bad 7 bad",
        "true 7 ctx missing select 6 7 * dup",
        ": f dup 1 + ; 2 f dup 1 + swap 2 +",
        ": sevens dup recur 1 1 pair ; 7 sevens first 9",
    ] {
        let mut p = Program::new();
        let word = source::compile(&mut p, source_text).unwrap();
        let mut control = Executor::new(&p);
        let expected_handles = control.start(word, &[], &Context::new(), 10_000).unwrap();
        let mut e = Executor::new(&p);
        let mut handles = e.start(word, &[], &Context::new(), 10_000).unwrap();
        for index in (0..handles.len()).rev().chain(0..handles.len()) {
            handles = e.collect(&handles, 20_000).unwrap();
            assert_eq!(
                e.force(handles[index]),
                control.force(expected_handles[index]),
                "{source_text}"
            );
            assert_eq!(
                e.stats().primitive_ops,
                control.stats().primitive_ops,
                "{source_text}"
            );
        }
    }
}

#[test]
fn storage_limit_failure_of_collection_is_atomic_and_aborts_stay_aborted() {
    let (p, word) = stream();
    let mut e = Executor::new(&p);
    let h = e
        .start(word, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    e.cell_limit = 0;
    let before = e.storage();
    let stats = e.stats().clone();
    assert_eq!(e.collect(&[h], 1000), Err(Error::StorageLimit));
    assert_eq!(e.storage(), before);
    assert_eq!(*e.stats(), stats);
    e.cell_limit = 7;
    assert_eq!(e.force(h), Err(Error::StorageLimit));
    assert_eq!(e.collect(&[], 1000), Err(Error::StorageLimit));
    e.cell_limit = 1024;
    let h = e
        .start(word, &[Literal::Int(0)], &Context::new(), 1000)
        .unwrap()[0];
    advance(&mut e, h, 0);
}
