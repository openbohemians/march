-- Database Integration Tests

-- Test basic namespace save/load flow (conceptual test)
-- For now we're just testing that the infrastructure compiles and links
-- Full database integration will need Rust tests since we need database access

-- Create a simple namespace with some definitions
NAMESPACE. testlib ;
SIGNATURE. i64 -> i64 ;
: add2 2 + ;
: double dup + ;

-- Test that the words exist and work
TEST. testlib-add2 5 testlib.add2 7 eq? ;
TEST. testlib-double 5 testlib.double 10 eq? ;
