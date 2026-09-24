//! R0 follow-up regressions from @march-claude: linear scaling of recursion
//! that carries unforced or structured state, and history-independent residuals.
//!
//! Scaling is asserted as a ratio of reducer steps between n and 2n.  Linear
//! work gives about 2.0 and quadratic work about 4.0; the 2.5 threshold keeps
//! the tests independent of machine speed and cheap in debug builds.

use march_research::{Atom, Bindings, Cid, Clause, Node, Reducer, Store};

const RATIO_LIMIT: f64 = 2.5;

fn int(store: &mut Store, value: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn truth(store: &mut Store) -> Cid {
    store.intern(Node::Const(Atom::Bool(true)))
}

fn run_steps(store: &mut Store, root: Cid) -> (Cid, usize) {
    let reduced = Reducer::with_budget(store, &Bindings::new(), usize::MAX)
        .run(root)
        .unwrap();
    (reduced.root, reduced.stats.steps)
}

fn assert_linear(label: &str, small: usize, large: usize) {
    let ratio = large as f64 / small as f64;
    assert!(
        ratio < RATIO_LIMIT,
        "{label}: steps grew {small} -> {large} (x{ratio:.2}) when n doubled",
    );
}

/// `sumacc(n, acc) = [ n == 0 -> acc ; true -> recur(n - 1, acc + n) ]`.
/// The guard never demands `acc`, so it legitimately stays a lazy chain until
/// the final clause returns it.
fn sumacc(n: i64) -> (i64, usize) {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let negative_one = int(&mut store, -1);
    let otherwise = truth(&mut store);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let decremented = store.intern(Node::Add(p0, negative_one));
    let accumulated = store.intern(Node::Add(p1, p0));
    let recur = store.intern(Node::Recur(vec![decremented, accumulated]));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: p1,
            },
            Clause {
                guard: otherwise,
                body: recur,
            },
        ],
    });
    let start = int(&mut store, n);
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![start, zero],
    });
    let (root, steps) = run_steps(&mut store, call);
    match store.get(root) {
        Some(Node::Const(Atom::Int(value))) => (*value, steps),
        other => panic!("sumacc({n}) returned {other:?}"),
    }
}

#[test]
fn lazy_accumulator_recursion_scales_linearly() {
    let (small_value, small) = sumacc(256);
    let (large_value, large) = sumacc(512);
    assert_eq!(small_value, 32_896);
    assert_eq!(large_value, 131_328);
    assert_linear("sumacc", small, large);
}

/// `count(list) = [ list == unit -> 0 ; true -> 1 + recur(second(list)) ]`
/// over a ground list `(pair 1 (pair 1 ... unit))`.  The structural guard
/// compares a whole remaining list at every level.
fn count_list(length: usize) -> (i64, usize) {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let unit = store.intern(Node::Const(Atom::Unit));
    let otherwise = truth(&mut store);
    let p0 = store.intern(Node::Param(0));
    let is_empty = store.intern(Node::Eq(p0, unit));
    let tail = store.intern(Node::Second(p0));
    let recur = store.intern(Node::Recur(vec![tail]));
    let step = store.intern(Node::Add(one, recur));
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause {
                guard: is_empty,
                body: zero,
            },
            Clause {
                guard: otherwise,
                body: step,
            },
        ],
    });
    let mut list = unit;
    for _ in 0..length {
        list = store.intern(Node::Pair(one, list));
    }
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![list],
    });
    let (root, steps) = run_steps(&mut store, call);
    match store.get(root) {
        Some(Node::Const(Atom::Int(value))) => (*value, steps),
        other => panic!("count({length}) returned {other:?}"),
    }
}

#[test]
fn structural_guard_list_walk_scales_linearly() {
    let (small_value, small) = count_list(256);
    let (large_value, large) = count_list(512);
    assert_eq!(small_value, 256);
    assert_eq!(large_value, 512);
    assert_linear("count", small, large);
}

/// `tick(n, world) = [ n == 0 -> world ; true ->
/// recur(n - 1, emit(world, "tick")) ]`.  The world chain stays suspended
/// until the base clause returns it, so selection must not repeatedly walk it.
fn effect_loop(length: i64) -> (usize, usize) {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let negative_one = int(&mut store, -1);
    let otherwise = truth(&mut store);
    let message = store.intern(Node::Const(Atom::Text("tick".into())));
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let decremented = store.intern(Node::Add(p0, negative_one));
    let emitted = store.intern(Node::Emit { token: p1, message });
    let recur = store.intern(Node::Recur(vec![decremented, emitted]));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: p1,
            },
            Clause {
                guard: otherwise,
                body: recur,
            },
        ],
    });
    let start = int(&mut store, length);
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![start, world],
    });
    let (root, steps) = run_steps(&mut store, call);
    match store.get(root) {
        Some(Node::Const(Atom::Trace(entries))) => (entries.len(), steps),
        other => panic!("effect loop returned {other:?}"),
    }
}

#[test]
fn world_threading_effect_loop_scales_linearly_in_reducer_steps() {
    let (small_entries, small) = effect_loop(128);
    let (large_entries, large) = effect_loop(256);
    assert_eq!(small_entries, 128);
    assert_eq!(large_entries, 256);
    assert_linear("effect loop", small, large);
}

/// `g(z, u) = [ u -> z ; true -> 0 ]` stays stuck while `u` is unknown.
/// `f(x, y, u) = [ x == 0 -> g(y, u) ; true -> 0 ]` never demands `y` in a
/// guard, so `y` must reach the residual in the same form regardless of
/// whether some unrelated sibling happened to reduce the same expression
/// first.  Returns (residual of the call alone, residual of the call after a
/// sibling demanded `y`).
fn residuals_with_and_without_prior_demand() -> (Store, Cid, Cid) {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let two = int(&mut store, 2);
    let otherwise = truth(&mut store);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let p2 = store.intern(Node::Param(2));

    let g = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: p1,
                body: p0,
            },
            Clause {
                guard: otherwise,
                body: zero,
            },
        ],
    });
    let x_is_zero = store.intern(Node::Eq(p0, zero));
    let call_g = store.intern(Node::Dispatch {
        family: g,
        arguments: vec![p1, p2],
    });
    let f = store.intern(Node::Family {
        parameters: 3,
        clauses: vec![
            Clause {
                guard: x_is_zero,
                body: call_g,
            },
            Clause {
                guard: otherwise,
                body: zero,
            },
        ],
    });

    let y = store.intern(Node::Add(one, two));
    let u = store.intern(Node::Hole("u".into()));
    let call = store.intern(Node::Dispatch {
        family: f,
        arguments: vec![zero, y, u],
    });

    let alone = Reducer::new(&mut store, &Bindings::new())
        .run(call)
        .unwrap()
        .root;

    // `Pair` reduces its left child first, so `y` is in the memo before `f`
    // selects its clause.
    let after_sibling = store.intern(Node::Pair(y, call));
    let reduced = Reducer::new(&mut store, &Bindings::new())
        .run(after_sibling)
        .unwrap()
        .root;
    let Some(Node::Pair(left, right)) = store.get(reduced).cloned() else {
        panic!("expected a residual pair, got {}", store.format(reduced));
    };
    assert_eq!(store.get(left), Some(&Node::Const(Atom::Int(3))));
    (store, alone, right)
}

#[test]
fn undemanded_argument_residual_is_history_independent() {
    let (store, alone, after_sibling) = residuals_with_and_without_prior_demand();
    assert_eq!(
        alone,
        after_sibling,
        "alone: {}  after sibling: {}",
        store.format(alone),
        store.format(after_sibling),
    );
    // The guard of `f` never demanded `y`, so it stays unevaluated.
    assert!(store.format(alone).ends_with("(+ 1 2) ?u)"));
}
