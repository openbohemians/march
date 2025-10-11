-- Test compile-time evaluation in state declarations

-- Simple expressions
$ x = 5 3 + ;
x drop

-- Arrays with expressions
$ arr = { 10 20 + 30 40 * } ;
arr drop

-- Nested collections
$ matrix = { { 1 2 } { 3 4 } } ;
matrix drop

-- Reference other state variables
$ base = 100 ;
$ derived = base 2 / ;
derived drop

-- Arrays with state references
$ coords = { base derived } ;
coords drop

-- Quotations
$ increment = [ 1 + ] ;
5 increment call drop

-- Complex initialization
$ computed = { 1 2 3 } 1 @ 10 * ;
computed drop
