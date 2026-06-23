#![allow(clippy::wildcard_enum_match_arm)]
//! IPC communication utilities for TUI E2E testing
//!
//! This module provides cross-platform IPC for communication between
//! TUI E2E tests and the TUI process using the `interprocess` library.

use anyhow::{anyhow, bail, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::io::ErrorKind;
use tokio::time::sleep;

use interprocess::local_socket::{
    prelude::*,
    {GenericFilePath, GenericNamespaced, ListenerOptions},
};

use super::{
    E2EToTuiMessage, IpcChannelId, TuiToE2EMessage, CONNECT_RETRY_INTERVAL, CONNECT_TIMEOUT,
};
use crate::core::task_manager::spawn_blocking_task;

const LEN_PREFIX_SIZE: usize = 8;

struct Pipe {
    conn: LocalSocketStream,
}

impl Pipe {
    const fn new(conn: LocalSocketStream) -> Self {
        Self { conn }
    }

    fn do_write<T: Serialize>(&mut self, data: &T) -> Result<()> {
        use std::io::Write;

        let payload = postcard::to_allocvec(data)?;
        let len_bytes = (payload.len() as u64).to_le_bytes();
        self.conn.write_all(&len_bytes)?;
        self.conn.write_all(&payload)?;
        self.conn.flush()?;
        Ok(())
    }

    fn do_read<T: DeserializeOwned>(&mut self) -> Result<T> {
        use std::io::Read;

        let mut len_buf = [0u8; LEN_PREFIX_SIZE];
        self.conn.read_exact(&mut len_buf)?;
        #[allow(clippy::cast_possible_truncation)]
        let payload_len = u64::from_le_bytes(len_buf) as usize;

        if payload_len > 10 * 1024 * 1024 {
            bail!("IPC payload too large: {payload_len} bytes");
        }

        let mut buf = vec![0u8; payload_len];
        self.conn.read_exact(&mut buf)?;
        Ok(postcard::from_bytes(&buf)?)
    }

    fn write<T: Serialize>(&mut self, data: &T) -> Result<()> {
        self.do_write(data).map_err(|err| {
            log::error!("Pipe failed to write: {err:?}");
            err
        })
    }

    fn read<T: DeserializeOwned>(&mut self) -> Result<T> {
        self.do_read().map_err(|err| {
            log::error!("Pipe failed to read: {err:?}");
            err
        })
    }
}

pub struct IpcSender {
    _channel_id: IpcChannelId,
    to_tui_pipe: Option<Pipe>,
    from_tui_pipe: Option<Pipe>,
}

impl IpcSender {
    #[allow(clippy::unused_async)]
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_name, from_tui_name) = channel_id.socket_names();

        log::info!(
            "IPC [{}] Creating server sockets: {} and {}",
            channel_id.0,
            to_tui_name,
            from_tui_name
        );

        let to_tui_listener = create_listener(&to_tui_name)?;
        let from_tui_listener = create_listener(&from_tui_name)?;

        log::info!(
            "IPC [{}] Server sockets created, waiting for TUI to connect...",
            channel_id.0
        );

        let to_tui_stream = to_tui_listener.accept()?;
        let from_tui_stream = from_tui_listener.accept()?;

        log::info!("IPC [{}] TUI connected successfully", channel_id.0);

        Ok(Self {
            _channel_id: channel_id,
            to_tui_pipe: Some(Pipe::new(to_tui_stream)),
            from_tui_pipe: Some(Pipe::new(from_tui_stream)),
        })
    }

    pub async fn send(&mut self, message: E2EToTuiMessage) -> Result<()> {
        let mut pipe = self
            .to_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || pipe.write(&message).map(|()| pipe)).await?;

        match result {
            Ok(p) => {
                self.to_tui_pipe = Some(p);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn receive(&mut self) -> Result<TuiToE2EMessage> {
        let mut pipe = self
            .from_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || pipe.read().map(|msg| (msg, pipe))).await?;

        match result {
            Ok((msg, p)) => {
                self.from_tui_pipe = Some(p);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn send_key_and_get_screen(&mut self, key: String) -> Result<String> {
        self.send(E2EToTuiMessage::KeyPress { key }).await?;
        self.send(E2EToTuiMessage::RequestScreen).await?;

        match self.receive().await? {
            TuiToE2EMessage::ScreenContent { content, .. } => Ok(content),
            msg => bail!("Unexpected message: {msg:?}"),
        }
    }
}

pub struct IpcReceiver {
    _channel_id: IpcChannelId,
    to_tui_pipe: Option<Pipe>,
    from_tui_pipe: Option<Pipe>,
}

impl IpcReceiver {
    pub async fn new(channel_id: IpcChannelId) -> Result<Self> {
        let (to_tui_name, from_tui_name) = channel_id.socket_names();

        log::info!(
            "IPC [{}] Connecting to E2E test server: {} and {}",
            channel_id.0,
            to_tui_name,
            from_tui_name
        );

        let to_tui_stream = connect_with_retry(&to_tui_name).await?;
        let from_tui_stream = connect_with_retry(&from_tui_name).await?;

        log::info!("IPC [{}] Connected to E2E test successfully", channel_id.0);

        Ok(Self {
            _channel_id: channel_id,
            to_tui_pipe: Some(Pipe::new(to_tui_stream)),
            from_tui_pipe: Some(Pipe::new(from_tui_stream)),
        })
    }

    pub async fn receive(&mut self) -> Result<E2EToTuiMessage> {
        let mut pipe = self
            .to_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || pipe.read().map(|msg| (msg, pipe))).await?;

        match result {
            Ok((msg, p)) => {
                self.to_tui_pipe = Some(p);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn send(&mut self, message: TuiToE2EMessage) -> Result<()> {
        let mut pipe = self
            .from_tui_pipe
            .take()
            .ok_or_else(|| anyhow!("Pipe already taken"))?;

        let result = spawn_blocking_task(move || pipe.write(&message).map(|()| pipe)).await?;

        match result {
            Ok(p) => {
                self.from_tui_pipe = Some(p);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

fn create_listener(name: &str) -> Result<LocalSocketListener> {
    let listener = if cfg!(unix) {
        if let Ok(ns) = name.to_ns_name::<GenericNamespaced>() {
            ListenerOptions::new().name(ns).create_sync()
        } else {
            let path = name.to_fs_name::<GenericFilePath>()?;
            ListenerOptions::new().name(path).create_sync()
        }
    } else {
        let pipe_name = name.to_ns_name::<GenericNamespaced>()?;
        ListenerOptions::new().name(pipe_name).create_sync()
    };

    listener.map_err(|e| {
        if e.kind() == ErrorKind::AddrInUse {
            anyhow!(
                "Socket address already in use: {name}. Please ensure no other instance is running."
            )
        } else {
            anyhow!("Failed to create listener for {name}: {e}")
        }
    })
}

async fn connect_with_retry(name: &str) -> Result<LocalSocketStream> {
    let start = std::time::Instant::now();
    let name = name.to_string();

    loop {
        let name_clone = name.clone();
        let connect_result = spawn_blocking_task(move || {
            if cfg!(unix) {
                let name_ref = name_clone.as_str();
                if let Ok(ns) = name_ref.to_ns_name::<GenericNamespaced>() {
                    Ok(LocalSocketStream::connect(ns)?)
                } else {
                    let path = name_ref.to_fs_name::<GenericFilePath>()?;
                    Ok(LocalSocketStream::connect(path)?)
                }
            } else {
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
