use chrono::Local;
use rmodbus::{
    server::{storage::ModbusStorageSmall, ModbusFrame},
    ModbusProto,
};

use crate::{
    i18n::lang,
    protocol::{
        modbus::{
            build_slave_coils_response, build_slave_discrete_inputs_response,
            build_slave_holdings_response, build_slave_inputs_response,
        },
        runtime::RuntimeCommand,
        status::*,
    },
};

impl Status {
    pub fn try_auto_slave_response_with_frame(
        &mut self,
        port_name: String,
        bytes: &[u8],
        cmd_tx: &flume::Sender<RuntimeCommand>,
    ) -> Option<(LogEntry, LogEntry)> {
        if bytes.len() < 5 {
            return None;
        }
        let sid = bytes[0];
        let func = bytes[1];
        match func {
            0x01..=0x06 | 0x0F | 0x10 => {}
            _ => return None,
        };

        // Parse request address and count (required for read functions)
        if bytes.len() < 6 {
            return None;
        }
        let req_addr = u16::from_be_bytes([bytes[2], bytes[3]]);
        let req_count = u16::from_be_bytes([bytes[4], bytes[5]]);

        // Map function to RegisterMode
        let req_mode = match func {
            0x01 => RegisterMode::Coils,
            0x02 => RegisterMode::DiscreteInputs,
            0x03 => RegisterMode::Holding,
            0x04 => RegisterMode::Input,
            _ => return None,
        };

        // Only auto-respond if this port has a simulated master entry that
        // explicitly owns the slave id, matches the requested register type,
        // and whose configured range covers the requested address/count.
        // Use Option<RegisterEntry> so we can short-circuit on first match and
        // avoid extra scanning.
        let mut matched: Option<RegisterEntry> = None;
        let mut matched_interval_ms: Option<u64> = None;
        let mut forms_iter = Vec::new();
        // Prefer the saved per-port form for this port. Only consider the
        // global `self.subpage_form` when the provided port is the currently
        // selected port in the UI. This avoids matching against unrelated
        // forms from other ports which can cause accidental auto-responses.
        if let Some(ps) = self.per_port.states.get(&port_name) {
            if let Some(form) = ps.subpage_form.as_ref() {
                if form.loop_enabled {
                    forms_iter.push(form);
                }
            }
        }
        // If the port_name corresponds to the currently selected port, allow
        // using the live `self.subpage_form` as well.
        if let Some(selected_info) = self.ports.list.get(self.ui.selected) {
            if selected_info.port_name == port_name {
                if let Some(form) = self.ui.subpage_form.as_ref() {
                    if form.loop_enabled {
                        forms_iter.push(form);
                    }
                }
            }
        }
        'outer: for form in forms_iter.iter() {
            for reg in form.registers.iter() {
                // Combine all matching criteria into a single predicate to avoid
                // accidental partial matches when multiple checks are evaluated
                // separately.
                let reg_start = reg.address as u32;
                let reg_len = reg.length as u32;
                let req_start = req_addr as u32;
                let req_len = req_count as u32;

                let is_match = reg.role == crate::protocol::status::EntryRole::Master
                    && reg.slave_id == sid
                    && reg.mode == req_mode
                    && req_len != 0
                    && req_start >= reg_start
                    && (req_start + req_len) <= (reg_start + reg_len);

                if is_match {
                    matched = Some(reg.clone());
                    matched_interval_ms = Some(form.global_interval_ms);
                    break 'outer;
                }
            }
        }

        matched.as_ref()?;

        // Build a temporary slave context populated only with the matched
        // register entry's values for the requested range and mode. This
        // prevents unrelated registers (or master-configured values) from
        // leaking into responses for a different function type.
        let matched_reg = matched.unwrap();
        let mut local_ctx: ModbusStorageSmall = ModbusStorageSmall::default();
        let addr = matched_reg.address as usize;
        let len = matched_reg.length as usize;
        match matched_reg.mode {
            crate::protocol::status::RegisterMode::Holding => {
                for i in 0..len {
                    if addr + i < local_ctx.holdings.len() && i < matched_reg.values.len() {
                        local_ctx.holdings[addr + i] = matched_reg.values[i];
                    }
                }
            }
            crate::protocol::status::RegisterMode::Input => {
                for i in 0..len {
                    if addr + i < local_ctx.inputs.len() && i < matched_reg.values.len() {
                        local_ctx.inputs[addr + i] = matched_reg.values[i];
                    }
                }
            }
            crate::protocol::status::RegisterMode::Coils => {
                for i in 0..len {
                    if addr + i < local_ctx.coils.len() {
                        let v = if i < matched_reg.values.len() {
                            matched_reg.values[i] != 0
                        } else {
                            false
                        };
                        local_ctx.coils[addr + i] = v;
                    }
                }
            }
            crate::protocol::status::RegisterMode::DiscreteInputs => {
                for i in 0..len {
                    if addr + i < local_ctx.discretes.len() {
                        let v = if i < matched_reg.values.len() {
                            matched_reg.values[i] != 0
                        } else {
                            false
                        };
                        local_ctx.discretes[addr + i] = v;
                    }
                }
            }
        }

        let mut response_buf: Vec<u8> = Vec::new();
        let mut frame = ModbusFrame::new(sid, bytes, ModbusProto::Rtu, &mut response_buf);
        if frame.parse().is_err() {
            return None;
        }

        let resp_res: anyhow::Result<Option<Vec<u8>>> = match func {
            0x01 => build_slave_coils_response(&mut frame, &mut local_ctx),
            0x02 => build_slave_discrete_inputs_response(&mut frame, &mut local_ctx),
            0x03 => build_slave_holdings_response(&mut frame, &mut local_ctx),
            0x04 => build_slave_inputs_response(&mut frame, &mut local_ctx),
            0x05 | 0x06 | 0x0F | 0x10 => {
                todo!("Write handling for write functions in auto-slave");
            }
            _ => return None,
        };

        let ret: Vec<u8> = match resp_res {
            Ok(Some(v)) => v,
            Ok(None) => {
                if frame.response_required {
                    if frame.finalize_response().is_err() {
                        return None;
                    }
                    frame.response.clone()
                } else {
                    return None;
                }
            }
            Err(_) => return None,
        };

        if ret.is_empty() {
            return None;
        }

        if ret.is_empty() {
            return None;
        }

        // Debounce: avoid auto-responding repeatedly to high-frequency incoming
        // requests. Use the matched form's global_interval_ms as the time window.
        if let Some(interval_ms) = matched_interval_ms {
            let now = std::time::Instant::now();
            // Clean expired entries
            while let Some((_, t)) = self.recent.auto_requests.front() {
                if now.duration_since(*t).as_millis() as u64 > interval_ms {
                    self.recent.auto_requests.pop_front();
                } else {
                    break;
                }
            }
            // Check if an identical request was recently auto-responded
            // Compare only sid/func/address/count for debounce (ignore CRC/extra bytes)
            // use existing `req_addr` and `req_count` from outer scope
            let seen_recent = self.recent.auto_requests.iter().any(|(bts, _)| {
                if bts.len() < 6 {
                    return false;
                }
                let b_sid = bts[0];
                let b_func = bts[1];
                let b_addr = u16::from_be_bytes([bts[2], bts[3]]);
                let b_qty = u16::from_be_bytes([bts[4], bts[5]]);
                b_sid == sid && b_func == func && b_addr == req_addr && b_qty == req_count
            });
            if seen_recent {
                return None;
            }
        }

        // Send the response to the runtime
        let _ = cmd_tx.send(RuntimeCommand::Write(ret.clone()));

        // Record that we recently auto-sent these bytes so the runtime's
        // FrameSent handler can dedupe the emitted FrameSent event.
        self.recent
            .auto_sent
            .push_back((ret.clone(), std::time::Instant::now()));

        // Also remember the incoming request signature for debounce purposes
        self.recent
            .auto_requests
            .push_back((bytes.to_vec(), std::time::Instant::now()));

        // Build the sent log entry but do NOT push it into logs here; return
        // it to the caller so the caller can append both entries in the
        // correct order (recv then sent).
        let sent_hex = ret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let sent_entry = LogEntry {
            when: Local::now(),
            raw: format!("{}: {sent_hex}", lang().protocol.modbus.log_sent_frame),
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
        };

        let recv_hex = bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let recv_entry = LogEntry {
            when: Local::now(),
            raw: format!("{}: {recv_hex}", lang().protocol.modbus.log_recv_match),
            parsed: Some(ParsedRequest {
                origin: "master".into(),
                rw: "R".into(),
                command: format!("func_{func:02X}"),
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
        };

        Some((recv_entry, sent_entry))
    }
}
