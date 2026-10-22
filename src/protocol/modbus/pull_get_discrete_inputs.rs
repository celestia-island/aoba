use anyhow::{anyhow, ensure, Result};

use rmodbus::client::ModbusRequest;

/// Generate a Modbus RTU request to read discrete inputs (function 0x02)
pub fn generate_pull_get_discrete_inputs_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<(ModbusRequest, Vec<u8>)> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::with_capacity(8);
    request.generate_get_discretes(start_address, count, &mut raw)?;
    Ok((request, raw))
}

/// Parse a Modbus response for discrete inputs (function 0x02) into a vector of bools.
pub fn parse_pull_get_discrete_inputs(
    request: &mut ModbusRequest,
    response: &[u8],
    count: u16,
) -> Result<Vec<bool>> {
    request.parse_ok(response)?;

    if response.len() < 5 {
        return Err(anyhow::anyhow!(
            "Response too short for discrete inputs: {} bytes",
            response.len()
        ));
    }

    let values = hare::protocol::modbus::parse_bit_values(response, count as usize)
        .map_err(|e| anyhow::anyhow!("discrete inputs parse: {e:?}"))?;
    ensure!(
        values.len() == count as usize,
        "Invalid number of discrete inputs in response"
    );

    Ok(values)
}
