mod frame;
mod header;
mod pull_get_coils;
mod pull_get_holdings;
mod pull_set_coils;
mod slave_coils;
mod slave_holdings;

use anyhow::Result;
use bytes::Bytes;
use chrono::Duration;
use flume::{Receiver, Sender};

use rmodbus::{
    client::ModbusRequest,
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};

pub use frame::read_modbus_frame;
pub use header::parse_modbus_header;
pub use pull_get_coils::{generate_pull_get_coils_request, parse_pull_get_coils};
pub use pull_get_holdings::{generate_pull_get_holdings_request, parse_pull_get_holdings};
pub use pull_set_coils::{generate_pull_set_coils_request, parse_pull_set_coils};
pub use slave_coils::parse_slave_coils;
pub use slave_holdings::parse_slave_holdings;

pub fn boot_modbus_pull_service(id: u8, request_sender: Sender<Bytes>) -> Result<()> {
    let request_tx = request_sender.to_owned();

    let mut last_sent_timestamp = chrono::Utc::now();

    #[derive(Debug, Clone, Copy)]
    enum PollTask {
        GetCoils,
        GetHoldings,
    }

    impl PollTask {
        fn next(&self) -> Self {
            match self {
                PollTask::GetCoils => PollTask::GetHoldings,
                PollTask::GetHoldings => PollTask::GetCoils,
            }
        }

        fn wait_next_duration(&self) -> Duration {
            match self {
                PollTask::GetCoils => Duration::seconds(2),
                PollTask::GetHoldings => Duration::seconds(1),
            }
        }
    }

    let mut current_task = PollTask::GetCoils;

    loop {
        let now = chrono::Utc::now();
        if request_tx.is_empty() && now - last_sent_timestamp > current_task.wait_next_duration() {
            log::info!("Sending Modbus pull request for {:?}", current_task);
            // Periodically send data pull requests
            let mut request = ModbusRequest::new(id, ModbusProto::Rtu);
            let mut raw = Vec::new();
            match current_task {
                PollTask::GetCoils => request.generate_get_coils(1, 11, &mut raw)?,
                PollTask::GetHoldings => request.generate_get_holdings(16, 33, &mut raw)?,
            }
            request_sender.send(Bytes::from(raw))?;

            last_sent_timestamp = chrono::Utc::now();
            current_task = current_task.next();
        }
    }
}

pub fn boot_modbus_slave_service(
    id: u8,
    mut context: ModbusStorageSmall,
    request_receiver: Receiver<Bytes>,
    response_sender: Sender<Bytes>,
) -> Result<()> {
    while let Ok(request) = request_receiver.recv() {
        log::info!(
            "Received Modbus request: {}",
            request
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let mut response = Vec::new();
        let mut frame = ModbusFrame::new(id, request.as_ref(), ModbusProto::Rtu, &mut response);
        frame.parse()?;

        match frame.func {
            0x01 => {
                // Handle read coils requests
                if let Ok(Some(ret)) = parse_slave_coils(&mut frame, &mut context) {
                    log::info!(
                        "Parsed slave coils: {}",
                        ret.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    response_sender.send(Bytes::from(ret))?;
                } else {
                    log::warn!("Failed to parse slave coils");
                }
            }
            0x03 => {
                // Handle read holding registers requests
                if let Ok(Some(ret)) = parse_slave_holdings(&mut frame, &mut context) {
                    log::info!(
                        "Parsed slave holdings: {}",
                        ret.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    response_sender.send(Bytes::from(ret))?;
                } else {
                    log::warn!("Failed to parse slave holdings");
                }
            }
            _ => {
                log::warn!("Unsupported function code: {}", frame.func);
            }
        }
    }

    Ok(())
}
