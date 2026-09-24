use crate::cid::{Cid, put_cid};
use crate::net::{Atom, Bindings, Clause, Node, Store};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReductionStats {
    pub steps: usize,
    pub visited: usize,
    pub rewritten: usize,
    pub memo_hits: usize,
    /// Maximum number of pending continuation/work frames in any iterative
    /// reducer traversal during this run.
    pub peak_frames: usize,
    /// Number of guarded bodies actually instantiated.  Unselected and
    /// unresolved clauses do not contribute.
    pub clauses_instantiated: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reduction {
    pub root: Cid,
    pub stats: ReductionStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Specialization {
    pub source: Cid,
    pub context: Cid,
    pub reducer: Cid,
    pub cache_key: Cid,
    pub residual: Cid,
    pub stats: ReductionStats,
    pub cache_hit: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpecializationCache {
    entries: BTreeMap<Cid, Cid>,
}

impl SpecializationCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn specialize(
        &mut self,
        store: &mut Store,
        root: Cid,
        bindings: &Bindings,
        step_limit: usize,
    ) -> Result<Specialization, ReduceError> {
        let source = store.artifact_cid(&[root]);
        let context = bindings.cid();
        let reducer = reducer_cid();
        let cache_key = specialization_key(source, context, reducer);
        if let Some(residual) = self.entries.get(&cache_key).copied() {
            if store.get(residual).is_none() {
                return Err(ReduceError::MissingNode(residual));
            }
            return Ok(Specialization {
                source,
                context,
                reducer,
                cache_key,
                residual,
                stats: ReductionStats::default(),
                cache_hit: true,
            });
        }

        let mut specialization =
            Reducer::specialize_with_budget(store, root, bindings, step_limit)?;
        self.entries
            .insert(specialization.cache_key, specialization.residual);
        specialization.cache_hit = false;
        Ok(specialization)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReduceError {
    MissingNode(Cid),
    Cycle(Cid),
    Arity { expected: u16, actual: usize },
    ParameterOutOfRange { index: u16, arguments: usize },
    Type(&'static str),
    IntegerOverflow(&'static str),
    LinearValueDuplicated(Cid),
    BudgetExhausted { limit: usize },
    EmptyFamily,
    FamilyArity { expected: u16, actual: usize },
    NoMatchingClause,
    UnboundRecursion,
    OpenCodeValue(String),
    ImpureGuard(Cid),
}

impl fmt::Display for ReduceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNode(cid) => write!(f, "missing node {cid}"),
            Self::Cycle(cid) => write!(f, "cyclic content graph at {cid}"),
            Self::Arity { expected, actual } => {
                write!(f, "quotation expects {expected} arguments, got {actual}")
            }
            Self::ParameterOutOfRange { index, arguments } => {
                write!(f, "parameter {index} is not in {arguments} arguments")
            }
            Self::Type(message) => f.write_str(message),
            Self::IntegerOverflow(operation) => {
                write!(f, "integer overflow in {operation}")
            }
            Self::LinearValueDuplicated(cid) => {
                write!(
                    f,
                    "linear effect value {cid} is duplicated or captured by reusable code"
                )
            }
            Self::BudgetExhausted { limit } => {
                write!(f, "reduction exceeded its explicit {limit}-step budget")
            }
            Self::EmptyFamily => f.write_str("guarded definition family has no clauses"),
            Self::FamilyArity { expected, actual } => {
                write!(
                    f,
                    "guarded family expects {expected} arguments, got {actual}"
                )
            }
            Self::NoMatchingClause => f.write_str("no guarded-family clause matched"),
            Self::UnboundRecursion => {
                f.write_str("recur appeared outside a selected family clause")
            }
            Self::OpenCodeValue(name) => {
                write!(f, "code value contains free named hole {name:?}")
            }
            Self::ImpureGuard(cid) => {
                write!(f, "guard {cid} is outside the pure guard subset")
            }
        }
    }
}

impl std::error::Error for ReduceError {}

pub struct Reducer<'a> {
    store: &'a mut Store,
    bindings: &'a Bindings,
    memo: BTreeMap<Cid, Cid>,
    ground_memo: BTreeMap<(Cid, bool), bool>,
    strict_guard_parameters: BTreeMap<Cid, BTreeSet<u16>>,
    linearity_summaries: BTreeMap<Cid, LinearitySummary>,
    carries_linear: BTreeMap<Cid, Option<Cid>>,
    visiting: BTreeSet<Cid>,
    stats: ReductionStats,
    remaining_steps: usize,
    step_limit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CodeScope {
    Family { parameters: u16, recur: bool },
    Quote { parameters: u16 },
}

#[derive(Clone, Copy, Debug)]
enum BinaryFrameOp {
    Add,
    Mul,
    Eq,
    Pair,
}

#[derive(Clone, Debug, Default)]
struct LinearitySummary {
    parameter_uses: BTreeMap<u16, usize>,
    token_parameters: BTreeSet<u16>,
}

#[derive(Debug)]
enum EvalFrame {
    Enter(Cid),
    Return {
        source: Cid,
    },
    Binary {
        source: Cid,
        operation: BinaryFrameOp,
    },
    If {
        source: Cid,
        when_true: Cid,
        when_false: Cid,
    },
    First {
        source: Cid,
    },
    Second {
        source: Cid,
    },
    Record {
        source: Cid,
        names: Vec<String>,
    },
    Get {
        source: Cid,
        field: String,
    },
    Put {
        source: Cid,
        field: String,
    },
    Apply {
        source: Cid,
        arguments: Vec<Cid>,
    },
    Emit {
        source: Cid,
    },
    DispatchFamily {
        source: Cid,
        arguments: Vec<Cid>,
    },
    DispatchNext {
        source: Cid,
        family: Cid,
        arguments: Vec<Cid>,
        clauses: Vec<Clause>,
        index: usize,
        demanded: BTreeSet<u16>,
    },
    DispatchGuard {
        source: Cid,
        family: Cid,
        arguments: Vec<Cid>,
        clauses: Vec<Clause>,
        index: usize,
        demanded: BTreeSet<u16>,
    },
}

#[derive(Clone, Copy, Debug)]
enum RewriteMode<'a> {
    Substitute { arguments: &'a [Cid] },
    FamilyBody { arguments: &'a [Cid], family: Cid },
    Bindings,
}

#[derive(Debug)]
enum RewriteFrame {
    Enter(Cid),
    Exit {
        source: Cid,
        node: Node,
        children: usize,
    },
}

pub const DEFAULT_REDUCTION_BUDGET: usize = 512;

impl<'a> Reducer<'a> {
    pub fn new(store: &'a mut Store, bindings: &'a Bindings) -> Self {
        Self::with_budget(store, bindings, DEFAULT_REDUCTION_BUDGET)
    }

    pub fn with_budget(store: &'a mut Store, bindings: &'a Bindings, step_limit: usize) -> Self {
        Self {
            store,
            bindings,
            memo: BTreeMap::new(),
            ground_memo: BTreeMap::new(),
            strict_guard_parameters: BTreeMap::new(),
            linearity_summaries: BTreeMap::new(),
            carries_linear: BTreeMap::new(),
            visiting: BTreeSet::new(),
            stats: ReductionStats::default(),
            remaining_steps: step_limit,
            step_limit,
        }
    }

    pub fn run(mut self, root: Cid) -> Result<Reduction, ReduceError> {
        let mut linear_roots = Vec::with_capacity(self.bindings.0.len() + 1);
        linear_roots.push(root);
        linear_roots.extend(self.bindings.0.values().copied());
        self.validate_linearity_roots(&linear_roots)?;
        let root = self.reduce(root)?;
        self.validate_linearity_roots(&[root])?;
        Ok(Reduction {
            root,
            stats: self.stats,
        })
    }

    pub fn specialize(
        store: &'a mut Store,
        root: Cid,
        bindings: &'a Bindings,
    ) -> Result<Specialization, ReduceError> {
        Self::specialize_with_budget(store, root, bindings, DEFAULT_REDUCTION_BUDGET)
    }

    pub fn specialize_with_budget(
        store: &'a mut Store,
        root: Cid,
        bindings: &'a Bindings,
        step_limit: usize,
    ) -> Result<Specialization, ReduceError> {
        let source = store.artifact_cid(&[root]);
        let context = bindings.cid();
        let reducer = reducer_cid();
        let cache_key = specialization_key(source, context, reducer);
        let reduced = Self::with_budget(store, bindings, step_limit).run(root)?;
        Ok(Specialization {
            source,
            context,
            reducer,
            cache_key,
            residual: reduced.root,
            stats: reduced.stats,
            cache_hit: false,
        })
    }

    fn charge(&mut self) -> Result<(), ReduceError> {
        if self.remaining_steps == 0 {
            return Err(ReduceError::BudgetExhausted {
                limit: self.step_limit,
            });
        }
        self.remaining_steps -= 1;
        self.stats.steps += 1;
        Ok(())
    }

    fn reduce(&mut self, root: Cid) -> Result<Cid, ReduceError> {
        let mut frames = vec![EvalFrame::Enter(root)];
        let mut values = Vec::new();

        loop {
            self.stats.peak_frames = self.stats.peak_frames.max(frames.len());
            let Some(frame) = frames.pop() else {
                break;
            };
            match frame {
                EvalFrame::Enter(cid) => {
                    if let Some(result) = self.memo.get(&cid).copied() {
                        self.stats.memo_hits += 1;
                        values.push(result);
                        continue;
                    }

                    self.charge()?;
                    if !self.visiting.insert(cid) {
                        return Err(ReduceError::Cycle(cid));
                    }
                    self.stats.visited += 1;
                    let node = self
                        .store
                        .get(cid)
                        .cloned()
                        .ok_or(ReduceError::MissingNode(cid))?;

                    match node {
                        Node::Const(_) | Node::Param(_) => {
                            self.finish_evaluation(cid, cid, &mut values);
                        }
                        Node::Hole(name) => match self.bindings.get(&name) {
                            Some(value) => {
                                frames.push(EvalFrame::Return { source: cid });
                                frames.push(EvalFrame::Enter(value));
                            }
                            None => self.finish_evaluation(cid, cid, &mut values),
                        },
                        Node::Add(left, right) => {
                            Self::schedule_binary(&mut frames, cid, BinaryFrameOp::Add, left, right)
                        }
                        Node::Mul(left, right) => {
                            Self::schedule_binary(&mut frames, cid, BinaryFrameOp::Mul, left, right)
                        }
                        Node::Eq(left, right) => {
                            Self::schedule_binary(&mut frames, cid, BinaryFrameOp::Eq, left, right)
                        }
                        Node::If {
                            condition,
                            when_true,
                            when_false,
                        } => {
                            frames.push(EvalFrame::If {
                                source: cid,
                                when_true,
                                when_false,
                            });
                            frames.push(EvalFrame::Enter(condition));
                        }
                        Node::Pair(left, right) => Self::schedule_binary(
                            &mut frames,
                            cid,
                            BinaryFrameOp::Pair,
                            left,
                            right,
                        ),
                        Node::First(pair) => {
                            frames.push(EvalFrame::First { source: cid });
                            frames.push(EvalFrame::Enter(pair));
                        }
                        Node::Second(pair) => {
                            frames.push(EvalFrame::Second { source: cid });
                            frames.push(EvalFrame::Enter(pair));
                        }
                        Node::Record(fields) => {
                            let (names, children): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
                            frames.push(EvalFrame::Record { source: cid, names });
                            for child in children.into_iter().rev() {
                                frames.push(EvalFrame::Enter(child));
                            }
                        }
                        Node::Get { record, field } => {
                            frames.push(EvalFrame::Get { source: cid, field });
                            frames.push(EvalFrame::Enter(record));
                        }
                        Node::Put {
                            record,
                            field,
                            value,
                        } => {
                            frames.push(EvalFrame::Put { source: cid, field });
                            frames.push(EvalFrame::Enter(value));
                            frames.push(EvalFrame::Enter(record));
                        }
                        Node::Quote { .. } => {
                            self.validate_code_value(cid)?;
                            self.finish_evaluation(cid, cid, &mut values);
                        }
                        Node::Apply {
                            function,
                            arguments,
                        } => {
                            frames.push(EvalFrame::Apply {
                                source: cid,
                                arguments,
                            });
                            frames.push(EvalFrame::Enter(function));
                        }
                        Node::Emit { token, message } => {
                            frames.push(EvalFrame::Emit { source: cid });
                            frames.push(EvalFrame::Enter(message));
                            frames.push(EvalFrame::Enter(token));
                        }
                        Node::Family { .. } => {
                            self.validate_code_value(cid)?;
                            self.finish_evaluation(cid, cid, &mut values);
                        }
                        Node::Dispatch { family, arguments } => {
                            frames.push(EvalFrame::DispatchFamily {
                                source: cid,
                                arguments,
                            });
                            frames.push(EvalFrame::Enter(family));
                        }
                        Node::Recur(_) => return Err(ReduceError::UnboundRecursion),
                    }
                }
                EvalFrame::Return { source } => {
                    let result = Self::pop_value(&mut values);
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Binary { source, operation } => {
                    let right = Self::pop_value(&mut values);
                    let left = Self::pop_value(&mut values);
                    let result = match operation {
                        BinaryFrameOp::Add => match (
                            self.store.get(left).cloned(),
                            self.store.get(right).cloned(),
                        ) {
                            (
                                Some(Node::Const(Atom::Int(left))),
                                Some(Node::Const(Atom::Int(right))),
                            ) => {
                                let value = left
                                    .checked_add(right)
                                    .ok_or(ReduceError::IntegerOverflow("add"))?;
                                self.store.intern(Node::Const(Atom::Int(value)))
                            }
                            (left_node, right_node)
                                if (left_node.as_ref().is_some_and(|node| {
                                    !matches!(node, Node::Const(Atom::Int(_)))
                                }) && self.is_ground(left)?)
                                    || (right_node.as_ref().is_some_and(|node| {
                                        !matches!(node, Node::Const(Atom::Int(_)))
                                    }) && self.is_ground(right)?) =>
                            {
                                return Err(ReduceError::Type("add expects two integers"));
                            }
                            _ => self.store.intern(Node::Add(left, right)),
                        },
                        BinaryFrameOp::Mul => match (
                            self.store.get(left).cloned(),
                            self.store.get(right).cloned(),
                        ) {
                            (
                                Some(Node::Const(Atom::Int(left))),
                                Some(Node::Const(Atom::Int(right))),
                            ) => {
                                let value = left
                                    .checked_mul(right)
                                    .ok_or(ReduceError::IntegerOverflow("multiply"))?;
                                self.store.intern(Node::Const(Atom::Int(value)))
                            }
                            (left_node, right_node)
                                if (left_node.as_ref().is_some_and(|node| {
                                    !matches!(node, Node::Const(Atom::Int(_)))
                                }) && self.is_ground(left)?)
                                    || (right_node.as_ref().is_some_and(|node| {
                                        !matches!(node, Node::Const(Atom::Int(_)))
                                    }) && self.is_ground(right)?) =>
                            {
                                return Err(ReduceError::Type("multiply expects two integers"));
                            }
                            _ => self.store.intern(Node::Mul(left, right)),
                        },
                        BinaryFrameOp::Eq => {
                            if self.is_ground(left)? && self.is_ground(right)? {
                                self.store.intern(Node::Const(Atom::Bool(left == right)))
                            } else {
                                self.store.intern(Node::Eq(left, right))
                            }
                        }
                        BinaryFrameOp::Pair => self.store.intern(Node::Pair(left, right)),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::If {
                    source,
                    when_true,
                    when_false,
                } => {
                    let condition = Self::pop_value(&mut values);
                    match self.store.get(condition).cloned() {
                        Some(Node::Const(Atom::Bool(true))) => {
                            frames.push(EvalFrame::Return { source });
                            frames.push(EvalFrame::Enter(when_true));
                        }
                        Some(Node::Const(Atom::Bool(false))) => {
                            frames.push(EvalFrame::Return { source });
                            frames.push(EvalFrame::Enter(when_false));
                        }
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("if condition is not a boolean"));
                        }
                        _ => {
                            let mut memo = BTreeMap::new();
                            let when_true = self.instantiate_bindings(when_true, &mut memo)?;
                            let when_false = self.instantiate_bindings(when_false, &mut memo)?;
                            let result = self.store.intern(Node::If {
                                condition,
                                when_true,
                                when_false,
                            });
                            self.finish_evaluation(source, result, &mut values);
                        }
                    }
                }
                EvalFrame::First { source } => {
                    let pair = Self::pop_value(&mut values);
                    let result = match self.store.get(pair).cloned() {
                        Some(Node::Pair(first, _)) => first,
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("first operand is not a pair"));
                        }
                        _ if self.is_ground(pair)? => {
                            return Err(ReduceError::Type("first operand is not a pair"));
                        }
                        _ => self.store.intern(Node::First(pair)),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Second { source } => {
                    let pair = Self::pop_value(&mut values);
                    let result = match self.store.get(pair).cloned() {
                        Some(Node::Pair(_, second)) => second,
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("second operand is not a pair"));
                        }
                        _ if self.is_ground(pair)? => {
                            return Err(ReduceError::Type("second operand is not a pair"));
                        }
                        _ => self.store.intern(Node::Second(pair)),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Record { source, names } => {
                    let first = values
                        .len()
                        .checked_sub(names.len())
                        .expect("record evaluation values");
                    let children = values.split_off(first);
                    let result = self
                        .store
                        .intern(Node::Record(names.into_iter().zip(children).collect()));
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Get { source, field } => {
                    let record = Self::pop_value(&mut values);
                    let result = match self.store.get(record).cloned() {
                        Some(Node::Record(fields)) => fields
                            .into_iter()
                            .find(|(name, _)| name == &field)
                            .map(|(_, value)| value)
                            .ok_or(ReduceError::Type("record field does not exist"))?,
                        Some(Node::Const(_)) | Some(Node::Pair(_, _)) => {
                            return Err(ReduceError::Type("get operand is not a record"));
                        }
                        _ if self.is_ground(record)? => {
                            return Err(ReduceError::Type("get operand is not a record"));
                        }
                        _ => self.store.intern(Node::Get { record, field }),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Put { source, field } => {
                    let value = Self::pop_value(&mut values);
                    let record = Self::pop_value(&mut values);
                    let result = match self.store.get(record).cloned() {
                        Some(Node::Record(mut fields)) => {
                            match fields.iter_mut().find(|(name, _)| name == &field) {
                                Some((_, old)) => *old = value,
                                None => fields.push((field, value)),
                            }
                            self.store.intern(Node::Record(fields))
                        }
                        Some(Node::Const(_)) | Some(Node::Pair(_, _)) => {
                            return Err(ReduceError::Type("put operand is not a record"));
                        }
                        _ if self.is_ground(record)? => {
                            return Err(ReduceError::Type("put operand is not a record"));
                        }
                        _ => self.store.intern(Node::Put {
                            record,
                            field,
                            value,
                        }),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::Apply { source, arguments } => {
                    let function = Self::pop_value(&mut values);
                    let mut binding_memo = BTreeMap::new();
                    let arguments = arguments
                        .into_iter()
                        .map(|argument| self.instantiate_bindings(argument, &mut binding_memo))
                        .collect::<Result<Vec<_>, _>>()?;
                    match self.store.get(function).cloned() {
                        Some(Node::Quote { params, body }) => {
                            if usize::from(params) != arguments.len() {
                                return Err(ReduceError::Arity {
                                    expected: params,
                                    actual: arguments.len(),
                                });
                            }
                            self.validate_instantiation_linearity(body, &arguments)?;
                            let body = self.substitute(body, &arguments, &mut BTreeMap::new())?;
                            frames.push(EvalFrame::Return { source });
                            frames.push(EvalFrame::Enter(body));
                        }
                        _ if self.is_ground(function)? => {
                            return Err(ReduceError::Type("apply operand is not a quotation"));
                        }
                        _ => {
                            let result = self.store.intern(Node::Apply {
                                function,
                                arguments,
                            });
                            self.finish_evaluation(source, result, &mut values);
                        }
                    }
                }
                EvalFrame::Emit { source } => {
                    let message = Self::pop_value(&mut values);
                    let token = Self::pop_value(&mut values);
                    let result = match (
                        self.store.get(token).cloned(),
                        self.store.get(message).cloned(),
                    ) {
                        (
                            Some(Node::Const(Atom::Trace(mut entries))),
                            Some(Node::Const(Atom::Text(message))),
                        ) => {
                            entries.push(message);
                            self.store.intern(Node::Const(Atom::Trace(entries)))
                        }
                        (Some(Node::Const(_)), Some(Node::Const(_))) => {
                            return Err(ReduceError::Type(
                                "emit expects an effect trace token and text message",
                            ));
                        }
                        _ => self.store.intern(Node::Emit { token, message }),
                    };
                    self.finish_evaluation(source, result, &mut values);
                }
                EvalFrame::DispatchFamily { source, arguments } => {
                    let family = Self::pop_value(&mut values);
                    let node = self
                        .store
                        .get(family)
                        .cloned()
                        .ok_or(ReduceError::MissingNode(family))?;
                    match node {
                        Node::Family {
                            parameters,
                            clauses,
                        } => {
                            if clauses.is_empty() {
                                return Err(ReduceError::EmptyFamily);
                            }
                            if usize::from(parameters) != arguments.len() {
                                return Err(ReduceError::FamilyArity {
                                    expected: parameters,
                                    actual: arguments.len(),
                                });
                            }
                            frames.push(EvalFrame::DispatchNext {
                                source,
                                family,
                                arguments,
                                clauses,
                                index: 0,
                                demanded: BTreeSet::new(),
                            });
                        }
                        _ if self.is_ground(family)? => {
                            return Err(ReduceError::Type(
                                "dispatch target is not a guarded definition family",
                            ));
                        }
                        _ => {
                            let result = self.residual_dispatch(family, &arguments)?;
                            self.finish_evaluation(source, result, &mut values);
                        }
                    }
                }
                EvalFrame::DispatchNext {
                    source,
                    family,
                    arguments,
                    clauses,
                    index,
                    mut demanded,
                } => {
                    let Some(clause) = clauses.get(index).cloned() else {
                        return Err(ReduceError::NoMatchingClause);
                    };
                    demanded.extend(self.strict_guard_parameters(clause.guard)?);
                    let guard = self.substitute(clause.guard, &arguments, &mut BTreeMap::new())?;
                    frames.push(EvalFrame::DispatchGuard {
                        source,
                        family,
                        arguments,
                        clauses,
                        index,
                        demanded,
                    });
                    frames.push(EvalFrame::Enter(guard));
                }
                EvalFrame::DispatchGuard {
                    source,
                    family,
                    arguments,
                    clauses,
                    index,
                    demanded,
                } => {
                    let guard = Self::pop_value(&mut values);
                    match self.store.get(guard).cloned() {
                        Some(Node::Const(Atom::Bool(false))) => {
                            frames.push(EvalFrame::DispatchNext {
                                source,
                                family,
                                arguments,
                                clauses,
                                index: index + 1,
                                demanded,
                            });
                        }
                        Some(Node::Const(Atom::Bool(true))) => {
                            self.stats.clauses_instantiated += 1;
                            let body = clauses.get(index).expect("guarded clause index").body;
                            // Share only reductions demanded in strict positions of the
                            // guards actually evaluated.  This is independent of unrelated
                            // memo-table history, while undemanded arguments remain lazy.
                            let arguments = arguments
                                .into_iter()
                                .enumerate()
                                .map(|(index, argument)| {
                                    let index = u16::try_from(index)
                                        .expect("family arity was represented by u16");
                                    if demanded.contains(&index) {
                                        self.memo.get(&argument).copied().unwrap_or(argument)
                                    } else {
                                        argument
                                    }
                                })
                                .collect::<Vec<_>>();
                            self.validate_instantiation_linearity(body, &arguments)?;
                            let selected = self.substitute_family_body(
                                body,
                                &arguments,
                                family,
                                &mut BTreeMap::new(),
                            )?;
                            frames.push(EvalFrame::Return { source });
                            frames.push(EvalFrame::Enter(selected));
                        }
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type(
                                "guarded-family guard did not produce a boolean",
                            ));
                        }
                        _ if self.is_ground(guard)? => {
                            return Err(ReduceError::Type(
                                "guarded-family guard did not produce a boolean",
                            ));
                        }
                        _ => {
                            let result = self.residual_dispatch(family, &arguments)?;
                            self.finish_evaluation(source, result, &mut values);
                        }
                    }
                }
            }
        }

        assert_eq!(values.len(), 1, "iterative reducer value-stack imbalance");
        Ok(values.pop().expect("root reduction result"))
    }

    fn schedule_binary(
        frames: &mut Vec<EvalFrame>,
        source: Cid,
        operation: BinaryFrameOp,
        left: Cid,
        right: Cid,
    ) {
        frames.push(EvalFrame::Binary { source, operation });
        frames.push(EvalFrame::Enter(right));
        frames.push(EvalFrame::Enter(left));
    }

    fn pop_value(values: &mut Vec<Cid>) -> Cid {
        values.pop().expect("evaluation frame value")
    }

    fn finish_evaluation(&mut self, source: Cid, result: Cid, values: &mut Vec<Cid>) {
        let removed = self.visiting.remove(&source);
        debug_assert!(removed, "finished node was not marked active");
        if result != source {
            self.stats.rewritten += 1;
        }
        self.memo.insert(source, result);
        values.push(result);
    }

    /// Parameters in these positions are demanded on every successful path
    /// through this guard.  Branch bodies stay non-strict even when a
    /// particular evaluation happens to select one of them; this keeps body
    /// normalization independent of unrelated memo-table history.
    fn strict_guard_parameters(&mut self, root: Cid) -> Result<BTreeSet<u16>, ReduceError> {
        if let Some(parameters) = self.strict_guard_parameters.get(&root) {
            return Ok(parameters.clone());
        }

        let mut parameters = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(cid) = pending.pop() {
            if !seen.insert(cid) {
                continue;
            }
            self.charge()?;
            let node = self
                .store
                .get(cid)
                .cloned()
                .ok_or(ReduceError::MissingNode(cid))?;
            match node {
                Node::Param(index) => {
                    parameters.insert(index);
                }
                Node::If { condition, .. } => pending.push(condition),
                Node::Const(_) | Node::Hole(_) => {}
                // Guard validation rejects these cases before dispatch.  Keep
                // them as boundaries here so this analysis cannot accidentally
                // make dormant code strict if validation is reordered later.
                Node::Quote { .. } | Node::Family { .. } => {}
                other => {
                    let children = other.children();
                    pending.extend(children.into_iter().rev());
                }
            }
        }
        self.strict_guard_parameters
            .insert(root, parameters.clone());
        Ok(parameters)
    }

    /// Check only the new aliasing introduced by substituting arguments into a
    /// previously validated code template.  The template summary and the
    /// "contains a linear value" property are immutable CID facts cached for
    /// this run, so a loop carrying a growing lazy accumulator does not rescan
    /// its entire history at every call.
    fn validate_instantiation_linearity(
        &mut self,
        template: Cid,
        arguments: &[Cid],
    ) -> Result<(), ReduceError> {
        let summary = self.linearity_summary(template)?;
        let mut argument_uses = BTreeMap::<Cid, (usize, bool)>::new();
        for (index, uses) in summary.parameter_uses {
            let argument = arguments.get(usize::from(index)).copied().ok_or(
                ReduceError::ParameterOutOfRange {
                    index,
                    arguments: arguments.len(),
                },
            )?;
            let entry = argument_uses.entry(argument).or_default();
            entry.0 = entry.0.saturating_add(uses);
            entry.1 |= summary.token_parameters.contains(&index);
        }

        for (argument, (uses, used_as_token)) in argument_uses {
            if uses <= 1 {
                continue;
            }
            if used_as_token {
                return Err(ReduceError::LinearValueDuplicated(argument));
            }
            if self.first_linear_descendant(argument)?.is_some() {
                return Err(ReduceError::LinearValueDuplicated(argument));
            }
        }
        Ok(())
    }

    fn linearity_summary(&mut self, root: Cid) -> Result<LinearitySummary, ReduceError> {
        if let Some(summary) = self.linearity_summaries.get(&root) {
            return Ok(summary.clone());
        }

        let mut reachable = BTreeSet::new();
        let mut incoming = BTreeMap::<Cid, usize>::new();
        let mut nodes = BTreeMap::<Cid, Node>::new();
        let mut pending = vec![root];
        while let Some(cid) = pending.pop() {
            if !reachable.insert(cid) {
                continue;
            }
            self.charge()?;
            let node = self
                .store
                .get(cid)
                .cloned()
                .ok_or(ReduceError::MissingNode(cid))?;
            let children = match &node {
                Node::Quote { .. } | Node::Family { .. } => Vec::new(),
                _ => node.children(),
            };
            for child in children.into_iter().rev() {
                *incoming.entry(child).or_default() += 1;
                pending.push(child);
            }
            nodes.insert(cid, node);
        }

        let mut linear = nodes
            .iter()
            .filter_map(|(cid, node)| match node {
                Node::Const(Atom::Trace(_)) | Node::Emit { .. } => Some(*cid),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let mut summary = LinearitySummary::default();
        for node in nodes.values() {
            if let Node::Emit { token, .. } = node {
                linear.insert(*token);
                if let Some(Node::Param(index)) = self.store.get(*token) {
                    summary.token_parameters.insert(*index);
                }
            }
        }
        if let Some(cid) = linear.into_iter().find(|cid| {
            incoming.get(cid).copied().unwrap_or(0) > 1
                && !matches!(nodes.get(cid), Some(Node::Param(_)))
        }) {
            return Err(ReduceError::LinearValueDuplicated(cid));
        }

        // Direct incoming-edge counts miss duplication through a shared
        // container: `x = Pair($0, 1); Pair(x, x)` has one edge into `$0`
        // but two paths from the body root to it.  Propagate path
        // multiplicities through the template DAG so instantiation can reject
        // copying an argument that carries a linear capability.
        let mut indegree = nodes
            .keys()
            .map(|cid| (*cid, incoming.get(cid).copied().unwrap_or(0)))
            .collect::<BTreeMap<_, _>>();
        let mut ready = indegree
            .iter()
            .filter_map(|(cid, degree)| (*degree == 0).then_some(*cid))
            .collect::<BTreeSet<_>>();
        let mut paths = BTreeMap::from([(root, 1_usize)]);
        let mut processed = 0;
        while let Some(cid) = ready.pop_first() {
            processed += 1;
            let path_count = paths.get(&cid).copied().unwrap_or(0);
            let node = nodes.get(&cid).expect("reachable summary node");
            let children = match node {
                Node::Quote { .. } | Node::Family { .. } => Vec::new(),
                _ => node.children(),
            };
            for child in children {
                let child_paths = paths.entry(child).or_default();
                *child_paths = child_paths.saturating_add(path_count);
                let degree = indegree.get_mut(&child).expect("reachable child indegree");
                *degree = degree.checked_sub(1).expect("positive child indegree");
                if *degree == 0 {
                    ready.insert(child);
                }
            }
        }
        if processed != nodes.len() {
            let cid = indegree
                .into_iter()
                .find_map(|(cid, degree)| (degree != 0).then_some(cid))
                .expect("unprocessed summary node");
            return Err(ReduceError::Cycle(cid));
        }

        for (cid, node) in &nodes {
            if let Node::Param(index) = node {
                summary
                    .parameter_uses
                    .insert(*index, paths.get(cid).copied().unwrap_or(0));
            }
        }
        self.linearity_summaries.insert(root, summary.clone());
        Ok(summary)
    }

    fn first_linear_descendant(&mut self, root: Cid) -> Result<Option<Cid>, ReduceError> {
        enum LinearFrame {
            Enter(Cid),
            Exit { cid: Cid, children: usize },
        }

        if let Some(linear) = self.carries_linear.get(&root) {
            return Ok(*linear);
        }
        let mut frames = vec![LinearFrame::Enter(root)];
        let mut values = Vec::new();
        let mut visiting = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            self.stats.peak_frames = self.stats.peak_frames.max(frames.len() + 1);
            match frame {
                LinearFrame::Enter(cid) => {
                    if let Some(linear) = self.carries_linear.get(&cid).copied() {
                        values.push(linear);
                        continue;
                    }
                    if !visiting.insert(cid) {
                        return Err(ReduceError::Cycle(cid));
                    }
                    self.charge()?;
                    let node = self
                        .store
                        .get(cid)
                        .cloned()
                        .ok_or(ReduceError::MissingNode(cid))?;
                    let immediate = match node {
                        Node::Const(Atom::Trace(_)) | Node::Emit { .. } => Some(Some(cid)),
                        Node::Quote { .. } | Node::Family { .. } => Some(None),
                        Node::Hole(name) => match self.bindings.get(&name) {
                            Some(value) => {
                                frames.push(LinearFrame::Exit { cid, children: 1 });
                                frames.push(LinearFrame::Enter(value));
                                None
                            }
                            None => Some(None),
                        },
                        node => {
                            let children = node.children();
                            frames.push(LinearFrame::Exit {
                                cid,
                                children: children.len(),
                            });
                            for child in children.into_iter().rev() {
                                frames.push(LinearFrame::Enter(child));
                            }
                            None
                        }
                    };
                    if let Some(linear) = immediate {
                        visiting.remove(&cid);
                        self.carries_linear.insert(cid, linear);
                        values.push(linear);
                    }
                }
                LinearFrame::Exit { cid, children } => {
                    let first = values
                        .len()
                        .checked_sub(children)
                        .expect("linear-carry frame values");
                    let linear = values.drain(first..).find_map(|value| value);
                    visiting.remove(&cid);
                    self.carries_linear.insert(cid, linear);
                    values.push(linear);
                }
            }
        }
        assert_eq!(values.len(), 1, "linear-carry value-stack imbalance");
        Ok(values.pop().expect("linear-carry result"))
    }

    fn residual_dispatch(&mut self, family: Cid, arguments: &[Cid]) -> Result<Cid, ReduceError> {
        let mut memo = BTreeMap::new();
        let arguments = arguments
            .iter()
            .map(|argument| self.instantiate_bindings(*argument, &mut memo))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.store.intern(Node::Dispatch { family, arguments }))
    }

    fn substitute_family_body(
        &mut self,
        cid: Cid,
        arguments: &[Cid],
        family: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        self.rewrite_graph(cid, RewriteMode::FamilyBody { arguments, family }, memo)
    }

    fn substitute(
        &mut self,
        cid: Cid,
        arguments: &[Cid],
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        self.rewrite_graph(cid, RewriteMode::Substitute { arguments }, memo)
    }

    /// Replace known named holes without otherwise evaluating the graph.  This
    /// is used beneath an unresolved lazy branch: compile-time facts must be
    /// captured, but branch errors and divergence must remain deferred.
    fn instantiate_bindings(
        &mut self,
        cid: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        self.rewrite_graph(cid, RewriteMode::Bindings, memo)
    }

    /// Iterative post-order graph rewrite shared by quotation substitution,
    /// selected-family instantiation, and epoch-binding capture.  Each policy
    /// keeps the exact boundary and child order of the former recursive
    /// implementation.
    fn rewrite_graph(
        &mut self,
        root: Cid,
        mode: RewriteMode<'_>,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        let mut frames = vec![RewriteFrame::Enter(root)];
        let mut values = Vec::new();

        loop {
            self.stats.peak_frames = self.stats.peak_frames.max(frames.len());
            let Some(frame) = frames.pop() else {
                break;
            };
            match frame {
                RewriteFrame::Enter(cid) => {
                    if let Some(result) = memo.get(&cid).copied() {
                        values.push(result);
                        continue;
                    }

                    self.charge()?;
                    let node = self
                        .store
                        .get(cid)
                        .cloned()
                        .ok_or(ReduceError::MissingNode(cid))?;

                    let leaf = match (&mode, &node) {
                        (
                            RewriteMode::Substitute { arguments }
                            | RewriteMode::FamilyBody { arguments, .. },
                            Node::Param(index),
                        ) => Some(arguments.get(usize::from(*index)).copied().ok_or(
                            ReduceError::ParameterOutOfRange {
                                index: *index,
                                arguments: arguments.len(),
                            },
                        )?),
                        (RewriteMode::Bindings, Node::Param(_)) => Some(cid),
                        (RewriteMode::Bindings, Node::Hole(name)) => {
                            Some(self.bindings.get(name).unwrap_or(cid))
                        }
                        (
                            RewriteMode::Substitute { .. } | RewriteMode::FamilyBody { .. },
                            Node::Hole(_),
                        ) => Some(cid),
                        (_, Node::Const(_) | Node::Quote { .. } | Node::Family { .. }) => Some(cid),
                        _ => None,
                    };

                    if let Some(result) = leaf {
                        memo.insert(cid, result);
                        values.push(result);
                        continue;
                    }

                    if let (
                        RewriteMode::FamilyBody { arguments, .. },
                        Node::Recur(recursive_arguments),
                    ) = (&mode, &node)
                        && recursive_arguments.len() != arguments.len()
                    {
                        return Err(ReduceError::FamilyArity {
                            expected: arguments.len().try_into().unwrap_or(u16::MAX),
                            actual: recursive_arguments.len(),
                        });
                    }

                    let children = node.children();
                    frames.push(RewriteFrame::Exit {
                        source: cid,
                        node,
                        children: children.len(),
                    });
                    for child in children.into_iter().rev() {
                        frames.push(RewriteFrame::Enter(child));
                    }
                }
                RewriteFrame::Exit {
                    source,
                    node,
                    children,
                } => {
                    let first = values
                        .len()
                        .checked_sub(children)
                        .expect("rewrite frame values");
                    let rewritten_children = values.split_off(first);
                    let result =
                        self.rebuild_rewritten_node(source, node, rewritten_children, mode)?;
                    memo.insert(source, result);
                    values.push(result);
                }
            }
        }

        assert_eq!(values.len(), 1, "graph-rewrite value-stack imbalance");
        Ok(values.pop().expect("graph-rewrite result"))
    }

    fn rebuild_rewritten_node(
        &mut self,
        source: Cid,
        node: Node,
        rewritten_children: Vec<Cid>,
        mode: RewriteMode<'_>,
    ) -> Result<Cid, ReduceError> {
        let mut children = rewritten_children.into_iter();
        let rebuilt = match node {
            Node::Add(_, _) => Node::Add(
                children.next().expect("add left child"),
                children.next().expect("add right child"),
            ),
            Node::Mul(_, _) => Node::Mul(
                children.next().expect("multiply left child"),
                children.next().expect("multiply right child"),
            ),
            Node::Eq(_, _) => Node::Eq(
                children.next().expect("equality left child"),
                children.next().expect("equality right child"),
            ),
            Node::If { .. } => Node::If {
                condition: children.next().expect("if condition"),
                when_true: children.next().expect("if true branch"),
                when_false: children.next().expect("if false branch"),
            },
            Node::Pair(_, _) => Node::Pair(
                children.next().expect("pair left child"),
                children.next().expect("pair right child"),
            ),
            Node::First(_) => Node::First(children.next().expect("first child")),
            Node::Second(_) => Node::Second(children.next().expect("second child")),
            Node::Record(fields) => Node::Record(
                fields
                    .into_iter()
                    .map(|(name, _)| (name, children.next().expect("record field child")))
                    .collect(),
            ),
            Node::Get { field, .. } => Node::Get {
                record: children.next().expect("get record child"),
                field,
            },
            Node::Put { field, .. } => Node::Put {
                record: children.next().expect("put record child"),
                field,
                value: children.next().expect("put value child"),
            },
            Node::Apply { .. } => {
                let function = children.next().expect("apply function child");
                Node::Apply {
                    function,
                    arguments: children.by_ref().collect(),
                }
            }
            Node::Emit { .. } => Node::Emit {
                token: children.next().expect("emit token child"),
                message: children.next().expect("emit message child"),
            },
            Node::Dispatch { .. } => {
                let family = children.next().expect("dispatch family child");
                Node::Dispatch {
                    family,
                    arguments: children.by_ref().collect(),
                }
            }
            Node::Recur(_) => match mode {
                RewriteMode::FamilyBody { family, .. } => Node::Dispatch {
                    family,
                    arguments: children.by_ref().collect(),
                },
                RewriteMode::Substitute { .. } | RewriteMode::Bindings => {
                    Node::Recur(children.by_ref().collect())
                }
            },
            Node::Const(_)
            | Node::Hole(_)
            | Node::Param(_)
            | Node::Quote { .. }
            | Node::Family { .. } => return Ok(source),
        };
        debug_assert!(children.next().is_none());
        Ok(self.store.intern(rebuilt))
    }
    /// Code values are closed over named holes.  Context therefore crosses a
    /// code boundary only as an explicit argument, making residual families
    /// independent of which epoch first encountered them.
    fn validate_code_value(&mut self, root: Cid) -> Result<(), ReduceError> {
        if self.store.validated_code.contains(&root) {
            return Ok(());
        }
        let root_node = self
            .store
            .get(root)
            .cloned()
            .ok_or(ReduceError::MissingNode(root))?;
        let mut pending = Vec::<(Cid, CodeScope)>::new();
        self.seed_code_validation(root_node, &mut pending)?;
        let mut seen = BTreeSet::new();
        while let Some((cid, scope)) = pending.pop() {
            if !seen.insert((cid, scope)) {
                continue;
            }
            let node = self
                .store
                .get(cid)
                .cloned()
                .ok_or(ReduceError::MissingNode(cid))?;
            match node {
                // A trace is a linear capability, not an inert constant that
                // reusable code may capture.  It must cross the code boundary
                // as an explicit parameter so every invocation has one
                // visible use topology.
                Node::Const(Atom::Trace(_)) => {
                    return Err(ReduceError::LinearValueDuplicated(cid));
                }
                Node::Const(_) => {}
                Node::Hole(name) => return Err(ReduceError::OpenCodeValue(name)),
                Node::Param(index) => {
                    let parameters = match scope {
                        CodeScope::Family { parameters, .. } | CodeScope::Quote { parameters } => {
                            parameters
                        }
                    };
                    if index >= parameters {
                        return Err(ReduceError::ParameterOutOfRange {
                            index,
                            arguments: usize::from(parameters),
                        });
                    }
                }
                Node::Recur(arguments) => match scope {
                    CodeScope::Family {
                        parameters,
                        recur: true,
                    } => {
                        if arguments.len() != usize::from(parameters) {
                            return Err(ReduceError::FamilyArity {
                                expected: parameters,
                                actual: arguments.len(),
                            });
                        }
                        pending.extend(arguments.into_iter().map(|cid| (cid, scope)));
                    }
                    _ => return Err(ReduceError::UnboundRecursion),
                },
                Node::Quote { params, body } => {
                    pending.push((body, CodeScope::Quote { parameters: params }))
                }
                Node::Family {
                    parameters,
                    clauses,
                } => {
                    for Clause { guard, body } in clauses {
                        self.validate_guard_purity(guard)?;
                        pending.push((
                            guard,
                            CodeScope::Family {
                                parameters,
                                recur: false,
                            },
                        ));
                        pending.push((
                            body,
                            CodeScope::Family {
                                parameters,
                                recur: true,
                            },
                        ));
                    }
                }
                other => pending.extend(other.children().into_iter().map(|cid| (cid, scope))),
            }
        }
        self.store.validated_code.insert(root);
        Ok(())
    }

    fn seed_code_validation(
        &mut self,
        node: Node,
        pending: &mut Vec<(Cid, CodeScope)>,
    ) -> Result<(), ReduceError> {
        match node {
            Node::Quote { params, body } => {
                pending.push((body, CodeScope::Quote { parameters: params }));
                Ok(())
            }
            Node::Family {
                parameters,
                clauses,
            } => {
                for Clause { guard, body } in clauses {
                    self.validate_guard_purity(guard)?;
                    pending.push((
                        guard,
                        CodeScope::Family {
                            parameters,
                            recur: false,
                        },
                    ));
                    pending.push((
                        body,
                        CodeScope::Family {
                            parameters,
                            recur: true,
                        },
                    ));
                }
                Ok(())
            }
            _ => Err(ReduceError::Type(
                "closure validation requires a quotation or family",
            )),
        }
    }

    fn validate_guard_purity(&self, root: Cid) -> Result<(), ReduceError> {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(cid) = pending.pop() {
            if !seen.insert(cid) {
                continue;
            }
            let node = self.store.get(cid).ok_or(ReduceError::MissingNode(cid))?;
            match node {
                Node::Const(Atom::Trace(_))
                | Node::Hole(_)
                | Node::Put { .. }
                | Node::Quote { .. }
                | Node::Apply { .. }
                | Node::Emit { .. }
                | Node::Family { .. }
                | Node::Dispatch { .. }
                | Node::Recur(_) => return Err(ReduceError::ImpureGuard(root)),
                Node::Const(_)
                | Node::Param(_)
                | Node::Add(_, _)
                | Node::Mul(_, _)
                | Node::Eq(_, _)
                | Node::If { .. }
                | Node::Pair(_, _)
                | Node::First(_)
                | Node::Second(_)
                | Node::Record(_)
                | Node::Get { .. } => pending.extend(node.children()),
            }
        }
        Ok(())
    }

    fn is_ground(&mut self, root: Cid) -> Result<bool, ReduceError> {
        enum GroundFrame {
            Enter {
                cid: Cid,
                bound_parameters: bool,
            },
            Exit {
                cid: Cid,
                bound_parameters: bool,
                children: usize,
            },
        }

        let mut frames = vec![GroundFrame::Enter {
            cid: root,
            bound_parameters: false,
        }];
        let mut values = Vec::new();
        let mut visiting = BTreeSet::new();

        loop {
            self.stats.peak_frames = self.stats.peak_frames.max(frames.len());
            let Some(frame) = frames.pop() else {
                break;
            };
            match frame {
                GroundFrame::Enter {
                    cid,
                    bound_parameters,
                } => {
                    if let Some(ground) = self.ground_memo.get(&(cid, bound_parameters)).copied() {
                        values.push(ground);
                        continue;
                    }
                    if !visiting.insert(cid) {
                        return Err(ReduceError::Cycle(cid));
                    }
                    self.charge()?;
                    let node = self
                        .store
                        .get(cid)
                        .cloned()
                        .ok_or(ReduceError::MissingNode(cid))?;
                    let ground = match node {
                        Node::Const(_) => Some(true),
                        Node::Hole(_) => Some(false),
                        Node::Param(_) => Some(bound_parameters),
                        Node::Quote { .. } | Node::Family { .. } => {
                            self.validate_code_value(cid)?;
                            Some(true)
                        }
                        node => {
                            let children = node.children();
                            frames.push(GroundFrame::Exit {
                                cid,
                                bound_parameters,
                                children: children.len(),
                            });
                            for child in children.into_iter().rev() {
                                frames.push(GroundFrame::Enter {
                                    cid: child,
                                    bound_parameters,
                                });
                            }
                            None
                        }
                    };
                    if let Some(ground) = ground {
                        visiting.remove(&cid);
                        self.ground_memo.insert((cid, bound_parameters), ground);
                        values.push(ground);
                    }
                }
                GroundFrame::Exit {
                    cid,
                    bound_parameters,
                    children,
                } => {
                    let first = values
                        .len()
                        .checked_sub(children)
                        .expect("groundness frame values");
                    let ground = values.drain(first..).all(|value| value);
                    visiting.remove(&cid);
                    self.ground_memo.insert((cid, bound_parameters), ground);
                    values.push(ground);
                }
            }
        }

        assert_eq!(values.len(), 1, "groundness value-stack imbalance");
        Ok(values.pop().expect("groundness result"))
    }

    /// Conservative E0 linearity check for the explicit effect carrier.  It
    /// follows reachable DAG edges, marks every `Emit` result and everything
    /// feeding an emit token as linear, then rejects shared consumers.  A
    /// typed successor should make linearity part of the typing judgment and
    /// treat mutually exclusive branches more precisely.
    #[cfg(test)]
    fn validate_linearity(&mut self, root: Cid) -> Result<(), ReduceError> {
        self.validate_linearity_roots(&[root])
    }

    /// Treat the program and every supplied context value as children of one
    /// virtual invocation root.  This exposes aliases across distinct binding
    /// names as well as sharing inside an individual bound value.
    fn validate_linearity_roots(&mut self, roots: &[Cid]) -> Result<(), ReduceError> {
        let mut reachable = BTreeSet::new();
        let mut incoming = BTreeMap::<Cid, usize>::new();
        let mut nodes = BTreeMap::<Cid, Node>::new();
        let mut pending = roots.to_vec();
        for root in roots {
            *incoming.entry(*root).or_default() += 1;
        }
        while let Some(cid) = pending.pop() {
            if !reachable.insert(cid) {
                continue;
            }
            self.charge()?;
            let node = self
                .store
                .get(cid)
                .cloned()
                .ok_or(ReduceError::MissingNode(cid))?;
            // Code values are lazy boundaries.  Their bodies are checked when
            // a quotation is applied or a family clause is selected, not when
            // the dormant definition merely becomes reachable.
            let children = match &node {
                Node::Quote { .. } | Node::Family { .. } => Vec::new(),
                _ => node.children(),
            };
            for child in children {
                *incoming.entry(child).or_default() += 1;
                pending.push(child);
            }
            nodes.insert(cid, node);
        }

        let mut linear = BTreeSet::new();
        for cid in nodes.keys().copied().collect::<Vec<_>>() {
            if self.first_linear_descendant(cid)?.is_some() {
                linear.insert(cid);
            }
        }
        for node in nodes.values() {
            if let Node::Emit { token, .. } = node {
                // An unresolved token position is linear by contract even
                // when the value currently beneath it carries no known Trace.
                linear.insert(*token);
            }
        }
        if let Some(cid) = linear
            .into_iter()
            .find(|cid| incoming.get(cid).copied().unwrap_or(0) > 1)
        {
            return Err(ReduceError::LinearValueDuplicated(cid));
        }
        Ok(())
    }
}

fn reducer_cid() -> Cid {
    Cid::digest(
        b"march6/reducer/v5",
        b"lazy-quote-and-arguments;ordered-guarded-families;lexical-recur;pure-explicit-effects;no-captured-capabilities;strict-guard-demand-sharing;incremental-linear-capability-summary",
    )
}

fn specialization_key(source: Cid, context: Cid, reducer: Cid) -> Cid {
    let mut bytes = Vec::new();
    put_cid(&mut bytes, source);
    put_cid(&mut bytes, context);
    put_cid(&mut bytes, reducer);
    Cid::digest(b"march6/specialization/v1", &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(store: &mut Store, value: i64) -> Cid {
        store.intern(Node::Const(Atom::Int(value)))
    }

    #[test]
    fn compilation_then_execution_matches_direct_reduction() {
        let mut store = Store::new();
        let mode = store.intern(Node::Hole("mode".into()));
        let input = store.intern(Node::Hole("input".into()));
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let plus = store.intern(Node::Add(input, one));
        let times = store.intern(Node::Mul(input, two));
        let program = store.intern(Node::If {
            condition: mode,
            when_true: plus,
            when_false: times,
        });
        let truth = store.intern(Node::Const(Atom::Bool(true)));
        let forty_one = int(&mut store, 41);

        let mut compile = Bindings::new();
        compile.insert("mode", truth);
        let residual = Reducer::specialize(&mut store, program, &compile)
            .unwrap()
            .residual;
        assert_eq!(store.format(residual), "(+ ?input 1)");

        let mut runtime = Bindings::new();
        runtime.insert("input", forty_one);
        let staged = Reducer::new(&mut store, &runtime).run(residual).unwrap();
        let all = compile.merged(&runtime).unwrap();
        let direct = Reducer::new(&mut store, &all).run(program).unwrap();
        assert_eq!(staged.root, direct.root);
        assert_eq!(store.get(staged.root), Some(&Node::Const(Atom::Int(42))));
    }

    #[test]
    fn cache_key_includes_even_irrelevant_context() {
        let mut store = Store::new();
        let root = int(&mut store, 7);
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let mut a = Bindings::new();
        a.insert("unused", one);
        let mut b = Bindings::new();
        b.insert("unused", two);
        let a = Reducer::specialize(&mut store, root, &a).unwrap();
        let b = Reducer::specialize(&mut store, root, &b).unwrap();
        assert_eq!(a.residual, b.residual);
        assert_ne!(a.cache_key, b.cache_key);
    }

    #[test]
    fn reduction_budget_is_bounded_execution_policy() {
        let mut store = Store::new();
        let one = int(&mut store, 1);
        let mut deep = one;
        for _ in 0..20 {
            deep = store.intern(Node::Add(deep, one));
        }
        assert_eq!(
            Reducer::with_budget(&mut store, &Bindings::new(), 5).run(deep),
            Err(ReduceError::BudgetExhausted { limit: 5 })
        );

        let small = Reducer::specialize_with_budget(&mut store, one, &Bindings::new(), 8).unwrap();
        let large = Reducer::specialize_with_budget(&mut store, one, &Bindings::new(), 16).unwrap();
        assert_eq!(small.residual, large.residual);
        assert_eq!(small.reducer, large.reducer);
        assert_eq!(small.cache_key, large.cache_key);
    }

    #[test]
    fn specialization_cache_checks_semantic_dependencies_not_execution_policy() {
        let mut store = Store::new();
        let input = store.intern(Node::Hole("x".into()));
        let one = int(&mut store, 1);
        let program = store.intern(Node::Add(input, one));
        let forty_one = int(&mut store, 41);
        let mut context = Bindings::new();
        context.insert("x", forty_one);
        let mut cache = SpecializationCache::new();

        let first = cache
            .specialize(&mut store, program, &context, DEFAULT_REDUCTION_BUDGET)
            .unwrap();
        let second = cache
            .specialize(&mut store, program, &context, DEFAULT_REDUCTION_BUDGET)
            .unwrap();
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(first.residual, second.residual);
        assert_eq!(second.stats.steps, 0);
        assert_eq!(cache.len(), 1);

        let mut different_context = context.clone();
        different_context.insert("irrelevant", one);
        let context_miss = cache
            .specialize(
                &mut store,
                program,
                &different_context,
                DEFAULT_REDUCTION_BUDGET,
            )
            .unwrap();
        assert!(!context_miss.cache_hit);
        assert_eq!(context_miss.residual, first.residual);
        assert_ne!(context_miss.cache_key, first.cache_key);

        let policy_hit = cache
            .specialize(&mut store, program, &context, DEFAULT_REDUCTION_BUDGET + 1)
            .unwrap();
        assert!(policy_hit.cache_hit);
        assert_eq!(policy_hit.reducer, first.reducer);
        assert_eq!(policy_hit.cache_key, first.cache_key);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn groundness_memoizes_shared_dags_and_charges_work() {
        let mut store = Store::new();
        let one = int(&mut store, 1);
        let mut shared = one;
        for _ in 0..40 {
            shared = store.intern(Node::Pair(shared, shared));
        }
        let invalid = store.intern(Node::Add(shared, one));
        assert_eq!(
            Reducer::with_budget(&mut store, &Bindings::new(), 300).run(invalid),
            Err(ReduceError::Type("add expects two integers"))
        );
        assert_eq!(
            Reducer::with_budget(&mut store, &Bindings::new(), 20).run(invalid),
            Err(ReduceError::BudgetExhausted { limit: 20 })
        );
    }

    #[test]
    fn quotation_application_uses_the_same_reducer() {
        let mut store = Store::new();
        let arg = store.intern(Node::Param(0));
        let body = store.intern(Node::Mul(arg, arg));
        let quote = store.intern(Node::Quote { params: 1, body });
        let nine = int(&mut store, 9);
        let application = store.intern(Node::Apply {
            function: quote,
            arguments: vec![nine],
        });
        let result = Reducer::new(&mut store, &Bindings::new())
            .run(application)
            .unwrap();
        assert_eq!(store.get(result.root), Some(&Node::Const(Atom::Int(81))));
    }

    #[test]
    fn incremental_linearity_matches_full_walk_on_core_instantiations() {
        fn duplicated(result: Result<(), ReduceError>) -> bool {
            matches!(result, Err(ReduceError::LinearValueDuplicated(_)))
        }

        let mut store = Store::new();
        let p0 = store.intern(Node::Param(0));
        let p1 = store.intern(Node::Param(1));
        let pair_same = store.intern(Node::Pair(p0, p0));
        let pair_distinct = store.intern(Node::Pair(p0, p1));
        let one = int(&mut store, 1);
        let world = store.intern(Node::Const(Atom::Trace(Vec::new())));
        let a = store.intern(Node::Const(Atom::Text("a".into())));
        let b = store.intern(Node::Const(Atom::Text("b".into())));
        let emit_a = store.intern(Node::Emit {
            token: p0,
            message: a,
        });
        let emit_b = store.intern(Node::Emit {
            token: p0,
            message: b,
        });
        let duplicate_emits = store.intern(Node::Pair(emit_a, emit_b));
        let cases = [
            (pair_same, vec![one]),
            (pair_same, vec![world]),
            (pair_distinct, vec![world, world]),
            (p0, vec![world]),
            (duplicate_emits, vec![world]),
        ];

        let bindings = Bindings::new();
        let mut reducer = Reducer::with_budget(&mut store, &bindings, usize::MAX);
        for (template, arguments) in cases {
            let incremental =
                duplicated(reducer.validate_instantiation_linearity(template, &arguments));
            let selected = reducer
                .substitute(template, &arguments, &mut BTreeMap::new())
                .unwrap();
            let full = duplicated(reducer.validate_linearity(selected));
            assert_eq!(
                incremental, full,
                "incremental and full linearity checks disagree for {template:?}",
            );
        }
    }

    #[test]
    fn immutable_record_models_explicit_state_transition() {
        let mut store = Store::new();
        let state = store.intern(Node::Hole("state".into()));
        let counter = store.intern(Node::Get {
            record: state,
            field: "counter".into(),
        });
        let one = int(&mut store, 1);
        let incremented = store.intern(Node::Add(counter, one));
        let program = store.intern(Node::Put {
            record: state,
            field: "counter".into(),
            value: incremented,
        });

        let residual = Reducer::new(&mut store, &Bindings::new())
            .run(program)
            .unwrap();
        assert!(store.format(residual.root).contains("?state"));

        let forty_one = int(&mut store, 41);
        let initial = store.intern(Node::Record(vec![("counter".into(), forty_one)]));
        let mut runtime = Bindings::new();
        runtime.insert("state", initial);
        let result = Reducer::new(&mut store, &runtime)
            .run(residual.root)
            .unwrap();
        assert_eq!(store.format(result.root), "{counter: 42}");
    }

    #[test]
    fn effect_token_is_ordered_and_never_executed_during_compilation() {
        let mut store = Store::new();
        let token = store.intern(Node::Hole("world".into()));
        let hello = store.intern(Node::Const(Atom::Text("hello".into())));
        let goodbye = store.intern(Node::Const(Atom::Text("goodbye".into())));
        let first = store.intern(Node::Emit {
            token,
            message: hello,
        });
        let program = store.intern(Node::Emit {
            token: first,
            message: goodbye,
        });

        let residual = Reducer::new(&mut store, &Bindings::new())
            .run(program)
            .unwrap();
        assert_eq!(
            store.format(residual.root),
            "(emit (emit ?world \"hello\") \"goodbye\")"
        );

        let empty = store.intern(Node::Const(Atom::Trace(Vec::new())));
        let mut runtime = Bindings::new();
        runtime.insert("world", empty);
        let result = Reducer::new(&mut store, &runtime)
            .run(residual.root)
            .unwrap();
        assert_eq!(
            store.get(result.root),
            Some(&Node::Const(Atom::Trace(vec![
                "hello".into(),
                "goodbye".into(),
            ])))
        );
    }

    #[test]
    fn unknown_if_does_not_reduce_an_unselected_erroneous_branch() {
        let mut store = Store::new();
        let condition = store.intern(Node::Hole("condition".into()));
        let one = int(&mut store, 1);
        let five = int(&mut store, 5);
        let invalid = store.intern(Node::First(five));
        let program = store.intern(Node::If {
            condition,
            when_true: one,
            when_false: invalid,
        });

        let residual = Reducer::new(&mut store, &Bindings::new())
            .run(program)
            .unwrap();
        assert_eq!(residual.root, program);
        let truth = store.intern(Node::Const(Atom::Bool(true)));
        let mut runtime = Bindings::new();
        runtime.insert("condition", truth);
        let staged = Reducer::new(&mut store, &runtime)
            .run(residual.root)
            .unwrap();
        assert_eq!(staged.root, one);
    }

    #[test]
    fn fully_ground_stuck_terms_are_errors() {
        let mut store = Store::new();
        let one = int(&mut store, 1);
        let truth = store.intern(Node::Const(Atom::Bool(true)));
        let invalid_add = store.intern(Node::Add(truth, one));
        assert!(matches!(
            Reducer::new(&mut store, &Bindings::new()).run(invalid_add),
            Err(ReduceError::Type(_))
        ));

        let pair = store.intern(Node::Pair(one, one));
        let equality = store.intern(Node::Eq(pair, pair));
        let result = Reducer::new(&mut store, &Bindings::new())
            .run(equality)
            .unwrap();
        assert_eq!(store.get(result.root), Some(&Node::Const(Atom::Bool(true))));
    }

    #[test]
    fn integer_overflow_is_profile_independent_error() {
        let mut store = Store::new();
        let max = int(&mut store, i64::MAX);
        let one = int(&mut store, 1);
        let add = store.intern(Node::Add(max, one));
        assert_eq!(
            Reducer::new(&mut store, &Bindings::new()).run(add),
            Err(ReduceError::IntegerOverflow("add"))
        );
    }

    #[test]
    fn effect_token_cannot_be_duplicated() {
        let mut store = Store::new();
        let token = store.intern(Node::Hole("world".into()));
        let a = store.intern(Node::Const(Atom::Text("a".into())));
        let b = store.intern(Node::Const(Atom::Text("b".into())));
        let left = store.intern(Node::Emit { token, message: a });
        let right = store.intern(Node::Emit { token, message: b });
        let program = store.intern(Node::Pair(left, right));
        assert!(matches!(
            Reducer::new(&mut store, &Bindings::new()).run(program),
            Err(ReduceError::LinearValueDuplicated(cid)) if cid == token
        ));
    }

    #[test]
    fn generated_staging_splits_match_direct_evaluation() {
        let mut store = Store::new();
        let x = store.intern(Node::Hole("x".into()));
        let condition = store.intern(Node::Hole("condition".into()));
        let zero = int(&mut store, 0);
        let one = int(&mut store, 1);
        let two = int(&mut store, 2);
        let invalid = store.intern(Node::First(one));
        let x_plus_one = store.intern(Node::Add(x, one));
        let x_times_two = store.intern(Node::Mul(x, two));
        let choose = store.intern(Node::If {
            condition,
            when_true: x_plus_one,
            when_false: x_times_two,
        });
        let lazy_error = store.intern(Node::If {
            condition,
            when_true: x,
            when_false: invalid,
        });
        let pair = store.intern(Node::Pair(x, one));
        let first = store.intern(Node::First(pair));
        let base = vec![
            x,
            zero,
            one,
            two,
            x_plus_one,
            x_times_two,
            choose,
            lazy_error,
            first,
        ];
        let mut programs = base.clone();
        for &left in &base {
            for &right in &base {
                programs.push(store.intern(Node::Add(left, right)));
                programs.push(store.intern(Node::Mul(left, right)));
                programs.push(store.intern(Node::If {
                    condition,
                    when_true: left,
                    when_false: right,
                }));
            }
        }

        for value in -2..=2 {
            for boolean in [false, true] {
                let value = int(&mut store, value);
                let boolean = store.intern(Node::Const(Atom::Bool(boolean)));
                let mut all = Bindings::new();
                all.insert("x", value);
                all.insert("condition", boolean);

                for static_mask in 0..4 {
                    let mut compile = Bindings::new();
                    let mut runtime = Bindings::new();
                    if static_mask & 1 != 0 {
                        compile.insert("x", value);
                    } else {
                        runtime.insert("x", value);
                    }
                    if static_mask & 2 != 0 {
                        compile.insert("condition", boolean);
                    } else {
                        runtime.insert("condition", boolean);
                    }

                    for &program in &programs {
                        let staged = Reducer::new(&mut store, &compile)
                            .run(program)
                            .and_then(|residual| {
                                Reducer::new(&mut store, &runtime).run(residual.root)
                            })
                            .map(|reduction| reduction.root);
                        let direct = Reducer::new(&mut store, &all)
                            .run(program)
                            .map(|reduction| reduction.root);
                        assert_eq!(
                            staged,
                            direct,
                            "program={} x={} condition={} static-mask={static_mask}",
                            store.format(program),
                            store.format(value),
                            store.format(boolean)
                        );
                    }
                }
            }
        }
    }
}
