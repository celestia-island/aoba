//! Integration tests for IPC communication

use anyhow::bail;
use std::time::Duration;

use aoba_ci_utils::{E2EToTuiMessage, IpcChannelId, IpcReceiver, IpcSender, TuiToE2EMessage};

fn random_channel_id() -> IpcChannelId {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    IpcChannelId(format!("test_{}", now.as_nanos()))
}

#[tokio::test]
async fn test_ipc_send_receive_roundtrip() -> anyhow::Result<()> {
    let channel_id = random_channel_id();
    let receiver_id = channel_id.clone();

    // Spawn receiver task
    let receiver_task = tokio::spawn(async move { IpcReceiver::new(receiver_id).await });

    // Give the receiver a moment to bind sockets before connecting
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Create sender
    let mut sender = IpcSender::new(channel_id.clone()).await?;
    let mut receiver = receiver_task.await??;

    // Test KeyPress message
    sender
        .send(E2EToTuiMessage::KeyPress {
            key: "enter".to_string(),
        })
        .await?;

    match receiver.receive().await? {
        E2EToTuiMessage::KeyPress { key } => assert_eq!(key, "enter"),
        msg => bail!("Unexpected message: {msg:?}"),
    }

    // Test response from TUI
    receiver.send(TuiToE2EMessage::KeyProcessed).await?;

    match sender.receive().await? {
        TuiToE2EMessage::KeyProcessed => {}
        msg => bail!("Unexpected message: {msg:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_ipc_char_input() -> anyhow::Result<()> {
    let channel_id = random_channel_id();
    let receiver_id = channel_id.clone();

    let receiver_task = tokio::spawn(async move { IpcReceiver::new(receiver_id).await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut sender = IpcSender::new(channel_id).await?;
    let mut receiver = receiver_task.await??;

    // Test CharInput message
    sender.send(E2EToTuiMessage::CharInput { ch: 'x' }).await?;

    match receiver.receive().await? {
        E2EToTuiMessage::CharInput { ch } => assert_eq!(ch, 'x'),
        msg => bail!("Unexpected message: {msg:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_ipc_screen_content() -> anyhow::Result<()> {
    let channel_id = random_channel_id();
    let receiver_id = channel_id.clone();

    let receiver_task = tokio::spawn(async move { IpcReceiver::new(receiver_id).await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut sender = IpcSender::new(channel_id).await?;
    let mut receiver = receiver_task.await??;

    // Request screen
    sender.send(E2EToTuiMessage::RequestScreen).await?;

    match receiver.receive().await? {
        E2EToTuiMessage::RequestScreen => {}
        msg => bail!("Unexpected message: {msg:?}"),
    }

    // Send screen content response
    let test_content = "Hello, World!".to_string();
    receiver
        .send(TuiToE2EMessage::ScreenContent {
            content: test_content.clone(),
            width: 80,
            height: 24,
        })
        .await?;

    match sender.receive().await? {
        TuiToE2EMessage::ScreenContent {
            content,
            width,
            height,
        } => {
            assert_eq!(content, test_content);
            assert_eq!(width, 80);
            assert_eq!(height, 24);
        }
        msg => bail!("Unexpected message: {msg:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_ipc_shutdown() -> anyhow::Result<()> {
    let channel_id = random_channel_id();
    let receiver_id = channel_id.clone();

    let receiver_task = tokio::spawn(async move { IpcReceiver::new(receiver_id).await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut sender = IpcSender::new(channel_id).await?;
    let mut receiver = receiver_task.await??;

    // Send shutdown message
    sender.send(E2EToTuiMessage::Shutdown).await?;

    match receiver.receive().await? {
        E2EToTuiMessage::Shutdown => {}
        msg => bail!("Unexpected message: {msg:?}"),
    }

    Ok(())
}
