#!/bin/bash
# Test database save/load functionality

# Clean up any existing database
rm -f march2.db

echo "=== Testing database save/load ==="
echo

echo "1. Creating a namespace with some words and saving it..."
./target/debug/march2 <<'EOF'
NAMESPACE. mylib ;
SIGNATURE. i64 -> i64 ;
: double dup + ;
: triple 3 * ;
: quadruple double double ;

-- Save the namespace
"mylib" march.save

bye
EOF

echo
echo "2. Starting fresh interpreter and loading the namespace..."
./target/debug/march2 <<'EOF'
-- Load the namespace
"mylib" march.load

-- Test that the words work
5 mylib.double .
5 mylib.triple .
5 mylib.quadruple .

bye
EOF

echo
echo "3. Checking if march2.db was created..."
if [ -f march2.db ]; then
    echo "✓ Database file exists"
    ls -lh march2.db
else
    echo "✗ Database file not found"
fi
