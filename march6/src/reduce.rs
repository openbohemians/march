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
    DepthExhausted { limit: usize },
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
                write!(f, "linear effect value {cid} has more than one consumer")
            }
            Self::BudgetExhausted { limit } => {
                write!(f, "reduction exceeded its explicit {limit}-step budget")
            }
            Self::DepthExhausted { limit } => {
                write!(
                    f,
                    "reduction exceeded its explicit {limit}-frame host depth limit"
                )
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
    visiting: BTreeSet<Cid>,
    stats: ReductionStats,
    remaining_steps: usize,
    step_limit: usize,
    depth: usize,
    depth_limit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CodeScope {
    Family { parameters: u16, recur: bool },
    Quote { parameters: u16 },
}

pub const DEFAULT_REDUCTION_BUDGET: usize = 512;
// This protects the current recursive Rust control implementation.  It is an
// execution-resource limit, not March semantics; the planned work-list reducer
// will remove the native-stack dependency.
pub const DEFAULT_HOST_DEPTH_LIMIT: usize = 64;

impl<'a> Reducer<'a> {
    pub fn new(store: &'a mut Store, bindings: &'a Bindings) -> Self {
        Self::with_budget(store, bindings, DEFAULT_REDUCTION_BUDGET)
    }

    pub fn with_budget(store: &'a mut Store, bindings: &'a Bindings, step_limit: usize) -> Self {
        Self {
            store,
            bindings,
            memo: BTreeMap::new(),
            visiting: BTreeSet::new(),
            stats: ReductionStats::default(),
            remaining_steps: step_limit,
            step_limit,
            depth: 0,
            depth_limit: DEFAULT_HOST_DEPTH_LIMIT,
        }
    }

    pub fn run(mut self, root: Cid) -> Result<Reduction, ReduceError> {
        self.validate_linearity(root)?;
        let root = self.reduce(root)?;
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

    fn enter_depth(&mut self) -> Result<(), ReduceError> {
        if self.depth == self.depth_limit {
            return Err(ReduceError::DepthExhausted {
                limit: self.depth_limit,
            });
        }
        self.depth += 1;
        Ok(())
    }

    fn reduce(&mut self, cid: Cid) -> Result<Cid, ReduceError> {
        self.enter_depth()?;
        let result = self.reduce_inner(cid);
        self.depth -= 1;
        result
    }

    fn reduce_inner(&mut self, cid: Cid) -> Result<Cid, ReduceError> {
        if let Some(result) = self.memo.get(&cid) {
            self.stats.memo_hits += 1;
            return Ok(*result);
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

        let result =
            match node {
                Node::Const(_) | Node::Param(_) => cid,
                Node::Hole(name) => match self.bindings.get(&name) {
                    Some(value) => self.reduce(value)?,
                    None => cid,
                },
                Node::Add(a, b) => {
                    let a = self.reduce(a)?;
                    let b = self.reduce(b)?;
                    match (self.store.get(a).cloned(), self.store.get(b).cloned()) {
                        (Some(Node::Const(Atom::Int(a))), Some(Node::Const(Atom::Int(b)))) => {
                            let value = a
                                .checked_add(b)
                                .ok_or(ReduceError::IntegerOverflow("add"))?;
                            self.store.intern(Node::Const(Atom::Int(value)))
                        }
                        (left, right)
                            if (left.as_ref().is_some_and(|node| {
                                !matches!(node, Node::Const(Atom::Int(_)))
                            }) && self.is_ground(a)?)
                                || (right.as_ref().is_some_and(|node| {
                                    !matches!(node, Node::Const(Atom::Int(_)))
                                }) && self.is_ground(b)?) =>
                        {
                            return Err(ReduceError::Type("add expects two integers"));
                        }
                        _ => self.store.intern(Node::Add(a, b)),
                    }
                }
                Node::Mul(a, b) => {
                    let a = self.reduce(a)?;
                    let b = self.reduce(b)?;
                    match (self.store.get(a).cloned(), self.store.get(b).cloned()) {
                        (Some(Node::Const(Atom::Int(a))), Some(Node::Const(Atom::Int(b)))) => {
                            let value = a
                                .checked_mul(b)
                                .ok_or(ReduceError::IntegerOverflow("multiply"))?;
                            self.store.intern(Node::Const(Atom::Int(value)))
                        }
                        (left, right)
                            if (left.as_ref().is_some_and(|node| {
                                !matches!(node, Node::Const(Atom::Int(_)))
                            }) && self.is_ground(a)?)
                                || (right.as_ref().is_some_and(|node| {
                                    !matches!(node, Node::Const(Atom::Int(_)))
                                }) && self.is_ground(b)?) =>
                        {
                            return Err(ReduceError::Type("multiply expects two integers"));
                        }
                        _ => self.store.intern(Node::Mul(a, b)),
                    }
                }
                Node::Eq(a, b) => {
                    let a = self.reduce(a)?;
                    let b = self.reduce(b)?;
                    if self.is_ground(a)? && self.is_ground(b)? {
                        self.store.intern(Node::Const(Atom::Bool(a == b)))
                    } else {
                        self.store.intern(Node::Eq(a, b))
                    }
                }
                Node::If {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let condition = self.reduce(condition)?;
                    match self.store.get(condition) {
                        Some(Node::Const(Atom::Bool(true))) => self.reduce(when_true)?,
                        Some(Node::Const(Atom::Bool(false))) => self.reduce(when_false)?,
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("if condition is not a boolean"));
                        }
                        // Branches are lazy.  Reducing them before the condition
                        // is known can surface an error or divergence that direct
                        // execution would never observe.  Static bindings still
                        // have to be embedded so the residual is closed over the
                        // compile context.
                        _ => {
                            let mut memo = BTreeMap::new();
                            let when_true = self.instantiate_bindings(when_true, &mut memo)?;
                            let when_false = self.instantiate_bindings(when_false, &mut memo)?;
                            self.store.intern(Node::If {
                                condition,
                                when_true,
                                when_false,
                            })
                        }
                    }
                }
                Node::Pair(a, b) => {
                    let a = self.reduce(a)?;
                    let b = self.reduce(b)?;
                    self.store.intern(Node::Pair(a, b))
                }
                Node::First(pair) => {
                    let pair = self.reduce(pair)?;
                    match self.store.get(pair).cloned() {
                        Some(Node::Pair(first, _)) => first,
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("first operand is not a pair"));
                        }
                        _ if self.is_ground(pair)? => {
                            return Err(ReduceError::Type("first operand is not a pair"));
                        }
                        _ => self.store.intern(Node::First(pair)),
                    }
                }
                Node::Second(pair) => {
                    let pair = self.reduce(pair)?;
                    match self.store.get(pair).cloned() {
                        Some(Node::Pair(_, second)) => second,
                        Some(Node::Const(_)) => {
                            return Err(ReduceError::Type("second operand is not a pair"));
                        }
                        _ if self.is_ground(pair)? => {
                            return Err(ReduceError::Type("second operand is not a pair"));
                        }
                        _ => self.store.intern(Node::Second(pair)),
                    }
                }
                Node::Record(fields) => {
                    let fields = fields
                        .into_iter()
                        .map(|(name, value)| Ok((name, self.reduce(value)?)))
                        .collect::<Result<Vec<_>, ReduceError>>()?;
                    self.store.intern(Node::Record(fields))
                }
                Node::Get { record, field } => {
                    let record = self.reduce(record)?;
                    match self.store.get(record).cloned() {
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
                    }
                }
                Node::Put {
                    record,
                    field,
                    value,
                } => {
                    let record = self.reduce(record)?;
                    let value = self.reduce(value)?;
                    match self.store.get(record).cloned() {
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
                    }
                }
                // A quotation is code-as-data and therefore already in weak
                // normal form.  Its body is demanded only by application.
                Node::Quote { .. } => {
                    self.validate_code_value(cid)?;
                    cid
                }
                Node::Apply {
                    function,
                    arguments,
                } => {
                    let function = self.reduce(function)?;
                    // Arguments are passed as graph references.  Capture facts
                    // from this epoch without evaluating an argument that the
                    // quotation may never use.
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
                            let body = self.substitute(body, &arguments, &mut BTreeMap::new())?;
                            self.validate_linearity(body)?;
                            self.reduce(body)?
                        }
                        _ if self.is_ground(function)? => {
                            return Err(ReduceError::Type("apply operand is not a quotation"));
                        }
                        _ => self.store.intern(Node::Apply {
                            function,
                            arguments,
                        }),
                    }
                }
                Node::Emit { token, message } => {
                    let token = self.reduce(token)?;
                    let message = self.reduce(message)?;
                    match (
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
                    }
                }
                Node::Family { .. } => {
                    self.validate_code_value(cid)?;
                    cid
                }
                Node::Dispatch {
                    family,
                    mut arguments,
                } => self.reduce_dispatch(family, &mut arguments)?,
                Node::Recur(_) => return Err(ReduceError::UnboundRecursion),
            };

        self.visiting.remove(&cid);
        if result != cid {
            self.stats.rewritten += 1;
        }
        self.memo.insert(cid, result);
        Ok(result)
    }

    fn reduce_dispatch(&mut self, family: Cid, arguments: &mut [Cid]) -> Result<Cid, ReduceError> {
        let family = self.reduce(family)?;
        let Some(node) = self.store.get(family).cloned() else {
            return Err(ReduceError::MissingNode(family));
        };
        let Node::Family {
            parameters,
            clauses,
        } = node
        else {
            if self.is_ground(family)? {
                return Err(ReduceError::Type(
                    "dispatch target is not a guarded definition family",
                ));
            }
            return self.residual_dispatch(family, arguments);
        };
        if clauses.is_empty() {
            return Err(ReduceError::EmptyFamily);
        }
        if usize::from(parameters) != arguments.len() {
            return Err(ReduceError::FamilyArity {
                expected: parameters,
                actual: arguments.len(),
            });
        }

        for Clause { guard, body } in clauses {
            // Guard expressions are closed templates over the family
            // parameters.  Instantiating a guard never touches its body or
            // any later clause.
            let guard = self.substitute(guard, arguments, &mut BTreeMap::new())?;
            let guard = self.reduce(guard)?;
            match self.store.get(guard).cloned() {
                Some(Node::Const(Atom::Bool(false))) => continue,
                Some(Node::Const(Atom::Bool(true))) => {
                    self.stats.clauses_instantiated += 1;
                    let selected =
                        self.substitute_family_body(body, arguments, family, &mut BTreeMap::new())?;
                    self.validate_linearity(selected)?;
                    return self.reduce(selected);
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
                _ => return self.residual_dispatch(family, arguments),
            }
        }
        Err(ReduceError::NoMatchingClause)
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
        self.enter_depth()?;
        let result = self.substitute_family_body_inner(cid, arguments, family, memo);
        self.depth -= 1;
        result
    }

    fn substitute_family_body_inner(
        &mut self,
        cid: Cid,
        arguments: &[Cid],
        family: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        if let Some(result) = memo.get(&cid) {
            return Ok(*result);
        }
        self.charge()?;
        let node = self
            .store
            .get(cid)
            .cloned()
            .ok_or(ReduceError::MissingNode(cid))?;
        let result = match node {
            Node::Param(index) => arguments.get(usize::from(index)).copied().ok_or(
                ReduceError::ParameterOutOfRange {
                    index,
                    arguments: arguments.len(),
                },
            )?,
            Node::Recur(recursive_arguments) => {
                if recursive_arguments.len() != arguments.len() {
                    return Err(ReduceError::FamilyArity {
                        expected: arguments.len().try_into().unwrap_or(u16::MAX),
                        actual: recursive_arguments.len(),
                    });
                }
                let recursive_arguments = recursive_arguments
                    .into_iter()
                    .map(|argument| self.substitute_family_body(argument, arguments, family, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Dispatch {
                    family,
                    arguments: recursive_arguments,
                })
            }
            // Nested binders and atoms are boundaries.
            Node::Quote { .. } | Node::Family { .. } | Node::Const(_) | Node::Hole(_) => cid,
            Node::Add(a, b) => {
                let (a, b) = self.substitute_family_binary(a, b, arguments, family, memo)?;
                self.store.intern(Node::Add(a, b))
            }
            Node::Mul(a, b) => {
                let (a, b) = self.substitute_family_binary(a, b, arguments, family, memo)?;
                self.store.intern(Node::Mul(a, b))
            }
            Node::Eq(a, b) => {
                let (a, b) = self.substitute_family_binary(a, b, arguments, family, memo)?;
                self.store.intern(Node::Eq(a, b))
            }
            Node::If {
                condition,
                when_true,
                when_false,
            } => {
                let condition = self.substitute_family_body(condition, arguments, family, memo)?;
                let when_true = self.substitute_family_body(when_true, arguments, family, memo)?;
                let when_false =
                    self.substitute_family_body(when_false, arguments, family, memo)?;
                self.store.intern(Node::If {
                    condition,
                    when_true,
                    when_false,
                })
            }
            Node::Pair(a, b) => {
                let (a, b) = self.substitute_family_binary(a, b, arguments, family, memo)?;
                self.store.intern(Node::Pair(a, b))
            }
            Node::First(pair) => {
                let pair = self.substitute_family_body(pair, arguments, family, memo)?;
                self.store.intern(Node::First(pair))
            }
            Node::Second(pair) => {
                let pair = self.substitute_family_body(pair, arguments, family, memo)?;
                self.store.intern(Node::Second(pair))
            }
            Node::Record(fields) => {
                let fields = fields
                    .into_iter()
                    .map(|(name, value)| {
                        Ok((
                            name,
                            self.substitute_family_body(value, arguments, family, memo)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ReduceError>>()?;
                self.store.intern(Node::Record(fields))
            }
            Node::Get { record, field } => {
                let record = self.substitute_family_body(record, arguments, family, memo)?;
                self.store.intern(Node::Get { record, field })
            }
            Node::Put {
                record,
                field,
                value,
            } => {
                let record = self.substitute_family_body(record, arguments, family, memo)?;
                let value = self.substitute_family_body(value, arguments, family, memo)?;
                self.store.intern(Node::Put {
                    record,
                    field,
                    value,
                })
            }
            Node::Apply {
                function,
                arguments: nested,
            } => {
                let function = self.substitute_family_body(function, arguments, family, memo)?;
                let nested = nested
                    .into_iter()
                    .map(|value| self.substitute_family_body(value, arguments, family, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Apply {
                    function,
                    arguments: nested,
                })
            }
            Node::Emit { token, message } => {
                let token = self.substitute_family_body(token, arguments, family, memo)?;
                let message = self.substitute_family_body(message, arguments, family, memo)?;
                self.store.intern(Node::Emit { token, message })
            }
            Node::Dispatch {
                family: nested_family,
                arguments: nested,
            } => {
                let nested_family =
                    self.substitute_family_body(nested_family, arguments, family, memo)?;
                let nested = nested
                    .into_iter()
                    .map(|value| self.substitute_family_body(value, arguments, family, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Dispatch {
                    family: nested_family,
                    arguments: nested,
                })
            }
        };
        memo.insert(cid, result);
        Ok(result)
    }

    fn substitute_family_binary(
        &mut self,
        a: Cid,
        b: Cid,
        arguments: &[Cid],
        family: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<(Cid, Cid), ReduceError> {
        Ok((
            self.substitute_family_body(a, arguments, family, memo)?,
            self.substitute_family_body(b, arguments, family, memo)?,
        ))
    }

    fn substitute(
        &mut self,
        cid: Cid,
        arguments: &[Cid],
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        self.enter_depth()?;
        let result = self.substitute_inner(cid, arguments, memo);
        self.depth -= 1;
        result
    }

    fn substitute_inner(
        &mut self,
        cid: Cid,
        arguments: &[Cid],
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        if let Some(result) = memo.get(&cid) {
            return Ok(*result);
        }
        self.charge()?;
        let node = self
            .store
            .get(cid)
            .cloned()
            .ok_or(ReduceError::MissingNode(cid))?;
        let result = match node {
            Node::Param(index) => arguments.get(usize::from(index)).copied().ok_or(
                ReduceError::ParameterOutOfRange {
                    index,
                    arguments: arguments.len(),
                },
            )?,
            // E0 quotations do not capture through a nested quotation.
            Node::Quote { .. } | Node::Family { .. } | Node::Const(_) | Node::Hole(_) => cid,
            Node::Add(a, b) => {
                let (a, b) = self.substitute_binary(a, b, arguments, memo)?;
                self.store.intern(Node::Add(a, b))
            }
            Node::Mul(a, b) => {
                let (a, b) = self.substitute_binary(a, b, arguments, memo)?;
                self.store.intern(Node::Mul(a, b))
            }
            Node::Eq(a, b) => {
                let (a, b) = self.substitute_binary(a, b, arguments, memo)?;
                self.store.intern(Node::Eq(a, b))
            }
            Node::If {
                condition,
                when_true,
                when_false,
            } => {
                let condition = self.substitute(condition, arguments, memo)?;
                let when_true = self.substitute(when_true, arguments, memo)?;
                let when_false = self.substitute(when_false, arguments, memo)?;
                self.store.intern(Node::If {
                    condition,
                    when_true,
                    when_false,
                })
            }
            Node::Pair(a, b) => {
                let (a, b) = self.substitute_binary(a, b, arguments, memo)?;
                self.store.intern(Node::Pair(a, b))
            }
            Node::First(pair) => {
                let pair = self.substitute(pair, arguments, memo)?;
                self.store.intern(Node::First(pair))
            }
            Node::Second(pair) => {
                let pair = self.substitute(pair, arguments, memo)?;
                self.store.intern(Node::Second(pair))
            }
            Node::Record(fields) => {
                let fields = fields
                    .into_iter()
                    .map(|(name, value)| Ok((name, self.substitute(value, arguments, memo)?)))
                    .collect::<Result<Vec<_>, ReduceError>>()?;
                self.store.intern(Node::Record(fields))
            }
            Node::Get { record, field } => {
                let record = self.substitute(record, arguments, memo)?;
                self.store.intern(Node::Get { record, field })
            }
            Node::Put {
                record,
                field,
                value,
            } => {
                let record = self.substitute(record, arguments, memo)?;
                let value = self.substitute(value, arguments, memo)?;
                self.store.intern(Node::Put {
                    record,
                    field,
                    value,
                })
            }
            Node::Apply {
                function,
                arguments: nested,
            } => {
                let function = self.substitute(function, arguments, memo)?;
                let nested = nested
                    .into_iter()
                    .map(|value| self.substitute(value, arguments, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Apply {
                    function,
                    arguments: nested,
                })
            }
            Node::Emit { token, message } => {
                let token = self.substitute(token, arguments, memo)?;
                let message = self.substitute(message, arguments, memo)?;
                self.store.intern(Node::Emit { token, message })
            }
            Node::Dispatch {
                family,
                arguments: nested,
            } => {
                let family = self.substitute(family, arguments, memo)?;
                let nested = nested
                    .into_iter()
                    .map(|value| self.substitute(value, arguments, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Dispatch {
                    family,
                    arguments: nested,
                })
            }
            Node::Recur(nested) => {
                let nested = nested
                    .into_iter()
                    .map(|value| self.substitute(value, arguments, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Recur(nested))
            }
        };
        memo.insert(cid, result);
        Ok(result)
    }

    fn substitute_binary(
        &mut self,
        a: Cid,
        b: Cid,
        arguments: &[Cid],
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<(Cid, Cid), ReduceError> {
        Ok((
            self.substitute(a, arguments, memo)?,
            self.substitute(b, arguments, memo)?,
        ))
    }

    /// Replace known named holes without otherwise evaluating the graph.  This
    /// is used beneath an unresolved lazy branch: compile-time facts must be
    /// captured, but branch errors and divergence must remain deferred.
    fn instantiate_bindings(
        &mut self,
        cid: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        self.enter_depth()?;
        let result = self.instantiate_bindings_inner(cid, memo);
        self.depth -= 1;
        result
    }

    fn instantiate_bindings_inner(
        &mut self,
        cid: Cid,
        memo: &mut BTreeMap<Cid, Cid>,
    ) -> Result<Cid, ReduceError> {
        if let Some(result) = memo.get(&cid) {
            return Ok(*result);
        }
        self.charge()?;
        let node = self
            .store
            .get(cid)
            .cloned()
            .ok_or(ReduceError::MissingNode(cid))?;
        let result = match node {
            Node::Const(_) | Node::Param(_) | Node::Quote { .. } | Node::Family { .. } => cid,
            Node::Hole(name) => self.bindings.get(&name).unwrap_or(cid),
            Node::Add(a, b) => {
                let a = self.instantiate_bindings(a, memo)?;
                let b = self.instantiate_bindings(b, memo)?;
                self.store.intern(Node::Add(a, b))
            }
            Node::Mul(a, b) => {
                let a = self.instantiate_bindings(a, memo)?;
                let b = self.instantiate_bindings(b, memo)?;
                self.store.intern(Node::Mul(a, b))
            }
            Node::Eq(a, b) => {
                let a = self.instantiate_bindings(a, memo)?;
                let b = self.instantiate_bindings(b, memo)?;
                self.store.intern(Node::Eq(a, b))
            }
            Node::If {
                condition,
                when_true,
                when_false,
            } => {
                let condition = self.instantiate_bindings(condition, memo)?;
                let when_true = self.instantiate_bindings(when_true, memo)?;
                let when_false = self.instantiate_bindings(when_false, memo)?;
                self.store.intern(Node::If {
                    condition,
                    when_true,
                    when_false,
                })
            }
            Node::Pair(a, b) => {
                let a = self.instantiate_bindings(a, memo)?;
                let b = self.instantiate_bindings(b, memo)?;
                self.store.intern(Node::Pair(a, b))
            }
            Node::First(pair) => {
                let pair = self.instantiate_bindings(pair, memo)?;
                self.store.intern(Node::First(pair))
            }
            Node::Second(pair) => {
                let pair = self.instantiate_bindings(pair, memo)?;
                self.store.intern(Node::Second(pair))
            }
            Node::Record(fields) => {
                let fields = fields
                    .into_iter()
                    .map(|(name, value)| Ok((name, self.instantiate_bindings(value, memo)?)))
                    .collect::<Result<Vec<_>, ReduceError>>()?;
                self.store.intern(Node::Record(fields))
            }
            Node::Get { record, field } => {
                let record = self.instantiate_bindings(record, memo)?;
                self.store.intern(Node::Get { record, field })
            }
            Node::Put {
                record,
                field,
                value,
            } => {
                let record = self.instantiate_bindings(record, memo)?;
                let value = self.instantiate_bindings(value, memo)?;
                self.store.intern(Node::Put {
                    record,
                    field,
                    value,
                })
            }
            Node::Apply {
                function,
                arguments,
            } => {
                let function = self.instantiate_bindings(function, memo)?;
                let arguments = arguments
                    .into_iter()
                    .map(|argument| self.instantiate_bindings(argument, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Apply {
                    function,
                    arguments,
                })
            }
            Node::Emit { token, message } => {
                let token = self.instantiate_bindings(token, memo)?;
                let message = self.instantiate_bindings(message, memo)?;
                self.store.intern(Node::Emit { token, message })
            }
            Node::Dispatch { family, arguments } => {
                let family = self.instantiate_bindings(family, memo)?;
                let arguments = arguments
                    .into_iter()
                    .map(|argument| self.instantiate_bindings(argument, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Dispatch { family, arguments })
            }
            Node::Recur(arguments) => {
                let arguments = arguments
                    .into_iter()
                    .map(|argument| self.instantiate_bindings(argument, memo))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store.intern(Node::Recur(arguments))
            }
        };
        memo.insert(cid, result);
        Ok(result)
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
        fn visit(
            reducer: &mut Reducer<'_>,
            cid: Cid,
            bound_parameters: bool,
            memo: &mut BTreeMap<(Cid, bool), bool>,
            visiting: &mut BTreeSet<Cid>,
        ) -> Result<bool, ReduceError> {
            if let Some(ground) = memo.get(&(cid, bound_parameters)) {
                return Ok(*ground);
            }
            if !visiting.insert(cid) {
                return Err(ReduceError::Cycle(cid));
            }
            reducer.charge()?;
            let node = reducer
                .store
                .get(cid)
                .cloned()
                .ok_or(ReduceError::MissingNode(cid))?;
            let ground = match node {
                Node::Const(_) => true,
                Node::Hole(_) => false,
                Node::Param(_) => bound_parameters,
                Node::Quote { .. } | Node::Family { .. } => {
                    reducer.validate_code_value(cid)?;
                    true
                }
                _ => {
                    let mut ground = true;
                    for child in node.children() {
                        ground &= visit(reducer, child, bound_parameters, memo, visiting)?;
                    }
                    ground
                }
            };
            visiting.remove(&cid);
            memo.insert((cid, bound_parameters), ground);
            Ok(ground)
        }
        visit(
            self,
            root,
            false,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
    }

    /// Conservative E0 linearity check for the explicit effect carrier.  It
    /// follows reachable DAG edges, marks every `Emit` result and everything
    /// feeding an emit token as linear, then rejects shared consumers.  A
    /// typed successor should make linearity part of the typing judgment and
    /// treat mutually exclusive branches more precisely.
    fn validate_linearity(&mut self, root: Cid) -> Result<(), ReduceError> {
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

        let mut linear = nodes
            .iter()
            .filter_map(|(cid, node)| match node {
                Node::Const(Atom::Trace(_)) | Node::Emit { .. } => Some(*cid),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for node in nodes.values() {
            if let Node::Emit { token, .. } = node {
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
        b"march6/reducer/v3",
        b"lazy-quote-and-arguments;ordered-guarded-families;lexical-recur;pure-explicit-effects",
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
