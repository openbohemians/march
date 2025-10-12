-- Basic test suite for March2 FORTH
-- Tests: 0 = FAIL, non-zero = PASS (standard truthy logic)

-- Arithmetic tests
TEST. addition 2 3 + 5 eq? ;
TEST. subtraction 10 3 - 7 eq? ;
TEST. multiplication 4 5 * 20 eq? ;
TEST. division 20 4 / 5 eq? ;

-- Stack manipulation tests
TEST. dup-works 5 dup + 10 eq? ;
TEST. swap-works 3 7 swap - 4 eq? ;
TEST. drop-works 1 2 3 drop drop 1 eq? ;
TEST. over-works 1 2 over + + 4 eq? ;
TEST. rot-works 1 2 3 rot 1 eq? swap 3 eq? swap 2 eq? + + 3 eq? ;

-- Comparison tests
TEST. lt-true 3 5 lt? ;
TEST. lt-false 5 3 lt? 0 eq? ;
TEST. gt-true 5 3 gt? ;
TEST. gt-false 3 5 gt? 0 eq? ;
TEST. eq-true 7 7 eq? ;
TEST. eq-false 7 8 eq? 0 eq? ;

-- Define helper words for testing
DEFINE. square ( dup * ) call ;
DEFINE. double ( dup + ) call ;
DEFINE. abs dup 0 lt? ( 0 swap - ) ( ) if ;

-- Custom word tests
TEST. square-works 5 square 25 eq? ;
TEST. double-works 7 double 14 eq? ;
TEST. abs-positive 5 abs 5 eq? ;
TEST. abs-negative -5 abs 5 eq? ;

-- Quotation tests
TEST. quotation-call 5 ( dup + ) call 10 eq? ;
-- TODO: nested quotations not yet supported
-- TEST. nested-quotation 3 ( ( 2 * ) call 1 + ) call 7 eq? ;

-- Conditional tests
TEST. if-true-branch 1 ( 100 ) ( 200 ) if 100 eq? ;
TEST. if-false-branch 0 ( 100 ) ( 200 ) if 200 eq? ;
TEST. iff-true 1 ( 42 ) iff 42 eq? ;

-- Return stack tests
TEST. to-r-from-r 5 >r 10 r> + 15 eq? ;
TEST. r-fetch 5 >r r@ r@ + r> drop 10 eq? ;

bye
