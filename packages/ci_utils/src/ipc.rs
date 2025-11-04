//! IPC communication utilities for TUI E2E testing
//!
//! This module provides Unix socket-based IPC for communication between
//! TUI E2E tests and the TUI process.

use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{sleep, timeout};

use super::{
    E2EToTuiMessage, IpcChannelId, TuiToE2EMessage, CONNECT_RETRY_INTERVAL, CONNECT_TIMEOUT,
    IO_TIMEOUT,
};

/// IPC sender (E2E test side)
pub struct IpcSender {
    channel_id: IpcChannelId,
    to_tui_writer: OwnedWriteHalf,
    from_tui_reader: BufReader<OwnedReadHalf>,
}

impl IpcSender {
    /// Create a new IPC sender
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_path, from_tui_path) = channel_id.paths();

        let to_tui_stream = connect_with_retry(&to_tui_path).await?;
        let from_tui_stream = connect_with_retry(&from_tui_path).await?;

        let (_, to_tui_writer) = to_tui_stream.into_split();
        let (from_tui_reader, _) = from_tui_stream.into_split();

        Ok(Self {
            channel_id,
            to_tui_writer,
            from_tui_reader: BufReader::new(from_tui_reader),
        })
    }

    /// Send a message to TUI
    pub async fn send(&mut self, message: E2EToTuiMessage) -> Result<()> {
        log::debug!("IPC [{}] Send: {:?}", self.channel_id.0, message);
        let payload = serde_json::to_vec(&message)?;

        timeout(IO_TIMEOUT, async {
            self.to_tui_writer.write_all(&payload).await?;
            self.to_tui_writer.write_all(b"\n").await?;
            self.to_tui_writer.flush().await
        })
        .await
        .map_err(|_| anyhow!("Timed out sending IPC message"))??;

        Ok(())
    }

    /// Receive a message from TUI
    pub async fn receive(&mut self) -> Result<TuiToE2EMessage> {
        log::debug!(
            "IPC [{}] Receive: waiting for message from TUI",
            self.channel_id.0
        );

        let mut line = String::new();
        let bytes_read = timeout(IO_TIMEOUT, self.from_tui_reader.read_line(&mut line))
            .await
            .map_err(|_| anyhow!("Timed out waiting for IPC message"))??;

        if bytes_read == 0 {
            anyhow::bail!("IPC connection closed by TUI");
        }

        let trimmed = line.trim_end();
        serde_json::from_str(trimmed)
            .with_context(|| format!("Failed to deserialize IPC message from TUI: {}", trimmed))
    }

    /// Send key press and wait for screen update
    pub async fn send_key_and_get_screen(&mut self, key: String) -> Result<String> {
        self.send(E2EToTuiMessage::KeyPress { key }).await?;
        self.send(E2EToTuiMessage::RequestScreen).await?;

        match self.receive().await? {
            TuiToE2EMessage::ScreenContent { content, .. } => Ok(content),
            msg => anyhow::bail!("Unexpected message: {:?}", msg),
        }
    }
}

/// IPC receiver (TUI side)
pub struct IpcReceiver {
    channel_id: IpcChannelId,
    to_tui_reader: BufReader<OwnedReadHalf>,
    from_tui_writer: OwnedWriteHalf,
}

impl IpcReceiver {
    /// Create a new IPC receiver
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_path, from_tui_path) = channel_id.paths();

        cleanup_socket(&to_tui_path);
        cleanup_socket(&from_tui_path);

        let to_tui_listener = UnixListener::bind(&to_tui_path)
            .with_context(|| format!("Failed to bind IPC socket: {}", to_tui_path.display()))?;
        let from_tui_listener = UnixListener::bind(&from_tui_path)
            .with_context(|| format!("Failed to bind IPC socket: {}", from_tui_path.display()))?;

        let (to_tui_stream, _) = to_tui_listener.accept().await.with_context(|| {
            format!(
                "Failed to accept IPC connection on {}",
                to_tui_path.display()
            )
        })?;
        let (from_tui_stream, _) = from_tui_listener.accept().await.with_context(|| {
            format!(
                "Failed to accept IPC connection on {}",
                from_tui_path.display()
            )
        })?;

        let (to_tui_reader, _) = to_tui_stream.into_split();
        let (_, from_tui_writer) = from_tui_stream.into_split();

        Ok(Self {
            channel_id,
            to_tui_reader: BufReader::new(to_tui_reader),
            from_tui_writer,
        })
    }

    /// Receive a message from E2E test
    pub async fn receive(&mut self) -> Result<E2EToTuiMessage> {
        log::debug!(
            "IPC [{}] Receive: waiting for message from E2E test",
            self.channel_id.0
        );

        let mut line = String::new();
        let bytes_read = timeout(IO_TIMEOUT, self.to_tui_reader.read_line(&mut line))
            .await
            .map_err(|_| anyhow!("Timed out waiting for IPC message"))??;

        if bytes_read == 0 {
            anyhow::bail!("IPC connection closed by E2E test");
        }

        let trimmed = line.trim_end();
        serde_json::from_str(trimmed).with_context(|| {
            format!(
                "Failed to deserialize IPC message from E2E test: {}",
                trimmed
            )
        })
    }

    /// Send a message to E2E test
    pub async fn send(&mut self, message: TuiToE2EMessage) -> Result<()> {
        log::debug!("IPC [{}] Send: {:?}", self.channel_id.0, message);
        let payload = serde_json::to_vec(&message)?;

        timeout(IO_TIMEOUT, async {
            self.from_tui_writer.write_all(&payload).await?;
            self.from_tui_writer.write_all(b"\n").await?;
            self.from_tui_writer.flush().await
        })
        .await
        .map_err(|_| anyhow!("Timed out sending IPC message"))??;

        Ok(())
    }
}

impl Drop for IpcReceiver {
    fn drop(&mut self) {
        let (to_tui_path, from_tui_path) = self.channel_id.paths();
        cleanup_socket(&to_tui_path);
        cleanup_socket(&from_tui_path);
    }
}

async fn connect_with_retry(path: &Path) -> Result<UnixStream> {
    let start = Instant::now();
    loop {
        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(err) => {
                if start.elapsed() >= CONNECT_TIMEOUT {
                    return Err(anyhow!(
                        "Failed to connect to {} within {:?}: {}",
                        path.display(),
                        CONNECT_TIMEOUT,
                        err
                    ));
                }

                sleep(CONNECT_RETRY_INTERVAL).await;
            }
        }
    }
}

fn cleanup_socket(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Failed to remove IPC socket {}: {}", path.display(), err);
        }
    }
}
