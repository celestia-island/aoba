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
    // Value extraction delegated to the family hub (single source since
    // 2026-10-22; the hand-rolled slicing here used to duplicate it).
    hare::protocol::modbus::parse_register_values(response)
        .map_err(|e| anyhow::anyhow!("holdings parse: {e:?}"))
}
