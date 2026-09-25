//! Isolated two-active-port Join experiment. No production evaluator calls.
//! Retired notes: git show 6f17b9b:march6/DEMAND-JOIN.md (and CONDITIONAL-JOIN.md).
//! All runs cap at 128 rewrites.

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Need,
    IfDormant,
    Select,
    Bool(bool),
    Join,
    Join1,
    Absorb,
    LGate,
    Gate,
    Release,
    Stop,
    Erase,
    Int(i64),
    Mul,
    MulK(i64),
    AddK(i64),
    Fan,
    Observe,
    Answer(i64),
    Tick,
    Spin,
}

impl Kind {
    fn ports(&self) -> usize {
        match self {
            Self::IfDormant | Self::Select => 6,
            Self::LGate => 5,
            Self::Gate => 4,
            Self::Join | Self::Mul | Self::Fan => 3,
            Self::Need
            | Self::Join1
            | Self::MulK(_)
            | Self::AddK(_)
            | Self::Observe
            | Self::Spin => 2,
            _ => 1,
        }
    }
    fn active(&self, port: usize) -> bool {
        if *self == Self::Join {
            port < 2
        } else {
            port == 0
        }
    }
}

include!("support/multiport_runner.rs");

fn rule(a: &Kind, ap: usize, b: &Kind, bp: usize) -> Option<Result<Rule, Halt>> {
    use Kind::*;
    if ap != 0 || !a.active(ap) || !b.active(bp) {
        return None;
    }
    // On either Join input the remaining ports are [other input, output].
    if *b == Join {
        return match a {
            Release => Some(Ok(Rule::new(
                "join-release",
                vec![Absorb, Release],
                vec![(N(0, 0), E(0)), (N(1, 0), E(1))],
            ))),
            Erase => Some(Ok(Rule::new(
                "join-erase",
                vec![Join1],
                vec![(N(0, 0), E(0)), (N(0, 1), E(1))],
            ))),
            _ => None,
        };
    }
    if bp != 0 {
        return None;
    }
    let r = match (a, b) {
        // Unchanged selector rules from selective_activation.rs.
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
        (Release, Join1) => Rule::new("join-last-release", vec![Release], vec![(N(0, 0), E(0))]),
        (Erase, Join1) => Rule::new("join-last-erase", vec![Erase], vec![(N(0, 0), E(0))]),
        (Release, Absorb) => Rule::new("absorb-release", vec![], vec![]),
        (Erase, Absorb) => Rule::new("absorb-erase", vec![], vec![]),
        (Release, LGate) => Rule::new(
            "request-operand",
            vec![Release, Release],
            vec![(E(1), E(2)), (N(0, 0), E(0)), (N(1, 0), E(3))],
        ),
        (Release, Gate) => Rule::new(
            "release",
            vec![Release],
            vec![(E(1), E(2)), (N(0, 0), E(0))],
        ),
        (Release, Stop) => Rule::new("stop", vec![], vec![]),
        (Int(n), Mul) => Rule::new(
            "mul-left",
            vec![MulK(*n)],
            vec![(N(0, 0), E(0)), (N(0, 1), E(1))],
        ),
        (Int(m), MulK(n)) => {
            let Some(v) = n.checked_mul(*m) else {
                return Some(Err(Halt::NumericDomainLimit));
            };
            Rule::new("mul-done", vec![Int(v)], vec![(N(0, 0), E(0))])
        }
        (Int(m), AddK(n)) => {
            let Some(v) = n.checked_add(*m) else {
                return Some(Err(Halt::NumericDomainLimit));
            };
            Rule::new("add-done", vec![Int(v)], vec![(N(0, 0), E(0))])
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
        (Erase, k) => {
            let name = match k {
                LGate => "erase-lgate",
                Gate => "erase-gate",
                Stop => "erase-stop",
                Mul => "erase-mul",
                MulK(_) => "erase-mul-k",
                AddK(_) => "erase-add-k",
                Fan => "erase-fan",
                Observe => "erase-observe",
                Int(_) => "erase-value",
                Tick => "erase-tick",
                Spin => "erase-spin",
                Erase => "erase-erase",
                _ => return None,
            };
            let count = k.ports() - 1;
            Rule::new(
                name,
                vec![Erase; count],
                (0..count).map(|i| (N(i, 0), E(i))).collect(),
            )
        }
        _ => return None,
    };
    Some(Ok(r))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Passive,
    Ask,
    Drop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Producer {
    Finite,
    Unknown,
    Spin,
}

impl Net {
    fn invoke_conditional(&mut self) {
        let ids: Vec<_> = ["entry", "request-result"]
            .iter()
            .map(|name| self.boundaries.iter().position(|(n, _)| n == name).unwrap())
            .collect();
        assert!(
            ids.iter().all(|id| !self.retired.contains(id)),
            "invocation already consumed"
        );
        let need = self.add(Kind::Need);
        for (port, id) in ids.into_iter().enumerate() {
            let target = self.unlink(End::Boundary(id));
            self.retired.insert(id);
            self.link(End::Port(need, port), target);
        }
        self.audit();
    }

    // Explicit host action at a named free interface. No node renumbering,
    // remote inspection, or repeat action at an already consumed boundary.
    fn supply(&mut self, name: &str, kind: Kind) {
        let id = self
            .boundaries
            .iter()
            .position(|(n, _)| n == name)
            .expect("boundary");
        assert!(!self.retired.contains(&id), "boundary already consumed");
        let target = self.unlink(End::Boundary(id));
        self.retired.insert(id);
        let token = self.add(kind);
        self.link(target, End::Port(token, 0));
        self.audit();
    }
    fn act(&mut self, consumer: usize, action: Action) {
        let control = ["control-A", "control-B"][consumer];
        match action {
            Action::Passive => (),
            Action::Ask => self.supply(control, Kind::Release),
            Action::Drop => self.supply(control, Kind::Erase),
        }
    }
    fn terminal(&self, name: &str) -> Option<Kind> {
        let (_, end) = self.boundaries.iter().find(|(n, _)| n == name)?;
        if let Some(End::Port(i, 0)) = end {
            Some(self.agent(*i).kind.clone())
        } else {
            None
        }
    }
    #[cfg(test)]
    fn number(&self, kind: Kind) -> usize {
        self.agents
            .iter()
            .flatten()
            .filter(|n| n.kind == kind)
            .count()
    }
    fn dropped(&self) -> Vec<String> {
        self.boundaries
            .iter()
            .filter_map(|(name, _)| {
                (self.terminal(name) == Some(Kind::Erase)).then_some(name.clone())
            })
            .collect()
    }
}

fn shared_producer(n: &mut Net, producer: Producer) -> (usize, usize) {
    shared_producer_with_input(n, producer, 2)
}

fn shared_producer_with_input(n: &mut Net, producer: Producer, input_value: i64) -> (usize, usize) {
    let join = n.add(Kind::Join);
    let gate = n.add(Kind::Gate);
    let stop = n.add(Kind::Stop);
    let fan = n.add(Kind::Fan);
    n.wire(join, 2, gate, 0);
    n.wire(gate, 1, stop, 0);
    if producer == Producer::Spin {
        let tick = n.add(Kind::Tick);
        let spin = n.add(Kind::Spin);
        n.wire(gate, 2, tick, 0);
        n.wire(gate, 3, spin, 0);
        n.wire(spin, 1, fan, 0);
    } else {
        let mul = n.add(Kind::Mul);
        let rhs = n.add(Kind::Int(3));
        n.wire(gate, 3, mul, 0);
        n.wire(mul, 1, rhs, 0);
        n.wire(mul, 2, fan, 0);
        let input = if producer == Producer::Unknown {
            n.boundary("input")
        } else {
            End::Port(n.add(Kind::Int(input_value)), 0)
        };
        n.link(End::Port(gate, 2), input);
    }
    (join, fan)
}

fn consumer(n: &mut Net, join: usize, fan: usize, i: usize, control: End, output: End) {
    let consumer = n.add(Kind::LGate);
    let stop = n.add(Kind::Stop);
    let add = n.add(Kind::AddK(i as i64 + 1));
    n.link(End::Port(consumer, 0), control);
    n.wire(consumer, 1, stop, 0);
    n.wire(consumer, 2, fan, i + 1);
    n.wire(consumer, 3, add, 0);
    n.wire(consumer, 4, join, i);
    n.link(End::Port(add, 1), output);
}

fn fixture(producer: Producer) -> Net {
    let mut n = Net::default();
    let observers = [n.observe("A"), n.observe("B")];
    let (join, fan) = shared_producer(&mut n, producer);
    for (i, observer) in observers.into_iter().enumerate() {
        let control = n.boundary(["control-A", "control-B"][i]);
        consumer(&mut n, join, fan, i, control, End::Port(observer, 0));
    }
    n.audit();
    n
}

fn conditional_fixture(condition: Option<bool>, producer: Producer, invoked: bool) -> Net {
    conditional_fixture_with_input(condition, producer, invoked, 2)
}

fn conditional_fixture_with_input(
    condition: Option<bool>,
    producer: Producer,
    invoked: bool,
    input: i64,
) -> Net {
    let mut n = Net::default();
    let obs = n.observe("result");
    let root = n.add(Kind::IfDormant);
    let (join, fan) = shared_producer_with_input(&mut n, producer, input);
    for i in 0..2 {
        consumer(
            &mut n,
            join,
            fan,
            i,
            End::Port(root, 2 + i * 2),
            End::Port(root, 3 + i * 2),
        );
    }
    let input = match condition {
        Some(b) => End::Port(n.add(Kind::Bool(b)), 0),
        None => n.boundary("condition"),
    };
    n.link(End::Port(root, 1), input);
    let entry = n.boundary("entry");
    let result = n.boundary("request-result");
    n.link(End::Port(root, 0), entry);
    n.link(End::Port(obs, 0), result);
    if invoked {
        n.invoke_conditional();
    }
    n.audit();
    n
}

fn show(name: &str, n: &mut Net, order: Order, budget: usize, trace: bool) {
    let previous = n.trace.len();
    let halt = n.run(order, budget);
    println!(
        "{}; dropped={:?}; spin={}",
        n.summary(name, order, halt),
        n.dropped(),
        n.count("spin")
    );
    if trace {
        for (i, line) in n.trace.iter().enumerate().skip(previous) {
            println!("  {}. {line}", i + 1);
        }
        for (a, ap, b, bp) in n.pairs() {
            println!(
                "  pending {name} / {order:?}: {a}.{ap}:{:?} >< {b}.{bp}:{:?}",
                n.agent(a).kind,
                n.agent(b).kind
            );
        }
    }
}

#[path = "support/demand_bench.rs"]
mod benchmark;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--bench"] {
        benchmark::run();
        return;
    }
    let trace = match args.as_slice() {
        [] => false,
        [arg] if arg == "--trace" => true,
        _ => {
            eprintln!("usage: demand_join [--trace | --bench]");
            std::process::exit(2);
        }
    };
    for order in [Order::Oldest, Order::Youngest] {
        for (name, producer, actions) in [
            (
                "neither-asks",
                Producer::Finite,
                [Action::Passive, Action::Passive],
            ),
            ("both-ask", Producer::Finite, [Action::Ask, Action::Ask]),
            (
                "ask-and-drop",
                Producer::Finite,
                [Action::Ask, Action::Drop],
            ),
            ("both-drop", Producer::Finite, [Action::Drop, Action::Drop]),
            (
                "discarded-spin",
                Producer::Spin,
                [Action::Drop, Action::Drop],
            ),
            (
                "discarded-unknown",
                Producer::Unknown,
                [Action::Drop, Action::Drop],
            ),
            ("demanded-spin", Producer::Spin, [Action::Ask, Action::Drop]),
        ] {
            let mut n = fixture(producer);
            for (i, action) in actions.into_iter().enumerate() {
                n.act(i, action);
            }
            show(name, &mut n, order, 128, trace);
        }
        let mut n = fixture(Producer::Finite);
        n.act(0, Action::Ask);
        show("A-asks-B-passive", &mut n, order, 64, trace);
        n.act(1, Action::Ask);
        show("B-asks-later", &mut n, order, 64, trace);
        let mut n = fixture(Producer::Unknown);
        n.act(0, Action::Drop);
        n.act(1, Action::Ask);
        show("discard-one-wait-input", &mut n, order, 64, trace);
        n.supply("input", Kind::Int(2));
        show("input-arrives", &mut n, order, 64, trace);
        for (name, condition, producer, invoked) in [
            ("conditional-uninvoked", Some(true), Producer::Finite, false),
            ("conditional-unknown", None, Producer::Finite, true),
            ("conditional-true", Some(true), Producer::Finite, true),
            ("conditional-false", Some(false), Producer::Finite, true),
            ("conditional-unrequested-spin", None, Producer::Spin, true),
            (
                "conditional-demanded-spin",
                Some(false),
                Producer::Spin,
                true,
            ),
        ] {
            let mut n = conditional_fixture(condition, producer, invoked);
            show(name, &mut n, order, 128, trace);
        }
        let mut n = conditional_fixture(None, Producer::Unknown, true);
        show("conditional-wait-condition", &mut n, order, 32, trace);
        n.supply("condition", Kind::Bool(true));
        show("conditional-wait-input", &mut n, order, 32, trace);
        n.supply("input", Kind::Int(2));
        show("conditional-input-arrives", &mut n, order, 64, trace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ACTIONS: [Action; 3] = [Action::Passive, Action::Ask, Action::Drop];

    fn check_finite(n: &Net, actions: [Action; 2]) {
        for (i, action) in actions.into_iter().enumerate() {
            let terminal = n.terminal(["A", "B"][i]);
            match action {
                Action::Ask => assert_eq!(terminal, Some(Kind::Answer(i as i64 + 7))),
                Action::Drop => assert_eq!(terminal, Some(Kind::Erase)),
                Action::Passive => assert_eq!(terminal, None),
            }
        }
        let asks = actions.iter().filter(|&&a| a == Action::Ask).count();
        let drops = actions.iter().filter(|&&a| a == Action::Drop).count();
        assert_eq!(n.count("mul-done"), usize::from(asks > 0));
        assert_eq!(n.count("copy-value"), usize::from(asks > 0));
        assert_eq!(n.count("add-done"), asks);
        let steps = match (asks, drops) {
            (0, 0) => 0,
            (0, 1) => 5,
            (0, 2) => 18,
            (1, 0) => 10,
            (1, 1) => 16,
            (2, 0) => 15,
            _ => unreachable!(),
        };
        assert_eq!(n.trace.len(), steps);
        if asks + drops == 2 {
            assert_eq!(n.live(), 2);
        }
        assert!(n.pairs().is_empty());
        n.audit();
    }

    #[test]
    fn all_finite_action_pairs_both_orders_and_schedules() {
        for a in ACTIONS {
            for b in ACTIONS {
                for order in [Order::Oldest, Order::Youngest] {
                    for first in [0, 1] {
                        let mut n = fixture(Producer::Finite);
                        let actions = [a, b];
                        n.act(first, actions[first]);
                        n.act(1 - first, actions[1 - first]);
                        assert_eq!(n.run(order, 128), Halt::Quiescent);
                        check_finite(&n, actions);
                    }
                }
            }
        }
    }

    #[test]
    fn delayed_second_consumer_reuses_value_or_preserves_first_drop() {
        for a in ACTIONS {
            for b in ACTIONS {
                for order in [Order::Oldest, Order::Youngest] {
                    for first in [0, 1] {
                        let actions = [a, b];
                        let mut n = fixture(Producer::Finite);
                        n.act(first, actions[first]);
                        assert_eq!(n.run(order, 64), Halt::Quiescent);
                        if actions[first] == Action::Ask {
                            assert_eq!(n.count("mul-done"), 1);
                            assert_eq!(n.count("add-done"), 1);
                            assert_eq!(n.number(Kind::Int(6)), 1); // parked for passive consumer
                        }
                        n.act(1 - first, actions[1 - first]);
                        assert_eq!(n.run(order, 64), Halt::Quiescent);
                        check_finite(&n, actions);
                    }
                }
            }
        }
    }

    #[test]
    fn dropping_one_use_does_not_erase_unknown_producer() {
        for dropped in [0, 1] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = fixture(Producer::Unknown);
                n.act(dropped, Action::Drop);
                assert_eq!(n.run(order, 32), Halt::Quiescent);
                assert_eq!(n.number(Kind::Mul), 1);
                assert_eq!(n.number(Kind::Join1), 1);
                assert_eq!(n.count("erase-gate"), 0);
                let fan = n
                    .agents
                    .iter()
                    .position(|a| a.as_ref().is_some_and(|a| a.kind == Kind::Fan))
                    .unwrap();
                let Some(End::Port(eraser, 0)) = n.slot(End::Port(fan, dropped + 1)) else {
                    panic!("no discarded-use eraser");
                };
                assert_eq!(n.agent(eraser).kind, Kind::Erase);
                n.act(1 - dropped, Action::Ask);
                assert_eq!(n.run(order, 32), Halt::Quiescent);
                assert_eq!(n.count("mul-left"), 0);
                n.supply("input", Kind::Int(2));
                assert_eq!(n.run(order, 64), Halt::Quiescent);
                let mut actions = [Action::Ask; 2];
                actions[dropped] = Action::Drop;
                check_finite(&n, actions);
            }
        }
    }

    #[test]
    fn both_drop_unknown_or_spin_without_running_it() {
        for producer in [Producer::Unknown, Producer::Spin] {
            for first in [0, 1] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = fixture(producer);
                    n.act(first, Action::Drop);
                    assert_eq!(n.run(order, 32), Halt::Quiescent);
                    assert_eq!(n.count("spin"), 0);
                    n.act(1 - first, Action::Drop);
                    assert_eq!(n.run(order, 64), Halt::Quiescent);
                    assert_eq!(n.trace.len(), 17);
                    assert_eq!(n.count("spin"), 0);
                    assert_eq!(n.count("mul-left"), 0);
                    assert_eq!(n.terminal("A"), Some(Kind::Erase));
                    assert_eq!(n.terminal("B"), Some(Kind::Erase));
                    assert_eq!(n.live(), if producer == Producer::Unknown { 3 } else { 2 });
                    if producer == Producer::Unknown {
                        assert_eq!(n.terminal("input"), Some(Kind::Erase));
                    }
                }
            }
        }
    }

    #[test]
    fn spin_runs_only_if_asked_and_has_one_producer() {
        for a in ACTIONS {
            for b in ACTIONS {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = fixture(Producer::Spin);
                    n.act(0, a);
                    n.act(1, b);
                    let asked = a == Action::Ask || b == Action::Ask;
                    assert_eq!(
                        n.run(order, 128),
                        if asked { Halt::Budget } else { Halt::Quiescent }
                    );
                    if asked {
                        assert_eq!(n.trace.len(), 128);
                        assert!(n.count("spin") > 0);
                        assert_eq!(n.number(Kind::Spin), 1);
                        assert_eq!(n.number(Kind::Tick), 1);
                    } else {
                        assert_eq!(n.count("spin"), 0);
                    }
                    assert!(n.answers().is_empty());
                }
            }
        }
    }

    #[test]
    fn all_short_fuel_cuts_resume_including_join_and_erasure() {
        for producer in [Producer::Finite, Producer::Spin] {
            for a in ACTIONS {
                for b in ACTIONS {
                    for order in [Order::Oldest, Order::Youngest] {
                        for cut in 0..=19 {
                            let mut n = fixture(producer);
                            n.act(0, a);
                            n.act(1, b);
                            assert!(matches!(n.run(order, cut), Halt::Budget | Halt::Quiescent));
                            n.audit();
                            let halt = n.run(order, 128 - cut);
                            if producer == Producer::Finite {
                                assert_eq!(halt, Halt::Quiescent);
                                check_finite(&n, [a, b]);
                            } else if a == Action::Ask || b == Action::Ask {
                                assert_eq!(halt, Halt::Budget);
                                assert_eq!(n.trace.len(), 128);
                            } else {
                                assert_eq!(halt, Halt::Quiescent);
                                assert_eq!(n.count("spin"), 0);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn join_arrival_critical_pairs_have_the_same_upstream_signal() {
        for left in [Kind::Release, Kind::Erase] {
            for right in [Kind::Release, Kind::Erase] {
                for first in [0, 1] {
                    let mut n = Net::default();
                    let join = n.add(Kind::Join);
                    let out = n.boundary("out");
                    n.link(End::Port(join, 2), out);
                    let tokens = [n.add(left.clone()), n.add(right.clone())];
                    for (p, id) in tokens.into_iter().enumerate() {
                        n.wire(join, p, id, 0);
                    }
                    n.audit();
                    let (a, ap, b, bp, r) = n.candidate(tokens[first], 0, join, first).unwrap();
                    n.rewrite(a, ap, b, bp, r.unwrap()).unwrap();
                    assert_eq!(n.run(Order::Oldest, 2), Halt::Quiescent);
                    let expected = if left == Kind::Release || right == Kind::Release {
                        Kind::Release
                    } else {
                        Kind::Erase
                    };
                    assert_eq!(n.terminal("out"), Some(expected));
                    assert_eq!(n.trace.len(), 2);
                    assert_eq!(n.live(), 1);
                }
            }
        }
    }

    #[test]
    fn join_output_is_passive_and_unsupported_arrival_is_not_quiescent() {
        let mut n = Net::default();
        let join = n.add(Kind::Join);
        let r = n.add(Kind::Release);
        n.wire(join, 2, r, 0);
        for p in 0..2 {
            let boundary = n.boundary(&format!("input-{p}"));
            n.link(End::Port(join, p), boundary);
        }
        assert_eq!(n.run(Order::Oldest, 8), Halt::Quiescent);
        assert!(n.trace.is_empty());
        n.supply("input-1", Kind::Int(9));
        let before = n.clone();
        assert_eq!(n.run(Order::Oldest, 8), Halt::StuckWithoutRule);
        assert_eq!(n, before);
    }

    #[test]
    fn numeric_limit_is_atomic() {
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
    fn unsupported_internal_wire_is_rejected_atomically() {
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
    fn rule_cannot_drop_an_interface_end() {
        Rule::new("invalid", vec![], vec![]).validate(1);
    }

    #[test]
    #[should_panic]
    fn rule_cannot_duplicate_an_interface_end() {
        Rule::new(
            "invalid",
            vec![Kind::Int(1), Kind::Int(1)],
            vec![(N(0, 0), E(0)), (N(1, 0), E(0))],
        )
        .validate(1);
    }

    #[test]
    #[should_panic(expected = "boundary already consumed")]
    fn a_consumer_cannot_send_a_second_control_token() {
        let mut n = fixture(Producer::Finite);
        n.act(0, Action::Ask);
        n.act(0, Action::Drop);
    }

    #[test]
    #[should_panic(expected = "diagnostic interaction limit")]
    fn oversized_budget_is_rejected() {
        fixture(Producer::Spin).run(Order::Oldest, 129);
    }
}

#[cfg(test)]
mod conditional_tests {
    use super::*;

    fn check_result(n: &Net, condition: bool) {
        assert_eq!(
            n.answers(),
            vec![("result".into(), if condition { 7 } else { 8 })]
        );
        assert_eq!(n.trace.len(), 18);
        assert_eq!(n.live(), 1);
        assert!(n.dropped().is_empty());
        assert!(n.pairs().is_empty());
        for name in [
            "invoke",
            "request-operand",
            "mul-left",
            "mul-done",
            "copy-value",
            "add-done",
            "erase-lgate",
            "erase-add-k",
            "erase-erase",
            "erase-value",
        ] {
            assert_eq!(n.count(name), 1, "{name}");
        }
        assert_eq!(n.count("choose-true"), usize::from(condition));
        assert_eq!(n.count("choose-false"), usize::from(!condition));
        assert_eq!(n.count("join-release") + n.count("join-last-release"), 1);
        assert_eq!(n.count("erase-gate"), 0);
        assert_eq!(n.count("erase-mul"), 0);
        n.audit();
    }

    // Choose an actual enabled local rule to exercise arrival order explicitly.
    // This is a test scheduler, never a semantic rewrite or remote erasure.
    fn step(n: &mut Net, name: &str) {
        let (a, ap, b, bp, r) = n
            .pairs()
            .into_iter()
            .filter_map(|(a, ap, b, bp)| n.candidate(a, ap, b, bp))
            .find(|(_, _, _, _, r)| r.as_ref().is_ok_and(|r| r.name == name))
            .unwrap_or_else(|| panic!("rule {name} not enabled"));
        n.rewrite(a, ap, b, bp, r.unwrap()).unwrap();
    }

    #[test]
    fn either_branch_computes_shared_producer_once() {
        for condition in [false, true] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = conditional_fixture(Some(condition), Producer::Finite, true);
                assert_eq!(n.run(order, 128), Halt::Quiescent);
                check_result(&n, condition);
            }
        }
    }

    #[test]
    fn known_condition_cannot_run_without_invocation() {
        for condition in [None, Some(false), Some(true)] {
            for producer in [Producer::Finite, Producer::Unknown, Producer::Spin] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = conditional_fixture(condition, producer, false);
                    let before = n.clone();
                    assert_eq!(n.run(order, 128), Halt::Quiescent);
                    assert_eq!(n, before);
                    assert!(n.trace.is_empty());
                }
            }
        }
    }

    #[test]
    fn invocation_waits_for_condition_without_requesting_either_branch() {
        for producer in [Producer::Finite, Producer::Unknown, Producer::Spin] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = conditional_fixture(None, producer, true);
                assert_eq!(n.run(order, 128), Halt::Quiescent);
                assert_eq!(n.rules, vec!["invoke"]);
                assert_eq!(n.number(Kind::Select), 1);
                assert_eq!(n.number(Kind::Join), 1);
                assert_eq!(n.number(Kind::LGate), 2);
                assert!(n.answers().is_empty());
            }
        }
    }

    #[test]
    fn late_invocation_and_late_condition_resume_same_graph() {
        for condition in [false, true] {
            for condition_first in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = conditional_fixture(None, Producer::Finite, false);
                    assert_eq!(n.run(order, 32), Halt::Quiescent);
                    if condition_first {
                        n.supply("condition", Kind::Bool(condition));
                    } else {
                        n.invoke_conditional();
                    }
                    assert_eq!(n.run(order, 32), Halt::Quiescent);
                    assert_eq!(n.trace.len(), usize::from(!condition_first));
                    if condition_first {
                        n.invoke_conditional();
                    } else {
                        n.supply("condition", Kind::Bool(condition));
                    }
                    assert_eq!(n.run(order, 64), Halt::Quiescent);
                    check_result(&n, condition);
                }
            }
        }
    }

    #[test]
    fn condition_and_producer_input_can_arrive_in_either_order() {
        for condition in [false, true] {
            for condition_first in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = conditional_fixture(None, Producer::Unknown, true);
                    assert_eq!(n.run(order, 32), Halt::Quiescent);
                    if condition_first {
                        n.supply("condition", Kind::Bool(condition));
                    } else {
                        n.supply("input", Kind::Int(2));
                    }
                    assert_eq!(n.run(order, 32), Halt::Quiescent);
                    assert_eq!(n.count("mul-left"), 0);
                    assert_eq!(n.trace.len(), if condition_first { 12 } else { 1 });
                    if condition_first {
                        assert_eq!(n.live(), 6);
                        assert_eq!(n.number(Kind::Mul), 1);
                        assert_eq!(n.count("erase-gate"), 0);
                        n.supply("input", Kind::Int(2));
                    } else {
                        n.supply("condition", Kind::Bool(condition));
                    }
                    assert_eq!(n.run(order, 64), Halt::Quiescent);
                    check_result(&n, condition);
                }
            }
        }
    }

    #[test]
    fn explicit_discard_first_and_request_first_preserve_selected_use() {
        for condition in [false, true] {
            for discard_first in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    let mut n = conditional_fixture(Some(condition), Producer::Finite, true);
                    step(&mut n, "invoke");
                    step(
                        &mut n,
                        if condition {
                            "choose-true"
                        } else {
                            "choose-false"
                        },
                    );
                    if discard_first {
                        step(&mut n, "erase-lgate");
                        step(&mut n, "join-erase");
                        assert_eq!(n.number(Kind::Join1), 1);
                        assert_eq!(n.number(Kind::Gate), 1);
                        assert_eq!(n.number(Kind::Mul), 1);
                        assert_eq!(n.count("release"), 0);
                        step(&mut n, "request-operand");
                        step(&mut n, "join-last-release");
                    } else {
                        step(&mut n, "request-operand");
                        step(&mut n, "join-release");
                        assert_eq!(n.number(Kind::Absorb), 1);
                        step(&mut n, "erase-lgate");
                        step(&mut n, "absorb-erase");
                    }
                    assert_eq!(n.run(order, 64), Halt::Quiescent);
                    check_result(&n, condition);
                }
            }
        }
    }

    #[test]
    fn demanded_common_spin_is_not_mistaken_for_unused_branch_work() {
        for condition in [false, true] {
            for order in [Order::Oldest, Order::Youngest] {
                let mut n = conditional_fixture(Some(condition), Producer::Spin, true);
                assert_eq!(n.run(order, 128), Halt::Budget);
                assert_eq!(n.trace.len(), 128);
                assert_eq!(n.number(Kind::Tick), 1);
                assert_eq!(n.number(Kind::Spin), 1);
                assert!(n.count("spin") > 0);
                assert_eq!(n.count("join-release") + n.count("join-last-release"), 1);
                assert!(n.answers().is_empty());
            }
        }
    }

    #[test]
    fn fuel_cuts_resume_across_selection_join_and_cleanup() {
        for producer in [Producer::Finite, Producer::Unknown, Producer::Spin] {
            for condition in [false, true] {
                for order in [Order::Oldest, Order::Youngest] {
                    for cut in 0..=19 {
                        let mut n = conditional_fixture(Some(condition), producer, true);
                        assert!(matches!(n.run(order, cut), Halt::Budget | Halt::Quiescent));
                        n.audit();
                        let halt = n.run(order, 128 - cut);
                        if producer == Producer::Spin {
                            assert_eq!(halt, Halt::Budget);
                            assert_eq!(n.trace.len(), 128);
                            assert_eq!(n.number(Kind::Spin), 1);
                        } else {
                            assert_eq!(halt, Halt::Quiescent);
                            if producer == Producer::Unknown {
                                assert_eq!(n.trace.len(), 12);
                                assert_eq!(n.live(), 6);
                                n.supply("input", Kind::Int(2));
                                assert_eq!(n.run(order, 32), Halt::Quiescent);
                            }
                            check_result(&n, condition);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_invocation_is_rejected_before_mutation() {
        let mut n = conditional_fixture(Some(true), Producer::Finite, true);
        let before = n.clone();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| n.invoke_conditional()))
                .is_err()
        );
        assert_eq!(n, before);
    }
}
