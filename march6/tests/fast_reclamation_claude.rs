//! Semantic fixtures a future reclamation scheme must keep, expressed through
//! the current public API only (`start`/`force`/`content_id`, never `run`, so
//! the scalar tail plan cannot mask heap behavior).
//!
//! Retention counters recorded here are MEASUREMENTS with upper bounds only;
//! a collector may lower them freely. The first nine fixtures use no
//! collection API at all; the `collect()` section at the end exercises the
//! explicit boundary collector (`Executor::collect`) that landed afterwards.
use march_research::fast::{Context, Error, Executor, Handle, Literal, Program, Value, stream};

const BUDGET: usize = 200_000;

fn compile(text: &str) -> (Program, usize) {
    let mut program = stream::seed().unwrap();
    let word = stream::compile(&mut program, text).unwrap();
    (program, word)
}

const STREAM: &str = ": from dup 1 + recur 1 1 pair ;";

fn pair(executor: &mut Executor<'_>, handle: Handle) -> (Handle, Handle) {
    match executor.force(handle) {
        Ok(Value::Pair(head, rest)) => (head, rest),
        other => panic!("expected a pair, got {other:?}"),
    }
}

fn int(executor: &mut Executor<'_>, handle: Handle) -> i64 {
    match executor.force(handle) {
        Ok(Value::Int(n)) => n,
        other => panic!("expected an integer, got {other:?}"),
    }
}

/// Walks `steps` cells down a stream, returning the handle of the cell reached.
fn walk(executor: &mut Executor<'_>, mut cell: Handle, steps: usize) -> Handle {
    for _ in 0..steps {
        cell = pair(executor, cell).1;
    }
    cell
}

#[test]
fn consumers_at_different_positions_share_one_stream_instance() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let mut executor = Executor::new(&program);
    let head = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    // Consumer A reads positions 0..=5 by walking; each new position costs
    // exactly one addition.
    let mut cell = head;
    for expected in 0..=5 {
        let (value, rest) = pair(&mut executor, cell);
        assert_eq!(int(&mut executor, value), expected);
        assert_eq!(executor.stats().primitive_ops as i64, expected);
        cell = rest;
    }
    // Consumer B starts from the same head and reads position 2: no new work.
    let b = walk(&mut executor, head, 2);
    let (value, _) = pair(&mut executor, b);
    assert_eq!(int(&mut executor, value), 2);
    assert_eq!(executor.stats().primitive_ops, 5);
    // Consumer B then overtakes A to position 7: exactly two more additions.
    let b = walk(&mut executor, b, 5);
    let (value, _) = pair(&mut executor, b);
    assert_eq!(int(&mut executor, value), 7);
    assert_eq!(executor.stats().primitive_ops, 7);
    // A catching up to 7 costs nothing (A's cursor already points at cell 6).
    let a = walk(&mut executor, cell, 1);
    let (value, _) = pair(&mut executor, a);
    assert_eq!(int(&mut executor, value), 7);
    assert_eq!(executor.stats().primitive_ops, 7);
}

#[test]
fn a_surviving_consumer_is_unaffected_when_another_use_is_dropped() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let mut executor = Executor::new(&program);
    let head = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let behind = walk(&mut executor, head, 3);
    {
        // A fast consumer walks far ahead, then its handles go out of scope.
        let ahead = walk(&mut executor, behind, 40);
        let (value, _) = pair(&mut executor, ahead);
        assert_eq!(int(&mut executor, value), 43);
    }
    assert_eq!(executor.stats().primitive_ops, 43);
    // The consumer left behind still reads its position and everything after
    // it, and nothing already computed is recomputed.
    let (value, rest) = pair(&mut executor, behind);
    assert_eq!(int(&mut executor, value), 3);
    let later = walk(&mut executor, rest, 39);
    let (value, _) = pair(&mut executor, later);
    assert_eq!(int(&mut executor, value), 43);
    assert_eq!(executor.stats().primitive_ops, 43);
    // Beyond the frontier, work resumes at one addition per new cell.
    let beyond = walk(&mut executor, later, 3);
    let (value, _) = pair(&mut executor, beyond);
    assert_eq!(int(&mut executor, value), 46);
    assert_eq!(executor.stats().primitive_ops, 46);
}

#[test]
fn a_failing_field_leaves_the_shared_field_usable_for_every_consumer() {
    let (program, word) = compile(": bad 9223372036854775807 1 + ; : p bad 7 pair ; p dup");
    let mut executor = Executor::new(&program);
    let handles = executor.start(word, &[], &Context::new(), BUDGET).unwrap();
    assert_eq!(handles.len(), 2);
    let (bad_a, good_a) = pair(&mut executor, handles[0]);
    let (bad_b, good_b) = pair(&mut executor, handles[1]);
    assert_eq!(int(&mut executor, good_a), 7);
    assert_eq!(executor.force(bad_a), Err(Error::Overflow));
    assert_eq!(executor.force(bad_b), Err(Error::Overflow));
    assert_eq!(int(&mut executor, good_b), 7);
    assert_eq!(
        executor.stats().primitive_ops,
        1,
        "one failed addition, memoized"
    );
    assert_eq!(executor.content_id(handles[1]), Err(Error::Overflow));
    assert_eq!(int(&mut executor, good_a), 7);
}

#[test]
fn unused_divergent_and_erroneous_paths_stay_dormant_under_selective_forcing() {
    // An infinite stream whose tail is never demanded costs nothing.
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let mut executor = Executor::new(&program);
    let head = executor.start(word, &[], &Context::new(), 50).unwrap()[0];
    let (value, _rest) = pair(&mut executor, head);
    assert_eq!(int(&mut executor, value), 0);
    assert_eq!(executor.stats().primitive_ops, 0);
    assert!(
        executor.stats().peak_frames <= 2,
        "{}",
        executor.stats().peak_frames
    );

    // A pair holding a spinning computation and an overflow beside a value.
    let text = ": spin recur 0 1 ; : bad 9223372036854775807 1 + ; spin bad pair 7 pair";
    let (program, word) = compile(text);
    let mut executor = Executor::new(&program);
    let outer = executor.start(word, &[], &Context::new(), 50).unwrap()[0];
    let (inner, seven) = pair(&mut executor, outer);
    assert_eq!(int(&mut executor, seven), 7);
    let (_spin, _bad) = pair(&mut executor, inner);
    assert_eq!(executor.stats().primitive_ops, 0);
    assert!(
        executor.stats().steps < 50,
        "no budget spent on dormant work"
    );
    assert_eq!(executor.force(_bad), Err(Error::Overflow));
    assert_eq!(int(&mut executor, seven), 7);
}

#[test]
fn duplicate_consumers_of_one_pending_instance_compute_it_once() {
    let (program, word) = compile(": square dup * ; 7 square dup");
    let mut executor = Executor::new(&program);
    let handles = executor.start(word, &[], &Context::new(), BUDGET).unwrap();
    assert_eq!(
        handles[0], handles[1],
        "dup of a pending cell is the same cell"
    );
    assert_eq!(int(&mut executor, handles[1]), 49);
    assert_eq!(int(&mut executor, handles[0]), 49);
    assert_eq!(executor.stats().primitive_ops, 1);

    let (program, word) = compile(": square dup * ; 7 square dup 1 + swap 2 +");
    let mut executor = Executor::new(&program);
    let handles = executor.start(word, &[], &Context::new(), BUDGET).unwrap();
    assert_ne!(handles[0], handles[1]);
    assert_eq!(
        int(&mut executor, handles[0]),
        50,
        "results are bottom-to-top"
    );
    assert_eq!(int(&mut executor, handles[1]), 51);
    assert_eq!(
        executor.stats().primitive_ops,
        3,
        "square once, two additions"
    );
}

#[test]
fn reset_isolates_context_memo_and_handles_between_invocations() {
    let (program, word) = compile(": read ctx n ; read dup +");
    let mut executor = Executor::new(&program);
    let context = |n: i64| Context::from([("n".to_string(), Literal::Int(n))]);
    let first = executor.start(word, &[], &context(3), BUDGET).unwrap()[0];
    assert_eq!(int(&mut executor, first), 6);
    let second = executor.start(word, &[], &context(5), BUDGET).unwrap()[0];
    assert_eq!(executor.force(first), Err(Error::StaleHandle));
    assert_eq!(int(&mut executor, second), 10);
    assert_eq!(
        executor.stats().primitive_ops,
        1,
        "stats and memo restart per invocation"
    );
    assert_eq!(executor.content_id(first), Err(Error::StaleHandle));
    // A second executor over the same program never accepts the first's handles.
    let mut other = Executor::new(&program);
    let _ = other.start(word, &[], &context(9), BUDGET).unwrap();
    assert_eq!(other.force(second), Err(Error::StaleHandle));
}

#[test]
fn productive_same_argument_recursion_is_data_but_bare_re_entry_is_a_cycle() {
    // Under a constructor, recurring on the same argument is a productive
    // infinite stream of sevens: every level is a completed pair, so no busy
    // cell is re-entered.
    let (program, word) = compile(": sevens dup recur 1 1 pair ; 7 sevens");
    let mut executor = Executor::new(&program);
    let head = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let cell = walk(&mut executor, head, 25);
    let (value, _) = pair(&mut executor, cell);
    assert_eq!(int(&mut executor, value), 7);
    assert_eq!(executor.stats().primitive_ops, 0);
    // Without the constructor the same recursion demands itself: Cycle, and
    // the error is stable across repeated forcing rather than turning into fuel.
    let (program, word) = compile(": same recur 1 1 ; 7 same");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    assert_eq!(executor.force(handle), Err(Error::Cycle));
    assert_eq!(executor.force(handle), Err(Error::Cycle));
    assert!(executor.stats().steps < 100);
    // A changed argument under no constructor is fuel, not a cycle.
    let (program, word) = compile(": grow 1 + recur 1 1 ; 7 grow");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), 500).unwrap()[0];
    assert_eq!(executor.force(handle), Err(Error::Budget));
}

#[test]
fn stream_walk_retention_is_linear_today_measured_not_required() {
    // Upper bounds only: a collector may reduce these without breaking the test.
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let mut measurements = Vec::new();
    for steps in [100usize, 1_000, 4_000] {
        let mut executor = Executor::new(&program);
        let head = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
        let cell = walk(&mut executor, head, steps);
        let (value, _) = pair(&mut executor, cell);
        assert_eq!(int(&mut executor, value) as usize, steps);
        assert_eq!(executor.stats().primitive_ops, steps);
        let stats = executor.stats();
        measurements.push((steps, stats.peak_cells, stats.peak_frames));
        assert!(stats.peak_cells <= 12 * steps + 16, "{stats:?}");
        assert!(stats.peak_frames <= 2 * steps + 4, "{stats:?}");
    }
    eprintln!("stream retention (steps, peak_cells, peak_frames): {measurements:?}");
}

#[test]
fn budget_exhaustion_mid_stream_aborts_this_invocation_only() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let mut executor = Executor::new(&program);
    let head = executor.start(word, &[], &Context::new(), 60).unwrap()[0];
    let mut cell = head;
    let mut reached = 0;
    loop {
        match executor.force(cell) {
            Ok(Value::Pair(_, rest)) => {
                reached += 1;
                cell = rest;
            }
            Err(Error::Budget) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(reached > 0);
    assert_eq!(executor.force(head), Err(Error::Budget), "abort is sticky");
    let head = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let cell = walk(&mut executor, head, reached + 5);
    let (value, _) = pair(&mut executor, cell);
    assert_eq!(int(&mut executor, value) as usize, reached + 5);
}

// ------------------------------------------------------------ collect()
//
// Explicit boundary collection: `collect(roots, budget)` keeps exactly what
// the roots reach, returns replacement handles in a fresh epoch, and changes
// nothing on failure.

fn stream_executor(program: &Program, word: usize, budget: usize) -> (Executor<'_>, Handle) {
    let mut executor = Executor::new(program);
    let head = executor.start(word, &[], &Context::new(), budget).unwrap()[0];
    (executor, head)
}

#[test]
fn collect_with_only_the_cursor_keeps_a_constant_window_and_old_handles_stay_stale() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let (mut executor, head) = stream_executor(&program, word, 2_000_000);
    executor.cell_limit = 400;
    let mut cursor = head;
    let mut plateau = None;
    let mut position = 0i64;
    for round in 0..300 {
        for _ in 0..50 {
            let (value, rest) = pair(&mut executor, cursor);
            assert_eq!(int(&mut executor, value), position);
            position += 1;
            cursor = rest;
        }
        let before = executor.storage();
        assert!(before.cells > 250, "round {round}: {before:?}");
        let stale = cursor;
        cursor = executor.collect(&[cursor], 100_000).unwrap()[0];
        let after = executor.storage();
        assert!(
            after.cells <= 12 && after.frames <= 2,
            "round {round}: {after:?}"
        );
        // The old handle's index is almost surely live again in the small new
        // heap, and it must still be refused: staleness is by epoch, not index.
        assert_eq!(executor.force(stale), Err(Error::StaleHandle));
        assert_eq!(executor.content_id(stale), Err(Error::StaleHandle));
        assert_eq!(executor.collect(&[stale], 1_000), Err(Error::StaleHandle));
        let size = (after.cells, after.frames, after.arguments);
        assert_eq!(*plateau.get_or_insert(size), size, "round {round}");
    }
    assert_eq!(
        executor.stats().primitive_ops,
        14_999,
        "head 0 is a literal"
    );
    assert_eq!(executor.stats().collections, 300);
    assert!(executor.stats().collected_cells > 80_000);
}

#[test]
fn two_consumers_keep_exactly_their_window_and_releasing_one_frees_its_prefix() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let (mut executor, head) = stream_executor(&program, word, 2_000_000);
    let mut fast = head;
    let mut slow = head;
    let mut fast_position = 0i64;
    let mut slow_position = 0i64;
    let distance = 64;
    for _ in 0..distance {
        let (value, rest) = pair(&mut executor, fast);
        assert_eq!(int(&mut executor, value), fast_position);
        fast_position += 1;
        fast = rest;
    }
    let mut window = None;
    for round in 0..40 {
        for _ in 0..16 {
            let (value, rest) = pair(&mut executor, fast);
            assert_eq!(int(&mut executor, value), fast_position);
            fast_position += 1;
            fast = rest;
            let (value, rest) = pair(&mut executor, slow);
            assert_eq!(int(&mut executor, value), slow_position);
            slow_position += 1;
            slow = rest;
        }
        let roots = executor.collect(&[fast, slow], 100_000).unwrap();
        fast = roots[0];
        slow = roots[1];
        let storage = executor.storage();
        assert!(
            storage.cells <= 8 * distance + 16,
            "round {round}: {storage:?}"
        );
        assert!(storage.frames <= distance + 3, "round {round}: {storage:?}");
        let size = (storage.cells, storage.frames);
        assert_eq!(*window.get_or_insert(size), size, "round {round}");
    }
    // The slow consumer never recomputed anything the fast one produced.
    assert_eq!(executor.stats().primitive_ops as i64, fast_position - 1);
    // Releasing the slow consumer (not rooting it) frees its prefix.
    let stale_slow = slow;
    fast = executor.collect(&[fast], 100_000).unwrap()[0];
    let storage = executor.storage();
    assert!(storage.cells <= 12 && storage.frames <= 2, "{storage:?}");
    assert_eq!(executor.force(stale_slow), Err(Error::StaleHandle));
    let (value, _) = pair(&mut executor, fast);
    assert_eq!(int(&mut executor, value), fast_position);
    assert_eq!(executor.stats().primitive_ops as i64, fast_position);
}

#[test]
fn a_shared_pending_computation_survives_when_its_other_consumer_is_dropped() {
    let (program, word) = compile(": square dup * ; 7 square dup 1 + swap 2 +");
    let mut executor = Executor::new(&program);
    let handles = executor.start(word, &[], &Context::new(), BUDGET).unwrap();
    // Nothing forced yet: the square is a pending cell shared by two branches.
    assert_eq!(executor.stats().primitive_ops, 0);
    let kept = executor.collect(&[handles[1]], 10_000).unwrap()[0];
    assert_eq!(executor.force(handles[0]), Err(Error::StaleHandle));
    assert_eq!(int(&mut executor, kept), 51);
    assert_eq!(
        executor.stats().primitive_ops,
        2,
        "square once, one addition"
    );
    // The dropped branch's own addition was never run and cannot be reached.
    let kept = executor.collect(&[kept], 10_000).unwrap()[0];
    assert_eq!(int(&mut executor, kept), 51);
    assert_eq!(executor.stats().primitive_ops, 2);
    assert_eq!(
        executor.storage().frames,
        0,
        "a ready scalar needs no frame"
    );
}

#[test]
fn cycle_identity_is_stable_across_collections_in_both_directions() {
    // Productive same-argument recursion stays productive after any number of
    // collections: no false Cycle from relocated or detached argument cells.
    let (program, word) = compile(": sevens dup recur 1 1 pair ; 7 sevens");
    let (mut executor, mut cursor) = stream_executor(&program, word, BUDGET);
    for _ in 0..20 {
        let (value, rest) = pair(&mut executor, cursor);
        assert_eq!(int(&mut executor, value), 7);
        cursor = executor.collect(&[rest], 10_000).unwrap()[0];
    }
    assert_eq!(executor.stats().primitive_ops, 0);
    // Bare re-entry is a Cycle before and after collection, and after a
    // collection that ran while the failed cell was rooted.
    let (program, word) = compile(": same recur 1 1 ; 7 same");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let handle = executor.collect(&[handle], 10_000).unwrap()[0];
    assert_eq!(executor.force(handle), Err(Error::Cycle));
    let handle = executor.collect(&[handle], 10_000).unwrap()[0];
    assert_eq!(executor.force(handle), Err(Error::Cycle));
    assert_eq!(executor.storage().frames, 0);
    // A family through dynamic application: same rule.
    let text = ": zero 0 eq? ; : always drop true ; : base dup drop ; : stay recur 1 1 ; \
                family stuck 1 1 zero base always stay ; 3 ' stuck apply 1 1";
    let (program, word) = compile(text);
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let handle = executor.collect(&[handle], 10_000).unwrap()[0];
    assert_eq!(executor.force(handle), Err(Error::Cycle));
}

#[test]
fn every_failed_collection_leaves_old_handles_usable_and_success_is_idempotent() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let (mut executor, head) = stream_executor(&program, word, BUDGET);
    let cursor = walk(&mut executor, head, 8);
    let mut succeeded = None;
    for budget in 0..400 {
        let storage = executor.storage();
        match executor.collect(&[cursor, head], budget) {
            Err(Error::Budget) => {
                assert_eq!(executor.storage(), storage, "budget {budget}");
                // Old handles still work after a failed collection.
                let (value, _) = pair(&mut executor, cursor);
                assert_eq!(int(&mut executor, value), 8);
            }
            Ok(roots) => {
                succeeded = Some((budget, roots));
                break;
            }
            other => panic!("budget {budget}: {other:?}"),
        }
    }
    let (budget, roots) = succeeded.expect("a small heap fits in 400 units");
    assert!(budget > 0);
    let storage = executor.storage();
    // Collecting again with the same roots changes no counts.
    let again = executor.collect(&roots, 10_000).unwrap();
    assert_eq!(executor.storage().cells, storage.cells);
    assert_eq!(executor.storage().frames, storage.frames);
    assert_eq!(executor.force(roots[0]), Err(Error::StaleHandle));
    let (value, _) = pair(&mut executor, again[0]);
    assert_eq!(int(&mut executor, value), 8);
    let (value, _) = pair(&mut executor, again[1]);
    assert_eq!(int(&mut executor, value), 0);
}

#[test]
fn keeping_the_head_grows_linearly_and_releasing_it_drops_to_the_window() {
    let (program, word) = compile(&format!("{STREAM} 0 from"));
    let (mut executor, head) = stream_executor(&program, word, 2_000_000);
    let mut head = head;
    let mut cursor = head;
    let mut sizes = Vec::new();
    for step in 1..=400 {
        cursor = pair(&mut executor, cursor).1;
        if step % 100 == 0 {
            let roots = executor.collect(&[head, cursor], 100_000).unwrap();
            head = roots[0];
            cursor = roots[1];
            sizes.push(executor.storage().cells);
        }
    }
    let slopes: Vec<_> = sizes.windows(2).map(|w| w[1] - w[0]).collect();
    eprintln!("head-retained sizes every 100 steps: {sizes:?}");
    assert!(slopes.iter().all(|&s| s == slopes[0] && s > 0), "{sizes:?}");
    let stale_head = head;
    cursor = executor.collect(&[cursor], 100_000).unwrap()[0];
    assert_eq!(executor.force(stale_head), Err(Error::StaleHandle));
    // No head was ever forced, so the cursor's value still depends on the
    // whole chain of pending additions: that prefix is live, not garbage.
    assert!(executor.storage().cells > 2_000, "{:?}", executor.storage());
    assert_eq!(executor.stats().primitive_ops, 0);
    let (value, _) = pair(&mut executor, cursor);
    assert_eq!(int(&mut executor, value), 400);
    assert_eq!(executor.stats().primitive_ops, 400);
    // Once the head is a value, the chain is history and collection drops it.
    cursor = executor.collect(&[cursor], 100_000).unwrap()[0];
    assert!(executor.storage().cells <= 12, "{:?}", executor.storage());
    let (value, _) = pair(&mut executor, cursor);
    assert_eq!(int(&mut executor, value), 400);
    assert_eq!(executor.stats().primitive_ops, 400);
}

#[test]
fn ready_and_failed_scalars_survive_collection_without_their_history() {
    let chain = "1 + ".repeat(200);
    let (program, word) = compile(&format!(
        ": chain {chain}; 0 chain 9223372036854775807 1 + pair"
    ));
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let (good, bad) = pair(&mut executor, handle);
    assert_eq!(int(&mut executor, good), 200);
    assert_eq!(executor.force(bad), Err(Error::Overflow));
    assert!(executor.storage().cells > 200);
    let handle = executor.collect(&[handle], 100_000).unwrap()[0];
    let storage = executor.storage();
    assert_eq!(storage.frames, 0, "{storage:?}");
    assert!(storage.cells <= 3, "{storage:?}");
    assert_eq!(storage.failures, 1);
    let (good, bad) = pair(&mut executor, handle);
    assert_eq!(int(&mut executor, good), 200);
    assert_eq!(executor.force(bad), Err(Error::Overflow));
    assert_eq!(executor.stats().primitive_ops, 201);
}

#[test]
fn collection_after_a_semantic_failure_is_allowed_and_keeps_the_error() {
    let (program, word) =
        compile(": bad 9223372036854775807 1 + ; : deep bad 1 + 1 + 1 + ; deep 7 pair");
    let mut executor = Executor::new(&program);
    let handle = executor.start(word, &[], &Context::new(), BUDGET).unwrap()[0];
    let (failing, seven) = pair(&mut executor, handle);
    assert_eq!(executor.force(failing), Err(Error::Overflow));
    // The whole demand chain failed; nothing is busy, so collection proceeds.
    let handle = executor.collect(&[handle], 10_000).unwrap()[0];
    let (failing, seven_again) = pair(&mut executor, handle);
    assert_eq!(executor.force(failing), Err(Error::Overflow));
    assert_eq!(int(&mut executor, seven_again), 7);
    assert_eq!(executor.force(seven), Err(Error::StaleHandle));
    assert_eq!(executor.stats().primitive_ops, 1);
}
