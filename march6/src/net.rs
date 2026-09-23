use crate::cid::{Cid, put_cid, put_str, put_u16, put_u64};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Atom {
    Int(i64),
    Bool(bool),
    Text(String),
    /// An immutable stand-in for an ordered external-effect token.  The E0
    /// reducer records intended effects; only a host adapter may perform them.
    Trace(Vec<String>),
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clause {
    /// A pure Boolean graph template over the enclosing family's parameters.
    /// Literal `true` is the ordinary representation of `otherwise`.
    pub guard: Cid,
    pub body: Cid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Const(Atom),
    /// A named fact that may be supplied during any reduction stage.
    Hole(String),
    /// A quotation-local parameter.  E0 deliberately excludes nested capture.
    Param(u16),
    Add(Cid, Cid),
    Mul(Cid, Cid),
    Eq(Cid, Cid),
    If {
        condition: Cid,
        when_true: Cid,
        when_false: Cid,
    },
    Pair(Cid, Cid),
    First(Cid),
    Second(Cid),
    Record(Vec<(String, Cid)>),
    Get {
        record: Cid,
        field: String,
    },
    Put {
        record: Cid,
        field: String,
        value: Cid,
    },
    Quote {
        params: u16,
        body: Cid,
    },
    Apply {
        function: Cid,
        arguments: Vec<Cid>,
    },
    /// Consume one immutable trace token and produce its successor.  Nesting
    /// these nodes makes effect order a graph dependency rather than ambient
    /// evaluator state.
    Emit {
        token: Cid,
        message: Cid,
    },
    /// A sealed, ordered family of condition-free bodies.  `Param` and
    /// `Recur` are lexically bound by this node.  Bodies stay dormant until a
    /// dispatch selects one of them.
    Family {
        parameters: u16,
        clauses: Vec<Clause>,
    },
    /// Demand-driven application of a guarded family.
    Dispatch {
        family: Cid,
        arguments: Vec<Cid>,
    },
    /// A family-local recursive call.  It is replaced with `Dispatch` when a
    /// selected clause is instantiated, avoiding cyclic content identities.
    Recur(Vec<Cid>),
}

impl Node {
    fn canonicalize(self) -> Self {
        match self {
            Self::Record(mut fields) => {
                fields.sort_by(|left, right| left.0.cmp(&right.0));
                for pair in fields.windows(2) {
                    assert_ne!(pair[0].0, pair[1].0, "duplicate record field");
                }
                Self::Record(fields)
            }
            other => other,
        }
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::Const(atom) => {
                out.push(0);
                encode_atom(&mut out, atom);
            }
            Self::Hole(name) => {
                out.push(1);
                put_str(&mut out, name);
            }
            Self::Param(index) => {
                out.push(2);
                put_u16(&mut out, *index);
            }
            Self::Add(a, b) => encode_binary(&mut out, 3, *a, *b),
            Self::Mul(a, b) => encode_binary(&mut out, 4, *a, *b),
            Self::Eq(a, b) => encode_binary(&mut out, 5, *a, *b),
            Self::If {
                condition,
                when_true,
                when_false,
            } => {
                out.push(6);
                put_cid(&mut out, *condition);
                put_cid(&mut out, *when_true);
                put_cid(&mut out, *when_false);
            }
            Self::Pair(a, b) => encode_binary(&mut out, 7, *a, *b),
            Self::First(pair) => encode_unary(&mut out, 8, *pair),
            Self::Second(pair) => encode_unary(&mut out, 9, *pair),
            Self::Record(fields) => {
                out.push(10);
                put_u64(&mut out, fields.len() as u64);
                for (name, value) in fields {
                    put_str(&mut out, name);
                    put_cid(&mut out, *value);
                }
            }
            Self::Get { record, field } => {
                out.push(11);
                put_cid(&mut out, *record);
                put_str(&mut out, field);
            }
            Self::Put {
                record,
                field,
                value,
            } => {
                out.push(12);
                put_cid(&mut out, *record);
                put_str(&mut out, field);
                put_cid(&mut out, *value);
            }
            Self::Quote { params, body } => {
                out.push(13);
                put_u16(&mut out, *params);
                put_cid(&mut out, *body);
            }
            Self::Apply {
                function,
                arguments,
            } => {
                out.push(14);
                put_cid(&mut out, *function);
                put_u64(&mut out, arguments.len() as u64);
                for argument in arguments {
                    put_cid(&mut out, *argument);
                }
            }
            Self::Emit { token, message } => {
                out.push(15);
                put_cid(&mut out, *token);
                put_cid(&mut out, *message);
            }
            Self::Family {
                parameters,
                clauses,
            } => {
                out.push(16);
                put_u16(&mut out, *parameters);
                put_u64(&mut out, clauses.len() as u64);
                for clause in clauses {
                    put_cid(&mut out, clause.guard);
                    put_cid(&mut out, clause.body);
                }
            }
            Self::Dispatch { family, arguments } => {
                out.push(17);
                put_cid(&mut out, *family);
                put_u64(&mut out, arguments.len() as u64);
                for argument in arguments {
                    put_cid(&mut out, *argument);
                }
            }
            Self::Recur(arguments) => {
                out.push(18);
                put_u64(&mut out, arguments.len() as u64);
                for argument in arguments {
                    put_cid(&mut out, *argument);
                }
            }
        }
        out
    }

    pub(crate) fn children(&self) -> Vec<Cid> {
        match self {
            Self::Const(_) | Self::Hole(_) | Self::Param(_) => Vec::new(),
            Self::Add(a, b) | Self::Mul(a, b) | Self::Eq(a, b) | Self::Pair(a, b) => {
                vec![*a, *b]
            }
            Self::If {
                condition,
                when_true,
                when_false,
            } => vec![*condition, *when_true, *when_false],
            Self::First(value) | Self::Second(value) => vec![*value],
            Self::Record(fields) => fields.iter().map(|(_, value)| *value).collect(),
            Self::Get { record, .. } => vec![*record],
            Self::Put { record, value, .. } => vec![*record, *value],
            Self::Quote { body, .. } => vec![*body],
            Self::Apply {
                function,
                arguments,
            } => std::iter::once(*function)
                .chain(arguments.iter().copied())
                .collect(),
            Self::Emit { token, message } => vec![*token, *message],
            Self::Family { clauses, .. } => clauses
                .iter()
                .flat_map(|clause| [clause.guard, clause.body])
                .collect(),
            Self::Dispatch { family, arguments } => std::iter::once(*family)
                .chain(arguments.iter().copied())
                .collect(),
            Self::Recur(arguments) => arguments.clone(),
        }
    }
}

fn encode_atom(out: &mut Vec<u8>, atom: &Atom) {
    match atom {
        Atom::Int(value) => {
            out.push(0);
            out.extend_from_slice(&value.to_be_bytes());
        }
        Atom::Bool(value) => {
            out.push(1);
            out.push(u8::from(*value));
        }
        Atom::Text(value) => {
            out.push(2);
            put_str(out, value);
        }
        Atom::Trace(entries) => {
            out.push(3);
            put_u64(out, entries.len() as u64);
            for entry in entries {
                put_str(out, entry);
            }
        }
        Atom::Unit => out.push(4),
    }
}

fn encode_binary(out: &mut Vec<u8>, tag: u8, a: Cid, b: Cid) {
    out.push(tag);
    put_cid(out, a);
    put_cid(out, b);
}

fn encode_unary(out: &mut Vec<u8>, tag: u8, value: Cid) {
    out.push(tag);
    put_cid(out, value);
}

#[derive(Default)]
pub struct Store {
    pub(crate) nodes: BTreeMap<Cid, Node>,
    /// Successful closure/purity checks are immutable facts about a CID and
    /// can be reused across reduction epochs.
    pub(crate) validated_code: std::collections::BTreeSet<Cid>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, node: Node) -> Cid {
        let node = node.canonicalize();
        let cid = Cid::digest(b"march6/node/v1", &node.canonical_bytes());
        match self.nodes.get(&cid) {
            Some(existing) => assert_eq!(
                existing, &node,
                "content-ID collision between distinct canonical nodes"
            ),
            None => {
                self.nodes.insert(cid, node);
            }
        }
        cid
    }

    pub fn get(&self, cid: Cid) -> Option<&Node> {
        self.nodes.get(&cid)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn artifact_cid(&self, roots: &[Cid]) -> Cid {
        let mut bytes = Vec::new();
        put_u64(&mut bytes, roots.len() as u64);
        for root in roots {
            put_cid(&mut bytes, *root);
        }
        Cid::digest(b"march6/artifact/v1", &bytes)
    }

    pub fn format(&self, root: Cid) -> String {
        match self.get(root) {
            None => format!("<missing:{}>", root.short()),
            Some(Node::Const(Atom::Int(value))) => value.to_string(),
            Some(Node::Const(Atom::Bool(value))) => value.to_string(),
            Some(Node::Const(Atom::Text(value))) => format!("{value:?}"),
            Some(Node::Const(Atom::Trace(entries))) => format!("trace{entries:?}"),
            Some(Node::Const(Atom::Unit)) => "unit".into(),
            Some(Node::Hole(name)) => format!("?{name}"),
            Some(Node::Param(index)) => format!("${index}"),
            Some(Node::Add(a, b)) => self.format_binary("+", *a, *b),
            Some(Node::Mul(a, b)) => self.format_binary("*", *a, *b),
            Some(Node::Eq(a, b)) => self.format_binary("=", *a, *b),
            Some(Node::If {
                condition,
                when_true,
                when_false,
            }) => format!(
                "(if {} {} {})",
                self.format(*condition),
                self.format(*when_true),
                self.format(*when_false)
            ),
            Some(Node::Pair(a, b)) => format!("(pair {} {})", self.format(*a), self.format(*b)),
            Some(Node::First(pair)) => format!("(first {})", self.format(*pair)),
            Some(Node::Second(pair)) => format!("(second {})", self.format(*pair)),
            Some(Node::Record(fields)) => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", self.format(*value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{fields}}}")
            }
            Some(Node::Get { record, field }) => {
                format!("(get {} {field:?})", self.format(*record))
            }
            Some(Node::Put {
                record,
                field,
                value,
            }) => format!(
                "(put {} {field:?} {})",
                self.format(*record),
                self.format(*value)
            ),
            Some(Node::Quote { params, body }) => {
                format!("(quote/{params} {})", self.format(*body))
            }
            Some(Node::Apply {
                function,
                arguments,
            }) => {
                let args = arguments
                    .iter()
                    .map(|arg| self.format(*arg))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("(apply {} {args})", self.format(*function))
            }
            Some(Node::Emit { token, message }) => {
                format!("(emit {} {})", self.format(*token), self.format(*message))
            }
            Some(Node::Family {
                parameters,
                clauses,
            }) => {
                let clauses = clauses
                    .iter()
                    .map(|clause| {
                        format!(
                            "{} => {}",
                            self.format(clause.guard),
                            self.format(clause.body)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("(family/{parameters} [{clauses}])")
            }
            Some(Node::Dispatch { family, arguments }) => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.format(*argument))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("(dispatch {} {arguments})", family.short())
            }
            Some(Node::Recur(arguments)) => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.format(*argument))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("(recur {arguments})")
            }
        }
    }

    fn format_binary(&self, op: &str, a: Cid, b: Cid) -> String {
        format!("({op} {} {})", self.format(a), self.format(b))
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Bindings(pub BTreeMap<String, Cid>);

impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: Cid) {
        self.0.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<Cid> {
        self.0.get(name).copied()
    }

    pub fn merged(&self, later: &Self) -> Result<Self, BindingConflict> {
        let mut result = self.clone();
        for (name, value) in &later.0 {
            if let Some(earlier) = result.0.insert(name.clone(), *value)
                && earlier != *value
            {
                return Err(BindingConflict(name.clone()));
            }
        }
        Ok(result)
    }

    pub fn cid(&self) -> Cid {
        let mut bytes = Vec::new();
        put_u64(&mut bytes, self.0.len() as u64);
        for (name, value) in &self.0 {
            put_str(&mut bytes, name);
            put_cid(&mut bytes, *value);
        }
        Cid::digest(b"march6/bindings/v1", &bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingConflict(pub String);

impl fmt::Display for BindingConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "binding {:?} was supplied with two values", self.0)
    }
}

impl std::error::Error for BindingConflict {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_tag_is_part_of_identity() {
        let mut store = Store::new();
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        assert_ne!(
            store.intern(Node::Add(one, two)),
            store.intern(Node::Mul(one, two))
        );
    }

    #[test]
    fn record_order_is_canonical() {
        let mut store = Store::new();
        let one = store.intern(Node::Const(Atom::Int(1)));
        let two = store.intern(Node::Const(Atom::Int(2)));
        let left = store.intern(Node::Record(vec![("a".into(), one), ("b".into(), two)]));
        let right = store.intern(Node::Record(vec![("b".into(), two), ("a".into(), one)]));
        assert_eq!(left, right);
    }
}
