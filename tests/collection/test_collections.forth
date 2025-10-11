-- Test adaptive collections (Arrays and Tuples)

-- Homogeneous arrays (just verify they parse and execute)
{ 1 2 3 } drop
{ "a" "b" "c" } drop
{ } drop

-- Heterogeneous tuples (just verify they parse and execute)
{ 1 "x" 3 } drop
{ "hello" 42 "world" } drop
{ 1 } drop

-- Array operations
TEST. { 10 20 30 } array.len 3 eq ;
TEST. { 10 20 30 } 1 array.get 20 eq ;
TEST. { 10 20 30 } 0 array.get 10 eq ;
TEST. { 10 20 30 } 2 array.get 30 eq ;

-- Empty array
TEST. { } array.len 0 eq ;

-- Nested collections (just verify they work)
{ 1 2 { 3 4 } } drop
{ { 1 2 } { 3 4 } } drop
