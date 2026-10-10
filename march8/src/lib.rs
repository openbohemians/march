//! March8: March's compiler in Rust (docs/MACHINE.md). The symbolic stack
//! machine, `stage`, runs the explicit form on judgments and leaves code
//! for the machine, `machine`, which runs it. System March, in march7, is
//! frozen; March will compile itself again once it is mature.
pub mod code;
pub mod error;
pub mod image;
pub mod machine;
pub mod prims;
pub mod read;
pub mod show;
pub mod stage;
pub mod symbols;
pub mod types;

pub use code::{Blob, Cid, Op, Primitive};
pub use error::{Error, Kind, Level, Pos, Warning};
pub use image::Image;
pub use machine::{Machine, Stats};

use stage::Stage;
use symbols::Symbols;
use types::Type;

/// The core vocabulary, in the explicit form.
pub const CORE: &str = include_str!("../core/core.march");

/// A machine, a stage and the types of the values on the machine's stack.
/// Each piece of source given to it is compiled as a word that takes the
/// stack as its inputs, then run.
pub struct Session {
    pub machine: Machine,
    pub stage: Stage,
    symbols: Symbols,
    /// The types of the values on the machine's stack, the deepest first.
    pub types: Vec<Type>,
    /// Steps a piece of source may run for.
    pub fuel: u64,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session with the core vocabulary.
    pub fn new() -> Self {
        let mut s = Self::bare();
        s.stage.core = true;
        s.eval(CORE).expect("the core vocabulary compiles");
        s.stage.core = false;
        s
    }

    /// A session with the primitives only.
    pub fn bare() -> Self {
        Self {
            machine: Machine::empty(),
            stage: Stage::new(),
            symbols: Symbols::standard(),
            types: Vec::new(),
            fuel: 10_000_000,
        }
    }

    /// Compiles source as a word taking the stack, without running it: its
    /// code, and the types of the stack after it. The code and data it made,
    /// the instances it compiled among them, are published even if it fails,
    /// since the stage keeps the instances.
    pub fn compile(&mut self, src: &str) -> error::Result<(Vec<Op>, Vec<Type>)> {
        let r = self.compile_word(src);
        for blob in std::mem::take(&mut self.stage.blobs) {
            if let Err(e) = self.machine.publish(blob) {
                self.stage.forget_instances();
                return Err(e.into());
            }
        }
        r
    }

    fn compile_word(&mut self, src: &str) -> error::Result<(Vec<Op>, Vec<Type>)> {
        let toks = read::read(src, &self.symbols)?;
        self.stage.begin(&self.types);
        self.stage.run(&toks)?;
        let types = self.stage.finish()?;
        let code = std::mem::take(&mut self.stage.code);
        Ok((self.stage.seal(code).0, types))
    }

    /// Compiles source and runs it.
    pub fn eval(&mut self, src: &str) -> error::Result<()> {
        let (ops, types) = self.compile(src)?;
        let before = self.machine.stack.clone();
        let xt = self.machine.define(&ops)?;
        if let Err(e) = self.machine.run(xt, self.fuel) {
            self.machine.stack = before;
            return Err(e.into());
        }
        assert_eq!(
            self.machine.stack.len(),
            types.len(),
            "the stage's judgments match the machine's stack"
        );
        self.types = types;
        Ok(())
    }

    /// The values on the stack, written by their types: `<2> 42 "abab"`.
    pub fn show(&self) -> String {
        let mut out = format!("<{}>", self.types.len());
        for (&v, &t) in self.machine.stack.iter().zip(&self.types) {
            out.push(' ');
            show::value(&self.machine, &self.stage.types, v, t, &mut out);
        }
        out
    }

    /// The values on the stack, each followed by its type in a bracket, so
    /// the line reads back as March: `<2> 42 < i64 > "abab" < string >`.
    pub fn show_typed(&self) -> String {
        let mut out = format!("<{}>", self.types.len());
        for (&v, &t) in self.machine.stack.iter().zip(&self.types) {
            out.push(' ');
            show::value(&self.machine, &self.stage.types, v, t, &mut out);
            out.push_str(&format!(" < {} >", self.stage.types.source(t)));
        }
        out
    }

    /// The warnings found since this was last asked.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.stage.warnings)
    }

    /// What the program has written since this was last asked.
    pub fn take_output(&mut self) -> Vec<u8> {
        self.machine.take_output()
    }
}
