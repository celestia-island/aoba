use anyhow::Result;

use rmodbus::client::ModbusRequest;

/// Build a frame to write a single holding register (function 0x06)
pub fn generate_pull_set_holding_request(
    id: u8,
    address: u16,
    value: u16,
) -> Result<(ModbusRequest, Vec<u8>)> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::new();
    request.generate_set_holding(address, value, &mut raw)?;
    Ok((request, raw))
}
