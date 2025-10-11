-- Test that array mutations in constraints don't affect main state

$ arr = { 10 20 30 } ;

-- Constraint that mutates the array
= i64 -> i64 ;
? 999 arr 0 ! -> arr -1 ;
: test-mutate dup * ;

-- Run the test - constraint should mutate arr but changes discarded
5 test-mutate drop

-- Verify arr is still { 10 20 30 }
arr 0 @ drop
arr 0 @ 10 eq drop

-- Another test: multiple mutations
= i64 -> i64 ;
? arr 1 @ 100 + arr 1 ! 2 @ 200 + arr 2 ! drop drop -1 ;
: test-multi + ;

3 4 test-multi drop

-- arr should still be unchanged
arr 1 @ 20 eq drop
arr 2 @ 30 eq drop

-- Test with state variable containing modified array
= -> ;
? { 1 2 3 } -> arr -1 ;
: replace-array ;

replace-array

-- arr should still be { 10 20 30 }, not { 1 2 3 }
arr 0 @ 10 eq drop
