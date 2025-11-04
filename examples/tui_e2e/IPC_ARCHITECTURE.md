# IPC Architecture for TUI E2E Testing

## Overview

This document describes the IPC-based architecture for TUI E2E testing, which completely eliminates the need for `expectrl` and `vt100` by using direct process communication.

## Architecture

### High-Level Design

```
┌─────────────┐                    ┌──────────────┐
│   TUI E2E   │                    │  TUI Process │
│   Test      │                    │              │
│             │                    │              │
│  IpcSender  │ ◄──── IPC ─────► │ IpcReceiver  │
│             │    (Unix Socket)   │              │
└─────────────┘                    └──────────────┘
       │                                   │
       │ 1. KeyPress                       │ 2. Process Input
       │ 2. RequestScreen                  │ 3. Render Frame
       │ 3. Receive ScreenContent          │ 4. Send Frame
       └───────────────────────────────────┘
```

### Communication Flow

**DrillDown Mode**:
1. E2E test spawns TUI with `--debug-ci ${IPC_ID}`
2. TUI starts IPC receiver listening on Unix socket
3. E2E sends `KeyPress` messages via IPC
4. TUI processes keyboard input through normal handler
5. E2E sends `RequestScreen` to get current state
6. TUI renders to TestBackend and sends `ScreenContent` back
7. E2E verifies screen content against TOML workflow expectations

**Screen Capture Mode**:
- No TUI process spawned
- Uses TestBackend directly in E2E process
- Manipulates mock state for testing

## Message Protocol

### E2E → TUI Messages

```rust
pub enum E2EToTuiMessage {
    /// Simulate a key press
    KeyPress { key: String },
    /// Simulate character input (typing)
    CharInput { ch: char },
    /// Request current screen rendering
    RequestScreen,
    /// Shutdown the TUI
    Shutdown,
}
```

### TUI → E2E Messages

```rust
pub enum TuiToE2EMessage {
    /// Screen content as rendered text
    ScreenContent { content: String, width: u16, height: u16 },
    /// Acknowledgment of key press
    KeyProcessed,
    /// TUI is ready
    Ready,
    /// Error occurred
    Error { message: String },
}
```

## Implementation Status

### ✅ Completed

- [x] IPC message type definitions
- [x] IPC channel ID generation system
- [x] `IpcSender` and `IpcReceiver` structure
- [x] `ExecutionContext` refactored to support IPC
- [x] `spawn_tui_with_ipc` framework
- [x] Tokio features added for networking
- [x] DrillDown/ScreenCapture mode separation

### 🚧 In Progress

#### IPC Transport Layer

**File**: `examples/tui_e2e/src/ipc.rs`

TODO:
- [ ] Implement Unix domain socket creation
- [ ] Implement async `send()` with JSON serialization
- [ ] Implement async `receive()` with JSON deserialization
- [ ] Add connection handling and retry logic
- [ ] Add timeout handling
- [ ] Clean up socket files on shutdown

**Example Implementation**:
```rust
use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

impl IpcSender {
    pub async fn send(&mut self, message: E2EToTuiMessage) -> Result<()> {
        let (to_tui, _) = self.channel_id.paths();
        let mut stream = UnixStream::connect(to_tui).await?;
        let json = serde_json::to_string(&message)?;
        stream.write_all(json.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        Ok(())
    }
}
```

#### TUI Integration

**File**: `packages/cli/src/lib.rs`

TODO:
- [ ] Add `--debug-ci` argument to CLI parser:
  ```rust
  Arg::new("debug-ci")
      .long("debug-ci")
      .value_name("IPC_ID")
      .help("Enable CI debug mode with IPC channel ID")
  ```

**File**: `packages/tui/src/tui/mod.rs`

TODO:
- [ ] Check for `--debug-ci` flag in `start()`
- [ ] Create `IpcReceiver` if flag present
- [ ] Start IPC listener loop in separate task
- [ ] Connect IPC messages to input handler
- [ ] Render to TestBackend on `RequestScreen`
- [ ] Send `ScreenContent` via IPC

**Example Integration**:
```rust
pub fn start(matches: &ArgMatches) -> Result<()> {
    // Check for CI debug mode
    if let Some(ipc_id) = matches.get_one::<String>("debug-ci") {
        return start_with_ipc(ipc_id)?;
    }
    // Normal TUI start
    start_normal()?;
}

fn start_with_ipc(ipc_id: &str) -> Result<()> {
    let receiver = IpcReceiver::new(IpcChannelId(ipc_id.to_string()))?;
    
    // Start IPC listener
    tokio::spawn(async move {
        loop {
            match receiver.receive().await {
                Ok(E2EToTuiMessage::KeyPress { key }) => {
                    // Send to input handler
                },
                Ok(E2EToTuiMessage::RequestScreen) => {
                    // Render and send back
                },
                _ => {}
            }
        }
    });
    
    // Start normal rendering loop
    // ...
}
```

#### E2E Test Integration

**File**: `examples/tui_e2e/src/executor.rs`

TODO:
- [ ] Implement `spawn_tui_with_ipc` to actually spawn process:
  ```rust
  let mut cmd = tokio::process::Command::new("cargo");
  cmd.args(&["run", "--package", "aoba", "--", "--tui", "--debug-ci", &channel_id.0]);
  cmd.spawn()?;
  ```
- [ ] Update `simulate_key_input` to use IPC:
  ```rust
  if let Some(sender) = &mut ctx.ipc_sender {
      sender.send(E2EToTuiMessage::KeyPress { key: key.to_string() }).await?;
  }
  ```
- [ ] Update screen verification to use IPC:
  ```rust
  if let Some(sender) = &mut ctx.ipc_sender {
      sender.send(E2EToTuiMessage::RequestScreen).await?;
      let content = sender.receive().await?;
      // Verify content
  }
  ```

**File**: `examples/tui_e2e/src/renderer.rs`

TODO:
- [ ] Add `render_tui_via_ipc` function for DrillDown mode
- [ ] Keep `render_tui_to_string` for ScreenCapture mode

### 📦 Dependencies

TODO:
- [ ] Remove `insta` dependency (move to TOML verification)
- [ ] Confirm `tokio` features sufficient
- [ ] May need `tokio-util` for framing

## Testing Strategy

### Unit Tests

- [ ] Test IPC message serialization/deserialization
- [ ] Test Unix socket creation and cleanup
- [ ] Test channel ID generation

### Integration Tests

- [ ] Test E2E → TUI key press
- [ ] Test TUI → E2E screen content
- [ ] Test full workflow execution
- [ ] Test error handling and timeouts

### End-to-End Tests

- [ ] Run existing TOML workflows with IPC
- [ ] Verify screen content matches expectations
- [ ] Test both DrillDown and ScreenCapture modes

## Migration Path

1. **Phase 1** (Current): IPC infrastructure ✅
   - Message types defined
   - Architecture in place
   - Dependencies ready

2. **Phase 2**: IPC Transport
   - Implement Unix socket communication
   - Test message passing

3. **Phase 3**: TUI Integration
   - Add `--debug-ci` parameter
   - Integrate IPC receiver
   - Connect to input handler

4. **Phase 4**: E2E Integration
   - Update simulate functions
   - Update render functions
   - Test workflows

5. **Phase 5**: Cleanup
   - Remove `insta` dependency
   - Remove `expectrl` completely
   - Update documentation

## Benefits

- ✅ No terminal emulation needed
- ✅ Direct process communication
- ✅ Real TUI testing (not mocked)
- ✅ Clear separation of concerns
- ✅ Both testing modes supported
- ✅ Complete elimination of `expectrl` and `vt100`

## Future Enhancements

- Support for Windows named pipes
- IPC performance monitoring
- Recording/playback of IPC sessions
- Visual debugging of IPC communication
