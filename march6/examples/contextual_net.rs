//! Standalone, bounded port-rule experiment. No March evaluator or CAS calls.
//! Retired notes: git show 6f17b9b:march6/CONTEXTUAL-NET-EXPERIMENT.md.
//! Run with --trace for every rewiring.
//! Append-only diagnostic allocation: dead slots are retained and reported.

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
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
    // Diagnostic recursion rules, not an encoding of March recursive words.
    Tick,
    Spin,
    From,
    Cons,
    Head,
}

impl Kind {
    fn ports(&self) -> usize {
        match self {
            Self::Gate => 4,
            Self::Add | Self::Mul | Self::Fan | Self::Cons => 3,
            Self::AddK(_)
            | Self::MulK(_)
            | Self::Observe
            | Self::Spin
            | Self::From
            | Self::Head => 2,
            _ => 1,
        }
    }
}

include!("support/port_runner.rs");

// External ends are the first agent's auxiliaries, then the second's.
// Rules inspect ONLY these two kind labels. Checked i64 is a probe limit,
// not a language-level March exception or an overflow rule.
fn rule(a: &Kind, b: &Kind) -> Option<Result<Rule, Halt>> {
    use Kind::*;
    let r = match (a, b) {
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
        (Int(_), Erase) => Rule::new("erase-value", vec![], vec![]),
        (Int(n), Observe) => Rule::new("observe", vec![Answer(*n)], vec![(N(0, 0), E(0))]),
        (Tick, Spin) => Rule::new(
            "spin",
            vec![Tick, Spin],
            vec![(N(0, 0), N(1, 0)), (N(1, 1), E(0))],
        ),
        (Int(n), From) => {
            let Some(next) = n.checked_add(1) else {
                return Some(Err(Halt::NumericDomainLimit));
            };
            Rule::new(
                "from",
                vec![Cons, Int(*n), From, Int(next)],
                vec![
                    (N(0, 0), E(0)),
                    (N(0, 1), N(1, 0)),
                    (N(0, 2), N(2, 1)),
                    (N(2, 0), N(3, 0)),
                ],
            )
        }
        (Head, Cons) => Rule::new("head", vec![Erase], vec![(E(0), E(1)), (N(0, 0), E(2))]),
        // Optional erasure rule from T4: even full field erasure does not
        // cancel an input-facing From producer through its output.
        (Erase, Cons) => Rule::new(
            "erase-cons",
            vec![Erase, Erase],
            vec![(N(0, 0), E(0)), (N(1, 0), E(1))],
        ),
        _ => return None,
    };
    Some(Ok(r))
}

fn sum(shared: bool, released: bool) -> Net {
    let mut net = Net::default();
    let a = net.add(Kind::Int(2));
    let b = net.add(Kind::Int(1));
    let add = net.add(Kind::Add);
    net.wire(b, 0, add, 1);
    net.gate(End::Port(a, 0), End::Port(add, 0), released);
    if shared {
        let fan = net.add(Kind::Fan);
        let mul = net.add(Kind::Mul);
        let obs = net.observe("result");
        net.wire(add, 2, fan, 0);
        net.wire(fan, 1, mul, 0);
        net.wire(fan, 2, mul, 1);
        net.wire(mul, 2, obs, 0);
    } else {
        let erase = net.add(Kind::Erase);
        net.wire(add, 2, erase, 0);
    }
    net
}

fn spin() -> Net {
    let mut net = Net::default();
    // Allocate the independent observation first so the youngest schedule
    // can deliberately starve it with freshly allocated loop pairs.
    let obs = net.observe("independent");
    let value = net.add(Kind::Int(7));
    net.wire(value, 0, obs, 0);
    let tick = net.add(Kind::Tick);
    let spin = net.add(Kind::Spin);
    let erase = net.add(Kind::Erase);
    net.wire(spin, 1, erase, 0);
    net.gate(End::Port(tick, 0), End::Port(spin, 0), true);
    net
}

fn stream() -> Net {
    let mut net = Net::default();
    let obs = net.observe("head");
    let head = net.add(Kind::Head);
    let from = net.add(Kind::From);
    let zero = net.add(Kind::Int(0));
    net.wire(obs, 0, head, 1);
    net.wire(head, 0, from, 1);
    net.wire(from, 0, zero, 0);
    net
}

fn quad() -> Net {
    let mut net = Net::default();
    let x = net.boundary("x");
    let out = net.boundary("out");
    let f1 = net.add(Kind::Fan);
    let m1 = net.add(Kind::Mul);
    let f2 = net.add(Kind::Fan);
    let m2 = net.add(Kind::Mul);
    net.link(x, End::Port(f1, 0));
    net.wire(f1, 1, m1, 0);
    net.wire(f1, 2, m1, 1);
    net.wire(m1, 2, f2, 0);
    net.wire(f2, 1, m2, 0);
    net.wire(f2, 2, m2, 1);
    net.link(out, End::Port(m2, 2));
    net
}

// Host supplies information at the declared open interface. This is NOT an
// interaction rule, parser, or automatic inference of missing stack inputs.
fn supply_quad(net: &mut Net, n: i64) {
    let input = net.unlink(End::Boundary(0));
    let output = net.unlink(End::Boundary(1));
    net.boundaries.clear();
    let value = net.add(Kind::Int(n));
    let obs = net.observe("result");
    net.link(input, End::Port(value, 0));
    net.link(output, End::Port(obs, 0));
    net.audit();
}

fn show(name: &str, mut net: Net, order: Order, trace: bool) {
    if name == "quad" {
        let halt = net.run(order, 40);
        println!("{}", net.summary("quad-open", order, halt));
        supply_quad(&mut net, 3);
    }
    let halt = net.run(order, 40);
    println!("{}", net.summary(name, order, halt));
    if trace {
        for (i, line) in net.trace.iter().enumerate() {
            println!("  {}. {line}", i + 1);
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let trace = match args.as_slice() {
        [] => false,
        [arg] if arg == "--trace" => true,
        _ => {
            eprintln!("usage: contextual_net [--trace]");
            std::process::exit(2);
        }
    };
    for order in [Order::Oldest, Order::Youngest] {
        show("discarded-sum", sum(false, true), order, trace);
        show("unreleased-sum", sum(false, false), order, trace);
        show("shared-sum", sum(true, true), order, trace);
        show("discarded-loop", spin(), order, trace);
        show("ungated-stream", stream(), order, trace);
        show("quad", quad(), order, trace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discarded_work_still_computes() {
        for order in [Order::Oldest, Order::Youngest] {
            let mut n = sum(false, true);
            assert_eq!(n.run(order, 5), Halt::Quiescent);
            assert_eq!(n.trace.len(), 5);
            assert_eq!(n.count("add-done"), 1);
            assert_eq!(n.live(), 0);
        }
    }

    #[test]
    fn unreleased_work_is_quiescent_but_retained() {
        let mut n = sum(false, false);
        assert_eq!(n.run(Order::Oldest, 40), Halt::Quiescent);
        assert!(n.trace.is_empty());
        assert_eq!(n.live(), 6);
    }

    #[test]
    fn a_gate_does_not_freeze_other_pairs_in_its_connected_subnet() {
        let mut n = Net::default();
        let input = n.add(Kind::Int(8));
        let upstream = n.add(Kind::AddK(2));
        let downstream = n.add(Kind::Mul);
        let rhs = n.add(Kind::Int(2));
        let erase = n.add(Kind::Erase);
        n.wire(input, 0, upstream, 0);
        n.wire(rhs, 0, downstream, 1);
        n.wire(downstream, 2, erase, 0);
        n.gate(End::Port(upstream, 1), End::Port(downstream, 0), false);
        assert_eq!(n.run(Order::Oldest, 40), Halt::Quiescent);
        assert_eq!(n.count("add-done"), 1);
        assert_eq!(n.count("release"), 0);
        assert_eq!(n.count("mul-left"), 0);
        assert!(n.agents.iter().flatten().any(|a| a.kind == Kind::Int(10)));
    }

    #[test]
    fn one_pending_sum_shared_by_two_uses() {
        for order in [Order::Oldest, Order::Youngest] {
            let mut n = sum(true, true);
            assert_eq!(n.run(order, 8), Halt::Quiescent);
            assert_eq!(n.answers(), vec![("result".into(), 9)]);
            assert_eq!(n.trace.len(), 8);
            assert_eq!(n.count("add-done"), 1);
            assert_eq!(n.count("mul-done"), 1);
            assert_eq!(n.live(), 1); // Answer is intentionally retained.
        }
    }

    #[test]
    fn open_quad_resumes_without_copying_the_inner_square() {
        for order in [Order::Oldest, Order::Youngest] {
            let mut n = quad();
            assert_eq!(n.run(order, 40), Halt::Quiescent);
            assert_eq!(n.trace.len(), 0);
            assert_eq!(n.live(), 4);
            supply_quad(&mut n, 3);
            assert_eq!(n.run(order, 7), Halt::Quiescent);
            assert_eq!(n.answers(), vec![("result".into(), 81)]);
            assert_eq!(n.count("mul-done"), 2);
            assert_eq!(n.trace.len(), 7);
        }
    }

    #[test]
    fn loop_observation_is_not_normalization_and_can_be_starved() {
        let mut oldest = spin();
        assert_eq!(oldest.run(Order::Oldest, 40), Halt::Budget);
        assert_eq!(oldest.answers(), vec![("independent".into(), 7)]);
        assert_eq!(oldest.live(), 4); // Answer + Spin + Tick + Erase.
        assert_eq!(oldest.pairs().len(), 1);
        let slots = oldest.agents.len();
        assert_eq!(oldest.run(Order::Oldest, 40), Halt::Budget);
        assert_eq!(oldest.live(), 4);
        assert_eq!(oldest.agents.len(), slots + 80); // Dead diagnostic slots.
        let mut youngest = spin();
        assert_eq!(youngest.run(Order::Youngest, 40), Halt::Budget);
        assert!(youngest.answers().is_empty());
        assert!(youngest.count("spin") > 0);
    }

    #[test]
    fn head_observed_but_ungated_tail_continues() {
        let mut n = stream();
        assert_eq!(n.run(Order::Oldest, 3), Halt::Budget);
        assert_eq!(n.answers(), vec![("head".into(), 0)]);
        assert_eq!(n.count("from"), 1);
        assert_eq!(n.run(Order::Oldest, 37), Halt::Budget);
        assert!(n.count("from") > 1);
        let mut starving = stream();
        assert_eq!(starving.run(Order::Youngest, 40), Halt::Budget);
        assert!(starving.answers().is_empty());
        assert_eq!(starving.count("from"), 40);
        assert_eq!(starving.live(), 84);
    }

    #[test]
    fn fuel_cut_and_resume_preserve_wiring_and_sharing() {
        for order in [Order::Oldest, Order::Youngest] {
            for budget in 0..=8 {
                let mut n = sum(true, true);
                let halt = n.run(order, budget);
                assert_eq!(
                    halt,
                    if budget == 8 {
                        Halt::Quiescent
                    } else {
                        Halt::Budget
                    }
                );
                n.audit();
                assert_eq!(n.run(order, 8), Halt::Quiescent);
                assert_eq!(n.answers(), vec![("result".into(), 9)]);
                assert_eq!(n.count("add-done"), 1);
            }
        }
    }

    #[test]
    fn no_rule_is_not_quiescence() {
        let mut n = Net::default();
        let a = n.add(Kind::Release);
        let b = n.add(Kind::Erase);
        n.wire(a, 0, b, 0);
        let before = n.clone();
        assert_eq!(n.run(Order::Oldest, 3), Halt::StuckWithoutRule);
        assert_eq!(n, before);
    }

    #[test]
    fn numeric_limit_is_atomic_and_not_a_march_error() {
        let mut n = Net::default();
        let a = n.add(Kind::Int(1));
        let b = n.add(Kind::AddK(i64::MAX));
        let out = n.boundary("out");
        n.wire(a, 0, b, 0);
        n.link(End::Port(b, 1), out);
        let before = n.clone();
        assert_eq!(n.run(Order::Oldest, 3), Halt::NumericDomainLimit);
        assert_eq!(n, before);
    }

    #[test]
    fn unsupported_internal_auxiliary_wire_is_rejected_atomically() {
        let mut n = Net::default();
        let release = n.add(Kind::Release);
        let gate = n.add(Kind::Gate);
        let stop = n.add(Kind::Stop);
        n.wire(release, 0, gate, 0);
        n.wire(gate, 1, stop, 0);
        n.wire(gate, 2, gate, 3);
        let before = n.clone();
        assert_eq!(n.run(Order::Oldest, 3), Halt::UnsupportedInternalAuxWire);
        assert_eq!(n, before);
    }

    #[test]
    #[should_panic]
    fn rule_cannot_drop_a_boundary_end() {
        Rule::new("invalid", vec![], vec![]).validate(1);
    }

    #[test]
    #[should_panic]
    fn rule_cannot_duplicate_a_boundary_end() {
        Rule::new(
            "invalid",
            vec![Kind::Int(1), Kind::Int(1)],
            vec![(N(0, 0), E(0)), (N(1, 0), E(0))],
        )
        .validate(1);
    }
}
