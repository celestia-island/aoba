#!/usr/bin/env bash
# Local CI Test Runner
# 
# This script runs GitHub Actions CI tests locally using Docker containers.
# It replicates the CI environment and outputs results to a specified directory
# for easy debugging and inspection.
#
# Usage:
#   ./scripts/run_ci_locally.sh [OPTIONS]
#
# Options:
#   --workflow <name>    Workflow to run: tui-rendering, tui-drilldown, cli, all (default: all)
#   --module <name>      Specific module to test (optional, runs all if not specified)
#   --output-dir <path>  Output directory for test results (default: ./ci-results)
#   --docker-image       Docker image to use (default: rust:latest)
#   --keep-container     Keep Docker container after test (for debugging)
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

set -e

# Default values
WORKFLOW="all"
MODULE=""
OUTPUT_DIR="./ci-results"
DOCKER_IMAGE="rust:latest"
KEEP_CONTAINER=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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
        --docker-image)
            DOCKER_IMAGE="$2"
            shift 2
            ;;
        --keep-container)
            KEEP_CONTAINER=true
            shift
            ;;
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
echo -e "${BLUE}║         Local CI Test Runner for Aoba Project         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}Configuration:${NC}"
echo -e "  Workflow:      ${GREEN}${WORKFLOW}${NC}"
echo -e "  Module:        ${GREEN}${MODULE:-all}${NC}"
echo -e "  Output Dir:    ${GREEN}${OUTPUT_DIR}${NC}"
echo -e "  Docker Image:  ${GREEN}${DOCKER_IMAGE}${NC}"
echo -e "  Repo Root:     ${GREEN}${REPO_ROOT}${NC}"
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
)

# Function to run a test in Docker
run_test_in_docker() {
    local workflow_type=$1
    local module_name=$2
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local container_name="aoba_ci_${workflow_type}_${module_name}_${timestamp}"
    local result_file="${OUTPUT_DIR}/${workflow_type}_${module_name}_${timestamp}.log"
    local status_file="${OUTPUT_DIR}/${workflow_type}_${module_name}_${timestamp}.status"
    
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}Running:${NC} ${workflow_type} / ${module_name}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    
    # Create Docker run script
    local docker_script="/tmp/run_test_${timestamp}.sh"
    
    cat > "$docker_script" << 'DOCKER_SCRIPT_EOF'
#!/bin/bash
set -e

echo "=== Installing system dependencies ==="
apt-get update -qq
apt-get install -y -qq libudev-dev pkg-config libx11-dev libxcb-shape0-dev libxcb-xfixes0-dev socat

echo "=== Setting up environment ==="
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1

cd /workspace

echo "=== Building packages ==="
DOCKER_SCRIPT_EOF

    # Add workflow-specific commands
    case "$workflow_type" in
        tui-rendering|tui-drilldown)
            cat >> "$docker_script" << 'DOCKER_SCRIPT_EOF'
cargo build --package aoba --package tui_e2e 2>&1 | tee build.log
chmod +x target/debug/aoba target/debug/tui_e2e

echo "=== Setting up virtual serial ports ==="
chmod +x scripts/socat_init.sh
./scripts/socat_init.sh 2>&1 | tee socat.log

echo "=== Running TUI E2E test ==="
DOCKER_SCRIPT_EOF
            if [[ "$workflow_type" == "tui-rendering" ]]; then
                echo "./target/debug/tui_e2e --module \$MODULE --screen-capture-only 2>&1" >> "$docker_script"
            else
                echo "./target/debug/tui_e2e --module \$MODULE 2>&1" >> "$docker_script"
            fi
            ;;
        cli)
            cat >> "$docker_script" << 'DOCKER_SCRIPT_EOF'
cargo build --package aoba --package aoba_cli --package cli_e2e 2>&1 | tee build.log
chmod +x target/debug/aoba target/debug/cli_e2e

if [[ "$MODULE" == "modbus_basic_master_slave" ]]; then
    echo "=== Setting up virtual serial ports ==="
    chmod +x scripts/socat_init.sh
    ./scripts/socat_init.sh 2>&1 | tee socat.log
fi

echo "=== Running CLI E2E test ==="
./target/debug/cli_e2e --module $MODULE 2>&1
DOCKER_SCRIPT_EOF
            ;;
    esac
    
    chmod +x "$docker_script"
    
    # Run Docker container
    local docker_run_args=(
        "run"
        "--name" "$container_name"
        "-v" "${REPO_ROOT}:/workspace"
        "-v" "${docker_script}:/run_test.sh"
        "-e" "MODULE=${module_name}"
        "-w" "/workspace"
        "--rm"
    )
    
    if [[ "$KEEP_CONTAINER" == "true" ]]; then
        docker_run_args=("${docker_run_args[@]/'--rm'/}")
    fi
    
    docker_run_args+=(
        "$DOCKER_IMAGE"
        "/run_test.sh"
    )
    
    # Execute and capture output
    local exit_code=0
    if docker "${docker_run_args[@]}" > "$result_file" 2>&1; then
        exit_code=0
        echo "SUCCESS" > "$status_file"
        echo -e "${GREEN}✓ PASSED${NC}: ${workflow_type} / ${module_name}"
    else
        exit_code=$?
        echo "FAILED (exit code: $exit_code)" > "$status_file"
        echo -e "${RED}✗ FAILED${NC}: ${workflow_type} / ${module_name} (exit code: $exit_code)"
    fi
    
    # Add summary to result file
    {
        echo ""
        echo "==================================="
        echo "Test Summary"
        echo "==================================="
        echo "Workflow: $workflow_type"
        echo "Module: $module_name"
        echo "Exit Code: $exit_code"
        echo "Timestamp: $(date)"
        echo "Log File: $result_file"
        echo "Status: $(cat "$status_file")"
    } >> "$result_file"
    
    echo -e "  Log saved to: ${GREEN}${result_file}${NC}"
    
    # Cleanup
    rm -f "$docker_script"
    
    return $exit_code
}

# Function to run all tests for a workflow
run_workflow_tests() {
    local workflow_type=$1
    local modules_array_name=$2
    
    # Get array reference
    declare -n modules=$modules_array_name
    
    local total=${#modules[@]}
    local passed=0
    local failed=0
    
    if [[ -n "$MODULE" ]]; then
        # Run specific module only
        if run_test_in_docker "$workflow_type" "$MODULE"; then
            ((passed++))
        else
            ((failed++))
        fi
    else
        # Run all modules
        for module in "${modules[@]}"; do
            if run_test_in_docker "$workflow_type" "$module"; then
                ((passed++))
            else
                ((failed++))
            fi
        done
    fi
    
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}Summary for ${workflow_type}:${NC}"
    echo -e "  Total:  ${total}"
    echo -e "  Passed: ${GREEN}${passed}${NC}"
    echo -e "  Failed: ${RED}${failed}${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    
    return $failed
}

# Main execution
main() {
    local total_failed=0
    
    # Check if Docker is available
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Error: Docker is not installed or not in PATH${NC}"
        exit 1
    fi
    
    # Pull Docker image if needed
    echo -e "${YELLOW}Checking Docker image...${NC}"
    if ! docker image inspect "$DOCKER_IMAGE" &> /dev/null; then
        echo -e "${YELLOW}Pulling Docker image: ${DOCKER_IMAGE}${NC}"
        docker pull "$DOCKER_IMAGE"
    fi
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
        echo -e "${GREEN}✓ All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}✗ $total_failed test(s) failed${NC}"
        echo ""
        echo "Failed tests:"
        grep -l "FAILED" "${OUTPUT_DIR}"/*.status 2>/dev/null | while read -r status_file; do
            local log_file="${status_file%.status}.log"
            echo -e "  ${RED}✗${NC} $(basename "$log_file")"
        done
        exit 1
    fi
}

# Run main function
main
