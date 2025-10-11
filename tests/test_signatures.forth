-- Test type signatures

-- Type variable
= a -> a ;
: identity ;

-- Concrete types
= string -> string ;
: process-string ;

-- Multiple inputs
= i64 i64 -> i64 ;
: add + ;

-- Multiple outputs
= i64 -> i64 i64 ;
: dup-int dup ;

-- No inputs
= -> i64 ;
: get-constant 42 ;

-- No outputs
= i64 -> ;
: consume drop ;

5 identity .
