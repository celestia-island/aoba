use anyhow::{anyhow, Result};
use std::time::Duration;

/// Open a serial port with the requested timeout, enabling exclusive access on Unix systems.
pub fn open_serial_port(
    port: &str,
    baud_rate: u32,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>> {
    use crate::protocol::status::types::port::PortType;

    // Check if this is a virtual port (IPC/HTTP)
    let port_type = PortType::detect(port);
    if port_type.is_virtual() {
        return Err(anyhow!(
            "Port {} is a virtual port (type: {}). Virtual ports cannot be opened as physical serial ports. \
            Use IPC or HTTP communication methods instead.",
            port,
            port_type
        ));
    }

    let builder = serialport::new(port, baud_rate).timeout(timeout);

    #[cfg(unix)]
    {
        let mut handle = builder
            .open_native()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))?;
        handle
            .set_exclusive(true)
            .map_err(|err| anyhow!("Failed to acquire exclusive access to {port}: {err}"))?;
        Ok(Box::new(handle))
    }

    #[cfg(not(unix))]
    {
        builder
            .open()
            .map_err(|err| anyhow!("Failed to open port {port}: {err}"))
    }
}
