-- Test context constraints (guards) with ?

-- Define max with two constrained variants
= i64 i64 -> i64 ;
? over over gt ;
: max drop ;

= i64 i64 -> i64 ;
? over over lte ;
: max swap drop ;

-- Test max function
5 3 max drop
3 5 max drop
10 10 max drop

-- Define abs with a constraint
= i64 -> i64 ;
? dup 0 lt ;
: abs 0 swap - ;

= i64 -> i64 ;
? dup 0 gte ;
: abs ;

-- Test abs function
-5 abs drop
5 abs drop
0 abs drop

-- Test constraint with state variables
$ threshold = 10 ;

= i64 -> i64 ;
? dup threshold gt ;
: classify drop 1 ;

= i64 -> i64 ;
? dup threshold lte ;
: classify drop 0 ;

-- Test classification
15 classify drop
5 classify drop
10 classify drop

-- Test that constraints don't mutate stack
= i64 i64 -> i64 ;
? over over eq ;
: special-add drop dup + ;

= i64 i64 -> i64 ;
: special-add + ;

-- Test special-add
5 5 special-add drop
5 3 special-add drop
