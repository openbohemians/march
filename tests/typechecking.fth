-- Compile-time type checking tests

-- Correct type usage
SIGNATURE. i64 i64 -> i64 ;
: add + ;
: sub - ;

TEST. typecheck-add 5 3 add 8 eq? ;
TEST. typecheck-sub 10 3 sub 7 eq? ;

-- Unary operations
SIGNATURE. i64 -> i64 ;
: double dup + ;
: triple dup dup + + ;

TEST. typecheck-double 5 double 10 eq? ;
TEST. typecheck-triple 3 triple 9 eq? ;

-- TODO: These should fail at compile time (uncomment to test):
-- SIGNATURE. i64 i64 -> i64 ;
-- : type-error-wrong-type "hello" + ;  ( Should fail: string instead of i64 )
-- : type-error-not-enough 5 + ;        ( Should fail: only 1 input, need 2 )

bye
