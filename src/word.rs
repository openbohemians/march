// Word definitions and metadata

use crate::xt::XT;

#[derive(Debug, Clone)]
pub struct Word {
    pub xt: XT,
    pub immediate: bool,  // Does this word execute even during compilation?
}

impl Word {
    pub fn new(xt: XT) -> Self {
        Word {
            xt,
            immediate: false,
        }
    }

    pub fn immediate(xt: XT) -> Self {
        Word {
            xt,
            immediate: true,
        }
    }
}
