-- Array Broadcasting Demo

-- Define the wildcard broadcast handler
= {a} a -> {a} ;
: [*]
  swap
  dup array-len
  [ {
    over i0 array-get
    rot
    [*]
    swap
  } ] #do
  drop
;

-- Test with +
"Broadcasting { 1 2 3 } + 10 =" .
{ 1 2 3 } 10 +
"Result: " .
dup 0 array-get . " " .
dup 1 array-get . " " .
2 array-get .
"" .

-- Test with *
"Broadcasting { 5 10 15 } * 2 =" .
{ 5 10 15 } 2 *
"Result: " .
dup 0 array-get . " " .
dup 1 array-get . " " .
2 array-get .
"" .

-- Test with -
"Broadcasting { 100 50 25 } - 5 =" .
{ 100 50 25 } 5 -
"Result: " .
dup 0 array-get . " " .
dup 1 array-get . " " .
2 array-get .
