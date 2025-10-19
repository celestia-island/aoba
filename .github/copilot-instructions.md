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

### Code Import Conventions
- When creating or modifying Rust code, organize `use` statements according to the following guidelines for imports in Rust:
- Group same-package/module imports together as much as possible, merging them where suitable (e.g., several separate `use a::xxx` can be combined into `use a::{xxx, xxx::{xxx, self}};`).
- Group all `use` statements into three categories, with an empty line between each group, and ensure a final empty line after all `use` statements to separate from the main code.
- **Group 1**: External general-purpose libraries like `anyhow`, `serde`, `base64`, `chrono`, `tokio`, etc. This includes `std` (despite it being the standard library, it's considered an internal lib but should be placed in this first group).
- **Group 2**: External domain-specific libraries like `rmodbus`, `ratatui`, `tauri`, `axum`, `sea_orm`, etc., used for specialized business requirements.
- **Group 3**: Internal references starting with `crate::`, `super::`, or references to local Workspace crates via relative paths (except for patched external crates, which are treated as emergency patches rather than internal).
- After organizing the imports, always run `cargo fmt` to ensure intra-group ordering follows Rust compiler formatting recommendations.
- `use` statements must all be placed at the start of the file, without exceptions, except in isolated modules like `mod test { ... }` for unit tests.
- Avoid abbreviations in imported identifiers unless the original name is particularly long; ensure readability (e.g., do not shorten `KeyCode` to `KC`).
- For frequently used modules like `types`, `models` where direct imports risk naming conflicts, import the module itself and reference items through it (e.g., start with `use xxx::types::{self, yyy::zzz::SpecialEnum}` and use `types::xxx` in code instead of importing individual items).

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
- Control test behavior through command-line arguments (e.g., `--loop-count`, `--debug`, `--test0`)

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

## TUI Keyboard Shortcuts

### Entry Page (Main Menu)
- `↑ / k`: Move cursor up to previous port
- `↓ / j`: Move cursor down to next port
- `PageUp`: Jump to first port in the list
- `PageDown`: Jump to last item (About)
- `Enter`: Enter selected port's configuration panel or execute selected action
- `q`: Quit the application
- `Esc`: (Context-dependent)

### Config Panel (Port Configuration)
- `↑ / k`: Move cursor up to previous option
- `↓ / j`: Move cursor down to next option
- `PageUp`: Jump to first option (Enable Port)
- `PageDown`: Jump to last option (Stop Bits)
- `Enter`: Edit selected configuration field or toggle boolean options
- `Esc`: Exit editing mode or return to entry page
- `← / h`: Navigate left in editing mode or move to previous option
- `→ / l`: Navigate right in editing mode or move to next option

### Modbus Dashboard (Modbus Configuration Panel)
- `↑ / k`: Move cursor up within current section
- `↓ / j`: Move cursor down within current section
- `← / h`: Move cursor left (for register table navigation)
- `→ / l`: Move cursor right (for register table navigation)
- `PageUp`: Jump to previous station group (first item of previous station)
- `PageDown`: Jump to next station group (first item of next station)
- `Ctrl+PageUp`: Jump to first group (Create Station / AddLine)
- `Ctrl+PageDown`: Jump to last group (first item of last station if exists)
- `Enter`: Edit selected field or create new station
- `Ctrl+S`: Save configuration and enable/restart port
- `Esc`: Exit editing mode or return to config panel
- `d`: Delete current master/slave station (when on station row)

### Log Panel (Communication Logs)
- `↑`: Scroll up one line in log view
- `↓`: Scroll down one line in log view
- `PageUp`: Scroll up one page (approximately 20 lines)
- `PageDown`: Scroll down one page (approximately 20 lines)
- `Ctrl+PageUp`: Jump to first log item
- `Ctrl+PageDown`: Jump to last log item
- `k`: Scroll up one line (vim-style)
- `j`: Scroll down one line (vim-style)
- `Enter`: Toggle input mode between ASCII and Hex
- `Esc / h`: Return to config panel
- `v`: Toggle auto-follow mode (automatically scroll to newest logs)
- `c`: Clear all logs

### About Page
- `↑ / k`: Scroll up one line
- `↓ / j`: Scroll down one line
- `PageUp`: Scroll up one page (10 lines)
- `PageDown`: Scroll down one page (10 lines)
- `Ctrl+PageUp`: Jump to top of content
- `Ctrl+PageDown`: Jump to bottom of content
- `Esc`: Return to entry page

## TUI Status Indicators

The TUI displays a communication status indicator in the top-right corner of the title bar when viewing a port's configuration or sub-pages. The indicator shows the current operational state of the port:

### Status Indicator States

1. **Not Started** (Red ×)
   - Port is not running or has been stopped
   - Configuration has not been applied
   - No communication is active

2. **Starting** (Yellow spinner animation ⟳)
   - Port is initializing
   - Serial port is being opened
   - Modbus runtime is starting up

3. **Running** (Green ● solid dot)
   - Port is active and running normally
   - Configuration is up-to-date and applied
   - Communication is operational

4. **Running with Changes** (Yellow ○ hollow circle)
   - Port is running but configuration has been modified
   - Changes have not been saved/applied yet
   - Press `Ctrl+S` to save and apply changes

5. **Saving** (Green spinner animation ⟳)
   - Configuration is being saved
   - Applies to slave modes (02/04 discrete inputs/input registers)
   - Data is being written to persistent storage

6. **Syncing** (Yellow spinner animation ⟳)
   - Configuration is being synchronized from CLI subprocess
   - IPC communication in progress
   - Port state is being updated

7. **Applied Success** (Green ✔ checkmark)
   - Configuration was successfully saved and applied
   - Displayed for 3 seconds after successful operation
   - Automatically transitions to Running state

## CLI and E2E Test Command-Line Arguments

### CLI Binary (`aoba`)

Main command-line arguments for the aoba binary:

```
Options:
  -t, --tui                            Force TUI mode
  -l, --list-ports                     List all available serial ports and exit
  -j, --json                           Output one-shot results in JSON format
  -c, --config <FILE>                  Load configuration from JSON file
      --config-json <JSON>             Load configuration from JSON string
      --slave-listen <PORT>            Modbus slave: listen for requests and respond once, then exit
      --slave-listen-persist <PORT>    Modbus slave: continuously listen for requests and respond (JSONL output)
      --slave-poll <PORT>              Modbus slave: send request and wait for response once, then exit (acts as client)
      --slave-poll-persist <PORT>      Modbus slave: continuously poll for data and output responses (JSONL output)
      --master-provide <PORT>          Modbus master: provide data once and respond to requests, then exit
      --master-provide-persist <PORT>  Modbus master: continuously provide data and respond to requests (JSONL output)
      --station-id <ID>                Modbus station ID (slave address) [default: 1]
      --register-address <ADDR>        Starting register address [default: 0]
      --register-length <LEN>          Number of registers [default: 10]
      --register-mode <MODE>           Register type: holding, input, coils, discrete [default: holding]
      --data-source <SOURCE>           Data source for master mode: file:<path> or pipe:<name>
      --output <OUTPUT>                Output destination for slave mode: file:<path> or pipe:<name> (default: stdout)
      --baud-rate <BAUD>               Serial port baud rate [default: 9600]
      --debounce-seconds <SECONDS>     Debounce window for duplicate JSON output in seconds [default: 1.0]
      --ipc-channel <UUID>             IPC channel UUID for TUI communication (internal use)
      --serial-daemon <PORT>           Run as serial port daemon (IPC mode)
      --modbus-daemon <IPC_CHANNEL>    Run as Modbus daemon (IPC mode)
  -h, --help                           Print help
```

### TUI E2E Test Suite (`tui_e2e`)

Test execution and configuration arguments:

```
Options:
  --port1 <PORT1>  Virtual serial port 1 path [default: /tmp/vcom1]
  --port2 <PORT2>  Virtual serial port 2 path [default: /tmp/vcom2]
  --debug          Enable debug mode (show debug breakpoints and additional logging)
  --test0          Run only test 0: CLI port release verification
  --test1          Run only test 1: TUI Slave + CLI Master
  --test2          Run only test 2: TUI Master + CLI Slave
  --test3          Run only test 3: Multiple TUI Masters
  --test4          Run only test 4: Multiple TUI Slaves
  -h, --help       Print help
```

**Usage Examples:**
```bash
# Run all tests (default)
cargo run --package tui_e2e

# Run specific test
cargo run --package tui_e2e -- --test3

# Use custom ports with debug mode
cargo run --package tui_e2e -- --port1 /dev/ttyUSB0 --port2 /dev/ttyUSB1 --debug

# Run multiple specific tests
cargo run --package tui_e2e -- --test1 --test3
```

### CLI E2E Test Suite (`cli_e2e`)

Test execution and configuration arguments:

```
Options:
  --port1 <PORT1>            Virtual serial port 1 path [default: /tmp/vcom1]
  --port2 <PORT2>            Virtual serial port 2 path [default: /tmp/vcom2]
  --debug                    Enable debug mode (show debug breakpoints and additional logging)
  --loop-count <LOOP_COUNT>  Number of test loop iterations [default: 1]
  --basic                    Run only basic CLI tests (help, list-ports)
  --modbus-cli               Run only Modbus CLI tests (temp/persist modes)
  --e2e                      Run only E2E tests with virtual ports
  -h, --help                 Print help
```

**Usage Examples:**
```bash
# Run all tests (default)
cargo run --package cli_e2e

# Run only basic CLI tests
cargo run --package cli_e2e -- --basic

# Run with loop mode and custom ports
cargo run --package cli_e2e -- --loop-count 5 --port1 /tmp/vcom1 --port2 /tmp/vcom2

# Run only E2E tests with debug
cargo run --package cli_e2e -- --e2e --debug

# Combine multiple test categories
cargo run --package cli_e2e -- --basic --modbus-cli
```

### Notes on E2E Testing

- **Default Behavior**: If no specific test flags are provided, all tests will run
- **Port Configuration**: Custom serial port paths can be specified to avoid conflicts
- **Debug Mode**: Enables additional logging and debug breakpoints for troubleshooting
- **Selective Execution**: Choose specific test categories or individual tests to speed up development
- **Loop Testing**: CLI E2E supports running tests multiple times to check for stability
- **Environment Variables**: Previously used environment variables (`AOBATEST_PORT1`, `AOBATEST_PORT2`, `TEST_LOOP`, `DEBUG_MODE`) have been replaced with command-line arguments for better clarity and flexibility
