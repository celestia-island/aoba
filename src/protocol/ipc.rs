/// IPC (Inter-Process Communication) module for TUI-CLI communication
/// This module implements a simple IPC protocol using interprocess local sockets
/// to allow the TUI to manage CLI subprocesses and receive status updates.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};

use interprocess::local_socket::{prelude::*, Stream};

/// Message types exchanged between TUI (parent) and CLI (child) processes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// CLI subprocess reports it has successfully opened a serial port
    PortOpened {
        port_name: String,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// CLI subprocess reports an error opening or using a serial port
    PortError {
        port_name: String,
        error: String,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// CLI subprocess is shutting down gracefully
    Shutdown {
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Modbus data received/sent (for monitoring)
    ModbusData {
        port_name: String,
        direction: String, // "tx" or "rx"
        data: String,
        #[serde(default)]
        timestamp: Option<i64>,
        #[serde(default)]
        station_id: Option<u8>,
        #[serde(default)]
        register_mode: Option<String>,
        #[serde(default)]
        start_address: Option<u16>,
        #[serde(default)]
        quantity: Option<u16>,
        #[serde(default)]
        success: Option<bool>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        config_index: Option<u16>,
    },

    /// Heartbeat to verify subprocess is alive
    Heartbeat {
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Status report from CLI subprocess
    Status {
        port_name: String,
        status: String, // "running", "idle", "error", etc.
        details: Option<String>,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Log message from subprocess (for debugging)
    Log {
        level: String, // "debug", "info", "warn", "error"
        message: String,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Full station configuration update (TUI -> CLI or CLI -> TUI)
    /// This replaces all previous station configs with the new list
    StationsUpdate {
        /// Serialized stations data using postcard
        stations_data: Vec<u8>,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Request to lock state for synchronization (before sending update)
    StateLockRequest {
        requester: String, // "tui" or "cli"
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Acknowledge state lock (lock granted or released)
    StateLockAck {
        locked: bool,
        #[serde(default)]
        timestamp: Option<i64>,
    },

    /// Register write operation completed (success or failure)
    RegisterWriteComplete {
        port_name: String,
        station_id: u8,
        register_address: u16,
        register_value: u16,
        register_type: String, // "holding", "coil", etc.
        success: bool,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        timestamp: Option<i64>,
    },
}

impl IpcMessage {
    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize from JSON string
    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Get current Unix timestamp in seconds
    fn timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Create a PortOpened message with current timestamp
    pub fn port_opened(port_name: String) -> Self {
        Self::PortOpened {
            port_name,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a PortError message with current timestamp
    pub fn port_error(port_name: String, error: String) -> Self {
        Self::PortError {
            port_name,
            error,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a Shutdown message with current timestamp
    pub fn shutdown() -> Self {
        Self::Shutdown {
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a Heartbeat message with current timestamp
    pub fn heartbeat() -> Self {
        Self::Heartbeat {
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a Status message with current timestamp
    pub fn status(port_name: String, status: String, details: Option<String>) -> Self {
        Self::Status {
            port_name,
            status,
            details,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a Log message with current timestamp
    pub fn log(level: String, message: String) -> Self {
        Self::Log {
            level,
            message,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a StationsUpdate message with current timestamp
    pub fn stations_update(stations_data: Vec<u8>) -> Self {
        Self::StationsUpdate {
            stations_data,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a StateLockRequest message with current timestamp
    pub fn state_lock_request(requester: String) -> Self {
        Self::StateLockRequest {
            requester,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a StateLockAck message with current timestamp
    pub fn state_lock_ack(locked: bool) -> Self {
        Self::StateLockAck {
            locked,
            timestamp: Some(Self::timestamp()),
        }
    }

    /// Create a RegisterWriteComplete message with current timestamp
    pub fn register_write_complete(
        port_name: String,
        station_id: u8,
        register_address: u16,
        register_value: u16,
        register_type: String,
        success: bool,
        error: Option<String>,
    ) -> Self {
        Self::RegisterWriteComplete {
            port_name,
            station_id,
            register_address,
            register_value,
            register_type,
            success,
            error,
            timestamp: Some(Self::timestamp()),
        }
    }
}

/// IPC Server (runs in CLI subprocess)
pub struct IpcServer {
    socket_name: String,
    writer: Option<Stream>,
    reader: Option<BufReader<Stream>>,
}

impl IpcServer {
    /// Create a new IPC server that connects to the given socket name
    pub fn connect(socket_name: String) -> Result<Self> {
        log::debug!("IPC: Attempting to connect to socket: {socket_name}");

        // Try to connect to the named socket (TUI is listening)
        let name = socket_name
            .clone()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let stream = Stream::connect(name)?;

        log::info!("IPC: Successfully connected to socket: {socket_name}");

        // We'll use the same stream for both reading and writing
        // by wrapping it appropriately
        Ok(Self {
            socket_name,
            writer: Some(stream),
            reader: None,
        })
    }

    /// Send a message to the parent TUI process
    pub fn send(&mut self, msg: &IpcMessage) -> Result<()> {
        if let Some(ref mut stream) = self.writer {
            let json = msg.to_json()?;
            writeln!(stream, "{json}")?;
            stream.flush()?;
            if matches!(msg, IpcMessage::Heartbeat { .. }) {
                log::debug!("IPC: Sent heartbeat");
            } else {
                log::info!("IPC: Sent message: {msg:?}");
            }
            Ok(())
        } else {
            Err(anyhow!("IPC stream not connected"))
        }
    }

    /// Try to receive a message from the parent TUI process (non-blocking)
    /// Note: This is a simplified implementation that doesn't actually support non-blocking reads
    /// The CLI subprocess should poll this periodically
    pub fn try_recv(&mut self) -> Result<Option<IpcMessage>> {
        // For now, return None to indicate no message available
        // In a real implementation, we'd need to set the stream to non-blocking mode
        // or use a timeout-based approach
        // Since we can't easily do non-blocking with interprocess streams,
        // we'll skip this for now and handle messages in the main loop differently
        Ok(None)
    }

    /// Close the IPC connection
    pub fn close(&mut self) {
        if self.writer.is_some() {
            let socket_name = self.socket_name.clone();
            log::debug!("IPC: Closing connection to {socket_name}");
            let _ = self.send(&IpcMessage::shutdown());
            self.writer = None;
            self.reader = None;
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.close();
    }
}

/// IPC Client (runs in TUI parent process to receive messages from CLI children)
pub struct IpcClient {
    socket_name: String,
    listener: interprocess::local_socket::Listener,
}

impl IpcClient {
    /// Create a new IPC client that listens on a socket with the given name
    pub fn listen(socket_name: String) -> Result<Self> {
        let name = socket_name
            .clone()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let opts = interprocess::local_socket::ListenerOptions::new().name(name);

        let listener = opts.create_sync()?;

        log::info!("IPC: Listening on socket: {socket_name}");

        Ok(Self {
            socket_name,
            listener,
        })
    }

    /// Accept a connection from a CLI subprocess (blocking)
    pub fn accept(&self) -> Result<IpcConnection> {
        let socket_name = self.socket_name.clone();
        log::debug!("IPC: Waiting for connection on {socket_name}");

        let stream = self.listener.accept()?;
        stream.set_nonblocking(true)?;
        log::info!("IPC: Accepted connection on {socket_name}");

        Ok(IpcConnection {
            reader: BufReader::new(stream),
            _writer: None,
        })
    }

    /// Get the socket name
    pub fn socket_name(&self) -> &str {
        &self.socket_name
    }
}

/// An active IPC connection from a CLI subprocess
pub struct IpcConnection {
    reader: BufReader<Stream>,
    _writer: Option<Stream>,
}

impl IpcConnection {
    /// Try to receive a message (non-blocking)
    pub fn try_recv(&mut self) -> Result<Option<IpcMessage>> {
        let mut line = String::new();

        // Set non-blocking mode temporarily
        // Note: interprocess streams don't directly support non-blocking,
        // so we'll use a timeout approach in practice

        match self.reader.read_line(&mut line) {
            Ok(0) => {
                // EOF - connection closed
                Ok(None)
            }
            Ok(_) => {
                let msg = IpcMessage::from_json(line.trim())?;
                if matches!(&msg, IpcMessage::Heartbeat { .. }) {
                    log::debug!("IPC: Received heartbeat");
                } else {
                    log::info!("IPC: Received message: {msg:?}");
                }
                Ok(Some(msg))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(anyhow!("IPC read error: {e}")),
        }
    }

    /// Receive a message (blocking)
    pub fn recv(&mut self) -> Result<IpcMessage> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;

        if line.is_empty() {
            return Err(anyhow!("IPC connection closed"));
        }

        let msg = IpcMessage::from_json(line.trim())?;
        if matches!(&msg, IpcMessage::Heartbeat { .. }) {
            log::debug!("IPC: Received heartbeat");
        } else {
            log::info!("IPC: Received message: {msg:?}");
        }
        Ok(msg)
    }

    /// Send a message to the CLI subprocess
    /// Note: Bidirectional communication with interprocess streams is complex
    /// For now, this is not implemented. Consider using a separate connection
    /// from TUI to CLI for sending messages.
    pub fn send(&mut self, _msg: &IpcMessage) -> Result<()> {
        Err(anyhow!(
            "IPC Connection send not implemented - use separate channel"
        ))
    }
}

/// IPC Command Client (runs in TUI to send commands to CLI subprocess)
/// This is the reverse channel: TUI → CLI
pub struct IpcCommandClient {
    _socket_name: String,
    stream: Option<Stream>,
}

impl IpcCommandClient {
    /// Connect to a CLI subprocess's command channel
    pub fn connect(command_channel_name: String) -> Result<Self> {
        let name = command_channel_name
            .clone()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let stream = Stream::connect(name)?;

        log::info!("IPC CMD: Successfully connected to command channel: {command_channel_name}");

        Ok(Self {
            _socket_name: command_channel_name,
            stream: Some(stream),
        })
    }

    /// Send a command message to the CLI subprocess
    pub fn send(&mut self, msg: &IpcMessage) -> Result<()> {
        if let Some(ref mut stream) = self.stream {
            let json = msg.to_json()?;
            writeln!(stream, "{json}")?;
            stream.flush()?;
            log::info!("IPC CMD: Sent command to CLI: {msg:?}");
            Ok(())
        } else {
            Err(anyhow!("IPC command stream not connected"))
        }
    }

    /// Close the command connection
    pub fn close(&mut self) {
        if self.stream.is_some() {
            log::debug!("IPC CMD: Closing command connection");
            self.stream = None;
        }
    }
}

impl Drop for IpcCommandClient {
    fn drop(&mut self) {
        self.close();
    }
}

/// IPC Command Listener (runs in CLI subprocess to receive commands from TUI)
/// This listens on the reverse channel: TUI → CLI
pub struct IpcCommandListener {
    _socket_name: String,
    listener: Option<interprocess::local_socket::Listener>,
    connection: Option<IpcCommandConnection>,
}

impl IpcCommandListener {
    /// Create a command listener for the CLI subprocess
    pub fn listen(command_channel_name: String) -> Result<Self> {
        let name = command_channel_name
            .clone()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let opts = interprocess::local_socket::ListenerOptions::new().name(name);

        let listener = opts.create_sync()?;

        // Set listener to non-blocking mode to prevent blocking the main loop
        listener.set_nonblocking(interprocess::local_socket::ListenerNonblockingMode::Both)?;

        log::info!("IPC CMD: Listening for commands on: {command_channel_name} (non-blocking)");

        Ok(Self {
            _socket_name: command_channel_name,
            listener: Some(listener),
            connection: None,
        })
    }

    /// Accept a connection from TUI (blocking, call once)
    pub fn accept(&mut self) -> Result<()> {
        if let Some(ref listener) = self.listener {
            let stream = listener.accept()?;
            log::info!("IPC CMD: Accepted TUI connection");

            self.connection = Some(IpcCommandConnection {
                reader: BufReader::new(stream),
            });
            Ok(())
        } else {
            Err(anyhow!("Command listener not initialized"))
        }
    }

    /// Try to receive a command message (non-blocking if connection exists)
    pub fn try_recv(&mut self) -> Result<Option<IpcMessage>> {
        if let Some(ref mut conn) = self.connection {
            conn.try_recv()
        } else {
            Ok(None)
        }
    }
}

/// Connection for receiving commands from TUI
pub struct IpcCommandConnection {
    reader: BufReader<Stream>,
}

impl IpcCommandConnection {
    /// Try to receive a command (non-blocking)
    pub fn try_recv(&mut self) -> Result<Option<IpcMessage>> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                let msg = IpcMessage::from_json(line.trim())?;
                log::info!("IPC CMD: CLI received command from TUI: {msg:?}");
                Ok(Some(msg))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(anyhow!("IPC command read error: {e}")),
        }
    }
}

/// Generate a unique IPC socket name using UUID
pub fn generate_socket_name() -> String {
    let uuid = uuid::Uuid::new_v4();
    // Use a simple name that works on both Unix and Windows
    format!("aoba-ipc-{uuid}")
}

/// Generate the command channel name from the status channel name
/// Command channel is used for TUI → CLI communication (reverse direction)
pub fn get_command_channel_name(status_channel: &str) -> String {
    format!("{status_channel}-cmd")
}
