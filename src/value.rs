// Value types that can be on the stack

use crate::xt::XT;
use im;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Number(i64),
    Quotation(Vec<XT>),  // Code block that can be executed
    String(String),

    // Immutable collections (from im crate)
    Array(im::Vector<Value>),
    Map(im::HashMap<String, Value>),  // Using String keys for simplicity

    // Mutable collections (for fast operations)
    MutableArray(Vec<Value>),
    MutableMap(HashMap<String, Value>),
}
