//! Bounded immutable host input, using the same cells as ordinary evaluation.
use super::*;

/// A topologically ordered finite value DAG. Pair edges must point backwards.
/// Indices are local to this import, not executor handles or code identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputNode {
    Scalar(Literal),
    Pair(usize, usize),
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
        self.context.clone_from(context);
        self.next_identity = nodes.len() as u64;
        self.cells
            .extend(nodes.iter().enumerate().map(|(i, node)| Cell {
                state: 2,
                value: match *node {
                    InputNode::Scalar(v) => v.into(),
                    InputNode::Pair(a, b) => Datum::Pair(a, b),
                    InputNode::CompilerState(s) => Datum::CompilerState(s),
                },
                frame: usize::MAX,
                slot: 0,
                identity: i as u64 + 1,
            }));
        let frame = self.frame(word, roots.to_vec(), word)?;
        let base = self.frames[frame].base;
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
