//! Variant B: requests routed through fan trees, a sequential demand probe.
//!
//! This is **not** the principal-pair interaction-net backend, and it does not
//! claim Lafont locality.  A request arriving at a fan's *auxiliary* side
//! climbs to the fan's principal side; that is exactly the one departure from
//! pure nets that `INET-DEMAND.md` (section 9.4) identifies for variant B.  A
//! single explicit continuation carries the one demand token, so at most one
//! request is ever in flight and two requests can never race at one fan.
//! Determinism therefore rests on that invariant, not on confluence.
//!
//! Agents: a `Cell` per program node that has been reached (with one principal
//! side, its fan tree of use sites), `Fan` agents holding local request/return
//! state plus a parked reply copy for the side that did not ask, and `Leaf`
//! agents, one per materialized use site.  Every transition touches one agent
//! and one neighbour, except materialization, which is template-sized work:
//!
//! - climb: a request at a leaf or fan moves to its parent.  At a fan, a valid
//!   parked copy for the asking side answers at once (a fan-level memo hit);
//!   otherwise the fan records the asking side and the request continues.
//! - evaluate: a request reaching a cell reuses a valid cached result, or
//!   materializes the node (leaves for its operands, attached to the operands'
//!   fan trees) and requests operands in the reference's order.
//! - descend: a reply at a fan follows the recorded asking side and parks a
//!   copy for the other side; a reply at a leaf resumes the consumer.
//! - erase: releasing a use site collapses fans; when a node's last *static*
//!   use is gone its cell is dropped and the cell's own use sites are released.
//!
//! Sharing is exact: a node is evaluated at most once per epoch, and ground
//! values are never recomputed.  A value-completed cell releases its operand
//! use sites immediately (last-consumer reclamation); a cell whose result is
//! unknown or an error keeps them for a later epoch.  Static use counts cover
//! dormant (never materialized) consumers too, so a cached value survives while
//! an unselected branch still refers to it.  Releasing a dormant branch walks
//! its static interior; batching that through captured-wire summaries is the
//! brief's `Susp` refinement and is not done here.
//!
//! Accounting: `live` = cells + fans + leaves + continuation frames + the
//! in-flight token; immutable `Program` templates are excluded.  `erasures`
//! counts released use sites, materialized or dormant.  `routing_steps` counts
//! fan agents traversed by requests and replies; a use site wired directly to
//! its cell costs none.  `memo_hits` counts cell-level and parked-copy hits.
//! `evaluations` and `node_evaluations` count evaluations *started*: a node
//! whose evaluation began but was cut off by the budget is counted again when
//! a later run evaluates it.  Budget exhaustion cancels the continuation and
//! clears request marks along the outstanding request paths; the next run
//! starts a fresh observation.
//!
//! Logical agent deletion is not backing-storage reclamation: fan/leaf arrays
//! retain vacant slots and do not reuse them. `storage()` reports this cost.
//! Release cascades and cancellation use host worklists/path walks outside
//! the transition budget; a transition is not a bounded unit of host work.

use crate::cid::Cid;
use crate::demand::{Expr, Fault, Outcome, ProbeError, ProbeRun, ProbeStats, Program, Scalar};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

impl Side {
    fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

/// The parent of a fan-tree node: the cell whose use sites the tree collects,
/// or a fan and the side this node hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Up {
    Cell(Cid),
    Fan(usize, Side),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeRef {
    Leaf(usize),
    Fan(usize),
}

#[derive(Clone, Debug)]
struct Fan {
    up: Up,
    children: [Option<TreeRef>; 2],
    /// The side whose request is climbing through this fan right now.
    asking: Option<Side>,
    /// A reply copy kept for the side that did not ask, with its epoch.
    parked: Option<(Side, Outcome, usize)>,
}

#[derive(Clone, Debug)]
struct Leaf {
    up: Up,
    child: Cid,
    /// The consuming cell and operand slot, or `None` for the observation.
    consumer: Option<(Cid, usize)>,
}

#[derive(Clone, Debug, Default)]
struct Cell {
    /// Root of the fan tree collecting this node's materialized use sites.
    uses: Option<TreeRef>,
    /// Outgoing use-site leaves, one per operand; empty until materialized.
    slots: Vec<Option<usize>>,
    materialized: bool,
    evaluating: bool,
    result: Option<(Outcome, usize)>,
}

#[derive(Clone, Debug)]
enum Phase {
    Left { multiply: bool },
    Right { multiply: bool, left: Outcome },
    Condition,
    Branch,
}

/// One outstanding operand request: the consumer and what it does with the reply.
#[derive(Clone, Debug)]
struct Frame {
    parent: Cid,
    slot: usize,
    phase: Phase,
}

/// The single demand token: a request climbing a fan tree, a request that has
/// reached a cell, or a reply descending toward the asking use site.
#[derive(Debug)]
enum Token {
    Climb(TreeRef),
    Evaluate(Cid),
    Descend { at: TreeRef, outcome: Outcome },
}

enum Step {
    Next(Token),
    Done(Outcome),
}

enum Release {
    Leaf(usize),
    Dormant(Cid),
}

fn valid(outcome: &Outcome, when: usize, epoch: usize) -> bool {
    matches!(outcome, Outcome::Value(_)) || when == epoch
}

/// Fan-tree demand protocol with monotonic bindings and epoch resumption.
#[derive(Clone)]
pub struct Machine {
    program: Program,
    bindings: BTreeMap<String, Scalar>,
    epoch: usize,
    /// Remaining static use sites per program node, materialized or dormant.
    holds: BTreeMap<Cid, usize>,
    cells: BTreeMap<Cid, Cell>,
    fans: Vec<Option<Fan>>,
    leaves: Vec<Option<Leaf>>,
    fan_count: usize,
    leaf_count: usize,
    observer: Option<usize>,
    stats: ProbeStats,
}

impl Machine {
    pub fn storage(&self) -> crate::demand::StorageStats {
        crate::demand::StorageStats {
            template_nodes: self.program.nodes.len(),
            use_count_entries: self.holds.len(),
            issued_slots: self.fans.len() + self.leaves.len(),
            vacant_slots: self.fans.len() + self.leaves.len() - self.fan_count - self.leaf_count,
            capacity_slots: self.fans.capacity() + self.leaves.capacity(),
            capacity_bytes: self.fans.capacity() * std::mem::size_of::<Option<Fan>>()
                + self.leaves.capacity() * std::mem::size_of::<Option<Leaf>>(),
        }
    }

    pub fn new(program: Program) -> Self {
        let mut holds = BTreeMap::new();
        for expr in program.nodes.values() {
            for child in expr.children() {
                *holds.entry(child).or_default() += 1;
            }
        }
        // The observation is the root's one use site.
        *holds.entry(program.root).or_default() += 1;
        Self {
            program,
            bindings: BTreeMap::new(),
            epoch: 0,
            holds,
            cells: BTreeMap::new(),
            fans: Vec::new(),
            leaves: Vec::new(),
            fan_count: 0,
            leaf_count: 0,
            observer: None,
            stats: ProbeStats::default(),
        }
    }

    /// Run one observation epoch; `budget` limits transitions in this call.
    /// Conflicting bindings are rejected before any machine state changes.
    /// Budget exhaustion cancels the continuation; a later run observes afresh,
    /// keeping cached values but not a mid-transition token.
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
        self.bindings.extend(
            bindings
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        self.epoch += 1;
        let observer = self.observer()?;
        let mut frames = Vec::new();
        let mut token = Token::Climb(TreeRef::Leaf(observer));
        self.stats.requests += 1;
        for _ in 0..budget {
            self.stats.transitions += 1;
            self.account(frames.len() + 1);
            match self.step(token, &mut frames) {
                Ok(Step::Next(next)) => token = next,
                Ok(Step::Done(outcome)) => {
                    self.account(0);
                    return Ok(ProbeRun {
                        outcome,
                        stats: self.stats.clone(),
                    });
                }
                Err(error) => {
                    self.cancel(&frames);
                    return Err(error);
                }
            }
        }
        self.cancel(&frames);
        Err(ProbeError::BudgetExhausted { limit: budget })
    }

    /// Check quiescent invariants: no evaluation or request in progress, every
    /// tree link consistent with its parent, every cell still held by a static
    /// use, and `live` equal to a recount.  Intended for tests between runs.
    pub fn audit(&self) -> Result<(), ProbeError> {
        for (cid, cell) in &self.cells {
            if cell.evaluating {
                return Err(ProbeError::Invariant("quiescent cell is evaluating"));
            }
            if self.holds.get(cid).copied().unwrap_or(0) == 0 {
                return Err(ProbeError::Invariant("cell survives without a static use"));
            }
            if let Some(root) = cell.uses
                && self.up_of(root)? != Up::Cell(*cid)
            {
                return Err(ProbeError::Invariant(
                    "fan-tree root does not point at its cell",
                ));
            }
            for (slot, leaf) in cell.slots.iter().enumerate() {
                let Some(leaf) = leaf else { continue };
                if self.leaf(*leaf)?.consumer != Some((*cid, slot)) {
                    return Err(ProbeError::Invariant("slot leaf belongs to another cell"));
                }
            }
        }
        for (id, fan) in self.fans.iter().enumerate() {
            let Some(fan) = fan else { continue };
            if fan.asking.is_some() {
                return Err(ProbeError::Invariant("quiescent fan has a pending request"));
            }
            for (index, child) in fan.children.iter().enumerate() {
                let Some(child) = child else { continue };
                let side = if index == 0 { Side::Left } else { Side::Right };
                if self.up_of(*child)? != Up::Fan(id, side) {
                    return Err(ProbeError::Invariant("fan child does not point back"));
                }
            }
        }
        for (id, leaf) in self.leaves.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            // Every leaf's chain of parents ends at the cell it uses.
            let mut up = leaf.up;
            let mut hops = 0;
            loop {
                match up {
                    Up::Cell(cid) => {
                        if cid != leaf.child {
                            return Err(ProbeError::Invariant("leaf attached to the wrong cell"));
                        }
                        break;
                    }
                    Up::Fan(fan, side) => {
                        let fan = self.fan(fan)?;
                        if fan.children[side.index()] != Some(TreeRef::Leaf(id)) && hops == 0 {
                            return Err(ProbeError::Invariant("fan does not hold its leaf"));
                        }
                        up = fan.up;
                    }
                }
                hops += 1;
                if hops > self.fans.len() + 1 {
                    return Err(ProbeError::Invariant("cyclic fan tree"));
                }
            }
        }
        let recount = self.cells.len() + self.fan_count + self.leaf_count;
        if recount != self.stats.live {
            return Err(ProbeError::Invariant("live accounting drifted"));
        }
        Ok(())
    }

    fn account(&mut self, in_flight: usize) {
        self.stats.live = self.cells.len() + self.fan_count + self.leaf_count + in_flight;
        self.stats.peak_live = self.stats.peak_live.max(self.stats.live);
    }

    fn observer(&mut self) -> Result<usize, ProbeError> {
        if let Some(leaf) = self.observer {
            return Ok(leaf);
        }
        let root = self.program.root;
        let leaf = self.new_leaf(Leaf {
            up: Up::Cell(root),
            child: root,
            consumer: None,
        });
        self.attach(root, leaf)?;
        self.observer = Some(leaf);
        Ok(leaf)
    }

    /// Host-side recovery after an aborted run: clear evaluation flags of the
    /// cells with outstanding requests and the asking marks along each such
    /// request's path.  This walks only those paths, never the whole net.
    fn cancel(&mut self, frames: &[Frame]) {
        for frame in frames.iter().rev() {
            let leaf = self.cells.get_mut(&frame.parent).and_then(|cell| {
                cell.evaluating = false;
                cell.slots.get(frame.slot).copied().flatten()
            });
            let mut up = leaf.and_then(|leaf| self.leaves[leaf].as_ref().map(|leaf| leaf.up));
            while let Some(Up::Fan(fan, _)) = up {
                match self.fans[fan].as_mut() {
                    Some(fan) if fan.asking.is_some() => {
                        fan.asking = None;
                        up = Some(fan.up);
                    }
                    _ => break,
                }
            }
        }
        if let Some(cell) = self.cells.get_mut(&self.program.root) {
            cell.evaluating = false;
        }
        self.account(0);
    }

    fn step(&mut self, token: Token, frames: &mut Vec<Frame>) -> Result<Step, ProbeError> {
        match token {
            Token::Climb(at) => match self.up_of(at)? {
                Up::Cell(cid) => Ok(Step::Next(Token::Evaluate(cid))),
                Up::Fan(fan, side) => self.climb_into(fan, side, at),
            },
            Token::Evaluate(cid) => self.evaluate(cid, frames),
            Token::Descend {
                at: TreeRef::Fan(fan),
                outcome,
            } => self.descend_fan(fan, outcome),
            Token::Descend {
                at: TreeRef::Leaf(leaf),
                outcome,
            } => match self.leaf(leaf)?.consumer {
                None => {
                    if !frames.is_empty() {
                        return Err(ProbeError::Invariant(
                            "observation completed with requests outstanding",
                        ));
                    }
                    Ok(Step::Done(outcome))
                }
                Some((parent, slot)) => self.deliver(parent, slot, outcome, frames),
            },
        }
    }

    /// A request arrives at a fan on its auxiliary side `side`, coming from
    /// tree node `from`.
    fn climb_into(&mut self, fan: usize, side: Side, from: TreeRef) -> Result<Step, ProbeError> {
        self.stats.routing_steps += 1;
        let epoch = self.epoch;
        let agent = self.fan_mut(fan)?;
        if let Some((for_side, outcome, when)) = &agent.parked
            && *for_side == side
            && valid(outcome, *when, epoch)
        {
            let outcome = outcome.clone();
            agent.parked = None;
            self.stats.memo_hits += 1;
            return Ok(Step::Next(Token::Descend { at: from, outcome }));
        }
        if agent.asking.is_some() {
            return Err(ProbeError::Invariant("two requests met at one fan"));
        }
        agent.asking = Some(side);
        Ok(Step::Next(Token::Climb(TreeRef::Fan(fan))))
    }

    /// A reply arrives at a fan from its principal side.
    fn descend_fan(&mut self, fan: usize, outcome: Outcome) -> Result<Step, ProbeError> {
        self.stats.routing_steps += 1;
        let epoch = self.epoch;
        let agent = self.fan_mut(fan)?;
        let side = agent.asking.take().ok_or(ProbeError::Invariant(
            "reply reached a fan without a request",
        ))?;
        let next = agent.children[side.index()]
            .ok_or(ProbeError::Invariant("reply toward an erased side"))?;
        if agent.children[side.other().index()].is_some() {
            agent.parked = Some((side.other(), outcome.clone(), epoch));
        }
        Ok(Step::Next(Token::Descend { at: next, outcome }))
    }

    fn evaluate(&mut self, cid: Cid, frames: &mut Vec<Frame>) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get(&cid)
            .ok_or(ProbeError::Invariant("request reached a missing cell"))?;
        if cell.evaluating {
            return Err(ProbeError::Invariant("cyclic request"));
        }
        let uses = cell.uses;
        if let Some((outcome, when)) = &cell.result
            && valid(outcome, *when, epoch)
        {
            let outcome = outcome.clone();
            self.stats.memo_hits += 1;
            let at = uses.ok_or(ProbeError::Invariant("cached cell has no consumer"))?;
            return Ok(Step::Next(Token::Descend { at, outcome }));
        }
        if !cell.materialized {
            self.materialize(cid)?;
        }
        let expr = self
            .program
            .nodes
            .get(&cid)
            .cloned()
            .ok_or(ProbeError::MissingNode(cid))?;
        self.cells
            .get_mut(&cid)
            .expect("cell checked above")
            .evaluating = true;
        self.stats.evaluations += 1;
        *self.stats.node_evaluations.entry(cid).or_default() += 1;
        match expr {
            Expr::Value(value) => self.complete(cid, Outcome::Value(value)),
            Expr::Hole(name) => {
                let outcome = self
                    .bindings
                    .get(&name)
                    .cloned()
                    .map_or(Outcome::Unknown, Outcome::Value);
                self.complete(cid, outcome)
            }
            Expr::Add(_, _) | Expr::Mul(_, _) => self.request(
                cid,
                0,
                Phase::Left {
                    multiply: matches!(expr, Expr::Mul(_, _)),
                },
                frames,
            ),
            Expr::If { .. } => self.request(cid, 0, Phase::Condition, frames),
        }
    }

    /// Instantiate a node's operand use sites.  This is template-sized work,
    /// charged as one transition here; the brief accounts it as materialization.
    fn materialize(&mut self, cid: Cid) -> Result<(), ProbeError> {
        let children = self
            .program
            .nodes
            .get(&cid)
            .ok_or(ProbeError::MissingNode(cid))?
            .children();
        let mut slots = Vec::with_capacity(children.len());
        for (slot, child) in children.into_iter().enumerate() {
            let leaf = self.new_leaf(Leaf {
                up: Up::Cell(child),
                child,
                consumer: Some((cid, slot)),
            });
            self.attach(child, leaf)?;
            slots.push(Some(leaf));
        }
        let cell = self
            .cells
            .get_mut(&cid)
            .ok_or(ProbeError::Invariant("materializing a missing cell"))?;
        cell.slots = slots;
        cell.materialized = true;
        Ok(())
    }

    /// Add a use site to a node's fan tree, creating the node's pending cell if
    /// this is its first materialized consumer.  The tree grows as a chain:
    /// a new fan takes the old tree on its left and the new leaf on its right.
    fn attach(&mut self, child: Cid, leaf: usize) -> Result<(), ProbeError> {
        let old = self.cells.entry(child).or_default().uses;
        match old {
            None => {
                self.cells.get_mut(&child).expect("cell just created").uses =
                    Some(TreeRef::Leaf(leaf));
                self.leaf_mut(leaf)?.up = Up::Cell(child);
            }
            Some(old) => {
                let fan = self.new_fan(Fan {
                    up: Up::Cell(child),
                    children: [Some(old), Some(TreeRef::Leaf(leaf))],
                    asking: None,
                    parked: None,
                });
                self.set_up(old, Up::Fan(fan, Side::Left))?;
                self.leaf_mut(leaf)?.up = Up::Fan(fan, Side::Right);
                self.cells.get_mut(&child).expect("cell exists").uses = Some(TreeRef::Fan(fan));
            }
        }
        Ok(())
    }

    fn request(
        &mut self,
        parent: Cid,
        slot: usize,
        phase: Phase,
        frames: &mut Vec<Frame>,
    ) -> Result<Step, ProbeError> {
        let leaf = self
            .cells
            .get(&parent)
            .and_then(|cell| cell.slots.get(slot).copied().flatten())
            .ok_or(ProbeError::Invariant("request through an erased use site"))?;
        frames.push(Frame {
            parent,
            slot,
            phase,
        });
        self.stats.requests += 1;
        Ok(Step::Next(Token::Climb(TreeRef::Leaf(leaf))))
    }

    fn complete(&mut self, cid: Cid, outcome: Outcome) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get_mut(&cid)
            .ok_or(ProbeError::Invariant("completing a missing cell"))?;
        cell.evaluating = false;
        cell.result = Some((outcome.clone(), epoch));
        if matches!(outcome, Outcome::Value(_)) {
            // A ground value no longer needs its operands: last-consumer release.
            let slots: Vec<usize> = cell.slots.iter_mut().filter_map(Option::take).collect();
            self.drain(slots.into_iter().map(Release::Leaf).collect())?;
        }
        let at = self
            .cells
            .get(&cid)
            .and_then(|cell| cell.uses)
            .ok_or(ProbeError::Invariant("completed cell has no consumer"))?;
        Ok(Step::Next(Token::Descend { at, outcome }))
    }

    fn deliver(
        &mut self,
        parent: Cid,
        slot: usize,
        outcome: Outcome,
        frames: &mut Vec<Frame>,
    ) -> Result<Step, ProbeError> {
        let frame = frames
            .pop()
            .ok_or(ProbeError::Invariant("reply without a pending request"))?;
        if frame.parent != parent || frame.slot != slot {
            return Err(ProbeError::Invariant("reply reached the wrong consumer"));
        }
        match frame.phase {
            Phase::Left { multiply } => {
                if matches!(outcome, Outcome::Error(_)) {
                    return self.complete(parent, outcome);
                }
                self.request(
                    parent,
                    1,
                    Phase::Right {
                        multiply,
                        left: outcome,
                    },
                    frames,
                )
            }
            Phase::Right { multiply, left } => {
                self.complete(parent, combine(left, outcome, multiply))
            }
            Phase::Condition => match outcome {
                Outcome::Value(Scalar::Bool(chosen)) => {
                    let (selected, rejected) = if chosen { (1, 2) } else { (2, 1) };
                    self.erase_use(parent, rejected)?;
                    self.request(parent, selected, Phase::Branch, frames)
                }
                Outcome::Value(_) => self.complete(
                    parent,
                    Outcome::Error(Fault::Type("if condition is not a boolean")),
                ),
                other => self.complete(parent, other),
            },
            Phase::Branch => self.complete(parent, outcome),
        }
    }

    fn erase_use(&mut self, parent: Cid, slot: usize) -> Result<(), ProbeError> {
        let leaf = self
            .cells
            .get_mut(&parent)
            .and_then(|cell| cell.slots.get_mut(slot))
            .and_then(Option::take);
        match leaf {
            Some(leaf) => self.drain(vec![Release::Leaf(leaf)]),
            None => Ok(()),
        }
    }

    /// Release use sites.  Each item releases one static use of a node; when a
    /// node's last use goes, its cell is dropped and what it held is released
    /// in turn: through its leaves if it was materialized, otherwise through
    /// its static operands (the dormant interior).
    fn drain(&mut self, mut work: Vec<Release>) -> Result<(), ProbeError> {
        while let Some(item) = work.pop() {
            let child = match item {
                Release::Leaf(leaf) => self.detach_leaf(leaf)?,
                Release::Dormant(cid) => cid,
            };
            self.stats.erasures += 1;
            let holds = self
                .holds
                .get_mut(&child)
                .ok_or(ProbeError::Invariant("released a node with no use count"))?;
            *holds = holds
                .checked_sub(1)
                .ok_or(ProbeError::Invariant("released more uses than exist"))?;
            if *holds > 0 {
                continue;
            }
            let dormant_children = || {
                self.program
                    .nodes
                    .get(&child)
                    .map(Expr::children)
                    .unwrap_or_default()
                    .into_iter()
                    .map(Release::Dormant)
            };
            match self.cells.remove(&child) {
                Some(cell) if cell.evaluating => {
                    return Err(ProbeError::Invariant(
                        "released a cell during its evaluation",
                    ));
                }
                Some(cell) if cell.materialized => {
                    work.extend(cell.slots.into_iter().flatten().map(Release::Leaf));
                }
                _ => work.extend(dormant_children()),
            }
        }
        Ok(())
    }

    /// Remove a leaf from its fan tree, collapsing fans left with one side.
    /// Returns the node the leaf used.
    fn detach_leaf(&mut self, leaf: usize) -> Result<Cid, ProbeError> {
        let agent = self
            .leaves
            .get_mut(leaf)
            .and_then(Option::take)
            .ok_or(ProbeError::Invariant("detached a missing leaf"))?;
        self.leaf_count -= 1;
        let mut up = agent.up;
        loop {
            match up {
                Up::Cell(cid) => {
                    if let Some(cell) = self.cells.get_mut(&cid) {
                        cell.uses = None;
                    }
                    return Ok(agent.child);
                }
                Up::Fan(fan, side) => {
                    let f = self.fan_mut(fan)?;
                    f.children[side.index()] = None;
                    if f.parked
                        .as_ref()
                        .is_some_and(|(for_side, ..)| *for_side == side)
                    {
                        f.parked = None;
                    }
                    if f.asking == Some(side) {
                        return Err(ProbeError::Invariant(
                            "erased the side of an in-flight request",
                        ));
                    }
                    match f.children[side.other().index()] {
                        None => {
                            // Both sides gone: the fan dies and the erasure moves up.
                            up = f.up;
                            self.fans[fan] = None;
                            self.fan_count -= 1;
                        }
                        Some(_) if f.asking.is_some() => {
                            // A request is climbing through the other side; keep
                            // the fan so the reply can follow its marks.
                            return Ok(agent.child);
                        }
                        Some(sibling) => {
                            // Collapse: the sibling takes the fan's place.  A copy
                            // parked for the sibling is dropped; the cell still
                            // holds the value.
                            let above = f.up;
                            self.fans[fan] = None;
                            self.fan_count -= 1;
                            self.set_up(sibling, above)?;
                            self.replace_child(above, TreeRef::Fan(fan), sibling)?;
                            return Ok(agent.child);
                        }
                    }
                }
            }
        }
    }

    fn new_leaf(&mut self, leaf: Leaf) -> usize {
        self.leaves.push(Some(leaf));
        self.leaf_count += 1;
        self.leaves.len() - 1
    }

    fn new_fan(&mut self, fan: Fan) -> usize {
        self.fans.push(Some(fan));
        self.fan_count += 1;
        self.fans.len() - 1
    }

    fn leaf(&self, id: usize) -> Result<&Leaf, ProbeError> {
        self.leaves
            .get(id)
            .and_then(Option::as_ref)
            .ok_or(ProbeError::Invariant("missing leaf agent"))
    }

    fn leaf_mut(&mut self, id: usize) -> Result<&mut Leaf, ProbeError> {
        self.leaves
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or(ProbeError::Invariant("missing leaf agent"))
    }

    fn fan(&self, id: usize) -> Result<&Fan, ProbeError> {
        self.fans
            .get(id)
            .and_then(Option::as_ref)
            .ok_or(ProbeError::Invariant("missing fan agent"))
    }

    fn fan_mut(&mut self, id: usize) -> Result<&mut Fan, ProbeError> {
        self.fans
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or(ProbeError::Invariant("missing fan agent"))
    }

    fn up_of(&self, at: TreeRef) -> Result<Up, ProbeError> {
        match at {
            TreeRef::Leaf(leaf) => Ok(self.leaf(leaf)?.up),
            TreeRef::Fan(fan) => Ok(self.fan(fan)?.up),
        }
    }

    fn set_up(&mut self, at: TreeRef, up: Up) -> Result<(), ProbeError> {
        match at {
            TreeRef::Leaf(leaf) => self.leaf_mut(leaf)?.up = up,
            TreeRef::Fan(fan) => self.fan_mut(fan)?.up = up,
        }
        Ok(())
    }

    fn replace_child(&mut self, up: Up, old: TreeRef, new: TreeRef) -> Result<(), ProbeError> {
        match up {
            Up::Cell(cid) => {
                let cell = self
                    .cells
                    .get_mut(&cid)
                    .ok_or(ProbeError::Invariant("tree root of a missing cell"))?;
                if cell.uses != Some(old) {
                    return Err(ProbeError::Invariant(
                        "cell does not hold the collapsed fan",
                    ));
                }
                cell.uses = Some(new);
            }
            Up::Fan(fan, side) => {
                let fan = self.fan_mut(fan)?;
                if fan.children[side.index()] != Some(old) {
                    return Err(ProbeError::Invariant("fan does not hold the collapsed fan"));
                }
                fan.children[side.index()] = Some(new);
            }
        }
        Ok(())
    }
}

/// The reference's operand order: a left error first, then a right error,
/// then the type check on known operands, then checked arithmetic.  An
/// unknown operand beside a known non-integer is still a type error.
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
    use crate::{Atom, Bindings, Node, ReduceError, Reducer, Store};

    const BUDGET: usize = 1_000_000;
    type Facts = BTreeMap<String, Scalar>;

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

    fn machine(store: &Store, root: Cid) -> Machine {
        Machine::new(Program::from_store(store, root).unwrap())
    }

    fn count(run: &ProbeRun, node: Cid) -> usize {
        run.stats.node_evaluations.get(&node).copied().unwrap_or(0)
    }

    /// The CAS reducer as the semantic control.
    fn reference(store: &mut Store, root: Cid, facts: &Facts) -> Outcome {
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
            Err(ReduceError::IntegerOverflow(operation)) => {
                Outcome::Error(Fault::Overflow(operation))
            }
            Err(ReduceError::Type(message)) => Outcome::Error(Fault::Type(message)),
            other => panic!("unexpected control result {other:?}"),
        }
    }

    /// `h(c, x) = Add(If(c, Add(x, 1), Mul(x, 2)), x)` with `x = Mul(6, 7)`.
    fn dynamic_first_consumer(store: &mut Store) -> (Cid, Cid, Cid, Cid) {
        let flag = hole(store, "flag");
        let six = int(store, 6);
        let seven = int(store, 7);
        let one = int(store, 1);
        let two = int(store, 2);
        let shared = store.intern(Node::Mul(six, seven));
        let yes = store.intern(Node::Add(shared, one));
        let no = store.intern(Node::Mul(shared, two));
        let branch = store.intern(Node::If {
            condition: flag,
            when_true: yes,
            when_false: no,
        });
        let root = store.intern(Node::Add(branch, shared));
        (root, shared, yes, no)
    }

    #[test]
    fn trace_1_either_branch_may_be_the_first_consumer_of_one_evaluation() {
        for (flag, expected, evaluated_branch) in [(true, 85, 0), (false, 126, 1)] {
            let mut store = Store::new();
            let (root, shared, yes, no) = dynamic_first_consumer(&mut store);
            let mut machine = machine(&store, root);
            let run = machine
                .run(&facts(&[("flag", Scalar::Bool(flag))]), BUDGET)
                .unwrap();
            assert_eq!(run.outcome, Outcome::Value(Scalar::Int(expected)));
            assert_eq!(count(&run, shared), 1);
            assert_eq!(count(&run, [yes, no][evaluated_branch]), 1);
            assert_eq!(count(&run, [no, yes][evaluated_branch]), 0);
            assert!(run.stats.memo_hits > 0);
            // Last-consumer reclamation: only the root value and its observer
            // remain once the whole program has a value.
            assert_eq!(run.stats.live, 2);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn trace_1_with_the_condition_arriving_in_a_later_epoch() {
        for (flag, expected) in [(true, 85), (false, 126)] {
            let mut store = Store::new();
            let (root, shared, ..) = dynamic_first_consumer(&mut store);
            let mut machine = machine(&store, root);
            let pending = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(pending.outcome, Outcome::Unknown);
            // The outer operand `x` is demanded in epoch 1; the branches stay dormant.
            assert_eq!(count(&pending, shared), 1);
            machine.audit().unwrap();
            let done = machine
                .run(&facts(&[("flag", Scalar::Bool(flag))]), BUDGET)
                .unwrap();
            assert_eq!(done.outcome, Outcome::Value(Scalar::Int(expected)));
            assert_eq!(count(&done, shared), 1);
            assert_eq!(done.stats.live, 2);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn trace_4_erasing_every_consumer_never_materializes_the_shared_work() {
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
        let mut machine = machine(&store, root);
        let run = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(3)));
        assert_eq!(count(&run, bad), 0);
        assert_eq!(count(&run, max), 0);
        assert!(run.stats.erasures >= 2);
        assert_eq!(run.stats.live, 2);
        machine.audit().unwrap();
    }

    #[test]
    fn trace_5_blocked_shared_work_resumes_without_recomputing_its_static_island() {
        let mut store = Store::new();
        let y = hole(&mut store, "y");
        let six = int(&mut store, 6);
        let seven = int(&mut store, 7);
        let island = store.intern(Node::Mul(six, seven));
        let shared = store.intern(Node::Add(y, island));
        let root = store.intern(Node::Add(shared, shared));
        let mut machine = machine(&store, root);
        let first = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(first.outcome, Outcome::Unknown);
        assert_eq!(count(&first, shared), 1);
        assert_eq!(count(&first, island), 1);
        // The second consumer took the parked Unknown: one fan-level hit.
        assert!(first.stats.memo_hits >= 1);
        machine.audit().unwrap();
        let second = machine
            .run(&facts(&[("y", Scalar::Int(1))]), BUDGET)
            .unwrap();
        assert_eq!(second.outcome, Outcome::Value(Scalar::Int(86)));
        assert_eq!(count(&second, shared), 2);
        assert_eq!(count(&second, island), 1);
        assert_eq!(second.stats.live, 2);
        machine.audit().unwrap();
    }

    #[test]
    fn nested_sharing_evaluates_every_node_once_and_reclaims_as_it_goes() {
        let mut store = Store::new();
        let mut root = int(&mut store, 1);
        let mut nodes = vec![root];
        for _ in 0..30 {
            root = store.intern(Node::Add(root, root));
            nodes.push(root);
        }
        let mut machine = machine(&store, root);
        let run = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(1 << 30)));
        for node in &nodes {
            assert_eq!(count(&run, *node), 1);
        }
        assert_eq!(run.stats.evaluations, nodes.len());
        assert!(run.stats.transitions < 1_000, "{}", run.stats.transitions);
        // About five agents per level are live at the deepest point; the
        // bound is linear in depth, not exponential in sharing.
        assert!(run.stats.peak_live < 200, "{}", run.stats.peak_live);
        assert_eq!(run.stats.live, 2);
        machine.audit().unwrap();
    }

    #[test]
    fn wide_sharing_evaluates_once_and_routes_through_the_fan_chain() {
        let mut store = Store::new();
        let six = int(&mut store, 6);
        let seven = int(&mut store, 7);
        let shared = store.intern(Node::Mul(six, seven));
        let mut root = shared;
        for _ in 0..31 {
            root = store.intern(Node::Add(shared, root));
        }
        let mut machine = machine(&store, root);
        let run = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(42 * 32)));
        assert_eq!(count(&run, shared), 1);
        assert_eq!(run.stats.memo_hits, 31);
        // Thirty-two use sites: the first request climbs the whole chain and
        // every later one is answered from a parked copy.
        assert!(run.stats.routing_steps >= 62, "{}", run.stats.routing_steps);
        assert_eq!(run.stats.live, 2);
        machine.audit().unwrap();
    }

    #[test]
    fn fault_order_matches_the_reference_table() {
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
            store.intern(Node::Add(bad_add, bad_mul)),
            store.intern(Node::Add(bad_mul, bad_add)),
            store.intern(Node::Add(truth, bad_mul)),
            store.intern(Node::Add(bad_type, bad_mul)),
            store.intern(Node::Add(x, truth)),
            store.intern(Node::Mul(truth, x)),
            store.intern(Node::Add(x, bad_add)),
            store.intern(Node::Add(bad_add, x)),
            store.intern(Node::If {
                condition: one,
                when_true: bad_add,
                when_false: bad_mul,
            }),
        ];
        for root in cases {
            let expected = reference(&mut store, root, &Facts::new());
            assert!(matches!(expected, Outcome::Error(_)));
            let mut machine = machine(&store, root);
            let run = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(run.outcome, expected, "{}", store.format(root));
            machine.audit().unwrap();
        }
    }

    #[test]
    fn errors_are_epoch_local_but_values_persist() {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let two = int(&mut store, 2);
        let truth = boolean(&mut store, true);
        let product = store.intern(Node::Mul(x, two));
        let root = store.intern(Node::Add(product, truth));
        let mut machine = machine(&store, root);
        let first = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(
            first.outcome,
            Outcome::Error(Fault::Type("add expects two integers"))
        );
        let second = machine
            .run(&facts(&[("x", Scalar::Int(i64::MAX))]), BUDGET)
            .unwrap();
        assert_eq!(second.outcome, Outcome::Error(Fault::Overflow("multiply")));
        // The known operand was evaluated once for both epochs.
        assert_eq!(count(&second, two), 1);
        let third = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(third.outcome, second.outcome);
        machine.audit().unwrap();
    }

    #[test]
    fn conflicting_bindings_are_rejected_before_any_state_changes() {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let a = hole(&mut store, "a");
        let root = store.intern(Node::Add(a, x));
        let mut machine = machine(&store, root);
        machine
            .run(&facts(&[("x", Scalar::Int(1))]), BUDGET)
            .unwrap();
        let snapshot = machine.stats.clone();
        assert_eq!(
            machine.run(
                &facts(&[("a", Scalar::Int(99)), ("x", Scalar::Int(2))]),
                BUDGET
            ),
            Err(ProbeError::ConflictingBinding("x".into()))
        );
        assert_eq!(machine.stats, snapshot);
        assert_eq!(
            machine
                .run(&facts(&[("a", Scalar::Int(3))]), BUDGET)
                .unwrap()
                .outcome,
            Outcome::Value(Scalar::Int(4))
        );
        machine.audit().unwrap();
    }

    #[test]
    fn every_budget_cut_leaves_a_consistent_machine_that_can_finish() {
        let mut store = Store::new();
        let (root, ..) = dynamic_first_consumer(&mut store);
        let bindings = facts(&[("flag", Scalar::Bool(false))]);
        let full = machine(&store, root).run(&bindings, BUDGET).unwrap();
        for budget in 0..=full.stats.transitions {
            let mut machine = machine(&store, root);
            match machine.run(&bindings, budget) {
                Ok(run) => assert_eq!(run.outcome, Outcome::Value(Scalar::Int(126))),
                Err(ProbeError::BudgetExhausted { limit }) => assert_eq!(limit, budget),
                Err(other) => panic!("budget {budget}: {other:?}"),
            }
            machine
                .audit()
                .unwrap_or_else(|error| panic!("budget {budget}: {error}"));
            let done = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(
                done.outcome,
                Outcome::Value(Scalar::Int(126)),
                "budget {budget}"
            );
            machine.audit().unwrap();
        }
    }

    #[test]
    fn a_cloned_machine_resumes_independently() {
        let mut store = Store::new();
        let (root, shared, ..) = dynamic_first_consumer(&mut store);
        let mut machine = machine(&store, root);
        machine.run(&Facts::new(), BUDGET).unwrap();
        let mut copy = machine.clone();
        let yes = copy
            .run(&facts(&[("flag", Scalar::Bool(true))]), BUDGET)
            .unwrap();
        let no = machine
            .run(&facts(&[("flag", Scalar::Bool(false))]), BUDGET)
            .unwrap();
        assert_eq!(yes.outcome, Outcome::Value(Scalar::Int(85)));
        assert_eq!(no.outcome, Outcome::Value(Scalar::Int(126)));
        assert_eq!(count(&yes, shared), 1);
        assert_eq!(count(&no, shared), 1);
    }

    fn random(seed: &mut u64) -> usize {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 32) as usize
    }

    #[test]
    fn generated_dags_match_the_reference_directly_and_in_staged_epochs() {
        let mut seed = 0x6261_6e5f_7472_6565_u64;
        for case in 0..150 {
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
            let x = ("x", Scalar::Int(2));
            let flag = ("flag", Scalar::Bool(case % 2 == 0));
            for stages in [vec![x.clone(), flag.clone()], vec![flag, x]] {
                let mut machine = machine(&store, root);
                let mut accumulated = Facts::new();
                let direct = machine.run(&Facts::new(), BUDGET).unwrap();
                assert_eq!(
                    direct.outcome,
                    reference(&mut store, root, &accumulated),
                    "case {case}: {}",
                    store.format(root)
                );
                machine.audit().unwrap();
                for stage in stages {
                    accumulated.insert(stage.0.into(), stage.1.clone());
                    let expected = reference(&mut store, root, &accumulated);
                    let run = machine.run(&facts(&[stage]), BUDGET).unwrap();
                    assert_eq!(run.outcome, expected, "case {case}: {}", store.format(root));
                    machine.audit().unwrap();
                    // Ground work is never repeated: every node is evaluated
                    // at most once per epoch, and values are never re-evaluated.
                    for (node, evaluations) in &run.stats.node_evaluations {
                        assert!(*evaluations <= machine.epoch, "{}", store.format(*node));
                    }
                }
            }
        }
    }
}
