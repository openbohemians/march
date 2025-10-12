// Word definitions and metadata

use crate::xt::XT;
use crate::value::Type;

// Type signature: input types -> output types
#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub inputs: Vec<Type>,
    pub outputs: Vec<Type>,
}

impl Signature {
    pub fn new(inputs: Vec<Type>, outputs: Vec<Type>) -> Self {
        Signature { inputs, outputs }
    }
}

#[derive(Debug, Clone)]
pub struct Word {
    pub xt: XT,
    pub immediate: bool,  // Does this word execute even during compilation?
    pub signature: Option<Signature>,  // Optional type signature
}

impl Word {
    pub fn new(xt: XT) -> Self {
        Word {
            xt,
            immediate: false,
            signature: None,
        }
    }

    pub fn immediate(xt: XT) -> Self {
        Word {
            xt,
            immediate: true,
            signature: None,
        }
    }

    pub fn with_signature(xt: XT, signature: Signature) -> Self {
        Word {
            xt,
            immediate: false,
            signature: Some(signature),
        }
    }
}
