//! Common inputs and observations for experimental demand-protocol probes.
//!
//! These machines compare request/memo designs. They are not the port-level
//! INet backend and do not establish principal-pair locality or confluence.
use crate::{Atom, Cid, Node, Store};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scalar {
    Int(i64),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    Overflow(&'static str),
    Type(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Value(Scalar),
    /// The probe retains pending computation internally; this observation is
    /// not a serialized or canonical representation of the residual graph.
    Unknown,
    Error(Fault),
}

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
}

impl Expr {
    pub fn children(&self) -> Vec<Cid> {
        match self {
            Self::Value(_) | Self::Hole(_) => vec![],
            Self::Add(a, b) | Self::Mul(a, b) => vec![*a, *b],
            Self::If {
                condition,
                when_true,
                when_false,
            } => vec![*condition, *when_true, *when_false],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Program {
    pub root: Cid,
    /// Immutable templates. Count their size separately from execution cells.
    pub nodes: BTreeMap<Cid, Expr>,
}

impl Program {
    /// Lower a scalar DAG without evaluating it. Unsupported nodes are rejected
    /// even in dormant branches; there is no fallback to the CAS evaluator.
    pub fn from_store(store: &Store, root: Cid) -> Result<Self, ProbeError> {
        let mut nodes = BTreeMap::new();
        let mut pending = vec![root];
        while let Some(cid) = pending.pop() {
            if nodes.contains_key(&cid) {
                continue;
            }
            let expression = match store.get(cid).ok_or(ProbeError::MissingNode(cid))? {
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
                _ => {
                    return Err(ProbeError::Unsupported(
                        "demand probe supports only scalar values, holes, arithmetic, and If",
                    ));
                }
            };
            pending.extend(expression.children());
            nodes.insert(cid, expression);
        }
        Ok(Self { root, nodes })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeStats {
    pub transitions: usize,
    pub requests: usize,
    pub evaluations: usize,
    pub memo_hits: usize,
    /// Released request/use-site cells or edges, not persistent template nodes.
    /// Each machine documents its exact accounting; do not compare as bytes.
    pub erasures: usize,
    /// Execution and retained memo storage; excludes immutable Program data.
    pub peak_live: usize,
    pub live: usize,
    pub routing_steps: usize,
    /// Actual evaluations, excluding memo hits. Unknown nodes may be retried
    /// in later epochs; ground pure results must not be recomputed.
    pub node_evaluations: BTreeMap<Cid, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRun {
    pub outcome: Outcome,
    /// Cumulative over the lifetime of the machine, including earlier epochs.
    pub stats: ProbeStats,
}

/// Diagnostic accounting for the B/C append-only agent arrays, not total heap
/// usage. Deleted agents leave vacant slots; neither probe reuses them yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageStats {
    pub template_nodes: usize,
    pub use_count_entries: usize,
    /// Sum of array lengths, including vacant slots.
    pub issued_slots: usize,
    pub vacant_slots: usize,
    /// Sum of array capacities; slots of different agent types have different sizes.
    pub capacity_slots: usize,
    /// Backing-array capacity bytes only: excludes maps, templates, nested
    /// allocations, allocator overhead, and temporary runtime worklists.
    pub capacity_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeError {
    Unsupported(&'static str),
    MissingNode(Cid),
    BudgetExhausted { limit: usize },
    ConflictingBinding(String),
    Invariant(&'static str),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "demand probe: {self:?}")
    }
}

impl std::error::Error for ProbeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_dormant_code_is_rejected_without_evaluating_it() {
        let mut store = Store::new();
        let value = store.intern(Node::Const(Atom::Int(7)));
        let quote = store.intern(Node::Quote {
            params: 0,
            body: value,
        });
        let yes = store.intern(Node::Const(Atom::Bool(true)));
        let root = store.intern(Node::If {
            condition: yes,
            when_true: value,
            when_false: quote,
        });
        assert!(matches!(
            Program::from_store(&store, root),
            Err(ProbeError::Unsupported(_))
        ));
    }

    #[test]
    fn lowering_reports_missing_references() {
        let mut store = Store::new();
        let missing = Cid::digest(b"probe-missing", b"node");
        let root = store.intern(Node::Add(missing, missing));
        assert!(
            matches!(Program::from_store(&store, root), Err(ProbeError::MissingNode(cid)) if cid == missing)
        );
    }
}
