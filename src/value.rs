// Value types that can be on the stack

use crate::xt::XT;
use im;
use std::collections::HashMap;
use std::fmt;

// Concrete types in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I64,
    String,
    Type,  // The type of types themselves
    Quotation,
    Array,
    Map,
    MutableArray,
    MutableMap,
    // Future: I32, I16, F64, Boolean, etc.
}

impl Type {
    pub fn name(&self) -> &'static str {
        match self {
            Type::I64 => "core.i64",
            Type::String => "core.string",
            Type::Type => "core.type",
            Type::Quotation => "core.quotation",
            Type::Array => "core.array",
            Type::Map => "core.map",
            Type::MutableArray => "core.mutable-array",
            Type::MutableMap => "core.mutable-map",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "core.i64" => Ok(Type::I64),
            "core.string" => Ok(Type::String),
            "core.type" => Ok(Type::Type),
            "core.quotation" => Ok(Type::Quotation),
            "core.array" => Ok(Type::Array),
            "core.map" => Ok(Type::Map),
            "core.mutable-array" => Ok(Type::MutableArray),
            "core.mutable-map" => Ok(Type::MutableMap),
            _ => Err(format!("Unknown type: {}", s)),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(i64),
    Quotation(Vec<XT>),  // Code block that can be executed
    String(String),
    Type(Type),  // Type as a first-class value

    // Immutable collections (from im crate)
    Array(im::Vector<Value>),
    Map(im::HashMap<String, Value>),  // Using String keys for simplicity

    // Mutable collections (for fast operations)
    MutableArray(Vec<Value>),
    MutableMap(HashMap<String, Value>),
}

impl Value {
    pub fn get_type(&self) -> Type {
        match self {
            Value::Number(_) => Type::I64,
            Value::String(_) => Type::String,
            Value::Type(_) => Type::Type,
            Value::Quotation(_) => Type::Quotation,
            Value::Array(_) => Type::Array,
            Value::Map(_) => Type::Map,
            Value::MutableArray(_) => Type::MutableArray,
            Value::MutableMap(_) => Type::MutableMap,
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.get_type().name()
    }
}
