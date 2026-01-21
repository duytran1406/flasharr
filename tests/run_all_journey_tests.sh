#!/bin/bash
#
# Run All User Journey Tests
# Executes all journey test suites and generates summary report
#

echo "======================================================================"
echo "  USER JOURNEY TEST SUITE - COMPREHENSIVE RUN"
echo "======================================================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run a test
run_test() {
    local test_name=$1
    local test_file=$2
    
    echo ""
    echo "----------------------------------------------------------------------"
    echo "Running: $test_name"
    echo "----------------------------------------------------------------------"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if python3 "$test_file"; then
        echo -e "${GREEN}✅ $test_name PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}❌ $test_name FAILED${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Change to tests directory
cd "$(dirname "$0")"

# Run all journey tests
run_test "Download Journey" "test_journey_download.py"
run_test "Search Journey" "test_journey_search.py"
run_test "Settings Journey" "test_journey_settings.py"
run_test "Error Scenarios" "test_journey_errors.py"

# Print final summary
echo ""
echo "======================================================================"
echo "  FINAL TEST SUMMARY"
echo "======================================================================"
echo "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED!${NC}"
    echo "======================================================================"
    exit 0
else
    echo -e "${RED}❌ SOME TESTS FAILED${NC}"
    echo "======================================================================"
    exit 1
fi
