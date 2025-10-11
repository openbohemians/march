-- Test error handling with type hierarchy

-- Define a division function (normal case)
= i64 i64 -> i64 ;
: safe-div swap dup 0 eq [ drop raise DivideByZero ] [ / ] if ;

-- Error handler for DivideByZero (gets error + original args)
= DivideByZero i64 i64 -> i64 ;
: safe-div drop drop drop 0 ;

-- Test normal division
10 2 safe-div drop
10 2 safe-div 5 eq drop

-- Test divide by zero (should catch and return 0)
10 0 safe-div drop
10 0 safe-div 0 eq drop

-- Test that original args are available
= i64 i64 -> i64 ;
: safe-div2 swap dup 0 eq [ drop raise DivideByZero ] [ / ] if ;

= DivideByZero i64 i64 -> i64 ;
: safe-div2 drop swap drop ;

10 0 safe-div2 drop
10 0 safe-div2 10 eq drop
