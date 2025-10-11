-- Test recursive word definitions

-- Factorial using recursion (n -> n!)
= i64 -> i64 ;
: factorial dup 1 lte [ ] [ dup 1 - factorial * ] if ;

TEST. 5 factorial 120 eq ;
TEST. 3 factorial 6 eq ;
TEST. 1 factorial 1 eq ;
TEST. 0 factorial 0 eq ;

-- Countdown using recursion
= i64 -> i64 ;
: countdown dup 0 gt [ 1 - countdown ] [ ] if ;

TEST. 5 countdown 0 eq ;
TEST. 0 countdown 0 eq ;

-- Test that infinite recursion hits the depth limit
-- = -> ;
-- : loop-forever loop-forever ;
-- loop-forever  -- Would error with "Maximum recursion depth exceeded"
