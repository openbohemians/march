// Execution Token (XT) - the compiled form of words

use crate::value::Value;
use crate::cid::CID;

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

    // Type operations
    Type,   // ( value -- type-string ) Get the type of a value as a string

    // User-defined word (list of XTs to execute)
    Compiled(Vec<XT>),

    // Push a literal value
    Literal(Value),

    // Native words that need access to interpreter internals
    // These are called with a reference to the Forth instance
    Native(NativeFn),

    // CID reference - needs to be resolved to actual XT on first execution
    Cid(CID),
}

impl XT {
    /// Convert a primitive XT to its CID
    /// Returns None for Compiled, Literal, and Native (these need special handling)
    pub fn to_primitive_cid(&self) -> Option<CID> {
        let id = match self {
            XT::Add => 1,
            XT::Sub => 2,
            XT::Mul => 3,
            XT::Div => 4,
            XT::Dup => 5,
            XT::Drop => 6,
            XT::Swap => 7,
            XT::Over => 8,
            XT::Rot => 9,
            XT::Dot => 10,
            XT::Lt => 11,
            XT::Gt => 12,
            XT::Lte => 13,
            XT::Gte => 14,
            XT::Eq => 15,
            XT::Neq => 16,
            XT::ToR => 17,
            XT::FromR => 18,
            XT::RFetch => 19,
            XT::Call => 20,
            XT::If => 21,
            XT::Iff => 22,
            XT::Type => 23,
            // These need special handling
            XT::Compiled(_) => return None,
            XT::Literal(_) => return None,
            XT::Native(_) => return None,
            XT::Cid(_) => return None,
        };
        Some(CID::primitive(id))
    }

    /// Convert a primitive CID back to an XT
    /// Returns None if the CID is not a known primitive
    pub fn from_primitive_cid(cid: &CID) -> Option<XT> {
        let id = cid.primitive_id()?;
        let xt = match id {
            1 => XT::Add,
            2 => XT::Sub,
            3 => XT::Mul,
            4 => XT::Div,
            5 => XT::Dup,
            6 => XT::Drop,
            7 => XT::Swap,
            8 => XT::Over,
            9 => XT::Rot,
            10 => XT::Dot,
            11 => XT::Lt,
            12 => XT::Gt,
            13 => XT::Lte,
            14 => XT::Gte,
            15 => XT::Eq,
            16 => XT::Neq,
            17 => XT::ToR,
            18 => XT::FromR,
            19 => XT::RFetch,
            20 => XT::Call,
            21 => XT::If,
            22 => XT::Iff,
            23 => XT::Type,
            _ => return None,
        };
        Some(xt)
    }
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
            XT::Type => write!(f, "Type"),
            XT::Compiled(words) => write!(f, "Compiled({:?})", words),
            XT::Literal(val) => write!(f, "Literal({:?})", val),
            XT::Native(_) => write!(f, "Native(<fn>)"),
            XT::Cid(cid) => write!(f, "Cid({})", cid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_to_cid() {
        let add_cid = XT::Add.to_primitive_cid().unwrap();
        assert!(add_cid.is_primitive());
        assert_eq!(add_cid.primitive_id(), Some(1));
    }

    #[test]
    fn test_cid_to_primitive() {
        let add_cid = CID::primitive(1);
        let xt = XT::from_primitive_cid(&add_cid).unwrap();
        matches!(xt, XT::Add);
    }

    #[test]
    fn test_roundtrip() {
        let original = XT::Mul;
        let cid = original.to_primitive_cid().unwrap();
        let restored = XT::from_primitive_cid(&cid).unwrap();
        // Can't directly compare XTs, but we can check the CID matches
        assert_eq!(restored.to_primitive_cid(), Some(cid));
    }
}
