use march::*;
use num_bigint::BigInt;
use num_rational::BigRational;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--test" {
        // Run tests
        test_march();
    } else {
        // Start REPL
        repl();
    }
}

fn test_march() {
    println!("March Interpreter v0.1.0 - Running Tests");

    let mut interp = Interpreter::new().unwrap();

    // Test basic operations
    println!("=== Testing I64 arithmetic ===");
    interp.push(Value::I64(100), ConcreteType::I64);
    interp.push(Value::I64(25), ConcreteType::I64);
    interp.print_stack();

    interp.sub().unwrap(); // 100 - 25 = 75
    println!("After subtraction (100 - 25):");
    interp.print_stack();

    interp.push(Value::I64(3), ConcreteType::I64);
    interp.mul().unwrap(); // 75 * 3 = 225
    println!("After multiplication (* 3):");
    interp.print_stack();

    interp.push(Value::I64(5), ConcreteType::I64);
    interp.div().unwrap(); // 225 / 5 = 45
    println!("After division (/ 5):");
    interp.print_stack();

    // Test BigRational precision
    println!("\n=== Testing BigRational precision ===");
    let rat1 = BigRational::new(BigInt::from(22), BigInt::from(7)); // 22/7 (≈ π)
    let rat2 = BigRational::new(BigInt::from(1), BigInt::from(3));  // 1/3

    interp.push(Value::BigRational(rat1), ConcreteType::BigRational);
    interp.push(Value::BigRational(rat2), ConcreteType::BigRational);
    println!("22/7 and 1/3 on stack:");
    interp.print_stack();

    interp.mul().unwrap(); // (22/7) * (1/3) = 22/21
    println!("After multiplication (22/7 * 1/3):");
    interp.print_stack();

    // Test stack operations
    println!("\n=== Testing stack manipulation ===");
    interp.push(Value::I64(999), ConcreteType::I64);
    println!("Added 999:");
    interp.print_stack();

    interp.dup().unwrap();
    println!("After dup:");
    interp.print_stack();

    interp.swap().unwrap();
    println!("After swap:");
    interp.print_stack();

    interp.drop().unwrap();
    println!("After drop:");
    interp.print_stack();
}
