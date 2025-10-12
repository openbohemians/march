-- Type system tests (first-class types)

-- Test type predicates with generic ? operator
TEST. pred-i64-true 10 i64 ? ;
TEST. pred-i64-false "hello" i64 ? 0 eq? ;
TEST. pred-string-true "hello" string ? ;
TEST. pred-string-false 10 string ? 0 eq? ;
TEST. pred-quotation-true ( 1 2 + ) quotation ? ;
TEST. pred-quotation-false 10 quotation ? 0 eq? ;

-- Test type casting with generic ! operator
TEST. cast-string-to-i64 "42" i64 ! 42 eq? ;
TEST. cast-i64-idempotent 123 i64 ! 123 eq? ;

-- TODO: String comparison needed for these tests
-- TEST. type-constant-i64 i64 type "core.type" eq? ;
-- TEST. type-returns-string 10 type "core.i64" eq? ;
-- TEST. cast-i64-to-string 123 string ! "123" eq? ;

bye
