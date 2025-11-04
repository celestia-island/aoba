# IPC Communication Implementation for TUI E2E Testing

## Overview

This document describes the implementation of IPC (Inter-Process Communication) for TUI E2E testing, which enables direct communication between E2E tests and the TUI process without terminal emulation.

## Architecture

### Components

1. **ci_utils Package** (`packages/ci_utils`)
   - Shared IPC message types and utilities
   - Used by both TUI E2E tests and TUI process
   - Provides Unix socket-based communication

2. **IPC Messages**
   - `E2EToTuiMessage`: Messages sent from E2E tests to TUI
     - `KeyPress { key }`: Simulate keyboard input
     - `CharInput { ch }`: Simulate character typing
     - `RequestScreen`: Request current screen content
     - `Shutdown`: Shutdown the TUI process
   
   - `TuiToE2EMessage`: Messages sent from TUI to E2E tests
     - `ScreenContent { content, width, height }`: Rendered screen content
     - `KeyProcessed`: Acknowledgment of key press
     - `Ready`: TUI is ready for testing
     - `Error { message }`: Error occurred

3. **IPC Transport**
   - Uses Unix domain sockets for communication
   - Two sockets per channel: `to_tui` and `from_tui`
   - JSON serialization for messages
   - Timeout and retry logic for robustness

## Usage

### Running TUI in IPC Mode

The TUI process accepts a `--debug-ci` flag with a channel ID:

```bash
cargo run --package aoba -- --tui --debug-ci my_channel_id
```

This starts the TUI in IPC mode:
- Creates Unix socket listeners
- Waits for E2E test to connect
- Processes keyboard events from IPC
- Sends screen content back via IPC

### E2E Test Workflow

The TUI E2E framework supports two execution modes:

1. **ScreenCaptureOnly Mode** (`--screen-capture-only`)
   - Uses TestBackend directly without spawning TUI process
   - Manipulates mock state for testing
   - Fast but limited to rendering tests

2. **DrillDown Mode** (default)
   - Spawns real TUI process with IPC
   - Sends keyboard events via IPC
   - Receives screen content via IPC
   - Tests full interactive workflows

Example test invocation:

```bash
# DrillDown mode (with IPC)
cargo run --package tui_e2e -- --module ipc_communication_test

# Screen capture mode (without IPC)
cargo run --package tui_e2e -- --module ipc_communication_test --screen-capture-only
```

## Implementation Details

### Spawning TUI Process

In `examples/tui_e2e/src/executor.rs`:

```rust
async fn spawn_tui_with_ipc(ctx: &mut ExecutionContext, workflow_id: &str) -> Result<()> {
    // Generate unique channel ID
    let channel_id = IpcChannelId(format!("{}_{}", workflow_id, std::process::id()));
    
    // Spawn TUI process
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.args(&["run", "--package", "aoba", "--", "--tui", "--debug-ci", &channel_id.0]);
    let child = cmd.spawn()?;
    
    // Connect IPC sender
    let sender = IpcSender::new(channel_id.clone()).await?;
    ctx.ipc_sender = Some(sender);
    
    Ok(())
}
```

### Sending Keyboard Events

```rust
async fn simulate_key_input(ctx: &mut ExecutionContext, key: &str) -> Result<()> {
    if let Some(sender) = ctx.ipc_sender.as_mut() {
        sender.send(E2EToTuiMessage::KeyPress { key: key.to_string() }).await?;
    }
    Ok(())
}
```

### Receiving Screen Content

```rust
pub async fn render_tui_via_ipc(sender: &mut IpcSender) -> Result<(String, u16, u16)> {
    // Request screen content
    sender.send(E2EToTuiMessage::RequestScreen).await?;
    
    // Wait for response
    match sender.receive().await? {
        TuiToE2EMessage::ScreenContent { content, width, height } => {
            Ok((content, width, height))
        }
        _ => Err(anyhow!("Unexpected response")),
    }
}
```

### TUI IPC Handler

In `packages/tui/src/tui/mod.rs`:

```rust
fn start_with_ipc(channel_id: &str) -> Result<()> {
    // Initialize TUI state
    let app = Arc::new(RwLock::new(Status::default()));
    self::status::init_status(app.clone())?;
    
    // Create IPC receiver
    let mut receiver = IpcReceiver::new(IpcChannelId(channel_id.to_string())).await?;
    
    // Send Ready message
    receiver.send(TuiToE2EMessage::Ready).await?;
    
    // Main IPC loop
    loop {
        match receiver.receive().await? {
            E2EToTuiMessage::KeyPress { key } => {
                // Process keyboard event
                let event = parse_key_string(&key)?;
                input::handle_event(event, &bus)?;
            }
            E2EToTuiMessage::RequestScreen => {
                // Render to TestBackend
                terminal.draw(|frame| render_ui(frame))?;
                let content = buffer_to_string(terminal.backend().buffer());
                receiver.send(TuiToE2EMessage::ScreenContent { content, ... }).await?;
            }
            E2EToTuiMessage::Shutdown => break,
            _ => {}
        }
    }
    
    Ok(())
}
```

## Error Handling

### Timeouts
- Connection timeout: 10 seconds
- IO timeout: 5 seconds
- Retry interval: 100ms

### Error Cases
1. **Socket creation failure**: Check file permissions and `/tmp` availability
2. **Connection timeout**: TUI process may not have started
3. **Message deserialization**: Check message format compatibility
4. **IPC connection closed**: TUI process exited unexpectedly

## Testing

### Unit Tests

The ci_utils package includes unit tests for IPC functionality:

```bash
cargo test --package aoba_ci_utils
```

Tests cover:
- Message serialization/deserialization
- IPC roundtrip communication
- KeyPress, CharInput, RequestScreen, Shutdown messages

### Integration Tests

The tui_e2e package includes integration tests:

```bash
cargo run --package tui_e2e -- --module ipc_communication_test
```

This tests:
- IPC connection establishment
- Keyboard event delivery
- Screen content retrieval
- Clean shutdown

## Future Enhancements

1. **Performance Monitoring**
   - Add metrics for IPC latency
   - Track message throughput

2. **Windows Support**
   - Implement named pipes for Windows
   - Platform-agnostic IPC abstraction

3. **Recording/Playback**
   - Record IPC sessions for debugging
   - Replay sessions for regression testing

4. **Visual Debugging**
   - Real-time IPC message visualization
   - Screen diff viewer for test failures
