//! Independent N0 review: outcomes, recovery, and explicit acceptance gaps.
use march_research::demand::{ProbeError, Scalar};
use march_research::inet_n0::{Machine, N0Fault, N0Outcome, Program};
use march_research::{Atom, Bindings, Cid, Node, ReduceError, Reducer, Store};
use std::collections::BTreeMap;

type Facts = BTreeMap<String, Scalar>;
const BUDGET: usize = 100_000;
fn int(s: &mut Store, n: i64) -> Cid {
    s.intern(Node::Const(Atom::Int(n)))
}
fn quote(s: &mut Store, params: u16, body: Cid) -> Cid {
    s.intern(Node::Quote { params, body })
}
fn apply(s: &mut Store, function: Cid, arguments: Vec<Cid>) -> Cid {
    s.intern(Node::Apply {
        function,
        arguments,
    })
}
fn oracle(s: &mut Store, root: Cid, facts: &Facts) -> N0Outcome {
    let mut bindings = Bindings::new();
    for (name, value) in facts {
        let value = s.intern(Node::Const(match value {
            Scalar::Int(n) => Atom::Int(*n),
            Scalar::Bool(b) => Atom::Bool(*b),
        }));
        bindings.insert(name, value);
    }
    match Reducer::with_budget(s, &bindings, BUDGET).run(root) {
        Ok(result) => match s.get(result.root) {
            Some(Node::Const(Atom::Int(n))) => N0Outcome::Value(Scalar::Int(*n)),
            Some(Node::Const(Atom::Bool(b))) => N0Outcome::Value(Scalar::Bool(*b)),
            Some(Node::Quote { .. }) => N0Outcome::Code(result.root),
            _ => N0Outcome::Unknown,
        },
        Err(ReduceError::Type(message)) => N0Outcome::Error(N0Fault::Type(message)),
        Err(ReduceError::IntegerOverflow(op)) => N0Outcome::Error(N0Fault::Overflow(op)),
        Err(ReduceError::Arity { expected, actual }) => {
            N0Outcome::Error(N0Fault::Arity { expected, actual })
        }
        other => panic!("unexpected reference outcome: {other:?}"),
    }
}

#[test]
fn code_conditioned_residuals_preserve_groundness_without_forcing_branches() {
    let mut s = Store::new();
    let one = int(&mut s, 1);
    let max = int(&mut s, i64::MAX);
    let bad = s.intern(Node::Add(max, one));
    let code = quote(&mut s, 0, one);
    let x = s.intern(Node::Hole("x".into()));
    for branch in [one, x, bad] {
        let stuck = s.intern(Node::If {
            condition: code,
            when_true: branch,
            when_false: one,
        });
        let sum = s.intern(Node::Add(stuck, one));
        let product = s.intern(Node::Mul(one, stuck));
        let call = apply(&mut s, stuck, vec![bad]);
        let nested = s.intern(Node::If {
            condition: stuck,
            when_true: x,
            when_false: one,
        });
        let nested_sum = s.intern(Node::Add(nested, one));
        for root in [stuck, sum, product, call, nested_sum] {
            let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
            for facts in [Facts::new(), Facts::from([("x".into(), Scalar::Int(2))])] {
                let expected = oracle(&mut s, root, &facts);
                let run = machine.run(&facts, BUDGET).unwrap();
                assert_eq!(run.outcome, expected, "{}", s.format(root));
                assert_eq!(
                    run.stats.node_evaluations.get(&bad),
                    None,
                    "groundness must not evaluate"
                );
                machine.audit().unwrap();
            }
        }
        // Child errors still precede the parent's ground non-integer check.
        let right_error = s.intern(Node::Add(stuck, bad));
        let mut machine = Machine::new(Program::from_store(&s, right_error).unwrap());
        assert_eq!(
            machine.run(&Facts::new(), BUDGET).unwrap().outcome,
            N0Outcome::Error(N0Fault::Overflow("add"))
        );
        machine.audit().unwrap();
    }
}

#[test]
fn proxy_branch_groundness_survives_every_budget_cut_and_new_facts() {
    let mut s = Store::new();
    let one = int(&mut s, 1);
    let p0 = s.intern(Node::Param(0));
    let condition = quote(&mut s, 0, one);
    let stuck = s.intern(Node::If {
        condition,
        when_true: p0,
        when_false: one,
    });
    let body = s.intern(Node::Add(stuck, one));
    let code = quote(&mut s, 1, body);
    let x = s.intern(Node::Hole("x".into()));
    let root = apply(&mut s, code, vec![x]);
    let initial = Facts::new();
    let later = Facts::from([("x".into(), Scalar::Int(2))]);
    assert_eq!(oracle(&mut s, root, &initial), N0Outcome::Unknown);
    let expected = N0Outcome::Error(N0Fault::Type("add expects two integers"));
    assert_eq!(oracle(&mut s, root, &later), expected);
    for cut in 0..100 {
        let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
        match machine.run(&initial, cut) {
            Ok(run) => assert_eq!(run.outcome, N0Outcome::Unknown),
            Err(ProbeError::BudgetExhausted { .. }) => {}
            other => panic!("cut {cut}: {other:?}"),
        }
        machine.audit().unwrap();
        assert_eq!(
            machine.run(&initial, BUDGET).unwrap().outcome,
            N0Outcome::Unknown
        );
        assert_eq!(machine.run(&later, BUDGET).unwrap().outcome, expected);
        assert_eq!(machine.detail().instances, 1);
        machine.audit().unwrap();
    }
}

#[test]
fn dormant_groundness_does_not_reuse_an_evaluated_branch_memo() {
    let mut s = Store::new();
    let one = int(&mut s, 1);
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let x = s.intern(Node::Hole("x".into()));
    let t = s.intern(Node::If {
        condition: yes,
        when_true: one,
        when_false: x,
    });
    let code = quote(&mut s, 0, one);
    let stuck = s.intern(Node::If {
        condition: code,
        when_true: t,
        when_false: one,
    });
    let right = s.intern(Node::Add(stuck, one));
    let root = s.intern(Node::Add(t, right));
    let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
    let initial = Facts::new();
    assert_eq!(oracle(&mut s, root, &initial), N0Outcome::Unknown);
    assert_eq!(
        machine.run(&initial, BUDGET).unwrap().outcome,
        N0Outcome::Unknown
    );
    let later = Facts::from([("x".into(), Scalar::Int(2))]);
    let expected = N0Outcome::Error(N0Fault::Type("add expects two integers"));
    assert_eq!(oracle(&mut s, root, &later), expected);
    let run = machine.run(&later, BUDGET).unwrap();
    assert_eq!(run.outcome, expected);
    assert_eq!(run.stats.node_evaluations[&t], 1);
    assert_eq!(run.stats.node_evaluations.get(&x), None);
    machine.audit().unwrap();
}

#[test]
fn characterize_missing_canonical_sharing_across_and_within_instances() {
    // These are acceptance-gap witnesses, not successful sharing tests.
    for same_instance in [false, true] {
        let mut s = Store::new();
        let one = int(&mut s, 1);
        let two = int(&mut s, 2);
        let six = int(&mut s, 6);
        let seven = int(&mut s, 7);
        let (root, work, expected_rewrites) = if same_instance {
            let p0 = s.intern(Node::Param(0));
            let p1 = s.intern(Node::Param(1));
            let a = s.intern(Node::Mul(p0, seven));
            let b = s.intern(Node::Mul(p1, seven));
            let body = s.intern(Node::Add(a, b));
            let code = quote(&mut s, 2, body);
            (apply(&mut s, code, vec![six, six]), vec![a, b], 3)
        } else {
            let work = s.intern(Node::Mul(six, seven));
            let code = quote(&mut s, 1, work);
            let a = apply(&mut s, code, vec![one]);
            let b = apply(&mut s, code, vec![two]);
            (s.intern(Node::Add(a, b)), vec![work], 4)
        };
        let reference = Reducer::with_budget(&mut s, &Bindings::new(), BUDGET)
            .run(root)
            .unwrap();
        assert_eq!(s.get(reference.root), Some(&Node::Const(Atom::Int(84))));
        // Only one multiplication rewrite after canonical substitution; the
        // others are Add and one/two Apply rewrites respectively.
        assert_eq!(reference.stats.rewritten, expected_rewrites);
        let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
        let run = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(run.outcome, N0Outcome::Value(Scalar::Int(84)));
        assert_eq!(
            work.iter()
                .map(|cid| run.stats.node_evaluations[cid])
                .sum::<usize>(),
            2
        );
        machine.audit().unwrap();
    }
}

#[test]
fn instance_records_and_argument_vectors_outlive_logical_agent_reclamation() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let body = s.intern(Node::Mul(p, p));
    let square = quote(&mut s, 1, body);
    let mut root = int(&mut s, 1);
    for _ in 0..30 {
        root = apply(&mut s, square, vec![root]);
    }
    let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
    let result = machine.run(&Facts::new(), BUDGET).unwrap();
    assert_eq!(result.outcome, N0Outcome::Value(Scalar::Int(1)));
    assert_eq!(result.stats.live, 2);
    let storage = machine.storage();
    assert_eq!(storage.instance_records, 30);
    assert_eq!(storage.argument_keys, 30);
    assert!(storage.agents.vacant_slots > 30);
    assert!(storage.agents.use_count_entries > 30);
    machine.audit().unwrap();
}

#[test]
fn conflicts_remain_atomic_after_partial_application_work() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let body = s.intern(Node::Add(p, p));
    let code = quote(&mut s, 1, body);
    let x = s.intern(Node::Hole("x".into()));
    let root = apply(&mut s, code, vec![x]);
    let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
    let initial = Facts::from([("z".into(), Scalar::Int(1))]);
    let before = machine.run(&initial, BUDGET).unwrap();
    let storage = machine.storage();
    let conflict = Facts::from([("a".into(), Scalar::Int(7)), ("z".into(), Scalar::Int(2))]);
    assert!(
        matches!(machine.run(&conflict, BUDGET), Err(ProbeError::ConflictingBinding(name)) if name == "z")
    );
    assert_eq!(machine.storage(), storage);
    machine.audit().unwrap();
    let later = Facts::from([("a".into(), Scalar::Int(8)), ("x".into(), Scalar::Int(3))]);
    let result = machine.run(&later, BUDGET).unwrap();
    assert_eq!(before.outcome, N0Outcome::Unknown);
    assert_eq!(result.outcome, N0Outcome::Value(Scalar::Int(6)));
    assert_eq!(machine.detail().instances, 1);
    machine.audit().unwrap();
}

#[test]
fn characterize_canonical_cycle_gap_and_safe_budget_cancellation() {
    let mut s = Store::new();
    let p = s.intern(Node::Param(0));
    let body = apply(&mut s, p, vec![p]);
    let code = quote(&mut s, 1, body);
    let root = apply(&mut s, code, vec![code]);
    let reference = Reducer::with_budget(&mut s, &Bindings::new(), 500).run(root);
    // Same identity gap, now observable as an error rather than work count:
    // the reference recognizes re-entry of an active canonical Apply CID;
    // fresh per-call keys in this probe continue until fuel is exhausted.
    assert!(
        matches!(reference, Err(ReduceError::Cycle(cid)) if cid == root),
        "{reference:?}"
    );
    let mut machine = Machine::new(Program::from_store(&s, root).unwrap());
    for budget in [200, 500, 1000] {
        assert!(matches!(
            machine.run(&Facts::new(), budget),
            Err(ProbeError::BudgetExhausted { .. })
        ));
        machine.audit().unwrap();
    }
    assert!(machine.detail().instances > 1);
    // Fuel exhaustion is not a semantic error or an empirical proof of
    // divergence. It must also not prevent erasing this whole computation.
    let yes = s.intern(Node::Const(Atom::Bool(true)));
    let seven = int(&mut s, 7);
    let discard = s.intern(Node::If {
        condition: yes,
        when_true: seven,
        when_false: root,
    });
    let mut machine = Machine::new(Program::from_store(&s, discard).unwrap());
    assert_eq!(
        machine.run(&Facts::new(), BUDGET).unwrap().outcome,
        N0Outcome::Value(Scalar::Int(7))
    );
    assert_eq!(machine.detail().instances, 0);
    machine.audit().unwrap();
}
