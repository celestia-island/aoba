use chrono::Local;
use rmodbus::{
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};

use crate::{i18n::lang, protocol::runtime::RuntimeCommand, protocol::status::*};

impl Status {
    pub fn try_auto_slave_response_with_frame(
        &mut self,
        port_name: String,
        bytes: &[u8],
        cmd_tx: &flume::Sender<RuntimeCommand>,
    ) -> Option<LogEntry> {
        if bytes.len() < 5 {
            return None;
        }
        let sid = bytes[0];
        let func = bytes[1];
        match func {
            0x01..=0x06 | 0x0F | 0x10 => {}
            _ => return None,
        };
        let ctx = self
            .per_port_slave_contexts
            .entry(port_name)
            .or_insert_with(ModbusStorageSmall::default);
        let mut response_buf: Vec<u8> = Vec::new();
        let mut frame = ModbusFrame::new(sid, bytes, ModbusProto::Rtu, &mut response_buf);
        if frame.parse().is_err() {
            return None;
        }
        use crate::protocol::modbus::{
            build_slave_coils_response, build_slave_discrete_inputs_response,
            build_slave_holdings_response, build_slave_inputs_response,
        };
        let resp_res: anyhow::Result<Option<Vec<u8>>> = match func {
            0x01 => build_slave_coils_response(&mut frame, ctx),
            0x02 => build_slave_discrete_inputs_response(&mut frame, ctx),
            0x03 => build_slave_holdings_response(&mut frame, ctx),
            0x04 => build_slave_inputs_response(&mut frame, ctx),
            0x05 | 0x06 | 0x0F | 0x10 => {
                if frame.processing_required {
                    let _ = if frame.readonly {
                        frame.process_read(ctx)
                    } else {
                        frame.process_write(ctx)
                    };
                }
                if frame.response_required {
                    if frame.finalize_response().is_err() {
                        return None;
                    }
                    Ok(Some(frame.response.clone()))
                } else {
                    Ok(None)
                }
            }
            _ => return None,
        };
        let ret = match resp_res {
            Ok(Some(v)) => v,
            _ => return None,
        };
        if ret.is_empty() {
            return None;
        }
        let _ = cmd_tx.send(RuntimeCommand::Write(ret.clone()));
        let hex = ret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        Some(LogEntry {
            when: Local::now(),
            raw: format!("{}: {hex}", lang().protocol.modbus.log_sent_frame),
            parsed: Some(ParsedRequest {
                origin: "slave".into(),
                rw: "W".into(),
                command: format!("resp_func_{func:02X}"),
                slave_id: sid,
                address: if bytes.len() >= 4 {
                    u16::from_be_bytes([bytes[2], bytes[3]])
                } else {
                    0
                },
                length: if bytes.len() >= 6 {
                    u16::from_be_bytes([bytes[4], bytes[5]])
                } else {
                    0
                },
            }),
        })
    }
}
