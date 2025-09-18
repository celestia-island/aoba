use anyhow::{anyhow, Result};

use rmodbus::{
    consts::ModbusFunction,
    server::{storage::ModbusStorageSmall, ModbusFrame},
};

pub fn build_slave_coils_response(
    request: &mut ModbusFrame<Vec<u8>>,
    context: &mut ModbusStorageSmall,
) -> Result<Option<Vec<u8>>> {
    log::info!("Parsed Modbus frame: {request:02x?}");
    log::info!("Is readonly: {}", request.readonly);
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
        if request.func == ModbusFunction::GetCoils {
            // Standard Modbus coils response; no custom byte order transformation applied.
            log::info!("Response length: {}", request.response.len());
        }
        request.finalize_response()?;

        log::info!("Send Modbus response: {:02x?}", request.response);
        return Ok(Some(request.response.clone()));
    }

    Ok(None)
}
