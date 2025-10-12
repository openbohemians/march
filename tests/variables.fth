-- Variable system tests

-- Create a simple numeric variable
VARIABLE. x 10 ;
TEST. var-create x 10 eq? ;

-- Store a new value
VARIABLE. y 10 ;
TEST. var-store 20 -> y y 20 eq? ;

-- Computed initial value
VARIABLE. sum 5 5 + ;
TEST. var-computed sum 10 eq? ;

-- Variables are independent
VARIABLE. a 5 ;
VARIABLE. b 15 ;
TEST. var-independent a b + 20 eq? ;

-- Can redefine variables
VARIABLE. z 100 ;
TEST. var-redefine 200 -> z z 200 eq? ;

-- TODO: String comparison not yet implemented
-- VARIABLE. msg "hello" ;
-- TEST. var-string msg "hello" eq? ;

bye
