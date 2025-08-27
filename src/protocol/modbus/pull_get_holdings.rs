use anyhow::Result;

use rmodbus::client::ModbusRequest;

pub fn generate_pull_get_holdings_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<ModbusRequest> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::new();
    request.generate_get_holdings(start_address, count, &mut raw)?;
    Ok(request)
}

pub fn parse_pull_get_holdings(request: &mut ModbusRequest, response: Vec<u8>) -> Result<Vec<u16>> {
    request.parse_ok(&response)?;
    println!(
        "Received holding registers response: {:02x?}, len: {}",
        response,
        response.len()
    );

    let values = response[3..response.len() - 2]
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    println!("Received holding registers (BE 0,1): {:?}", values);

    Ok(values)
}
