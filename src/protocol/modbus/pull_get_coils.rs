use anyhow::{ensure, Result};

use rmodbus::client::ModbusRequest;

pub fn generate_pull_get_coils_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<(ModbusRequest, Vec<u8>)> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::with_capacity(8);
    request.generate_get_coils(start_address, count, &mut raw)?;
    Ok((request, raw))
}

pub fn parse_pull_get_coils(
    request: &mut ModbusRequest,
    response: &[u8],
    count: u16,
) -> Result<Vec<bool>> {
    request.parse_ok(response)?;

    if response.len() < 5 {
        return Err(anyhow::anyhow!(
            "Response too short for coils: {} bytes",
            response.len()
        ));
    }

    let values = hare::protocol::modbus::parse_bit_values(response, count as usize)
        .map_err(|e| anyhow::anyhow!("coils parse: {e:?}"))?;
    ensure!(
        values.len() == count as usize,
        "Invalid number of coils in response"
    );

    Ok(values)
}
