use anyhow::{anyhow, Result};

use rmodbus::server::{storage::ModbusStorageSmall, ModbusFrame};

pub fn build_slave_holdings_response(
    request: &mut ModbusFrame<Vec<u8>>,
    context: &mut ModbusStorageSmall,
) -> Result<Option<Vec<u8>>> {
    if request.processing_required {
        let result = if request.readonly {
            request.process_read(context)
        } else {
            request.process_write(context)
        };
        if result.is_err() {
            return Err(anyhow!("Frame processing error"));
        }
    }

    if request.response_required {
        request.finalize_response()?;

        log::info!("Send Modbus response: {:02x?}", request.response);
        return Ok(Some(request.response.clone()));
    }

    Ok(None)
}
