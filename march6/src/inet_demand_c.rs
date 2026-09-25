//! Variant C: a separate control port; requests travel on control wires only.
//!
//! This is **not** the principal-pair interaction-net backend.  Every
//! computation agent here has two interaction-capable sides, a *data* side
//! carrying its value and a *control* side carrying the demand token, so
//! Lafont's single-principal determinism theorem does not apply.  What holds
//! instead is stated as scheduling constraints, checked by the machine:
//!
//! - S1: one demand token drives every control rule; it is never duplicated.
//! - S2: when a cell completes, its value distribution (a series of stateless
//!   fan copies) runs to quiescence, as budgeted transitions, before the token
//!   returns.  No other rule fires meanwhile.
//! - S3: an erasure cascade runs to completion when triggered.
//! - S4: nothing fires ahead of the token.  Ahead-of-token firing of pure
//!   arithmetic is a separately testable alternative, not part of this slice.
//!
//! One sequential scheduler succeeding is not evidence of confluence.  The
//! critical pairs that a free scheduler would have to resolve are listed in
//! `git show 6f17b9b:march6/INET-DEMAND.md`, section 10 (retired research).
//!
//! The token carries its own return address: the use site to resume.  A
//! reply therefore goes straight back to its caller, and nothing on the way
//! in has to remember who called.  Agents and where state lives:
//! - `Cell`, one per reached program node: its cached result, whether it is
//!   evaluating, its operand use sites, and its *control state* (`Idle`, or
//!   `Waiting` for one operand with a phase and the site to return to).  The
//!   chain of waiting cells is the continuation; there is no host-side stack.
//! - `Site`, one per materialized use site: which cell it uses, its consumer,
//!   its position in both of the used cell's trees, and (reply-on-data only)
//!   a parked copy of the value with its epoch.
//! - `Fan`, the data-side k-to-1 tree: stateless.  A value entering a fan is
//!   copied to both children.
//! - `Merge`, the control-side k-to-1 tree: stateless.  A call entering a
//!   merge from either side continues upward.
//!
//! Two reply topologies are kept visible.  With `Topology::ReplyOnData`, the
//! default, a completing cell broadcasts its value down the fan tree, every
//! site parks a copy, and the token returns to its caller's site empty; a
//! consumer whose site already holds a valid copy never enters the control
//! tree at all.  With `Topology::ReplyOnControl` the value rides back with the
//! token and sites hold no copies, so every consumer calls.  A site attached
//! after its cell already has a valid value, or a broadcast cut short by the
//! budget, is served by a late copy carried on the return.
//!
//! Sharing is exact and ground values are never recomputed, with the same
//! static use counts as variant B: a cached value survives while a dormant
//! consumer still refers to it, and releasing a dormant branch walks its
//! static interior.
//!
//! Accounting: `live` = cells + fans + merges + sites + the in-flight token;
//! immutable `Program` templates are excluded.  `erasures` counts released use
//! sites, materialized or dormant.  `routing_steps` counts control-merge hops
//! (calls only; replies are direct) plus data-fan hops during distribution;
//! `detail()` separates them and adds copies, teardown steps, and agent
//! counts.  `memo_hits` counts site-level (parked copy) and cell-level hits.
//! `evaluations` and `node_evaluations` count evaluations *started*; a node cut
//! off by the budget is counted again when a later run evaluates it.  Budget
//! exhaustion clears the chain of waiting cells from the root; nothing is
//! persisted, and the next run observes afresh.
//!
//! Logical deletion leaves vacant backing-array slots; `storage()` reports
//! retained lengths and capacities. The distribution token and release cascade
//! use host worklists. Cleanup and cancellation run outside transition fuel.
//! Return-site IDs are explicit stored references, dereferenced directly by
//! the host; counted direct returns do not establish local port-rewrite rules.

use crate::cid::Cid;
use crate::demand::{Expr, Fault, Outcome, ProbeError, ProbeRun, ProbeStats, Program, Scalar};
use std::collections::BTreeMap;

/// How a completed value reaches its consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topology {
    /// Broadcast copies down the data fan tree; the token returns empty.
    ReplyOnData,
    /// The value rides back with the token; sites keep no copies.
    ReplyOnControl,
}

/// Counters beyond the shared `ProbeStats`, so control routing, data
/// distribution, and teardown are visible separately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    pub control_hops: usize,
    pub data_hops: usize,
    pub data_copies: usize,
    pub site_hits: usize,
    pub cell_hits: usize,
    pub teardown_steps: usize,
    pub cells: usize,
    pub fans: usize,
    pub merges: usize,
    pub sites: usize,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tree {
    Data,
    Control,
}

/// The parent of a tree node: the cell the tree belongs to, or a fan or
/// merge and the side this node hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Up {
    Cell(Cid),
    Fan(usize, Side),
    Merge(usize, Side),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeRef {
    Site(usize),
    Fan(usize),
    Merge(usize),
}

#[derive(Clone, Debug)]
struct Fan {
    up: Up,
    children: [Option<TreeRef>; 2],
}

#[derive(Clone, Debug)]
struct Merge {
    up: Up,
    children: [Option<TreeRef>; 2],
}

#[derive(Clone, Debug)]
struct Site {
    child: Cid,
    /// The consuming cell and operand slot, or `None` for the observation.
    consumer: Option<(Cid, usize)>,
    data_up: Up,
    control_up: Up,
    /// A copy of the used cell's value, with its epoch (reply-on-data only).
    parked: Option<(Outcome, usize)>,
}

#[derive(Clone, Debug)]
enum Phase {
    Left { multiply: bool },
    Right { multiply: bool, left: Outcome },
    Condition,
    Branch,
}

#[derive(Clone, Debug, Default)]
enum Control {
    #[default]
    Idle,
    /// Demanding operand `slot`; `return_to` is the site this cell's own
    /// caller is waiting at.
    Waiting {
        slot: usize,
        phase: Phase,
        return_to: usize,
    },
}

#[derive(Clone, Debug, Default)]
struct Cell {
    data_uses: Option<TreeRef>,
    control_uses: Option<TreeRef>,
    slots: Vec<Option<usize>>,
    materialized: bool,
    evaluating: bool,
    result: Option<(Outcome, usize)>,
    control: Control,
}

/// The single demand token: where it is and what it carries.  A call always
/// carries `from`, the site to return to.
#[derive(Debug)]
enum Token {
    /// At a use site, about to call the used cell (or read a parked copy).
    Call(usize),
    /// A call moving up the control tree from this node.
    Climb { at: TreeRef, from: usize },
    /// A call that reached its cell.
    AtCell { cid: Cid, from: usize },
    /// Distributing a completed value down the data tree (S2), then
    /// returning to `return_to`.
    Distribute {
        pending: Vec<TreeRef>,
        outcome: Outcome,
        return_to: usize,
    },
    /// Back at the calling site; `Some` carries a value (reply-on-control or
    /// a late copy), `None` means read the site's parked copy.
    Resume {
        site: usize,
        outcome: Option<Outcome>,
    },
}

enum Step {
    Next(Token),
    Done(Outcome),
}

enum Release {
    Site(usize),
    Dormant(Cid),
}

fn valid(outcome: &Outcome, when: usize, epoch: usize) -> bool {
    matches!(outcome, Outcome::Value(_)) || when == epoch
}

/// Control-port demand protocol with monotonic bindings and epoch resumption.
#[derive(Clone)]
pub struct Machine {
    program: Program,
    topology: Topology,
    bindings: BTreeMap<String, Scalar>,
    epoch: usize,
    /// Remaining static use sites per program node, materialized or dormant.
    holds: BTreeMap<Cid, usize>,
    cells: BTreeMap<Cid, Cell>,
    fans: Vec<Option<Fan>>,
    merges: Vec<Option<Merge>>,
    sites: Vec<Option<Site>>,
    fan_count: usize,
    merge_count: usize,
    site_count: usize,
    observer: Option<usize>,
    stats: ProbeStats,
    detail: Detail,
}

impl Machine {
    pub fn storage(&self) -> crate::demand::StorageStats {
        let issued_slots = self.fans.len() + self.merges.len() + self.sites.len();
        crate::demand::StorageStats {
            template_nodes: self.program.nodes.len(),
            use_count_entries: self.holds.len(),
            issued_slots,
            vacant_slots: issued_slots - self.fan_count - self.merge_count - self.site_count,
            capacity_slots: self.fans.capacity() + self.merges.capacity() + self.sites.capacity(),
            capacity_bytes: self.fans.capacity() * std::mem::size_of::<Option<Fan>>()
                + self.merges.capacity() * std::mem::size_of::<Option<Merge>>()
                + self.sites.capacity() * std::mem::size_of::<Option<Site>>(),
        }
    }

    pub fn new(program: Program) -> Self {
        Self::with_topology(program, Topology::ReplyOnData)
    }

    pub fn with_topology(program: Program, topology: Topology) -> Self {
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
            topology,
            bindings: BTreeMap::new(),
            epoch: 0,
            holds,
            cells: BTreeMap::new(),
            fans: Vec::new(),
            merges: Vec::new(),
            sites: Vec::new(),
            fan_count: 0,
            merge_count: 0,
            site_count: 0,
            observer: None,
            stats: ProbeStats::default(),
            detail: Detail::default(),
        }
    }

    pub fn topology(&self) -> Topology {
        self.topology
    }

    /// Counters that the shared `ProbeStats` does not separate.
    pub fn detail(&self) -> Detail {
        Detail {
            cells: self.cells.len(),
            fans: self.fan_count,
            merges: self.merge_count,
            sites: self.site_count,
            ..self.detail
        }
    }

    /// Run one observation epoch; `budget` limits transitions in this call.
    /// Conflicting bindings are rejected before any machine state changes.
    /// Budget exhaustion clears the waiting chain; a later run observes
    /// afresh, keeping cached values but no mid-transition token.
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
        let mut token = Token::Call(observer);
        for _ in 0..budget {
            self.stats.transitions += 1;
            self.account(1);
            match self.step(token) {
                Ok(Step::Next(next)) => token = next,
                Ok(Step::Done(outcome)) => {
                    self.account(0);
                    return Ok(ProbeRun {
                        outcome,
                        stats: self.stats.clone(),
                    });
                }
                Err(error) => {
                    self.cancel();
                    return Err(error);
                }
            }
        }
        self.cancel();
        Err(ProbeError::BudgetExhausted { limit: budget })
    }

    /// Check quiescent invariants: no evaluating or waiting cell; every tree
    /// link consistent with its parent; every cell still held by a static
    /// use; `live` equal to a recount.  Diagnostic only; the running machine
    /// never scans globally.
    pub fn audit(&self) -> Result<(), ProbeError> {
        for (cid, cell) in &self.cells {
            if cell.evaluating {
                return Err(ProbeError::Invariant("quiescent cell is evaluating"));
            }
            if !matches!(cell.control, Control::Idle) {
                return Err(ProbeError::Invariant("quiescent cell is waiting"));
            }
            if self.holds.get(cid).copied().unwrap_or(0) == 0 {
                return Err(ProbeError::Invariant("cell survives without a static use"));
            }
            if let Some(root) = cell.data_uses
                && self.up_of(Tree::Data, root)? != Up::Cell(*cid)
            {
                return Err(ProbeError::Invariant(
                    "data tree root does not point at its cell",
                ));
            }
            if let Some(root) = cell.control_uses
                && self.up_of(Tree::Control, root)? != Up::Cell(*cid)
            {
                return Err(ProbeError::Invariant(
                    "control tree root does not point at its cell",
                ));
            }
            for (slot, site) in cell.slots.iter().enumerate() {
                let Some(site) = site else { continue };
                if self.site(*site)?.consumer != Some((*cid, slot)) {
                    return Err(ProbeError::Invariant("slot site belongs to another cell"));
                }
            }
        }
        for (id, fan) in self.fans.iter().enumerate() {
            let Some(fan) = fan else { continue };
            for (index, child) in fan.children.iter().enumerate() {
                let Some(child) = child else { continue };
                let side = if index == 0 { Side::Left } else { Side::Right };
                if self.up_of(Tree::Data, *child)? != Up::Fan(id, side) {
                    return Err(ProbeError::Invariant("fan child does not point back"));
                }
            }
        }
        for (id, merge) in self.merges.iter().enumerate() {
            let Some(merge) = merge else { continue };
            for (index, child) in merge.children.iter().enumerate() {
                let Some(child) = child else { continue };
                let side = if index == 0 { Side::Left } else { Side::Right };
                if self.up_of(Tree::Control, *child)? != Up::Merge(id, side) {
                    return Err(ProbeError::Invariant("merge child does not point back"));
                }
            }
        }
        for site in self.sites.iter().flatten() {
            for tree in [Tree::Data, Tree::Control] {
                let mut up = if tree == Tree::Data {
                    site.data_up
                } else {
                    site.control_up
                };
                let mut hops = 0;
                loop {
                    match up {
                        Up::Cell(cid) => {
                            if cid != site.child {
                                return Err(ProbeError::Invariant(
                                    "site attached to the wrong cell",
                                ));
                            }
                            break;
                        }
                        Up::Fan(fan, _) => up = self.fan(fan)?.up,
                        Up::Merge(merge, _) => up = self.merge(merge)?.up,
                    }
                    hops += 1;
                    if hops > self.fans.len() + self.merges.len() + 1 {
                        return Err(ProbeError::Invariant("cyclic tree"));
                    }
                }
            }
        }
        let recount = self.cells.len() + self.fan_count + self.merge_count + self.site_count;
        if recount != self.stats.live {
            return Err(ProbeError::Invariant("live accounting drifted"));
        }
        Ok(())
    }

    fn account(&mut self, in_flight: usize) {
        self.stats.live =
            self.cells.len() + self.fan_count + self.merge_count + self.site_count + in_flight;
        self.stats.peak_live = self.stats.peak_live.max(self.stats.live);
    }

    fn observer(&mut self) -> Result<usize, ProbeError> {
        if let Some(site) = self.observer {
            return Ok(site);
        }
        let root = self.program.root;
        let site = self.new_site(Site {
            child: root,
            consumer: None,
            data_up: Up::Cell(root),
            control_up: Up::Cell(root),
            parked: None,
        });
        self.attach(root, site)?;
        self.observer = Some(site);
        Ok(site)
    }

    /// Host-side recovery after an aborted run: follow the chain of waiting
    /// cells from the root, clearing each one.  Merges hold no state, so
    /// nothing else needs clearing.  Only that chain is visited.
    fn cancel(&mut self) {
        let mut cid = self.program.root;
        loop {
            let Some(cell) = self.cells.get_mut(&cid) else {
                break;
            };
            cell.evaluating = false;
            let Control::Waiting { slot, .. } = std::mem::take(&mut cell.control) else {
                break;
            };
            let Some(site) = cell.slots.get(slot).copied().flatten() else {
                break;
            };
            let Some(next) = self.sites[site].as_ref().map(|site| site.child) else {
                break;
            };
            cid = next;
        }
        self.account(0);
    }

    fn step(&mut self, token: Token) -> Result<Step, ProbeError> {
        match token {
            Token::Call(site) => self.call(site),
            Token::Climb { at, from } => match self.up_of(Tree::Control, at)? {
                Up::Cell(cid) => Ok(Step::Next(Token::AtCell { cid, from })),
                Up::Merge(merge, _) => {
                    self.stats.routing_steps += 1;
                    self.detail.control_hops += 1;
                    Ok(Step::Next(Token::Climb {
                        at: TreeRef::Merge(merge),
                        from,
                    }))
                }
                Up::Fan(..) => Err(ProbeError::Invariant("call entered a data fan")),
            },
            Token::AtCell { cid, from } => self.at_cell(cid, from),
            Token::Distribute {
                mut pending,
                outcome,
                return_to,
            } => {
                let epoch = self.epoch;
                match pending.pop() {
                    Some(TreeRef::Fan(fan)) => {
                        self.stats.routing_steps += 1;
                        self.detail.data_hops += 1;
                        let agent = self.fan(fan)?;
                        pending.extend(agent.children.iter().flatten().copied());
                    }
                    Some(TreeRef::Site(site)) => {
                        self.detail.data_copies += 1;
                        self.site_mut(site)?.parked = Some((outcome.clone(), epoch));
                    }
                    Some(TreeRef::Merge(_)) => {
                        return Err(ProbeError::Invariant("value entered a control merge"));
                    }
                    None => {
                        return Ok(Step::Next(Token::Resume {
                            site: return_to,
                            outcome: None,
                        }));
                    }
                }
                Ok(Step::Next(Token::Distribute {
                    pending,
                    outcome,
                    return_to,
                }))
            }
            Token::Resume { site, outcome } => {
                let epoch = self.epoch;
                let topology = self.topology;
                let (outcome, consumer, late_copy) = {
                    let agent = self.site_mut(site)?;
                    let mut late_copy = false;
                    let outcome = match outcome {
                        Some(outcome) => {
                            if topology == Topology::ReplyOnData {
                                // A late copy: the site missed the broadcast.
                                agent.parked = Some((outcome.clone(), epoch));
                                late_copy = true;
                            }
                            outcome
                        }
                        None => match &agent.parked {
                            Some((outcome, when)) if valid(outcome, *when, epoch) => {
                                outcome.clone()
                            }
                            _ => {
                                return Err(ProbeError::Invariant(
                                    "return reached a site without its copy",
                                ));
                            }
                        },
                    };
                    (outcome, agent.consumer, late_copy)
                };
                if late_copy {
                    self.detail.data_copies += 1;
                }
                match consumer {
                    None => Ok(Step::Done(outcome)),
                    Some((parent, slot)) => self.resume(parent, slot, outcome),
                }
            }
        }
    }

    /// A consumer demands an operand.  With reply-on-data a valid parked copy
    /// answers without any control traffic; otherwise the call enters the used
    /// cell's control tree, carrying this site as its return address.
    fn call(&mut self, site: usize) -> Result<Step, ProbeError> {
        self.stats.requests += 1;
        let epoch = self.epoch;
        if self.topology == Topology::ReplyOnData
            && let Some((outcome, when)) = &self.site(site)?.parked
            && valid(outcome, *when, epoch)
        {
            self.stats.memo_hits += 1;
            self.detail.site_hits += 1;
            return Ok(Step::Next(Token::Resume {
                site,
                outcome: None,
            }));
        }
        Ok(Step::Next(Token::Climb {
            at: TreeRef::Site(site),
            from: site,
        }))
    }

    fn at_cell(&mut self, cid: Cid, from: usize) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get(&cid)
            .ok_or(ProbeError::Invariant("call reached a missing cell"))?;
        if cell.evaluating {
            return Err(ProbeError::Invariant("cyclic call"));
        }
        if let Some((outcome, when)) = &cell.result
            && valid(outcome, *when, epoch)
        {
            let outcome = outcome.clone();
            self.stats.memo_hits += 1;
            self.detail.cell_hits += 1;
            return Ok(Step::Next(Token::Resume {
                site: from,
                outcome: Some(outcome),
            }));
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
            Expr::Value(value) => self.complete(cid, Outcome::Value(value), from),
            Expr::Hole(name) => {
                let outcome = self
                    .bindings
                    .get(&name)
                    .cloned()
                    .map_or(Outcome::Unknown, Outcome::Value);
                self.complete(cid, outcome, from)
            }
            Expr::Add(_, _) | Expr::Mul(_, _) => self.demand(
                cid,
                0,
                Phase::Left {
                    multiply: matches!(expr, Expr::Mul(_, _)),
                },
                from,
            ),
            Expr::If { .. } => self.demand(cid, 0, Phase::Condition, from),
        }
    }

    /// Instantiate a node's operand use sites in both of each operand's trees.
    /// Template-sized work, charged as part of one transition.
    fn materialize(&mut self, cid: Cid) -> Result<(), ProbeError> {
        let children = self
            .program
            .nodes
            .get(&cid)
            .ok_or(ProbeError::MissingNode(cid))?
            .children();
        let mut slots = Vec::with_capacity(children.len());
        for (slot, child) in children.into_iter().enumerate() {
            let site = self.new_site(Site {
                child,
                consumer: Some((cid, slot)),
                data_up: Up::Cell(child),
                control_up: Up::Cell(child),
                parked: None,
            });
            self.attach(child, site)?;
            slots.push(Some(site));
        }
        let cell = self
            .cells
            .get_mut(&cid)
            .ok_or(ProbeError::Invariant("materializing a missing cell"))?;
        cell.slots = slots;
        cell.materialized = true;
        Ok(())
    }

    /// Add a use site to both trees of the used node, creating its pending
    /// cell if this is its first materialized consumer.  Trees grow as chains:
    /// a new fan or merge takes the old tree on its left and the site on its
    /// right.  With reply-on-data, a site attached to a cell that already has a
    /// valid value receives its copy at once.
    fn attach(&mut self, child: Cid, site: usize) -> Result<(), ProbeError> {
        let epoch = self.epoch;
        let (old_data, old_control, copy) = {
            let cell = self.cells.entry(child).or_default();
            let copy = cell
                .result
                .as_ref()
                .filter(|(outcome, when)| valid(outcome, *when, epoch))
                .map(|(outcome, _)| outcome.clone());
            (cell.data_uses, cell.control_uses, copy)
        };
        match old_data {
            None => {
                self.cells.get_mut(&child).expect("cell exists").data_uses =
                    Some(TreeRef::Site(site));
                self.site_mut(site)?.data_up = Up::Cell(child);
            }
            Some(old) => {
                let fan = self.new_fan(Fan {
                    up: Up::Cell(child),
                    children: [Some(old), Some(TreeRef::Site(site))],
                });
                self.set_up(Tree::Data, old, Up::Fan(fan, Side::Left))?;
                self.site_mut(site)?.data_up = Up::Fan(fan, Side::Right);
                self.cells.get_mut(&child).expect("cell exists").data_uses =
                    Some(TreeRef::Fan(fan));
            }
        }
        match old_control {
            None => {
                self.cells
                    .get_mut(&child)
                    .expect("cell exists")
                    .control_uses = Some(TreeRef::Site(site));
                self.site_mut(site)?.control_up = Up::Cell(child);
            }
            Some(old) => {
                let merge = self.new_merge(Merge {
                    up: Up::Cell(child),
                    children: [Some(old), Some(TreeRef::Site(site))],
                });
                self.set_up(Tree::Control, old, Up::Merge(merge, Side::Left))?;
                self.site_mut(site)?.control_up = Up::Merge(merge, Side::Right);
                self.cells
                    .get_mut(&child)
                    .expect("cell exists")
                    .control_uses = Some(TreeRef::Merge(merge));
            }
        }
        if self.topology == Topology::ReplyOnData
            && let Some(outcome) = copy
        {
            self.site_mut(site)?.parked = Some((outcome, epoch));
            self.detail.data_copies += 1;
        }
        Ok(())
    }

    /// A cell demands one of its operands: record what it waits for and where
    /// its own caller waits, then send the token to that operand's use site.
    fn demand(
        &mut self,
        cid: Cid,
        slot: usize,
        phase: Phase,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let cell = self
            .cells
            .get_mut(&cid)
            .ok_or(ProbeError::Invariant("demand from a missing cell"))?;
        let site = cell
            .slots
            .get(slot)
            .copied()
            .flatten()
            .ok_or(ProbeError::Invariant("demand through an erased use site"))?;
        cell.control = Control::Waiting {
            slot,
            phase,
            return_to,
        };
        Ok(Step::Next(Token::Call(site)))
    }

    fn complete(
        &mut self,
        cid: Cid,
        outcome: Outcome,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get_mut(&cid)
            .ok_or(ProbeError::Invariant("completing a missing cell"))?;
        cell.evaluating = false;
        cell.control = Control::Idle;
        cell.result = Some((outcome.clone(), epoch));
        let data_root = cell.data_uses;
        if matches!(outcome, Outcome::Value(_)) {
            // A ground value no longer needs its operands: last-consumer release.
            let slots: Vec<usize> = cell.slots.iter_mut().filter_map(Option::take).collect();
            self.drain(slots.into_iter().map(Release::Site).collect())?;
        }
        match self.topology {
            Topology::ReplyOnData => Ok(Step::Next(Token::Distribute {
                pending: data_root.into_iter().collect(),
                outcome,
                return_to,
            })),
            Topology::ReplyOnControl => Ok(Step::Next(Token::Resume {
                site: return_to,
                outcome: Some(outcome),
            })),
        }
    }

    fn resume(&mut self, parent: Cid, slot: usize, outcome: Outcome) -> Result<Step, ProbeError> {
        let cell = self
            .cells
            .get_mut(&parent)
            .ok_or(ProbeError::Invariant("resume at a missing cell"))?;
        let Control::Waiting {
            slot: waiting_slot,
            phase,
            return_to,
        } = std::mem::take(&mut cell.control)
        else {
            return Err(ProbeError::Invariant(
                "resume at a cell that was not waiting",
            ));
        };
        if waiting_slot != slot {
            return Err(ProbeError::Invariant("return reached the wrong operand"));
        }
        match phase {
            Phase::Left { multiply } => {
                if matches!(outcome, Outcome::Error(_)) {
                    return self.complete(parent, outcome, return_to);
                }
                self.demand(
                    parent,
                    1,
                    Phase::Right {
                        multiply,
                        left: outcome,
                    },
                    return_to,
                )
            }
            Phase::Right { multiply, left } => {
                self.complete(parent, combine(left, outcome, multiply), return_to)
            }
            Phase::Condition => match outcome {
                Outcome::Value(Scalar::Bool(chosen)) => {
                    let (selected, rejected) = if chosen { (1, 2) } else { (2, 1) };
                    self.erase_use(parent, rejected)?;
                    self.demand(parent, selected, Phase::Branch, return_to)
                }
                Outcome::Value(_) => self.complete(
                    parent,
                    Outcome::Error(Fault::Type("if condition is not a boolean")),
                    return_to,
                ),
                other => self.complete(parent, other, return_to),
            },
            Phase::Branch => self.complete(parent, outcome, return_to),
        }
    }

    fn erase_use(&mut self, parent: Cid, slot: usize) -> Result<(), ProbeError> {
        let site = self
            .cells
            .get_mut(&parent)
            .and_then(|cell| cell.slots.get_mut(slot))
            .and_then(Option::take);
        match site {
            Some(site) => self.drain(vec![Release::Site(site)]),
            None => Ok(()),
        }
    }

    /// Release use sites (S3).  Each item releases one static use of a node;
    /// when a node's last use goes, its cell is dropped and what it held is
    /// released in turn: through its sites if it was materialized, otherwise
    /// through its static operands (the dormant interior).
    fn drain(&mut self, mut work: Vec<Release>) -> Result<(), ProbeError> {
        while let Some(item) = work.pop() {
            let child = match item {
                Release::Site(site) => self.detach_site(site)?,
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
            let dormant = || {
                self.program
                    .nodes
                    .get(&child)
                    .map(Expr::children)
                    .unwrap_or_default()
                    .into_iter()
                    .map(Release::Dormant)
            };
            match self.cells.remove(&child) {
                Some(cell) if cell.evaluating || !matches!(cell.control, Control::Idle) => {
                    return Err(ProbeError::Invariant(
                        "released a cell during its evaluation",
                    ));
                }
                Some(cell) if cell.materialized => {
                    work.extend(cell.slots.into_iter().flatten().map(Release::Site));
                }
                _ => work.extend(dormant()),
            }
        }
        Ok(())
    }

    /// Remove a site from both trees of the cell it uses, collapsing fans and
    /// merges left with one side.  Returns the used node.
    fn detach_site(&mut self, site: usize) -> Result<Cid, ProbeError> {
        let agent = self
            .sites
            .get_mut(site)
            .and_then(Option::take)
            .ok_or(ProbeError::Invariant("detached a missing site"))?;
        self.site_count -= 1;
        self.detail.teardown_steps += 1;
        for tree in [Tree::Data, Tree::Control] {
            let mut up = if tree == Tree::Data {
                agent.data_up
            } else {
                agent.control_up
            };
            loop {
                self.detail.teardown_steps += 1;
                match up {
                    Up::Cell(cid) => {
                        if let Some(cell) = self.cells.get_mut(&cid) {
                            match tree {
                                Tree::Data => cell.data_uses = None,
                                Tree::Control => cell.control_uses = None,
                            }
                        }
                        break;
                    }
                    Up::Fan(node, side) if tree == Tree::Data => {
                        let children = &mut self.fan_mut(node)?.children;
                        children[side.index()] = None;
                        let sibling = children[side.other().index()];
                        let above = self.fan(node)?.up;
                        self.fans[node] = None;
                        self.fan_count -= 1;
                        match sibling {
                            None => up = above,
                            Some(sibling) => {
                                self.set_up(tree, sibling, above)?;
                                self.replace_child(tree, above, TreeRef::Fan(node), sibling)?;
                                break;
                            }
                        }
                    }
                    Up::Merge(node, side) if tree == Tree::Control => {
                        let children = &mut self.merge_mut(node)?.children;
                        children[side.index()] = None;
                        let sibling = children[side.other().index()];
                        let above = self.merge(node)?.up;
                        self.merges[node] = None;
                        self.merge_count -= 1;
                        match sibling {
                            None => up = above,
                            Some(sibling) => {
                                self.set_up(tree, sibling, above)?;
                                self.replace_child(tree, above, TreeRef::Merge(node), sibling)?;
                                break;
                            }
                        }
                    }
                    _ => return Err(ProbeError::Invariant("tree node in the wrong tree")),
                }
            }
        }
        Ok(agent.child)
    }

    fn new_site(&mut self, site: Site) -> usize {
        self.sites.push(Some(site));
        self.site_count += 1;
        self.sites.len() - 1
    }

    fn new_fan(&mut self, fan: Fan) -> usize {
        self.fans.push(Some(fan));
        self.fan_count += 1;
        self.fans.len() - 1
    }

    fn new_merge(&mut self, merge: Merge) -> usize {
        self.merges.push(Some(merge));
        self.merge_count += 1;
        self.merges.len() - 1
    }

    fn site(&self, id: usize) -> Result<&Site, ProbeError> {
        self.sites
            .get(id)
            .and_then(Option::as_ref)
            .ok_or(ProbeError::Invariant("missing site agent"))
    }

    fn site_mut(&mut self, id: usize) -> Result<&mut Site, ProbeError> {
        self.sites
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or(ProbeError::Invariant("missing site agent"))
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

    fn merge(&self, id: usize) -> Result<&Merge, ProbeError> {
        self.merges
            .get(id)
            .and_then(Option::as_ref)
            .ok_or(ProbeError::Invariant("missing merge agent"))
    }

    fn merge_mut(&mut self, id: usize) -> Result<&mut Merge, ProbeError> {
        self.merges
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or(ProbeError::Invariant("missing merge agent"))
    }

    fn up_of(&self, tree: Tree, at: TreeRef) -> Result<Up, ProbeError> {
        match (tree, at) {
            (Tree::Data, TreeRef::Site(site)) => Ok(self.site(site)?.data_up),
            (Tree::Control, TreeRef::Site(site)) => Ok(self.site(site)?.control_up),
            (Tree::Data, TreeRef::Fan(fan)) => Ok(self.fan(fan)?.up),
            (Tree::Control, TreeRef::Merge(merge)) => Ok(self.merge(merge)?.up),
            _ => Err(ProbeError::Invariant("tree node in the wrong tree")),
        }
    }

    fn set_up(&mut self, tree: Tree, at: TreeRef, up: Up) -> Result<(), ProbeError> {
        match (tree, at) {
            (Tree::Data, TreeRef::Site(site)) => self.site_mut(site)?.data_up = up,
            (Tree::Control, TreeRef::Site(site)) => self.site_mut(site)?.control_up = up,
            (Tree::Data, TreeRef::Fan(fan)) => self.fan_mut(fan)?.up = up,
            (Tree::Control, TreeRef::Merge(merge)) => self.merge_mut(merge)?.up = up,
            _ => return Err(ProbeError::Invariant("tree node in the wrong tree")),
        }
        Ok(())
    }

    fn replace_child(
        &mut self,
        tree: Tree,
        up: Up,
        old: TreeRef,
        new: TreeRef,
    ) -> Result<(), ProbeError> {
        let current = match (tree, up) {
            (Tree::Data, Up::Cell(cid)) => {
                &mut self
                    .cells
                    .get_mut(&cid)
                    .ok_or(ProbeError::Invariant("tree root of a missing cell"))?
                    .data_uses
            }
            (Tree::Control, Up::Cell(cid)) => {
                &mut self
                    .cells
                    .get_mut(&cid)
                    .ok_or(ProbeError::Invariant("tree root of a missing cell"))?
                    .control_uses
            }
            (Tree::Data, Up::Fan(fan, side)) => &mut self.fan_mut(fan)?.children[side.index()],
            (Tree::Control, Up::Merge(merge, side)) => {
                &mut self.merge_mut(merge)?.children[side.index()]
            }
            _ => return Err(ProbeError::Invariant("parent in the wrong tree")),
        };
        if *current != Some(old) {
            return Err(ProbeError::Invariant(
                "parent does not hold the collapsed node",
            ));
        }
        *current = Some(new);
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
    const TOPOLOGIES: [Topology; 2] = [Topology::ReplyOnData, Topology::ReplyOnControl];
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

    fn machine(store: &Store, root: Cid, topology: Topology) -> Machine {
        Machine::with_topology(Program::from_store(store, root).unwrap(), topology)
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
    fn trace_1_either_branch_may_be_the_first_consumer_under_both_topologies() {
        for topology in TOPOLOGIES {
            for (flag, expected, evaluated) in [(true, 85, 0), (false, 126, 1)] {
                let mut store = Store::new();
                let (root, shared, yes, no) = dynamic_first_consumer(&mut store);
                let mut machine = machine(&store, root, topology);
                let run = machine
                    .run(&facts(&[("flag", Scalar::Bool(flag))]), BUDGET)
                    .unwrap();
                assert_eq!(run.outcome, Outcome::Value(Scalar::Int(expected)));
                assert_eq!(count(&run, shared), 1);
                assert_eq!(count(&run, [yes, no][evaluated]), 1);
                assert_eq!(count(&run, [no, yes][evaluated]), 0);
                assert!(run.stats.memo_hits > 0);
                // Last-consumer reclamation: the root value and its observer.
                assert_eq!(run.stats.live, 2, "{topology:?}");
                machine.audit().unwrap();
                let detail = machine.detail();
                match topology {
                    // The outer use of `x` is answered by its parked copy.
                    Topology::ReplyOnData => assert!(detail.site_hits >= 1, "{detail:?}"),
                    Topology::ReplyOnControl => {
                        assert_eq!(detail.data_copies, 0);
                        assert!(detail.cell_hits >= 1, "{detail:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn trace_1_with_the_condition_arriving_in_a_later_epoch() {
        for topology in TOPOLOGIES {
            for (flag, expected) in [(true, 85), (false, 126)] {
                let mut store = Store::new();
                let (root, shared, ..) = dynamic_first_consumer(&mut store);
                let mut machine = machine(&store, root, topology);
                let pending = machine.run(&Facts::new(), BUDGET).unwrap();
                assert_eq!(pending.outcome, Outcome::Unknown);
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
    }

    #[test]
    fn trace_4_erasing_every_consumer_never_materializes_the_shared_work() {
        for topology in TOPOLOGIES {
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
            let mut machine = machine(&store, root, topology);
            let run = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(run.outcome, Outcome::Value(Scalar::Int(3)));
            assert_eq!(count(&run, bad), 0);
            assert_eq!(count(&run, max), 0);
            assert!(run.stats.erasures >= 2);
            assert_eq!(run.stats.live, 2);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn trace_5_blocked_shared_work_resumes_without_recomputing_its_static_island() {
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let y = hole(&mut store, "y");
            let six = int(&mut store, 6);
            let seven = int(&mut store, 7);
            let island = store.intern(Node::Mul(six, seven));
            let shared = store.intern(Node::Add(y, island));
            let root = store.intern(Node::Add(shared, shared));
            let mut machine = machine(&store, root, topology);
            let first = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(first.outcome, Outcome::Unknown);
            assert_eq!(count(&first, shared), 1);
            assert_eq!(count(&first, island), 1);
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
    }

    #[test]
    fn nested_sharing_evaluates_every_node_once_and_reclaims_as_it_goes() {
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let mut root = int(&mut store, 1);
            let mut nodes = vec![root];
            for _ in 0..30 {
                root = store.intern(Node::Add(root, root));
                nodes.push(root);
            }
            let mut machine = machine(&store, root, topology);
            let run = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(run.outcome, Outcome::Value(Scalar::Int(1 << 30)));
            for node in &nodes {
                assert_eq!(count(&run, *node), 1);
            }
            assert_eq!(run.stats.evaluations, nodes.len());
            assert!(run.stats.transitions < 1_500, "{}", run.stats.transitions);
            // Linear in depth, not exponential in sharing.
            assert!(run.stats.peak_live < 250, "{}", run.stats.peak_live);
            assert_eq!(run.stats.live, 2);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn wide_sharing_evaluates_once_and_routes_through_the_merge_chain() {
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let six = int(&mut store, 6);
            let seven = int(&mut store, 7);
            let shared = store.intern(Node::Mul(six, seven));
            let mut root = shared;
            for _ in 0..31 {
                root = store.intern(Node::Add(shared, root));
            }
            let mut machine = machine(&store, root, topology);
            let run = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(run.outcome, Outcome::Value(Scalar::Int(42 * 32)));
            assert_eq!(count(&run, shared), 1);
            assert_eq!(run.stats.memo_hits, 31);
            let detail = machine.detail();
            match topology {
                // Thirty-one site hits with no control traffic at all.  Every
                // completion copies to its consumers: 32 copies of the shared
                // node (one broadcast, 31 on attach), one per inner addition
                // (30), the root to its observer (1), and the constants (2).
                Topology::ReplyOnData => {
                    assert_eq!(detail.site_hits, 31);
                    assert_eq!(detail.control_hops, 0);
                    assert_eq!(detail.data_copies, 65);
                }
                // Every later consumer calls while its site is still the
                // newest leaf of the merge chain (one hop), except the
                // innermost `Add(shared, shared)`, which attaches both of its
                // sites before calling: its left site is one level deeper.
                // Replies come straight back and cost nothing.
                Topology::ReplyOnControl => {
                    assert_eq!(detail.cell_hits, 31);
                    assert_eq!(detail.control_hops, 32);
                }
            }
            assert_eq!(run.stats.live, 2);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn a_dormant_consumer_keeps_a_value_alive_until_it_is_released() {
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let flag = hole(&mut store, "flag");
            let six = int(&mut store, 6);
            let seven = int(&mut store, 7);
            let zero = int(&mut store, 0);
            let shared = store.intern(Node::Mul(six, seven));
            let branch = store.intern(Node::If {
                condition: flag,
                when_true: shared,
                when_false: zero,
            });
            let root = store.intern(Node::Add(shared, branch));
            let mut machine = machine(&store, root, topology);
            let pending = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(pending.outcome, Outcome::Unknown);
            assert_eq!(count(&pending, shared), 1);
            // `shared` was evaluated for the outer use and is still held by
            // the dormant `then` branch: it must survive, not be recomputed.
            assert!(machine.cells.contains_key(&shared));
            let done = machine
                .run(&facts(&[("flag", Scalar::Bool(true))]), BUDGET)
                .unwrap();
            assert_eq!(done.outcome, Outcome::Value(Scalar::Int(84)));
            assert_eq!(count(&done, shared), 1);
            assert_eq!(done.stats.live, 2);
            machine.audit().unwrap();
        }
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
            for topology in TOPOLOGIES {
                let mut machine = machine(&store, root, topology);
                let run = machine.run(&Facts::new(), BUDGET).unwrap();
                assert_eq!(
                    run.outcome,
                    expected,
                    "{topology:?}: {}",
                    store.format(root)
                );
                machine.audit().unwrap();
            }
        }
    }

    #[test]
    fn errors_are_epoch_local_but_values_persist() {
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let x = hole(&mut store, "x");
            let two = int(&mut store, 2);
            let truth = boolean(&mut store, true);
            let product = store.intern(Node::Mul(x, two));
            let root = store.intern(Node::Add(product, truth));
            let mut machine = machine(&store, root, topology);
            let first = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(
                first.outcome,
                Outcome::Error(Fault::Type("add expects two integers"))
            );
            let second = machine
                .run(&facts(&[("x", Scalar::Int(i64::MAX))]), BUDGET)
                .unwrap();
            assert_eq!(second.outcome, Outcome::Error(Fault::Overflow("multiply")));
            assert_eq!(count(&second, two), 1);
            let third = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(third.outcome, second.outcome);
            machine.audit().unwrap();
        }
    }

    #[test]
    fn conflicting_bindings_are_rejected_before_any_state_changes() {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let a = hole(&mut store, "a");
        let root = store.intern(Node::Add(a, x));
        let mut machine = machine(&store, root, Topology::ReplyOnData);
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
        for topology in TOPOLOGIES {
            let mut store = Store::new();
            let (root, ..) = dynamic_first_consumer(&mut store);
            let bindings = facts(&[("flag", Scalar::Bool(false))]);
            let full = machine(&store, root, topology)
                .run(&bindings, BUDGET)
                .unwrap();
            for budget in 0..=full.stats.transitions {
                let mut machine = machine(&store, root, topology);
                match machine.run(&bindings, budget) {
                    Ok(run) => assert_eq!(run.outcome, Outcome::Value(Scalar::Int(126))),
                    Err(ProbeError::BudgetExhausted { limit }) => assert_eq!(limit, budget),
                    Err(other) => panic!("{topology:?} budget {budget}: {other:?}"),
                }
                machine
                    .audit()
                    .unwrap_or_else(|error| panic!("{topology:?} budget {budget}: {error}"));
                let done = machine.run(&Facts::new(), BUDGET).unwrap();
                assert_eq!(
                    done.outcome,
                    Outcome::Value(Scalar::Int(126)),
                    "{topology:?} budget {budget}"
                );
                machine.audit().unwrap();
            }
        }
    }

    fn random(seed: &mut u64) -> usize {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 32) as usize
    }

    #[test]
    fn generated_dags_match_the_reference_directly_and_in_staged_epochs() {
        let mut seed = 0x636f_6e74_726f_6c5f_u64;
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
            for topology in TOPOLOGIES {
                for stages in [vec![x.clone(), flag.clone()], vec![flag.clone(), x.clone()]] {
                    let mut machine = machine(&store, root, topology);
                    let mut accumulated = Facts::new();
                    let direct = machine.run(&Facts::new(), BUDGET).unwrap();
                    assert_eq!(
                        direct.outcome,
                        reference(&mut store, root, &accumulated),
                        "case {case} {topology:?}: {}",
                        store.format(root)
                    );
                    machine.audit().unwrap();
                    for stage in stages {
                        accumulated.insert(stage.0.into(), stage.1.clone());
                        let expected = reference(&mut store, root, &accumulated);
                        let run = machine.run(&facts(&[stage]), BUDGET).unwrap();
                        assert_eq!(
                            run.outcome,
                            expected,
                            "case {case} {topology:?}: {}",
                            store.format(root)
                        );
                        machine.audit().unwrap();
                        for (node, evaluations) in &run.stats.node_evaluations {
                            assert!(*evaluations <= machine.epoch, "{}", store.format(*node));
                        }
                    }
                }
            }
        }
    }

    /// Side-by-side numbers for the three probes.  Semantics are asserted;
    /// costs are printed (run with `--nocapture`).
    #[test]
    fn compare_a_b_and_c_on_sharing_workloads() {
        use crate::inet_demand_a::Machine as A;
        use crate::inet_demand_b::Machine as B;

        fn workloads() -> Vec<(&'static str, Store, Cid, Facts, Outcome)> {
            let mut out = Vec::new();
            {
                let mut store = Store::new();
                let mut root = int(&mut store, 1);
                for _ in 0..30 {
                    root = store.intern(Node::Add(root, root));
                }
                out.push((
                    "nested sharing, depth 30",
                    store,
                    root,
                    Facts::new(),
                    Outcome::Value(Scalar::Int(1 << 30)),
                ));
            }
            {
                let mut store = Store::new();
                let six = int(&mut store, 6);
                let seven = int(&mut store, 7);
                let shared = store.intern(Node::Mul(six, seven));
                let mut root = shared;
                for _ in 0..31 {
                    root = store.intern(Node::Add(shared, root));
                }
                out.push((
                    "wide sharing, 32 sites",
                    store,
                    root,
                    Facts::new(),
                    Outcome::Value(Scalar::Int(42 * 32)),
                ));
            }
            {
                let mut store = Store::new();
                let (root, ..) = dynamic_first_consumer(&mut store);
                out.push((
                    "dynamic first consumer",
                    store,
                    root,
                    facts(&[("flag", Scalar::Bool(false))]),
                    Outcome::Value(Scalar::Int(126)),
                ));
            }
            out
        }

        for (name, store, root, bindings, expected) in workloads() {
            let program = Program::from_store(&store, root).unwrap();
            let mut a = A::new(program.clone());
            let mut b = B::new(program.clone());
            let mut c_data = Machine::with_topology(program.clone(), Topology::ReplyOnData);
            let mut c_control = Machine::with_topology(program, Topology::ReplyOnControl);
            let runs = [
                ("A", a.run(&bindings, BUDGET).unwrap()),
                ("B", b.run(&bindings, BUDGET).unwrap()),
                ("C/data", c_data.run(&bindings, BUDGET).unwrap()),
                ("C/control", c_control.run(&bindings, BUDGET).unwrap()),
            ];
            eprintln!("{name}");
            for (label, run) in &runs {
                assert_eq!(run.outcome, expected, "{name}: {label}");
                assert!(
                    run.stats.node_evaluations.values().all(|n| *n == 1),
                    "{name}: {label}"
                );
                eprintln!(
                    "  {label:<10} transitions={:<5} routing={:<4} memo_hits={:<3} peak_live={:<4} final_live={}",
                    run.stats.transitions,
                    run.stats.routing_steps,
                    run.stats.memo_hits,
                    run.stats.peak_live,
                    run.stats.live
                );
            }
            eprintln!(
                "  C/data detail    {:?}\n  C/control detail {:?}",
                c_data.detail(),
                c_control.detail()
            );
        }
    }
}
