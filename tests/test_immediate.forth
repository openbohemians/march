-- Test immediate words (compile-time execution)

-- Simple immediate word that pushes a constant
= -> i64 ;
:: five 5 ;

-- Test: five should execute at compile-time
= -> i64 ;
: get-five five ;

TEST. get-five 5 eq ;

-- Immediate word that does computation
= -> i64 ;
:: ten 5 5 + ;

= -> i64 ;
: get-ten ten ;

TEST. get-ten 10 eq ;

-- Immediate word can be used multiple times
= -> i64 ;
: double-five five five + ;

TEST. double-five 10 eq ;

-- Immediate words can manipulate what gets compiled
-- For now, they just run at compile time with empty stack
-- (More advanced: they could read tokens and generate code)
