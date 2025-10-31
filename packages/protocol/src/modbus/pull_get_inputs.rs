use anyhow::Result;

use rmodbus::client::ModbusRequest;

/// Generate a Modbus RTU request to read input registers (function 0x04)
pub fn generate_pull_get_inputs_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<(ModbusRequest, Vec<u8>)> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::with_capacity(8);
    request.generate_get_inputs(start_address, count, &mut raw)?;
    Ok((request, raw))
}

/// Parse a Modbus response for input registers (function 0x04) into u16 values.
pub fn parse_pull_get_inputs(request: &mut ModbusRequest, response: Vec<u8>) -> Result<Vec<u16>> {
    request.parse_ok(&response)?;

    let values = response[3..response.len() - 2]
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect::<Vec<_>>();
    log::debug!("Received input registers: {values:?}");

    Ok(values)
}
