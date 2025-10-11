-- Test collection element access with @ and !

-- Array access with @
{ 10 20 30 } 0 @ drop
{ 10 20 30 } 1 @ drop
{ 10 20 30 } 2 @ drop

-- Tuple access with @
{ 1 "x" 3 } 0 @ drop
{ 1 "x" 3 } 1 @ drop
{ 1 "x" 3 } 2 @ drop

-- Array update with !
99 { 10 20 30 } 0 ! drop
99 { 10 20 30 } 1 ! drop
99 { 10 20 30 } 2 ! drop

-- Chaining operations
{ 10 20 30 } 1 @ 5 + drop
100 { 10 20 30 } 1 ! 2 @ drop

-- Using in word definitions
= a i64 -> a ;
: get-first 0 @ ;

{ 10 20 30 } get-first drop
{ "a" "b" "c" } get-first drop
