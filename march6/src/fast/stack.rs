//! Experimental strict stack baseline over canonical definitions, not lowered
//! dependency graphs. No demand cells, memoization, or hidden state-edge chain.
//! Values are independently owned immutable data, not handles into an evaluator.
//! Compiler primitives remain unsupported. It is not yet the default runtime.
mod value;
use super::definition::{Definition, Item};
use super::store::{FrozenNode, FrozenValue, Store};
use super::{Context, Error, Literal, Program, WordId};
pub use value::{Tuple, Value};

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub steps: usize,
    pub calls: usize,
    pub peak_stack: usize,
    pub peak_continuations: usize,
    /// Tail entries that reuse an identical pending return check.
    pub tail_calls: usize,
    /// Cumulative tuple allocations/field slots during this invocation, not RSS.
    pub tuple_nodes: usize,
    pub tuple_fields: usize,
    /// Newly allocated text only; literals and store imports share existing bytes.
    pub text_allocations: usize,
    pub text_bytes_allocated: usize,
    pub frozen_nodes: usize,
}

enum Task {
    Enter {
        word: WordId,
        recur: WordId,
        floor: usize,
    },
    Return {
        base: usize,
        outputs: usize,
    },
    Next {
        word: WordId,
        pc: usize,
        recur: WordId,
        base: usize,
    },
    Guard {
        family: WordId,
        clause: usize,
        base: usize,
        args: Vec<Value>,
    },
    Decide {
        family: WordId,
        clause: usize,
        base: usize,
        args: Vec<Value>,
    },
}

fn copy_cost(values: &[Value]) -> usize {
    values.len()
}

/// Invocation-local real data stack and explicit return/control stack. Stores
/// are immutable snapshots; only a successful run returns the updated snapshot.
pub struct Machine<'p> {
    program: &'p Program,
    stack: Vec<Value>,
    tasks: Vec<Task>,
    store: Store,
    guard_depth: usize,
    remaining: usize,
    pub stack_limit: usize,
    pub continuation_limit: usize,
    pub text_byte_limit: usize,
    pub value_node_limit: usize,
    pub tuple_field_limit: usize,
    /// Disable only for baseline comparison/testing; never changes word identity.
    pub tail_call_optimization: bool,
    stats: Stats,
}

impl<'p> Machine<'p> {
    pub fn new(program: &'p Program) -> Self {
        Self {
            program,
            stack: Vec::new(),
            tasks: Vec::new(),
            store: Store::new(),
            guard_depth: 0,
            remaining: 0,
            stack_limit: 100_000,
            continuation_limit: 100_000,
            text_byte_limit: 32 * 1024 * 1024,
            value_node_limit: 1_000_000,
            tuple_field_limit: 1_000_000,
            tail_call_optimization: true,
            stats: Stats::default(),
        }
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.remaining = self.remaining.checked_sub(n).ok_or(Error::Budget)?;
        self.stats.steps += n;
        Ok(())
    }

    fn literal(&mut self, value: Literal) -> Result<Value, Error> {
        Ok(match value {
            Literal::Int(n) => Value::Int(n),
            Literal::Bool(b) => Value::Bool(b),
            Literal::Unit => Value::Unit,
            Literal::Quote(w) => Value::Quote(self.program.cid(w)?),
            Literal::Text(t) => {
                let text = self.program.text(t)?;
                if text.len() > self.text_byte_limit {
                    return Err(Error::StorageLimit);
                }
                Value::Text(self.program.texts[t].clone())
            }
        })
    }

    fn require(&self, n: usize, floor: usize) -> Result<(), Error> {
        let actual = self.stack.len().saturating_sub(floor);
        if actual < n {
            return Err(Error::Arity {
                expected: n,
                actual,
            });
        }
        Ok(())
    }

    fn pop(&mut self, floor: usize) -> Result<Value, Error> {
        self.require(1, floor)?;
        Ok(self.stack.pop().unwrap())
    }

    fn quotation(&self, value: Value) -> Result<WordId, Error> {
        let Value::Quote(cid) = value else {
            return Err(Error::Type("call requires a quotation"));
        };
        self.program
            .identities
            .get(&cid)
            .copied()
            .ok_or(Error::Type("unknown quotation CID"))
    }

    /// Existing source syntax still carries apply/recur counts. They are checked
    /// here, not used to allocate output cells. Typed count-free syntax follows
    /// separately; canonical identity and image semantics are not migrated here.
    pub fn run(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
        initial: &Store,
    ) -> Result<(Vec<Value>, Store), Error> {
        self.stack.clear();
        self.tasks.clear();
        self.guard_depth = 0;
        self.remaining = budget;
        self.stats = Stats::default();
        self.store = initial.clone();
        let result = self.run_inner(word, args, context);
        if result.is_err() {
            self.stack.clear();
            self.tasks.clear();
            self.store = initial.clone();
        }
        result
    }

    fn run_inner(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
    ) -> Result<(Vec<Value>, Store), Error> {
        let (inputs, _) = self.program.signature(word)?;
        if inputs != args.len() {
            return Err(Error::Arity {
                expected: inputs,
                actual: args.len(),
            });
        }
        if args.len() > self.stack_limit {
            return Err(Error::StorageLimit);
        }
        for &arg in args {
            let value = self.literal(arg)?;
            self.stack.push(value);
        }
        self.tasks.push(Task::Enter {
            word,
            recur: word,
            floor: 0,
        });
        while let Some(task) = self.tasks.pop() {
            self.charge(1)?;
            match task {
                Task::Enter { word, recur, floor } => {
                    let (inputs, outputs) = self.program.signature(word)?;
                    self.require(inputs, floor)?;
                    let base = self.stack.len() - inputs;
                    self.stats.calls += 1;
                    // With no remaining caller work, an identical pending
                    // return check validates both calls. Reuse it rather than
                    // accumulate return frames. Decide/Next tasks are barriers:
                    // guards still decide, and non-tail callers still resume.
                    if self.tail_call_optimization
                        && matches!(self.tasks.last(), Some(Task::Return { base: b, outputs: n })
                            if *b == base && *n == outputs)
                    {
                        self.stats.tail_calls += 1;
                    } else {
                        self.tasks.push(Task::Return { base, outputs });
                    }
                    match self
                        .program
                        .definition(word)?
                        .ok_or(Error::Type("stack engine requires canonical definitions"))?
                    {
                        Definition::Primitive(name) => self.primitive(name, base)?,
                        Definition::Kernel(_) => {
                            return Err(Error::Type(
                                "compiler execution is not supported by the stack baseline",
                            ));
                        }
                        Definition::Sequence(_) => self.tasks.push(Task::Next {
                            word,
                            pc: 0,
                            recur,
                            base,
                        }),
                        Definition::Family(_) => {
                            self.charge(copy_cost(&self.stack[base..]))?;
                            let args = self.stack[base..].to_vec();
                            self.tasks.push(Task::Guard {
                                family: word,
                                clause: 0,
                                base,
                                args,
                            });
                        }
                    }
                }
                Task::Return { base, outputs } => {
                    let actual = self.stack.len().saturating_sub(base);
                    if actual != outputs {
                        return Err(Error::OutputArity {
                            expected: outputs,
                            actual,
                        });
                    }
                }
                Task::Next {
                    word,
                    pc,
                    recur,
                    base,
                } => {
                    let Some(Definition::Sequence(items)) = self.program.definition(word)? else {
                        unreachable!()
                    };
                    if let Some(item) = items.get(pc) {
                        if !self.tail_call_optimization || pc + 1 < items.len() {
                            self.tasks.push(Task::Next {
                                word,
                                pc: pc + 1,
                                recur,
                                base,
                            });
                        }
                        match item {
                            Item::Literal(value) => {
                                let value = self.literal(*value)?;
                                self.stack.push(value);
                            }
                            Item::Word(word) => self.tasks.push(Task::Enter {
                                word: *word,
                                recur: *word,
                                floor: base,
                            }),
                            Item::Context(key) => {
                                let v = *context
                                    .get(key)
                                    .ok_or_else(|| Error::MissingContext(key.clone()))?;
                                let value = self.literal(v)?;
                                self.stack.push(value);
                            }
                            Item::StaticCall | Item::Apply { .. } => {
                                let value = self.pop(base)?;
                                let target = self.quotation(value)?;
                                if let Item::Apply { inputs, outputs } = item {
                                    self.check_contract(target, *inputs, *outputs)?;
                                }
                                self.tasks.push(Task::Enter {
                                    word: target,
                                    recur: target,
                                    floor: base,
                                });
                            }
                            Item::Recur { inputs, outputs } => {
                                self.check_contract(recur, *inputs, *outputs)?;
                                self.tasks.push(Task::Enter {
                                    word: recur,
                                    recur,
                                    floor: base,
                                });
                            }
                            Item::Tuple(n) => {
                                self.require(*n, base)?;
                                self.check_tuple_size(*n)?;
                                let fields = self.stack.split_off(self.stack.len() - n);
                                let tuple = self.make_tuple(fields)?;
                                self.stack.push(tuple);
                            }
                            Item::Untuple(n) => {
                                let tuple = self.pop(base)?;
                                let fields = Self::tuple_fields(&tuple)?;
                                if fields.len() != *n {
                                    return Err(Error::Type(
                                        "tuple length does not match untuple contract",
                                    ));
                                }
                                self.charge(fields.len())?;
                                self.stack.extend_from_slice(fields);
                            }
                        }
                    }
                }
                Task::Guard {
                    family,
                    clause,
                    base,
                    args,
                } => {
                    let Some(Definition::Family(clauses)) = self.program.definition(family)? else {
                        unreachable!()
                    };
                    let &(guard, _) = clauses.get(clause).ok_or(Error::NoClause)?;
                    self.charge(copy_cost(&args))?;
                    self.stack.truncate(base);
                    self.stack.extend(args.iter().cloned());
                    self.guard_depth += 1;
                    self.tasks.push(Task::Decide {
                        family,
                        clause,
                        base,
                        args,
                    });
                    self.tasks.push(Task::Enter {
                        word: guard,
                        recur: family,
                        floor: base,
                    });
                }
                Task::Decide {
                    family,
                    clause,
                    base,
                    args,
                } => {
                    self.guard_depth -= 1;
                    let Value::Bool(selected) = self.pop(base)? else {
                        return Err(Error::Type("guard must return Boolean"));
                    };
                    self.stack.truncate(base);
                    if selected {
                        self.stack.extend(args);
                        let Some(Definition::Family(clauses)) = self.program.definition(family)?
                        else {
                            unreachable!()
                        };
                        self.tasks.push(Task::Enter {
                            word: clauses[clause].1,
                            recur: family,
                            floor: base,
                        });
                    } else {
                        self.tasks.push(Task::Guard {
                            family,
                            clause: clause + 1,
                            base,
                            args,
                        });
                    }
                }
            }
            self.stats.peak_stack = self.stats.peak_stack.max(self.stack.len());
            self.stats.peak_continuations = self.stats.peak_continuations.max(self.tasks.len());
            if self.stack.len() > self.stack_limit || self.tasks.len() > self.continuation_limit {
                return Err(Error::StorageLimit);
            }
        }
        self.charge(copy_cost(&self.stack))?;
        Ok((self.stack.clone(), self.store.clone()))
    }

    fn check_contract(&self, word: WordId, inputs: usize, outputs: usize) -> Result<(), Error> {
        let actual = self.program.signature(word)?;
        if actual.0 != inputs {
            return Err(Error::Arity {
                expected: inputs,
                actual: actual.0,
            });
        }
        if actual.1 != outputs {
            return Err(Error::OutputArity {
                expected: outputs,
                actual: actual.1,
            });
        }
        Ok(())
    }

    fn primitive(&mut self, name: &str, base: usize) -> Result<(), Error> {
        match name {
            "dup" => {
                self.charge(copy_cost(&self.stack[self.stack.len() - 1..]))?;
                let v = self.stack.last().unwrap().clone();
                self.stack.push(v);
            }
            "drop" => {
                self.pop(base)?;
            }
            "swap" => {
                let n = self.stack.len();
                self.stack.swap(n - 1, n - 2);
            }
            "over" => {
                let n = self.stack.len();
                self.charge(copy_cost(&self.stack[n - 2..n - 1]))?;
                self.stack.push(self.stack[n - 2].clone());
            }
            "true" => self.stack.push(Value::Bool(true)),
            "false" => self.stack.push(Value::Bool(false)),
            "unit" => self.stack.push(Value::Unit),
            "+" | "-" | "*" | "<" | "=" => {
                let b = self.pop(base)?;
                let a = self.pop(base)?;
                let value = if name == "=" {
                    Value::Bool(value::equal(&a, &b, |n| self.charge(n))?)
                } else if name == "<" && matches!((&a, &b), (Value::Text(_), Value::Text(_))) {
                    let a = Self::text(&a)?;
                    let b = Self::text(&b)?;
                    self.charge(a.len().min(b.len()))?;
                    Value::Bool(a.as_bytes() < b.as_bytes())
                } else {
                    let (Value::Int(a), Value::Int(b)) = (a, b) else {
                        return Err(Error::Type("integer operands required"));
                    };
                    match name {
                        "<" => Value::Bool(a < b),
                        "+" => Value::Int(a.checked_add(b).ok_or(Error::Overflow)?),
                        "-" => Value::Int(a.checked_sub(b).ok_or(Error::Overflow)?),
                        "*" => Value::Int(a.checked_mul(b).ok_or(Error::Overflow)?),
                        _ => unreachable!(),
                    }
                };
                self.stack.push(value);
            }
            "select" => {
                let no = self.pop(base)?;
                let yes = self.pop(base)?;
                let Value::Bool(condition) = self.pop(base)? else {
                    return Err(Error::Type("select requires Boolean"));
                };
                self.stack.push(if condition { yes } else { no });
            }
            "pair" => {
                let b = self.pop(base)?;
                let a = self.pop(base)?;
                let tuple = self.make_tuple(vec![a, b])?;
                self.stack.push(tuple);
            }
            "first" | "second" | "tuple-length" | "nth" | "tuple-set" => {
                let replacement = if name == "tuple-set" {
                    Some(self.pop(base)?)
                } else {
                    None
                };
                let index = if name == "nth" || name == "tuple-set" {
                    Some(Self::index(self.pop(base)?)?)
                } else {
                    None
                };
                let tuple = self.pop(base)?;
                let fields = Self::tuple_fields(&tuple)?;
                let value = if name == "tuple-length" {
                    Value::Int(fields.len() as i64)
                } else {
                    let i = index.unwrap_or(usize::from(name == "second"));
                    let field = fields
                        .get(i)
                        .ok_or(Error::Type("tuple index out of range"))?;
                    if let Some(replacement) = replacement {
                        self.check_tuple_size(fields.len())?;
                        self.charge(fields.len())?;
                        let mut changed = fields.to_vec();
                        changed[i] = replacement;
                        self.make_tuple(changed)?
                    } else {
                        field.clone()
                    }
                };
                self.stack.push(value);
            }
            "text-bytes" | "text-chars" | "text-concat" | "text-slice" => {
                let end = if name == "text-slice" {
                    Some(Self::index(self.pop(base)?)?)
                } else {
                    None
                };
                let start = if name == "text-slice" {
                    Some(Self::index(self.pop(base)?)?)
                } else {
                    None
                };
                let second = if name == "text-concat" {
                    Some(self.pop(base)?)
                } else {
                    None
                };
                let value = self.pop(base)?;
                let text = Self::text(&value)?;
                let result = match name {
                    "text-bytes" => Value::Int(text.len() as i64),
                    "text-chars" => {
                        self.charge(text.len())?;
                        Value::Int(text.chars().count() as i64)
                    }
                    "text-concat" => {
                        let b = Self::text(second.as_ref().unwrap())?;
                        let len = text.len().checked_add(b.len()).ok_or(Error::StorageLimit)?;
                        self.reserve_text(len)?;
                        let mut joined = String::with_capacity(len);
                        joined.push_str(text);
                        joined.push_str(b);
                        Value::Text(joined.into())
                    }
                    "text-slice" => {
                        let slice = text
                            .get(start.unwrap()..end.unwrap())
                            .ok_or(Error::Type("invalid UTF-8 slice boundaries"))?;
                        self.reserve_text(slice.len())?;
                        Value::Text(slice.into())
                    }
                    _ => unreachable!(),
                };
                self.stack.push(result);
            }
            "store.get" | "store.put" => {
                let path_value = self.pop(base)?;
                let path = self.path(&path_value)?;
                let parts: Vec<_> = path.iter().map(|s| s.as_ref()).collect();
                if name == "store.get" {
                    let frozen = self
                        .store
                        .get(&parts)?
                        .cloned()
                        .ok_or_else(|| Error::Store("missing store entry".into()))?;
                    let value = self.import_value(&frozen)?;
                    self.stack.push(value);
                } else {
                    if self.guard_depth != 0 {
                        return Err(Error::Type("guards cannot write state"));
                    }
                    self.store.check_write(&parts)?;
                    let value = self.pop(base)?;
                    let frozen = self.freeze_value(&value)?;
                    self.store = self.store.with(&parts, frozen)?;
                }
            }
            _ => {
                return Err(Error::Type(
                    "primitive is not yet supported by the stack baseline",
                ));
            }
        }
        Ok(())
    }
}
