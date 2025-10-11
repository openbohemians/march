-- Test new marker-based array construction

-- Basic: inputs from main, outputs to buffer
10 20 { + } .

-- Empty array
{ } .

-- Multiple outputs
10 { dup dup } .

-- Literals inside braces (should error)
-- { 10 20 + }
