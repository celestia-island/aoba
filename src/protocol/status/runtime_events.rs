use chrono::Local;

use crate::{
    i18n::lang,
    protocol::{runtime::RuntimeEvent, status::*},
};

impl Status {
    pub fn drain_runtime_events(&mut self) {
        if self.ports.is_empty() {
            return;
        }
        let selected = self.selected;
        let mut pending_logs: Vec<LogEntry> = Vec::new();
        let mut pending_error: Option<String> = None;
        // auto slave response now handled in separate module when needed (simplified here)
        for (idx, rt_opt) in self.port_runtimes.iter_mut().enumerate() {
            if let Some(rt) = rt_opt.as_mut() {
                let port_name = self.ports.get(idx).map(|p| p.port_name.clone());
                let _cmd_tx_clone = rt.cmd_tx.clone(); // kept for potential future use
                if let Some(ref pname) = port_name {
                    self.per_port_states
                        .entry(pname.clone())
                        .or_insert(PerPortState {
                            subpage_active: false,
                            subpage_form: None,
                            subpage_tab_index: 0,
                            logs: Vec::new(),
                            log_selected: 0,
                            log_view_offset: 0,
                            log_auto_scroll: true,
                            input_mode: InputMode::Ascii,
                            input_editing: false,
                            input_buffer: String::new(),
                            app_mode: AppMode::Modbus,
                        });
                }
                while let Ok(evt) = rt.evt_rx.try_recv() {
                    match evt {
                        RuntimeEvent::FrameReceived(bytes) => {
                            let raw_hex = bytes
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let mut consumed = false;
                            let nowi = std::time::Instant::now();
                            let form_opt: Option<&mut SubpageForm> = if idx == selected {
                                self.subpage_form.as_mut()
                            } else if let Some(ref pname) = port_name {
                                self.per_port_states
                                    .get_mut(pname)
                                    .and_then(|ps| ps.subpage_form.as_mut())
                            } else {
                                None
                            };
                            if let Some(form) = form_opt {
                                let mut advance_after: Option<usize> = None;
                                let registers_len_cache = form.registers.len();
                                for (reg_index, reg) in form.registers.iter_mut().enumerate() {
                                    if reg.role != EntryRole::Slave {
                                        continue;
                                    }
                                    let mut remove_indices: Vec<usize> = Vec::new();
                                    let pending_len = reg.pending_requests.len();
                                    for pi in 0..pending_len {
                                        if bytes.first().copied() != Some(reg.slave_id) {
                                            break;
                                        }
                                        if let Some(pending) = reg.pending_requests.get(pi) {
                                            if bytes.get(1).copied() != Some(pending.func) {
                                                continue;
                                            }
                                            let frame_vec = bytes.to_vec();
                                            if let Ok(mut saved_req) = pending.request.lock() {
                                                let parse_ok: Option<Vec<u8>> = match reg.mode {
                                                    RegisterMode::Coils => match crate::protocol::modbus::parse_pull_get_coils(&mut saved_req, frame_vec.clone(), pending.count) {
                                                        Ok(vb) => Some(vb.into_iter().map(|b| if b {1} else {0}).collect()),
                                                        Err(e) => { pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (coils): {e} raw={raw_hex}"), parsed: None }); remove_indices.push(pi); consumed = true; None }
                                                    },
                                                    RegisterMode::DiscreteInputs => match crate::protocol::modbus::parse_pull_get_discrete_inputs(&mut saved_req, frame_vec.clone(), pending.count) {
                                                        Ok(vb) => Some(vb.into_iter().map(|b| if b {1} else {0}).collect()),
                                                        Err(e) => { pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (discrete): {e} raw={raw_hex}"), parsed: None }); remove_indices.push(pi); consumed = true; None }
                                                    },
                                                    RegisterMode::Holding => match crate::protocol::modbus::parse_pull_get_holdings(&mut saved_req, frame_vec.clone()) {
                                                        Ok(v) => Some(v.into_iter().flat_map(|w| w.to_be_bytes()).collect()),
                                                        Err(e) => { pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (holding): {e} raw={raw_hex}"), parsed: None }); remove_indices.push(pi); consumed = true; None }
                                                    },
                                                    RegisterMode::Input => match crate::protocol::modbus::parse_pull_get_inputs(&mut saved_req, frame_vec.clone()) {
                                                        Ok(v) => Some(v.into_iter().flat_map(|w| w.to_be_bytes()).collect()),
                                                        Err(e) => { pending_logs.push(LogEntry { when: Local::now(), raw: format!("parse error (input): {e} raw={raw_hex}"), parsed: None }); remove_indices.push(pi); consumed = true; None }
                                                    },
                                                };
                                                if let Some(mut bts) = parse_ok {
                                                    match reg.mode {
                                                        RegisterMode::Holding
                                                        | RegisterMode::Input => {
                                                            if bts.len() % 2 != 0 {
                                                                bts.push(0);
                                                            }
                                                            let regs: Vec<u16> = bts
                                                                .chunks_exact(2)
                                                                .map(|c| {
                                                                    u16::from_be_bytes([c[0], c[1]])
                                                                })
                                                                .collect();
                                                            let mut vals = regs;
                                                            if vals.len() < reg.length as usize {
                                                                vals.resize(reg.length as usize, 0);
                                                            }
                                                            if vals.len() > reg.length as usize {
                                                                vals.truncate(reg.length as usize);
                                                            }
                                                            reg.values = vals;
                                                        }
                                                        _ => {
                                                            let mut vals: Vec<u16> = bts
                                                                .into_iter()
                                                                .map(|v| v as u16)
                                                                .collect();
                                                            if vals.len() < reg.length as usize {
                                                                vals.resize(reg.length as usize, 0);
                                                            }
                                                            if vals.len() > reg.length as usize {
                                                                vals.truncate(reg.length as usize);
                                                            }
                                                            reg.values = vals;
                                                        }
                                                    }
                                                    reg.req_success =
                                                        reg.req_success.saturating_add(1);
                                                    remove_indices.push(pi);
                                                    consumed = true;
                                                    pending_logs.push(LogEntry {
                                                        when: Local::now(),
                                                        raw: format!(
                                                            "{} sid={} func=0x{:02X} len={} raw={}",
                                                            lang().protocol.modbus.log_recv_match,
                                                            reg.slave_id,
                                                            pending.func,
                                                            bytes.len(),
                                                            raw_hex
                                                        ),
                                                        parsed: Some(ParsedRequest {
                                                            origin: "master".into(),
                                                            rw: "R".into(),
                                                            command: format!(
                                                                "func_{:02X}",
                                                                pending.func
                                                            ),
                                                            slave_id: reg.slave_id,
                                                            address: pending.address,
                                                            length: pending.count,
                                                        }),
                                                    });
                                                    let interval_ms = form.global_interval_ms;
                                                    reg.next_poll_at = nowi
                                                        + std::time::Duration::from_millis(
                                                            interval_ms,
                                                        );
                                                    if form.in_flight_reg_index == Some(reg_index) {
                                                        form.in_flight_reg_index = None;
                                                        advance_after = Some(reg_index);
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if let Some(done_idx) = advance_after {
                                        if registers_len_cache > 0 {
                                            form.poll_round_index =
                                                (done_idx + 1) % registers_len_cache;
                                        }
                                    }
                                    for &ri in remove_indices.iter().rev() {
                                        reg.pending_requests.remove(ri);
                                    }
                                    if consumed {
                                        break;
                                    }
                                }
                            }
                            if !consumed {
                                let sid = bytes.first().copied().unwrap_or(0);
                                let func = bytes.get(1).copied().unwrap_or(0);
                                let addr = if bytes.len() >= 4 {
                                    u16::from_be_bytes([bytes[2], bytes[3]])
                                } else {
                                    0
                                };
                                let qty_raw = if bytes.len() >= 6 {
                                    u16::from_be_bytes([bytes[4], bytes[5]])
                                } else {
                                    0
                                };
                                let effective_qty = match func {
                                    0x05 | 0x06 => 1,
                                    _ => qty_raw,
                                };
                                let unmatched_entry = LogEntry {
                                    when: Local::now(),
                                    raw: format!(
                                        "{}: {raw_hex}",
                                        lang().protocol.modbus.log_recv_unmatched
                                    ),
                                    parsed: Some(ParsedRequest {
                                        origin: "master".into(),
                                        rw: "R".into(),
                                        command: format!("func_{func:02X}"),
                                        slave_id: sid,
                                        address: addr,
                                        length: effective_qty,
                                    }),
                                };
                                if idx == selected {
                                    pending_logs.push(unmatched_entry);
                                } else if let Some(ref pname) = port_name {
                                    if let Some(ps) = self.per_port_states.get_mut(pname) {
                                        ps.logs.push(unmatched_entry);
                                    }
                                }
                            }
                        }
                        RuntimeEvent::FrameSent(bytes) => {
                            let hex = bytes
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let sid = bytes.first().copied().unwrap_or(0);
                            let func = bytes.get(1).copied().unwrap_or(0);
                            let addr = if bytes.len() >= 4 {
                                u16::from_be_bytes([bytes[2], bytes[3]])
                            } else {
                                0
                            };
                            let len_or_cnt = if bytes.len() >= 6 {
                                u16::from_be_bytes([bytes[4], bytes[5]])
                            } else {
                                0
                            };
                            let cmd = match func {
                                0x01 => "rd_coils",
                                0x02 => "rd_discrete",
                                0x03 => "rd_holdings",
                                0x04 => "rd_inputs",
                                0x05 => "wr_coil",
                                0x06 => "wr_holding",
                                0x0F => "wr_coils",
                                0x10 => "wr_holdings",
                                _ => "func",
                            };
                            let entry = LogEntry {
                                when: Local::now(),
                                raw: format!("{}: {hex}", lang().protocol.modbus.log_sent_frame),
                                parsed: Some(ParsedRequest {
                                    origin: "master".into(),
                                    rw: "W".into(),
                                    command: cmd.to_string(),
                                    slave_id: sid,
                                    address: addr,
                                    length: len_or_cnt,
                                }),
                            };
                            if idx == selected {
                                pending_logs.push(entry);
                            } else if let Some(ref pname) = port_name {
                                if let Some(ps) = self.per_port_states.get_mut(pname) {
                                    ps.logs.push(entry);
                                }
                            }
                        }
                        RuntimeEvent::Reconfigured(cfg) => {
                            let entry = LogEntry {
                                when: Local::now(),
                                raw: format!(
                                    "{}: baud={} data_bits={} stop_bits={} parity={:?}",
                                    lang().protocol.modbus.log_reconfigured,
                                    cfg.baud,
                                    cfg.data_bits,
                                    cfg.stop_bits,
                                    cfg.parity
                                ),
                                parsed: None,
                            };
                            if idx == selected {
                                pending_logs.push(entry);
                            } else if let Some(ref pname) = port_name {
                                if let Some(ps) = self.per_port_states.get_mut(pname) {
                                    ps.logs.push(entry);
                                }
                            }
                        }
                        RuntimeEvent::Error(e) => {
                            if idx == selected {
                                pending_error = Some(e);
                            } else if let Some(ref pname) = port_name {
                                if let Some(ps) = self.per_port_states.get_mut(pname) {
                                    ps.logs.push(LogEntry {
                                        when: Local::now(),
                                        raw: format!("Runtime error: {e}"),
                                        parsed: None,
                                    });
                                }
                            }
                        }
                        RuntimeEvent::Stopped => {}
                    }
                }
            }
        }
        for l in pending_logs {
            self.append_log(l);
        }
        if let Some(e) = pending_error {
            self.set_error(e);
        }
    }
}
