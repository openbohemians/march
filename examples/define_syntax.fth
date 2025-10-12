-- Examples showing DEFINE. syntax (long-form) vs : (short-form)

-- Both syntaxes work identically

-- Short form (REPL-friendly)
: square ( dup * ) call ;
: double ( dup + ) call ;
: abs dup 0 lt? ( 0 swap - ) ( ) if ;

-- Long form (database serialization format)
DEFINE. cube ( dup dup * * ) call ;
DEFINE. triple ( dup dup + + ) call ;
DEFINE. max ( ) swap gt? ( swap ) ( ) if ;

-- Testing
5 square .        -- 25
3 cube .          -- 27
4 double .        -- 8
6 triple .        -- 18
-7 abs .          -- 7
3 5 max .         -- 5

-- Note: SIGNATURE. and CONTEXT. are not yet implemented
-- They will be added in future versions
