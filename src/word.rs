// Word definitions and metadata

use crate::xt::XT;
use crate::value::Type;
use crate::cid::CID;

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
    pub xt: XT,                         // Execution token (runtime)
    pub cid: Option<CID>,               // Content ID (for storage/sharing)
    pub immediate: bool,                // Does this word execute even during compilation?
    pub signature: Option<Signature>,  // Optional type signature
}

impl Word {
    pub fn new(xt: XT) -> Self {
        // Try to get CID for primitives
        let cid = xt.to_primitive_cid();
        Word {
            xt,
            cid,
            immediate: false,
            signature: None,
        }
    }

    pub fn immediate(xt: XT) -> Self {
        let cid = xt.to_primitive_cid();
        Word {
            xt,
            cid,
            immediate: true,
            signature: None,
        }
    }

    pub fn with_signature(xt: XT, signature: Signature) -> Self {
        let cid = xt.to_primitive_cid();
        Word {
            xt,
            cid,
            immediate: false,
            signature: Some(signature),
        }
    }

    pub fn with_cid(xt: XT, cid: CID) -> Self {
        Word {
            xt,
            cid: Some(cid),
            immediate: false,
            signature: None,
        }
    }
}
