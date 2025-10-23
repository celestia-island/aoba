// Daemon module disabled - TUI now uses CLI subprocesses exclusively via IPC
// The daemon expected Arc<RwLock<PortData>> but TUI now stores PortData directly
// TODO: Refactor daemon to work with new structure or remove it entirely
// pub mod daemon;

// Runtime module disabled - depends on daemon module
// TUI now uses CLI subprocesses instead of direct runtime handles
// pub mod runtime;

pub mod ipc;
pub mod modbus;
pub mod status;
pub mod tty;
