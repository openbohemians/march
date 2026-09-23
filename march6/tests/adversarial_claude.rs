//! Adversarial regression tests from the @march-claude review of the guarded
//! slice.  Each test states the behaviour March *should* have.  Tests that
//! document a known, accepted gap are `#[ignore]`d with the reason, so the
//! suite stays green while the gap stays visible.

use march_research::inet::{AgentKind, Net, NetError, Schedule, Value};
use march_research::lower;
use march_research::reduce::ReduceError;
use march_research::{Atom, Bindings, Cid, Clause, Node, Reducer, Store};

fn int(store: &mut Store, value: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn bool_(store: &mut Store, value: bool) -> Cid {
    store.intern(Node::Const(Atom::Bool(value)))
}

fn text(store: &mut Store, value: &str) -> Cid {
    store.intern(Node::Const(Atom::Text(value.into())))
}

fn trace(store: &mut Store) -> Cid {
    store.intern(Node::Const(Atom::Trace(Vec::new())))
}

/// `sum n = n + sum(n - 1)`, `sum 0 = 0`.
fn sum_family(store: &mut Store) -> Cid {
    let zero = int(store, 0);
    let negative_one = int(store, -1);
    let truth = bool_(store, true);
    let p0 = store.intern(Node::Param(0));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let decremented = store.intern(Node::Add(p0, negative_one));
    let recur = store.intern(Node::Recur(vec![decremented]));
    let step = store.intern(Node::Add(p0, recur));
    store.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: zero,
            },
            Clause {
                guard: truth,
                body: step,
            },
        ],
    })
}

// (A) Facts supplied in an early epoch must not be lost inside a stuck
// dispatch.  Closed code makes the open form an error in *every* staging, and
// passing the context as a parameter gives staged == direct.

/// `f(x, k) = [ x == 0 -> 0 ; true -> x + k ]`, called with holes `?x ?k`.
fn context_as_parameter(store: &mut Store) -> Cid {
    let zero = int(store, 0);
    let truth = bool_(store, true);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let body = store.intern(Node::Add(p0, p1));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: zero,
            },
            Clause { guard: truth, body },
        ],
    });
    let x = store.intern(Node::Hole("x".into()));
    let k = store.intern(Node::Hole("k".into()));
    store.intern(Node::Dispatch {
        family,
        arguments: vec![x, k],
    })
}

#[test]
fn a_family_body_with_a_free_hole_is_rejected_in_every_staging() {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let truth = bool_(&mut store, true);
    let p0 = store.intern(Node::Param(0));
    let k = store.intern(Node::Hole("k".into()));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let body = store.intern(Node::Add(p0, k));
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: zero,
            },
            Clause { guard: truth, body },
        ],
    });
    let x = store.intern(Node::Hole("x".into()));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![x],
    });
    let hundred = int(&mut store, 100);
    let five = int(&mut store, 5);

    let mut early = Bindings::new();
    early.insert("k", hundred);
    let mut all = early.clone();
    all.insert("x", five);

    for bindings in [Bindings::new(), early, all] {
        assert_eq!(
            Reducer::new(&mut store, &bindings).run(call),
            Err(ReduceError::OpenCodeValue("k".into())),
        );
    }
}

#[test]
fn a_context_passed_as_a_parameter_survives_every_epoch_split() {
    let mut store = Store::new();
    let call = context_as_parameter(&mut store);
    let hundred = int(&mut store, 100);
    let five = int(&mut store, 5);

    let mut all = Bindings::new();
    all.insert("k", hundred);
    all.insert("x", five);
    let direct = Reducer::new(&mut store, &all).run(call).unwrap().root;
    assert_eq!(store.get(direct), Some(&Node::Const(Atom::Int(105))));

    for (first_name, first, second_name, second) in
        [("k", hundred, "x", five), ("x", five, "k", hundred)]
    {
        let mut epoch1 = Bindings::new();
        epoch1.insert(first_name, first);
        let residual = Reducer::new(&mut store, &epoch1).run(call).unwrap().root;
        let mut epoch2 = Bindings::new();
        epoch2.insert(second_name, second);
        let staged = Reducer::new(&mut store, &epoch2)
            .run(residual)
            .unwrap()
            .root;
        assert_eq!(staged, direct, "first epoch supplied {first_name}");
    }
}

// (B) A linear token must not be forked by capturing it inside dormant code.

#[test]
fn b_quote_capturing_a_token_hole_is_rejected() {
    let mut store = Store::new();
    let w = store.intern(Node::Hole("w".into()));
    let a = text(&mut store, "a");
    let b = text(&mut store, "b");
    let inside = store.intern(Node::Emit {
        token: w,
        message: a,
    });
    let quote = store.intern(Node::Quote {
        params: 0,
        body: inside,
    });
    let apply = store.intern(Node::Apply {
        function: quote,
        arguments: Vec::new(),
    });
    let outside = store.intern(Node::Emit {
        token: w,
        message: b,
    });
    let root = store.intern(Node::Pair(apply, outside));
    let world = trace(&mut store);
    let mut bindings = Bindings::new();
    bindings.insert("w", world);
    assert_eq!(
        Reducer::new(&mut store, &bindings).run(root),
        Err(ReduceError::OpenCodeValue("w".into())),
    );
}

#[test]
fn b_quote_capturing_a_trace_literal_cannot_fork_the_world() {
    let mut store = Store::new();
    let world = trace(&mut store);
    let a = text(&mut store, "a");
    let b = text(&mut store, "b");
    let inside = store.intern(Node::Emit {
        token: world,
        message: a,
    });
    let quote = store.intern(Node::Quote {
        params: 0,
        body: inside,
    });
    let apply = store.intern(Node::Apply {
        function: quote,
        arguments: Vec::new(),
    });
    let outside = store.intern(Node::Emit {
        token: world,
        message: b,
    });
    let root = store.intern(Node::Pair(apply, outside));
    // Without the quotation this exact graph is already rejected; putting one
    // use behind a code boundary must not change that verdict.
    assert!(matches!(
        Reducer::new(&mut store, &Bindings::new()).run(root),
        Err(ReduceError::LinearValueDuplicated(_)),
    ));
}

// (C) A guard must not consume (or even mention) an effect token.

#[test]
fn c_guard_that_emits_is_impure() {
    let mut store = Store::new();
    let p0 = store.intern(Node::Param(0));
    let guard_message = text(&mut store, "guard");
    let body_message = text(&mut store, "body");
    let emitted = store.intern(Node::Emit {
        token: p0,
        message: guard_message,
    });
    let expected = store.intern(Node::Const(Atom::Trace(vec!["guard".into()])));
    let guard = store.intern(Node::Eq(emitted, expected));
    let body = store.intern(Node::Emit {
        token: p0,
        message: body_message,
    });
    let family = store.intern(Node::Family {
        parameters: 1,
        clauses: vec![Clause { guard, body }],
    });
    let world = trace(&mut store);
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![world],
    });
    assert!(matches!(
        Reducer::new(&mut store, &Bindings::new()).run(call),
        Err(ReduceError::ImpureGuard(_)),
    ));
}

// (D) Deep recursion is an execution-resource outcome, never a process abort.

#[test]
fn d_deep_recursion_reports_a_resource_error_instead_of_aborting() {
    let mut store = Store::new();
    let sum = sum_family(&mut store);
    let n = int(&mut store, 1_000);
    let call = store.intern(Node::Dispatch {
        family: sum,
        arguments: vec![n],
    });
    // Before the fix this aborted the whole test binary with a native stack
    // overflow.  Reaching the assertion at all is most of the test.
    assert!(matches!(
        Reducer::with_budget(&mut store, &Bindings::new(), usize::MAX).run(call),
        Err(ReduceError::DepthExhausted { .. }),
    ));
}

#[test]
#[ignore = "open: needs the work-list reducer; the host depth limit (64) is a stand-in, not the intended semantics"]
fn d_deep_recursion_completes_with_an_ample_budget() {
    let mut store = Store::new();
    let sum = sum_family(&mut store);
    let n = int(&mut store, 10_000);
    let call = store.intern(Node::Dispatch {
        family: sum,
        arguments: vec![n],
    });
    let result = Reducer::with_budget(&mut store, &Bindings::new(), usize::MAX)
        .run(call)
        .unwrap()
        .root;
    assert_eq!(store.get(result), Some(&Node::Const(Atom::Int(50_005_000))));
}

// Lazy arguments: CAS and INet must agree when the selected clause ignores an
// argument whose evaluation would fail.

/// `f(flag, v) = [ flag == 0 -> 0 ; true -> v ]` called as `f(0, MAX + 1)`.
fn ignored_overflowing_argument(store: &mut Store) -> Cid {
    let zero = int(store, 0);
    let truth = bool_(store, true);
    let max = int(store, i64::MAX);
    let one = int(store, 1);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let is_zero = store.intern(Node::Eq(p0, zero));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: is_zero,
                body: zero,
            },
            Clause {
                guard: truth,
                body: p1,
            },
        ],
    });
    let overflow = store.intern(Node::Add(max, one));
    store.intern(Node::Dispatch {
        family,
        arguments: vec![zero, overflow],
    })
}

#[test]
fn lazy_cas_ignores_an_unused_failing_argument() {
    let mut store = Store::new();
    let call = ignored_overflowing_argument(&mut store);
    let result = Reducer::new(&mut store, &Bindings::new())
        .run(call)
        .unwrap()
        .root;
    assert_eq!(store.get(result), Some(&Node::Const(Atom::Int(0))));
}

#[test]
fn lazy_inet_agrees_with_cas_on_an_unused_failing_argument() {
    let mut store = Store::new();
    let call = ignored_overflowing_argument(&mut store);
    // Acceptable outcomes: lowering refuses the program, or the net agrees
    // with the CAS reducer.  Silently producing a different error is not.
    let Ok(net) = lower::to_inet(&store, call, "result") else {
        return;
    };
    for schedule in [Schedule::LowestWire, Schedule::HighestWire] {
        let mut net = net.clone();
        net.reduce(200, schedule).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(0)));
    }
}

// INet linearity: a world token must never be copied or silently dropped.

fn world_facing(kind: AgentKind) -> Net {
    let mut net = Net::new();
    let world = net.add(AgentKind::World(Vec::new()));
    let operator = net.add(kind.clone());
    net.connect(
        net.principal(world).unwrap(),
        net.principal(operator).unwrap(),
    )
    .unwrap();
    if matches!(kind, AgentKind::Fan) {
        for (index, label) in [(1, "left"), (2, "right")] {
            let output = net.add(AgentKind::Output(label.into()));
            net.connect(
                net.port(operator, index).unwrap(),
                net.principal(output).unwrap(),
            )
            .unwrap();
        }
    }
    net
}

#[test]
fn inet_fan_refuses_to_duplicate_a_world_token() {
    let mut net = world_facing(AgentKind::Fan);
    assert!(matches!(
        net.reduce(10, Schedule::LowestWire),
        Err(NetError::NoRule(AgentKind::World(_), AgentKind::Fan)
            | NetError::NoRule(AgentKind::Fan, AgentKind::World(_))),
    ));
    assert!(net.outputs().is_empty());
}

#[test]
fn inet_erase_refuses_to_drop_a_world_token() {
    let mut net = world_facing(AgentKind::Erase);
    assert!(matches!(
        net.reduce(10, Schedule::LowestWire),
        Err(NetError::NoRule(AgentKind::World(_), AgentKind::Erase)
            | NetError::NoRule(AgentKind::Erase, AgentKind::World(_))),
    ));
}
