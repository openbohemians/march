// Execution Token (XT) - the compiled form of words

use crate::value::Value;

// Function pointer type for native words
pub type NativeFn = fn(&mut crate::forth::Forth, &mut crate::input::InputBuffer) -> Result<(), String>;

#[derive(Clone)]
pub enum XT {
    // Primitives (implemented in Rust)
    Add,
    Sub,
    Mul,
    Div,
    Dup,
    Drop,
    Swap,
    Over,
    Rot,
    Dot,  // Print TOS

    // Comparison operators (return 1 for true, 0 for false)
    Lt,   // <  (a b -- flag) a < b
    Gt,   // >  (a b -- flag) a > b
    Lte,  // <= (a b -- flag) a <= b
    Gte,  // >= (a b -- flag) a >= b
    Eq,   // =  (a b -- flag) a = b
    Neq,  // != (a b -- flag) a != b

    // Return stack operations
    ToR,    // >r - Move TOS to return stack
    FromR,  // r> - Move top of return stack to data stack
    RFetch, // r@ - Copy top of return stack to data stack

    // Quotation execution
    Call,   // Execute a quotation from the stack

    // Conditional execution
    If,     // ( cond true-quot false-quot -- ) Execute branch based on condition
    Iff,    // ( cond quot -- ) Execute quotation only if condition is true

    // User-defined word (list of XTs to execute)
    Compiled(Vec<XT>),

    // Push a literal value
    Literal(Value),

    // Native words that need access to interpreter internals
    // These are called with a reference to the Forth instance
    Native(NativeFn),
}

impl std::fmt::Debug for XT {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            XT::Add => write!(f, "Add"),
            XT::Sub => write!(f, "Sub"),
            XT::Mul => write!(f, "Mul"),
            XT::Div => write!(f, "Div"),
            XT::Dup => write!(f, "Dup"),
            XT::Drop => write!(f, "Drop"),
            XT::Swap => write!(f, "Swap"),
            XT::Over => write!(f, "Over"),
            XT::Rot => write!(f, "Rot"),
            XT::Dot => write!(f, "Dot"),
            XT::ToR => write!(f, "ToR"),
            XT::FromR => write!(f, "FromR"),
            XT::RFetch => write!(f, "RFetch"),
            XT::Lt => write!(f, "Lt"),
            XT::Gt => write!(f, "Gt"),
            XT::Lte => write!(f, "Lte"),
            XT::Gte => write!(f, "Gte"),
            XT::Eq => write!(f, "Eq"),
            XT::Neq => write!(f, "Neq"),
            XT::Call => write!(f, "Call"),
            XT::If => write!(f, "If"),
            XT::Iff => write!(f, "Iff"),
            XT::Compiled(words) => write!(f, "Compiled({:?})", words),
            XT::Literal(val) => write!(f, "Literal({:?})", val),
            XT::Native(_) => write!(f, "Native(<fn>)"),
        }
    }
}
