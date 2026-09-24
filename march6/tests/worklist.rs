//! Deep regressions for the explicit reducer work lists.  These graphs are
//! intentionally much deeper than a native recursive evaluator can safely
//! traverse in a test thread.

use march_research::reduce::ReduceError;
use march_research::{Atom, Bindings, Cid, Node, Reducer, Store};

const DEEP: usize = 20_000;

fn int(store: &mut Store, value: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn left_add_chain(store: &mut Store, leaf: Cid, zero: Cid, depth: usize) -> Cid {
    (0..depth).fold(leaf, |left, _| store.intern(Node::Add(left, zero)))
}

#[test]
fn deep_evaluation_uses_explicit_continuations() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let root = left_add_chain(&mut store, one, zero, DEEP);

    let reduction = Reducer::with_budget(&mut store, &Bindings::new(), usize::MAX)
        .run(root)
        .unwrap();

    assert_eq!(store.get(reduction.root), Some(&Node::Const(Atom::Int(1))));
    assert!(reduction.stats.peak_frames >= DEEP);
}

#[test]
fn deep_quotation_substitution_uses_an_explicit_work_list() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let seven = int(&mut store, 7);
    let parameter = store.intern(Node::Param(0));
    let body = left_add_chain(&mut store, parameter, zero, DEEP);
    let quotation = store.intern(Node::Quote { params: 1, body });
    let application = store.intern(Node::Apply {
        function: quotation,
        arguments: vec![seven],
    });

    let reduction = Reducer::with_budget(&mut store, &Bindings::new(), usize::MAX)
        .run(application)
        .unwrap();

    assert_eq!(store.get(reduction.root), Some(&Node::Const(Atom::Int(7))));
    assert!(reduction.stats.peak_frames >= DEEP);
}

#[test]
fn deep_binding_capture_beneath_an_unresolved_branch_is_iterative_and_lazy() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let seven = int(&mut store, 7);
    let condition = store.intern(Node::Hole("condition".into()));
    let value = store.intern(Node::Hole("value".into()));
    let when_true = left_add_chain(&mut store, value, zero, DEEP);
    let root = store.intern(Node::If {
        condition,
        when_true,
        when_false: zero,
    });
    let mut compile = Bindings::new();
    compile.insert("value", seven);

    let residual = Reducer::with_budget(&mut store, &compile, usize::MAX)
        .run(root)
        .unwrap();
    assert!(residual.stats.peak_frames >= DEEP);

    let truth = store.intern(Node::Const(Atom::Bool(true)));
    let mut runtime = Bindings::new();
    runtime.insert("condition", truth);
    let result = Reducer::with_budget(&mut store, &runtime, usize::MAX)
        .run(residual.root)
        .unwrap();
    assert_eq!(store.get(result.root), Some(&Node::Const(Atom::Int(7))));
}

#[test]
fn deep_groundness_check_reports_a_type_error_without_host_recursion() {
    let mut store = Store::new();
    let one = int(&mut store, 1);
    let mut pair = one;
    for _ in 0..DEEP {
        pair = store.intern(Node::Pair(pair, one));
    }
    let invalid = store.intern(Node::Add(pair, one));

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), usize::MAX).run(invalid),
        Err(ReduceError::Type("add expects two integers")),
    );
}

#[test]
fn explicit_budget_still_bounds_deep_iterative_work() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let root = left_add_chain(&mut store, one, zero, DEEP);

    assert_eq!(
        Reducer::with_budget(&mut store, &Bindings::new(), 100).run(root),
        Err(ReduceError::BudgetExhausted { limit: 100 }),
    );
}
