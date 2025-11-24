//! IPC communication utilities for TUI E2E testing
//!
//! This module provides cross-platform IPC for communication between
//! TUI E2E tests and the TUI process using the `interprocess` library.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::{io::ErrorKind, time::Instant};
use tokio::time::sleep;

use interprocess::local_socket::{
    prelude::*,
    {GenericFilePath, GenericNamespaced, ListenerOptions},
};

use super::{
    E2EToTuiMessage, IpcChannelId, TuiToE2EMessage, CONNECT_RETRY_INTERVAL, CONNECT_TIMEOUT,
    IO_TIMEOUT,
};
use crate::core::task_manager::spawn_blocking_task;

/// Helper struct for bidirectional IPC pipe communication
struct Pipe {
    conn: LocalSocketStream,
    buffer: Vec<u8>,
}

impl Pipe {
    /// Create new pipe from stream
    fn new(conn: LocalSocketStream) -> Self {
        let buffer = Vec::with_capacity(1024);
        Pipe { conn, buffer }
    }

    /// Write serialized data using postcard encoding with chunked transfer
    fn do_write<T: Serialize>(&mut self, data: &T) -> Result<()> {
        use std::io::Write;

        let data = postcard::to_allocvec(data)?;

        let len = data.len();
        let chunks_len = len / 1024 + if len % 1024 == 0 { 0 } else { 1 };

        // Send length metadata
        let metadata = postcard::to_allocvec(&(len, chunks_len))?;
        self.conn.write_all(&metadata)?;
        self.conn.flush()?;

        // Send data in chunks
        for chunk in data.chunks(1024) {
            self.conn.write_all(chunk)?;
            self.conn.flush()?;
        }

        // Send ACK
        self.conn.write_all(b"ACK")?;
        self.conn.flush()?;

        Ok(())
    }

    /// Read serialized data using postcard encoding with chunked transfer
    fn do_read<T: for<'de> Deserialize<'de>>(&mut self) -> Result<T> {
        use std::io::Read;

        // Read length metadata
        self.buffer.resize(1024, 0);
        self.conn.read_exact(&mut self.buffer[..1024])?;
        let (len, chunks_len): (usize, usize) = postcard::from_bytes(&self.buffer)?;
        self.buffer.clear();

        // Read data chunks
        let mut data = Vec::with_capacity(len);
        for _ in 0..chunks_len {
            let mut chunk = vec![0u8; 1024];
            self.conn.read_exact(&mut chunk)?;
            data.extend_from_slice(&chunk);
        }

        // Read ACK
        let mut ack = [0u8; 3];
        self.conn.read_exact(&mut ack)?;
        if &ack != b"ACK" {
            return Err(anyhow!("No ACK received"));
        }

        // Deserialize
        Ok(postcard::from_bytes(&data[0..len])?)
    }

    /// Write with error logging
    fn write<T: Serialize>(&mut self, data: &T) -> Result<()> {
        self.do_write(data).map_err(|err| {
            log::error!("Pipe failed to write: {err:?}");
            err
        })
    }

    /// Read with error logging
    fn read<T: for<'de> Deserialize<'de>>(&mut self) -> Result<T> {
        self.do_read().map_err(|err| {
            log::error!("Pipe failed to read: {err:?}");
            err
        })
    }
}

/// IPC sender (E2E test side - SERVER)
pub struct IpcSender {
    _channel_id: IpcChannelId,
    to_tui_pipe: Option<Pipe>,
    from_tui_pipe: Option<Pipe>,
}

impl IpcSender {
    /// Create a new IPC sender (as server - binds sockets and waits for TUI to connect)
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_name, from_tui_name) = channel_id.socket_names();

        log::info!(
            "IPC [{}] Creating server sockets: {} and {}",
            channel_id.0,
            to_tui_name,
            from_tui_name
        );

        // Create listeners
        let to_tui_listener = create_listener(&to_tui_name)?;
        let from_tui_listener = create_listener(&from_tui_name)?;

        log::info!(
            "IPC [{}] Server sockets created, waiting for TUI to connect...",
            channel_id.0
        );

        // Accept connections from TUI in a blocking task
        let to_tui_stream = to_tui_listener.accept()?;
        let from_tui_stream = from_tui_listener.accept()?;

        log::info!("IPC [{}] TUI connected successfully", channel_id.0);

        Ok(Self {
            _channel_id: channel_id,
            to_tui_pipe: Some(Pipe::new(to_tui_stream)),
            from_tui_pipe: Some(Pipe::new(from_tui_stream)),
        })
    }

    /// Send a message to TUI
    pub async fn send(&mut self, message: E2EToTuiMessage) -> Result<()> {
        let mut pipe = self
            .to_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || {
            let start = Instant::now();
            let result = pipe.write(&message);

            if start.elapsed() > IO_TIMEOUT {
                return Err(anyhow!("Operation timed out"));
            }

            result.map(|_| pipe)
        })
        .await?;

        match result {
            Ok(p) => {
                self.to_tui_pipe = Some(p);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Receive a message from TUI
    pub async fn receive(&mut self) -> Result<TuiToE2EMessage> {
        let mut pipe = self
            .from_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || {
            let start = Instant::now();
            let result = pipe.read();

            if start.elapsed() > IO_TIMEOUT {
                return Err(anyhow!("Operation timed out"));
            }

            result.map(|msg| (msg, pipe))
        })
        .await?;

        match result {
            Ok((msg, p)) => {
                self.from_tui_pipe = Some(p);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    /// Send key press and wait for screen update
    pub async fn send_key_and_get_screen(&mut self, key: String) -> Result<String> {
        self.send(E2EToTuiMessage::KeyPress { key }).await?;
        self.send(E2EToTuiMessage::RequestScreen).await?;

        match self.receive().await? {
            TuiToE2EMessage::ScreenContent { content, .. } => Ok(content),
            msg => bail!("Unexpected message: {msg:?}"),
        }
    }
}

/// IPC receiver (TUI side - CLIENT)
pub struct IpcReceiver {
    _channel_id: IpcChannelId,
    to_tui_pipe: Option<Pipe>,
    from_tui_pipe: Option<Pipe>,
}

impl IpcReceiver {
    /// Create a new IPC receiver (as client - connects to existing server sockets)
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_name, from_tui_name) = channel_id.socket_names();

        log::info!(
            "IPC [{}] Connecting to E2E test server: {} and {}",
            channel_id.0,
            to_tui_name,
            from_tui_name
        );

        // Connect to E2E test's server sockets with retry
        let to_tui_stream = connect_with_retry(&to_tui_name).await?;
        let from_tui_stream = connect_with_retry(&from_tui_name).await?;

        log::info!("IPC [{}] Connected to E2E test successfully", channel_id.0);

        Ok(Self {
            _channel_id: channel_id,
            to_tui_pipe: Some(Pipe::new(to_tui_stream)),
            from_tui_pipe: Some(Pipe::new(from_tui_stream)),
        })
    }

    /// Receive a message from E2E test
    pub async fn receive(&mut self) -> Result<E2EToTuiMessage> {
        let mut pipe = self
            .to_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || {
            let start = Instant::now();
            let result = pipe.read();

            if start.elapsed() > IO_TIMEOUT {
                return Err(anyhow!("Operation timed out"));
            }

            result.map(|msg| (msg, pipe))
        })
        .await?;

        match result {
            Ok((msg, p)) => {
                self.to_tui_pipe = Some(p);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    /// Send a message to E2E test
    pub async fn send(&mut self, message: TuiToE2EMessage) -> Result<()> {
        let mut pipe = self
            .from_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || {
            let start = Instant::now();
            let result = pipe.write(&message);

            if start.elapsed() > IO_TIMEOUT {
                return Err(anyhow!("Operation timed out"));
            }

            result.map(|_| pipe)
        })
        .await?;

        match result {
            Ok(p) => {
                self.from_tui_pipe = Some(p);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

/// Create a local socket listener with proper platform detection
fn create_listener(name: &str) -> Result<LocalSocketListener> {
    // Use the simpler API - GenericNamespaced supports both namespaced and path-based sockets
    let socket_name = {
        if cfg!(unix) {
            // On Unix, try abstract namespace first, fall back to file path
            match name.to_ns_name::<GenericNamespaced>() {
                Ok(ns) => ListenerOptions::new().name(ns).create_sync(),
                Err(_) => {
                    let path = name.to_fs_name::<GenericFilePath>()?;
                    ListenerOptions::new().name(path).create_sync()
                }
            }
        } else {
            // On Windows, use named pipes
            let pipe_name = name.to_ns_name::<GenericNamespaced>()?;
            ListenerOptions::new().name(pipe_name).create_sync()
        }
    };

    socket_name.map_err(|e| {
        if e.kind() == ErrorKind::AddrInUse {
            anyhow!(
                "Socket address already in use: {name}. Please ensure no other instance is running."
            )
        } else {
            anyhow!("Failed to create listener for {name}: {e}")
        }
    })
}

/// Connect to local socket with retry logic
async fn connect_with_retry(name: &str) -> Result<LocalSocketStream> {
    let start = Instant::now();
    let name = name.to_string();

    loop {
        let name_clone = name.clone();
        let connect_result = spawn_blocking_task(move || {
            if cfg!(unix) {
                // On Unix, try abstract namespace first, fall back to file path
                let name_ref = name_clone.as_str();
                match name_ref.to_ns_name::<GenericNamespaced>() {
                    Ok(ns) => Ok(LocalSocketStream::connect(ns)?),
                    Err(_) => {
                        let path = name_ref.to_fs_name::<GenericFilePath>()?;
                        Ok(LocalSocketStream::connect(path)?)
                    }
                }
            } else {
                // On Windows, use named pipes
                let pipe_name = name_clone.to_ns_name::<GenericNamespaced>()?;
                Ok(LocalSocketStream::connect(pipe_name)?)
            }
        })
        .await?;

        match connect_result {
            Ok(stream) => {
                log::info!("Connected to {name}");
                return Ok(stream);
            }
            Err(err) => {
                if start.elapsed() >= CONNECT_TIMEOUT {
                    return Err(anyhow!(
                        "Failed to connect to {name} within {CONNECT_TIMEOUT:?}: {err}"
                    ));
                }

                sleep(CONNECT_RETRY_INTERVAL).await;
            }
        }
    }
}
