// Daemon module disabled - TUI now uses CLI subprocesses exclusively via IPC
// The daemon expected Arc<RwLock<PortData>> but TUI now stores PortData directly
// TODO: Refactor daemon to work with new structure or remove it entirely
// pub mod daemon;

pub mod ipc;
pub mod modbus;
pub mod runtime;
pub mod status;
pub mod tty;
