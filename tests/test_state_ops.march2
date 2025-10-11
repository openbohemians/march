-- Test state variable operators (-> and =>)

-- Test ->
$ x = 0 ;
42 -> x
x drop

-- Test =>
$ y = 0 ;
10 => y drop
y drop

-- Test both together
$ z = 0 ;
5 => z -> z
z drop

-- Test in word definitions
= -> ;
: set-x 100 -> x ;
set-x
x drop

= i64 -> ;
: add-to-x x + -> x ;
5 add-to-x
x drop
