//! Logical reclamation must not be confused with backing-array reclamation.
use march_research::demand::{Outcome, Program, Scalar};
use march_research::inet_demand_b;
use march_research::inet_demand_c::{self, Topology};
use march_research::{Atom, Node, Store};
use std::collections::BTreeMap;

#[test]
fn reclaimed_agents_leave_retained_backing_slots_visible() {
    let mut store = Store::new();
    let mut root = store.intern(Node::Const(Atom::Int(1)));
    for _ in 0..30 {
        root = store.intern(Node::Add(root, root));
    }
    let program = Program::from_store(&store, root).unwrap();
    let facts = BTreeMap::new();
    let mut b = inet_demand_b::Machine::new(program.clone());
    let mut data = inet_demand_c::Machine::new(program.clone());
    let mut control = inet_demand_c::Machine::with_topology(program, Topology::ReplyOnControl);
    let runs = [
        (b.run(&facts, 10_000).unwrap(), b.storage(), 91),
        (data.run(&facts, 10_000).unwrap(), data.storage(), 121),
        (control.run(&facts, 10_000).unwrap(), control.storage(), 121),
    ];
    b.audit().unwrap();
    data.audit().unwrap();
    control.audit().unwrap();
    for (run, storage, slots) in runs {
        assert_eq!(run.outcome, Outcome::Value(Scalar::Int(1 << 30)));
        assert_eq!(run.stats.live, 2);
        assert_eq!(storage.template_nodes, 31);
        assert_eq!(storage.use_count_entries, 31);
        assert_eq!(storage.issued_slots, slots);
        assert_eq!(storage.vacant_slots, slots - 1);
        assert!(storage.capacity_slots >= slots);
        assert!(storage.capacity_bytes > storage.capacity_slots);
        eprintln!("logical live={}, storage={storage:?}", run.stats.live);
    }
    let before = data.storage();
    assert_eq!(data.run(&facts, 10_000).unwrap().stats.live, 2);
    assert_eq!(data.storage(), before);
}
