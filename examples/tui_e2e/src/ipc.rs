//! IPC communication for TUI E2E testing
//!
//! This module provides IPC-based communication between the TUI process and E2E tests.
//! Instead of using expectrl/vt100 to capture terminal output, we use a direct IPC channel
//! where:
//! - E2E tests send keyboard events to TUI
//! - TUI sends rendered screen content back to E2E tests

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{sleep, timeout};

/// Connection timeout while waiting for sockets to appear
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Interval between connection retry attempts
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);
/// Timeout for a single read/write operation
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Message from E2E test to TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Message from TUI to E2E test
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// IPC channel identifier
#[derive(Debug, Clone)]
pub struct IpcChannelId(pub String);

impl IpcChannelId {
    /// Create IPC channel paths for the given ID
    pub fn paths(&self) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir();
        let to_tui = base.join(format!("aoba_e2e_to_tui_{}.sock", self.0));
        let from_tui = base.join(format!("aoba_tui_to_e2e_{}.sock", self.0));
        (to_tui, from_tui)
    }
}

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

        timeout(
            IO_TIMEOUT,
            async {
                self.to_tui_writer.write_all(&payload).await?;
                self.to_tui_writer.write_all(b"\n").await?;
                self.to_tui_writer.flush().await
            },
        )
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
        serde_json::from_str(trimmed).with_context(|| {
            format!("Failed to deserialize IPC message from TUI: {}", trimmed)
        })
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

        let (to_tui_stream, _) = to_tui_listener
            .accept()
            .await
            .with_context(|| format!("Failed to accept IPC connection on {}", to_tui_path.display()))?;
        let (from_tui_stream, _) = from_tui_listener
            .accept()
            .await
            .with_context(|| format!("Failed to accept IPC connection on {}", from_tui_path.display()))?;

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
            format!("Failed to deserialize IPC message from E2E test: {}", trimmed)
        })
    }

    /// Send a message to E2E test
    pub async fn send(&mut self, message: TuiToE2EMessage) -> Result<()> {
    log::debug!("IPC [{}] Send: {:?}", self.channel_id.0, message);
        let payload = serde_json::to_vec(&message)?;

        timeout(
            IO_TIMEOUT,
            async {
                self.from_tui_writer.write_all(&payload).await?;
                self.from_tui_writer.write_all(b"\n").await?;
                self.from_tui_writer.flush().await
            },
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    fn random_channel_id() -> IpcChannelId {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        IpcChannelId(format!("test_{}", now.as_nanos()))
    }

    #[test]
    fn test_channel_paths() {
        let id = IpcChannelId("test123".to_string());
        let (to_tui, from_tui) = id.paths();
        
        assert!(to_tui.to_string_lossy().contains("aoba_e2e_to_tui_test123"));
        assert!(from_tui.to_string_lossy().contains("aoba_tui_to_e2e_test123"));
    }

    #[tokio::test]
    async fn test_ipc_send_receive_roundtrip() -> Result<()> {
        let channel_id = random_channel_id();
        let receiver_id = channel_id.clone();

        let receiver_task = tokio::spawn(async move { IpcReceiver::new(receiver_id).await });

        // Give the receiver a moment to bind sockets before connecting
        sleep(Duration::from_millis(50)).await;

        let mut sender = IpcSender::new(channel_id.clone()).await?;
        let mut receiver = receiver_task.await??;

        sender
            .send(E2EToTuiMessage::KeyPress {
                key: "enter".to_string(),
            })
            .await?;

        match receiver.receive().await? {
            E2EToTuiMessage::KeyPress { key } => assert_eq!(key, "enter"),
            msg => anyhow::bail!("Unexpected message: {:?}", msg),
        }

        receiver
            .send(TuiToE2EMessage::KeyProcessed)
            .await?;

        match sender.receive().await? {
            TuiToE2EMessage::KeyProcessed => {}
            msg => anyhow::bail!("Unexpected message: {:?}", msg),
        }

        Ok(())
    }

    #[test]
    fn test_message_serialization() -> Result<()> {
        let msg = E2EToTuiMessage::CharInput { ch: 'x' };
        let json = serde_json::to_string(&msg)?;
        let decoded: E2EToTuiMessage = serde_json::from_str(&json)?;
        match decoded {
            E2EToTuiMessage::CharInput { ch } => assert_eq!(ch, 'x'),
            _ => anyhow::bail!("Deserialized to unexpected variant"),
        }

        let response = TuiToE2EMessage::ScreenContent {
            content: "hello".to_string(),
            width: 80,
            height: 24,
        };
        let json = serde_json::to_string(&response)?;
        let decoded: TuiToE2EMessage = serde_json::from_str(&json)?;
        match decoded {
            TuiToE2EMessage::ScreenContent { content, width, height } => {
                assert_eq!(content, "hello");
                assert_eq!(width, 80);
                assert_eq!(height, 24);
            }
            _ => anyhow::bail!("Deserialized to unexpected variant"),
        }

        Ok(())
    }
}
