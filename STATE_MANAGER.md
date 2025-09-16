# StateManager Architecture Documentation

## Overview

The StateManager has been successfully implemented to replace the direct locking approach with a Flume-based message queue system for controlling write operations to the application state.

## Key Components

### 1. StateManager (`src/protocol/status/state_manager.rs`)

- **StateManager**: Main struct that provides the interface for reading and writing state
- **StateWriteMessage**: Message type containing closures for state modifications
- **run_state_writer_thread**: Dedicated thread function that processes write messages

### 2. Architecture Benefits

#### Before (Direct Locking):
```rust
let _ = write_status(&app, |s| {
    s.temporarily.busy.busy = true;
    Ok(())
});
```

#### After (Message Queue):
```rust
let _ = state_mgr.write_status_async(|s| {
    s.temporarily.busy.busy = true;
    Ok(())
});
```

### 3. Key Features

- **Non-blocking writes**: All write operations are queued and processed by a dedicated thread
- **Closure preservation**: Maintains the original closure-based interface for easy migration
- **Compatibility**: Provides legacy `write_status` method that maintains existing API
- **Thread safety**: Eliminates lock contention by serializing all writes through the message queue

### 4. Thread Architecture

The application now uses a 4-thread architecture:

1. **State Writer Thread**: Processes all state write operations sequentially
2. **Core Processing Thread**: Handles business logic and UI communication  
3. **Input Thread**: Processes keyboard and mouse input
4. **UI Rendering Thread**: Handles terminal rendering

### 5. API Reference

#### StateManager Methods

- `read_status<R, F>(&self, f: F) -> Result<R>`: Read state using a closure
- `write_status_async<F>(&self, f: F) -> Result<()>`: Queue async write operation
- `write_status_sync<F>(&self, f: F) -> Result<()>`: Queue write and wait for completion
- `write_status<R, F>(&self, mut f: F) -> Result<R>`: Legacy compatibility method

#### Message Types

- `StateWriteMessage::new(closure)`: Create async message
- `StateWriteMessage::with_result(closure)`: Create message that returns result

### 6. Testing

Unit tests are provided in `src/protocol/status/test.rs` that validate:
- Basic read/write operations
- Async and sync write modes
- Closure interface compatibility
- Thread communication

### 7. Performance Benefits

- **Reduced lock contention**: No more competing for write locks
- **Better concurrency**: Read operations can happen while writes are queued
- **Predictable latency**: Write operations are processed in FIFO order
- **Scalability**: Easy to add write prioritization or batching in the future

## Migration Guide

### Simple Write Operations
```rust
// Old
write_status(&app, |s| { s.field = value; Ok(()) });

// New  
state_mgr.write_status_async(|s| { s.field = value; Ok(()) });
```

### Write Operations with Results
```rust
// Old
let result = write_status(&app, |s| { Ok(some_computation(s)) })?;

// New
let result = state_mgr.write_status(|s| { Ok(some_computation(s)) })?;
```

The StateManager successfully eliminates temporary locking for write operations while preserving the closure-based interface and maintaining full backward compatibility.