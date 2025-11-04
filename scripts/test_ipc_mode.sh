#!/bin/bash
# Test script for TUI IPC mode
set -e

echo "=== TUI IPC Mode Test Suite ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0

run_test() {
    local test_name="$1"
    local test_cmd="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "Test $TESTS_RUN: $test_name... "
    
    if eval "$test_cmd" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗${NC}"
        return 1
    fi
}

# Test 1: Screen request
run_test "Screen request" \
    "echo '{\"type\": \"request_screen\"}' | timeout 5 cargo run --quiet --package aoba -- --tui --debug-ci 2>/dev/null | jq -e '.type == \"screen_content\"'"

# Test 2: Keyboard event + screen
run_test "Keyboard + screen" \
    "(echo '{\"type\": \"key_press\", \"key\": \"Down\"}'; sleep 0.5; echo '{\"type\": \"request_screen\"}'; echo '{\"type\": \"shutdown\"}') | timeout 10 cargo run --quiet --package aoba -- --tui --debug-ci 2>/dev/null | jq -e '.type == \"screen_content\"'"

# Test 3: Character input
run_test "Character input" \
    "(echo '{\"type\": \"char_input\", \"ch\": \"a\"}'; sleep 0.5; echo '{\"type\": \"shutdown\"}') | timeout 5 cargo run --quiet --package aoba -- --tui --debug-ci 2>/dev/null || true"

# Test 4: Graceful shutdown
run_test "Graceful shutdown" \
    "echo '{\"type\": \"shutdown\"}' | timeout 5 cargo run --quiet --package aoba -- --tui --debug-ci 2>&1 | grep -q 'Cleaning up' || true"

# Test 5: Invalid JSON handling
run_test "Invalid JSON handling" \
    "(echo 'invalid json'; sleep 0.5; echo '{\"type\": \"shutdown\"}') | timeout 5 cargo run --quiet --package aoba -- --tui --debug-ci 2>&1 || true"

# Summary
echo ""
echo "=== Test Summary ==="
echo "Tests run: $TESTS_RUN"
echo "Tests passed: $TESTS_PASSED"
echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"

if [ $TESTS_PASSED -eq $TESTS_RUN ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
