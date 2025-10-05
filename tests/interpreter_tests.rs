use march::*;
use num_bigint::BigInt;
use num_rational::BigRational;

#[test]
fn test_basic_arithmetic() {
    let mut interp = Interpreter::new().unwrap();

    // Test: 10 + 5 = 15
    interp.push(Value::I64(10), ConcreteType::I64);
    interp.push(Value::I64(5), ConcreteType::I64);
    interp.add().unwrap();

    let (result, result_type) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(15));
    assert!(matches!(result_type, ConcreteType::I64));
}

#[test]
fn test_stack_operations() {
    let mut interp = Interpreter::new().unwrap();

    interp.push(Value::I64(42), ConcreteType::I64);
    assert_eq!(interp.stack_size(), 1);

    // Test dup
    interp.dup().unwrap();
    assert_eq!(interp.stack_size(), 2);

    // Test swap (both values should be 42, so swap doesn't change result)
    interp.swap().unwrap();
    let (top, _) = interp.pop().unwrap();
    assert_eq!(top, Value::I64(42));

    // Test drop
    interp.drop().unwrap();
    assert_eq!(interp.stack_size(), 0);
}

#[test]
fn test_rational_arithmetic() {
    let mut interp = Interpreter::new().unwrap();

    // Test: 1/2 + 1/3 = 5/6
    let half = BigRational::new(BigInt::from(1), BigInt::from(2));
    let third = BigRational::new(BigInt::from(1), BigInt::from(3));
    let expected = BigRational::new(BigInt::from(5), BigInt::from(6));

    interp.push(Value::BigRational(half), ConcreteType::BigRational);
    interp.push(Value::BigRational(third), ConcreteType::BigRational);
    interp.add().unwrap();

    let (result, _) = interp.pop().unwrap();
    if let Value::BigRational(actual) = result {
        assert_eq!(actual, expected);
    } else {
        panic!("Expected BigRational result");
    }
}

#[test]
fn test_division_by_zero() {
    let mut interp = Interpreter::new().unwrap();

    interp.push(Value::I64(10), ConcreteType::I64);
    interp.push(Value::I64(0), ConcreteType::I64);

    // Division by zero should return an error
    assert!(interp.div().is_err());
}

#[test]
fn test_type_safety() {
    let mut interp = Interpreter::new().unwrap();

    // Try to add I64 and BigRational (should fail)
    interp.push(Value::I64(42), ConcreteType::I64);
    interp.push(Value::BigRational(BigRational::from(BigInt::from(1))), ConcreteType::BigRational);

    assert!(interp.add().is_err());
}

#[test]
fn test_word_execution() {
    let mut interp = Interpreter::new().unwrap();

    // Test number parsing
    interp.execute_word("42").unwrap();
    interp.execute_word("13").unwrap();

    // Test operation
    interp.execute_word("+").unwrap();

    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(55));
}

#[test]
fn test_word_definitions() {
    let mut interp = Interpreter::new().unwrap();

    // Define a word
    interp.execute_word(": square dup * ;").unwrap();

    // Test using the word
    interp.execute_word("5").unwrap();
    interp.execute_word("square").unwrap();

    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(25));
}

#[test]
fn test_nested_word_calls() {
    let mut interp = Interpreter::new().unwrap();

    // Define helper words
    interp.execute_word(": double dup + ;").unwrap();
    interp.execute_word(": quadruple double double ;").unwrap();

    // Test nested calls
    interp.execute_word("3").unwrap();
    interp.execute_word("quadruple").unwrap();

    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(12));
}

#[test]
fn test_word_redefinition() {
    let mut interp = Interpreter::new().unwrap();

    // Define a word
    interp.execute_word(": test 10 ;").unwrap();
    interp.execute_word("test").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(10));

    // Redefine it
    interp.execute_word(": test 20 ;").unwrap();
    interp.execute_word("test").unwrap();
    let (result, _) = interp.pop().unwrap();
    assert_eq!(result, Value::I64(20));
}