use march_research::reduce::ReduceError;
use march_research::{Atom, Bindings, Clause, Image, Node, Reducer, Store};

fn int(store: &mut Store, value: i64) -> march_research::Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn bool_(store: &mut Store, value: bool) -> march_research::Cid {
    store.intern(Node::Const(Atom::Bool(value)))
}

struct Workload {
    root: march_research::Cid,
}

/// Build the vertical-slice workload from MODEL.md:
///
///   6*7 + main(flag, n)
///   main false _ = 0
///   main true  n = let s = sum(n); s+s
///   sum 0 = 0
///   sum n = n + sum(n-1)
///
/// The repeated `s` is one shared CID, so the reducer's memo table gives
/// call-by-need behavior within an epoch.
fn workload(store: &mut Store) -> Workload {
    let zero = int(store, 0);
    let negative_one = int(store, -1);
    let six = int(store, 6);
    let seven = int(store, 7);
    let truth = bool_(store, true);
    let falsity = bool_(store, false);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));

    let sum_is_zero = store.intern(Node::Eq(p0, zero));
    let decremented = store.intern(Node::Add(p0, negative_one));
    let recur = store.intern(Node::Recur(vec![decremented]));
    let sum_step = store.intern(Node::Add(p0, recur));
    let sum = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause {
                guard: sum_is_zero,
                body: zero,
            },
            Clause {
                guard: truth,
                body: sum_step,
            },
        ],
    });

    let flag_is_false = store.intern(Node::Eq(p0, falsity));
    let shared_sum = store.intern(Node::Dispatch {
        family: sum,
        arguments: vec![p1],
    });
    let doubled_sum = store.intern(Node::Add(shared_sum, shared_sum));
    let main = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: flag_is_false,
                body: zero,
            },
            Clause {
                guard: truth,
                body: doubled_sum,
            },
        ],
    });

    let flag = store.intern(Node::Hole("flag".into()));
    let n = store.intern(Node::Hole("n".into()));
    let call = store.intern(Node::Dispatch {
        family: main,
        arguments: vec![flag, n],
    });
    let static_island = store.intern(Node::Mul(six, seven));
    let root = store.intern(Node::Add(static_island, call));
    Workload { root }
}

fn result_int(store: &Store, root: march_research::Cid) -> i64 {
    match store.get(root) {
        Some(Node::Const(Atom::Int(value))) => *value,
        other => panic!("expected integer result, got {other:?}"),
    }
}

#[test]
fn unknown_dispatch_reduces_static_islands_without_instantiating_a_clause() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let reduced = Reducer::new(&mut store, &Bindings::new())
        .run(workload.root)
        .unwrap();

    assert_eq!(reduced.stats.clauses_instantiated, 0);
    let residual = store.format(reduced.root);
    assert!(residual.starts_with("(+ 42 (dispatch "), "{residual}");
}

#[test]
fn rejected_recursive_clause_is_neither_instantiated_nor_evaluated() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let negative = int(&mut store, -1);
    let falsity = bool_(&mut store, false);
    let mut bindings = Bindings::new();
    bindings.insert("flag", falsity);
    bindings.insert("n", negative);

    let reduced = Reducer::new(&mut store, &bindings)
        .run(workload.root)
        .unwrap();
    assert_eq!(result_int(&store, reduced.root), 42);
    assert_eq!(reduced.stats.clauses_instantiated, 1);
}

#[test]
fn shared_recursive_call_is_evaluated_once() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let three = int(&mut store, 3);
    let truth = bool_(&mut store, true);
    let mut bindings = Bindings::new();
    bindings.insert("flag", truth);
    bindings.insert("n", three);

    let reduced = Reducer::new(&mut store, &bindings)
        .run(workload.root)
        .unwrap();
    assert_eq!(result_int(&store, reduced.root), 54);
    // main once; sum for 3, 2, 1, and 0 once each.  A duplicated evaluation
    // of the shared sum would instantiate nine clauses instead of five.
    assert_eq!(reduced.stats.clauses_instantiated, 5);
    assert!(reduced.stats.memo_hits > 0);
}

#[test]
fn facts_can_arrive_in_either_epoch_order() {
    for n_first in [false, true] {
        let mut store = Store::new();
        let workload = workload(&mut store);
        let three = int(&mut store, 3);
        let truth = bool_(&mut store, true);

        let (first_name, first_value, second_name, second_value) = if n_first {
            ("n", three, "flag", truth)
        } else {
            ("flag", truth, "n", three)
        };
        let mut first = Bindings::new();
        first.insert(first_name, first_value);
        let residual = Reducer::new(&mut store, &first)
            .run(workload.root)
            .unwrap()
            .root;

        let mut second = Bindings::new();
        second.insert(second_name, second_value);
        let staged = Reducer::new(&mut store, &second).run(residual).unwrap();

        let mut all = Bindings::new();
        all.insert("flag", truth);
        all.insert("n", three);
        let direct = Reducer::new(&mut store, &all).run(workload.root).unwrap();
        assert_eq!(staged.root, direct.root);
        assert_eq!(result_int(&store, staged.root), 54);
    }
}

#[test]
fn stuck_dispatch_survives_an_image_round_trip() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let three = int(&mut store, 3);
    let mut first = Bindings::new();
    first.insert("n", three);
    let residual = Reducer::new(&mut store, &first)
        .run(workload.root)
        .unwrap()
        .root;

    let image = Image::from_store(&store, &[residual]).unwrap();
    let (_, mut loaded) = Image::parse(image.as_bytes()).unwrap();
    let truth = bool_(&mut loaded, true);
    let mut second = Bindings::new();
    second.insert("flag", truth);
    let resumed = Reducer::new(&mut loaded, &second).run(residual).unwrap();
    assert_eq!(result_int(&loaded, resumed.root), 54);
}

#[test]
fn later_clause_errors_are_not_observed_after_an_earlier_match() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let one = int(&mut store, 1);
    let two = int(&mut store, 2);
    let invalid_guard = store.intern(Node::Add(one, truth));
    let family = store.intern(Node::Family {
        parameters: 0,
        clauses: vec![
            Clause {
                guard: truth,
                body: one,
            },
            Clause {
                guard: invalid_guard,
                body: two,
            },
        ],
    });
    let call = store.intern(Node::Dispatch {
        family,
        arguments: Vec::new(),
    });
    let reduced = Reducer::new(&mut store, &Bindings::new())
        .run(call)
        .unwrap();
    assert_eq!(result_int(&store, reduced.root), 1);
    assert_eq!(reduced.stats.clauses_instantiated, 1);
}

fn stuck_steps_with_dormant_body_size(size: usize) -> usize {
    let mut store = Store::new();
    let parameter = store.intern(Node::Param(0));
    let one = int(&mut store, 1);
    let mut body = one;
    for _ in 0..size {
        body = store.intern(Node::Add(body, one));
    }
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause {
            guard: parameter,
            body,
        }],
    });
    let unknown = store.intern(Node::Hole("unknown".into()));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![unknown],
    });
    Reducer::new(&mut store, &Bindings::new())
        .run(call)
        .unwrap()
        .stats
        .steps
}

#[test]
fn stuck_dispatch_does_not_traverse_dormant_bodies() {
    assert_eq!(
        stuck_steps_with_dormant_body_size(10),
        stuck_steps_with_dormant_body_size(10_000)
    );
}

#[test]
fn quotations_are_lazy_code_values() {
    let mut store = Store::new();
    let one = int(&mut store, 1);
    let truth = bool_(&mut store, true);
    let invalid_body = store.intern(Node::Add(one, truth));
    let quotation = store.intern(Node::Quote {
        params: 0,
        body: invalid_body,
    });

    let dormant = Reducer::new(&mut store, &Bindings::new())
        .run(quotation)
        .unwrap();
    assert_eq!(dormant.root, quotation);

    let apply = store.intern(Node::Apply {
        function: quotation,
        arguments: Vec::new(),
    });
    assert!(matches!(
        Reducer::new(&mut store, &Bindings::new()).run(apply),
        Err(ReduceError::Type(_))
    ));
}

#[test]
fn identical_recursive_call_has_a_defined_cycle_error() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let parameter = store.intern(Node::Param(0));
    let recur = store.intern(Node::Recur(vec![parameter]));
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause {
            guard: truth,
            body: recur,
        }],
    });
    let one = int(&mut store, 1);
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![one],
    });
    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::Cycle(call))
    );
}

#[test]
fn code_values_reject_ambient_holes_instead_of_losing_epoch_facts() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let ambient = store.intern(Node::Hole("ambient".into()));
    let family = store.intern(Node::Family {
        parameters: 0,
        clauses: vec![Clause {
            guard: truth,
            body: ambient,
        }],
    });
    let call = store.intern(Node::Dispatch {
        family,
        arguments: Vec::new(),
    });
    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::OpenCodeValue("ambient".into()))
    );
}

#[test]
fn guards_are_syntactically_pure() {
    let mut store = Store::new();
    let parameter = store.intern(Node::Param(0));
    let message = store.intern(Node::Const(Atom::Text("guard".into())));
    let impure_guard = store.intern(Node::Emit {
        token: parameter,
        message,
    });
    let zero = int(&mut store, 0);
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause {
            guard: impure_guard,
            body: zero,
        }],
    });
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![world],
    });
    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::ImpureGuard(impure_guard))
    );
}

#[test]
fn quotation_parameters_cannot_duplicate_effect_tokens() {
    let mut store = Store::new();
    let parameter = store.intern(Node::Param(0));
    let a = store.intern(Node::Const(Atom::Text("a".into())));
    let b = store.intern(Node::Const(Atom::Text("b".into())));
    let emit_a = store.intern(Node::Emit {
        token: parameter,
        message: a,
    });
    let emit_b = store.intern(Node::Emit {
        token: parameter,
        message: b,
    });
    let body = store.intern(Node::Pair(emit_a, emit_b));
    let quotation = store.intern(Node::Quote { params: 1, body });
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let apply = store.intern(Node::Apply {
        function: quotation,
        arguments: vec![world],
    });
    assert!(matches!(
        Reducer::new(&mut store, &Bindings::new()).run(apply),
        Err(ReduceError::LinearValueDuplicated(_))
    ));
}

#[test]
fn deep_recursion_fails_as_a_resource_error_before_host_stack_overflow() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let truth = bool_(&mut store, true);
    let thousand = int(&mut store, 1_000);
    let mut bindings = Bindings::new();
    bindings.insert("flag", truth);
    bindings.insert("n", thousand);
    assert!(matches!(
        Reducer::with_budget(&mut store, &bindings, usize::MAX).run(workload.root),
        Err(ReduceError::DepthExhausted { .. })
    ));
}
