# Local CI Test Runner

This script allows you to run GitHub Actions CI tests locally using Docker containers. It replicates the CI environment and outputs results to a specified directory for easy debugging.

## Prerequisites

- Docker installed and running
- Bash shell (Linux/macOS) or WSL2 (Windows)
- Sufficient disk space for Docker images and test artifacts

## Quick Start

### Run all tests
```bash
./scripts/run_ci_locally.sh
```

### Run specific workflow
```bash
# TUI rendering tests only
./scripts/run_ci_locally.sh --workflow tui-rendering

# TUI drill-down tests only
./scripts/run_ci_locally.sh --workflow tui-drilldown

# CLI tests only
./scripts/run_ci_locally.sh --workflow cli
```

### Run specific module
```bash
# Run single TUI rendering test
./scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils

# Run single CLI test
./scripts/run_ci_locally.sh --workflow cli --module help
```

## Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `--workflow <name>` | Workflow to run: `tui-rendering`, `tui-drilldown`, `cli`, or `all` | `all` |
| `--module <name>` | Specific module to test (optional) | All modules |
| `--output-dir <path>` | Output directory for test results | `./ci-results` |
| `--docker-image <name>` | Docker image to use | `rust:latest` |
| `--keep-container` | Keep Docker container after test for debugging | false |
| `--help` | Show help message | - |

## Output Directory Structure

Test results are saved in the output directory (default: `./ci-results/`) with the following structure:

```
ci-results/
├── tui-rendering_single_station_master_coils_20250103_120000.log
├── tui-rendering_single_station_master_coils_20250103_120000.status
├── tui-drilldown_single_station_master_coils_20250103_120100.log
├── tui-drilldown_single_station_master_coils_20250103_120100.status
├── cli_help_20250103_120200.log
└── cli_help_20250103_120200.status
```

Each test generates two files:
- `.log` - Complete test output including build logs and test results
- `.status` - Test status (SUCCESS or FAILED with exit code)

## Available Test Modules

### TUI Rendering Tests (14 modules)
Tests that verify UI rendering with mocked state (`--screen-capture-only` mode):
- `single_station_master_coils`
- `single_station_master_discrete_inputs`
- `single_station_master_holding`
- `single_station_master_input`
- `single_station_slave_coils`
- `single_station_slave_discrete_inputs`
- `single_station_slave_holding`
- `single_station_slave_input`
- `multi_station_master_mixed_types`
- `multi_station_master_spaced_addresses`
- `multi_station_master_mixed_ids`
- `multi_station_slave_mixed_types`
- `multi_station_slave_spaced_addresses`
- `multi_station_slave_mixed_ids`

### TUI Drill-down Tests (14 modules)
Full integration tests with live TUI process (same modules as rendering):
- Same 14 modules as above, but runs in default mode (not `--screen-capture-only`)

### CLI Tests (5 modules)
- `help` - Test CLI help output
- `list_ports` - Test port listing
- `list_ports_json` - Test JSON port listing
- `list_ports_status` - Test port listing with status
- `modbus_basic_master_slave` - Test basic Modbus communication

## Examples

### Debug a failing test
```bash
# Run the specific failing test and examine output
./scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils --output-dir /tmp/debug

# View the log
cat /tmp/debug/tui-rendering_single_station_master_coils_*.log

# Keep container for interactive debugging
./scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils --keep-container
```

### Use custom Docker image
```bash
# Use a specific Rust version
./scripts/run_ci_locally.sh --docker-image rust:1.75

# Use Ubuntu-based image
./scripts/run_ci_locally.sh --docker-image ubuntu:22.04
```

### Run subset of tests
```bash
# Run only single station master tests
for module in single_station_master_{coils,discrete_inputs,holding,input}; do
    ./scripts/run_ci_locally.sh --workflow tui-rendering --module $module
done
```

## Debugging Failed Tests

### 1. Check the log file
```bash
# Find failed tests
grep -l "FAILED" ci-results/*.status

# View the complete log
cat ci-results/tui-rendering_single_station_master_coils_*.log
```

### 2. Keep container for interactive debugging
```bash
# Run with --keep-container
./scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils --keep-container

# Find the container name
docker ps -a | grep aoba_ci

# Attach to container
docker exec -it <container-name> /bin/bash
```

### 3. Run locally without Docker
If you have the same environment as CI:
```bash
cd /path/to/aoba

# TUI rendering test
cargo build --package aoba --package tui_e2e
./scripts/socat_init.sh
./target/debug/tui_e2e --module single_station_master_coils --screen-capture-only

# CLI test
cargo build --package aoba --package aoba_cli --package cli_e2e
./target/debug/cli_e2e --module help
```

## Troubleshooting

### Docker not found
```
Error: Docker is not installed or not in PATH
```
**Solution:** Install Docker from https://docs.docker.com/get-docker/

### Permission denied
```
permission denied while trying to connect to the Docker daemon socket
```
**Solution:** 
- Add your user to the docker group: `sudo usermod -aG docker $USER`
- Log out and back in, or run: `newgrp docker`

### Out of disk space
```
no space left on device
```
**Solution:** 
- Clean up Docker: `docker system prune -a`
- Remove old test results: `rm -rf ci-results/`

### Test fails but CI passes
This usually means environment differences. Check:
1. Docker image version matches CI (use `rust:latest` or specific version)
2. System dependencies are correctly installed in the Docker script
3. Virtual serial ports are set up correctly (socat)

## CI Workflow Mapping

This script replicates the following GitHub Actions workflows:

| GitHub Actions Job | Script Workflow | Modules |
|-------------------|-----------------|---------|
| `tui-e2e-rendering` | `tui-rendering` | 14 TUI modules with `--screen-capture-only` |
| `tui-e2e-drilldown` | `tui-drilldown` | 14 TUI modules (default mode) |
| `cli-e2e-basic` | `cli` | 5 CLI modules |

The script executes the same steps as CI:
1. Install system dependencies (libudev-dev, pkg-config, X11 libs, socat)
2. Build packages (`cargo build`)
3. Set up virtual serial ports (`socat_init.sh`)
4. Run tests with appropriate flags

## Performance Tips

### Speed up repeated runs
```bash
# Use a persistent Docker volume for cargo cache
docker volume create cargo-cache

# Modify script to use the volume (add to docker run command):
# -v cargo-cache:/usr/local/cargo/registry
```

### Parallel execution
The script runs tests sequentially. For parallel execution:
```bash
# Run different workflows in parallel
./scripts/run_ci_locally.sh --workflow tui-rendering --output-dir ci-results/rendering &
./scripts/run_ci_locally.sh --workflow tui-drilldown --output-dir ci-results/drilldown &
./scripts/run_ci_locally.sh --workflow cli --output-dir ci-results/cli &
wait
```

## Contributing

If you find issues or want to improve this script:
1. Test your changes locally
2. Ensure all workflows still work
3. Update this README if adding new features
4. Submit a PR with clear description

## License

Same as the Aoba project.
