//! Syntax-neutral operations used by graph-defined readers.
use super::{Atom, Cid, Node, ReduceError, Reducer, ReductionStats};

fn charge(
    remaining: &mut usize,
    stats: &mut ReductionStats,
    limit: usize,
) -> Result<(), ReduceError> {
    if *remaining == 0 {
        return Err(ReduceError::BudgetExhausted { limit });
    }
    *remaining -= 1;
    stats.steps += 1;
    Ok(())
}

fn whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

impl Reducer<'_> {
    fn optional_value(&mut self, found: bool, value: Cid) -> Cid {
        let found = self.store.intern(Node::Const(Atom::Bool(found)));
        self.store.intern(Node::Record(vec![
            ("found".into(), found),
            ("value".into(), value),
        ]))
    }

    pub(super) fn reduce_reader(
        &mut self,
        node: Node,
        children: Vec<Cid>,
    ) -> Result<Cid, ReduceError> {
        match node {
            Node::NextToken { .. } => {
                let (text, position) = (children[0], children[1]);
                match (self.store.get(text), self.store.get(position)) {
                    (
                        Some(Node::Const(Atom::Text(input))),
                        Some(Node::Const(Atom::Int(offset))),
                    ) => {
                        let mut end = usize::try_from(*offset)
                            .map_err(|_| ReduceError::Type("invalid text cursor"))?;
                        if end > input.len() || !input.is_char_boundary(end) {
                            return Err(ReduceError::Type("invalid text cursor"));
                        }
                        let bytes = input.as_bytes();
                        while end < bytes.len() && whitespace(bytes[end]) {
                            charge(&mut self.remaining_steps, &mut self.stats, self.step_limit)?;
                            end += 1;
                        }
                        let start = end;
                        while end < bytes.len() && !whitespace(bytes[end]) {
                            charge(&mut self.remaining_steps, &mut self.stats, self.step_limit)?;
                            end += 1;
                        }
                        let token = input[start..end].to_owned();
                        let offset = i64::try_from(end)
                            .map_err(|_| ReduceError::IntegerOverflow("text cursor"))?;
                        let found = self.store.intern(Node::Const(Atom::Bool(start != end)));
                        let token = self.store.intern(Node::Const(Atom::Text(token)));
                        let position = self.store.intern(Node::Const(Atom::Int(offset)));
                        Ok(self.store.intern(Node::Record(vec![
                            ("found".into(), found),
                            ("token".into(), token),
                            ("position".into(), position),
                        ])))
                    }
                    _ => {
                        self.reader_operand(text, "text", |n| {
                            matches!(n, Node::Const(Atom::Text(_)))
                        })?;
                        self.reader_operand(position, "integer cursor", |n| {
                            matches!(n, Node::Const(Atom::Int(_)))
                        })?;
                        Ok(self.store.intern(Node::NextToken { text, position }))
                    }
                }
            }
            Node::ParseInt(_) => {
                let text = children[0];
                if let Some(Node::Const(Atom::Text(input))) = self.store.get(text) {
                    // Accumulate negatively so i64::MIN is representable.
                    let bytes = input.as_bytes();
                    let negative = bytes.first() == Some(&b'-');
                    let sign = matches!(bytes.first(), Some(b'+' | b'-'));
                    let mut value = Some(0i64);
                    let mut digits = 0;
                    for (index, byte) in bytes.iter().enumerate() {
                        charge(&mut self.remaining_steps, &mut self.stats, self.step_limit)?;
                        if index == 0 && sign {
                            continue;
                        }
                        digits += 1;
                        value = if byte.is_ascii_digit() {
                            value
                                .and_then(|v| v.checked_mul(10))
                                .and_then(|v| v.checked_sub(i64::from(byte - b'0')))
                        } else {
                            None
                        };
                    }
                    if digits == 0 {
                        value = None;
                    }
                    if !negative {
                        value = value.and_then(i64::checked_neg);
                    }
                    let result = self
                        .store
                        .intern(Node::Const(value.map_or(Atom::Unit, Atom::Int)));
                    Ok(self.optional_value(value.is_some(), result))
                } else {
                    self.reader_operand(text, "text", |n| matches!(n, Node::Const(Atom::Text(_))))?;
                    Ok(self.store.intern(Node::ParseInt(text)))
                }
            }
            Node::Lookup { .. } | Node::PutKey { .. } => {
                let (record, key) = (children[0], children[1]);
                self.reader_operand(record, "record", |n| matches!(n, Node::Record(_)))?;
                self.reader_operand(key, "text key", |n| matches!(n, Node::Const(Atom::Text(_))))?;
                if let (Some(Node::Record(fields)), Some(Node::Const(Atom::Text(name)))) =
                    (self.store.get(record), self.store.get(key))
                {
                    // Account for the current flat record's traversal/copy. Persistent
                    // dictionaries are a later representation/performance experiment.
                    for _ in fields {
                        charge(&mut self.remaining_steps, &mut self.stats, self.step_limit)?;
                    }
                    let index = fields.binary_search_by(|(field, _)| field.cmp(name));
                    if matches!(node, Node::Lookup { .. }) {
                        let found = index.is_ok();
                        let value = index.map(|i| fields[i].1).ok();
                        let value =
                            value.unwrap_or_else(|| self.store.intern(Node::Const(Atom::Unit)));
                        Ok(self.optional_value(found, value))
                    } else {
                        let mut fields = fields.clone();
                        match index {
                            Ok(i) => fields[i].1 = children[2],
                            Err(i) => fields.insert(i, (name.clone(), children[2])),
                        }
                        Ok(self.store.intern(Node::Record(fields)))
                    }
                } else if matches!(node, Node::Lookup { .. }) {
                    Ok(self.store.intern(Node::Lookup { record, key }))
                } else {
                    Ok(self.store.intern(Node::PutKey {
                        record,
                        key,
                        value: children[2],
                    }))
                }
            }
            _ => unreachable!("reader frame only contains reader operations"),
        }
    }

    fn reader_operand(
        &mut self,
        cid: Cid,
        expected: &'static str,
        accepts: impl FnOnce(&Node) -> bool,
    ) -> Result<(), ReduceError> {
        if !accepts(self.store.get(cid).ok_or(ReduceError::MissingNode(cid))?)
            && self.is_ground(cid)?
        {
            return Err(ReduceError::Type(expected));
        }
        Ok(())
    }
}
