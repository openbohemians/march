use crate::{Value, ConcreteType, AbstractType, RuntimeError};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StateVariable {
    pub name: String,
    pub abstract_type: AbstractType,
    pub concrete_type: ConcreteType,
    pub constraints: Vec<Constraint>,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub enum Constraint {
    GreaterThan(Value),
    GreaterThanOrEqual(Value),
    LessThan(Value),
    LessThanOrEqual(Value),
    OneOf(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct StateMapping {
    pub from_name: String,  // External name (from import)
    pub to_name: String,    // Local name (in our state)
}

pub struct ProgramState {
    variables: Vec<StateVariable>,
    name_to_index: HashMap<String, usize>,
    mappings: Vec<StateMapping>,
}

impl ProgramState {
    pub fn new() -> Self {
        ProgramState {
            variables: Vec::new(),
            name_to_index: HashMap::new(),
            mappings: Vec::new(),
        }
    }

    pub fn declare_variable(
        &mut self,
        name: &str,
        abstract_type: AbstractType,
        constraints: Vec<Constraint>,
        initial_value: Value,
    ) -> Result<(), RuntimeError> {
        // Check if variable already exists
        if self.name_to_index.contains_key(name) {
            return Err(RuntimeError::ParseError); // TODO: Add VariableExists error
        }

        // Infer concrete type from initial value
        let concrete_type = Self::infer_concrete_type(&abstract_type, &initial_value)?;

        // Validate constraints
        Self::validate_constraints(&initial_value, &constraints)?;

        let index = self.variables.len();
        let var = StateVariable {
            name: name.to_string(),
            abstract_type,
            concrete_type,
            constraints,
            value: initial_value,
        };

        self.variables.push(var);
        self.name_to_index.insert(name.to_string(), index);

        Ok(())
    }

    pub fn get_variable(&self, name: &str) -> Result<&Value, RuntimeError> {
        let resolved_name = self.resolve_name(name);
        let index = self.name_to_index.get(resolved_name)
            .ok_or(RuntimeError::ParseError)?; // TODO: Add VariableNotFound error
        Ok(&self.variables[*index].value)
    }

    pub fn set_variable(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let resolved_name = self.resolve_name(name);
        let index = *self.name_to_index.get(resolved_name)
            .ok_or(RuntimeError::ParseError)?; // TODO: Add VariableNotFound error

        let var = &mut self.variables[index];

        // Type check
        let expected_type = &var.concrete_type;
        let actual_type = Self::infer_concrete_type(&var.abstract_type, &value)?;
        if !Self::types_compatible(expected_type, &actual_type) {
            return Err(RuntimeError::TypeMismatch);
        }

        // Constraint check
        Self::validate_constraints(&value, &var.constraints)?;

        var.value = value;
        Ok(())
    }

    pub fn add_mapping(&mut self, from_name: String, to_name: String) -> Result<(), RuntimeError> {
        // Check that to_name exists in our state
        if !self.name_to_index.contains_key(&to_name) {
            return Err(RuntimeError::ParseError); // TODO: Add VariableNotFound error
        }

        self.mappings.push(StateMapping { from_name, to_name });
        Ok(())
    }

    pub fn list_variables(&self) -> Vec<&StateVariable> {
        self.variables.iter().collect()
    }

    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    // Helper methods
    fn resolve_name<'a>(&'a self, name: &'a str) -> &'a str {
        // Check if there's a mapping for this name
        for mapping in &self.mappings {
            if mapping.from_name == name {
                return &mapping.to_name;
            }
        }
        name
    }

    fn infer_concrete_type(abstract_type: &AbstractType, value: &Value) -> Result<ConcreteType, RuntimeError> {
        match (abstract_type, value) {
            (AbstractType::Int, Value::I64(_)) => Ok(ConcreteType::I64),
            (AbstractType::Int, Value::BigInt(_)) => Ok(ConcreteType::BigInt),
            (AbstractType::Rational, Value::BigRational(_)) => Ok(ConcreteType::BigRational),
            (AbstractType::String, Value::String(_)) => Ok(ConcreteType::String),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    fn types_compatible(expected: &ConcreteType, actual: &ConcreteType) -> bool {
        expected == actual
    }

    fn validate_constraints(value: &Value, constraints: &[Constraint]) -> Result<(), RuntimeError> {
        for constraint in constraints {
            match constraint {
                Constraint::GreaterThanOrEqual(threshold) => {
                    if !Self::value_gte(value, threshold) {
                        return Err(RuntimeError::TypeMismatch); // TODO: Add ConstraintViolation error
                    }
                }
                // TODO: Implement other constraints
                _ => {} // For now, skip other constraints
            }
        }
        Ok(())
    }

    fn value_gte(value: &Value, threshold: &Value) -> bool {
        match (value, threshold) {
            (Value::I64(a), Value::I64(b)) => a >= b,
            // TODO: Implement for other value types
            _ => true, // For now, assume constraint is satisfied
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, ConcreteType, AbstractType};

    #[test]
    fn test_state_declaration() {
        let mut state = ProgramState::new();

        // Declare a variable
        state.declare_variable(
            "velocity",
            AbstractType::Int,
            vec![Constraint::GreaterThanOrEqual(Value::I64(0))],
            Value::I64(10),
        ).unwrap();

        assert_eq!(state.variable_count(), 1);

        // Get the variable
        let value = state.get_variable("velocity").unwrap();
        assert_eq!(*value, Value::I64(10));
    }

    #[test]
    fn test_state_update() {
        let mut state = ProgramState::new();

        state.declare_variable(
            "mode",
            AbstractType::String,
            vec![],
            Value::String("running".to_string()),
        ).unwrap();

        // Update the variable
        state.set_variable("mode", Value::String("paused".to_string())).unwrap();

        let value = state.get_variable("mode").unwrap();
        assert_eq!(*value, Value::String("paused".to_string()));
    }

    #[test]
    fn test_state_mapping() {
        let mut state = ProgramState::new();

        // Declare local variable
        state.declare_variable(
            "mySpeed",
            AbstractType::Int,
            vec![],
            Value::I64(50),
        ).unwrap();

        // Add mapping for import
        state.add_mapping("velocity".to_string(), "mySpeed".to_string()).unwrap();

        // Access via mapped name
        let value = state.get_variable("velocity").unwrap();
        assert_eq!(*value, Value::I64(50));
    }

    #[test]
    fn test_constraint_validation() {
        let mut state = ProgramState::new();

        state.declare_variable(
            "speed",
            AbstractType::Int,
            vec![Constraint::GreaterThanOrEqual(Value::I64(0))],
            Value::I64(10),
        ).unwrap();

        // Valid update
        state.set_variable("speed", Value::I64(20)).unwrap();

        // Invalid update (violates constraint)
        assert!(state.set_variable("speed", Value::I64(-5)).is_err());
    }
}