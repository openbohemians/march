//! Shared differential contracts for isolated demand-protocol probes.
//!
//! These machines are not the principal-port INet backend. Passing these tests
//! does not establish arbitrary rewrite-schedule independence, serialization,
//! or higher-order code duplication. The CAS reducer is the semantic control.

use march_research::demand::{Fault, Outcome, ProbeError, ProbeRun, Program, Scalar};
use march_research::{Atom, Bindings, Cid, Node, ReduceError, Reducer, Store};
use std::collections::BTreeMap;

const BUDGET: usize = 1_000_000;
type Facts = BTreeMap<String, Scalar>;

enum Machine {
    A(march_research::inet_demand_a::Machine),
    B(march_research::inet_demand_b::Machine),
    C(march_research::inet_demand_c::Machine),
}

impl Machine {
    fn run(&mut self, facts: &Facts, budget: usize) -> Result<ProbeRun, ProbeError> {
        match self {
            Self::A(machine) => machine.run(facts, budget),
            Self::B(machine) => {
                let result = machine.run(facts, budget);
                machine.audit().expect("B quiescent invariants after run");
                result
            }
            Self::C(machine) => {
                let result = machine.run(facts, budget);
                machine.audit().expect("C quiescent invariants after run");
                result
            }
        }
    }
}

fn machines(store: &Store, root: Cid) -> [Machine; 4] {
    use march_research::inet_demand_c::Topology;
    let program = Program::from_store(store, root).unwrap();
    [
        Machine::A(march_research::inet_demand_a::Machine::new(program.clone())),
        Machine::B(march_research::inet_demand_b::Machine::new(program.clone())),
        Machine::C(march_research::inet_demand_c::Machine::with_topology(
            program.clone(),
            Topology::ReplyOnData,
        )),
        Machine::C(march_research::inet_demand_c::Machine::with_topology(
            program,
            Topology::ReplyOnControl,
        )),
    ]
}

fn int(store: &mut Store, n: i64) -> Cid {
    store.intern(Node::Const(Atom::Int(n)))
}

fn boolean(store: &mut Store, b: bool) -> Cid {
    store.intern(Node::Const(Atom::Bool(b)))
}

fn hole(store: &mut Store, name: &str) -> Cid {
    store.intern(Node::Hole(name.into()))
}

fn facts(entries: &[(&str, Scalar)]) -> Facts {
    entries
        .iter()
        .map(|(name, value)| ((*name).into(), value.clone()))
        .collect()
}

fn control(store: &mut Store, root: Cid, facts: &Facts) -> Outcome {
    let mut bindings = Bindings::new();
    for (name, value) in facts {
        let value = match value {
            Scalar::Int(n) => int(store, *n),
            Scalar::Bool(b) => boolean(store, *b),
        };
        bindings.insert(name, value);
    }
    match Reducer::with_budget(store, &bindings, BUDGET).run(root) {
        Ok(reduction) => match store.get(reduction.root) {
            Some(Node::Const(Atom::Int(n))) => Outcome::Value(Scalar::Int(*n)),
            Some(Node::Const(Atom::Bool(b))) => Outcome::Value(Scalar::Bool(*b)),
            _ => Outcome::Unknown,
        },
        Err(ReduceError::IntegerOverflow(operation)) => Outcome::Error(Fault::Overflow(operation)),
        Err(ReduceError::Type(message)) => Outcome::Error(Fault::Type(message)),
        other => panic!("unexpected scalar control result: {other:?}"),
    }
}

fn check(store: &mut Store, root: Cid, facts: &Facts) -> Outcome {
    let expected = control(store, root, facts);
    for (variant, mut machine) in machines(store, root).into_iter().enumerate() {
        let run = machine.run(facts, BUDGET).unwrap();
        assert_eq!(
            run.outcome,
            expected,
            "variant {variant}: {}",
            store.format(root)
        );
    }
    expected
}

#[test]
fn unknown_operand_does_not_block_static_island_in_either_position() {
    for unknown_on_left in [true, false] {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let six = int(&mut store, 6);
        let seven = int(&mut store, 7);
        let island = store.intern(Node::Mul(six, seven));
        let root = store.intern(if unknown_on_left {
            Node::Add(x, island)
        } else {
            Node::Add(island, x)
        });
        assert_eq!(control(&mut store, root, &Facts::new()), Outcome::Unknown);
        for mut machine in machines(&store, root) {
            let first = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(first.outcome, Outcome::Unknown);
            assert_eq!(first.stats.node_evaluations.get(&island), Some(&1));
            let second = machine
                .run(&facts(&[("x", Scalar::Int(1))]), BUDGET)
                .unwrap();
            assert_eq!(second.outcome, Outcome::Value(Scalar::Int(43)));
            assert_eq!(second.stats.node_evaluations.get(&island), Some(&1));
        }
    }
}

#[test]
fn demanded_overflow_surfaces_on_either_side_of_unknown() {
    let mut store = Store::new();
    let x = hole(&mut store, "x");
    let max = int(&mut store, i64::MAX);
    let one = int(&mut store, 1);
    let bad = store.intern(Node::Add(max, one));
    for node in [Node::Add(x, bad), Node::Add(bad, x)] {
        let root = store.intern(node);
        assert_eq!(
            check(&mut store, root, &Facts::new()),
            Outcome::Error(Fault::Overflow("add"))
        );
    }
}

#[test]
fn faults_match_reference_order_including_deferred_type_checks() {
    let mut store = Store::new();
    let truth = boolean(&mut store, true);
    let one = int(&mut store, 1);
    let two = int(&mut store, 2);
    let max = int(&mut store, i64::MAX);
    let x = hole(&mut store, "x");
    let bad_add = store.intern(Node::Add(max, one));
    let bad_mul = store.intern(Node::Mul(max, two));
    let bad_type = store.intern(Node::Add(truth, one));
    let cases = [
        (Node::Add(bad_add, bad_mul), Fault::Overflow("add")),
        (Node::Add(bad_mul, bad_add), Fault::Overflow("multiply")),
        (Node::Add(truth, bad_mul), Fault::Overflow("multiply")),
        (
            Node::Add(bad_type, bad_mul),
            Fault::Type("add expects two integers"),
        ),
        (Node::Add(x, truth), Fault::Type("add expects two integers")),
        (
            Node::Mul(truth, x),
            Fault::Type("multiply expects two integers"),
        ),
        (
            Node::If {
                condition: one,
                when_true: bad_add,
                when_false: bad_mul,
            },
            Fault::Type("if condition is not a boolean"),
        ),
    ];
    for (node, expected) in cases {
        let root = store.intern(node);
        assert_eq!(
            check(&mut store, root, &Facts::new()),
            Outcome::Error(expected)
        );
    }
}

#[test]
fn branches_stay_dormant_until_selection_and_unused_overflow_is_erased() {
    for selected in [true, false] {
        let mut store = Store::new();
        let flag = hole(&mut store, "flag");
        let max = int(&mut store, i64::MAX);
        let one = int(&mut store, 1);
        let bad = store.intern(Node::Add(max, one));
        let root = store.intern(Node::If {
            condition: flag,
            when_true: if selected { one } else { bad },
            when_false: if selected { bad } else { one },
        });
        for mut machine in machines(&store, root) {
            let pending = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(pending.outcome, Outcome::Unknown);
            assert_eq!(
                pending
                    .stats
                    .node_evaluations
                    .get(&bad)
                    .copied()
                    .unwrap_or(0),
                0
            );
            let done = machine
                .run(&facts(&[("flag", Scalar::Bool(selected))]), BUDGET)
                .unwrap();
            assert_eq!(done.outcome, Outcome::Value(Scalar::Int(1)));
            assert_eq!(
                done.stats.node_evaluations.get(&bad).copied().unwrap_or(0),
                0
            );
            assert!(done.stats.erasures > 0);
        }
    }
}

#[test]
fn erasing_one_branch_preserves_shared_computation_for_surviving_consumers() {
    for (flag_value, expected) in [(true, 85), (false, 126)] {
        let mut store = Store::new();
        let flag = hole(&mut store, "flag");
        let six = int(&mut store, 6);
        let seven = int(&mut store, 7);
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let shared = store.intern(Node::Mul(six, seven));
        let yes = store.intern(Node::Add(shared, one));
        let no = store.intern(Node::Mul(shared, two));
        let branch = store.intern(Node::If {
            condition: flag,
            when_true: yes,
            when_false: no,
        });
        let root = store.intern(Node::Add(branch, shared));
        let bindings = facts(&[("flag", Scalar::Bool(flag_value))]);
        assert_eq!(
            control(&mut store, root, &bindings),
            Outcome::Value(Scalar::Int(expected))
        );
        for mut machine in machines(&store, root) {
            let done = machine.run(&bindings, BUDGET).unwrap();
            assert_eq!(done.outcome, Outcome::Value(Scalar::Int(expected)));
            assert_eq!(done.stats.node_evaluations.get(&shared), Some(&1));
            assert_eq!(
                done.stats
                    .node_evaluations
                    .get(&if flag_value { no } else { yes })
                    .copied()
                    .unwrap_or(0),
                0
            );
            assert!(done.stats.memo_hits > 0);
        }
    }
}

#[test]
fn erasing_all_consumers_never_evaluates_their_shared_computation() {
    let mut store = Store::new();
    let truth = boolean(&mut store, true);
    let falsity = boolean(&mut store, false);
    let one = int(&mut store, 1);
    let two = int(&mut store, 2);
    let max = int(&mut store, i64::MAX);
    let bad = store.intern(Node::Mul(max, two));
    let left = store.intern(Node::If {
        condition: falsity,
        when_true: bad,
        when_false: one,
    });
    let right = store.intern(Node::If {
        condition: truth,
        when_true: two,
        when_false: bad,
    });
    let root = store.intern(Node::Add(left, right));
    assert_eq!(
        control(&mut store, root, &Facts::new()),
        Outcome::Value(Scalar::Int(3))
    );
    for mut machine in machines(&store, root) {
        let run = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(3)));
        assert_eq!(
            run.stats.node_evaluations.get(&bad).copied().unwrap_or(0),
            0
        );
        assert_eq!(
            run.stats.node_evaluations.get(&max).copied().unwrap_or(0),
            0
        );
    }
}

#[test]
fn shared_partial_computation_is_not_retried_by_second_consumer_in_same_epoch() {
    let mut store = Store::new();
    let y = hole(&mut store, "y");
    let six = int(&mut store, 6);
    let seven = int(&mut store, 7);
    let island = store.intern(Node::Mul(six, seven));
    let shared = store.intern(Node::Add(y, island));
    let root = store.intern(Node::Add(shared, shared));
    for mut machine in machines(&store, root) {
        let first = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(first.outcome, Outcome::Unknown);
        assert_eq!(first.stats.node_evaluations.get(&shared), Some(&1));
        assert_eq!(first.stats.node_evaluations.get(&island), Some(&1));
        let second = machine
            .run(&facts(&[("y", Scalar::Int(1))]), BUDGET)
            .unwrap();
        assert_eq!(second.outcome, Outcome::Value(Scalar::Int(86)));
        assert_eq!(second.stats.node_evaluations.get(&shared), Some(&2));
        assert_eq!(second.stats.node_evaluations.get(&island), Some(&1));
    }
}

#[test]
fn nested_sharing_does_not_expand_into_exponential_evaluation() {
    let mut store = Store::new();
    let mut root = int(&mut store, 1);
    let mut nodes = vec![root];
    for _ in 0..30 {
        root = store.intern(Node::Add(root, root));
        nodes.push(root);
    }
    for mut machine in machines(&store, root) {
        let done = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(done.outcome, Outcome::Value(Scalar::Int(1 << 30)));
        for node in &nodes {
            assert_eq!(done.stats.node_evaluations.get(node), Some(&1));
        }
        assert_eq!(done.stats.evaluations, nodes.len());
        assert!(done.stats.transitions < 10_000);
    }
}

#[test]
fn conflicting_bindings_are_rejected_atomically() {
    let mut store = Store::new();
    let x = hole(&mut store, "x");
    let a = hole(&mut store, "a");
    let root = store.intern(Node::Add(a, x));
    for mut machine in machines(&store, root) {
        assert_eq!(
            machine
                .run(&facts(&[("x", Scalar::Int(1))]), BUDGET)
                .unwrap()
                .outcome,
            Outcome::Unknown
        );
        // `a` sorts before conflicting `x`: a sequential merge must not leak it.
        assert!(
            machine
                .run(
                    &facts(&[("a", Scalar::Int(99)), ("x", Scalar::Int(2))]),
                    BUDGET
                )
                .is_err()
        );
        assert_eq!(
            machine.run(&Facts::new(), BUDGET).unwrap().outcome,
            Outcome::Unknown
        );
        assert_eq!(
            machine
                .run(&facts(&[("a", Scalar::Int(3))]), BUDGET)
                .unwrap()
                .outcome,
            Outcome::Value(Scalar::Int(4))
        );
    }
}

#[test]
fn an_error_around_unknown_can_change_when_more_facts_arrive() {
    let mut store = Store::new();
    let x = hole(&mut store, "x");
    let two = int(&mut store, 2);
    let truth = boolean(&mut store, true);
    let product = store.intern(Node::Mul(x, two));
    let root = store.intern(Node::Add(product, truth));
    let later = facts(&[("x", Scalar::Int(i64::MAX))]);
    let first_expected = control(&mut store, root, &Facts::new());
    let second_expected = control(&mut store, root, &later);
    assert_eq!(
        first_expected,
        Outcome::Error(Fault::Type("add expects two integers"))
    );
    assert_eq!(second_expected, Outcome::Error(Fault::Overflow("multiply")));
    for mut machine in machines(&store, root) {
        assert_eq!(
            machine.run(&Facts::new(), BUDGET).unwrap().outcome,
            first_expected
        );
        assert_eq!(
            machine.run(&later, BUDGET).unwrap().outcome,
            second_expected
        );
    }
}

#[test]
fn bounded_runs_can_be_retried_without_corrupting_pending_or_cached_work() {
    let mut store = Store::new();
    let x = hole(&mut store, "x");
    let one = int(&mut store, 1);
    let mut root = x;
    for _ in 0..20 {
        root = store.intern(Node::Add(root, one));
    }
    let bindings = facts(&[("x", Scalar::Int(3))]);
    for budget in [0, 1, 2, 5, 13, 40] {
        for mut machine in machines(&store, root) {
            match machine.run(&bindings, budget) {
                Err(ProbeError::BudgetExhausted { limit }) => assert_eq!(limit, budget),
                Ok(run) => assert_eq!(run.outcome, Outcome::Value(Scalar::Int(23))),
                other => panic!("unexpected budget result: {other:?}"),
            }
            // The protocol may restart its control path; it need not expose
            // a serializable mid-transition continuation for this experiment.
            assert_eq!(
                machine.run(&bindings, BUDGET).unwrap().outcome,
                Outcome::Value(Scalar::Int(23))
            );
        }
    }
}

#[test]
fn every_budget_cut_preserves_branch_erasure_and_later_epoch_reentry() {
    for error_case in [false, true] {
        let mut store = Store::new();
        let flag = hole(&mut store, "flag");
        let x = hole(&mut store, "x");
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let max = int(&mut store, i64::MAX);
        let truth = boolean(&mut store, true);
        let discarded = store.intern(Node::Add(max, one));
        let selected = if error_case {
            let multiply = store.intern(Node::Mul(x, two));
            store.intern(Node::Add(multiply, truth))
        } else {
            store.intern(Node::Add(x, one))
        };
        let root = store.intern(Node::If {
            condition: flag,
            when_true: selected,
            when_false: discarded,
        });
        let initial = facts(&[("flag", Scalar::Bool(true))]);
        let final_facts = facts(&[
            ("flag", Scalar::Bool(true)),
            ("x", Scalar::Int(if error_case { i64::MAX } else { 3 })),
        ]);
        let expected_initial = control(&mut store, root, &initial);
        let expected_final = control(&mut store, root, &final_facts);
        for cut in 0..45 {
            for mut machine in machines(&store, root) {
                match machine.run(&initial, cut) {
                    Ok(result) => assert_eq!(result.outcome, expected_initial),
                    Err(ProbeError::BudgetExhausted { .. }) => {}
                    other => panic!("unexpected cut result: {other:?}"),
                }
                assert_eq!(
                    machine.run(&initial, BUDGET).unwrap().outcome,
                    expected_initial
                );
                let final_run = machine.run(&final_facts, BUDGET).unwrap();
                assert_eq!(final_run.outcome, expected_final);
                assert_eq!(final_run.stats.node_evaluations.get(&discarded), None);
            }
        }
    }
}

// Deterministic, dependency-free graph generator. Operands are chosen from an
// existing pool, deliberately introducing DAG sharing and ill-typed programs.
fn random(seed: &mut u64) -> usize {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 32) as usize
}

#[test]
fn generated_budget_cuts_allow_retry_then_new_facts() {
    let mut seed = 0x43555453_u64;
    for case in 0..24 {
        let mut store = Store::new();
        let mut pool = vec![
            int(&mut store, 1),
            int(&mut store, i64::MAX),
            boolean(&mut store, true),
            hole(&mut store, "x"),
            hole(&mut store, "flag"),
        ];
        for _ in 0..10 {
            let a = pool[random(&mut seed) % pool.len()];
            let b = pool[random(&mut seed) % pool.len()];
            let c = pool[random(&mut seed) % pool.len()];
            pool.push(store.intern(match random(&mut seed) % 3 {
                0 => Node::Add(a, b),
                1 => Node::Mul(a, b),
                _ => Node::If {
                    condition: a,
                    when_true: b,
                    when_false: c,
                },
            }));
        }
        let root = *pool.last().unwrap();
        let initial = Facts::new();
        let later = facts(&[("x", Scalar::Int(2)), ("flag", Scalar::Bool(true))]);
        let initial_expected = control(&mut store, root, &initial);
        let later_expected = control(&mut store, root, &later);
        for cut in 0..80 {
            for (variant, mut machine) in machines(&store, root).into_iter().enumerate() {
                match machine.run(&initial, cut) {
                    Ok(run) => assert_eq!(run.outcome, initial_expected),
                    Err(ProbeError::BudgetExhausted { .. }) => {}
                    other => panic!("case {case} variant {variant} cut {cut}: {other:?}"),
                }
                assert_eq!(
                    machine.run(&initial, BUDGET).unwrap().outcome,
                    initial_expected
                );
                assert_eq!(machine.run(&later, BUDGET).unwrap().outcome, later_expected);
            }
        }
    }
}

#[test]
fn dormant_branch_cleanup_is_not_bounded_by_transition_fuel() {
    let mut store = Store::new();
    let one = int(&mut store, 1);
    let seven = int(&mut store, 7);
    let yes = boolean(&mut store, true);
    let mut discarded = one;
    for _ in 0..2_000 {
        discarded = store.intern(Node::Add(discarded, one));
    }
    let root = store.intern(Node::If {
        condition: yes,
        when_true: seven,
        when_false: discarded,
    });
    for (variant, mut machine) in machines(&store, root).into_iter().enumerate() {
        let run = machine.run(&Facts::new(), 100).unwrap();
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(7)));
        assert_eq!(run.stats.node_evaluations.get(&discarded), None);
        assert!(run.stats.transitions <= 100);
        if variant != 0 {
            // Characterize a limitation, not a bounded-work success: B/C
            // walk the unused template's edges inside a release cascade.
            assert!(run.stats.erasures >= 4_000);
        }
    }
}

#[test]
fn generated_dags_match_exact_reference_outcomes() {
    let mut seed = 0x4d41524348_u64;
    for case in 0..180 {
        let mut store = Store::new();
        let mut pool = vec![
            int(&mut store, 0),
            int(&mut store, 1),
            int(&mut store, -3),
            int(&mut store, i64::MAX),
            boolean(&mut store, true),
            boolean(&mut store, false),
            hole(&mut store, "x"),
            hole(&mut store, "flag"),
        ];
        for _ in 0..18 {
            let a = pool[random(&mut seed) % pool.len()];
            let b = pool[random(&mut seed) % pool.len()];
            let c = pool[random(&mut seed) % pool.len()];
            let node = match random(&mut seed) % 3 {
                0 => Node::Add(a, b),
                1 => Node::Mul(a, b),
                _ => Node::If {
                    condition: a,
                    when_true: b,
                    when_false: c,
                },
            };
            pool.push(store.intern(node));
        }
        let root = *pool.last().unwrap();
        for bindings in [
            Facts::new(),
            facts(&[("x", Scalar::Int(2))]),
            facts(&[("flag", Scalar::Bool(false))]),
            facts(&[("x", Scalar::Int(-1)), ("flag", Scalar::Bool(true))]),
        ] {
            let expected = control(&mut store, root, &bindings);
            for (variant, mut machine) in machines(&store, root).into_iter().enumerate() {
                let actual = machine.run(&bindings, BUDGET).unwrap();
                assert_eq!(
                    actual.outcome,
                    expected,
                    "case {case}, variant {variant}, bindings {bindings:?}, graph {}",
                    store.format(root)
                );
            }
        }
    }
}

#[test]
fn typed_generated_dags_agree_when_facts_arrive_in_either_order() {
    let mut seed = 0x5348415245_u64;
    for case in 0..60 {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let flag = hole(&mut store, "flag");
        let mut pool = vec![
            int(&mut store, -1),
            int(&mut store, 0),
            int(&mut store, 1),
            x,
        ];
        for _ in 0..12 {
            let left = pool[random(&mut seed) % pool.len()];
            let right = pool[random(&mut seed) % pool.len()];
            pool.push(store.intern(match random(&mut seed) % 3 {
                0 => Node::Add(left, right),
                1 => Node::Mul(left, right),
                _ => Node::If {
                    condition: flag,
                    when_true: left,
                    when_false: right,
                },
            }));
        }
        let root = *pool.last().unwrap();
        for flag_first in [false, true] {
            let x_fact = ("x", Scalar::Int(2));
            let flag_fact = ("flag", Scalar::Bool(case % 2 == 0));
            let stages = if flag_first {
                [flag_fact, x_fact]
            } else {
                [x_fact, flag_fact]
            };
            for (variant, mut machine) in machines(&store, root).into_iter().enumerate() {
                let mut accumulated = Facts::new();
                for stage in &stages {
                    accumulated.insert(stage.0.into(), stage.1.clone());
                    let expected = control(&mut store, root, &accumulated);
                    let actual = machine
                        .run(&facts(std::slice::from_ref(stage)), BUDGET)
                        .unwrap();
                    assert_eq!(
                        actual.outcome, expected,
                        "case {case}, variant {variant}, facts {accumulated:?}"
                    );
                }
            }
        }
    }
}
