use anyhow::Result;

use rmodbus::client::ModbusRequest;

pub fn generate_pull_set_coils_request(id: u8, coils: Vec<u8>) -> Result<ModbusRequest> {
    // TODO: Accept coils as boolean values with an explicit start address; handle byte alignment (pack bits into bytes) here.
    let mut request = ModbusRequest::new(id, rmodbus::ModbusProto::Rtu);
    let mut raw = Vec::new();
    request.generate_set_coils_bulk(1, &coils, &mut raw)?;
    Ok(request)
}

pub fn parse_pull_set_coils(request: &mut ModbusRequest, response: Vec<u8>) -> Result<()> {
    request.parse_ok(&response)?;

    Ok(())
}
