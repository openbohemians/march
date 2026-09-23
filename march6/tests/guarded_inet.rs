use march_research::inet::{Schedule, Value};
use march_research::lower;
use march_research::{Atom, Clause, Node, Store};
use std::collections::BTreeMap;

fn int(store: &mut Store, value: i64) -> march_research::Cid {
    store.intern(Node::Const(Atom::Int(value)))
}

fn bool_(store: &mut Store, value: bool) -> march_research::Cid {
    store.intern(Node::Const(Atom::Bool(value)))
}

fn workload(store: &mut Store) -> march_research::Cid {
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
    store.intern(Node::Add(static_island, call))
}

#[test]
fn guarded_net_reduces_static_work_then_waits_for_runtime_facts() {
    let mut store = Store::new();
    let root = workload(&mut store);
    let mut net = lower::to_inet(&store, root, "result").unwrap();

    let compile = net.reduce(100, Schedule::LowestWire).unwrap();
    assert!(compile.rewrites > 0);
    assert!(net.outputs().is_empty());

    net.bind(&BTreeMap::from([
        ("flag".into(), Value::Bool(false)),
        ("n".into(), Value::Int(-1)),
    ]));
    net.reduce(100, Schedule::LowestWire).unwrap();
    assert_eq!(net.outputs().get("result"), Some(&Value::Int(42)));
    assert_eq!(net.live_agents(), 0);
}

#[test]
fn guarded_recursive_net_matches_under_both_schedules() {
    let mut store = Store::new();
    let root = workload(&mut store);
    let source = lower::to_inet(&store, root, "result").unwrap();

    for schedule in [Schedule::LowestWire, Schedule::HighestWire] {
        let mut net = source.clone();
        net.bind(&BTreeMap::from([
            ("flag".into(), Value::Bool(true)),
            ("n".into(), Value::Int(3)),
        ]));
        net.reduce(500, schedule).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(54)));
        assert_eq!(net.live_agents(), 0);
    }
}

fn rejected_body_metrics(size: usize) -> (usize, usize) {
    let mut store = Store::new();
    let zero = int(&mut store, 0);
    let one = int(&mut store, 1);
    let truth = bool_(&mut store, true);
    let falsity = bool_(&mut store, false);
    let p0 = store.intern(Node::Param(0));
    let p1 = store.intern(Node::Param(1));
    let first_guard = store.intern(Node::Eq(p0, falsity));
    let mut dormant = p1;
    for _ in 0..size {
        dormant = store.intern(Node::Add(dormant, one));
    }
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![
            Clause {
                guard: first_guard,
                body: zero,
            },
            Clause {
                guard: truth,
                body: dormant,
            },
        ],
    });
    let flag = store.intern(Node::Hole("flag".into()));
    let ignored = store.intern(Node::Hole("ignored".into()));
    let root = store.intern(Node::Dispatch {
        family,
        arguments: vec![flag, ignored],
    });
    let mut net = lower::to_inet(&store, root, "result").unwrap();
    let initial = net.live_agents();
    net.bind(&BTreeMap::from([
        ("flag".into(), Value::Bool(false)),
        ("ignored".into(), Value::Int(9)),
    ]));
    let stats = net.reduce(100, Schedule::LowestWire).unwrap();
    assert_eq!(net.outputs().get("result"), Some(&Value::Int(0)));
    (initial, stats.peak_live_agents)
}

#[test]
fn rejected_template_size_does_not_allocate_live_agents() {
    assert_eq!(rejected_body_metrics(10), rejected_body_metrics(10_000));
}

#[test]
fn lowering_rejects_a_call_that_would_make_an_undemanded_argument_strict() {
    let mut store = Store::new();
    let truth = bool_(&mut store, true);
    let one = int(&mut store, 1);
    let p1 = store.intern(Node::Param(1));
    let family = store.intern(Node::Family {
        parameters: 2,
        clauses: vec![Clause {
            guard: truth,
            body: p1,
        }],
    });
    let invalid = store.intern(Node::Add(one, truth));
    let call = store.intern(Node::Dispatch {
        family,
        arguments: vec![invalid, one],
    });
    assert!(matches!(
        lower::to_inet(&store, call, "result"),
        Err(lower::LowerError::UnsupportedNode(_))
    ));
}
