//! Language definitions are canonical; register graphs and plans are derived.
//! WordId is a local handle only: the wire encoding always uses dependency CIDs.
use super::{Binary, Error, Literal, Op, Program, Slot, WordId, put, source, stream, text_bytes};

pub(super) const DOMAIN: &[u8] = b"march-definition-v1";

/// These spellings are stable semantic primitive identifiers, not dictionary
/// bindings. Changing a primitive's meaning requires a new identifier/version.
pub const RUNTIME_PRIMITIVES: &[&str] = &[
    "dup",
    "drop",
    "swap",
    "over",
    "+",
    "-",
    "*",
    "=",
    "<",
    "pair",
    "first",
    "second",
    "select",
    "true",
    "false",
    "unit",
    "tuple-length",
    "nth",
    "tuple-set",
    "text-bytes",
    "text-chars",
    "text-concat",
    "text-slice",
];

/// Source names are separate from the stable primitive identities above.
/// Renaming a dictionary entry must not change the CID of existing code.
pub(super) fn install_runtime(program: &mut Program) -> Result<(), Error> {
    for &semantic in RUNTIME_PRIMITIVES {
        let name = match semantic {
            "=" => "eq?",
            "<" => "lt?",
            name => name,
        };
        if program.lookup(name).is_none() {
            let word = program.add_definition(Definition::Primitive(semantic.into()))?;
            program.bind(name, word)?;
        }
    }
    // Ordinary compositions, exactly as if defined in March:
    // : gt? swap lt? ; : gte? lt? false true select ;
    // : lte? gt? false true select ;
    for (name, body) in [
        ("gt?", &["swap", "lt?"][..]),
        ("gte?", &["lt?", "false", "true", "select"][..]),
        ("lte?", &["gt?", "false", "true", "select"][..]),
    ] {
        if program.lookup(name).is_none() {
            let items =
                body.iter()
                    .map(|name| {
                        program.lookup(name).map(Item::Word).ok_or_else(|| {
                            Error::InvalidCode("missing comparison dependency".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            let word = program.add_definition(Definition::Sequence(items))?;
            program.bind(name, word)?;
        }
    }
    Ok(())
}

fn primitive(name: &str) -> Result<(usize, Vec<Op>, Vec<Slot>), Error> {
    if let Some(&(_, primitive)) = super::data::PRIMITIVES.iter().find(|(n, _)| *n == name) {
        let n = primitive.arity();
        let mut ops: Vec<_> = (0..n).map(Op::Arg).collect();
        ops.push(Op::Data {
            primitive,
            arguments: (0..n).collect(),
        });
        return Ok((n, ops, vec![n]));
    }
    let (inputs, operation, outputs) = match name {
        "dup" => (1, None, vec![0, 0]),
        "drop" => (1, None, vec![]),
        "swap" => (2, None, vec![1, 0]),
        "over" => (2, None, vec![0, 1, 0]),
        "+" | "-" | "*" | "=" | "<" => {
            let binary = match name {
                "+" => Binary::Add,
                "-" => Binary::Sub,
                "*" => Binary::Mul,
                "=" => Binary::Eq,
                "<" => Binary::Lt,
                _ => unreachable!(),
            };
            (2, Some(Op::Binary(binary, 0, 1)), vec![2])
        }
        "pair" => (2, Some(Op::Pair(0, 1)), vec![2]),
        "first" => (1, Some(Op::First(0)), vec![1]),
        "second" => (1, Some(Op::Second(0)), vec![1]),
        "select" => (
            3,
            Some(Op::Select {
                condition: 0,
                when_true: 1,
                when_false: 2,
            }),
            vec![3],
        ),
        "true" => (0, Some(Op::Const(Literal::Bool(true))), vec![0]),
        "false" => (0, Some(Op::Const(Literal::Bool(false))), vec![0]),
        "unit" => (0, Some(Op::Const(Literal::Unit)), vec![0]),
        _ => return Err(Error::InvalidCode("unknown runtime primitive".into())),
    };
    let mut ops: Vec<_> = (0..inputs).map(Op::Arg).collect();
    ops.extend(operation);
    Ok((inputs, ops, outputs))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Tuple(usize),
    Untuple(usize),
    Word(WordId),
    Literal(Literal),
    Context(String),
    StaticCall,
    // Explicit application/recursion contracts are not inferred wiring. Keep
    // them until the language can express these contracts through definitions.
    Apply { inputs: usize, outputs: usize },
    Recur { inputs: usize, outputs: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Definition {
    Primitive(String),
    Kernel(stream::Primitive),
    Sequence(Vec<Item>),
    /// Ordered first-match guard/body references. Arity follows the clauses.
    Family(Vec<(WordId, WordId)>),
}

impl Definition {
    pub(super) fn dependencies(&self) -> Vec<WordId> {
        match self {
            Self::Sequence(items) => items
                .iter()
                .filter_map(|item| match item {
                    Item::Word(w) | Item::Literal(Literal::Quote(w)) => Some(*w),
                    _ => None,
                })
                .collect(),
            Self::Family(clauses) => clauses.iter().flat_map(|&(g, b)| [g, b]).collect(),
            _ => Vec::new(),
        }
    }
}

impl Program {
    /// None denotes an in-memory evaluator fixture, not a language definition.
    /// Graph fixtures cannot be persisted or referenced by language definitions.
    pub fn definition(&self, word: WordId) -> Result<Option<&Definition>, Error> {
        Ok(self.word(word)?.definition.as_ref())
    }

    pub fn add_definition(&mut self, definition: Definition) -> Result<WordId, Error> {
        for dependency in definition.dependencies() {
            if self.definition(dependency)?.is_none() {
                return Err(Error::InvalidCode(
                    "definition cannot reference a graph-only evaluator fixture".into(),
                ));
            }
        }
        let cid = crate::Cid::digest(DOMAIN, &self.encode_definition(&definition)?);
        if let Some(&word) = self.identities.get(&cid) {
            return Ok(word);
        }
        let lowered = match &definition {
            Definition::Primitive(name) => primitive(name)?,
            Definition::Kernel(primitive) => {
                let n = primitive.arity();
                let mut ops: Vec<_> = (0..n).map(Op::Arg).collect();
                ops.push(Op::Kernel {
                    primitive: *primitive,
                    arguments: (0..n).collect(),
                });
                (n, ops, vec![n])
            }
            Definition::Sequence(items) => {
                if items.len() > 1_000_000 {
                    return Err(Error::InvalidCode("definition size limit".into()));
                }
                let mut body = source::Body::default();
                for item in items {
                    match item {
                        Item::Tuple(n) | Item::Untuple(n) => {
                            if *n > 4096 {
                                return Err(Error::InvalidCode("tuple contract limit".into()));
                            }
                            body.tuple(*n, matches!(item, Item::Untuple(_)));
                        }
                        Item::Word(w) => {
                            body.call(self, *w).map_err(|e| Error::InvalidCode(e.0))?
                        }
                        Item::Literal(v) => body.push_op(
                            Op::Const(*v),
                            match v {
                                Literal::Quote(w) => Some(*w),
                                _ => None,
                            },
                        ),
                        Item::Context(key) => body.push_op(Op::Context(key.clone()), None),
                        Item::StaticCall => body
                            .static_call(self)
                            .map_err(|e| Error::InvalidCode(e.0))?,
                        Item::Apply { inputs, outputs } | Item::Recur { inputs, outputs } => {
                            if *inputs > 4096 || *outputs > 4096 {
                                return Err(Error::InvalidCode(
                                    "definition stack contract limit".into(),
                                ));
                            }
                            body.dynamic(*inputs, *outputs, matches!(item, Item::Recur { .. }));
                        }
                    }
                    // Bound expansion too: a short sequence can reference
                    // words with many inputs/outputs.
                    if body.operation_count() > 1_000_000 {
                        return Err(Error::InvalidCode("lowered definition size limit".into()));
                    }
                }
                body.lowered()
            }
            Definition::Family(clauses) => {
                let &(_, first) = clauses
                    .first()
                    .ok_or_else(|| Error::InvalidCode("empty family".into()))?;
                let (inputs, outputs) = self.signature(first)?;
                return self.add_source_family(inputs, outputs, clauses.clone(), true);
            }
        };
        self.add_lowered(lowered.0, lowered.1, lowered.2, Some(definition))
    }

    pub(super) fn encode_definition(&self, definition: &Definition) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        match definition {
            Definition::Primitive(name) => {
                out.push(0);
                text_bytes(&mut out, name);
            }
            Definition::Kernel(primitive) => {
                out.push(1);
                out.push(*primitive as u8);
            }
            Definition::Sequence(items) => {
                out.push(2);
                put(&mut out, items.len());
                for item in items {
                    match item {
                        Item::Tuple(n) => {
                            out.push(6);
                            put(&mut out, *n);
                        }
                        Item::Untuple(n) => {
                            out.push(7);
                            put(&mut out, *n);
                        }
                        Item::Word(w) => {
                            out.push(0);
                            out.extend_from_slice(&self.cid(*w)?.0);
                        }
                        Item::Literal(v) => {
                            out.push(1);
                            self.encode_literal(&mut out, *v)?;
                        }
                        Item::Context(key) => {
                            out.push(2);
                            text_bytes(&mut out, key);
                        }
                        Item::StaticCall => out.push(3),
                        Item::Apply { inputs, outputs } | Item::Recur { inputs, outputs } => {
                            out.push(if matches!(item, Item::Apply { .. }) {
                                4
                            } else {
                                5
                            });
                            put(&mut out, *inputs);
                            put(&mut out, *outputs);
                        }
                    }
                }
            }
            Definition::Family(clauses) => {
                out.push(3);
                put(&mut out, clauses.len());
                for &(g, b) in clauses {
                    out.extend_from_slice(&self.cid(g)?.0);
                    out.extend_from_slice(&self.cid(b)?.0);
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_optimization_does_not_define_code_or_image_identity() {
        let mut p = stream::seed().unwrap();
        let entry = stream::compile(&mut p, ": square dup * ; 7 square").unwrap();
        let cid = p.cid(entry).unwrap();
        let bytes = p.to_image(entry).unwrap();
        // Disable derived plans, and replace this closed entry's graph with
        // its constant-folded result. Neither change touches the definition.
        let w = &mut p.words[entry];
        w.fast = None;
        w.tail = None;
        w.ops = vec![Op::Const(Literal::Int(49))];
        w.outputs = vec![0];
        assert_eq!(p.cid(entry).unwrap(), cid);
        assert_eq!(p.to_image(entry).unwrap(), bytes);
        assert_eq!(
            super::super::Executor::new(&p)
                .run(entry, &[], &Default::default(), 1000)
                .unwrap(),
            [super::super::Value::Int(49)]
        );
        let (loaded, loaded_entry) = Program::from_image(&bytes).unwrap();
        assert_eq!(loaded.cid(loaded_entry).unwrap(), cid);
        assert_eq!(
            super::super::Executor::new(&loaded)
                .run(loaded_entry, &[], &Default::default(), 1000)
                .unwrap(),
            [super::super::Value::Int(49)]
        );
    }
}
