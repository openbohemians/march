#!/bin/bash
# Run all FORTH test files and summarize results

echo "=== FORTH Test Suite ==="
echo

total_pass=0
total_fail=0

for test_file in tests/*.fth; do
    if [ -f "$test_file" ]; then
        echo "Running: $test_file"
        result=$(cat "$test_file" | ./target/debug/march2 2>&1 | grep -E "✓|✗")

        if [ -n "$result" ]; then
            echo "$result"
            pass=$(echo "$result" | grep -c "✓" || true)
            fail=$(echo "$result" | grep -c "✗" || true)
            total_pass=$((total_pass + pass))
            total_fail=$((total_fail + fail))
        else
            echo "  (no tests found)"
        fi
        echo
    fi
done

echo "=== Summary ==="
echo "Total PASS: $total_pass"
echo "Total FAIL: $total_fail"
echo "Total tests: $((total_pass + total_fail))"
