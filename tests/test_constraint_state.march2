-- Test that state mutations in constraints are discarded

-- Test 1: Constraint reads state variable
$ counter = 0 ;

= i64 -> i64 ;
? dup counter gt ;
: process drop 1 ;

= i64 -> i64 ;
: process drop 0 ;

5 process drop
-5 process drop

-- Test 2: Constraint modifies state variable - mutation should be discarded
$ x = 10 ;

= i64 -> i64 ;
? 999 -> x -1 ;
: mutate-test + ;

-- Even though constraint stores 999 to x, it should still be 10
5 mutate-test drop
x drop

-- x should still be 10, not 999
x 10 eq drop

-- Test 3: Multiple constraints modifying same variable
$ value = 100 ;

= i64 i64 -> i64 ;
? 200 -> value -1 ;
: test-multi + ;

= i64 i64 -> i64 ;
? 300 -> value -1 ;
: test-multi * ;

-- Both constraints modify value, but it should remain 100
10 20 test-multi drop
value drop
value 100 eq drop

-- Test 4: Constraint uses => (copy-store)
$ y = 5 ;

= i64 -> i64 ;
? dup 50 => y gt ;
: copy-test dup * ;

= i64 -> i64 ;
: copy-test ;

10 copy-test drop
y drop
y 5 eq drop

-- Test 5: Constraint reads multiple state variables
$ a = 10 ;
$ b = 20 ;

= i64 -> i64 ;
? dup a b + gt ;
: multi-read 1 + ;

= i64 -> i64 ;
: multi-read 1 - ;

50 multi-read drop
5 multi-read drop

-- Test 6: Failed constraint with state mutation
$ z = 42 ;

= i64 -> i64 ;
? 1000 -> z 0 ;
: fail-mutate ;

= i64 -> i64 ;
? -1 ;
: fail-mutate ;

-- First constraint modifies z then fails (returns 0)
-- Second constraint succeeds
-- z should still be 42
7 fail-mutate drop
z drop
z 42 eq drop
