-- Test state variable declarations and operations

-- Declare state variables with initial values
$ count = 0 ;
TEST. count 0 eq ;

-- Update state variable
42 -> count
TEST. count 42 eq ;

-- Increment pattern
count 1 + -> count
TEST. count 43 eq ;

-- String state variable (can't test equality with strings, just verify it works)
$ name = "World" ;
"Hello, " name ++ drop

-- Multiple state variables
$ x = 10 ;
$ y = 20 ;
TEST. x y + 30 eq ;

-- State variables auto-fetch in expressions
TEST. x 10 eq ;
TEST. y 20 eq ;
