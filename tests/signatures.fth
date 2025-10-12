-- Type signature tests

-- Define binary operations with signature
SIGNATURE. i64 i64 -> i64 ;
: add + ;
: sub - ;
: multiply * ;

-- Test they work
TEST. sig-add 5 3 add 8 eq? ;
TEST. sig-sub 10 3 sub 7 eq? ;
TEST. sig-mul 4 5 multiply 20 eq? ;

-- Define unary operations with new signature
SIGNATURE. i64 -> i64 ;
: square dup * ;
: double dup + ;

-- Test they work
TEST. sig-square 5 square 25 eq? ;
TEST. sig-double 7 double 14 eq? ;

-- TODO: Runtime type checking not yet enforced
-- These should fail with type errors once checking is implemented:
-- TEST. sig-type-error "hello" square ;

bye
