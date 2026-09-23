//! Explicit lowerings from the content-addressed semantic DAG into the two
//! experimental execution/resource representations.

use crate::cid::Cid;
use crate::inet::{AgentKind, Net as InetNet, NetError, Port};
use crate::memory::Instr;
use crate::net::{Atom, Node, Store};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharingPolicy {
    /// Preserve directly repeated children with an explicit `dup`.
    Preserve,
    /// Materialize each occurrence independently.
    Rematerialize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    MissingNode(Cid),
    UnsupportedNode(&'static str),
    WrongHole { expected: String, actual: String },
    PortDemand(Cid),
    Inet(NetError),
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNode(cid) => write!(f, "missing node {cid}"),
            Self::UnsupportedNode(kind) => write!(f, "unsupported lowering node {kind}"),
            Self::WrongHole { expected, actual } => {
                write!(f, "expected hole {expected:?}, found {actual:?}")
            }
            Self::PortDemand(cid) => write!(f, "inconsistent port demand for node {cid}"),
            Self::Inet(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LowerError {}

impl From<NetError> for LowerError {
    fn from(value: NetError) -> Self {
        Self::Inet(value)
    }
}

/// Lower the currently implemented unary arithmetic subset into genuine
/// interaction-net agents.  Unsupported binary-dynamic expressions fail
/// explicitly rather than falling back to hidden host evaluation.
pub fn to_unary_inet(
    store: &Store,
    root: Cid,
    input_hole: &str,
    output_label: &str,
) -> Result<InetNet, LowerError> {
    let mut operators = Vec::new();
    let source = lower_unary(store, root, input_hole, &mut operators)?;
    Ok(InetNet::unary_pipeline(source, &operators, output_label)?)
}

/// Lower an integer expression DAG with holes, arithmetic, and the first
/// guarded-family subset.  Guarded bodies remain immutable templates and are
/// installed as rules, not allocated as live agents, until a `Call` interacts
/// with its demanded first argument.
pub fn to_inet(store: &Store, root: Cid, output_label: &str) -> Result<InetNet, LowerError> {
    let mut net = InetNet::new();
    let mut order = Vec::new();
    collect_expression_dag(store, root, &mut BTreeSet::new(), &mut order)?;

    let mut demand = BTreeMap::<Cid, usize>::new();
    *demand.entry(root).or_default() += 1;
    for cid in &order {
        match store.get(*cid).ok_or(LowerError::MissingNode(*cid))? {
            Node::Add(left, right) | Node::Mul(left, right) => {
                *demand.entry(*left).or_default() += 1;
                *demand.entry(*right).or_default() += 1;
            }
            Node::Dispatch { arguments, .. } => {
                for argument in arguments {
                    *demand.entry(*argument).or_default() += 1;
                }
            }
            Node::Hole(_) | Node::Const(Atom::Int(_) | Atom::Bool(_)) => {}
            _ => return Err(LowerError::UnsupportedNode("non-integer expression DAG")),
        }
    }

    let mut outlets = BTreeMap::<Cid, Vec<Port>>::new();
    let mut installed_templates = BTreeSet::new();
    for cid in order {
        let node = store
            .get(cid)
            .cloned()
            .ok_or(LowerError::MissingNode(cid))?;
        let source = match node {
            Node::Hole(name) => {
                let agent = net.add(AgentKind::Hole(name));
                net.principal(agent)?
            }
            Node::Const(Atom::Int(value)) => {
                let agent = net.add(AgentKind::Int(value));
                net.principal(agent)?
            }
            Node::Const(Atom::Bool(value)) => {
                let agent = net.add(AgentKind::Bool(value));
                net.principal(agent)?
            }
            Node::Add(left, right) | Node::Mul(left, right) => {
                let operator = net.add(if matches!(node, Node::Add(_, _)) {
                    AgentKind::AddPair
                } else {
                    AgentKind::MulPair
                });
                let left = take_outlet(&mut outlets, left)?;
                let right = take_outlet(&mut outlets, right)?;
                let principal = net.principal(operator)?;
                let right_input = net.port(operator, 1)?;
                net.connect(left, principal)?;
                net.connect(right, right_input)?;
                net.port(operator, 2)?
            }
            Node::Dispatch { family, arguments } => {
                install_template_graph(store, family, &mut net, &mut installed_templates)?;
                let parameters = family_parameters(store, family)?;
                if parameters == 0 || arguments.len() != usize::from(parameters) {
                    return Err(LowerError::UnsupportedNode(
                        "guarded INet call has unsupported arity",
                    ));
                }
                validate_first_argument_guards(store, family)?;
                validate_passive_call_arguments(store, &arguments)?;
                let call = net.add(AgentKind::Call { family, parameters });
                for (index, argument) in arguments.into_iter().enumerate() {
                    let source = take_outlet(&mut outlets, argument)?;
                    let target = if index == 0 {
                        net.principal(call)?
                    } else {
                        net.port(call, index)?
                    };
                    net.connect(source, target)?;
                }
                net.port(call, usize::from(parameters))?
            }
            _ => return Err(LowerError::UnsupportedNode("non-integer expression DAG")),
        };
        let count = demand.get(&cid).copied().unwrap_or(0);
        if count == 0 {
            return Err(LowerError::PortDemand(cid));
        }
        outlets.insert(cid, fan_out(&mut net, source, count)?);
    }

    let expression = take_outlet(&mut outlets, root)?;
    if outlets.values().any(|ports| !ports.is_empty()) {
        return Err(LowerError::PortDemand(root));
    }
    let output = net.add(AgentKind::Output(output_label.into()));
    let output_port = net.principal(output)?;
    net.connect(expression, output_port)?;
    Ok(net)
}

fn collect_expression_dag(
    store: &Store,
    root: Cid,
    seen: &mut BTreeSet<Cid>,
    order: &mut Vec<Cid>,
) -> Result<(), LowerError> {
    if !seen.insert(root) {
        return Ok(());
    }
    let node = store.get(root).ok_or(LowerError::MissingNode(root))?;
    match node {
        Node::Hole(_) | Node::Const(Atom::Int(_) | Atom::Bool(_)) => {}
        Node::Add(left, right) | Node::Mul(left, right) => {
            collect_expression_dag(store, *left, seen, order)?;
            collect_expression_dag(store, *right, seen, order)?;
        }
        Node::Dispatch { arguments, .. } => {
            for argument in arguments {
                collect_expression_dag(store, *argument, seen, order)?;
            }
        }
        _ => return Err(LowerError::UnsupportedNode("non-integer expression DAG")),
    }
    order.push(root);
    Ok(())
}

fn family_parameters(store: &Store, family: Cid) -> Result<u16, LowerError> {
    match store.get(family) {
        Some(Node::Family { parameters, .. }) => Ok(*parameters),
        Some(_) => Err(LowerError::UnsupportedNode(
            "dispatch target is not a guarded family",
        )),
        None => Err(LowerError::MissingNode(family)),
    }
}

fn install_template_graph(
    store: &Store,
    root: Cid,
    net: &mut InetNet,
    installed: &mut BTreeSet<Cid>,
) -> Result<(), LowerError> {
    let mut pending = vec![root];
    while let Some(cid) = pending.pop() {
        if !installed.insert(cid) {
            continue;
        }
        let node = store
            .get(cid)
            .cloned()
            .ok_or(LowerError::MissingNode(cid))?;
        match &node {
            Node::Dispatch { arguments, .. } | Node::Recur(arguments) => {
                validate_passive_call_arguments(store, arguments)?;
            }
            _ => {}
        }
        pending.extend(node.children());
        net.install_template(cid, node);
    }
    Ok(())
}

fn validate_first_argument_guards(store: &Store, family: Cid) -> Result<(), LowerError> {
    let Node::Family { clauses, .. } = store.get(family).ok_or(LowerError::MissingNode(family))?
    else {
        return Err(LowerError::UnsupportedNode(
            "dispatch target is not a guarded family",
        ));
    };
    let mut parameter_zero_is_semantically_demanded = false;
    for clause in clauses {
        match store
            .get(clause.guard)
            .ok_or(LowerError::MissingNode(clause.guard))?
        {
            Node::Const(Atom::Bool(false)) => continue,
            Node::Const(Atom::Bool(true)) => {
                if !parameter_zero_is_semantically_demanded {
                    return Err(LowerError::UnsupportedNode(
                        "guarded INet call would make an undemanded first parameter strict",
                    ));
                }
                break;
            }
            Node::Param(0) => parameter_zero_is_semantically_demanded = true,
            Node::Eq(left, right)
                if is_parameter_zero(store, *left) && is_pattern_atom(store, *right)
                    || is_parameter_zero(store, *right) && is_pattern_atom(store, *left) =>
            {
                parameter_zero_is_semantically_demanded = true;
            }
            _ => {
                return Err(LowerError::UnsupportedNode(
                    "guarded INet currently requires guards on parameter zero",
                ));
            }
        }
    }
    if !parameter_zero_is_semantically_demanded {
        return Err(LowerError::UnsupportedNode(
            "guarded INet call does not demand its first parameter",
        ));
    }
    Ok(())
}

fn is_parameter_zero(store: &Store, cid: Cid) -> bool {
    matches!(store.get(cid), Some(Node::Param(0)))
}

fn is_pattern_atom(store: &Store, cid: Cid) -> bool {
    matches!(
        store.get(cid),
        Some(Node::Const(Atom::Int(_) | Atom::Bool(_)))
    )
}

/// Only the principal argument of the first guarded-call encoding carries
/// demand.  A computed subnet attached to an auxiliary port could otherwise
/// reduce before selection and report an error even when the selected clause
/// erases it.  Until explicit thunk/decision agents exist, accept only passive
/// atoms on those ports and reject a lowering that would change CAS laziness.
fn validate_passive_call_arguments(store: &Store, arguments: &[Cid]) -> Result<(), LowerError> {
    for argument in arguments.iter().skip(1) {
        match store
            .get(*argument)
            .ok_or(LowerError::MissingNode(*argument))?
        {
            Node::Const(Atom::Int(_) | Atom::Bool(_)) | Node::Hole(_) | Node::Param(_) => {}
            _ => {
                return Err(LowerError::UnsupportedNode(
                    "guarded INet cannot defer a computed non-principal argument",
                ));
            }
        }
    }
    Ok(())
}

fn take_outlet(outlets: &mut BTreeMap<Cid, Vec<Port>>, cid: Cid) -> Result<Port, LowerError> {
    outlets
        .get_mut(&cid)
        .and_then(Vec::pop)
        .ok_or(LowerError::PortDemand(cid))
}

fn fan_out(net: &mut InetNet, source: Port, count: usize) -> Result<Vec<Port>, LowerError> {
    if count == 1 {
        return Ok(vec![source]);
    }
    let fan = net.add(AgentKind::Fan);
    let principal = net.principal(fan)?;
    net.connect(source, principal)?;
    let left = net.port(fan, 1)?;
    let right = net.port(fan, 2)?;
    let left_count = count / 2;
    let mut outputs = fan_out(net, left, left_count)?;
    outputs.extend(fan_out(net, right, count - left_count)?);
    Ok(outputs)
}

fn lower_unary(
    store: &Store,
    root: Cid,
    input_hole: &str,
    operators: &mut Vec<AgentKind>,
) -> Result<AgentKind, LowerError> {
    let node = store.get(root).ok_or(LowerError::MissingNode(root))?;
    match node {
        Node::Hole(name) if name == input_hole => Ok(AgentKind::Hole(name.clone())),
        Node::Hole(name) => Err(LowerError::WrongHole {
            expected: input_hole.into(),
            actual: name.clone(),
        }),
        Node::Const(Atom::Int(value)) => Ok(AgentKind::Int(*value)),
        Node::Add(left, right) => {
            let (inner, constant) = unary_constant_side(store, *left, *right)?;
            let source = lower_unary(store, inner, input_hole, operators)?;
            operators.push(AgentKind::Add(constant));
            Ok(source)
        }
        Node::Mul(left, right) => {
            let (inner, constant) = unary_constant_side(store, *left, *right)?;
            let source = lower_unary(store, inner, input_hole, operators)?;
            operators.push(AgentKind::Mul(constant));
            Ok(source)
        }
        _ => Err(LowerError::UnsupportedNode("non-unary expression")),
    }
}

fn unary_constant_side(store: &Store, left: Cid, right: Cid) -> Result<(Cid, i64), LowerError> {
    if let Some(Node::Const(Atom::Int(value))) = store.get(right) {
        return Ok((left, *value));
    }
    if let Some(Node::Const(Atom::Int(value))) = store.get(left) {
        return Ok((right, *value));
    }
    Err(LowerError::UnsupportedNode(
        "binary operation with two dynamic inputs",
    ))
}

/// Lower immutable integer/pair values into the stack resource IR and consume
/// the root.  The policy makes the CAS-sharing decision explicit.
pub fn value_to_memory(
    store: &Store,
    root: Cid,
    sharing: SharingPolicy,
) -> Result<Vec<Instr>, LowerError> {
    let mut program = Vec::new();
    lower_value(store, root, sharing, &mut program)?;
    program.push(Instr::Drop);
    Ok(program)
}

fn lower_value(
    store: &Store,
    root: Cid,
    sharing: SharingPolicy,
    program: &mut Vec<Instr>,
) -> Result<(), LowerError> {
    match store.get(root).ok_or(LowerError::MissingNode(root))? {
        Node::Const(Atom::Int(value)) => program.push(Instr::Int(*value)),
        Node::Pair(left, right) if left == right && sharing == SharingPolicy::Preserve => {
            lower_value(store, *left, sharing, program)?;
            program.push(Instr::Dup);
            program.push(Instr::Pair);
        }
        Node::Pair(left, right) => {
            lower_value(store, *left, sharing, program)?;
            lower_value(store, *right, sharing, program)?;
            program.push(Instr::Pair);
        }
        _ => return Err(LowerError::UnsupportedNode("non-value memory graph")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inet::{Schedule, Value};
    use crate::memory::{InputIdentities, compare};
    use std::collections::BTreeMap;

    #[test]
    fn residual_dag_is_lowered_instead_of_rebuilt_by_hand() {
        let mut store = Store::new();
        let input = store.intern(Node::Hole("x".into()));
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        let add = store.intern(Node::Add(input, one));
        let root = store.intern(Node::Mul(add, two));
        let mut net = to_unary_inet(&store, root, "x", "result").unwrap();
        net.bind(&BTreeMap::from([("x".into(), Value::Int(41))]));
        net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(84)));
    }

    #[test]
    fn cas_sharing_becomes_explicit_duplication_or_rematerialization() {
        let mut store = Store::new();
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        let leaf = store.intern(Node::Pair(one, two));
        let root = store.intern(Node::Pair(leaf, leaf));

        let preserve = value_to_memory(&store, root, SharingPolicy::Preserve).unwrap();
        let preserve = compare(&preserve, &InputIdentities::new()).unwrap();
        assert_eq!(preserve.plan.metrics.local_allocations, 2);
        assert_eq!(preserve.plan.metrics.exact_frees, 2);
        assert!(preserve.oracle.premature_frees.is_empty());

        let rematerialize = value_to_memory(&store, root, SharingPolicy::Rematerialize).unwrap();
        let rematerialize = compare(&rematerialize, &InputIdentities::new()).unwrap();
        assert_eq!(rematerialize.plan.metrics.local_allocations, 3);
        assert_eq!(rematerialize.plan.metrics.exact_frees, 3);
    }

    #[test]
    fn unsupported_dynamic_binary_net_does_not_hide_host_evaluation() {
        let mut store = Store::new();
        let left = store.intern(Node::Hole("x".into()));
        let right = store.intern(Node::Hole("y".into()));
        let root = store.intern(Node::Add(left, right));
        assert!(matches!(
            to_unary_inet(&store, root, "x", "result"),
            Err(LowerError::UnsupportedNode(_)) | Err(LowerError::WrongHole { .. })
        ));
    }

    #[test]
    fn dynamic_binary_net_stages_one_operand_then_continues() {
        let mut store = Store::new();
        let left = store.intern(Node::Hole("x".into()));
        let right = store.intern(Node::Hole("y".into()));
        let root = store.intern(Node::Add(left, right));
        let mut net = to_inet(&store, root, "result").unwrap();

        net.bind(&BTreeMap::from([("x".into(), Value::Int(20))]));
        let compile = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(compile.rewrites, 1);
        assert!(net.outputs().is_empty());

        net.bind(&BTreeMap::from([("y".into(), Value::Int(22))]));
        let runtime = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(runtime.rewrites, 2);
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(42)));
    }

    #[test]
    fn dynamic_binary_net_can_defer_its_left_operand() {
        let mut store = Store::new();
        let left = store.intern(Node::Hole("x".into()));
        let right = store.intern(Node::Hole("y".into()));
        let root = store.intern(Node::Add(left, right));
        let mut net = to_inet(&store, root, "result").unwrap();

        net.bind(&BTreeMap::from([("y".into(), Value::Int(22))]));
        let compile = net.reduce(100, Schedule::HighestWire).unwrap();
        assert_eq!(compile.rewrites, 0);
        assert!(net.outputs().is_empty());

        net.bind(&BTreeMap::from([("x".into(), Value::Int(20))]));
        net.reduce(100, Schedule::HighestWire).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(42)));
    }

    #[test]
    fn shared_cas_subexpression_is_computed_once_then_fanned() {
        let mut store = Store::new();
        let x = store.intern(Node::Hole("x".into()));
        let one = store.intern(Node::Const(Atom::Int(1)));
        let shared = store.intern(Node::Add(x, one));
        let root = store.intern(Node::Mul(shared, shared));
        let mut net = to_inet(&store, root, "result").unwrap();

        // Hole, constant, add, fan, multiply, output.  Rematerializing the
        // shared add would require eight agents instead of six.
        assert_eq!(net.live_agents(), 6);
        net.bind(&BTreeMap::from([("x".into(), Value::Int(2))]));
        net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(9)));
        assert_eq!(net.live_agents(), 0);
    }

    #[test]
    fn repeated_dag_lowering_grows_linearly_with_explicit_fans() {
        let mut store = Store::new();
        let mut root = store.intern(Node::Hole("x".into()));
        for _ in 0..12 {
            root = store.intern(Node::Add(root, root));
        }
        let mut net = to_inet(&store, root, "result").unwrap();

        // 1 hole + 12 add agents + 12 fans + 1 output, rather than an
        // exponentially rematerialized arithmetic tree.
        assert_eq!(net.live_agents(), 26);
        net.bind(&BTreeMap::from([("x".into(), Value::Int(1))]));
        net.reduce(200, Schedule::HighestWire).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(4096)));
    }

    #[test]
    fn expression_net_reports_type_and_overflow_errors_like_reference() {
        let mut store = Store::new();
        let x = store.intern(Node::Hole("x".into()));
        let one = store.intern(Node::Const(Atom::Int(1)));
        let root = store.intern(Node::Add(x, one));

        let truth = store.intern(Node::Const(Atom::Bool(true)));
        let mut wrong_type = crate::net::Bindings::new();
        wrong_type.insert("x", truth);
        assert!(matches!(
            crate::reduce::Reducer::new(&mut store, &wrong_type).run(root),
            Err(crate::reduce::ReduceError::Type(_))
        ));
        let mut wrong_type_net = to_inet(&store, root, "result").unwrap();
        wrong_type_net.bind(&BTreeMap::from([("x".into(), Value::Bool(true))]));
        assert!(matches!(
            wrong_type_net.reduce(100, Schedule::LowestWire),
            Err(NetError::NoRule(_, _))
        ));

        let max = store.intern(Node::Const(Atom::Int(i64::MAX)));
        let mut overflow = crate::net::Bindings::new();
        overflow.insert("x", max);
        assert_eq!(
            crate::reduce::Reducer::new(&mut store, &overflow).run(root),
            Err(crate::reduce::ReduceError::IntegerOverflow("add"))
        );
        let mut overflow_net = to_inet(&store, root, "result").unwrap();
        overflow_net.bind(&BTreeMap::from([("x".into(), Value::Int(i64::MAX))]));
        assert_eq!(
            overflow_net.reduce(100, Schedule::LowestWire),
            Err(NetError::IntegerOverflow("add"))
        );
    }

    #[test]
    fn binary_expression_net_matches_reference_under_both_schedules() {
        let mut store = Store::new();
        let x = store.intern(Node::Hole("x".into()));
        let y = store.intern(Node::Hole("y".into()));
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        let left = store.intern(Node::Add(x, one));
        let right = store.intern(Node::Add(y, two));
        let root = store.intern(Node::Mul(left, right));

        for x_value in -8..=8 {
            for y_value in -8..=8 {
                let x_literal = store.intern(Node::Const(Atom::Int(x_value)));
                let y_literal = store.intern(Node::Const(Atom::Int(y_value)));
                let mut bindings = crate::net::Bindings::new();
                bindings.insert("x", x_literal);
                bindings.insert("y", y_literal);
                let reference = crate::reduce::Reducer::new(&mut store, &bindings)
                    .run(root)
                    .unwrap();
                let expected = match store.get(reference.root) {
                    Some(Node::Const(Atom::Int(value))) => *value,
                    other => panic!("unexpected reference result: {other:?}"),
                };

                let net = to_inet(&store, root, "result").unwrap();
                for schedule in [Schedule::LowestWire, Schedule::HighestWire] {
                    let mut net = net.clone();
                    net.bind(&BTreeMap::from([
                        ("x".into(), Value::Int(x_value)),
                        ("y".into(), Value::Int(y_value)),
                    ]));
                    net.reduce(100, schedule).unwrap();
                    assert_eq!(net.outputs().get("result"), Some(&Value::Int(expected)));
                }
            }
        }
    }
}
