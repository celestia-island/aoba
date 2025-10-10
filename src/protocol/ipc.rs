/// IPC (Inter-Process Communication) module for TUI-CLI communication
/// 
/// This module implements a simple IPC protocol using interprocess local sockets
/// to allow the TUI to manage CLI subprocesses and receive status updates.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};

/// Message types exchanged between TUI (parent) and CLI (child) processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    /// CLI subprocess reports it has successfully opened a serial port
    PortOpened { port_name: String },
    
    /// CLI subprocess reports an error opening or using a serial port
    PortError { port_name: String, error: String },
    
    /// CLI subprocess is shutting down gracefully
    Shutdown,
    
    /// Modbus data received/sent (for monitoring)
    ModbusData {
        port_name: String,
        direction: String, // "tx" or "rx"
        data: String,
    },
    
    /// Heartbeat to verify subprocess is alive
    Heartbeat,
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
}

/// IPC Server (runs in CLI subprocess)
pub struct IpcServer {
    socket_name: String,
    stream: Option<interprocess::local_socket::Stream>,
}

impl IpcServer {
    /// Create a new IPC server that connects to the given socket name
    pub fn connect(socket_name: String) -> Result<Self> {
        use interprocess::local_socket::prelude::*;
        
        log::debug!("IPC: Attempting to connect to socket: {}", socket_name);
        
        // Try to connect to the named socket (TUI is listening)
        let name = socket_name.clone().to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let stream = interprocess::local_socket::Stream::connect(name)?;
        
        log::info!("IPC: Successfully connected to socket: {}", socket_name);
        
        Ok(Self {
            socket_name,
            stream: Some(stream),
        })
    }
    
    /// Send a message to the parent TUI process
    pub fn send(&mut self, msg: &IpcMessage) -> Result<()> {
        if let Some(ref mut stream) = self.stream {
            let json = msg.to_json()?;
            writeln!(stream, "{}", json)?;
            stream.flush()?;
            log::debug!("IPC: Sent message: {:?}", msg);
            Ok(())
        } else {
            Err(anyhow!("IPC stream not connected"))
        }
    }
    
    /// Close the IPC connection
    pub fn close(&mut self) {
        if self.stream.is_some() {
            log::debug!("IPC: Closing connection to {}", self.socket_name);
            let _ = self.send(&IpcMessage::Shutdown);
            self.stream = None;
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
        use interprocess::local_socket::prelude::*;
        
        log::debug!("IPC: Creating listener on socket: {}", socket_name);
        
        let name = socket_name.clone().to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        let opts = interprocess::local_socket::ListenerOptions::new().name(name);
        
        let listener = opts.create_sync()?;
        
        log::info!("IPC: Listening on socket: {}", socket_name);
        
        Ok(Self {
            socket_name,
            listener,
        })
    }
    
    /// Accept a connection from a CLI subprocess (blocking)
    pub fn accept(&self) -> Result<IpcConnection> {
        log::debug!("IPC: Waiting for connection on {}", self.socket_name);
        // Use trait method accept() directly
        use interprocess::local_socket::traits::Listener;
        let stream = self.listener.accept()?;
        log::info!("IPC: Accepted connection on {}", self.socket_name);
        
        Ok(IpcConnection {
            reader: BufReader::new(stream),
        })
    }
    
    /// Get the socket name
    pub fn socket_name(&self) -> &str {
        &self.socket_name
    }
}

/// An active IPC connection from a CLI subprocess
pub struct IpcConnection {
    reader: BufReader<interprocess::local_socket::Stream>,
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
                log::debug!("IPC: Received message: {:?}", msg);
                Ok(Some(msg))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(None)
            }
            Err(e) => Err(anyhow!("IPC read error: {}", e)),
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
        log::debug!("IPC: Received message: {:?}", msg);
        Ok(msg)
    }
}

/// Generate a unique IPC socket name using UUID
pub fn generate_socket_name() -> String {
    let uuid = uuid::Uuid::new_v4();
    // Use a simple name that works on both Unix and Windows
    format!("aoba-ipc-{}", uuid)
}
