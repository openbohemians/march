// Isolated multi-active-port diagnostic runner; retired notes:
// git show 6f17b9b:march6/DEMAND-JOIN.md.
// Driver supplies Kind::{ports,active}, the label constructors used by helpers,
// and rule(&Kind, arriving_port, &Kind, arriving_port).
const MAX_INTERACTIONS_PER_RUN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum End {
    Port(usize, usize),
    // An interface endpoint is NOT an agent with a principal port.
    Boundary(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Agent {
    kind: Kind,
    wires: Vec<Option<End>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Net {
    agents: Vec<Option<Agent>>,
    boundaries: Vec<(String, Option<End>)>,
    retired: std::collections::BTreeSet<usize>,
    trace: Vec<String>,
    rules: Vec<&'static str>,
    diagnostics: Diagnostics,
}

// Benchmark ablations only: normal examples/tests retain full diagnostics.
// Lean still uses the same matching, allocation, wiring, and local assertions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Diagnostics {
    #[default]
    Full,
    NoTrace,
    Lean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ref {
    External(usize),
    New(usize, usize),
}
use Ref::{External as E, New as N};

struct Rule {
    name: &'static str,
    agents: Vec<Kind>,
    wires: Vec<(Ref, Ref)>,
}

impl Rule {
    fn new(name: &'static str, agents: Vec<Kind>, wires: Vec<(Ref, Ref)>) -> Self {
        Self {
            name,
            agents,
            wires,
        }
    }

    // Validate the entire rule interface BEFORE any graph mutation.
    fn validate(&self, external: usize) {
        let mut seen_external = vec![0; external];
        let mut seen_new: Vec<Vec<usize>> = self
            .agents
            .iter()
            .map(|kind| vec![0; kind.ports()])
            .collect();
        for &(a, b) in &self.wires {
            for end in [a, b] {
                match end {
                    E(i) => seen_external[i] += 1,
                    N(i, p) => seen_new[i][p] += 1,
                }
            }
        }
        assert!(seen_external.iter().all(|&n| n == 1));
        assert!(seen_new.iter().flatten().all(|&n| n == 1));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Halt {
    Quiescent,
    StuckWithoutRule,
    Budget,
    NumericDomainLimit,
    UnsupportedInternalAuxWire,
}

#[derive(Clone, Copy, Debug)]
enum Order {
    Oldest,
    Youngest,
}

type Candidate = (usize, usize, usize, usize, Result<Rule, Halt>);

impl Net {
    fn add(&mut self, kind: Kind) -> usize {
        let id = self.agents.len();
        let wires = vec![None; kind.ports()];
        self.agents.push(Some(Agent { kind, wires }));
        id
    }

    fn boundary(&mut self, name: &str) -> End {
        let id = self.boundaries.len();
        self.boundaries.push((name.into(), None));
        End::Boundary(id)
    }

    fn agent(&self, id: usize) -> &Agent {
        self.agents[id].as_ref().unwrap()
    }

    fn slot(&self, end: End) -> Option<End> {
        match end {
            End::Port(i, p) => self.agent(i).wires[p],
            End::Boundary(i) => self.boundaries[i].1,
        }
    }

    fn slot_mut(&mut self, end: End) -> &mut Option<End> {
        match end {
            End::Port(i, p) => &mut self.agents[i].as_mut().unwrap().wires[p],
            End::Boundary(i) => &mut self.boundaries[i].1,
        }
    }

    fn link(&mut self, a: End, b: End) {
        assert_ne!(a, b);
        assert!(self.slot(a).is_none() && self.slot(b).is_none());
        *self.slot_mut(a) = Some(b);
        *self.slot_mut(b) = Some(a);
    }

    fn wire(&mut self, a: usize, ap: usize, b: usize, bp: usize) {
        self.link(End::Port(a, ap), End::Port(b, bp));
    }

    fn unlink(&mut self, a: End) -> End {
        let b = self.slot(a).unwrap();
        assert_eq!(self.slot(b), Some(a));
        *self.slot_mut(a) = None;
        *self.slot_mut(b) = None;
        b
    }

    fn audit(&self) {
        for (id, node) in self.agents.iter().enumerate() {
            if let Some(node) = node {
                assert_eq!(node.wires.len(), node.kind.ports());
                for (p, &other) in node.wires.iter().enumerate() {
                    assert_eq!(
                        self.slot(other.expect("unwired port")),
                        Some(End::Port(id, p))
                    );
                }
            }
        }
        for (i, (_, end)) in self.boundaries.iter().enumerate() {
            if self.retired.contains(&i) {
                assert!(end.is_none());
                continue;
            }
            assert_eq!(
                self.slot(end.expect("unwired boundary")),
                Some(End::Boundary(i))
            );
        }
    }

    fn pairs(&self) -> Vec<(usize, usize, usize, usize)> {
        let mut pairs = Vec::new();
        for (a, node) in self.agents.iter().enumerate() {
            let Some(node) = node else { continue };
            for (ap, neighbor) in node.wires.iter().enumerate() {
                if !node.kind.active(ap) {
                    continue;
                }
                if let Some(End::Port(b, bp)) = neighbor
                    && a < *b
                    && self.agent(*b).kind.active(*bp)
                {
                    pairs.push((a, ap, *b, *bp));
                }
            }
        }
        pairs
    }

    fn candidate(&self, a: usize, ap: usize, b: usize, bp: usize) -> Option<Candidate> {
        let ak = &self.agent(a).kind;
        let bk = &self.agent(b).kind;
        let forward = rule(ak, ap, bk, bp);
        let backward = rule(bk, bp, ak, ap);
        assert!(
            (ak == bk && ap == bp) || forward.is_none() || backward.is_none(),
            "ambiguous rule orientation"
        );
        if let Some(r) = forward {
            Some((a, ap, b, bp, r))
        } else {
            backward.map(|r| (b, bp, a, ap, r))
        }
    }

    fn rewrite(
        &mut self,
        a: usize,
        ap: usize,
        b: usize,
        bp: usize,
        rule: Rule,
    ) -> Result<(), Halt> {
        assert_ne!(a, b);
        assert!(self.agent(a).kind.active(ap) && self.agent(b).kind.active(bp));
        assert_eq!(self.slot(End::Port(a, ap)), Some(End::Port(b, bp)));
        let aux: Vec<End> = [(a, ap), (b, bp)]
            .into_iter()
            .flat_map(|(id, interacting)| {
                (0..self.agent(id).wires.len())
                    .filter(move |&p| p != interacting)
                    .map(move |p| End::Port(id, p))
            })
            .collect();
        let external: Vec<End> = aux.iter().map(|&p| self.slot(p).unwrap()).collect();
        // Fixtures have distinct external boundary ends. Internal auxiliary
        // wires within the same redex require an additional splice algorithm;
        // reject atomically, never silently erase them or pretend quiescence.
        if external
            .iter()
            .any(|p| matches!(p, End::Port(i, _) if *i == a || *i == b))
        {
            return Err(Halt::UnsupportedInternalAuxWire);
        }
        if self.diagnostics != Diagnostics::Lean {
            rule.validate(external.len());
        }
        let before = if self.diagnostics == Diagnostics::Full {
            Some(format!(
                "{a}.{ap}:{:?} >< {b}.{bp}:{:?}",
                self.agent(a).kind,
                self.agent(b).kind
            ))
        } else {
            None
        };
        self.unlink(End::Port(a, ap));
        for p in aux {
            self.unlink(p);
        }
        self.agents[a] = None;
        self.agents[b] = None;
        let ids: Vec<usize> = rule.agents.into_iter().map(|k| self.add(k)).collect();
        let resolve = |r| match r {
            E(i) => external[i],
            N(i, p) => End::Port(ids[i], p),
        };
        let connections: Vec<_> = rule
            .wires
            .into_iter()
            .map(|(a, b)| (resolve(a), resolve(b)))
            .collect();
        for &(a, b) in &connections {
            self.link(a, b);
        }
        if let Some(before) = before {
            self.trace.push(format!(
                "{}: {before} => new {ids:?}; wires {connections:?}",
                rule.name
            ));
        }
        self.rules.push(rule.name);
        if self.diagnostics != Diagnostics::Lean {
            self.audit();
        }
        Ok(())
    }

    fn run(&mut self, order: Order, budget: usize) -> Halt {
        assert!(
            budget <= MAX_INTERACTIONS_PER_RUN,
            "diagnostic interaction limit"
        );
        if self.diagnostics != Diagnostics::Lean {
            self.audit();
        }
        for done in 0..=budget {
            let pairs = self.pairs();
            if pairs.is_empty() {
                return Halt::Quiescent;
            }
            let mut choices = pairs
                .into_iter()
                .filter_map(|(a, ap, b, bp)| self.candidate(a, ap, b, bp));
            let choice = match order {
                Order::Oldest => choices.next(),
                Order::Youngest => choices.next_back(),
            };
            let Some((a, ap, b, bp, r)) = choice else {
                return Halt::StuckWithoutRule;
            };
            if done == budget {
                return Halt::Budget;
            }
            let r = match r {
                Ok(r) => r,
                Err(h) => return h,
            };
            if let Err(h) = self.rewrite(a, ap, b, bp, r) {
                return h;
            }
        }
        unreachable!()
    }

    fn answers(&self) -> Vec<(String, i64)> {
        self.boundaries
            .iter()
            .filter_map(|(name, end)| {
                if let Some(End::Port(i, 0)) = end
                    && let Kind::Answer(n) = self.agent(*i).kind
                {
                    return Some((name.clone(), n));
                }
                None
            })
            .collect()
    }

    fn live(&self) -> usize {
        self.agents.iter().flatten().count()
    }
    fn count(&self, name: &str) -> usize {
        self.rules.iter().filter(|&&r| r == name).count()
    }

    fn observe(&mut self, name: &str) -> usize {
        let obs = self.add(Kind::Observe);
        let result = self.boundary(name);
        self.link(End::Port(obs, 1), result);
        obs
    }

    fn summary(&self, name: &str, order: Order, halt: Halt) -> String {
        format!(
            "{name} / {order:?}: {halt:?}; steps={}; answers={:?}; active_pairs={}; live={}; slots={}; add={}; mul={}",
            self.trace.len(),
            self.answers(),
            self.pairs().len(),
            self.live(),
            self.agents.len(),
            self.count("add-done"),
            self.count("mul-done")
        )
    }
}
