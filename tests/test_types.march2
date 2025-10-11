-- Test type stack
5 10 .types
-- Should show: Types <2> i64 i64

-- Add and check
+ .types
-- Should show: Types <1> i64

-- Add a quotation
[ 42 ] .types
-- Should show: Types <2> i64 quot

-- Type error: try to add quotation and number (should fail)
-- +

-- Proper usage
drop drop
5 [ dup * ] .types
-- Should show: Types <2> i64 quot

-- Execute quotation
call .types
-- Should show: Types <1> i64

-- Print result
.
-- Should print 25

-- Test type checking with times
[ i0 . ] 3 .types
-- Should show: Types <2> quot i64

#do
-- Should execute and clear stack
.types
-- Should show: Types <0>
