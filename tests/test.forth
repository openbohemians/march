-- Test comparisons
TEST. 5 5 eq -1 eq ;
TEST. 5 3 eq 0 eq ;
TEST. 3 5 lt -1 eq ;
TEST. 5 3 lt 0 eq ;

-- Test loop with #do (just verify it works)
[ 42 ] 1 #do drop
[ i0 ] 5 #do drop drop drop drop drop
