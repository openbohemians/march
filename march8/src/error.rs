//! Errors, with where in the source they arose.

use std::fmt;

/// A place in the source: line and column, from 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// What went wrong. The first five are found while compiling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The text is not March: a bracket closing nothing, a bad escape.
    Syntax,
    /// The text ends inside a bracket or a string: more may follow, as in
    /// the REPL.
    Unfinished,
    /// No word: a name with no definition, or a family with no clause for
    /// the values given (doc/design/TYPES.md 2.15, "no match is no word").
    NoWord,
    /// Types that disagree, clauses that tie, values missing, a value that
    /// has no form at run time.
    Mismatch,
    /// A literal that cannot become the type wanted.
    Literal,
    /// Arithmetic that overflows or divides by zero, found at compile time.
    Arithmetic,
    /// A limit of the machine, or something not built yet.
    Limit,
    /// An effect where none is allowed: a guard that writes.
    Effect,
    /// Inside the compiler only: a recursive call whose results' types are
    /// not known yet (docs/MACHINE.md, "Recursion").
    Ghost,
    /// Inside the compiler only: a family applied again to the same types
    /// inside its own application, unwinding to that application, the
    /// `n`th, to make it an instance.
    Again(usize),
    /// An error while running.
    Run(crate::machine::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: Kind,
    pub pos: Option<Pos>,
    pub msg: String,
    /// The words being applied when it arose, innermost first.
    pub trace: Vec<String>,
}

impl Error {
    pub fn new(kind: Kind, pos: Option<Pos>, msg: impl Into<String>) -> Self {
        Self {
            kind,
            pos,
            msg: msg.into(),
            trace: Vec::new(),
        }
    }
    /// Notes that the error arose inside `word`, applied at `pos`.
    pub fn within(mut self, word: &str, pos: Pos) -> Self {
        self.trace.push(format!("in `{word}`, applied at {pos}"));
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(p) = self.pos {
            write!(f, "{p}: ")?;
        }
        write!(f, "{}", self.msg)?;
        for t in &self.trace {
            write!(f, "\n  {t}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

impl From<crate::machine::Error> for Error {
    fn from(e: crate::machine::Error) -> Self {
        let msg = match &e {
            crate::machine::Error::Arithmetic => "arithmetic overflow, or division by zero".into(),
            crate::machine::Error::Fuel => "step budget exhausted".into(),
            crate::machine::Error::User(1) => "no word: no clause's guard holds".into(),
            crate::machine::Error::User(2) => {
                "`within`: the array has fewer elements than its quotation takes".into()
            }
            other => format!("{other}"),
        };
        Error::new(Kind::Run(e), None, msg)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
