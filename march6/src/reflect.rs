//! Syntax-neutral reflection from ordinary March values into executable code.
//!
//! A description is a record with a text `op` field.  Child descriptions are
//! records too; variable-length children use `Pair(head, tail)` lists ending
//! in `Unit`.  `embed` is the only escape from the description language: its
//! value is an existing graph edge, not a textual CID.  This keeps image
//! reachability explicit and prevents the nucleus from learning any surface
//! spelling such as `:` or `;`.

use crate::cid::Cid;
use crate::net::{Atom, Clause, Node, Store};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReflectError {
    MissingNode(Cid),
    ExpectedDescriptionRecord(Cid),
    ExpectedText {
        field: String,
        value: Cid,
    },
    ExpectedInteger {
        field: String,
        value: Cid,
    },
    ExpectedBoolean {
        field: String,
        value: Cid,
    },
    IntegerOutOfRange {
        field: String,
        value: i64,
    },
    MissingField {
        operation: String,
        field: String,
    },
    UnexpectedFields {
        operation: String,
        fields: Vec<String>,
    },
    UnknownOperation(String),
    ExpectedList(Cid),
    CyclicList(Cid),
    ExpectedPair {
        context: String,
        value: Cid,
    },
    ExpectedCodeValue(Cid),
    DuplicateRecordField(String),
    CyclicDescription(Cid),
    WorkLimit,
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNode(cid) => {
                write!(f, "reflection description refers to missing node {cid}")
            }
            Self::ExpectedDescriptionRecord(cid) => {
                write!(f, "reflection description {cid} is not a record")
            }
            Self::ExpectedText { field, value } => {
                write!(f, "reflection field {field:?} at {value} is not text")
            }
            Self::ExpectedInteger { field, value } => {
                write!(f, "reflection field {field:?} at {value} is not an integer")
            }
            Self::ExpectedBoolean { field, value } => {
                write!(f, "reflection field {field:?} at {value} is not a Boolean")
            }
            Self::IntegerOutOfRange { field, value } => {
                write!(
                    f,
                    "reflection field {field:?} value {value} is outside 0..=65535"
                )
            }
            Self::MissingField { operation, field } => {
                write!(
                    f,
                    "reflection operation {operation:?} is missing field {field:?}"
                )
            }
            Self::UnexpectedFields { operation, fields } => {
                write!(
                    f,
                    "reflection operation {operation:?} has unexpected fields {fields:?}"
                )
            }
            Self::UnknownOperation(operation) => {
                write!(f, "unknown reflection operation {operation:?}")
            }
            Self::ExpectedList(cid) => {
                write!(
                    f,
                    "reflection value {cid} is not a pair list ending in unit"
                )
            }
            Self::CyclicList(cid) => write!(f, "cyclic reflection list at {cid}"),
            Self::ExpectedPair { context, value } => {
                write!(f, "reflection {context} entry {value} is not a pair")
            }
            Self::ExpectedCodeValue(cid) => {
                write!(
                    f,
                    "reflection embed value {cid} is not a quotation or family"
                )
            }
            Self::DuplicateRecordField(field) => {
                write!(f, "reflected record repeats field {field:?}")
            }
            Self::CyclicDescription(cid) => {
                write!(f, "cyclic reflection description at {cid}")
            }
            Self::WorkLimit => f.write_str("reflection exhausted its work allowance"),
        }
    }
}

impl std::error::Error for ReflectError {}

pub(crate) struct Reflected {
    pub root: Cid,
    pub work: usize,
    pub peak_frames: usize,
    inserted: Vec<Cid>,
}

impl Reflected {
    pub fn rollback(self, store: &mut Store) {
        rollback(store, &self.inserted);
    }
}

#[derive(Debug)]
enum Frame {
    Enter(Cid),
    Exit {
        description: Cid,
        build: Build,
        children: usize,
    },
}

#[derive(Debug)]
enum Build {
    Add,
    Multiply,
    Equal,
    If,
    Pair,
    First,
    Second,
    Record(Vec<String>),
    Get(String),
    Put(String),
    Quote(u16),
    Apply,
    Emit,
    Family { parameters: u16, clauses: usize },
    Dispatch,
    Recur,
    Intern,
}

enum Decoded {
    Immediate(Cid),
    Deferred { build: Build, children: Vec<Cid> },
}

/// Decode and intern a normalized, ground description.  Newly inserted graph
/// nodes are rolled back on every decoding failure.  Closure and guard-purity
/// validation are intentionally performed by the reducer afterward so there
/// remains one authority for those invariants.
pub(crate) fn intern_description(
    store: &mut Store,
    root: Cid,
    work_limit: usize,
) -> Result<Reflected, ReflectError> {
    let mut inserted = Vec::new();
    let result = {
        let mut decoder = Decoder {
            store,
            inserted: &mut inserted,
            work: 0,
            work_limit,
            peak_frames: 0,
        };
        decoder.decode(root)
    };
    match result {
        Ok((root, work, peak_frames)) => Ok(Reflected {
            root,
            work,
            peak_frames,
            inserted,
        }),
        Err(error) => {
            rollback(store, &inserted);
            Err(error)
        }
    }
}

fn rollback(store: &mut Store, inserted: &[Cid]) {
    for cid in inserted.iter().rev() {
        store.nodes.remove(cid);
        store.validated_code.remove(cid);
    }
}

struct Decoder<'a> {
    store: &'a mut Store,
    inserted: &'a mut Vec<Cid>,
    work: usize,
    work_limit: usize,
    peak_frames: usize,
}

impl Decoder<'_> {
    fn decode(&mut self, root: Cid) -> Result<(Cid, usize, usize), ReflectError> {
        let mut frames = vec![Frame::Enter(root)];
        let mut values = Vec::new();
        let mut memo = BTreeMap::new();
        let mut visiting = BTreeSet::new();

        while let Some(frame) = frames.pop() {
            self.peak_frames = self.peak_frames.max(frames.len() + 1);
            match frame {
                Frame::Enter(description) => {
                    self.tick()?;
                    if let Some(value) = memo.get(&description).copied() {
                        values.push(value);
                        continue;
                    }
                    if !visiting.insert(description) {
                        return Err(ReflectError::CyclicDescription(description));
                    }
                    match self.decode_one(description)? {
                        Decoded::Immediate(value) => {
                            visiting.remove(&description);
                            memo.insert(description, value);
                            values.push(value);
                        }
                        Decoded::Deferred { build, children } => {
                            frames.push(Frame::Exit {
                                description,
                                children: children.len(),
                                build,
                            });
                            for child in children.into_iter().rev() {
                                frames.push(Frame::Enter(child));
                            }
                        }
                    }
                }
                Frame::Exit {
                    description,
                    build,
                    children,
                } => {
                    let first = values
                        .len()
                        .checked_sub(children)
                        .expect("reflection value-stack balance");
                    let children = values.split_off(first);
                    let node = build_node(build, children);
                    let value = self.intern(node);
                    visiting.remove(&description);
                    memo.insert(description, value);
                    values.push(value);
                }
            }
        }

        assert_eq!(values.len(), 1, "reflection value-stack imbalance");
        Ok((
            values.pop().expect("reflection result"),
            self.work,
            self.peak_frames,
        ))
    }

    fn tick(&mut self) -> Result<(), ReflectError> {
        if self.work == self.work_limit {
            return Err(ReflectError::WorkLimit);
        }
        self.work += 1;
        Ok(())
    }

    fn decode_one(&mut self, description: Cid) -> Result<Decoded, ReflectError> {
        let (operation, mut fields) = self.description_fields(description)?;
        let decoded = match operation.as_str() {
            "int" => {
                exact_fields(&operation, &fields, &["value"])?;
                let value = self.expect_i64(take(&operation, &mut fields, "value")?, "value")?;
                Decoded::Immediate(self.intern(Node::Const(Atom::Int(value))))
            }
            "bool" => {
                exact_fields(&operation, &fields, &["value"])?;
                let value = self.expect_bool(take(&operation, &mut fields, "value")?, "value")?;
                Decoded::Immediate(self.intern(Node::Const(Atom::Bool(value))))
            }
            "text" => {
                exact_fields(&operation, &fields, &["value"])?;
                let value = self.expect_text(take(&operation, &mut fields, "value")?, "value")?;
                Decoded::Immediate(self.intern(Node::Const(Atom::Text(value))))
            }
            "unit" => {
                exact_fields(&operation, &fields, &[])?;
                Decoded::Immediate(self.intern(Node::Const(Atom::Unit)))
            }
            "embed" => {
                exact_fields(&operation, &fields, &["value"])?;
                let value = take(&operation, &mut fields, "value")?;
                match self.store.get(value) {
                    Some(Node::Quote { .. } | Node::Family { .. }) => {}
                    Some(_) => return Err(ReflectError::ExpectedCodeValue(value)),
                    None => return Err(ReflectError::MissingNode(value)),
                }
                Decoded::Immediate(value)
            }
            "param" => {
                exact_fields(&operation, &fields, &["index"])?;
                let index = self.expect_u16(take(&operation, &mut fields, "index")?, "index")?;
                Decoded::Immediate(self.intern(Node::Param(index)))
            }
            "add" => binary(&operation, &mut fields, Build::Add, "left", "right")?,
            "multiply" => binary(&operation, &mut fields, Build::Multiply, "left", "right")?,
            "equal" => binary(&operation, &mut fields, Build::Equal, "left", "right")?,
            "pair" => binary(&operation, &mut fields, Build::Pair, "first", "second")?,
            "if" => {
                exact_fields(&operation, &fields, &["condition", "true", "false"])?;
                Decoded::Deferred {
                    build: Build::If,
                    children: vec![
                        take(&operation, &mut fields, "condition")?,
                        take(&operation, &mut fields, "true")?,
                        take(&operation, &mut fields, "false")?,
                    ],
                }
            }
            "first" | "second" => {
                exact_fields(&operation, &fields, &["pair"])?;
                Decoded::Deferred {
                    build: if operation == "first" {
                        Build::First
                    } else {
                        Build::Second
                    },
                    children: vec![take(&operation, &mut fields, "pair")?],
                }
            }
            "record" => {
                exact_fields(&operation, &fields, &["fields"])?;
                let entries = self.list(take(&operation, &mut fields, "fields")?)?;
                let mut names = Vec::with_capacity(entries.len());
                let mut children = Vec::with_capacity(entries.len());
                let mut seen = BTreeSet::new();
                for entry in entries {
                    self.tick()?;
                    let (name, value) = self.pair(entry, "record-field")?;
                    let name = self.expect_text(name, "record field name")?;
                    if !seen.insert(name.clone()) {
                        return Err(ReflectError::DuplicateRecordField(name));
                    }
                    names.push(name);
                    children.push(value);
                }
                Decoded::Deferred {
                    build: Build::Record(names),
                    children,
                }
            }
            "get" => {
                exact_fields(&operation, &fields, &["record", "field"])?;
                let record = take(&operation, &mut fields, "record")?;
                let field = self.expect_text(take(&operation, &mut fields, "field")?, "field")?;
                Decoded::Deferred {
                    build: Build::Get(field),
                    children: vec![record],
                }
            }
            "put" => {
                exact_fields(&operation, &fields, &["record", "field", "value"])?;
                let record = take(&operation, &mut fields, "record")?;
                let field = self.expect_text(take(&operation, &mut fields, "field")?, "field")?;
                let value = take(&operation, &mut fields, "value")?;
                Decoded::Deferred {
                    build: Build::Put(field),
                    children: vec![record, value],
                }
            }
            "quote" => {
                exact_fields(&operation, &fields, &["parameters", "body"])?;
                let parameters =
                    self.expect_u16(take(&operation, &mut fields, "parameters")?, "parameters")?;
                Decoded::Deferred {
                    build: Build::Quote(parameters),
                    children: vec![take(&operation, &mut fields, "body")?],
                }
            }
            "apply" | "dispatch" => {
                let target = if operation == "apply" {
                    "function"
                } else {
                    "family"
                };
                exact_fields(&operation, &fields, &[target, "arguments"])?;
                let head = take(&operation, &mut fields, target)?;
                let mut children = vec![head];
                children.extend(self.list(take(&operation, &mut fields, "arguments")?)?);
                Decoded::Deferred {
                    build: if operation == "apply" {
                        Build::Apply
                    } else {
                        Build::Dispatch
                    },
                    children,
                }
            }
            "emit" => {
                exact_fields(&operation, &fields, &["token", "message"])?;
                Decoded::Deferred {
                    build: Build::Emit,
                    children: vec![
                        take(&operation, &mut fields, "token")?,
                        take(&operation, &mut fields, "message")?,
                    ],
                }
            }
            "family" => {
                exact_fields(&operation, &fields, &["parameters", "clauses"])?;
                let parameters =
                    self.expect_u16(take(&operation, &mut fields, "parameters")?, "parameters")?;
                let entries = self.list(take(&operation, &mut fields, "clauses")?)?;
                let clause_count = entries.len();
                let mut children = Vec::with_capacity(clause_count.saturating_mul(2));
                for entry in entries {
                    self.tick()?;
                    let (guard, body) = self.pair(entry, "family-clause")?;
                    children.push(guard);
                    children.push(body);
                }
                Decoded::Deferred {
                    build: Build::Family {
                        parameters,
                        clauses: clause_count,
                    },
                    children,
                }
            }
            "recur" => {
                exact_fields(&operation, &fields, &["arguments"])?;
                Decoded::Deferred {
                    build: Build::Recur,
                    children: self.list(take(&operation, &mut fields, "arguments")?)?,
                }
            }
            "intern" => {
                exact_fields(&operation, &fields, &["description"])?;
                Decoded::Deferred {
                    build: Build::Intern,
                    children: vec![take(&operation, &mut fields, "description")?],
                }
            }
            _ => return Err(ReflectError::UnknownOperation(operation)),
        };
        Ok(decoded)
    }

    fn description_fields(
        &self,
        description: Cid,
    ) -> Result<(String, BTreeMap<String, Cid>), ReflectError> {
        let Node::Record(fields) = self
            .store
            .get(description)
            .cloned()
            .ok_or(ReflectError::MissingNode(description))?
        else {
            return Err(ReflectError::ExpectedDescriptionRecord(description));
        };
        let mut fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        let operation_value = fields
            .remove("op")
            .ok_or_else(|| ReflectError::MissingField {
                operation: "<description>".into(),
                field: "op".into(),
            })?;
        let operation = self.expect_text(operation_value, "op")?;
        Ok((operation, fields))
    }

    fn expect_text(&self, cid: Cid, field: &str) -> Result<String, ReflectError> {
        match self.store.get(cid) {
            Some(Node::Const(Atom::Text(value))) => Ok(value.clone()),
            Some(_) => Err(ReflectError::ExpectedText {
                field: field.into(),
                value: cid,
            }),
            None => Err(ReflectError::MissingNode(cid)),
        }
    }

    fn expect_i64(&self, cid: Cid, field: &str) -> Result<i64, ReflectError> {
        match self.store.get(cid) {
            Some(Node::Const(Atom::Int(value))) => Ok(*value),
            Some(_) => Err(ReflectError::ExpectedInteger {
                field: field.into(),
                value: cid,
            }),
            None => Err(ReflectError::MissingNode(cid)),
        }
    }

    fn expect_u16(&self, cid: Cid, field: &str) -> Result<u16, ReflectError> {
        let value = self.expect_i64(cid, field)?;
        value
            .try_into()
            .map_err(|_| ReflectError::IntegerOutOfRange {
                field: field.into(),
                value,
            })
    }

    fn expect_bool(&self, cid: Cid, field: &str) -> Result<bool, ReflectError> {
        match self.store.get(cid) {
            Some(Node::Const(Atom::Bool(value))) => Ok(*value),
            Some(_) => Err(ReflectError::ExpectedBoolean {
                field: field.into(),
                value: cid,
            }),
            None => Err(ReflectError::MissingNode(cid)),
        }
    }

    fn pair(&self, cid: Cid, context: &str) -> Result<(Cid, Cid), ReflectError> {
        match self.store.get(cid) {
            Some(Node::Pair(first, second)) => Ok((*first, *second)),
            Some(_) => Err(ReflectError::ExpectedPair {
                context: context.into(),
                value: cid,
            }),
            None => Err(ReflectError::MissingNode(cid)),
        }
    }

    fn list(&mut self, root: Cid) -> Result<Vec<Cid>, ReflectError> {
        let mut result = Vec::new();
        let mut cursor = root;
        let mut seen = BTreeSet::new();
        loop {
            self.tick()?;
            if !seen.insert(cursor) {
                return Err(ReflectError::CyclicList(cursor));
            }
            match self.store.get(cursor) {
                Some(Node::Const(Atom::Unit)) => return Ok(result),
                Some(Node::Pair(head, tail)) => {
                    result.push(*head);
                    cursor = *tail;
                }
                Some(_) => return Err(ReflectError::ExpectedList(cursor)),
                None => return Err(ReflectError::MissingNode(cursor)),
            }
        }
    }

    fn intern(&mut self, node: Node) -> Cid {
        let before = self.store.len();
        let cid = self.store.intern(node);
        if self.store.len() != before {
            self.inserted.push(cid);
        }
        cid
    }
}

fn take(
    operation: &str,
    fields: &mut BTreeMap<String, Cid>,
    field: &str,
) -> Result<Cid, ReflectError> {
    fields
        .remove(field)
        .ok_or_else(|| ReflectError::MissingField {
            operation: operation.into(),
            field: field.into(),
        })
}

fn exact_fields(
    operation: &str,
    fields: &BTreeMap<String, Cid>,
    expected: &[&str],
) -> Result<(), ReflectError> {
    for field in expected {
        if !fields.contains_key(*field) {
            return Err(ReflectError::MissingField {
                operation: operation.into(),
                field: (*field).into(),
            });
        }
    }
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    let unexpected = fields
        .keys()
        .filter(|field| !expected.contains(field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(ReflectError::UnexpectedFields {
            operation: operation.into(),
            fields: unexpected,
        })
    }
}

fn binary(
    operation: &str,
    fields: &mut BTreeMap<String, Cid>,
    build: Build,
    left: &str,
    right: &str,
) -> Result<Decoded, ReflectError> {
    exact_fields(operation, fields, &[left, right])?;
    Ok(Decoded::Deferred {
        build,
        children: vec![
            take(operation, fields, left)?,
            take(operation, fields, right)?,
        ],
    })
}

fn build_node(build: Build, children: Vec<Cid>) -> Node {
    let mut children = children.into_iter();
    match build {
        Build::Add => Node::Add(
            children.next().expect("add left description"),
            children.next().expect("add right description"),
        ),
        Build::Multiply => Node::Mul(
            children.next().expect("multiply left description"),
            children.next().expect("multiply right description"),
        ),
        Build::Equal => Node::Eq(
            children.next().expect("equal left description"),
            children.next().expect("equal right description"),
        ),
        Build::If => Node::If {
            condition: children.next().expect("if condition description"),
            when_true: children.next().expect("if true description"),
            when_false: children.next().expect("if false description"),
        },
        Build::Pair => Node::Pair(
            children.next().expect("pair first description"),
            children.next().expect("pair second description"),
        ),
        Build::First => Node::First(children.next().expect("first pair description")),
        Build::Second => Node::Second(children.next().expect("second pair description")),
        Build::Record(names) => Node::Record(names.into_iter().zip(children).collect()),
        Build::Get(field) => Node::Get {
            record: children.next().expect("get record description"),
            field,
        },
        Build::Put(field) => Node::Put {
            record: children.next().expect("put record description"),
            field,
            value: children.next().expect("put value description"),
        },
        Build::Quote(params) => Node::Quote {
            params,
            body: children.next().expect("quote body description"),
        },
        Build::Apply => Node::Apply {
            function: children.next().expect("apply function description"),
            arguments: children.collect(),
        },
        Build::Emit => Node::Emit {
            token: children.next().expect("emit token description"),
            message: children.next().expect("emit message description"),
        },
        Build::Family {
            parameters,
            clauses,
        } => {
            let clauses = (0..clauses)
                .map(|_| Clause {
                    guard: children.next().expect("family guard description"),
                    body: children.next().expect("family body description"),
                })
                .collect();
            Node::Family {
                parameters,
                clauses,
            }
        }
        Build::Dispatch => Node::Dispatch {
            family: children.next().expect("dispatch family description"),
            arguments: children.collect(),
        },
        Build::Recur => Node::Recur(children.collect()),
        Build::Intern => Node::Intern(children.next().expect("intern description")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(store: &mut Store, value: &str) -> Cid {
        store.intern(Node::Const(Atom::Text(value.into())))
    }

    fn description(store: &mut Store, operation: &str, fields: Vec<(&str, Cid)>) -> Cid {
        let operation = text(store, operation);
        let mut fields = fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect::<Vec<_>>();
        fields.push(("op".into(), operation));
        store.intern(Node::Record(fields))
    }

    #[test]
    fn budget_failure_rolls_back_nodes_built_earlier_in_the_same_decode() {
        let mut store = Store::new();
        let unit = description(&mut store, "unit", vec![]);
        let pair = description(&mut store, "pair", vec![("first", unit), ("second", unit)]);
        let parameters = store.intern(Node::Const(Atom::Int(0)));
        let quote = description(
            &mut store,
            "quote",
            vec![("parameters", parameters), ("body", pair)],
        );
        let before = store.len();

        assert!(matches!(
            intern_description(&mut store, quote, 3),
            Err(ReflectError::WorkLimit)
        ));
        assert_eq!(store.len(), before);
        assert!(
            !store
                .nodes
                .values()
                .any(|node| node == &Node::Const(Atom::Unit))
        );
    }
}
