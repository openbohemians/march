# Testing in March2

## Overview

March2 includes a built-in testing framework with:
- **`TEST.` word** for defining tests in Forth code
- **`march2 test` command** for running test suites
- **Global test counters** for tracking pass/fail counts
- **Clean output** with immediate feedback

## Writing Tests

### Basic Test Syntax

```forth
TEST. test-name code... ;
```

The `TEST.` word:
1. Reads a test name (token or string)
2. Executes code until `;`
3. Pops the top of stack (0 = fail, non-zero = pass)
4. Prints result immediately
5. Updates `test.pass-count` and `test.fail-count` global variables

### Simple Example

```forth
-- Test arithmetic
TEST. addition 2 3 + 5 eq? ;
TEST. multiplication 4 5 * 20 eq? ;
```

Output:
```
✓ PASS: addition
✓ PASS: multiplication
```

### Test Naming

Tests accept either bare tokens or string literals:

```forth
-- Bare token (backward compatible)
TEST. my-test 5 3 + 8 eq? ;

-- String literal (allows spaces and special chars)
TEST. "addition with negatives" -5 3 + -2 eq? ;
```

**Note**: String literal support is implemented but not yet widely used in existing tests.

## Test Files

### File Organization

Test files go in the `tests/` directory with `.fth` extension:

```
tests/
├── basic.fth           -- Core functionality
├── types.fth           -- Type system tests
├── signatures.fth      -- Type signatures
├── namespaces.fth      -- Namespace tests
├── variables.fth       -- Variable tests
└── database.fth        -- Database tests
```

### Example Test File

```forth
-- tests/myfeature.fth
-- Tests for my feature

NAMESPACE. mylib ;

SIGNATURE. i64 -> i64 ;
: double dup + ;
: triple 3 * ;

-- Test the words
TEST. double-five 5 double 10 eq? ;
TEST. triple-four 4 triple 12 eq? ;
TEST. double-zero 0 double 0 eq? ;

-- Print summary at end (optional)
test.print-results.
```

## Running Tests

### Test Runner Command

```bash
# Run all tests
march2 test

# From project root
./target/debug/march2 test
```

### Test Output

```
March2 FORTH Test Runner

=== Running basic.fth ===
✓ PASS: addition
✓ PASS: subtraction
✓ PASS: multiplication
...

=== Running types.fth ===
✓ PASS: type-check-i64
✓ PASS: type-check-string
...

==================================================
Test Summary:
  Files: 6
  Total tests: 49
  ✓ Passed: 49
==================================================
```

### Exit Codes

- **0** - All tests passed
- **1** - One or more tests failed or file errors

Useful for CI/CD integration:

```bash
march2 test && echo "Tests passed!" || echo "Tests failed!"
```

## Test Counters

### Global Variables

The testing system maintains global counters:

- `test.pass-count` - Number of passing tests
- `test.fail-count` - Number of failing tests

These are stored in `global_state` and accessible as words:

```forth
test.pass-count .    -- prints pass count
test.fail-count .    -- prints fail count
```

### Printing Results

Use `test.print-results.` to display a summary:

```forth
test.print-results.
```

Output:
```
Test Results:
  Total: 10
  ✓ Passed: 9
  ✗ Failed: 1
```

## Test Isolation

Each test file runs in a **fresh interpreter**:
- Clean namespace stack
- Empty global state
- No carryover from previous files

This ensures tests don't interfere with each other.

Within a file, tests share state:
- Variables persist between tests
- Namespace definitions available to all tests
- Global counters accumulate

## Writing Good Tests

### 1. Test One Thing

```forth
-- Good: focused test
TEST. add-positive 5 3 + 8 eq? ;
TEST. add-negative -5 -3 + -8 eq? ;

-- Less good: testing multiple things
TEST. arithmetic 5 3 + 8 eq? 4 2 * 8 eq? and ;
```

### 2. Use Descriptive Names

```forth
-- Good names
TEST. empty-string-length "" string-length 0 eq? ;
TEST. quotation-returns-value ( 42 ) call 42 eq? ;

-- Poor names
TEST. test1 5 3 + 8 eq? ;
TEST. x "" string-length 0 eq? ;
```

### 3. Test Edge Cases

```forth
TEST. divide-by-positive 10 2 / 5 eq? ;
TEST. divide-by-negative 10 -2 / -5 eq? ;
TEST. divide-by-one 7 1 / 7 eq? ;
```

### 4. Group Related Tests

```forth
-- Stack operations
TEST. dup-duplicates 5 dup 5 eq? swap 5 eq? and ;
TEST. swap-swaps 1 2 swap 1 eq? ;
TEST. drop-drops 1 2 drop 1 eq? ;

-- Type operations
TEST. type-i64 42 type "core.i64" eq? ;
TEST. type-string "hello" type "core.string" eq? ;
```

### 5. Clean Up After Tests

If tests modify global state:

```forth
VARIABLE. test-var = 0 ;

TEST. set-var 10 -> test-var test-var 10 eq? ;
TEST. increment-var test-var 1 + -> test-var test-var 11 eq? ;

-- Reset for other test files if needed
0 -> test-var
```

## Testing Patterns

### Testing Return Values

```forth
SIGNATURE. i64 i64 -> i64 ;
: add + ;

TEST. add-returns-sum 5 3 add 8 eq? ;
```

### Testing Side Effects

```forth
VARIABLE. counter = 0 ;

: increment counter 1 + -> counter ;

TEST. increment-works
  0 -> counter
  increment
  counter 1 eq? ;
```

### Testing Error Cases

Currently, error handling in tests is limited. Errors will stop test execution:

```forth
-- This test will fail and stop the file
TEST. divide-by-zero 10 0 / 5 eq? ;  -- ERROR: division by zero
```

Future: Expect-error syntax planned.

### Testing Quotations

```forth
TEST. quotation-call ( 42 ) call 42 eq? ;
TEST. quotation-with-args ( + ) 3 5 rot call 8 eq? ;
```

### Testing Type Signatures

```forth
SIGNATURE. i64 i64 -> i64 ;
: add + ;

TEST. sig-add sig add "(core.i64 core.i64 -> core.i64)" eq? ;
```

## Test Helpers

### Common Predicates

```forth
-- Equality
eq?    -- ( a b -- bool ) true if equal
neq?   -- ( a b -- bool ) true if not equal

-- Comparison
lt?    -- ( a b -- bool ) true if a < b
gt?    -- ( a b -- bool ) true if a > b
lte?   -- ( a b -- bool ) true if a <= b
gte?   -- ( a b -- bool ) true if a >= b

-- Type checking
?      -- ( value type -- bool ) true if value is of type
```

### Creating Custom Assertions

```forth
-- Assert non-zero
SIGNATURE. i64 -> i64 ;
: assert-true dup 0 eq? if drop 0 else 1 then ;

TEST. custom-assert 5 3 gt? assert-true ;
```

## Example Test Suite

```forth
-- tests/math.fth
-- Mathematical operations test suite

NAMESPACE. math ;

-- Define words to test
SIGNATURE. i64 -> i64 ;
: square dup * ;
: cube dup dup * * ;
: abs dup 0 lt? if 0 swap - then ;

-- Basic tests
TEST. square-positive 5 square 25 eq? ;
TEST. square-zero 0 square 0 eq? ;
TEST. square-negative -3 square 9 eq? ;

TEST. cube-positive 3 cube 27 eq? ;
TEST. cube-negative -2 cube -8 eq? ;

TEST. abs-positive 5 abs 5 eq? ;
TEST. abs-negative -5 abs 5 eq? ;
TEST. abs-zero 0 abs 0 eq? ;

-- Edge cases
TEST. large-square 100 square 10000 eq? ;

-- Print summary
test.print-results.
```

## Debugging Failed Tests

### 1. Check Test Output

Failed tests print immediately:

```
✗ FAIL: my-test
```

### 2. Run Test Interactively

```bash
march2
> TEST. my-test 5 3 + 9 eq? ;
✗ FAIL: my-test
```

### 3. Inspect Stack

```forth
-- Add . to see intermediate values
TEST. debug 5 3 + . 8 eq? ;  -- prints 8, then checks
```

### 4. Break Down Complex Tests

```forth
-- Instead of:
TEST. complex 5 3 + dup * 64 eq? ;

-- Break into steps:
TEST. step1 5 3 + 8 eq? ;
TEST. step2 8 dup * 64 eq? ;
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Build
        run: cargo build --release
      - name: Run tests
        run: ./target/release/march2 test
```

### Exit Code Usage

```bash
#!/bin/bash
# run-tests.sh

march2 test
EXIT_CODE=$?

if [ $EXIT_CODE -eq 0 ]; then
    echo "✓ All tests passed"
    exit 0
else
    echo "✗ Tests failed"
    exit 1
fi
```

## Test Coverage

Currently, March2 has 49 tests covering:
- Arithmetic operations (4 tests)
- Stack operations (8 tests)
- Comparison operators (6 tests)
- Control flow (4 tests)
- Variables (5 tests)
- Type system (8 tests)
- Type checking (4 tests)
- Type signatures (5 tests)
- Namespaces (2 tests)
- Database (2 tests)

### Areas to Test

When adding features, consider tests for:
- **Happy path** - Normal usage
- **Edge cases** - Boundary conditions
- **Error cases** - Invalid inputs
- **Type checking** - Signature validation
- **Namespace isolation** - No cross-contamination

## Future Enhancements

### Planned Features

1. **Expect-error syntax**
   ```forth
   TEST. divide-by-zero EXPECT-ERROR 10 0 / ;
   ```

2. **Setup/teardown hooks**
   ```forth
   BEFORE-EACH. 0 -> counter ;
   AFTER-EACH. counter . ;
   ```

3. **Test fixtures**
   ```forth
   FIXTURE. sample-data [1 2 3 4 5] ;
   TEST. uses-fixture sample-data array-sum 15 eq? ;
   ```

4. **Skip/focus tests**
   ```forth
   SKIP. slow-test ... ;
   FOCUS. debug-this ... ;
   ```

5. **Benchmark tests**
   ```forth
   BENCHMARK. fibonacci-speed 10 fib ;
   ```

## Best Practices Summary

✓ **Write focused tests** - One assertion per test
✓ **Use descriptive names** - Clear purpose from name
✓ **Test edge cases** - Zero, negative, empty, large values
✓ **Organize by feature** - Group related tests in files
✓ **Run tests frequently** - Before commits, in CI/CD
✓ **Keep tests fast** - Quick feedback loop
✓ **Document test purpose** - Comments for complex tests
✓ **Use test isolation** - Fresh interpreter per file

## Related Documentation

- `docs/manual/namespaces.md` - Namespace system
- `docs/manual/types.md` - Type system and signatures
- `docs/manual/variables.md` - Variable management
- `src/forth.rs:native_test` - TEST. implementation
- `src/repl.rs:run_tests` - Test runner implementation

## Getting Help

If you encounter issues with testing:

1. Check test output for error messages
2. Run test interactively in REPL
3. Review existing test files for examples
4. See related documentation above
