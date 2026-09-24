//! N0a: closed quotations and exact-arity application as demand-driven agents.
//!
//! This is a scalar probe, not the port-level backend, and it claims nothing
//! about confluence or parallelism.  It extends the revised variant C
//! (`inet_demand_c`, reply-on-control only: values ride back with the token,
//! there are no data fans) with three things and nothing else:
//!
//! - **Code values.** A `Quote` node is a value: demanding it yields
//!   `Code(cid)`, a reference to closed code.  Copying a code value copies the
//!   reference; nothing ever walks into a template.
//! - **Instances.** A cell is keyed by `(instance, template node)`.  An `Apply`
//!   cell demands its function operand; a `Code` value of matching arity
//!   creates one instance of that template for this cell (kept across epochs
//!   and never re-created on retry), holds one use of the instance's root
//!   cell, and demands it.  Two application nodes of the same code get two
//!   instances with independent wiring; one application node demanded twice
//!   is one cell and one evaluation. Arguments shared in a caller instance
//!   are evaluated once per epoch. ACCEPTANCE GAP: instantiated body nodes
//!   that coalesce under reference substitution are not globally shared here,
//!   either across calls or within one body. Tests characterize this mismatch;
//!   this prototype does not pass March's full shared-evaluation requirement.
//! - **Parameters as proxies.** A `Param` cell in an instance, when
//!   materialized, attaches a use site to the caller's argument node and
//!   forwards demand to it.  That attachment is runtime wiring: the argument's
//!   use count goes up by one then, and comes down when the instance dies.
//!   An argument no parameter demands is never evaluated.
//!
//! Use counts are therefore static per template (one per operand edge,
//! computed once per template body and cached) plus dynamic increments for the
//! observation, each instance root, and each proxy.  A node is dropped when
//! its count reaches zero, releasing what it held: its sites if materialized,
//! its static operands if dormant.
//!
//! The token carries its return address: the use site to resume.  That site
//! is a stored reference, dereferenced directly by the host; resuming it is
//! one counted transition, not proof of a local port-rewrite encoding.
//! Scheduling constraints are those of C: one token (S1), release cascades run
//! to completion (S3), nothing fires ahead of the token (S4); there is no
//! distribution step because values return with the token.
//!
//! Accounting: `live` = cells + merges + sites + the in-flight token.
//! `routing_steps` counts merge hops on calls.  `memo_hits` counts cell-level
//! hits.  `node_evaluations` is keyed by template node and sums over
//! instances, so a template evaluated in two instances counts twice; the
//! detail counters give instances created, template walks (use counting,
//! charged as template-sized work), proxy sites, and teardown steps.
//! `evaluations` counts evaluations started, as in B and C.
//!
//! Not in this slice: `Intern` (N0b), effects, forking, ahead-of-token firing,
//! a persisted paused-net codec, and any node kind outside the subset.
//! Logical deletion retains array slots, instance records, argument vectors,
//! and use-count maps; `storage()` exposes these. Structural groundness walks
//! preserve the reference's stuck-term type checks without evaluating branches,
//! but run outside transition fuel just like template census and cleanup.

use crate::cid::Cid;
use crate::demand::{ProbeError, ProbeStats, Scalar};
use crate::{Atom, Node, Store};
use std::collections::{BTreeMap, BTreeSet};

/// The N0 subset: scalars, holes, arithmetic, `If`, closed quotations,
/// parameters, and exact-arity application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Value(Scalar),
    Hole(String),
    Add(Cid, Cid),
    Mul(Cid, Cid),
    If {
        condition: Cid,
        when_true: Cid,
        when_false: Cid,
    },
    Quote {
        params: u16,
        body: Cid,
    },
    Param(u16),
    Apply {
        function: Cid,
        arguments: Vec<Cid>,
    },
}

impl Expr {
    /// Operand edges inside the same template.  A quotation is a value; its
    /// body is a separate template, not an operand.
    pub fn operands(&self) -> Vec<Cid> {
        match self {
            Self::Value(_) | Self::Hole(_) | Self::Param(_) | Self::Quote { .. } => vec![],
            Self::Add(a, b) | Self::Mul(a, b) => vec![*a, *b],
            Self::If {
                condition,
                when_true,
                when_false,
            } => vec![*condition, *when_true, *when_false],
            Self::Apply {
                function,
                arguments,
            } => std::iter::once(*function)
                .chain(arguments.iter().copied())
                .collect(),
        }
    }
}

/// An immutable program DAG including every template body reachable from the
/// root.  Machines keep their own instances and demand state.
#[derive(Clone, Debug)]
pub struct Program {
    pub root: Cid,
    pub nodes: BTreeMap<Cid, Expr>,
}

impl Program {
    /// Lower without evaluating.  Unsupported nodes are rejected wherever they
    /// occur, including inside dormant branches and quotation bodies; every
    /// quotation must be closed with its parameters in range.
    pub fn from_store(store: &Store, root: Cid) -> Result<Self, ProbeError> {
        let mut nodes = BTreeMap::new();
        let mut pending = vec![root];
        while let Some(cid) = pending.pop() {
            if nodes.contains_key(&cid) {
                continue;
            }
            let expr = match store.get(cid).ok_or(ProbeError::MissingNode(cid))? {
                Node::Const(Atom::Int(n)) => Expr::Value(Scalar::Int(*n)),
                Node::Const(Atom::Bool(b)) => Expr::Value(Scalar::Bool(*b)),
                Node::Hole(name) => Expr::Hole(name.clone()),
                Node::Add(a, b) => Expr::Add(*a, *b),
                Node::Mul(a, b) => Expr::Mul(*a, *b),
                Node::If {
                    condition,
                    when_true,
                    when_false,
                } => Expr::If {
                    condition: *condition,
                    when_true: *when_true,
                    when_false: *when_false,
                },
                Node::Quote { params, body } => Expr::Quote {
                    params: *params,
                    body: *body,
                },
                Node::Param(index) => Expr::Param(*index),
                Node::Apply {
                    function,
                    arguments,
                } => Expr::Apply {
                    function: *function,
                    arguments: arguments.clone(),
                },
                _ => {
                    return Err(ProbeError::Unsupported(
                        "N0 probe supports scalars, holes, arithmetic, If, closed quotations, parameters, and application",
                    ));
                }
            };
            pending.extend(expr.operands());
            if let Expr::Quote { body, .. } = &expr {
                pending.push(*body);
            }
            nodes.insert(cid, expr);
        }
        let program = Self { root, nodes };
        program.validate()?;
        Ok(program)
    }

    fn validate(&self) -> Result<(), ProbeError> {
        for expr in self.nodes.values() {
            let Expr::Quote { params, body } = expr else {
                continue;
            };
            for cid in self.template_nodes(*body)? {
                match self.nodes.get(&cid) {
                    Some(Expr::Param(index)) if *index >= *params => {
                        return Err(ProbeError::Unsupported(
                            "parameter index outside its quotation",
                        ));
                    }
                    Some(Expr::Hole(_)) => {
                        return Err(ProbeError::Unsupported(
                            "open code: a hole inside a quotation",
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Nodes of one template: reachable from `root` through operand edges.
    fn template_nodes(&self, root: Cid) -> Result<Vec<Cid>, ProbeError> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(cid) = pending.pop() {
            if !seen.insert(cid) {
                continue;
            }
            let expr = self.nodes.get(&cid).ok_or(ProbeError::MissingNode(cid))?;
            pending.extend(expr.operands());
        }
        Ok(seen.into_iter().collect())
    }

    /// Static use counts of one template: one per operand edge.
    fn template_counts(&self, root: Cid) -> Result<(BTreeMap<Cid, usize>, usize), ProbeError> {
        let nodes = self.template_nodes(root)?;
        let mut counts = BTreeMap::new();
        for cid in &nodes {
            for operand in self.nodes[cid].operands() {
                *counts.entry(operand).or_default() += 1;
            }
        }
        Ok((counts, nodes.len()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum N0Fault {
    Overflow(&'static str),
    Type(&'static str),
    Arity { expected: u16, actual: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum N0Outcome {
    Value(Scalar),
    /// A reference to closed code.
    Code(Cid),
    /// Internal observation metadata, not a canonical residual graph. At the
    /// public root this is normalized to Unknown; consumers still need its
    /// groundness to match the reference's downstream type checks.
    Residual {
        ground: bool,
    },
    Unknown,
    Error(N0Fault),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct N0Run {
    pub outcome: N0Outcome,
    /// Cumulative over the lifetime of the machine.
    pub stats: ProbeStats,
}

/// Counters the shared `ProbeStats` does not separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct N0Detail {
    pub instances: usize,
    /// Nodes visited while counting a template's static uses, once per body.
    pub template_walks: usize,
    /// Structural groundness visits without evaluating dormant branches.
    /// Like template census/cleanup, these are outside transition fuel.
    pub ground_visits: usize,
    /// Use sites attached by parameters to caller argument nodes.
    pub proxy_sites: usize,
    pub control_hops: usize,
    pub cell_hits: usize,
    pub teardown_steps: usize,
    pub cells: usize,
    pub merges: usize,
    pub sites: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct N0Storage {
    pub agents: crate::demand::StorageStats,
    pub instance_records: usize,
    pub argument_keys: usize,
    pub template_count_entries: usize,
}

/// A cell identity: which instance, which template node.  Instance 0 is the
/// program itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub instance: u32,
    pub node: Cid,
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
enum Up {
    Cell(Key),
    Merge(usize, Side),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeRef {
    Site(usize),
    Merge(usize),
}

/// Stateless control-side k-to-1 tree node.
#[derive(Clone, Debug)]
struct Merge {
    up: Up,
    children: [Option<TreeRef>; 2],
}

#[derive(Clone, Debug)]
struct Site {
    child: Key,
    /// The consuming cell and operand slot, or `None` for the observation.
    consumer: Option<(Key, usize)>,
    up: Up,
}

#[derive(Clone, Debug)]
enum Phase {
    Left { multiply: bool },
    Right { multiply: bool, left: N0Outcome },
    Condition,
    Branch,
    Function,
    Body,
    Proxy,
}

#[derive(Clone, Debug, Default)]
enum Control {
    #[default]
    Idle,
    Waiting {
        slot: usize,
        phase: Phase,
        return_to: usize,
    },
}

#[derive(Clone, Debug, Default)]
struct Cell {
    control_uses: Option<TreeRef>,
    /// Operand sites.  For `Apply`: function, arguments, then the instance
    /// root once created.  For a proxying `Param`: the caller's argument.
    slots: Vec<Option<usize>>,
    materialized: bool,
    evaluating: bool,
    result: Option<(N0Outcome, usize)>,
    control: Control,
    /// For `Apply`: the instance created for this cell, kept across epochs.
    instance: Option<u32>,
}

#[derive(Clone, Debug)]
struct Instance {
    body: Cid,
    /// Argument nodes in the caller's instance, one per parameter.
    args: Vec<Key>,
}

/// The single demand token, always carrying its return site.
#[derive(Debug)]
enum Token {
    Call(usize),
    Climb { at: TreeRef, from: usize },
    AtCell { key: Key, from: usize },
    Resume { site: usize, outcome: N0Outcome },
}

enum Step {
    Next(Token),
    Done(N0Outcome),
}

enum Release {
    Site(usize),
    Dormant(Key),
}

fn valid(outcome: &N0Outcome, when: usize, epoch: usize) -> bool {
    matches!(
        outcome,
        N0Outcome::Value(_) | N0Outcome::Code(_) | N0Outcome::Residual { ground: true }
    ) || when == epoch
}

/// Demand-driven quotation/application machine with monotonic bindings and
/// epoch resumption.
#[derive(Clone)]
pub struct Machine {
    program: Program,
    bindings: BTreeMap<String, Scalar>,
    epoch: usize,
    /// Static use counts per template root, computed once each.
    counts: BTreeMap<Cid, BTreeMap<Cid, usize>>,
    /// Remaining uses per cell: static plus dynamic.
    holds: BTreeMap<Key, usize>,
    instances: Vec<Instance>,
    cells: BTreeMap<Key, Cell>,
    merges: Vec<Option<Merge>>,
    sites: Vec<Option<Site>>,
    merge_count: usize,
    site_count: usize,
    observer: Option<usize>,
    stats: ProbeStats,
    detail: N0Detail,
}

impl Machine {
    /// Retained storage, not just logical live agents. Instance argument
    /// vectors/count maps remain after their execution cells are released.
    pub fn storage(&self) -> N0Storage {
        N0Storage {
            agents: crate::demand::StorageStats {
                template_nodes: self.program.nodes.len(),
                use_count_entries: self.holds.len(),
                issued_slots: self.merges.len() + self.sites.len(),
                vacant_slots: self.merges.len() + self.sites.len()
                    - self.merge_count
                    - self.site_count,
                capacity_slots: self.merges.capacity() + self.sites.capacity(),
                capacity_bytes: self.merges.capacity() * std::mem::size_of::<Option<Merge>>()
                    + self.sites.capacity() * std::mem::size_of::<Option<Site>>(),
            },
            instance_records: self.instances.len(),
            argument_keys: self
                .instances
                .iter()
                .map(|instance| instance.args.len())
                .sum(),
            template_count_entries: self.counts.values().map(BTreeMap::len).sum(),
        }
    }

    pub fn new(program: Program) -> Self {
        Self {
            program,
            bindings: BTreeMap::new(),
            epoch: 0,
            counts: BTreeMap::new(),
            holds: BTreeMap::new(),
            instances: Vec::new(),
            cells: BTreeMap::new(),
            merges: Vec::new(),
            sites: Vec::new(),
            merge_count: 0,
            site_count: 0,
            observer: None,
            stats: ProbeStats::default(),
            detail: N0Detail::default(),
        }
    }

    pub fn detail(&self) -> N0Detail {
        N0Detail {
            cells: self.cells.len(),
            merges: self.merge_count,
            sites: self.site_count,
            ..self.detail
        }
    }

    /// Run one observation epoch; `budget` limits transitions in this call.
    /// Conflicting bindings are rejected before any machine state changes.
    /// Budget exhaustion clears the waiting chain; a later run observes
    /// afresh, keeping cached values and existing instances.
    pub fn run(
        &mut self,
        bindings: &BTreeMap<String, Scalar>,
        budget: usize,
    ) -> Result<N0Run, ProbeError> {
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
                    return Ok(N0Run {
                        outcome: match outcome {
                            N0Outcome::Residual { .. } => N0Outcome::Unknown,
                            other => other,
                        },
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

    /// Check quiescent invariants: no evaluating or waiting cell; tree links
    /// consistent; every cell still held; `live` equal to a recount.
    /// Diagnostic only.
    pub fn audit(&self) -> Result<(), ProbeError> {
        for (key, cell) in &self.cells {
            if cell.evaluating {
                return Err(ProbeError::Invariant("quiescent cell is evaluating"));
            }
            if !matches!(cell.control, Control::Idle) {
                return Err(ProbeError::Invariant("quiescent cell is waiting"));
            }
            if self.holds.get(key).copied().unwrap_or(0) == 0 {
                return Err(ProbeError::Invariant("cell survives without a use"));
            }
            if let Some(root) = cell.control_uses
                && self.up_of(root)? != Up::Cell(*key)
            {
                return Err(ProbeError::Invariant(
                    "tree root does not point at its cell",
                ));
            }
            for (slot, site) in cell.slots.iter().enumerate() {
                let Some(site) = site else { continue };
                if self.site(*site)?.consumer != Some((*key, slot)) {
                    return Err(ProbeError::Invariant("slot site belongs to another cell"));
                }
            }
        }
        for (id, merge) in self.merges.iter().enumerate() {
            let Some(merge) = merge else { continue };
            for (index, child) in merge.children.iter().enumerate() {
                let Some(child) = child else { continue };
                let side = if index == 0 { Side::Left } else { Side::Right };
                if self.up_of(*child)? != Up::Merge(id, side) {
                    return Err(ProbeError::Invariant("merge child does not point back"));
                }
            }
        }
        for site in self.sites.iter().flatten() {
            let mut up = site.up;
            let mut hops = 0;
            loop {
                match up {
                    Up::Cell(key) => {
                        if key != site.child {
                            return Err(ProbeError::Invariant("site attached to the wrong cell"));
                        }
                        break;
                    }
                    Up::Merge(merge, _) => up = self.merge(merge)?.up,
                }
                hops += 1;
                if hops > self.merges.len() + 1 {
                    return Err(ProbeError::Invariant("cyclic tree"));
                }
            }
        }
        if self.cells.len() + self.merge_count + self.site_count != self.stats.live {
            return Err(ProbeError::Invariant("live accounting drifted"));
        }
        Ok(())
    }

    fn account(&mut self, in_flight: usize) {
        self.stats.live = self.cells.len() + self.merge_count + self.site_count + in_flight;
        self.stats.peak_live = self.stats.peak_live.max(self.stats.live);
    }

    fn template_root(&self, instance: u32) -> Cid {
        match instance {
            0 => self.program.root,
            k => self.instances[k as usize - 1].body,
        }
    }

    /// Remaining uses of a cell, initialized from its template's static count
    /// on first touch.  The template's counts are computed once.
    fn hold_mut(&mut self, key: Key) -> Result<&mut usize, ProbeError> {
        if !self.holds.contains_key(&key) {
            let root = self.template_root(key.instance);
            if !self.counts.contains_key(&root) {
                let (counts, visited) = self.program.template_counts(root)?;
                self.detail.template_walks += visited;
                self.counts.insert(root, counts);
            }
            let count = self.counts[&root].get(&key.node).copied().unwrap_or(0);
            self.holds.insert(key, count);
        }
        Ok(self.holds.get_mut(&key).expect("inserted above"))
    }

    fn observer(&mut self) -> Result<usize, ProbeError> {
        if let Some(site) = self.observer {
            return Ok(site);
        }
        let root = Key {
            instance: 0,
            node: self.program.root,
        };
        *self.hold_mut(root)? += 1;
        let site = self.new_site(Site {
            child: root,
            consumer: None,
            up: Up::Cell(root),
        });
        self.attach(root, site)?;
        self.observer = Some(site);
        Ok(site)
    }

    /// Host-side recovery after an aborted run: follow the chain of waiting
    /// cells from the root, across instances, clearing each one.
    fn cancel(&mut self) {
        let mut key = Key {
            instance: 0,
            node: self.program.root,
        };
        loop {
            let Some(cell) = self.cells.get_mut(&key) else {
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
            key = next;
        }
        self.account(0);
    }

    fn step(&mut self, token: Token) -> Result<Step, ProbeError> {
        match token {
            Token::Call(site) => {
                self.stats.requests += 1;
                Ok(Step::Next(Token::Climb {
                    at: TreeRef::Site(site),
                    from: site,
                }))
            }
            Token::Climb { at, from } => match self.up_of(at)? {
                Up::Cell(key) => Ok(Step::Next(Token::AtCell { key, from })),
                Up::Merge(merge, _) => {
                    self.stats.routing_steps += 1;
                    self.detail.control_hops += 1;
                    Ok(Step::Next(Token::Climb {
                        at: TreeRef::Merge(merge),
                        from,
                    }))
                }
            },
            Token::AtCell { key, from } => self.at_cell(key, from),
            Token::Resume { site, outcome } => match self.site(site)?.consumer {
                None => Ok(Step::Done(outcome)),
                Some((parent, slot)) => self.resume(parent, slot, outcome),
            },
        }
    }

    /// The reference instantiates but does not evaluate branches of a stuck
    /// If. Inspect those original templates with parameter substitution and
    /// context bindings, not memoized evaluation results. In particular an
    /// overflowing dormant branch can be ground without being evaluated.
    fn branch_ground(&mut self, root: Key) -> Result<bool, ProbeError> {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(key) = pending.pop() {
            if !seen.insert(key) {
                continue;
            }
            self.detail.ground_visits += 1;
            let expr = self
                .program
                .nodes
                .get(&key.node)
                .ok_or(ProbeError::MissingNode(key.node))?;
            match expr {
                Expr::Value(_) | Expr::Quote { .. } => {}
                Expr::Hole(name) if !self.bindings.contains_key(name) => return Ok(false),
                Expr::Hole(_) => {}
                Expr::Param(_) if key.instance == 0 => return Ok(false),
                Expr::Param(index) => {
                    let target = self.instances[key.instance as usize - 1]
                        .args
                        .get(usize::from(*index))
                        .copied()
                        .ok_or(ProbeError::Invariant(
                            "groundness parameter without argument",
                        ))?;
                    pending.push(target);
                }
                other => pending.extend(other.operands().into_iter().map(|node| Key {
                    instance: key.instance,
                    node,
                })),
            }
        }
        Ok(true)
    }

    fn stuck_if(
        &mut self,
        parent: Key,
        condition_ground: bool,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let Some(Expr::If {
            when_true,
            when_false,
            ..
        }) = self.program.nodes.get(&parent.node)
        else {
            return Err(ProbeError::Invariant("condition phase outside If"));
        };
        let branches = [*when_true, *when_false];
        let mut ground = condition_ground;
        for node in branches {
            if !ground {
                break;
            }
            ground = self.branch_ground(Key {
                instance: parent.instance,
                node,
            })?;
        }
        self.complete(parent, N0Outcome::Residual { ground }, return_to)
    }

    fn at_cell(&mut self, key: Key, from: usize) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get(&key)
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
                outcome,
            }));
        }
        if !cell.materialized {
            self.materialize(key)?;
        }
        let expr = self
            .program
            .nodes
            .get(&key.node)
            .cloned()
            .ok_or(ProbeError::MissingNode(key.node))?;
        self.cells
            .get_mut(&key)
            .expect("cell checked above")
            .evaluating = true;
        self.stats.evaluations += 1;
        *self.stats.node_evaluations.entry(key.node).or_default() += 1;
        match expr {
            Expr::Value(value) => self.complete(key, N0Outcome::Value(value), from),
            Expr::Hole(name) => {
                let outcome = self
                    .bindings
                    .get(&name)
                    .cloned()
                    .map_or(N0Outcome::Unknown, N0Outcome::Value);
                self.complete(key, outcome, from)
            }
            Expr::Quote { .. } => self.complete(key, N0Outcome::Code(key.node), from),
            Expr::Param(_) if key.instance == 0 => self.complete(key, N0Outcome::Unknown, from),
            Expr::Param(_) => self.demand(key, 0, Phase::Proxy, from),
            Expr::Add(_, _) | Expr::Mul(_, _) => self.demand(
                key,
                0,
                Phase::Left {
                    multiply: matches!(expr, Expr::Mul(_, _)),
                },
                from,
            ),
            Expr::If { .. } => self.demand(key, 0, Phase::Condition, from),
            Expr::Apply { .. } => self.demand(key, 0, Phase::Function, from),
        }
    }

    /// Instantiate a cell's operand use sites.  A proxying parameter attaches
    /// to the caller's argument node and raises its use count: runtime wiring.
    fn materialize(&mut self, key: Key) -> Result<(), ProbeError> {
        let expr = self
            .program
            .nodes
            .get(&key.node)
            .cloned()
            .ok_or(ProbeError::MissingNode(key.node))?;
        let targets = match expr {
            Expr::Param(index) if key.instance > 0 => {
                let target = self.instances[key.instance as usize - 1]
                    .args
                    .get(usize::from(index))
                    .copied()
                    .ok_or(ProbeError::Invariant("parameter without an argument"))?;
                *self.hold_mut(target)? += 1;
                self.detail.proxy_sites += 1;
                vec![target]
            }
            other => other
                .operands()
                .into_iter()
                .map(|node| Key {
                    instance: key.instance,
                    node,
                })
                .collect(),
        };
        let mut slots = Vec::with_capacity(targets.len());
        for (slot, target) in targets.into_iter().enumerate() {
            let site = self.new_site(Site {
                child: target,
                consumer: Some((key, slot)),
                up: Up::Cell(target),
            });
            self.attach(target, site)?;
            slots.push(Some(site));
        }
        let cell = self
            .cells
            .get_mut(&key)
            .ok_or(ProbeError::Invariant("materializing a missing cell"))?;
        cell.slots = slots;
        cell.materialized = true;
        Ok(())
    }

    /// Add a use site to a node's control tree, creating its pending cell (and
    /// initializing its static use count) if this is its first consumer.  The
    /// tree grows as a chain.
    fn attach(&mut self, child: Key, site: usize) -> Result<(), ProbeError> {
        self.hold_mut(child)?;
        let old = self.cells.entry(child).or_default().control_uses;
        match old {
            None => {
                self.cells
                    .get_mut(&child)
                    .expect("cell exists")
                    .control_uses = Some(TreeRef::Site(site));
                self.site_mut(site)?.up = Up::Cell(child);
            }
            Some(old) => {
                let merge = self.new_merge(Merge {
                    up: Up::Cell(child),
                    children: [Some(old), Some(TreeRef::Site(site))],
                });
                self.set_up(old, Up::Merge(merge, Side::Left))?;
                self.site_mut(site)?.up = Up::Merge(merge, Side::Right);
                self.cells
                    .get_mut(&child)
                    .expect("cell exists")
                    .control_uses = Some(TreeRef::Merge(merge));
            }
        }
        Ok(())
    }

    fn demand(
        &mut self,
        key: Key,
        slot: usize,
        phase: Phase,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let cell = self
            .cells
            .get_mut(&key)
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
        key: Key,
        outcome: N0Outcome,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let epoch = self.epoch;
        let cell = self
            .cells
            .get_mut(&key)
            .ok_or(ProbeError::Invariant("completing a missing cell"))?;
        cell.evaluating = false;
        cell.control = Control::Idle;
        cell.result = Some((outcome.clone(), epoch));
        if matches!(
            outcome,
            N0Outcome::Value(_) | N0Outcome::Code(_) | N0Outcome::Residual { ground: true }
        ) {
            // A ground result no longer needs its operands (nor, for an
            // application, its instance): last-consumer release.
            let slots: Vec<usize> = cell.slots.iter_mut().filter_map(Option::take).collect();
            self.drain(slots.into_iter().map(Release::Site).collect())?;
        }
        Ok(Step::Next(Token::Resume {
            site: return_to,
            outcome,
        }))
    }

    fn resume(&mut self, parent: Key, slot: usize, outcome: N0Outcome) -> Result<Step, ProbeError> {
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
                if matches!(outcome, N0Outcome::Error(_)) {
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
                N0Outcome::Value(Scalar::Bool(chosen)) => {
                    let (selected, rejected) = if chosen { (1, 2) } else { (2, 1) };
                    self.erase_use(parent, rejected)?;
                    self.demand(parent, selected, Phase::Branch, return_to)
                }
                N0Outcome::Value(_) => self.complete(
                    parent,
                    N0Outcome::Error(N0Fault::Type("if condition is not a boolean")),
                    return_to,
                ),
                N0Outcome::Code(_) => self.stuck_if(parent, true, return_to),
                N0Outcome::Residual { ground } => self.stuck_if(parent, ground, return_to),
                other => self.complete(parent, other, return_to),
            },
            Phase::Branch | Phase::Body | Phase::Proxy => self.complete(parent, outcome, return_to),
            Phase::Function => self.apply(parent, outcome, return_to),
        }
    }

    /// The function operand of an application has been demanded.
    fn apply(
        &mut self,
        parent: Key,
        function: N0Outcome,
        return_to: usize,
    ) -> Result<Step, ProbeError> {
        let code = match function {
            N0Outcome::Code(code) => code,
            N0Outcome::Value(_) | N0Outcome::Residual { ground: true } => {
                return self.complete(
                    parent,
                    N0Outcome::Error(N0Fault::Type("apply operand is not a quotation")),
                    return_to,
                );
            }
            other => return self.complete(parent, other, return_to),
        };
        let Some(Expr::Apply { arguments, .. }) = self.program.nodes.get(&parent.node).cloned()
        else {
            return Err(ProbeError::Invariant("function phase at a non-application"));
        };
        let Some(Expr::Quote { params, body }) = self.program.nodes.get(&code).cloned() else {
            return Err(ProbeError::Invariant("code value is not a quotation"));
        };
        if usize::from(params) != arguments.len() {
            return self.complete(
                parent,
                N0Outcome::Error(N0Fault::Arity {
                    expected: params,
                    actual: arguments.len(),
                }),
                return_to,
            );
        }
        let body_slot = 1 + arguments.len();
        let has_instance = self.cells[&parent].instance.is_some();
        if !has_instance {
            // One instance per application cell: fresh wiring for this use of
            // the code, kept for later epochs.
            let args = arguments
                .iter()
                .map(|argument| Key {
                    instance: parent.instance,
                    node: *argument,
                })
                .collect();
            self.instances.push(Instance { body, args });
            let instance = self.instances.len() as u32;
            self.detail.instances += 1;
            let root = Key {
                instance,
                node: body,
            };
            *self.hold_mut(root)? += 1;
            let site = self.new_site(Site {
                child: root,
                consumer: Some((parent, body_slot)),
                up: Up::Cell(root),
            });
            self.attach(root, site)?;
            let cell = self.cells.get_mut(&parent).expect("application cell");
            cell.instance = Some(instance);
            if cell.slots.len() != body_slot {
                return Err(ProbeError::Invariant("application slots out of order"));
            }
            cell.slots.push(Some(site));
        }
        self.demand(parent, body_slot, Phase::Body, return_to)
    }

    fn erase_use(&mut self, parent: Key, slot: usize) -> Result<(), ProbeError> {
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

    /// Release uses (S3).  When a node's last use goes, its cell is dropped and
    /// what it held is released in turn: its sites if materialized (including
    /// an application's instance root and a parameter's proxy), otherwise its
    /// static operands in the same instance.
    fn drain(&mut self, mut work: Vec<Release>) -> Result<(), ProbeError> {
        while let Some(item) = work.pop() {
            let child = match item {
                Release::Site(site) => self.detach_site(site)?,
                Release::Dormant(key) => key,
            };
            self.stats.erasures += 1;
            let holds = self.hold_mut(child)?;
            *holds = holds
                .checked_sub(1)
                .ok_or(ProbeError::Invariant("released more uses than exist"))?;
            if *holds > 0 {
                continue;
            }
            match self.cells.remove(&child) {
                Some(cell) if cell.evaluating || !matches!(cell.control, Control::Idle) => {
                    return Err(ProbeError::Invariant(
                        "released a cell during its evaluation",
                    ));
                }
                Some(cell) if cell.materialized => {
                    work.extend(cell.slots.into_iter().flatten().map(Release::Site));
                }
                _ => {
                    let operands = self
                        .program
                        .nodes
                        .get(&child.node)
                        .map(Expr::operands)
                        .unwrap_or_default();
                    work.extend(operands.into_iter().map(|node| {
                        Release::Dormant(Key {
                            instance: child.instance,
                            node,
                        })
                    }));
                }
            }
        }
        Ok(())
    }

    /// Remove a site from its tree, collapsing merges left with one side.
    fn detach_site(&mut self, site: usize) -> Result<Key, ProbeError> {
        let agent = self
            .sites
            .get_mut(site)
            .and_then(Option::take)
            .ok_or(ProbeError::Invariant("detached a missing site"))?;
        self.site_count -= 1;
        self.detail.teardown_steps += 1;
        let mut up = agent.up;
        loop {
            self.detail.teardown_steps += 1;
            match up {
                Up::Cell(key) => {
                    if let Some(cell) = self.cells.get_mut(&key) {
                        cell.control_uses = None;
                    }
                    break;
                }
                Up::Merge(node, side) => {
                    let children = &mut self.merge_mut(node)?.children;
                    children[side.index()] = None;
                    let sibling = children[side.other().index()];
                    let above = self.merge(node)?.up;
                    self.merges[node] = None;
                    self.merge_count -= 1;
                    match sibling {
                        None => up = above,
                        Some(sibling) => {
                            self.set_up(sibling, above)?;
                            self.replace_child(above, TreeRef::Merge(node), sibling)?;
                            break;
                        }
                    }
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

    fn up_of(&self, at: TreeRef) -> Result<Up, ProbeError> {
        match at {
            TreeRef::Site(site) => Ok(self.site(site)?.up),
            TreeRef::Merge(merge) => Ok(self.merge(merge)?.up),
        }
    }

    fn set_up(&mut self, at: TreeRef, up: Up) -> Result<(), ProbeError> {
        match at {
            TreeRef::Site(site) => self.site_mut(site)?.up = up,
            TreeRef::Merge(merge) => self.merge_mut(merge)?.up = up,
        }
        Ok(())
    }

    fn replace_child(&mut self, up: Up, old: TreeRef, new: TreeRef) -> Result<(), ProbeError> {
        let current = match up {
            Up::Cell(key) => {
                &mut self
                    .cells
                    .get_mut(&key)
                    .ok_or(ProbeError::Invariant("tree root of a missing cell"))?
                    .control_uses
            }
            Up::Merge(merge, side) => &mut self.merge_mut(merge)?.children[side.index()],
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
/// then the type check on known operands (a code value is a known
/// non-integer), then checked arithmetic.
fn combine(left: N0Outcome, right: N0Outcome, multiply: bool) -> N0Outcome {
    use N0Outcome::{Code, Error, Residual, Unknown, Value};
    match (left, right) {
        (Error(error), _) | (_, Error(error)) => Error(error),
        (Value(Scalar::Bool(_)) | Code(_) | Residual { ground: true }, _)
        | (_, Value(Scalar::Bool(_)) | Code(_) | Residual { ground: true }) => {
            Error(N0Fault::Type(if multiply {
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
                || Error(N0Fault::Overflow(if multiply { "multiply" } else { "add" })),
                |value| Value(Scalar::Int(value)),
            )
        }
        _ => Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bindings, ReduceError, Reducer};

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

    fn param(store: &mut Store, index: u16) -> Cid {
        store.intern(Node::Param(index))
    }

    fn quote(store: &mut Store, params: u16, body: Cid) -> Cid {
        store.intern(Node::Quote { params, body })
    }

    fn apply(store: &mut Store, function: Cid, arguments: &[Cid]) -> Cid {
        store.intern(Node::Apply {
            function,
            arguments: arguments.to_vec(),
        })
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

    fn count(run: &N0Run, node: Cid) -> usize {
        run.stats.node_evaluations.get(&node).copied().unwrap_or(0)
    }

    fn value(n: i64) -> N0Outcome {
        N0Outcome::Value(Scalar::Int(n))
    }

    /// `square = [ dup * ]`: `Quote(1, Mul(P0, P0))`.
    fn square(store: &mut Store) -> (Cid, Cid) {
        let p0 = param(store, 0);
        let body = store.intern(Node::Mul(p0, p0));
        (quote(store, 1, body), body)
    }

    /// `apply_twice = Quote(2, Apply(P0, [Apply(P0, [P1])]))`.
    fn apply_twice(store: &mut Store) -> Cid {
        let f = param(store, 0);
        let x = param(store, 1);
        let once = apply(store, f, &[x]);
        let twice = apply(store, f, &[once]);
        quote(store, 2, twice)
    }

    /// The CAS reducer as the semantic control.
    fn reference(store: &mut Store, root: Cid, facts: &Facts) -> N0Outcome {
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
                Some(Node::Const(Atom::Int(n))) => value(*n),
                Some(Node::Const(Atom::Bool(b))) => N0Outcome::Value(Scalar::Bool(*b)),
                Some(Node::Quote { .. }) => N0Outcome::Code(reduction.root),
                _ => N0Outcome::Unknown,
            },
            Err(ReduceError::IntegerOverflow(operation)) => {
                N0Outcome::Error(N0Fault::Overflow(operation))
            }
            Err(ReduceError::Type(message)) => N0Outcome::Error(N0Fault::Type(message)),
            Err(ReduceError::Arity { expected, actual }) => {
                N0Outcome::Error(N0Fault::Arity { expected, actual })
            }
            other => panic!("unexpected control result {other:?}"),
        }
    }

    fn check(store: &mut Store, root: Cid, facts: &Facts) -> (Machine, N0Run) {
        let expected = reference(store, root, facts);
        let mut machine = machine(store, root);
        let run = machine.run(facts, BUDGET).unwrap();
        assert_eq!(run.outcome, expected, "{}", store.format(root));
        machine.audit().unwrap();
        (machine, run)
    }

    #[test]
    fn nested_square_makes_two_instances_and_reclaims_them() {
        let mut store = Store::new();
        let (square, body) = square(&mut store);
        let three = int(&mut store, 3);
        let inner = apply(&mut store, square, &[three]);
        let root = apply(&mut store, square, &[inner]);
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(81));
        // The same template body evaluated once in each of two instances.
        assert_eq!(count(&run, body), 2);
        assert_eq!(count(&run, three), 1);
        let detail = machine.detail();
        assert_eq!(detail.instances, 2);
        // Static counting happened once for the square body and once for the program.
        assert_eq!(detail.template_walks, 2 + 4);
        assert_eq!(detail.proxy_sites, 2);
        assert_eq!(run.stats.live, 2);
    }

    #[test]
    fn higher_order_apply_twice_passes_code_as_a_value() {
        let mut store = Store::new();
        let (square, body) = square(&mut store);
        let twice = apply_twice(&mut store);
        let three = int(&mut store, 3);
        let root = apply(&mut store, twice, &[square, three]);
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(81));
        assert_eq!(count(&run, body), 2);
        // apply_twice once, square twice.
        assert_eq!(machine.detail().instances, 3);
        assert_eq!(run.stats.live, 2);
    }

    #[test]
    fn code_survives_being_passed_through_several_calls() {
        let mut store = Store::new();
        let (square, body) = square(&mut store);
        let p0 = param(&mut store, 0);
        let id = quote(&mut store, 1, p0);
        // call = Quote(2, Apply(P0, [P1])): apply its first argument to its second.
        let f = param(&mut store, 0);
        let x = param(&mut store, 1);
        let call_body = apply(&mut store, f, &[x]);
        let call = quote(&mut store, 2, call_body);
        let passed = apply(&mut store, id, &[square]);
        let passed_again = apply(&mut store, id, &[passed]);
        let three = int(&mut store, 3);
        let root = apply(&mut store, call, &[passed_again, three]);
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(9));
        assert_eq!(count(&run, body), 1);
        // id twice, call once, square once.
        assert_eq!(machine.detail().instances, 4);
        assert_eq!(run.stats.live, 2);
        // A code value observed directly is a reference to the quotation.
        let (_, run) = check(&mut store, passed_again, &Facts::new());
        assert_eq!(run.outcome, N0Outcome::Code(square));
    }

    #[test]
    fn same_code_with_distinct_inputs_has_independent_wiring() {
        let mut store = Store::new();
        let (square, body) = square(&mut store);
        let three = int(&mut store, 3);
        let four = int(&mut store, 4);
        let a = apply(&mut store, square, &[three]);
        let b = apply(&mut store, square, &[four]);
        let root = store.intern(Node::Add(a, b));
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(25));
        assert_eq!(count(&run, body), 2);
        assert_eq!(machine.detail().instances, 2);
        assert_eq!(machine.detail().proxy_sites, 2);
    }

    #[test]
    fn a_repeated_identical_application_is_one_evaluation() {
        let mut store = Store::new();
        let (square, body) = square(&mut store);
        let three = int(&mut store, 3);
        let a = apply(&mut store, square, &[three]);
        let root = store.intern(Node::Add(a, a));
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(18));
        assert_eq!(count(&run, a), 1);
        assert_eq!(count(&run, body), 1);
        assert_eq!(machine.detail().instances, 1);
        assert!(run.stats.memo_hits >= 1);
    }

    #[test]
    fn a_shared_argument_is_evaluated_once_however_many_parameters_reach_it() {
        let mut store = Store::new();
        let six = int(&mut store, 6);
        let seven = int(&mut store, 7);
        let shared = store.intern(Node::Mul(six, seven));
        // Inside the template: `P0 + P0` demands the parameter twice.
        let p0 = param(&mut store, 0);
        let sum_body = store.intern(Node::Add(p0, p0));
        let doubler = quote(&mut store, 1, sum_body);
        let doubled = apply(&mut store, doubler, &[shared]);
        // And the argument is shared with a consumer outside the application.
        let root = store.intern(Node::Add(doubled, shared));
        let (machine, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, value(126));
        assert_eq!(count(&run, shared), 1);
        assert_eq!(count(&run, p0), 1);
        assert_eq!(machine.detail().proxy_sites, 1);
        assert_eq!(run.stats.live, 2);
    }

    #[test]
    fn an_argument_no_parameter_demands_stays_dormant() {
        let mut store = Store::new();
        let max = int(&mut store, i64::MAX);
        let one = int(&mut store, 1);
        let bad = store.intern(Node::Add(max, one));
        let seven = int(&mut store, 7);
        let p0 = param(&mut store, 0);
        let p1 = param(&mut store, 1);
        let first = quote(&mut store, 2, p0);
        let second = quote(&mut store, 2, p1);
        for (function, arguments) in [(first, [seven, bad]), (second, [bad, seven])] {
            let root = apply(&mut store, function, &arguments);
            let (machine, run) = check(&mut store, root, &Facts::new());
            assert_eq!(run.outcome, value(7));
            assert_eq!(count(&run, bad), 0);
            assert_eq!(count(&run, max), 0);
            assert_eq!(machine.detail().proxy_sites, 1);
            assert_eq!(run.stats.live, 2);
        }
        // Demanding it does fail.
        let root = apply(&mut store, first, &[bad, seven]);
        let (_, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, N0Outcome::Error(N0Fault::Overflow("add")));
    }

    #[test]
    fn erasing_one_consumer_preserves_work_shared_with_an_application() {
        for (flag, expected) in [(true, 1764 + 42), (false, 84 + 42)] {
            let mut store = Store::new();
            let (square, body) = square(&mut store);
            let six = int(&mut store, 6);
            let seven = int(&mut store, 7);
            let two = int(&mut store, 2);
            let shared = store.intern(Node::Mul(six, seven));
            let squared = apply(&mut store, square, &[shared]);
            let doubled = store.intern(Node::Mul(shared, two));
            let flag_hole = hole(&mut store, "flag");
            let branch = store.intern(Node::If {
                condition: flag_hole,
                when_true: squared,
                when_false: doubled,
            });
            let root = store.intern(Node::Add(branch, shared));
            let bindings = facts(&[("flag", Scalar::Bool(flag))]);
            let (machine, run) = check(&mut store, root, &bindings);
            assert_eq!(run.outcome, value(expected));
            assert_eq!(count(&run, shared), 1);
            assert_eq!(count(&run, body), usize::from(flag));
            assert_eq!(machine.detail().instances, usize::from(flag));
            assert_eq!(run.stats.live, 2);
        }
    }

    #[test]
    fn unknown_function_and_argument_resume_without_re_instantiation() {
        let mut store = Store::new();
        let (square, _) = square(&mut store);
        let p0 = param(&mut store, 0);
        let cube_body = {
            let sq = store.intern(Node::Mul(p0, p0));
            store.intern(Node::Mul(sq, p0))
        };
        let cube = quote(&mut store, 1, cube_body);
        let flag = hole(&mut store, "flag");
        let chosen = store.intern(Node::If {
            condition: flag,
            when_true: square,
            when_false: cube,
        });
        let x = hole(&mut store, "x");
        let root = apply(&mut store, chosen, &[x]);

        // Unknown function: nothing is instantiated, the argument is untouched.
        let mut machine = machine(&store, root);
        let first = machine.run(&Facts::new(), BUDGET).unwrap();
        assert_eq!(first.outcome, N0Outcome::Unknown);
        assert_eq!(machine.detail().instances, 0);
        assert_eq!(count(&first, x), 0);
        machine.audit().unwrap();

        // Known function, unknown argument: one instance, blocked inside it.
        let second = machine
            .run(&facts(&[("flag", Scalar::Bool(false))]), BUDGET)
            .unwrap();
        assert_eq!(second.outcome, N0Outcome::Unknown);
        assert_eq!(machine.detail().instances, 1);
        assert_eq!(count(&second, cube_body), 1);
        machine.audit().unwrap();

        // The argument arrives: the same instance finishes; nothing re-created.
        let third = machine
            .run(&facts(&[("x", Scalar::Int(3))]), BUDGET)
            .unwrap();
        assert_eq!(third.outcome, value(27));
        assert_eq!(machine.detail().instances, 1);
        assert_eq!(count(&third, cube_body), 2);
        // The rejected branch's quotation was never demanded.
        assert_eq!(count(&third, square), 0);
        assert_eq!(third.stats.live, 2);
        machine.audit().unwrap();

        let mut all = Facts::new();
        all.insert("flag".into(), Scalar::Bool(false));
        all.insert("x".into(), Scalar::Int(3));
        assert_eq!(reference(&mut store, root, &all), value(27));
    }

    #[test]
    fn faults_match_the_reference() {
        let mut store = Store::new();
        let (square, _) = square(&mut store);
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let seven = int(&mut store, 7);
        let max = int(&mut store, i64::MAX);
        let bad = store.intern(Node::Add(max, one));
        let f = hole(&mut store, "f");
        let cases = [
            apply(&mut store, square, &[one, two]),
            apply(&mut store, seven, &[one]),
            store.intern(Node::Add(square, one)),
            store.intern(Node::Mul(one, square)),
            store.intern(Node::If {
                condition: seven,
                when_true: one,
                when_false: two,
            }),
            apply(&mut store, square, &[bad]),
            apply(&mut store, bad, &[one]),
        ];
        for root in cases {
            let (_, run) = check(&mut store, root, &Facts::new());
            assert!(
                matches!(run.outcome, N0Outcome::Error(_)),
                "{}",
                store.format(root)
            );
        }
        // An unknown function leaves even a failing argument untouched.
        let root = apply(&mut store, f, &[bad]);
        let (_, run) = check(&mut store, root, &Facts::new());
        assert_eq!(run.outcome, N0Outcome::Unknown);
        assert_eq!(count(&run, bad), 0);
        // Arity mismatch is reported as such.
        let root = apply(&mut store, square, &[one, two]);
        let (_, run) = check(&mut store, root, &Facts::new());
        assert_eq!(
            run.outcome,
            N0Outcome::Error(N0Fault::Arity {
                expected: 1,
                actual: 2
            })
        );
    }

    #[test]
    fn every_budget_cut_leaves_a_consistent_machine_that_can_finish() {
        let mut store = Store::new();
        let (square, _) = square(&mut store);
        let twice = apply_twice(&mut store);
        let three = int(&mut store, 3);
        let root = apply(&mut store, twice, &[square, three]);
        let full = machine(&store, root).run(&Facts::new(), BUDGET).unwrap();
        for budget in 0..=full.stats.transitions {
            let mut machine = machine(&store, root);
            match machine.run(&Facts::new(), budget) {
                Ok(run) => assert_eq!(run.outcome, value(81)),
                Err(ProbeError::BudgetExhausted { limit }) => assert_eq!(limit, budget),
                Err(other) => panic!("budget {budget}: {other:?}"),
            }
            machine
                .audit()
                .unwrap_or_else(|error| panic!("budget {budget}: {error}"));
            let done = machine.run(&Facts::new(), BUDGET).unwrap();
            assert_eq!(done.outcome, value(81), "budget {budget}");
            // Instances created before the cut are reused, never duplicated.
            assert_eq!(machine.detail().instances, 3, "budget {budget}");
            machine.audit().unwrap();
        }
    }

    #[test]
    fn open_or_ill_formed_quotations_are_rejected_at_lowering() {
        let mut store = Store::new();
        let x = hole(&mut store, "x");
        let open = quote(&mut store, 1, x);
        assert!(matches!(
            Program::from_store(&store, open),
            Err(ProbeError::Unsupported(_))
        ));
        let p3 = param(&mut store, 3);
        let out_of_range = quote(&mut store, 1, p3);
        assert!(matches!(
            Program::from_store(&store, out_of_range),
            Err(ProbeError::Unsupported(_))
        ));
        let body = int(&mut store, 1);
        let guard = boolean(&mut store, true);
        let family = store.intern(Node::Family {
            parameters: 0,
            clauses: vec![crate::Clause { guard, body }],
        });
        assert!(matches!(
            Program::from_store(&store, family),
            Err(ProbeError::Unsupported(_))
        ));
    }

    fn random(seed: &mut u64) -> usize {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 32) as usize
    }

    #[test]
    fn generated_programs_match_the_reference_directly_and_in_staged_epochs() {
        let mut seed = 0x6e30_5f70_726f_6265_u64;
        for case in 0..120 {
            let mut store = Store::new();
            let (square, _) = square(&mut store);
            let p0 = param(&mut store, 0);
            let p1 = param(&mut store, 1);
            let one = int(&mut store, 1);
            let templates = [
                (square, 1),
                (quote(&mut store, 1, p0), 1),
                (
                    {
                        let body = store.intern(Node::Add(p0, one));
                        quote(&mut store, 1, body)
                    },
                    1,
                ),
                (quote(&mut store, 2, p0), 2),
                (quote(&mut store, 2, p1), 2),
                (
                    {
                        let body = store.intern(Node::Add(p0, p1));
                        quote(&mut store, 2, body)
                    },
                    2,
                ),
                (apply_twice(&mut store), 2),
                (
                    {
                        let seven = int(&mut store, 7);
                        quote(&mut store, 0, seven)
                    },
                    0,
                ),
            ];
            let mut pool = vec![
                int(&mut store, 0),
                one,
                int(&mut store, -3),
                int(&mut store, i64::MAX),
                boolean(&mut store, true),
                boolean(&mut store, false),
                hole(&mut store, "x"),
                hole(&mut store, "flag"),
            ];
            pool.extend(templates.iter().map(|(code, _)| *code));
            for _ in 0..16 {
                let a = pool[random(&mut seed) % pool.len()];
                let b = pool[random(&mut seed) % pool.len()];
                let c = pool[random(&mut seed) % pool.len()];
                let condition = pool[random(&mut seed) % pool.len()];
                let kind = random(&mut seed) % 5;
                let node = match kind {
                    0 => Node::Add(a, b),
                    1 => Node::Mul(a, b),
                    2 => Node::If {
                        condition,
                        when_true: b,
                        when_false: c,
                    },
                    _ => {
                        let (code, arity) = templates[random(&mut seed) % templates.len()];
                        // Usually the right arity; sometimes not, for the fault path.
                        let arity = if random(&mut seed).is_multiple_of(6) {
                            arity + 1
                        } else {
                            arity
                        };
                        let function = if random(&mut seed).is_multiple_of(4) {
                            a
                        } else {
                            code
                        };
                        Node::Apply {
                            function,
                            arguments: (0..arity)
                                .map(|_| pool[random(&mut seed) % pool.len()])
                                .collect(),
                        }
                    }
                };
                let cid = store.intern(node);
                pool.push(cid);
            }
            let root = *pool.last().unwrap();
            let x = ("x", Scalar::Int(2));
            let flag = ("flag", Scalar::Bool(case % 2 == 0));
            for stages in [vec![x.clone(), flag.clone()], vec![flag.clone(), x.clone()]] {
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
                }
            }
        }
    }
}
