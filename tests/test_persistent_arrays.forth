-- Test persistent arrays in state

-- Store array in state (converts to persistent)
$ arr = { 10 20 30 } ;

-- Read and use persistent array directly
arr 1 @ drop
arr 1 @ 20 eq drop

-- Modify persistent array (creates new version with structural sharing)
100 arr 1 ! -> arr

-- Verify modification
arr 1 @ 100 eq drop
arr 0 @ 10 eq drop
arr 2 @ 30 eq drop

-- Convert persistent to mutable for heavy manipulation
arr to-mutable
200 swap 0 !
300 swap 2 !
-> arr

-- Verify mutations
arr 0 @ 200 eq drop
arr 1 @ 100 eq drop
arr 2 @ 300 eq drop

-- Test that persistent arrays work in constraints
$ limit = { 5 10 15 } ;

= i64 -> i64 ;
? dup limit 1 @ gt ;
: classify drop 1 ;

= i64 -> i64 ;
: classify drop 0 ;

12 classify drop
3 classify drop

-- Test array length with persistent
arr array.len drop
arr array.len 3 eq drop

-- Create nested persistent array
$ matrix = { { 1 2 } { 3 4 } } ;
matrix 0 @ 1 @ drop
matrix 0 @ 1 @ 2 eq drop
