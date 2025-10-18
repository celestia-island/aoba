# Aoba Copilot Instructions

## Project Overview

Aoba is a multi-protocol debugging and simulation CLI tool that primarily supports Modbus RTU, MQTT, TCP, and other protocols. The project is written in Rust and features a dual architecture consisting of CLI (Command Line Interface) and TUI (Terminal User Interface) components.

### Project Structure

#### CLI Component (`src/cli/`)
- **Core Functionality**: Automated protocol debugging and simulation
- **Main Modules**:
  - `actions.rs` - CLI action handling and IPC communication
  - `modbus/` - Modbus master/slave implementation
  - `config.rs` - Configuration file processing
- **Use Cases**: Automated testing, scripted operations, CI/CD integration

#### TUI Component (`src/tui/`)
- **Core Functionality**: Interactive terminal user interface
- **Main Modules**:
  - `ui/` - User interface rendering and components
  - `input.rs` - Keyboard input handling
  - `subprocess.rs` - CLI subprocess management
  - `persistence/` - Configuration persistence
- **Use Cases**: Real-time debugging, interactive operations, monitoring

#### Protocol Layer (`src/protocol/`)
- **Core Functionality**: Protocol abstraction and implementation
- **Main Modules**:
  - `modbus/` - Modbus protocol implementation
  - `tty/` - Serial communication
  - `ipc/` - Inter-process communication
  - `runtime/` - Runtime management
  - `status/` - Status management

#### Tests and Examples (`examples/`)
- `cli_e2e/` - CLI end-to-end tests
- `tui_e2e/` - TUI end-to-end tests
- `ci_utils/` - CI utility tools

## Development Guidelines

### Coding Standards
- Use `cargo fmt` to format code
- Use `cargo clippy` for static analysis
- Use `cargo check` to verify compilation
- Follow Rust best practices and ownership semantics

### Testing Process

To ensure smooth testing, the following requirements must be followed:

Please strictly follow the cycle of "run tests and output to log files → batch summarize logs → generate improvement plans based on logs → execute plans to modify code". Do not stop iterative attempts until completion.

#### Logging and Debugging Facilities
When needed, make extensive use of existing logging facilities and smoke testing (i.e., using the `--debug` parameter with `CursorAction::DebugBreakpoint` for temporary screenshots). Consider incorporating these elements mentioned as steps in improvement plans, and feel free to expand them at any time.

#### Log Analysis
Actively compare simulation terminal logs with CLI communication logs to identify any missing logic that needs to be supplemented. This establishes cause-and-effect relationships between steps, ensuring each step runs successfully according to the process I outlined at the beginning—not only fixing the tests themselves, but also addressing potential errors in the code being tested.

#### Long-term Testing
When conducting long-term tests, if you are attempting to continuously wait for terminal output and read log files, please redirect log content to both terminal and files simultaneously. This allows me to observe in real-time whether the program is hanging, facilitating emergency stops.

#### Port Management
When needed, execute `script/socat_init.sh` to reset ports before each formal test startup.

#### Code Quality Checks
Before completion, execute `cargo check`, `cargo clippy`, and `cargo fmt` in sequence. Pay attention to examples as well, and confirm that completely unused functions, parameters, struct keys, etc. can be removed.

#### Development Environment Instructions
If your work environment is on a user development machine rather than a CI environment (copilot agent), please use commands like `wsl bash -lc 'cargo run --package tui_e2e...'` to start debugging. Generally, avoid using PowerShell.

### Debugging Techniques

#### Virtual Serial Port Testing
- Use `scripts/socat_init.sh` to create virtual serial port pairs
- Supports `--debug` mode for screenshot debugging
- Control test behavior through environment variables (e.g., `TEST_LOOP`, `SKIP_BASIC_TESTS`)

#### IPC Debugging
- CLI and TUI communicate through IPC channels
- Use `--ipc-channel` parameter to establish connections
- Debug complex state issues through state snapshots

### Common Issues

#### Port Conflicts
- Use `scripts/socat_init.sh` to reset ports
- Check if processes properly release port resources
- Pay attention to virtual serial port permission issues

#### Compilation Issues
- Ensure all dependencies are correctly installed
- Check compilation status of examples directory
- Use `cargo clean` to clear cache

#### Test Failures
- Check if virtual serial ports are created correctly
- Verify that log output meets expectations
- Use debug mode to locate issues

## Architecture Decisions

### Dual Interface Design
- CLI: Automation and batch processing scenarios
- TUI: Interactive debugging and monitoring
- Achieve efficient collaboration through IPC

### Modular Protocol Support
- Abstract protocol interfaces for easy extension
- Unified state management system
- Flexible configuration mechanism

### Asynchronous Architecture
- Uses Tokio runtime
- Non-blocking I/O operations
- Efficient resource utilization

## Future Plans

- [ ] Support more protocols (MQTT, TCP, etc.)
- [ ] Enhance GUI (Maybe WebUI) interface
- [ ] Improve test coverage
- [ ] Optimize performance and resource usage
