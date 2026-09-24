//! Variant A: an experimental, sequential demand-protocol machine.
//!
//! This is **not** the principal-pair interaction-net backend. A CID-addressed
//! memo has directly addressable input/use-site slots; an explicit continuation
//! carries the one demand token. Pending slots survive an unknown result and
//! known values are retained for the next epoch. Errors are epoch-local because
//! newly bound holes can change which error takes precedence. No source term is evaluated
//! outside a request, and no recursive Rust evaluator or active-net scan is used.
//!
//! Accounting excludes immutable `Program` templates. `live` counts memo cells,
//! retained input slots, continuation cells, and the in-flight token. `erasures`
//! counts input slots actually released (including unrequested branch slots).
//! Cached memo cells remain live for the machine's lifetime: this first probe
//! does NOT implement last-consumer memo reclamation or establish the N1 gate.
//! In particular, requests use direct CID lookup: the input slots represent
//! outgoing use sites, not concrete incoming request ports on a multi-principal
//! net agent. This tests the candidate's sequential memo semantics, not port
//! growth, request races, token rewiring, locality, or schedule independence.

use crate::cid::Cid;
use crate::demand::{Expr, Fault, Outcome, ProbeError, ProbeRun, ProbeStats, Program, Scalar};
use std::collections::BTreeMap;

#[derive(Clone)]
enum Slot {
    Pending(Cid),
    Ready {
        source: Cid,
        outcome: Outcome,
        epoch: usize,
    },
    Erased,
}

struct Memo {
    result: Option<Outcome>,
    epoch: usize,
    evaluating: bool,
    slots: Vec<Slot>,
}

#[derive(Clone, Copy)]
enum Phase {
    Left { multiply: bool },
    Right { multiply: bool },
    Condition,
    Branch { index: usize },
}

struct Frame {
    parent: Cid,
    phase: Phase,
}

enum Token {
    Request(Cid),
    Reply(Outcome),
}

/// Persistent scalar memo protocol with monotonic bindings and epoch resumption.
pub struct Machine {
    program: Program,
    bindings: BTreeMap<String, Scalar>,
    memos: BTreeMap<Cid, Memo>,
    epoch: usize,
    stats: ProbeStats,
}

impl Machine {
    pub fn new(program: Program) -> Self {
        Self {
            program,
            bindings: BTreeMap::new(),
            memos: BTreeMap::new(),
            epoch: 0,
            stats: ProbeStats::default(),
        }
    }

    /// Run one observation epoch; `budget` limits transitions in this call.
    /// Conflicting bindings are rejected before any machine state is changed.
    /// Budget exhaustion cancels continuations; a subsequent run starts a new
    /// observation, retaining completed values, rather than resuming the token.
    pub fn run(
        &mut self,
        bindings: &BTreeMap<String, Scalar>,
        budget: usize,
    ) -> Result<ProbeRun, ProbeError> {
        for (name, value) in bindings {
            if self.bindings.get(name).is_some_and(|old| old != value) {
                return Err(ProbeError::ConflictingBinding(name.clone()));
            }
        }
        self.bindings.extend(bindings.clone());
        self.epoch += 1;
        let mut frames = Vec::new();
        let mut token = Token::Request(self.program.root);
        self.allocate(1);
        for _ in 0..budget {
            self.stats.transitions += 1;
            let next = match token {
                Token::Request(cid) => self.request(cid, &mut frames),
                Token::Reply(outcome) => {
                    if let Some(frame) = frames.pop() {
                        self.stats.live -= 1;
                        self.deliver(frame, outcome, &mut frames)
                    } else {
                        self.stats.live -= 1;
                        return Ok(ProbeRun {
                            outcome,
                            stats: self.stats.clone(),
                        });
                    }
                }
            };
            match next {
                Ok(next) => token = next,
                Err(error) => {
                    self.cancel(&frames);
                    return Err(error);
                }
            }
        }
        self.cancel(&frames);
        Err(ProbeError::BudgetExhausted { limit: budget })
    }

    fn allocate(&mut self, count: usize) {
        self.stats.live += count;
        self.stats.peak_live = self.stats.peak_live.max(self.stats.live);
    }

    fn cancel(&mut self, frames: &[Frame]) {
        // Each unfinished evaluating memo has exactly one continuation. Walk
        // that explicit stack, not the graph or a global list of active cells.
        for frame in frames {
            if let Some(memo) = self.memos.get_mut(&frame.parent) {
                memo.evaluating = false;
            }
        }
        self.stats.live -= frames.len() + 1;
    }

    fn request(&mut self, cid: Cid, frames: &mut Vec<Frame>) -> Result<Token, ProbeError> {
        self.stats.requests += 1;
        self.stats.routing_steps += 1; // direct request-port lookup, no fan path
        if let Some(memo) = self.memos.get(&cid) {
            if memo.evaluating {
                return Err(ProbeError::Invariant("cyclic memo request"));
            }
            if let Some(result) = &memo.result
                && (matches!(result, Outcome::Value(_)) || memo.epoch == self.epoch)
            {
                self.stats.memo_hits += 1;
                return Ok(Token::Reply(result.clone()));
            }
        }
        let expr = self
            .program
            .nodes
            .get(&cid)
            .cloned()
            .ok_or(ProbeError::MissingNode(cid))?;
        if !self.memos.contains_key(&cid) {
            let slots: Vec<_> = expr.children().into_iter().map(Slot::Pending).collect();
            self.allocate(1 + slots.len());
            self.memos.insert(
                cid,
                Memo {
                    result: None,
                    epoch: self.epoch,
                    evaluating: false,
                    slots,
                },
            );
        }
        let memo = self.memos.get_mut(&cid).expect("memo created above");
        memo.evaluating = true;
        memo.epoch = self.epoch;
        self.stats.evaluations += 1;
        *self.stats.node_evaluations.entry(cid).or_default() += 1;
        match expr {
            Expr::Value(value) => Ok(self.complete(cid, Outcome::Value(value))),
            Expr::Hole(name) => {
                let result = self
                    .bindings
                    .get(&name)
                    .cloned()
                    .map_or(Outcome::Unknown, Outcome::Value);
                Ok(self.complete(cid, result))
            }
            Expr::Add(_, _) | Expr::Mul(_, _) => self.descend(
                cid,
                0,
                Phase::Left {
                    multiply: matches!(expr, Expr::Mul(_, _)),
                },
                frames,
            ),
            Expr::If { .. } => self.descend(cid, 0, Phase::Condition, frames),
        }
    }

    fn descend(
        &mut self,
        parent: Cid,
        index: usize,
        phase: Phase,
        frames: &mut Vec<Frame>,
    ) -> Result<Token, ProbeError> {
        let slot = self.memos[&parent]
            .slots
            .get(index)
            .ok_or(ProbeError::Invariant("missing request port"))?
            .clone();
        let token = match slot {
            Slot::Pending(cid) => Token::Request(cid),
            Slot::Ready {
                source,
                outcome,
                epoch,
            } => {
                if matches!(outcome, Outcome::Value(_)) || epoch == self.epoch {
                    self.stats.memo_hits += 1;
                    Token::Reply(outcome)
                } else {
                    Token::Request(source)
                }
            }
            Slot::Erased => return Err(ProbeError::Invariant("request to erased port")),
        };
        frames.push(Frame { parent, phase });
        self.allocate(1);
        Ok(token)
    }

    fn receive(&mut self, cid: Cid, index: usize, outcome: &Outcome) {
        let source = match self.memos[&cid].slots[index] {
            Slot::Pending(source) | Slot::Ready { source, .. } => source,
            Slot::Erased => unreachable!("answer on a live request port"),
        };
        self.memos.get_mut(&cid).expect("live parent").slots[index] =
            if *outcome == Outcome::Unknown {
                Slot::Pending(source)
            } else {
                Slot::Ready {
                    source,
                    outcome: outcome.clone(),
                    epoch: self.epoch,
                }
            };
    }

    fn erase_slot(&mut self, cid: Cid, index: usize) {
        let slot = &mut self.memos.get_mut(&cid).expect("live parent").slots[index];
        if !matches!(slot, Slot::Erased) {
            *slot = Slot::Erased;
            self.stats.live -= 1;
            self.stats.erasures += 1;
        }
    }

    fn complete(&mut self, cid: Cid, outcome: Outcome) -> Token {
        if matches!(outcome, Outcome::Value(_)) {
            let slots = self.memos[&cid].slots.len();
            for index in 0..slots {
                self.erase_slot(cid, index);
            }
            self.memos.get_mut(&cid).expect("live memo").slots.clear();
        }
        let memo = self.memos.get_mut(&cid).expect("live memo");
        memo.result = Some(outcome.clone());
        memo.evaluating = false;
        Token::Reply(outcome)
    }

    fn deliver(
        &mut self,
        frame: Frame,
        outcome: Outcome,
        frames: &mut Vec<Frame>,
    ) -> Result<Token, ProbeError> {
        let parent = frame.parent;
        match frame.phase {
            Phase::Left { multiply } => {
                self.receive(parent, 0, &outcome);
                if matches!(outcome, Outcome::Error(_)) {
                    return Ok(self.complete(parent, outcome));
                }
                self.descend(parent, 1, Phase::Right { multiply }, frames)
            }
            Phase::Right { multiply } => {
                self.receive(parent, 1, &outcome);
                let left = match &self.memos[&parent].slots[0] {
                    Slot::Ready { outcome, .. } => outcome.clone(),
                    Slot::Pending(_) => Outcome::Unknown,
                    Slot::Erased => return Err(ProbeError::Invariant("erased left operand")),
                };
                Ok(self.complete(parent, combine(left, outcome, multiply)))
            }
            Phase::Condition => {
                self.receive(parent, 0, &outcome);
                match outcome {
                    Outcome::Value(Scalar::Bool(value)) => {
                        let index = if value { 1 } else { 2 };
                        self.erase_slot(parent, if value { 2 } else { 1 });
                        self.descend(parent, index, Phase::Branch { index }, frames)
                    }
                    Outcome::Value(_) => Ok(self.complete(
                        parent,
                        Outcome::Error(Fault::Type("if condition is not a boolean")),
                    )),
                    other => Ok(self.complete(parent, other)),
                }
            }
            Phase::Branch { index } => {
                self.receive(parent, index, &outcome);
                Ok(self.complete(parent, outcome))
            }
        }
    }
}

fn combine(left: Outcome, right: Outcome, multiply: bool) -> Outcome {
    use Outcome::{Error, Unknown, Value};
    match (left, right) {
        (Error(error), _) | (_, Error(error)) => Error(error),
        (Value(Scalar::Bool(_)), _) | (_, Value(Scalar::Bool(_))) => {
            Error(Fault::Type(if multiply {
                "multiply expects two integers"
            } else {
                "add expects two integers"
            }))
        }
        (Value(Scalar::Int(left)), Value(Scalar::Int(right))) => {
            let result = if multiply {
                left.checked_mul(right)
            } else {
                left.checked_add(right)
            };
            result.map_or_else(
                || Error(Fault::Overflow(if multiply { "multiply" } else { "add" })),
                |value| Value(Scalar::Int(value)),
            )
        }
        _ => Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid(name: &str) -> Cid {
        Cid::digest(b"demand-a-test", name.as_bytes())
    }

    fn machine(root: &str, nodes: Vec<(&str, Expr)>) -> Machine {
        Machine::new(Program {
            root: cid(root),
            nodes: nodes
                .into_iter()
                .map(|(name, expr)| (cid(name), expr))
                .collect(),
        })
    }

    fn staged() -> Machine {
        machine(
            "root",
            vec![
                ("root", Expr::Add(cid("pending"), cid("pending"))),
                ("pending", Expr::Add(cid("hole"), cid("static"))),
                ("hole", Expr::Hole("x".into())),
                ("static", Expr::Mul(cid("six"), cid("seven"))),
                ("six", Expr::Value(Scalar::Int(6))),
                ("seven", Expr::Value(Scalar::Int(7))),
            ],
        )
    }

    fn check_quiescent_accounting(machine: &Machine) {
        let slots: usize = machine
            .memos
            .values()
            .map(|memo| {
                assert!(!memo.evaluating);
                memo.slots
                    .iter()
                    .filter(|slot| !matches!(slot, Slot::Erased))
                    .count()
            })
            .sum();
        assert_eq!(machine.stats.live, machine.memos.len() + slots);
    }

    #[test]
    fn unknown_epochs_keep_static_work_and_share_pending_requests() {
        let mut machine = staged();
        let first = machine.run(&BTreeMap::new(), 100).unwrap();
        assert_eq!(first.outcome, Outcome::Unknown);
        assert_eq!(first.stats.node_evaluations[&cid("pending")], 1);
        check_quiescent_accounting(&machine);
        let bindings = BTreeMap::from([("x".into(), Scalar::Int(1))]);
        let second = machine.run(&bindings, 100).unwrap();
        assert_eq!(second.outcome, Outcome::Value(Scalar::Int(86)));
        assert_eq!(second.stats.node_evaluations[&cid("static")], 1);
        assert_eq!(second.stats.node_evaluations[&cid("pending")], 2);
        check_quiescent_accounting(&machine);
        let third = machine.run(&BTreeMap::new(), 100).unwrap();
        assert_eq!(third.outcome, second.outcome);
        assert_eq!(third.stats.evaluations, second.stats.evaluations);
        let snapshot = machine.stats.clone();
        let conflict = BTreeMap::from([("x".into(), Scalar::Int(2))]);
        assert_eq!(
            machine.run(&conflict, 100),
            Err(ProbeError::ConflictingBinding("x".into()))
        );
        assert_eq!(machine.stats, snapshot);
    }

    #[test]
    fn every_budget_cut_can_start_another_observation() {
        for budget in 0..30 {
            let mut machine = staged();
            let bindings = BTreeMap::from([("x".into(), Scalar::Int(1))]);
            let _ = machine.run(&bindings, budget);
            check_quiescent_accounting(&machine);
            let result = machine.run(&BTreeMap::new(), 100).unwrap();
            assert_eq!(
                result.outcome,
                Outcome::Value(Scalar::Int(86)),
                "budget {budget}"
            );
            check_quiescent_accounting(&machine);
        }
    }

    #[test]
    fn new_bindings_can_change_error_precedence() {
        let mut machine = machine(
            "root",
            vec![
                ("root", Expr::Add(cid("mul"), cid("bool"))),
                ("mul", Expr::Mul(cid("hole"), cid("two"))),
                ("hole", Expr::Hole("x".into())),
                ("two", Expr::Value(Scalar::Int(2))),
                ("bool", Expr::Value(Scalar::Bool(true))),
            ],
        );
        let first = machine.run(&BTreeMap::new(), 100).unwrap();
        assert_eq!(
            first.outcome,
            Outcome::Error(Fault::Type("add expects two integers"))
        );
        let bindings = BTreeMap::from([("x".into(), Scalar::Int(i64::MAX))]);
        let second = machine.run(&bindings, 100).unwrap();
        assert_eq!(second.outcome, Outcome::Error(Fault::Overflow("multiply")));
        check_quiescent_accounting(&machine);
    }

    #[test]
    fn erased_branch_never_requests_its_missing_template() {
        let mut machine = machine(
            "root",
            vec![
                (
                    "root",
                    Expr::If {
                        condition: cid("yes"),
                        when_true: cid("answer"),
                        when_false: cid("absent"),
                    },
                ),
                ("yes", Expr::Value(Scalar::Bool(true))),
                ("answer", Expr::Value(Scalar::Int(7))),
            ],
        );
        let result = machine.run(&BTreeMap::new(), 100).unwrap();
        assert_eq!(result.outcome, Outcome::Value(Scalar::Int(7)));
        assert!(!result.stats.node_evaluations.contains_key(&cid("absent")));
        assert_eq!(result.stats.erasures, 3);
        check_quiescent_accounting(&machine);
    }
}
