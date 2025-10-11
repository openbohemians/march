// Value types that can be on the stack

use crate::xt::XT;

#[derive(Debug, Clone)]
pub enum Value {
    Number(i64),
    Quotation(Vec<XT>),  // Code block that can be executed
    // Will add more types later:
    // String(String),
    // Bool(bool),
    // etc.
}
