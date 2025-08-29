use anyhow::{ensure, Result};

use rmodbus::client::ModbusRequest;

pub fn generate_pull_get_coils_request(
    id: u8,
    start_address: u16,
    count: u16,
) -> Result<ModbusRequest> {
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::new();
    request.generate_get_coils(start_address, count, &mut raw)?;
    Ok(request)
}

pub fn parse_pull_get_coils(
    request: &mut ModbusRequest,
    response: Vec<u8>,
    count: u16,
) -> Result<Vec<bool>> {
    log::info!(
        "Coils payload bytes: {:?}",
        response[3..response.len() - 2]
            .iter()
            .map(|b| format!("{:08b}", b))
            .collect::<Vec<_>>()
    );
    request.parse_ok(&response)?;
    println!("Received coils response: {:02x?}", response);
    ensure!(
        count as usize <= (response.len() - 5) * 8,
        "Invalid response length"
    );

    // Parse coils bit order: high bit to low bit within each byte

    let values = response[3..response.len() - 2]
        .iter()
        .map(|chunk| {
            let byte = chunk;
            (0..8)
                .rev() // iterate bits from high to low
                .map(|i| (byte & (1 << i)) != 0)
                .collect::<Vec<bool>>()
        })
        .flatten()
        .collect::<Vec<bool>>();
    // Skip leading zeros that were used as padding
    let values = values
        .iter()
        .skip(values.len() - count as usize)
        .cloned()
        .collect::<Vec<bool>>();
    ensure!(
        values.len() == count as usize,
        "Invalid number of coils in response"
    );
    log::debug!("Received coils: {:?}", values);

    Ok(values)
}
