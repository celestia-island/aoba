//! Async serial port stream backed by `tokio-serial`.
//!
//! Provides [`AsyncSerialPort`] which implements [`tokio::io::AsyncRead`] and
//! [`tokio::io::AsyncWrite`], enabling non-blocking serial I/O inside a tokio
//! runtime.
//!
//! This module is only available when the `async-serial` feature is enabled.
//!
//! # Example
//!
//! ```rust,no_run
//! use aoba::api::async_serial::AsyncSerialPort;
//! use std::time::Duration;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut stream = AsyncSerialPort::open("/dev/ttyUSB0", 115200, Duration::from_secs(2))?;
//!     stream.write_all(b"hello").await?;
//!     let mut buf = [0u8; 64];
//!     let n = stream.read(&mut buf).await?;
//!     Ok(())
//! }
//! ```

use anyhow::{anyhow, Result};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub use tokio_serial::SerialStream as InnerStream;

/// Tokio-native async serial port.
///
/// Wraps [`tokio_serial::SerialStream`] which natively implements
/// [`AsyncRead`] + [`AsyncWrite`] without blocking the tokio runtime.
pub struct AsyncSerialPort {
    inner: InnerStream,
}

impl AsyncSerialPort {
    /// Open a serial port for async I/O.
    ///
    /// Rejects virtual ports (UUID/HTTP).
    ///
    /// Note: unlike the synchronous [`crate::api::utils::open_serial_port`],
    /// this function does **not** set exclusive-access mode on Unix because
    /// `tokio-serial` does not expose the underlying file descriptor. If
    /// exclusive access is required, use the synchronous API with
    /// `spawn_blocking`.
    pub fn open(port: &str, baud_rate: u32, timeout: std::time::Duration) -> Result<Self> {
        if crate::api::is_virtual_port(port) {
            return Err(anyhow!(
                "Port {} is a virtual port and cannot be opened as a physical serial port",
                port
            ));
        }

        let builder = tokio_serial::new(port, baud_rate).timeout(timeout);

        let stream = tokio_serial::SerialStream::open(&builder)
            .map_err(|e| anyhow!("tokio-serial open: {e}"))?;

        Ok(Self { inner: stream })
    }

    /// Access the underlying [`tokio_serial::SerialStream`].
    pub fn inner(&self) -> &InnerStream {
        &self.inner
    }

    /// Decompose into the underlying stream.
    pub fn into_inner(self) -> InnerStream {
        self.inner
    }
}

impl AsyncRead for AsyncSerialPort {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for AsyncSerialPort {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
