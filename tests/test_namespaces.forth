-- Test namespaces

-- Define a namespace
<> math ;

-- Define a word in the namespace using qualified name
= i64 i64 -> i64 ;
: math.div / ;

= i64 i64 -> i64 ;
: math.add + ;

-- Test direct qualified name call
TEST. 10 5 math.div 2 eq ;
TEST. 20 15 math.add 35 eq ;

-- Test compile-time namespace dispatch
TEST. 10 5 math div 2 eq ;
TEST. 20 15 math add 35 eq ;

-- Test multiple namespaces
<> strings ;

= str str -> str ;
: strings.join ++ ;

-- Just verify strings.join works (can't test string equality easily)
"hello" " world" strings.join drop
