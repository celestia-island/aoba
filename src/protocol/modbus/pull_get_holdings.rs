use anyhow::Result;

use rmodbus::client::ModbusRequest;

pub fn generate_pull_get_holdings_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<(ModbusRequest, Vec<u8>)> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::with_capacity(8);
    request.generate_get_holdings(start_address, count, &mut raw)?;
    Ok((request, raw))
}

pub fn parse_pull_get_holdings(request: &mut ModbusRequest, response: &[u8]) -> Result<Vec<u16>> {
    request.parse_ok(response)?;

    if response.len() < 5 {
        return Err(anyhow::anyhow!(
            "Response too short for holdings: {} bytes",
            response.len()
        ));
    }

    let values = response[3..response.len() - 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| u16::from_be_bytes(*chunk))
        .collect::<Vec<_>>();

    Ok(values)
}
