use march_research::reduce::ReduceError;
use march_research::{Atom, Bindings, Cid, Clause, Image, Node, Reducer, Store};

fn int(store: &mut Store, value: i64) -> march_research::Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn bool_(store: &mut Store, value: bool) -> march_research::Cid {
    store.intern(Node::Const(Atom::Bool(value)))
}

struct Workload {
    root: march_research::Cid,
}

/// Build the historical workload from `6f17b9b:march6/MODEL.md`:
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
fn recursive_arguments_cannot_duplicate_an_effect_token() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let parameter = store.intern(Node::Param(0));
    let recur = store.intern(Node::Recur(vec![parameter, parameter]));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![Clause {
            guard: truth,
            body: recur,
        }],
    });
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let zero = int(&mut store, 0);
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![world, zero],
    });

    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::LinearValueDuplicated(world)),
    );
}

#[test]
fn a_bound_effect_hole_cannot_be_duplicated_by_a_selected_body() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let parameter = store.intern(Node::Param(0));
    let body = store.intern(Node::Pair(parameter, parameter));
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard: truth, body }],
    });
    let hole = store.intern(Node::Hole("world".into()));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![hole],
    });
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let mut bindings = Bindings::new();
    bindings.insert("world", world);

    assert_eq!(
        Reducer::new(&mut store, &bindings).run(call),
        Err(ReduceError::LinearValueDuplicated(hole)),
    );
}

fn fork_world_through_a_shared_container(store: &mut Store, world: Cid) -> Cid {
    let one = int(store, 1);
    let message_a = store.intern(Node::Const(Atom::Text("a".into())));
    let message_b = store.intern(Node::Const(Atom::Text("b".into())));
    let carried = store.intern(Node::Pair(world, one));
    let shared = store.intern(Node::Pair(carried, carried));
    let left_container = store.intern(Node::First(shared));
    let right_container = store.intern(Node::Second(shared));
    let left_world = store.intern(Node::First(left_container));
    let right_world = store.intern(Node::First(right_container));
    let emit_a = store.intern(Node::Emit {
        token: left_world,
        message: message_a,
    });
    let emit_b = store.intern(Node::Emit {
        token: right_world,
        message: message_b,
    });
    store.intern(Node::Pair(emit_a, emit_b))
}

#[test]
fn a_world_inside_a_shared_root_container_cannot_be_forked() {
    let mut store = Store::new();
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let root = fork_world_through_a_shared_container(&mut store, world);

    assert!(matches!(
        Reducer::new(&mut store, &Bindings::new()).run(root),
        Err(ReduceError::LinearValueDuplicated(_)),
    ));
}

#[test]
fn a_family_cannot_fork_a_world_through_a_shared_container() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let parameter = store.intern(Node::Param(0));
    let body = fork_world_through_a_shared_container(&mut store, parameter);
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard: truth, body }],
    });
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![world],
    });

    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::LinearValueDuplicated(world)),
    );
}

#[test]
fn an_unbound_world_fork_is_rejected_in_the_residual_epoch() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let parameter = store.intern(Node::Param(0));
    let body = fork_world_through_a_shared_container(&mut store, parameter);
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard: truth, body }],
    });
    let hole = store.intern(Node::Hole("world".into()));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![hole],
    });

    assert_eq!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::LinearValueDuplicated(hole)),
    );
}

#[test]
fn distinct_bindings_cannot_alias_one_effect_token() {
    let mut store = Store::new();
    let a = store.intern(Node::Hole("a".into()));
    let b = store.intern(Node::Hole("b".into()));
    let message_a = store.intern(Node::Const(Atom::Text("a".into())));
    let message_b = store.intern(Node::Const(Atom::Text("b".into())));
    let emit_a = store.intern(Node::Emit {
        token: a,
        message: message_a,
    });
    let emit_b = store.intern(Node::Emit {
        token: b,
        message: message_b,
    });
    let root = store.intern(Node::Pair(emit_a, emit_b));
    let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
    let mut bindings = Bindings::new();
    bindings.insert("a", world);
    bindings.insert("b", world);

    assert_eq!(
        Reducer::new(&mut store, &bindings).run(root),
        Err(ReduceError::LinearValueDuplicated(world)),
    );
}

#[test]
fn deep_recursion_completes_without_using_the_host_stack() {
    let mut store = Store::new();
    let workload = workload(&mut store);
    let truth = bool_(&mut store, true);
    let thousand = int(&mut store, 1_000);
    let mut bindings = Bindings::new();
    bindings.insert("flag", truth);
    bindings.insert("n", thousand);
    let reduced = Reducer::with_budget(&mut store, &bindings, usize::MAX)
        .run(workload.root)
        .unwrap();
    assert_eq!(
        store.get(reduced.root),
        Some(&Node::Const(Atom::Int(1_001_042)))
    );
    assert!(reduced.stats.peak_frames > 64);
}

#[test]
fn a_guard_forced_argument_is_shared_with_the_selected_residual_body() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let negative_one = int(&mut store, -1);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let body = store.intern(Node::If {
        condition: p1,
        when_true: p0,
        when_false: zero,
    });
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![Clause {
            guard: is_zero,
            body,
        }],
    });
    let computed_zero = store.intern(Node::Add(one, negative_one));
    let unknown = store.intern(Node::Hole("later".into()));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![computed_zero, unknown],
    });

    let residual = Reducer::new(&mut store, &Bindings::new())
        .run(call)
        .unwrap();
    assert_eq!(
        store.get(residual.root),
        Some(&Node::If {
            condition: unknown,
            when_true: zero,
            when_false: zero,
        })
    );
}
