-- Test if word

-- Basic if tests (0 is false, non-zero is true)
TEST. 0 [ 1 ] [ 0 ] if 0 eq ;
TEST. -1 [ 1 ] [ 0 ] if 1 eq ;
TEST. 1 [ 1 ] [ 0 ] if 1 eq ;

-- if with operations
TEST. 5 3 gt [ 10 ] [ 20 ] if 10 eq ;
TEST. 3 5 gt [ 10 ] [ 20 ] if 20 eq ;

-- abs function using if
= i64 -> i64 ;
: abs dup 0 lt [ 0 swap - ] [ ] if ;

TEST. 5 abs 5 eq ;
TEST. -3 abs 3 eq ;
TEST. 0 abs 0 eq ;

-- max function using if
= i64 i64 -> i64 ;
: max over over gt [ drop ] [ swap drop ] if ;

TEST. 5 3 max 5 eq ;
TEST. 3 5 max 5 eq ;
TEST. 10 10 max 10 eq ;
