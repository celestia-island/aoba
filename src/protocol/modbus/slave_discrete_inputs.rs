use anyhow::{anyhow, Result};
use rmodbus::server::{storage::ModbusStorageSmall, ModbusFrame};

/// Parse and process a Modbus Read Discrete Inputs (0x02) frame using rmodbus server helpers.
/// Mirrors structure of existing coil / holding handlers for consistency.
pub fn parse_slave_discrete_inputs(
    request: &mut ModbusFrame<Vec<u8>>,
    context: &mut ModbusStorageSmall,
) -> Result<Option<Vec<u8>>> {
    if request.processing_required {
        let result = if request.readonly { request.process_read(context) } else { request.process_write(context) };
        if result.is_err() { return Err(anyhow!("Frame processing error")); }
    }
    if request.response_required {
        request.finalize_response()?;
        log::info!("Send Modbus 0x02 response: {:02x?}", request.response);
        return Ok(Some(request.response.clone()));
    }
    Ok(None)
}
