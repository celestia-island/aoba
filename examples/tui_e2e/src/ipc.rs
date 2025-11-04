//! IPC communication for TUI E2E testing
//!
//! This module provides IPC-based communication between the TUI process and E2E tests.
//! Instead of using expectrl/vt100 to capture terminal output, we use a direct IPC channel
//! where:
//! - E2E tests send keyboard events to TUI
//! - TUI sends rendered screen content back to E2E tests

// Re-export all IPC types from ci_utils
pub use aoba_ci_utils::{
    E2EToTuiMessage, IpcChannelId, IpcReceiver, IpcSender, TuiToE2EMessage, CONNECT_RETRY_INTERVAL,
    CONNECT_TIMEOUT, IO_TIMEOUT,
};

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
