#!/bin/bash
# Script to test all error cases and verify they produce errors
# Usage: ./tests/errors/test_all.sh

echo "VeGen Type Error Test Suite"
echo "============================"
echo ""

total=0
passed=0
failed=0

# Get the directory of this script to locate .vg files
SCRIPT_DIR="$(dirname "$0")"

# Find all .vg files in the errors directory
for file in $SCRIPT_DIR/examples/*.vg; do
    
    total=$((total + 1))
    echo "======= $file ======="
    
    # Run the compiler and capture exit code
    cargo run -q "$file" -q > /dev/null
    exit_code=$?
    
    # We expect compilation to fail (non-zero exit code)
    if [ $exit_code -ne 0 ]; then
        echo "✓ PASS (error detected)"
        passed=$((passed + 1))
    else
        echo "✗ FAIL (no error detected)"
        failed=$((failed + 1))
    fi
done

echo ""
echo "============================"
echo "Results: $passed/$total passed"

if [ $failed -gt 0 ]; then
    echo "WARNING: $failed tests did not produce expected errors"
    exit 1
else
    echo "All tests passed!"
    exit 0
fi
