use anyhow::Result;

use rmodbus::{guess_response_frame_len, ModbusProto};

pub fn parse_modbus_header(buf: [u8; 6]) -> Result<usize> {
    Ok(guess_response_frame_len(&buf, ModbusProto::Rtu)? as usize)
}
