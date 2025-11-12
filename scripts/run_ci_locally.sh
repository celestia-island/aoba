#!/usr/bin/env bash
# Local CI Test Runner
#
# This script runs GitHub Actions CI tests locally.
# It replicates the CI environment (as much as possible) and outputs results to a specified directory
# for easy debugging and inspection.
#
# Usage:
#   ./scripts/run_ci_locally.sh [OPTIONS]
#
# Options:
#   --workflow <name>    Workflow to run: tui-rendering, tui-drilldown, cli, all (default: all)
#   --module <name>      Specific module to test (optional, runs all if not specified)
#   --output-dir <path>  Output directory for test results (default: ./ci-results)
#   --help               Show this help message
#
# Examples:
#   # Run all tests
#   ./scripts/run_ci_locally.sh
#
#   # Run only TUI rendering tests
#   ./scripts/run_ci_locally.sh --workflow tui-rendering
#
#   # Run specific module
#   ./scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils
#
#   # Specify custom output directory
#   ./scripts/run_ci_locally.sh --output-dir /tmp/my-ci-results

set -o pipefail

# Default values
WORKFLOW="all"
MODULE=""
OUTPUT_DIR="./ci-results"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Force English locale so UI text matches workflow expectations
export LANGUAGE="en_US"
export LC_ALL="en_US.UTF-8"
export LANG="en_US.UTF-8"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

MODULE_TIMEOUT_SECS="${MODULE_TIMEOUT_SECS:-60}"
USE_TIMEOUT=0

if [[ -z "$MODULE_TIMEOUT_SECS" ]]; then
    MODULE_TIMEOUT_SECS=0
elif [[ "$MODULE_TIMEOUT_SECS" =~ ^[0-9]+$ ]]; then
    if (( MODULE_TIMEOUT_SECS > 0 )); then
        if ! command -v timeout >/dev/null 2>&1; then
            echo -e "${RED}Error: timeout command not found but MODULE_TIMEOUT_SECS=${MODULE_TIMEOUT_SECS}${NC}"
            exit 1
        fi
        USE_TIMEOUT=1
    fi
else
    echo -e "${RED}Error: MODULE_TIMEOUT_SECS must be a non-negative integer (current: ${MODULE_TIMEOUT_SECS})${NC}"
    exit 1
fi

PYTHON_CMD="${PYTHON_CMD:-python3}"
WATCHDOG_HELPER="${SCRIPT_DIR}/run_with_watchdog.py"

INACTIVITY_TIMEOUT_SECS="${INACTIVITY_TIMEOUT_SECS:-60}"
INACTIVITY_NOTIFY_SECS="${INACTIVITY_NOTIFY_SECS:-10}"

if ! command -v "$PYTHON_CMD" >/dev/null 2>&1; then
    echo -e "${RED}Error: ${PYTHON_CMD} not found in PATH${NC}"
    exit 1
fi

if [[ ! -f "$WATCHDOG_HELPER" ]]; then
    echo -e "${RED}Error: Watchdog helper not found at ${WATCHDOG_HELPER}${NC}"
    echo -e "${RED}Please ensure scripts/run_with_watchdog.py exists.${NC}"
    exit 1
fi

if [[ ! "$INACTIVITY_TIMEOUT_SECS" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}Error: INACTIVITY_TIMEOUT_SECS must be a non-negative integer (current: ${INACTIVITY_TIMEOUT_SECS})${NC}"
    exit 1
fi

if [[ ! "$INACTIVITY_NOTIFY_SECS" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}Error: INACTIVITY_NOTIFY_SECS must be a non-negative integer (current: ${INACTIVITY_NOTIFY_SECS})${NC}"
    exit 1
fi

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --workflow)
            WORKFLOW="$2"
            shift 2
            ;;
        --module)
            MODULE="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        # (docker options removed - script now runs locally by default)
        --help)
            head -n 30 "$0" | grep "^#" | sed 's/^# //g' | sed 's/^#//g'
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Validate workflow argument
if [[ ! "$WORKFLOW" =~ ^(tui-rendering|tui-drilldown|cli|all)$ ]]; then
    echo -e "${RED}Error: Invalid workflow. Must be one of: tui-rendering, tui-drilldown, cli, all${NC}"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         Local CI Test Runner for Aoba Project          ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}Configuration:${NC}"
echo -e "  Workflow:      ${GREEN}${WORKFLOW}${NC}"
echo -e "  Module:        ${GREEN}${MODULE:-all}${NC}"
echo -e "  Output Dir:    ${GREEN}${OUTPUT_DIR}${NC}"
echo -e "  Repo Root:     ${GREEN}${REPO_ROOT}${NC}"
if [[ $USE_TIMEOUT -eq 1 ]]; then
    echo -e "  Module Timeout:${GREEN}${MODULE_TIMEOUT_SECS}s${NC}"
else
    echo -e "  Module Timeout:${GREEN}disabled${NC}"
fi
if (( INACTIVITY_TIMEOUT_SECS > 0 )); then
    echo -e "  Inactivity Timeout:${GREEN}${INACTIVITY_TIMEOUT_SECS}s${NC}"
else
    echo -e "  Inactivity Timeout:${GREEN}disabled${NC}"
fi
if (( INACTIVITY_NOTIFY_SECS > 0 )); then
    echo -e "  Inactivity Notify :${GREEN}${INACTIVITY_NOTIFY_SECS}s${NC}"
else
    echo -e "  Inactivity Notify :${GREEN}disabled${NC}"
fi
echo -e "  Python Runner  : ${GREEN}${PYTHON_CMD}${NC}"
echo ""

# Define test modules
declare -a TUI_RENDERING_MODULES=(
    "single_station_master_coils"
    "single_station_master_discrete_inputs"
    "single_station_master_holding"
    "single_station_master_input"
    "single_station_slave_coils"
    "single_station_slave_discrete_inputs"
    "single_station_slave_holding"
    "single_station_slave_input"
    "multi_station_master_mixed_types"
    "multi_station_master_spaced_addresses"
    "multi_station_master_mixed_ids"
    "multi_station_slave_mixed_types"
    "multi_station_slave_spaced_addresses"
    "multi_station_slave_mixed_ids"
)

declare -a TUI_DRILLDOWN_MODULES=("${TUI_RENDERING_MODULES[@]}")

declare -a CLI_MODULES=(
    "help"
    "list_ports"
    "list_ports_json"
    "list_ports_status"
    "modbus_basic_master_slave"
    "data_source_http"
    "data_source_http_persist"
)

## Docker support removed: this script runs tests locally only.
TUI_E2E_EXTRA_ARGS="${TUI_E2E_EXTRA_ARGS:-}"

# Helper: run a shell command string and append its combined stdout/stderr to a log file
# Returns the exit code of the command (not tee). Uses bash -c to run the provided string.
run_and_log_cmd() {
    local result_file="$1"; shift
    local label="$1"; shift
    local cmd="$*"

    local watchdog_args=("--cmd" "$cmd" "--log-file" "$result_file" "--label" "$label")
    watchdog_args+=("--inactivity-timeout" "$INACTIVITY_TIMEOUT_SECS")
    watchdog_args+=("--notify-interval" "$INACTIVITY_NOTIFY_SECS")

    # preserve errexit state, disable it while running helper so we can capture its exit code
    local errexit_state
    errexit_state=$(set -o | awk '/errexit/ {print $2}')
    set +e
    "$PYTHON_CMD" "$WATCHDOG_HELPER" "${watchdog_args[@]}"
    local exit_code=$?
    # restore previous errexit state
    if [[ "$errexit_state" == "on" ]]; then
        set -e
    else
        set +e
    fi

    if [[ $exit_code -eq 124 && $USE_TIMEOUT -eq 1 ]]; then
        echo "Command timed out after ${MODULE_TIMEOUT_SECS}s and was terminated." | tee -a "$result_file"
    elif [[ $exit_code -eq 137 && $USE_TIMEOUT -eq 1 ]]; then
        echo "Command exceeded the timeout and was killed (exit ${exit_code})." | tee -a "$result_file"
    fi

    return $exit_code
}

# Function to run all tests for a workflow (simplified/reliable implementation)
run_workflow_tests() {
    local workflow_type=$1
    local modules_array_name=$2
    declare -n modules=$modules_array_name

    local modules_to_run=("${modules[@]}")
    if [[ -n "$MODULE" ]]; then
        modules_to_run=("$MODULE")
    fi

    local total=${#modules_to_run[@]}
    local passed=0
    local failed=0

    echo -e "${YELLOW}Running locally. Will run socat_init.sh before each module.${NC}"

    # Build packages once per workflow
    case "$workflow_type" in
        tui-rendering|tui-drilldown)
            echo "=== Building (local) packages: aoba, tui_e2e ==="
            run_and_log_cmd "$OUTPUT_DIR/${workflow_type}_build.log" "build:${workflow_type}" "cd \"${REPO_ROOT}\" && cargo build --package aoba --package tui_e2e"
            local build_exit=$?
            if [[ $build_exit -ne 0 ]]; then
                echo -e "${RED}Build failed for workflow ${workflow_type} (exit ${build_exit}). See $OUTPUT_DIR/${workflow_type}_build.log${NC}"
                return $build_exit
            fi
            chmod +x "${REPO_ROOT}/target/debug/aoba" "${REPO_ROOT}/target/debug/tui_e2e" || true
            ;;
        cli)
            echo "=== Building (local) packages: aoba, cli_e2e ==="
            run_and_log_cmd "$OUTPUT_DIR/${workflow_type}_build.log" "build:${workflow_type}" "cd \"${REPO_ROOT}\" && cargo build --package aoba --package cli_e2e"
            local build_exit=$?
            if [[ $build_exit -ne 0 ]]; then
                echo -e "${RED}Build failed for workflow ${workflow_type} (exit ${build_exit}). See $OUTPUT_DIR/${workflow_type}_build.log${NC}"
                return $build_exit
            fi
            chmod +x "${REPO_ROOT}/target/debug/aoba" "${REPO_ROOT}/target/debug/cli_e2e" || true
            ;;
        *)
            echo "Unknown workflow type: $workflow_type"
            return 2
            ;;
    esac

    for module in "${modules_to_run[@]}"; do
        local timestamp
        timestamp=$(date +%Y%m%d_%H%M%S)
        local result_file="${OUTPUT_DIR}/${workflow_type}_${module}_${timestamp}.log"
        local status_file="${OUTPUT_DIR}/${workflow_type}_${module}_${timestamp}.status"
        local module_runner=""
        if [[ $USE_TIMEOUT -eq 1 ]]; then
            module_runner="timeout --foreground ${MODULE_TIMEOUT_SECS}s"
        fi

        echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${YELLOW}Running (local):${NC} ${workflow_type} / ${module}"
        echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

        # Reset socat before each module if available
        if [[ -x "${REPO_ROOT}/scripts/socat_init.sh" ]]; then
            echo "=== Resetting virtual serial ports before module: ${module} ===" | tee -a "$result_file"
            run_and_log_cmd "$result_file" "socat_init" "cd \"${REPO_ROOT}\" && ./scripts/socat_init.sh" || true
        else
            echo "Warning: ${REPO_ROOT}/scripts/socat_init.sh not found or not executable" | tee -a "$result_file"
        fi

        # Run module
        if [[ "$workflow_type" == "cli" ]]; then
            run_and_log_cmd "$result_file" "module:${workflow_type}/${module}" "cd \"${REPO_ROOT}\" && ${module_runner:+$module_runner }./target/debug/cli_e2e --module \"$module\""
        else
            run_and_log_cmd "$result_file" "module:${workflow_type}/${module}" "cd \"${REPO_ROOT}\" && ${module_runner:+$module_runner }./target/debug/tui_e2e --module \"$module\" ${TUI_E2E_EXTRA_ARGS:-} --screen-capture-only"
        fi
        local exit_code=$?

        if [[ $exit_code -eq 0 ]]; then
            echo "SUCCESS" > "$status_file" || true
            echo -e "${GREEN}✅ PASSED${NC}: ${workflow_type} / ${module}"
            ((passed++))
        else
            echo "FAILED (exit code: $exit_code)" > "$status_file" || true
            echo -e "${RED}❌ FAILED${NC}: ${workflow_type} / ${module} (exit code: $exit_code)"
            ((failed++))
        fi

        # Write a small summary
        {
            echo ""
            echo "==================================="
            echo "Test Summary"
            echo "==================================="
            echo "Workflow: $workflow_type"
            echo "Module: $module"
            echo "Exit Code: $exit_code"
            echo "Timestamp: $(date)"
            echo "Log File: $result_file"
                echo -n "Status: "
                cat "$status_file" || true
        } >> "$result_file" 2>/dev/null || true

        echo -e "  Log saved to: ${GREEN}${result_file}${NC}"
    done

    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}Summary for ${workflow_type}:${NC}"
    echo -e "  Total:  ${total}"
    echo -e "  Passed: ${GREEN}${passed}${NC}"
    echo -e "  Failed: ${RED}${failed}${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    # run_workflow_tests completed for this workflow

    return $failed
}

# Main execution
main() {
    local total_failed=0

    # This runner executes tests locally by default.
    echo -e "${YELLOW}Running tests locally.${NC}"
    echo ""

    case "$WORKFLOW" in
        tui-rendering)
            run_workflow_tests "tui-rendering" "TUI_RENDERING_MODULES"
            total_failed=$?
            ;;
        tui-drilldown)
            run_workflow_tests "tui-drilldown" "TUI_DRILLDOWN_MODULES"
            total_failed=$?
            ;;
        cli)
            run_workflow_tests "cli" "CLI_MODULES"
            total_failed=$?
            ;;
        all)
            echo -e "${YELLOW}Running all workflows...${NC}"
            echo ""

            run_workflow_tests "tui-rendering" "TUI_RENDERING_MODULES"
            ((total_failed+=$?))

            run_workflow_tests "tui-drilldown" "TUI_DRILLDOWN_MODULES"
            ((total_failed+=$?))

            run_workflow_tests "cli" "CLI_MODULES"
        ((total_failed+=$?))
            ;;
    esac

    # Final summary
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║                   Final Summary                        ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "Output directory: ${GREEN}${OUTPUT_DIR}${NC}"
    echo ""
    echo "Test results have been saved to individual log files."
    echo "You can examine each test's output in the following format:"
    echo "  ${OUTPUT_DIR}/<workflow>_<module>_<timestamp>.log"
    echo "  ${OUTPUT_DIR}/<workflow>_<module>_<timestamp>.status"
    echo ""

    if [[ $total_failed -eq 0 ]]; then
        echo -e "${GREEN}✅ All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}❌ $total_failed test(s) failed${NC}"
        echo ""
        echo "Failed tests:"
        grep -l "FAILED" "${OUTPUT_DIR}"/*.status 2>/dev/null | while read -r status_file; do
            local log_file="${status_file%.status}.log"
            echo -e "  ${RED}❌${NC} $(basename "$log_file")"
        done
        exit 1
    fi
}

# Run main function inside a subshell so we can capture and return its exit code reliably
( main "$@" )
final_rc=$?
if [[ $final_rc -ne 0 ]]; then
    echo -e "${RED}Runner exited with code ${final_rc}${NC}"
fi
exit $final_rc
