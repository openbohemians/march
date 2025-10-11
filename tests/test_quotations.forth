-- Test basic quotation
TEST. [ 42 ] call 42 eq ;

-- Test quotation on stack
TEST. [ 10 20 + ] dup call swap call + 60 eq ;

-- Nested quotations
TEST. [ [ 99 ] call ] call 99 eq ;
