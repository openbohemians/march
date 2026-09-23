//! A deliberately small, genuine port-level interaction-net experiment.
//!
//! Every agent has exactly one principal port (port zero).  A rewrite can see
//! only two agents whose principal ports are connected and the external ends
//! of their auxiliary wires.  `Hole` agents have no rules; replacing one with
//! a value supplies information at either the compile or runtime stage.

use crate::cid::{Cid, put_str, put_u64};
use crate::net::{Atom, Clause, Node};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    World(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Hole(String),
    Int(i64),
    Bool(bool),
    World(Vec<String>),
    /// Unary arithmetic is enough to test staged local reduction without
    /// hiding a global evaluator inside an agent rule.
    Add(i64),
    Mul(i64),
    /// Binary arithmetic first interacts with its left input.  The rewrite
    /// turns it into `Add(left)`/`Mul(left)` facing the right subnet, keeping
    /// both steps principal-to-principal and local.
    AddPair,
    MulPair,
    ChooseInt {
        when_true: i64,
        when_false: i64,
    },
    Emit(String),
    Fan,
    Erase,
    /// A dormant call to a content-addressed guarded family.  Its principal
    /// port demands parameter zero; remaining parameters and the result are
    /// auxiliary ports.  The selected body is instantiated only when a value
    /// reaches the principal port.
    Call {
        family: Cid,
        parameters: u16,
    },
    Output(String),
}

impl AgentKind {
    pub fn arity(&self) -> usize {
        match self {
            Self::Add(_) | Self::Mul(_) | Self::ChooseInt { .. } | Self::Emit(_) => 2,
            Self::AddPair | Self::MulPair => 3,
            Self::Fan => 3,
            Self::Call { parameters, .. } => usize::from(*parameters) + 1,
            Self::Hole(_)
            | Self::Int(_)
            | Self::Bool(_)
            | Self::World(_)
            | Self::Erase
            | Self::Output(_) => 1,
        }
    }

    fn value(&self) -> Option<Value> {
        match self {
            Self::Int(value) => Some(Value::Int(*value)),
            Self::Bool(value) => Some(Value::Bool(*value)),
            Self::World(trace) => Some(Value::World(trace.clone())),
            _ => None,
        }
    }

    fn from_value(value: Value) -> Self {
        match value {
            Value::Int(value) => Self::Int(value),
            Value::Bool(value) => Self::Bool(value),
            Value::World(trace) => Self::World(trace),
        }
    }

    fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Self::Hole(name) => {
                out.push(0);
                put_str(out, name);
            }
            Self::Int(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::Bool(value) => {
                out.push(2);
                out.push(u8::from(*value));
            }
            Self::World(trace) => {
                out.push(3);
                put_u64(out, trace.len() as u64);
                for entry in trace {
                    put_str(out, entry);
                }
            }
            Self::Add(value) => {
                out.push(4);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::Mul(value) => {
                out.push(5);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::ChooseInt {
                when_true,
                when_false,
            } => {
                out.push(6);
                out.extend_from_slice(&when_true.to_be_bytes());
                out.extend_from_slice(&when_false.to_be_bytes());
            }
            Self::Emit(message) => {
                out.push(7);
                put_str(out, message);
            }
            Self::Fan => out.push(8),
            Self::Erase => out.push(9),
            Self::Output(label) => {
                out.push(10);
                put_str(out, label);
            }
            Self::AddPair => out.push(11),
            Self::MulPair => out.push(12),
            Self::Call { family, parameters } => {
                out.push(13);
                out.extend_from_slice(&family.0);
                out.extend_from_slice(&parameters.to_be_bytes());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WireId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port {
    pub agent: AgentId,
    pub index: usize,
}

#[derive(Clone, Debug)]
struct Agent {
    kind: AgentKind,
    ports: Vec<Option<WireId>>,
}

#[derive(Clone, Debug)]
struct Wire(Option<(Port, Port)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    LowestWire,
    HighestWire,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReductionStats {
    pub rewrites: usize,
    pub deleted_agents: usize,
    pub fresh_agent_cells: usize,
    pub reused_agent_cells: usize,
    pub peak_live_agents: usize,
}

#[derive(Clone, Debug, Default)]
struct Counters {
    deleted_agents: usize,
    fresh_agent_cells: usize,
    reused_agent_cells: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Net {
    agents: Vec<Option<Agent>>,
    wires: Vec<Wire>,
    free_agents: BTreeSet<usize>,
    free_wires: BTreeSet<usize>,
    outputs: BTreeMap<String, Value>,
    /// Immutable rule/template graph.  These nodes are not live agents and do
    /// not contribute to live-cell or allocation measurements.
    templates: BTreeMap<Cid, Node>,
    counters: Counters,
}

impl Net {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unary_pipeline(
        source: AgentKind,
        operators: &[AgentKind],
        output_label: &str,
    ) -> Result<Self, NetError> {
        let mut net = Self::new();
        let source = net.add(source);
        if operators.is_empty() {
            let output = net.add(AgentKind::Output(output_label.into()));
            net.connect(net.principal(source)?, net.principal(output)?)?;
            return Ok(net);
        }
        let mut current = net.add(operators[0].clone());
        net.connect(net.principal(source)?, net.principal(current)?)?;
        for operator in &operators[1..] {
            let next = net.add(operator.clone());
            net.connect(net.port(current, 1)?, net.principal(next)?)?;
            current = next;
        }
        let output = net.add(AgentKind::Output(output_label.into()));
        net.connect(net.port(current, 1)?, net.principal(output)?)?;
        Ok(net)
    }

    pub fn add(&mut self, kind: AgentKind) -> AgentId {
        let agent = Agent {
            ports: vec![None; kind.arity()],
            kind,
        };
        match self.free_agents.pop_first() {
            Some(index) => {
                self.agents[index] = Some(agent);
                self.counters.reused_agent_cells += 1;
                AgentId(index)
            }
            None => {
                let id = AgentId(self.agents.len());
                self.agents.push(Some(agent));
                self.counters.fresh_agent_cells += 1;
                id
            }
        }
    }

    pub(crate) fn install_template(&mut self, cid: Cid, node: Node) {
        if let Some(existing) = self.templates.insert(cid, node.clone()) {
            assert_eq!(existing, node, "template CID collision");
        }
    }

    pub fn port(&self, agent: AgentId, index: usize) -> Result<Port, NetError> {
        let value = self.agent(agent)?;
        if index >= value.ports.len() {
            return Err(NetError::BadPort { agent, index });
        }
        Ok(Port { agent, index })
    }

    pub fn principal(&self, agent: AgentId) -> Result<Port, NetError> {
        self.port(agent, 0)
    }

    pub fn connect(&mut self, left: Port, right: Port) -> Result<WireId, NetError> {
        self.check_free_port(left)?;
        self.check_free_port(right)?;
        let id = match self.free_wires.pop_first() {
            Some(index) => {
                self.wires[index] = Wire(Some((left, right)));
                WireId(index)
            }
            None => {
                let id = WireId(self.wires.len());
                self.wires.push(Wire(Some((left, right))));
                id
            }
        };
        self.agent_mut(left.agent)?.ports[left.index] = Some(id);
        self.agent_mut(right.agent)?.ports[right.index] = Some(id);
        Ok(id)
    }

    pub fn bind(&mut self, bindings: &BTreeMap<String, Value>) {
        for agent in self.agents.iter_mut().flatten() {
            if let AgentKind::Hole(name) = &agent.kind
                && let Some(value) = bindings.get(name)
            {
                agent.kind = AgentKind::from_value(value.clone());
            }
        }
    }

    pub fn outputs(&self) -> &BTreeMap<String, Value> {
        &self.outputs
    }

    pub fn live_agents(&self) -> usize {
        self.agents.iter().flatten().count()
    }

    pub fn snapshot_cid(&self) -> Cid {
        let mut bytes = Vec::new();
        let live_agents = self
            .agents
            .iter()
            .enumerate()
            .filter_map(|(index, agent)| agent.as_ref().map(|agent| (index, agent)))
            .collect::<Vec<_>>();
        put_u64(&mut bytes, live_agents.len() as u64);
        for (index, agent) in live_agents {
            put_u64(&mut bytes, index as u64);
            agent.kind.encode(&mut bytes);
            put_u64(&mut bytes, agent.ports.len() as u64);
            for wire in &agent.ports {
                match wire {
                    Some(wire) => {
                        bytes.push(1);
                        put_u64(&mut bytes, wire.0 as u64);
                    }
                    None => bytes.push(0),
                }
            }
        }
        let live_wires = self
            .wires
            .iter()
            .enumerate()
            .filter_map(|(index, wire)| wire.0.map(|ends| (index, ends)))
            .collect::<Vec<_>>();
        put_u64(&mut bytes, live_wires.len() as u64);
        for (index, (left, right)) in live_wires {
            put_u64(&mut bytes, index as u64);
            encode_port(&mut bytes, left);
            encode_port(&mut bytes, right);
        }
        put_u64(&mut bytes, self.outputs.len() as u64);
        for (label, value) in &self.outputs {
            put_str(&mut bytes, label);
            AgentKind::from_value(value.clone()).encode(&mut bytes);
        }
        Cid::digest(b"march6/inet-snapshot/v1", &bytes)
    }

    pub fn reduce(&mut self, fuel: usize, schedule: Schedule) -> Result<ReductionStats, NetError> {
        let before = self.counters.clone();
        let mut rewrites = 0_usize;
        let mut peak_live = self.live_agents();
        while rewrites < fuel {
            let Some((left, right)) = self.find_redex(schedule)? else {
                break;
            };
            self.rewrite(left, right)?;
            rewrites += 1;
            peak_live = peak_live.max(self.live_agents());
        }
        if rewrites == fuel && self.find_redex(schedule)?.is_some() {
            return Err(NetError::FuelExhausted { limit: fuel });
        }
        Ok(ReductionStats {
            rewrites,
            deleted_agents: self.counters.deleted_agents - before.deleted_agents,
            fresh_agent_cells: self.counters.fresh_agent_cells - before.fresh_agent_cells,
            reused_agent_cells: self.counters.reused_agent_cells - before.reused_agent_cells,
            peak_live_agents: peak_live,
        })
    }

    fn find_redex(&self, schedule: Schedule) -> Result<Option<(AgentId, AgentId)>, NetError> {
        let iter: Box<dyn Iterator<Item = &Wire>> = match schedule {
            Schedule::LowestWire => Box::new(self.wires.iter()),
            Schedule::HighestWire => Box::new(self.wires.iter().rev()),
        };
        for wire in iter {
            let Some((left, right)) = wire.0 else {
                continue;
            };
            if left.index != 0 || right.index != 0 {
                continue;
            }
            let left_kind = &self.agent(left.agent)?.kind;
            let right_kind = &self.agent(right.agent)?.kind;
            if has_rule(left_kind, right_kind) {
                return Ok(Some((left.agent, right.agent)));
            }
            if !matches!(left_kind, AgentKind::Hole(_)) && !matches!(right_kind, AgentKind::Hole(_))
            {
                return Err(NetError::NoRule(left_kind.clone(), right_kind.clone()));
            }
        }
        Ok(None)
    }

    fn rewrite(&mut self, left: AgentId, right: AgentId) -> Result<(), NetError> {
        let left_kind = self.agent(left)?.kind.clone();
        let right_kind = self.agent(right)?.kind.clone();

        if let Some(value) = left_kind.value() {
            return self.rewrite_value(value, left, &right_kind, right);
        }
        if let Some(value) = right_kind.value() {
            return self.rewrite_value(value, right, &left_kind, left);
        }
        Err(NetError::NoRule(left_kind, right_kind))
    }

    fn rewrite_value(
        &mut self,
        value: Value,
        value_id: AgentId,
        operator: &AgentKind,
        operator_id: AgentId,
    ) -> Result<(), NetError> {
        match (value, operator) {
            (Value::Int(value), AgentKind::Add(addend)) => {
                let result = value
                    .checked_add(*addend)
                    .ok_or(NetError::IntegerOverflow("add"))?;
                self.rewrite_unary(value_id, operator_id, Value::Int(result))
            }
            (Value::Int(value), AgentKind::Mul(factor)) => {
                let result = value
                    .checked_mul(*factor)
                    .ok_or(NetError::IntegerOverflow("multiply"))?;
                self.rewrite_unary(value_id, operator_id, Value::Int(result))
            }
            (Value::Int(value), AgentKind::AddPair) => {
                self.rewrite_binary_left(value_id, operator_id, AgentKind::Add(value))
            }
            (Value::Int(value), AgentKind::MulPair) => {
                self.rewrite_binary_left(value_id, operator_id, AgentKind::Mul(value))
            }
            (
                Value::Bool(value),
                AgentKind::ChooseInt {
                    when_true,
                    when_false,
                },
            ) => self.rewrite_unary(
                value_id,
                operator_id,
                Value::Int(if value { *when_true } else { *when_false }),
            ),
            (Value::World(mut trace), AgentKind::Emit(message)) => {
                trace.push(message.clone());
                self.rewrite_unary(value_id, operator_id, Value::World(trace))
            }
            (value, AgentKind::Fan) => {
                let left = self.external(Port {
                    agent: operator_id,
                    index: 1,
                })?;
                let right = self.external(Port {
                    agent: operator_id,
                    index: 2,
                })?;
                self.remove_agent(value_id)?;
                self.remove_agent(operator_id)?;
                let left_value = self.add(AgentKind::from_value(value.clone()));
                let right_value = self.add(AgentKind::from_value(value));
                self.connect(self.principal(left_value)?, left)?;
                self.connect(self.principal(right_value)?, right)?;
                Ok(())
            }
            (value, AgentKind::Call { family, parameters }) => {
                self.rewrite_call(value, value_id, *family, *parameters, operator_id)
            }
            (_, AgentKind::Erase) => {
                self.remove_agent(value_id)?;
                self.remove_agent(operator_id)
            }
            (value, AgentKind::Output(label)) => {
                let label = label.clone();
                self.remove_agent(value_id)?;
                self.remove_agent(operator_id)?;
                if self.outputs.insert(label.clone(), value).is_some() {
                    return Err(NetError::DuplicateOutput(label));
                }
                Ok(())
            }
            (value, operator) => Err(NetError::NoRule(
                AgentKind::from_value(value),
                operator.clone(),
            )),
        }
    }

    fn rewrite_unary(
        &mut self,
        value: AgentId,
        operator: AgentId,
        result: Value,
    ) -> Result<(), NetError> {
        let next = self.external(Port {
            agent: operator,
            index: 1,
        })?;
        self.remove_agent(value)?;
        self.remove_agent(operator)?;
        let result = self.add(AgentKind::from_value(result));
        self.connect(self.principal(result)?, next)?;
        Ok(())
    }

    fn rewrite_binary_left(
        &mut self,
        value: AgentId,
        operator: AgentId,
        continuation: AgentKind,
    ) -> Result<(), NetError> {
        let right = self.external(Port {
            agent: operator,
            index: 1,
        })?;
        let next = self.external(Port {
            agent: operator,
            index: 2,
        })?;
        self.remove_agent(value)?;
        self.remove_agent(operator)?;
        let continuation = self.add(continuation);
        self.connect(self.principal(continuation)?, right)?;
        self.connect(self.port(continuation, 1)?, next)?;
        Ok(())
    }

    fn rewrite_call(
        &mut self,
        value: Value,
        value_id: AgentId,
        family: Cid,
        parameters: u16,
        call_id: AgentId,
    ) -> Result<(), NetError> {
        if parameters == 0 {
            return Err(NetError::UnsupportedTemplate(
                "zero-argument family call has no demanded principal argument",
            ));
        }
        let body = self.select_clause(family, &value)?;
        let mut arguments = Vec::with_capacity(usize::from(parameters));
        for index in 1..usize::from(parameters) {
            arguments.push(self.external(Port {
                agent: call_id,
                index,
            })?);
        }
        let continuation = self.external(Port {
            agent: call_id,
            index: usize::from(parameters),
        })?;

        self.remove_agent(value_id)?;
        self.remove_agent(call_id)?;
        let demanded = self.add(AgentKind::from_value(value));
        let mut actual = vec![self.principal(demanded)?];
        actual.extend(arguments);
        self.instantiate_template(body, family, actual, continuation)
    }

    fn select_clause(&self, family: Cid, value: &Value) -> Result<Cid, NetError> {
        let node = self
            .templates
            .get(&family)
            .ok_or(NetError::MissingTemplate(family))?;
        let Node::Family { clauses, .. } = node else {
            return Err(NetError::UnsupportedTemplate(
                "call target is not a guarded family",
            ));
        };
        for Clause { guard, body } in clauses {
            if self.template_guard_matches(*guard, value)? {
                return Ok(*body);
            }
        }
        Err(NetError::NoMatchingClause(family))
    }

    /// The first INet gate deliberately accepts only guards reducible to a
    /// pattern on parameter zero.  The CAS semantics remains general; this
    /// restriction is explicit instead of hiding a CAS evaluator in a rewrite.
    fn template_guard_matches(&self, guard: Cid, value: &Value) -> Result<bool, NetError> {
        match self
            .templates
            .get(&guard)
            .ok_or(NetError::MissingTemplate(guard))?
        {
            Node::Const(Atom::Bool(value)) => Ok(*value),
            Node::Param(0) => match value {
                Value::Bool(value) => Ok(*value),
                _ => Err(NetError::UnsupportedTemplate(
                    "bare parameter guard expected a boolean",
                )),
            },
            Node::Eq(left, right) => {
                if matches!(self.templates.get(left), Some(Node::Param(0))) {
                    return self.template_atom_equals(*right, value);
                }
                if matches!(self.templates.get(right), Some(Node::Param(0))) {
                    return self.template_atom_equals(*left, value);
                }
                Err(NetError::UnsupportedTemplate(
                    "INet guard is not equality on parameter zero",
                ))
            }
            _ => Err(NetError::UnsupportedTemplate(
                "INet guard is outside the first pattern subset",
            )),
        }
    }

    fn template_atom_equals(&self, cid: Cid, value: &Value) -> Result<bool, NetError> {
        let node = self
            .templates
            .get(&cid)
            .ok_or(NetError::MissingTemplate(cid))?;
        Ok(matches!(
            (node, value),
            (Node::Const(Atom::Int(left)), Value::Int(right)) if left == right
        ) || matches!(
            (node, value),
            (Node::Const(Atom::Bool(left)), Value::Bool(right)) if left == right
        ))
    }

    fn instantiate_template(
        &mut self,
        body: Cid,
        current_family: Cid,
        actual: Vec<Port>,
        continuation: Port,
    ) -> Result<(), NetError> {
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        collect_template_dag(&self.templates, body, &mut seen, &mut order)?;

        let mut demand = BTreeMap::<Cid, usize>::new();
        *demand.entry(body).or_default() += 1;
        for cid in &order {
            let node = self
                .templates
                .get(cid)
                .ok_or(NetError::MissingTemplate(*cid))?;
            for child in template_expression_children(node)? {
                *demand.entry(child).or_default() += 1;
            }
        }

        let mut parameter_cids = BTreeMap::<usize, Cid>::new();
        for cid in &order {
            if let Some(Node::Param(index)) = self.templates.get(cid) {
                parameter_cids.insert(usize::from(*index), *cid);
            }
        }
        if actual.len() != usize::from(self.template_family_parameters(current_family)?) {
            return Err(NetError::TemplateArity {
                family: current_family,
                expected: self.template_family_parameters(current_family)?,
                actual: actual.len(),
            });
        }

        let mut outlets = BTreeMap::<Cid, Vec<Port>>::new();
        for (index, source) in actual.into_iter().enumerate() {
            match parameter_cids.get(&index).copied() {
                Some(cid) => {
                    let count = demand.get(&cid).copied().unwrap_or(0);
                    outlets.insert(cid, self.fan_ports(source, count)?);
                }
                None => self.erase_port(source)?,
            }
        }

        for cid in order {
            let node = self
                .templates
                .get(&cid)
                .cloned()
                .ok_or(NetError::MissingTemplate(cid))?;
            if matches!(node, Node::Param(_)) {
                continue;
            }
            let source = match node {
                Node::Const(Atom::Int(value)) => {
                    let agent = self.add(AgentKind::Int(value));
                    self.principal(agent)?
                }
                Node::Const(Atom::Bool(value)) => {
                    let agent = self.add(AgentKind::Bool(value));
                    self.principal(agent)?
                }
                Node::Add(left, right) => {
                    let operator = self.add(AgentKind::AddPair);
                    let left = take_template_outlet(&mut outlets, left)?;
                    let right = take_template_outlet(&mut outlets, right)?;
                    self.connect(left, self.principal(operator)?)?;
                    self.connect(right, self.port(operator, 1)?)?;
                    self.port(operator, 2)?
                }
                Node::Mul(left, right) => {
                    let operator = self.add(AgentKind::MulPair);
                    let left = take_template_outlet(&mut outlets, left)?;
                    let right = take_template_outlet(&mut outlets, right)?;
                    self.connect(left, self.principal(operator)?)?;
                    self.connect(right, self.port(operator, 1)?)?;
                    self.port(operator, 2)?
                }
                Node::Dispatch { family, arguments } => {
                    self.build_template_call(family, arguments, &mut outlets)?
                }
                Node::Recur(arguments) => {
                    self.build_template_call(current_family, arguments, &mut outlets)?
                }
                _ => {
                    return Err(NetError::UnsupportedTemplate(
                        "selected body is outside the INet expression subset",
                    ));
                }
            };
            let count = demand.get(&cid).copied().unwrap_or(0);
            outlets.insert(cid, self.fan_ports(source, count)?);
        }

        let result = take_template_outlet(&mut outlets, body)?;
        if outlets.values().any(|ports| !ports.is_empty()) {
            return Err(NetError::UnsupportedTemplate(
                "selected template left unmatched wire demand",
            ));
        }
        self.connect(result, continuation)?;
        Ok(())
    }

    fn build_template_call(
        &mut self,
        family: Cid,
        arguments: Vec<Cid>,
        outlets: &mut BTreeMap<Cid, Vec<Port>>,
    ) -> Result<Port, NetError> {
        let parameters = self.template_family_parameters(family)?;
        if parameters == 0 || arguments.len() != usize::from(parameters) {
            return Err(NetError::TemplateArity {
                family,
                expected: parameters,
                actual: arguments.len(),
            });
        }
        let call = self.add(AgentKind::Call { family, parameters });
        for (index, argument) in arguments.into_iter().enumerate() {
            let source = take_template_outlet(outlets, argument)?;
            let target = if index == 0 {
                self.principal(call)?
            } else {
                self.port(call, index)?
            };
            self.connect(source, target)?;
        }
        self.port(call, usize::from(parameters))
    }

    fn template_family_parameters(&self, family: Cid) -> Result<u16, NetError> {
        match self.templates.get(&family) {
            Some(Node::Family { parameters, .. }) => Ok(*parameters),
            Some(_) => Err(NetError::UnsupportedTemplate(
                "template call target is not a family",
            )),
            None => Err(NetError::MissingTemplate(family)),
        }
    }

    fn fan_ports(&mut self, source: Port, count: usize) -> Result<Vec<Port>, NetError> {
        match count {
            0 => {
                self.erase_port(source)?;
                Ok(Vec::new())
            }
            1 => Ok(vec![source]),
            _ => {
                let fan = self.add(AgentKind::Fan);
                self.connect(source, self.principal(fan)?)?;
                let left_count = count / 2;
                let mut outputs = self.fan_ports(self.port(fan, 1)?, left_count)?;
                outputs.extend(self.fan_ports(self.port(fan, 2)?, count - left_count)?);
                Ok(outputs)
            }
        }
    }

    fn erase_port(&mut self, source: Port) -> Result<(), NetError> {
        let erase = self.add(AgentKind::Erase);
        self.connect(source, self.principal(erase)?)?;
        Ok(())
    }

    fn external(&self, port: Port) -> Result<Port, NetError> {
        let wire = self
            .agent(port.agent)?
            .ports
            .get(port.index)
            .copied()
            .flatten()
            .ok_or(NetError::Unconnected(port))?;
        let (left, right) = self
            .wires
            .get(wire.0)
            .and_then(|wire| wire.0)
            .ok_or(NetError::Unconnected(port))?;
        if left == port {
            Ok(right)
        } else if right == port {
            Ok(left)
        } else {
            Err(NetError::Unconnected(port))
        }
    }

    fn remove_agent(&mut self, id: AgentId) -> Result<(), NetError> {
        let wires = self
            .agent(id)?
            .ports
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for wire in wires {
            self.disconnect_wire(wire)?;
        }
        self.agents[id.0] = None;
        self.free_agents.insert(id.0);
        self.counters.deleted_agents += 1;
        Ok(())
    }

    fn disconnect_wire(&mut self, id: WireId) -> Result<(), NetError> {
        let ends = self
            .wires
            .get_mut(id.0)
            .ok_or(NetError::MissingWire(id))?
            .0
            .take();
        if let Some((left, right)) = ends {
            if let Ok(agent) = self.agent_mut(left.agent) {
                agent.ports[left.index] = None;
            }
            if let Ok(agent) = self.agent_mut(right.agent) {
                agent.ports[right.index] = None;
            }
            self.free_wires.insert(id.0);
        }
        Ok(())
    }

    fn check_free_port(&self, port: Port) -> Result<(), NetError> {
        let agent = self.agent(port.agent)?;
        let wire = agent.ports.get(port.index).ok_or(NetError::BadPort {
            agent: port.agent,
            index: port.index,
        })?;
        if wire.is_some() {
            Err(NetError::PortOccupied(port))
        } else {
            Ok(())
        }
    }

    fn agent(&self, id: AgentId) -> Result<&Agent, NetError> {
        self.agents
            .get(id.0)
            .and_then(Option::as_ref)
            .ok_or(NetError::MissingAgent(id))
    }

    fn agent_mut(&mut self, id: AgentId) -> Result<&mut Agent, NetError> {
        self.agents
            .get_mut(id.0)
            .and_then(Option::as_mut)
            .ok_or(NetError::MissingAgent(id))
    }
}

fn collect_template_dag(
    templates: &BTreeMap<Cid, Node>,
    root: Cid,
    seen: &mut BTreeSet<Cid>,
    order: &mut Vec<Cid>,
) -> Result<(), NetError> {
    if !seen.insert(root) {
        return Ok(());
    }
    let node = templates
        .get(&root)
        .ok_or(NetError::MissingTemplate(root))?;
    for child in template_expression_children(node)? {
        collect_template_dag(templates, child, seen, order)?;
    }
    order.push(root);
    Ok(())
}

fn template_expression_children(node: &Node) -> Result<Vec<Cid>, NetError> {
    match node {
        Node::Const(Atom::Int(_)) | Node::Const(Atom::Bool(_)) | Node::Param(_) => Ok(Vec::new()),
        Node::Add(left, right) | Node::Mul(left, right) => Ok(vec![*left, *right]),
        Node::Dispatch { arguments, .. } | Node::Recur(arguments) => Ok(arguments.clone()),
        _ => Err(NetError::UnsupportedTemplate(
            "selected body is outside the INet expression subset",
        )),
    }
}

fn take_template_outlet(
    outlets: &mut BTreeMap<Cid, Vec<Port>>,
    cid: Cid,
) -> Result<Port, NetError> {
    outlets
        .get_mut(&cid)
        .and_then(Vec::pop)
        .ok_or(NetError::TemplateWireDemand(cid))
}

fn has_rule(left: &AgentKind, right: &AgentKind) -> bool {
    match (left.value(), right.value()) {
        (Some(value), _) => value_rule(&value, right),
        (_, Some(value)) => value_rule(&value, left),
        _ => false,
    }
}

fn value_rule(value: &Value, other: &AgentKind) -> bool {
    matches!(
        (value, other),
        (Value::Int(_), AgentKind::Add(_))
            | (Value::Int(_), AgentKind::Mul(_))
            | (Value::Int(_), AgentKind::AddPair)
            | (Value::Int(_), AgentKind::MulPair)
            | (Value::Bool(_), AgentKind::ChooseInt { .. })
            | (Value::World(_), AgentKind::Emit(_))
            | (Value::Int(_) | Value::Bool(_), AgentKind::Fan)
            | (Value::Int(_) | Value::Bool(_), AgentKind::Erase)
            | (_, AgentKind::Call { .. })
            | (_, AgentKind::Output(_))
    )
}

fn encode_port(out: &mut Vec<u8>, port: Port) {
    put_u64(out, port.agent.0 as u64);
    put_u64(out, port.index as u64);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetError {
    MissingAgent(AgentId),
    MissingWire(WireId),
    BadPort {
        agent: AgentId,
        index: usize,
    },
    PortOccupied(Port),
    Unconnected(Port),
    NoRule(AgentKind, AgentKind),
    DuplicateOutput(String),
    IntegerOverflow(&'static str),
    FuelExhausted {
        limit: usize,
    },
    MissingTemplate(Cid),
    UnsupportedTemplate(&'static str),
    NoMatchingClause(Cid),
    TemplateArity {
        family: Cid,
        expected: u16,
        actual: usize,
    },
    TemplateWireDemand(Cid),
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for NetError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Atom, Bindings, Node, Store};
    use crate::reduce::Reducer;

    fn connect_principals(net: &mut Net, left: AgentId, right: AgentId) {
        net.connect(net.principal(left).unwrap(), net.principal(right).unwrap())
            .unwrap();
    }

    fn unary_pipeline(source: AgentKind, operators: &[AgentKind], label: &str) -> Net {
        Net::unary_pipeline(source, operators, label).unwrap()
    }

    #[test]
    fn compile_blocks_at_a_hole_and_runtime_continues_the_same_net() {
        let mut net = unary_pipeline(
            AgentKind::Hole("x".into()),
            &[AgentKind::Add(1), AgentKind::Mul(2)],
            "result",
        );
        let source = net.snapshot_cid();
        let compile = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(compile.rewrites, 0);
        assert_eq!(net.snapshot_cid(), source);

        net.bind(&BTreeMap::from([("x".into(), Value::Int(41))]));
        let runtime = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(84)));
        assert_eq!(runtime.rewrites, 3);
        assert_eq!(runtime.fresh_agent_cells, 0);
        assert_eq!(runtime.reused_agent_cells, 2);
        assert_eq!(runtime.deleted_agents, 6);
        assert_eq!(net.live_agents(), 0);
    }

    #[test]
    fn compile_context_can_choose_and_reduce_a_static_path() {
        let mut net = unary_pipeline(
            AgentKind::Hole("fast".into()),
            &[
                AgentKind::ChooseInt {
                    when_true: 20,
                    when_false: 10,
                },
                AgentKind::Add(1),
            ],
            "result",
        );
        net.bind(&BTreeMap::from([("fast".into(), Value::Bool(true))]));
        let compile = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(compile.rewrites, 3);
        assert_eq!(net.outputs().get("result"), Some(&Value::Int(21)));
    }

    #[test]
    fn unsupported_active_pair_is_an_error_not_silent_stuck_state() {
        let mut net = unary_pipeline(AgentKind::Bool(true), &[AgentKind::Add(1)], "result");
        assert_eq!(
            net.reduce(100, Schedule::LowestWire),
            Err(NetError::NoRule(AgentKind::Bool(true), AgentKind::Add(1)))
        );
        assert!(net.outputs().is_empty());
    }

    #[test]
    fn exhausting_fuel_is_an_error_not_a_successful_partial_net() {
        let mut net = unary_pipeline(
            AgentKind::Int(1),
            &[AgentKind::Add(1), AgentKind::Add(1)],
            "result",
        );
        assert_eq!(
            net.reduce(1, Schedule::LowestWire),
            Err(NetError::FuelExhausted { limit: 1 })
        );
        assert!(net.outputs().is_empty());
    }

    #[test]
    fn effect_tokens_preserve_order_as_local_rewrites() {
        let mut net = unary_pipeline(
            AgentKind::Hole("world".into()),
            &[
                AgentKind::Emit("hello".into()),
                AgentKind::Emit("goodbye".into()),
            ],
            "world",
        );
        net.bind(&BTreeMap::from([(
            "world".into(),
            Value::World(Vec::new()),
        )]));
        net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(
            net.outputs().get("world"),
            Some(&Value::World(vec!["hello".into(), "goodbye".into()]))
        );
    }

    #[test]
    fn fan_and_erase_make_copying_and_destruction_explicit() {
        let mut net = Net::new();
        let value = net.add(AgentKind::Int(7));
        let fan = net.add(AgentKind::Fan);
        let left = net.add(AgentKind::Output("left".into()));
        let right = net.add(AgentKind::Erase);
        connect_principals(&mut net, value, fan);
        net.connect(net.port(fan, 1).unwrap(), net.principal(left).unwrap())
            .unwrap();
        net.connect(net.port(fan, 2).unwrap(), net.principal(right).unwrap())
            .unwrap();

        let stats = net.reduce(100, Schedule::LowestWire).unwrap();
        assert_eq!(net.outputs().get("left"), Some(&Value::Int(7)));
        assert_eq!(stats.rewrites, 3);
        assert_eq!(stats.fresh_agent_cells, 0);
        assert_eq!(stats.reused_agent_cells, 2);
        assert_eq!(net.live_agents(), 0);
    }

    #[test]
    fn independent_redex_schedule_preserves_observable_results() {
        let mut net = Net::new();
        for (label, value) in [("a", 1), ("b", 10)] {
            let source = net.add(AgentKind::Int(value));
            let add = net.add(AgentKind::Add(1));
            let output = net.add(AgentKind::Output(label.into()));
            connect_principals(&mut net, source, add);
            net.connect(net.port(add, 1).unwrap(), net.principal(output).unwrap())
                .unwrap();
        }
        let mut low = net.clone();
        let mut high = net;
        low.reduce(100, Schedule::LowestWire).unwrap();
        high.reduce(100, Schedule::HighestWire).unwrap();
        assert_eq!(low.outputs(), high.outputs());
        assert_eq!(low.snapshot_cid(), high.snapshot_cid());
    }

    #[test]
    fn port_net_matches_the_reference_graph_across_runtime_inputs() {
        let mut store = Store::new();
        let input = store.intern(Node::Hole("x".into()));
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        let plus = store.intern(Node::Add(input, one));
        let program = store.intern(Node::Mul(plus, two));

        for value in -32..=32 {
            let literal = store.intern(Node::Const(Atom::Int(value)));
            let mut bindings = Bindings::new();
            bindings.insert("x", literal);
            let reference = Reducer::new(&mut store, &bindings).run(program).unwrap();
            let expected = match store.get(reference.root) {
                Some(Node::Const(Atom::Int(value))) => *value,
                other => panic!("unexpected reference result: {other:?}"),
            };

            let mut net = unary_pipeline(
                AgentKind::Hole("x".into()),
                &[AgentKind::Add(1), AgentKind::Mul(2)],
                "result",
            );
            net.bind(&BTreeMap::from([("x".into(), Value::Int(value))]));
            net.reduce(100, Schedule::LowestWire).unwrap();
            assert_eq!(net.outputs().get("result"), Some(&Value::Int(expected)));
        }
    }
}
