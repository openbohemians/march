-- Test array broadcasting

-- Define wildcard broadcast handler
= {a} a -> {a} ;
: [*]
  swap                   -- scalar {array}
  dup array.len          -- scalar {array} len
  [ {
    over i0 array.get    -- scalar {array} elem
    rot                  -- {array} elem scalar
    [*]                  -- {array} result (redispatch to scalar version)
    swap                 -- result {array}
  } ] #do
  drop                   -- {results}
;

-- Define array equality helper
= {a} {a} -> i64 ;
: array.eq?
  over array.len over array.len eq not [ drop drop 0 ] [
    dup array.len
    [ {
      over i0 array.get
      rot i0 array.get
      eq not [ drop drop 0 ] [ swap ] if
    } ] #do
    drop drop -1
  ] if
;

-- Test broadcasting with +
TEST. { 1 2 3 } 1 + { 2 3 4 } array.eq? ;

-- Test broadcasting with *
TEST. { 2 4 6 } 2 * { 4 8 12 } array.eq? ;

-- Test broadcasting with -
TEST. { 10 20 30 } 5 - { 5 15 25 } array.eq? ;

-- Test broadcasting with /
TEST. { 10 20 30 } 2 / { 5 10 15 } array.eq? ;
