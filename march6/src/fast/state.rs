//! Implicit invocation state, sequenced separately from lazy stack results.
//! State edges are local execution dependencies, not language values or CIDs.
use super::*;
use std::collections::HashSet;
use store::{FrozenNode, FrozenValue, Store};

/// A conservative summary for ordinary calls and guarded families. Dynamic
/// `apply` currently has a non-writing contract; this is not a full effect type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub reads: bool,
    pub writes: bool,
    /// Dynamic non-writing application may read state. Keep its enclosing
    /// calls distinct across snapshots without eagerly executing pure targets.
    pub dynamic: bool,
}

pub(super) fn effects(program: &Program, ops: &[Op]) -> Effects {
    let mut out = Effects::default();
    for op in ops {
        let e = match op {
            Op::StoreRead(_) => Effects {
                reads: true,
                writes: false,
                dynamic: false,
            },
            Op::StoreWrite { .. } => Effects {
                reads: false,
                writes: true,
                dynamic: false,
            },
            Op::Apply { .. } => Effects {
                dynamic: true,
                ..Effects::default()
            },
            Op::Call { word, .. } => program.words[*word].effects,
            Op::Dispatch { clauses, .. } => {
                let mut e = Effects::default();
                for &(g, b) in clauses {
                    for w in [g, b] {
                        e.reads |= program.words[w].effects.reads;
                        e.writes |= program.words[w].effects.writes;
                        e.dynamic |= program.words[w].effects.dynamic;
                    }
                }
                e
            }
            _ => Effects::default(),
        };
        out.reads |= e.reads;
        out.writes |= e.writes;
        out.dynamic |= e.dynamic;
    }
    out
}

pub(super) fn sensitive(program: &Program, op: &Op) -> bool {
    matches!(
        op,
        Op::StoreRead(_) | Op::StoreWrite { .. } | Op::Recur { .. } | Op::Apply { .. }
    ) || effects(program, std::slice::from_ref(op)) != Effects::default()
}

impl Program {
    pub fn effects(&self, word: WordId) -> Result<Effects, Error> {
        Ok(self.word(word)?.effects)
    }
}

struct Version {
    previous: usize,
    operation: usize,
    running: bool,
    value: Option<Store>,
}

pub(super) struct Execution {
    versions: Vec<Version>,
    // One prefix-state edge per operation plus the final edge, for each frame.
    frames: Vec<Vec<usize>>,
    pub(super) failure: Option<Error>,
}

impl Execution {
    pub(super) fn storage(&self) -> (usize, usize, usize) {
        (
            self.versions.len(),
            self.frames.iter().map(Vec::len).sum(),
            self.versions.capacity() * std::mem::size_of::<Version>()
                + self.frames.capacity() * std::mem::size_of::<Vec<usize>>()
                + self
                    .frames
                    .iter()
                    .map(|v| v.capacity() * std::mem::size_of::<usize>())
                    .sum::<usize>(),
        )
    }
}

pub(super) enum Task {
    Need(usize),
    Execute(usize),
    Join(usize),
    Copy(usize, usize),
    Start(usize),
    Transfer(Box<Transfer>),
}

enum Visit {
    Need(usize),
    Fields(usize, Vec<usize>),
}
enum Purpose {
    Path,
    Value(Vec<String>),
}

/// A resumable deep observation. It schedules ordinary evaluator tasks rather
/// than re-entering public force/freeze and overwriting the work stack.
pub(super) struct Transfer {
    dst: usize,
    purpose: Purpose,
    todo: Vec<Visit>,
    visiting: HashSet<usize>,
    memo: HashMap<usize, usize>,
    nodes: Vec<FrozenNode>,
    cids: Vec<Cid>,
    bytes: usize,
    fields: usize,
}

impl Transfer {
    fn new(dst: usize, root: usize, purpose: Purpose) -> Self {
        Self {
            dst,
            purpose,
            todo: vec![Visit::Need(root)],
            visiting: HashSet::new(),
            memo: HashMap::new(),
            nodes: Vec::new(),
            cids: Vec::new(),
            bytes: 0,
            fields: 0,
        }
    }
}

impl Executor<'_> {
    pub(super) fn state_before(&self, dst: usize) -> usize {
        let c = self.cells[dst];
        self.state.as_ref().map_or(0, |s| s.frames[c.frame][c.slot])
    }

    fn state_after(&self, dst: usize) -> usize {
        let c = self.cells[dst];
        self.state.as_ref().unwrap().frames[c.frame][c.slot + 1]
    }

    pub(super) fn attach_state_frame(&mut self, id: usize, input: usize) -> Result<(), Error> {
        if self.state.is_none() {
            return Ok(());
        }
        let frame = &self.frames[id];
        let word = self.program.word(frame.word)?;
        let recur_writes = self.program.word(frame.recur)?.effects.writes;
        let base = frame.base;
        let ops = &word.ops;
        self.remaining = self.remaining.checked_sub(ops.len()).ok_or(Error::Budget)?;
        self.stats.steps += ops.len();
        let state = self.state.as_mut().unwrap();
        let mut prefix = Vec::with_capacity(ops.len() + 1);
        let mut current = input;
        for (slot, op) in ops.iter().enumerate() {
            prefix.push(current);
            let writes = effects(self.program, std::slice::from_ref(op)).writes
                || (matches!(op, Op::Recur { .. }) && recur_writes);
            if writes {
                if state.versions.len() >= self.cell_limit {
                    return Err(Error::StorageLimit);
                }
                let next = state.versions.len();
                state.versions.push(Version {
                    previous: current,
                    operation: base + slot,
                    running: false,
                    value: None,
                });
                current = next;
            }
        }
        prefix.push(current);
        debug_assert_eq!(state.frames.len(), id);
        state.frames.push(prefix);
        Ok(())
    }

    pub(super) fn enable_state(&mut self, initial: Store) -> Result<(), Error> {
        // Called only immediately after start, before any demand.
        if let Some(state) = &mut self.state {
            state.versions[0].value = Some(initial);
            return Ok(());
        }
        self.state = Some(Execution {
            versions: vec![Version {
                previous: 0,
                operation: 0,
                running: false,
                value: Some(initial),
            }],
            frames: Vec::new(),
            failure: None,
        });
        let result = self.attach_state_frame(0, 0);
        if let Err(e) = &result {
            self.state.as_mut().unwrap().failure = Some(e.clone());
        }
        result
    }

    pub fn start_with_store(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
        initial: &Store,
    ) -> Result<Vec<Handle>, Error> {
        let handles = self.start(word, args, context, budget)?;
        self.enable_state(initial.clone())?;
        Ok(handles)
    }

    /// Complete the invocation's state output, even with no ordinary results.
    /// The input snapshot is never modified. No snapshot is returned on error.
    pub fn finish_state(&mut self) -> Result<Store, Error> {
        if let Some(e) = &self.aborted {
            return Err(e.clone());
        }
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| Error::Store("no state invocation".into()))?;
        if let Some(e) = &state.failure {
            return Err(e.clone());
        }
        let final_state = *state.frames[0].last().unwrap();
        self.work.clear();
        self.work.push(super::Task::State(Task::Need(final_state)));
        self.drain_work()?;
        Ok(self.state.as_ref().unwrap().versions[final_state]
            .value
            .as_ref()
            .unwrap()
            .clone())
    }

    pub fn run_with_store(
        &mut self,
        word: WordId,
        args: &[Literal],
        context: &Context,
        budget: usize,
        initial: &Store,
    ) -> Result<(Vec<Value>, Store), Error> {
        let handles = self.start_with_store(word, args, context, budget, initial)?;
        let mut values = Vec::new();
        for h in handles {
            values.push(self.force(h)?);
        }
        let state = self.finish_state()?;
        Ok((values, state))
    }

    fn snapshot(&self, edge: usize) -> &Store {
        self.state.as_ref().unwrap().versions[edge]
            .value
            .as_ref()
            .expect("preceding state demanded")
    }

    pub(super) fn schedule_store(&mut self, dst: usize) -> Result<(), Error> {
        if self.kernel.is_some() {
            return Err(Error::Store(
                "runtime store operations are unavailable during compiler execution".into(),
            ));
        }
        if self.state.is_none() {
            return Err(Error::Store(
                "store operation requires a state invocation".into(),
            ));
        }
        self.work.push(super::Task::State(Task::Start(dst)));
        self.work
            .push(super::Task::State(Task::Need(self.state_before(dst))));
        Ok(())
    }

    pub(super) fn state_step(&mut self, task: Task) -> Result<(), Error> {
        match task {
            Task::Need(edge) => {
                let state = self.state.as_mut().unwrap();
                let v = &mut state.versions[edge];
                if v.value.is_some() {
                    return Ok(());
                }
                if v.running {
                    return Err(Error::Cycle);
                }
                v.running = true;
                self.work.push(super::Task::State(Task::Execute(edge)));
                self.work.push(super::Task::State(Task::Need(v.previous)));
            }
            Task::Execute(edge) => {
                let dst = self.state.as_ref().unwrap().versions[edge].operation;
                self.work.push(super::Task::State(Task::Join(edge)));
                self.work.push(super::Task::Need(dst));
            }
            Task::Join(edge) => {
                if self.state.as_ref().unwrap().versions[edge].value.is_some() {
                    return Ok(());
                }
                let dst = self.state.as_ref().unwrap().versions[edge].operation;
                let Datum::Frame(frame) = self.value(dst) else {
                    return Err(Error::Type("expected stateful call"));
                };
                let child = *self.state.as_ref().unwrap().frames[frame].last().unwrap();
                self.work.push(super::Task::State(Task::Copy(edge, child)));
                self.work.push(super::Task::State(Task::Need(child)));
            }
            Task::Copy(edge, child) => {
                let snapshot = self.snapshot(child).clone();
                self.state.as_mut().unwrap().versions[edge].value = Some(snapshot);
            }
            Task::Start(dst) => {
                let c = self.cells[dst];
                let f = &self.frames[c.frame];
                let path = match self.program.words[f.word].ops[c.slot] {
                    Op::StoreRead(path) | Op::StoreWrite { path, .. } => f.base + path,
                    _ => unreachable!(),
                };
                self.work
                    .push(super::Task::State(Task::Transfer(Box::new(Transfer::new(
                        dst,
                        path,
                        Purpose::Path,
                    )))));
            }
            Task::Transfer(transfer) => self.transfer_state(transfer)?,
        }
        Ok(())
    }

    fn transfer_state(&mut self, mut transfer: Box<Transfer>) -> Result<(), Error> {
        let Some(visit) = transfer.todo.pop() else {
            return self.complete_transfer(*transfer);
        };
        let (cell, node, cid) = match visit {
            Visit::Need(cell) => {
                if transfer.memo.contains_key(&cell) {
                    self.work.push(super::Task::State(Task::Transfer(transfer)));
                    return Ok(());
                }
                if self.cells[cell].state != 2 {
                    transfer.todo.push(Visit::Need(cell));
                    self.work.push(super::Task::State(Task::Transfer(transfer)));
                    self.work.push(super::Task::Need(cell));
                    return Ok(());
                }
                if !transfer.visiting.insert(cell) {
                    return Err(Error::Cycle);
                }
                if transfer.visiting.len() + transfer.nodes.len() > self.cell_limit {
                    return Err(Error::StorageLimit);
                }
                let value = self.external(self.value(cell))?;
                if let Value::Text(s) = &value {
                    transfer.bytes = transfer
                        .bytes
                        .checked_add(s.len())
                        .ok_or(Error::StorageLimit)?;
                    if transfer.bytes > self.text_byte_limit {
                        return Err(Error::StorageLimit);
                    }
                    self.data_charge(s.len())?;
                }
                let cid = value.scalar_cid();
                let node = match value {
                    Value::Int(n) => FrozenNode::Int(n),
                    Value::Bool(b) => FrozenNode::Bool(b),
                    Value::Unit => FrozenNode::Unit,
                    Value::Text(s) => FrozenNode::Text(s.into()),
                    Value::Quote(cid) => FrozenNode::Quote(cid),
                    Value::CompilerState(_) => {
                        return Err(Error::Type("compiler state cannot be stored"));
                    }
                    Value::Pair(a, b) => FrozenNode::Tuple(vec![a.cell, b.cell].into()),
                    Value::Tuple(fields) => {
                        FrozenNode::Tuple(fields.into_iter().map(|h| h.cell).collect())
                    }
                };
                if let FrozenNode::Tuple(fields) = node {
                    transfer.fields = transfer
                        .fields
                        .checked_add(fields.len())
                        .ok_or(Error::StorageLimit)?;
                    if transfer.fields > self.tuple_field_limit {
                        return Err(Error::StorageLimit);
                    }
                    self.data_charge(fields.len())?;
                    transfer.todo.push(Visit::Fields(cell, fields.to_vec()));
                    transfer
                        .todo
                        .extend(fields.into_vec().into_iter().rev().map(Visit::Need));
                    self.work.push(super::Task::State(Task::Transfer(transfer)));
                    return Ok(());
                }
                (cell, node, cid.unwrap())
            }
            Visit::Fields(cell, fields) => {
                let indices: Vec<_> = fields.iter().map(|c| transfer.memo[c]).collect();
                let mut bytes = Vec::new();
                if fields.len() == 2 {
                    bytes.push(4);
                } else {
                    bytes.push(5);
                    put(&mut bytes, fields.len());
                }
                for &i in &indices {
                    bytes.extend_from_slice(&transfer.cids[i].0);
                }
                (
                    cell,
                    FrozenNode::Tuple(indices.into()),
                    Cid::digest(b"march-fast-value-v1", &bytes),
                )
            }
        };
        transfer.visiting.remove(&cell);
        transfer.memo.insert(cell, transfer.nodes.len());
        transfer.nodes.push(node);
        transfer.cids.push(cid);
        self.work.push(super::Task::State(Task::Transfer(transfer)));
        Ok(())
    }

    fn complete_transfer(&mut self, transfer: Transfer) -> Result<(), Error> {
        let dst = transfer.dst;
        let before = self.state_before(dst);
        match transfer.purpose {
            Purpose::Path => {
                let nodes = transfer.nodes;
                let root = nodes.last().unwrap();
                let path = match root {
                    FrozenNode::Text(s) => vec![s.to_string()],
                    FrozenNode::Tuple(fields) => fields
                        .iter()
                        .map(|&i| match &nodes[i] {
                            FrozenNode::Text(s) => Ok(s.to_string()),
                            _ => Err(Error::Type("store path fields must be text")),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return Err(Error::Type("store path must be text or a tuple of text")),
                };
                let parts: Vec<_> = path.iter().map(String::as_str).collect();
                store::check_path(&parts, false)?;
                let c = self.cells[dst];
                let frame = &self.frames[c.frame];
                match self.program.words[frame.word].ops[c.slot] {
                    Op::StoreRead(_) => {
                        let value = self
                            .snapshot(before)
                            .get(&parts)?
                            .cloned()
                            .ok_or_else(|| Error::Store("missing store entry".into()))?;
                        let value = self.import_frozen(&value)?;
                        self.ready(dst, value);
                    }
                    Op::StoreWrite { value, .. } => {
                        self.snapshot(before).check_write(&parts)?;
                        let root = frame.base + value;
                        self.work.push(super::Task::State(Task::Transfer(Box::new(
                            Transfer::new(dst, root, Purpose::Value(path)),
                        ))));
                    }
                    _ => unreachable!(),
                }
            }
            Purpose::Value(path) => {
                let value =
                    FrozenValue::from_frozen_nodes(transfer.nodes, *transfer.cids.last().unwrap());
                let parts: Vec<_> = path.iter().map(String::as_str).collect();
                let next = self.snapshot(before).with(&parts, value)?;
                let edge = self.state_after(dst);
                self.state.as_mut().unwrap().versions[edge].value = Some(next);
                self.ready(dst, Datum::Unit);
            }
        }
        Ok(())
    }

    fn import_frozen(&mut self, value: &FrozenValue) -> Result<Datum, Error> {
        if self.cells.len().saturating_add(value.nodes().len()) > self.cell_limit {
            return Err(Error::StorageLimit);
        }
        let base = self.cells.len();
        for node in value.nodes() {
            self.data_charge(1)?;
            let value =
                match node {
                    FrozenNode::Int(n) => Datum::Int(*n),
                    FrozenNode::Bool(b) => Datum::Bool(*b),
                    FrozenNode::Unit => Datum::Unit,
                    FrozenNode::Text(s) => {
                        self.data_charge(s.len())?;
                        self.store_text(s.to_string())?
                    }
                    FrozenNode::Quote(cid) => {
                        Datum::Quote(*self.program.identities.get(cid).ok_or_else(|| {
                            Error::Store("stored code is absent from program".into())
                        })?)
                    }
                    FrozenNode::Tuple(fields) => {
                        self.data_charge(fields.len())?;
                        self.make_tuple(fields.iter().map(|&i| base + i).collect())?
                    }
                };
            self.next_identity = self
                .next_identity
                .checked_add(1)
                .ok_or(Error::StorageLimit)?;
            self.cells.push(Cell {
                state: 2,
                value,
                frame: usize::MAX,
                slot: 0,
                identity: self.next_identity,
            });
        }
        self.stats.peak_cells = self.stats.peak_cells.max(self.cells.len());
        Ok(self.value(base + value.root()))
    }
}
