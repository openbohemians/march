//! Bounded immutable host input, using the same cells as ordinary evaluation.
use super::*;

/// A topologically ordered finite value DAG. Tuple/pair edges point backwards.
/// Indices are local to this import, not executor handles or code identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputNode {
    Scalar(Literal),
    Pair(usize, usize),
    Tuple(Vec<usize>),
    Text(String),
    CompilerState(stream::StateHandle),
}

impl Executor<'_> {
    /// Start with immutable structured inputs. No compiler-specific evaluator:
    /// imported pairs are ordinary ready cells with ordinary shared handles.
    /// Like `start`, beginning an invocation invalidates previous handles.
    pub fn start_graph(
        &mut self,
        word: WordId,
        nodes: &[InputNode],
        roots: &[usize],
        context: &Context,
        budget: usize,
    ) -> Result<Vec<Handle>, Error> {
        self.reset(budget);
        let expected = self.program.word(word)?.inputs;
        if expected != roots.len() {
            return Err(Error::Arity {
                expected,
                actual: roots.len(),
            });
        }
        if nodes.len() > self.cell_limit {
            return Err(Error::StorageLimit);
        }
        if roots.iter().any(|&r| r >= nodes.len()) {
            return Err(Error::InvalidCode("input root outside value graph".into()));
        }
        for (i, node) in nodes.iter().enumerate() {
            match node {
                InputNode::Tuple(fields) if fields.iter().any(|&f| f >= i) => {
                    return Err(Error::InvalidCode(
                        "input tuple edges must precede their user".into(),
                    ));
                }
                InputNode::Scalar(Literal::Text(t)) => {
                    self.program.text(*t)?;
                }
                InputNode::Pair(a, b) if *a >= i || *b >= i => {
                    return Err(Error::InvalidCode(
                        "input pair edges must precede their user".into(),
                    ));
                }
                InputNode::Scalar(Literal::Quote(w)) => {
                    self.program.word(*w)?;
                }
                _ => (),
            }
        }
        let text_bytes = nodes.iter().try_fold(0usize, |n, node| match node {
            InputNode::Text(t) => n.checked_add(t.len()).ok_or(Error::StorageLimit),
            _ => Ok(n),
        })?;
        let tuple_fields = nodes.iter().try_fold(0usize, |n, node| match node {
            InputNode::Tuple(fields) if fields.len() != 2 => {
                n.checked_add(fields.len()).ok_or(Error::StorageLimit)
            }
            _ => Ok(n),
        })?;
        if text_bytes > self.text_byte_limit || tuple_fields > self.tuple_field_limit {
            return Err(Error::StorageLimit);
        }
        self.context.clone_from(context);
        self.next_identity = nodes.len() as u64;
        for (i, node) in nodes.iter().enumerate() {
            let value = match node {
                InputNode::Scalar(v) => (*v).into(),
                InputNode::Pair(a, b) => Datum::Pair(*a, *b),
                InputNode::Tuple(fields) => self.make_tuple(fields.clone())?,
                InputNode::Text(text) => self.store_text(text.clone())?,
                InputNode::CompilerState(s) => Datum::CompilerState(*s),
            };
            self.cells.push(Cell {
                state: 2,
                value,
                frame: usize::MAX,
                slot: 0,
                identity: i as u64 + 1,
            });
        }
        let frame = self.frame(word, roots.to_vec(), word)?;
        let base = self.frames[frame].base;
        if self.program.word(word)?.effects.reads || self.program.word(word)?.effects.writes {
            self.enable_state(store::Store::new())?;
        }
        Ok(self.program.words[word]
            .outputs
            .iter()
            .map(|&slot| Handle {
                epoch: self.epoch,
                cell: base + slot,
            })
            .collect())
    }
}
