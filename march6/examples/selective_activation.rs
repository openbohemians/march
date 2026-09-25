//! Selective conditional activation, not a general demand evaluator.
//! Every run is capped at 128 local rewrites, including erasure and Spin.
//! Retired rules: git show 6f17b9b:march6/SELECTIVE-ACTIVATION.md.

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Need,
    IfDormant,
    Select,
    Bool(bool),
    Int(i64),
    Add,
    AddK(i64),
    Mul,
    MulK(i64),
    Fan,
    Erase,
    Gate,
    Release,
    Stop,
    Observe,
    Answer(i64),
    Tick,
    Spin,
}

impl Kind {
    fn ports(&self) -> usize {
        match self {
            Self::IfDormant | Self::Select => 6,
            Self::Gate => 4,
            Self::Add | Self::Mul | Self::Fan => 3,
            Self::Need | Self::AddK(_) | Self::MulK(_) | Self::Observe | Self::Spin => 2,
            _ => 1,
        }
    }
}

include!("support/port_runner.rs");

fn rule(a: &Kind, b: &Kind) -> Option<Result<Rule, Halt>> {
    use Kind::*;
    let r = match (a, b) {
        // External order: Need.result; If.cond,tc,tv,fc,fv.
        (Need, IfDormant) => Rule::new(
            "invoke",
            vec![Select],
            vec![
                (N(0, 0), E(1)),
                (N(0, 1), E(0)),
                (N(0, 2), E(2)),
                (N(0, 3), E(3)),
                (N(0, 4), E(4)),
                (N(0, 5), E(5)),
            ],
        ),
        // External order: Select.result,tc,tv,fc,fv.
        (Bool(true), Select) => Rule::new(
            "choose-true",
            vec![Release, Erase, Erase],
            vec![
                (E(0), E(2)),
                (N(0, 0), E(1)),
                (N(1, 0), E(3)),
                (N(2, 0), E(4)),
            ],
        ),
        (Bool(false), Select) => Rule::new(
            "choose-false",
            vec![Release, Erase, Erase],
            vec![
                (E(0), E(4)),
                (N(0, 0), E(3)),
                (N(1, 0), E(1)),
                (N(2, 0), E(2)),
            ],
        ),
        (Release, Gate) => Rule::new(
            "release",
            vec![Release],
            vec![(E(1), E(2)), (N(0, 0), E(0))],
        ),
        (Release, Stop) => Rule::new("stop", vec![], vec![]),
        (Int(n), Add) => Rule::new(
            "add-left",
            vec![AddK(*n)],
            vec![(N(0, 0), E(0)), (N(0, 1), E(1))],
        ),
        (Int(n), Mul) => Rule::new(
            "mul-left",
            vec![MulK(*n)],
            vec![(N(0, 0), E(0)), (N(0, 1), E(1))],
        ),
        (Int(m), AddK(n)) => {
            let Some(v) = n.checked_add(*m) else {
                return Some(Err(Halt::NumericDomainLimit));
            };
            Rule::new("add-done", vec![Int(v)], vec![(N(0, 0), E(0))])
        }
        (Int(m), MulK(n)) => {
            let Some(v) = n.checked_mul(*m) else {
                return Some(Err(Halt::NumericDomainLimit));
            };
            Rule::new("mul-done", vec![Int(v)], vec![(N(0, 0), E(0))])
        }
        (Int(n), Fan) => Rule::new(
            "copy-value",
            vec![Int(*n), Int(*n)],
            vec![(N(0, 0), E(0)), (N(1, 0), E(1))],
        ),
        (Int(n), Observe) => Rule::new("observe", vec![Answer(*n)], vec![(N(0, 0), E(0))]),
        (Tick, Spin) => Rule::new(
            "spin",
            vec![Tick, Spin],
            vec![(N(0, 0), N(1, 0)), (N(1, 1), E(0))],
        ),
        // One eraser on each auxiliary. This expands to a fixed small RHS
        // using only the matched agent label, not a host subnet traversal.
        (Erase, k) => {
            let name = match k {
                Gate => "erase-gate",
                Add => "erase-add",
                Mul => "erase-mul",
                AddK(_) => "erase-add-k",
                MulK(_) => "erase-mul-k",
                Fan => "erase-fan",
                Spin => "erase-spin",
                Tick => "erase-tick",
                Stop => "erase-stop",
                Erase => "erase-erase",
                Int(_) => "erase-value",
                _ => return None,
            };
            let count = k.ports() - 1;
            Rule::new(
                name,
                vec![Erase; count],
                (0..count).map(|i| (N(i, 0), E(i))).collect(),
            )
        }
        // In particular: no Fan/IfDormant or generic template activation rule.
        _ => return None,
    };
    Some(Ok(r))
}

fn fixture(condition: Option<bool>, divergent: bool, invoked: bool) -> Net {
    let mut n = Net::default();
    let obs = n.observe("result");
    let root = n.add(Kind::IfDormant);
    let g1 = n.add(Kind::Gate);
    let g2 = n.add(Kind::Gate);
    let stop = n.add(Kind::Stop);
    n.wire(root, 2, g1, 0);
    n.wire(g1, 1, g2, 0);
    n.wire(g2, 1, stop, 0);
    let (input, producer, out) = if divergent {
        (n.add(Kind::Tick), n.add(Kind::Spin), 1)
    } else {
        let input = n.add(Kind::Int(2));
        let mul = n.add(Kind::Mul);
        let rhs = n.add(Kind::Int(3));
        n.wire(mul, 1, rhs, 0);
        (input, mul, 2)
    };
    let fan = n.add(Kind::Fan);
    let add = n.add(Kind::Add);
    n.wire(input, 0, g1, 2);
    n.wire(producer, 0, g1, 3);
    n.wire(producer, out, fan, 0);
    n.wire(fan, 1, g2, 2);
    n.wire(g2, 3, add, 0);
    n.wire(fan, 2, add, 1);
    n.wire(add, 2, root, 3);

    let zero = n.add(Kind::Int(0));
    n.gate(End::Port(zero, 0), End::Port(root, 5), false);
    let control = take_last_boundary(&mut n, "control");
    n.link(control, End::Port(root, 4));

    if let Some(b) = condition {
        let value = n.add(Kind::Bool(b));
        n.wire(value, 0, root, 1);
    } else {
        let condition = n.boundary("condition");
        n.link(condition, End::Port(root, 1));
    }
    let entry = n.boundary("entry");
    let request_result = n.boundary("request-result");
    n.link(entry, End::Port(root, 0));
    n.link(request_result, End::Port(obs, 0));
    if invoked {
        invoke(&mut n);
    }
    n.audit();
    n
}

// Host fixture actions at declared interfaces, not rewrite rules. Only the
// most recently added boundary is removed, so existing endpoint IDs stay valid.
fn take_last_boundary(n: &mut Net, name: &str) -> End {
    assert_eq!(n.boundaries.last().unwrap().0, name);
    let id = n.boundaries.len() - 1;
    let target = n.unlink(End::Boundary(id));
    assert!(n.boundaries.pop().unwrap().1.is_none());
    target
}

fn invoke(n: &mut Net) {
    let result = take_last_boundary(n, "request-result");
    let entry = take_last_boundary(n, "entry");
    let need = n.add(Kind::Need);
    n.link(End::Port(need, 0), entry);
    n.link(End::Port(need, 1), result);
    n.audit();
}

fn supply_condition(n: &mut Net, b: bool) {
    let input = take_last_boundary(n, "condition");
    let value = n.add(Kind::Bool(b));
    n.link(End::Port(value, 0), input);
    n.audit();
}

fn show(name: &str, mut n: Net, order: Order, trace: bool) {
    let halt = n.run(order, MAX_INTERACTIONS_PER_RUN);
    println!(
        "{}; spin={}; release={}",
        n.summary(name, order, halt),
        n.count("spin"),
        n.count("release")
    );
    if trace {
        for (i, step) in n.trace.iter().enumerate() {
            println!("  {}. {step}", i + 1);
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let trace = match args.as_slice() {
        [] => false,
        [arg] if arg == "--trace" => true,
        _ => {
            eprintln!("usage: selective_activation [--trace]");
            std::process::exit(2);
        }
    };
    for order in [Order::Oldest, Order::Youngest] {
        show("uninvoked", fixture(Some(true), false, false), order, trace);
        show("unknown", fixture(None, false, true), order, trace);
        show(
            "false-finite",
            fixture(Some(false), false, true),
            order,
            trace,
        );
        show("false-spin", fixture(Some(false), true, true), order, trace);
        show(
            "true-finite",
            fixture(Some(true), false, true),
            order,
            trace,
        );
        show("true-spin", fixture(Some(true), true, true), order, trace);
        let mut late = fixture(None, false, true);
        assert_eq!(late.run(order, 1), Halt::Quiescent);
        supply_condition(&mut late, true);
        show("late-true-finite", late, order, trace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(n: &Net, answer: i64, steps: usize) {
        assert_eq!(n.answers(), vec![("result".into(), answer)]);
        assert_eq!(n.rules.len(), steps);
        assert_eq!(n.live(), 1);
        assert!(n.pairs().is_empty());
        assert_eq!(n.count("invoke"), 1);
        n.audit();
    }

    fn no_work(n: &Net) {
        for name in [
            "add-left",
            "add-done",
            "mul-left",
            "mul-done",
            "copy-value",
            "spin",
        ] {
            assert_eq!(n.count(name), 0, "unexpected {name}");
        }
    }

    #[test]
    fn no_invocation_means_no_execution_even_with_known_condition() {
        for divergent in [false, true] {
            let mut n = fixture(Some(true), divergent, false);
            assert_eq!(n.run(Order::Oldest, 128), Halt::Quiescent);
            assert!(n.rules.is_empty());
            invoke(&mut n);
            let halt = n.run(Order::Oldest, 128);
            assert_eq!(
                halt,
                if divergent {
                    Halt::Budget
                } else {
                    Halt::Quiescent
                }
            );
        }
    }

    #[test]
    fn unknown_condition_holds_both_branches() {
        for divergent in [false, true] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = fixture(None, divergent, true);
                assert_eq!(n.run(order, 128), Halt::Quiescent);
                assert_eq!(n.rules, ["invoke"]);
                assert_eq!(n.count("release"), 0);
                assert_eq!(n.live(), if divergent { 12 } else { 13 });
                no_work(&n);
            }
        }
    }

    #[test]
    fn false_erases_finite_and_divergent_work_without_starting_it() {
        for divergent in [false, true] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = fixture(Some(false), divergent, true);
                assert_eq!(n.run(order, 128), Halt::Quiescent);
                finished(&n, 0, if divergent { 15 } else { 16 });
                no_work(&n);
                assert_eq!(n.count("release"), 1);
            }
        }
    }

    #[test]
    fn true_reduces_shared_work_once() {
        for order in [Order::Oldest, Order::Youngest] {
            let mut n = fixture(Some(true), false, true);
            assert_eq!(n.run(order, 128), Halt::Quiescent);
            finished(&n, 12, 15);
            assert_eq!(n.count("mul-done"), 1);
            assert_eq!(n.count("add-done"), 1);
            assert_eq!(n.count("copy-value"), 1);
            assert_eq!(n.count("release"), 2);
        }
    }

    #[test]
    fn true_spin_stops_at_exact_cap_and_does_not_duplicate_producer() {
        for order in [Order::Oldest, Order::Youngest] {
            let mut n = fixture(Some(true), true, true);
            assert_eq!(n.run(order, 128), Halt::Budget);
            assert_eq!(n.rules.len(), 128);
            assert!(n.answers().is_empty());
            assert!(n.count("spin") > 0);
            assert_eq!(
                n.agents
                    .iter()
                    .flatten()
                    .filter(|a| a.kind == Kind::Spin)
                    .count(),
                1
            );
            assert_eq!(
                n.agents
                    .iter()
                    .flatten()
                    .filter(|a| a.kind == Kind::Tick)
                    .count(),
                1
            );
            assert_eq!(n.count("mul-done"), 0);
            assert_eq!(n.count("add-done"), 0);
            n.audit();
        }
    }

    #[test]
    fn late_condition_continues_the_same_residual() {
        for divergent in [false, true] {
            for condition in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = fixture(None, divergent, true);
                    assert_eq!(n.run(order, 128), Halt::Quiescent);
                    assert_eq!(n.rules, ["invoke"]);
                    supply_condition(&mut n, condition);
                    let halt = n.run(order, 127);
                    if divergent && condition {
                        assert_eq!(halt, Halt::Budget);
                        assert_eq!(n.rules.len(), 128);
                    } else {
                        assert_eq!(halt, Halt::Quiescent);
                        finished(
                            &n,
                            if condition { 12 } else { 0 },
                            if !condition && !divergent { 16 } else { 15 },
                        );
                    }
                    assert_eq!(n.count("invoke"), 1);
                    if !condition {
                        no_work(&n);
                    }
                }
            }
        }
    }

    #[test]
    fn every_fuel_cut_resumes_including_mid_erasure() {
        for divergent in [false, true] {
            for condition in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    for cut in 0..=17 {
                        let mut n = fixture(Some(condition), divergent, true);
                        let first = n.run(order, cut);
                        assert!(matches!(first, Halt::Budget | Halt::Quiescent));
                        n.audit();
                        let halt = n.run(order, 128 - cut);
                        if divergent && condition {
                            assert_eq!(halt, Halt::Budget);
                            assert_eq!(n.rules.len(), 128);
                        } else {
                            assert_eq!(halt, Halt::Quiescent);
                            finished(
                                &n,
                                if condition { 12 } else { 0 },
                                if !condition && !divergent { 16 } else { 15 },
                            );
                            if !condition {
                                no_work(&n);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fanning_a_suspended_conditional_has_no_implicit_activation_rule() {
        let mut n = fixture(Some(true), false, false);
        let entry = End::Boundary(
            n.boundaries
                .iter()
                .position(|(name, _)| name == "entry")
                .unwrap(),
        );
        let root = n.unlink(entry);
        let fan = n.add(Kind::Fan);
        let other = n.boundary("second-use");
        n.link(root, End::Port(fan, 0));
        n.link(End::Port(fan, 1), entry);
        n.link(End::Port(fan, 2), other);
        let before = n.clone();
        assert_eq!(n.run(Order::Oldest, 128), Halt::StuckWithoutRule);
        assert_eq!(n, before);
    }

    #[test]
    #[should_panic(expected = "diagnostic interaction limit")]
    fn oversized_budget_is_rejected_before_execution() {
        fixture(Some(true), true, true).run(Order::Oldest, 129);
    }
}
