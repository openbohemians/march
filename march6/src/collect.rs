//! Explicit-root collection at host safe points, not a reduction rule.
use crate::{Cid, Store};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectionStats {
    pub before: usize,
    pub retained: usize,
    pub reclaimed: usize,
    pub traced_edges: usize,
    pub validation_entries_removed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionError {
    MissingNode(Cid),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNode(cid) => {
                write!(f, "collection root or edge references missing node {cid}")
            }
        }
    }
}

impl std::error::Error for CollectionError {}

impl Store {
    /// Remove nodes unreachable from the COMPLETE set of live roots.
    ///
    /// Call only between reductions. CIDs held by host code, bindings, caches,
    /// old snapshots, and other sessions are NOT registered automatically:
    /// include their node roots here if they must remain usable in this store.
    /// An empty root list intentionally releases every node. Serialized images
    /// are independent and do not need their roots kept in this store.
    ///
    /// Traces all structural edges, including dormant code and pending work;
    /// never evaluates them. Missing roots/edges leave nodes AND validation
    /// metadata unchanged. Uses an iterative mark pass and an in-place sweep.
    /// Collection work is separate from reducer fuel and is not a byte bound.
    pub fn collect(&mut self, roots: &[Cid]) -> Result<CollectionStats, CollectionError> {
        let mut marked = BTreeSet::new();
        let mut pending = roots.to_vec();
        let mut traced_edges = 0;
        while let Some(cid) = pending.pop() {
            if !marked.insert(cid) {
                continue;
            }
            let node = self
                .nodes
                .get(&cid)
                .ok_or(CollectionError::MissingNode(cid))?;
            let children = node.children();
            traced_edges += children.len();
            pending.extend(children);
        }
        // Nothing mutates until the complete live graph has been validated.
        let before = self.nodes.len();
        let validated_before = self.validated_code.len();
        self.nodes.retain(|cid, _| marked.contains(cid));
        self.validated_code.retain(|cid| marked.contains(cid));
        Ok(CollectionStats {
            before,
            retained: self.nodes.len(),
            reclaimed: before - self.nodes.len(),
            traced_edges,
            validation_entries_removed: validated_before - self.validated_code.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Atom, Node};

    #[test]
    fn validation_cache_is_pruned_and_failed_collection_is_atomic() {
        let mut store = Store::new();
        let body = store.intern(Node::Const(Atom::Int(7)));
        let live = store.intern(Node::Quote { params: 0, body });
        let dead = store.intern(Node::Quote { params: 1, body });
        store.validated_code.extend([live, dead]);
        let missing = Cid::digest(b"missing", b"node");
        let nodes = store.nodes.clone();
        let cache = store.validated_code.clone();
        assert_eq!(
            store.collect(&[live, missing]),
            Err(CollectionError::MissingNode(missing))
        );
        assert_eq!(store.nodes, nodes);
        assert_eq!(store.validated_code, cache);
        let stats = store.collect(&[live]).unwrap();
        assert_eq!(stats.validation_entries_removed, 1);
        assert_eq!(store.validated_code, BTreeSet::from([live]));
        store.collect(&[]).unwrap();
        assert!(store.nodes.is_empty());
        assert!(store.validated_code.is_empty());
    }
}
