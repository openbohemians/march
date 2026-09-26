//! Explicit, atomic collection between observations. Roots are a host contract:
//! success replaces all handles with a new epoch; failure changes nothing.
use super::*;

fn charge(remaining: &mut usize, amount: usize) -> Result<(), Error> {
    *remaining = remaining.checked_sub(amount).ok_or(Error::Budget)?;
    Ok(())
}

fn enqueue(cell: usize, marked: &mut [bool], todo: &mut Vec<usize>) {
    assert!(
        cell < marked.len(),
        "live dependency points outside the heap"
    );
    if !marked[cell] {
        marked[cell] = true;
        todo.push(cell);
    }
}

impl Executor<'_> {
    /// Retain exactly the graph reachable from `roots`, without evaluating it.
    ///
    /// May be called only between observations. The caller must supply every
    /// handle it intends to keep, including fields held in previously returned
    /// Tuple/Pair values. Successful collection invalidates ALL old handles and returns
    /// replacements for the supplied roots, preserving their order/duplicates.
    /// Re-observe a retained pair to acquire new handles to its fields.
    ///
    /// Invalid roots, an aborted invocation, or insufficient collection budget
    /// leave the evaluator and handles unchanged. This budget is separate from
    /// evaluation fuel. Collection never retries failed work or resets fuel.
    /// This baseline does not collect within force/content_id or eliminate live
    /// recursive continuations; their existing bounds remain in force.
    pub fn collect(&mut self, roots: &[Handle], budget: usize) -> Result<Vec<Handle>, Error> {
        for h in roots {
            if h.epoch != self.epoch || h.cell >= self.cells.len() {
                return Err(Error::StaleHandle);
            }
        }
        if let Some(error) = &self.aborted {
            return Err(error.clone());
        }
        if !self.work.is_empty() || !self.active_demands.is_empty() {
            return Err(Error::CollectionBusy);
        }
        let mut remaining = budget;
        // Charge scanning/copying capacity up front, before allocating scratch.
        // Each queued cell and each dependency edge is charged additionally.
        for count in [
            self.cells.len(),
            self.frames.len(),
            self.argument_slots,
            roots.len(),
            self.tuples.len(),
            self.tuple_fields,
            self.texts.len(),
            self.text_bytes,
        ] {
            charge(&mut remaining, count)?;
        }
        let mut marked = vec![false; self.cells.len()];
        let mut frames_marked = vec![false; self.frames.len()];
        let mut tuples_marked = vec![false; self.tuples.len()];
        let mut texts_marked = vec![false; self.texts.len()];
        let mut todo = Vec::new();
        for h in roots {
            enqueue(h.cell, &mut marked, &mut todo);
        }
        while let Some(id) = todo.pop() {
            charge(&mut remaining, 1)?;
            let cell = self.cells[id];
            match cell.state {
                0 => {
                    let f = &self.frames[cell.frame];
                    frames_marked[cell.frame] = true;
                    let op = &self.program.words[f.word].ops[cell.slot];
                    match op {
                        Op::Arg(i) => {
                            charge(&mut remaining, 1)?;
                            enqueue(f.arguments[*i], &mut marked, &mut todo);
                        }
                        _ => {
                            let deps = dependencies(op);
                            charge(&mut remaining, deps.len())?;
                            for slot in deps {
                                enqueue(f.base + slot, &mut marked, &mut todo);
                            }
                        }
                    }
                }
                2 => match cell.value {
                    Datum::Tuple(t) => {
                        if !tuples_marked[t] {
                            tuples_marked[t] = true;
                            charge(&mut remaining, self.tuples[t].len())?;
                            for &field in &self.tuples[t] {
                                enqueue(field, &mut marked, &mut todo);
                            }
                        }
                    }
                    Datum::Text(data::TextRef::Runtime(t)) => {
                        texts_marked[t] = true;
                    }
                    Datum::Pair(a, b) => {
                        charge(&mut remaining, 2)?;
                        enqueue(a, &mut marked, &mut todo);
                        enqueue(b, &mut marked, &mut todo);
                    }
                    Datum::Frame(id) => {
                        let f = &self.frames[id];
                        frames_marked[id] = true;
                        let outputs = &self.program.words[f.word].outputs;
                        charge(&mut remaining, outputs.len())?;
                        for &slot in outputs {
                            enqueue(f.base + slot, &mut marked, &mut todo);
                        }
                    }
                    _ => (),
                },
                3 => (), // Failed cells keep their Error, not construction history.
                _ => return Err(Error::CollectionBusy),
            }
        }

        let mut cell_map = vec![usize::MAX; self.cells.len()];
        let mut frame_map = vec![usize::MAX; self.frames.len()];
        let mut bases = vec![0; self.frames.len()];
        let mut cell_count = 0usize;
        let mut frame_count = 0usize;
        for (id, f) in self.frames.iter().enumerate() {
            if !frames_marked[id] {
                continue;
            }
            frame_map[id] = frame_count;
            frame_count += 1;
            bases[id] = cell_count;
            let size = self.program.words[f.word].ops.len();
            // Contiguous slots are a conservative storage reservation, NOT
            // additional strong references from already completed expressions.
            charge(&mut remaining, size)?;
            for slot in 0..size {
                let old = f.base + slot;
                if marked[old] {
                    cell_map[old] = cell_count + slot;
                }
            }
            cell_count += size;
        }
        for (id, &live) in marked.iter().enumerate() {
            if live && cell_map[id] == usize::MAX {
                cell_map[id] = cell_count;
                cell_count += 1;
            }
        }
        // Keeping a frame's block plus standalone ready values never expands the
        // old heap; retain this bound explicitly if the layout later changes.
        if cell_count > self.cell_limit || frame_count > self.cell_limit {
            return Err(Error::StorageLimit);
        }
        let empty = Cell {
            state: 4,
            value: Datum::Unit,
            frame: usize::MAX,
            slot: 0,
            identity: 0,
        };
        let mut cells = vec![empty; cell_count];
        let mut frames = Vec::with_capacity(frame_count);
        let mut argument_slots = 0usize;
        for (id, f) in self.frames.iter().enumerate() {
            if !frames_marked[id] {
                continue;
            }
            argument_slots += f.arguments.len();
            frames.push(Frame {
                word: f.word,
                recur: f.recur,
                base: bases[id],
                key_hash: f.key_hash,
                // Unreachable argument slots are deliberately not strong edges.
                // Their frozen identities still participate in cycle keys.
                arguments: f
                    .arguments
                    .iter()
                    .map(|&a| {
                        if a == usize::MAX {
                            usize::MAX
                        } else {
                            cell_map[a]
                        }
                    })
                    .collect(),
                identities: f.identities.clone(),
            });
        }
        if argument_slots > self.argument_limit {
            return Err(Error::StorageLimit);
        }
        let mut failures = HashMap::new();
        let mut tuple_map = vec![usize::MAX; self.tuples.len()];
        let mut tuples = Vec::new();
        let mut tuple_fields = 0;
        for (i, fields) in self.tuples.iter().enumerate() {
            if tuples_marked[i] {
                tuple_map[i] = tuples.len();
                tuple_fields += fields.len();
                tuples.push(fields.iter().map(|&c| cell_map[c]).collect());
            }
        }
        let mut text_map = vec![usize::MAX; self.texts.len()];
        let mut texts = Vec::new();
        let mut text_bytes = 0;
        for (i, text) in self.texts.iter().enumerate() {
            if texts_marked[i] {
                text_map[i] = texts.len();
                text_bytes += text.len();
                texts.push(text.clone());
            }
        }
        for (old, c) in self.cells.iter().enumerate() {
            if !marked[old] {
                continue;
            }
            let mut c = *c;
            c.frame = if c.state == 0 {
                frame_map[c.frame]
            } else {
                usize::MAX
            };
            if c.state == 2 {
                c.value = match c.value {
                    Datum::Tuple(t) => Datum::Tuple(tuple_map[t]),
                    Datum::Text(data::TextRef::Runtime(t)) => {
                        Datum::Text(data::TextRef::Runtime(text_map[t]))
                    }
                    Datum::Pair(a, b) => Datum::Pair(cell_map[a], cell_map[b]),
                    Datum::Frame(f) => Datum::Frame(frame_map[f]),
                    other => other,
                };
            }
            if c.state == 3 {
                failures.insert(cell_map[old], self.failures[&old].clone());
            }
            cells[cell_map[old]] = c;
        }
        // Commit only after validation, tracing, allocation and remapping succeed.
        let epoch = EPOCH.fetch_add(1, Ordering::Relaxed);
        let replacement = roots
            .iter()
            .map(|h| Handle {
                epoch,
                cell: cell_map[h.cell],
            })
            .collect();
        self.stats.collections += 1;
        self.stats.collection_steps += budget - remaining;
        self.stats.collected_cells += self.cells.len() - cells.len();
        self.stats.collected_frames += self.frames.len() - frames.len();
        self.cells = cells;
        self.tuples = tuples;
        self.tuple_fields = tuple_fields;
        self.texts = texts;
        self.text_bytes = text_bytes;
        self.frames = frames;
        self.argument_slots = argument_slots;
        self.failures = failures;
        self.work = Vec::new();
        self.active_demands = HashMap::new();
        self.epoch = epoch;
        Ok(replacement)
    }
}
