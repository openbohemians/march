-- Test that constraints don't mutate the actual stack

-- Test 1: Constraint that drops values
= i64 i64 -> i64 ;
? drop drop -1 ;
: test1 + ;

-- Stack should have both values after constraint runs
10 20 test1 drop

-- Test 2: Constraint that uses swap
= i64 i64 -> i64 ;
? swap drop drop -1 ;
: test2 * ;

5 6 test2 drop

-- Test 3: Constraint that duplicates and drops
= i64 -> i64 ;
? dup dup drop drop 0 lt ;
: test3 0 swap - ;

= i64 -> i64 ;
? dup 0 gte ;
: test3 ;

-5 test3 drop
5 test3 drop

-- Test 4: Constraint that manipulates stack heavily
= i64 i64 i64 -> i64 ;
? rot rot rot drop drop drop -1 ;
: test4 + + ;

1 2 3 test4 drop

-- Test 5: Constraint fails but stack should be unchanged
= i64 i64 -> i64 ;
? over over lt ;
: safe-div / ;

= i64 i64 -> i64 ;
? over over gte ;
: safe-div drop drop 0 ;

10 5 safe-div drop
5 10 safe-div drop

-- Test 6: Complex constraint with over
= i64 i64 i64 -> i64 i64 ;
? over over over drop drop drop -1 ;
: pair-sum + ;

1 2 3 pair-sum drop drop

-- Test 7: Constraint that reads deep in stack
= i64 i64 i64 i64 -> i64 ;
? rot rot rot rot drop drop drop drop -1 ;
: sum4 + + + ;

10 20 30 40 sum4 drop

-- Test 8: Constraint uses state variable
$ limit = 100 ;

= i64 -> i64 ;
? dup limit gt ;
: cap drop limit ;

= i64 -> i64 ;
: cap ;

150 cap drop
50 cap drop
