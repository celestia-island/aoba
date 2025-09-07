use chrono::Local;

use crate::{
    i18n::lang,
    protocol::modbus::{
        generate_pull_get_coils_request, generate_pull_get_discrete_inputs_request,
        generate_pull_get_holdings_request, generate_pull_get_inputs_request,
    },
    protocol::{
        runtime::RuntimeCommand,
        status::{RegisterMode, *},
    },
};

impl Status {
    pub fn drive_slave_polling(&mut self) {
        if self.busy.polling_paused {
            return;
        }
        let now = std::time::Instant::now();
        let mut deferred_logs: Vec<(Option<String>, LogEntry)> = Vec::new();
        for (idx, rt_opt) in self.ports.runtimes.iter_mut().enumerate() {
            let mut form_opt_idx = None; // 0 global, 1 per-port
            let mut p_name: Option<String> = None;
            if idx == self.ui.selected {
                if self.ui.subpage_active && self.ui.subpage_form.is_some() {
                    form_opt_idx = Some(0);
                }
            } else if let Some(p) = self.ports.list.get(idx) {
                p_name = Some(p.port_name.clone());
                if let Some(ps) = self.per_port.states.get_mut(&p.port_name) {
                    if ps.subpage_form.is_some() && ps.subpage_active {
                        form_opt_idx = Some(1);
                    }
                }
            }
            if form_opt_idx.is_none() {
                continue;
            }
            // Compute whether we should skip sending to this port because a per-port
            // subpage form exists or the form is marked passive (avoid sending to self).
            // Do this before taking any mutable borrows on self to prevent borrow conflicts.
            // Determine whether to skip sending based on master_passive Option and derived default
            let skip_self_send_candidate = if let Some(ref pname) = p_name {
                if let Some(ps) = self.per_port.states.get(pname) {
                    if let Some(ref f) = ps.subpage_form {
                        // If user set explicit Some(true/false), use it; otherwise derive default
                        if let Some(v) = f.master_passive {
                            v
                        } else {
                            // derived default: passive if any Master entries exist
                            let derived_default_passive = f
                                .registers
                                .iter()
                                .any(|r| r.role == crate::protocol::status::EntryRole::Master);
                            derived_default_passive
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else if let Some(f) = self.ui.subpage_form.as_ref() {
                if let Some(v) = f.master_passive {
                    v
                } else {
                    let derived_default_passive = f
                        .registers
                        .iter()
                        .any(|r| r.role == crate::protocol::status::EntryRole::Master);
                    derived_default_passive
                }
            } else {
                false
            };

            let form_opt: Option<&mut SubpageForm> = if form_opt_idx == Some(0) {
                self.ui.subpage_form.as_mut()
            } else if let Some(ref pname) = p_name {
                self.per_port
                    .states
                    .get_mut(pname)
                    .and_then(|ps| ps.subpage_form.as_mut())
            } else {
                None
            };
            let form = match form_opt {
                Some(f) => f,
                None => continue,
            };
            if idx == self.ui.selected {
                if !form.loop_enabled || !self.ui.subpage_active {
                    continue;
                }
            } else if !form.loop_enabled {
                continue;
            }
            let timeout_ms = form.global_timeout_ms;
            if let Some(in_idx) = form.in_flight_reg_index {
                if in_idx < form.registers.len() {
                    let mut timeout_log: Option<LogEntry> = None;
                    let mut need_advance = false;
                    {
                        let reg = &mut form.registers[in_idx];
                        if let Some(p) = reg.pending_requests.first() {
                            if now.duration_since(p.sent_at).as_millis() as u64 > timeout_ms {
                                let func = p.func;
                                let addr = p.address;
                                let cnt = p.count;
                                let sid = reg.slave_id;
                                timeout_log = Some(LogEntry {
                                    when: Local::now(),
                                    raw: format!(
                                        "{} func=0x{:02X} sid={} addr={} cnt={}",
                                        lang().protocol.modbus.log_req_timeout,
                                        func,
                                        sid,
                                        addr,
                                        cnt
                                    ),
                                    parsed: Some(ParsedRequest {
                                        origin: "master".into(),
                                        rw: "R".into(),
                                        command: format!("func_{func:02X}"),
                                        slave_id: sid,
                                        address: addr,
                                        length: cnt,
                                    }),
                                });
                                reg.pending_requests.clear();
                                reg.next_poll_at = now + std::time::Duration::from_millis(1000);
                                need_advance = true;
                            }
                        } else {
                            need_advance = true;
                        }
                    }
                    if let Some(le) = timeout_log {
                        let target = if idx == self.ui.selected {
                            None
                        } else {
                            p_name.clone()
                        };
                        deferred_logs.push((target, le));
                    }
                    if need_advance {
                        form.in_flight_reg_index = None;
                        if !form.registers.is_empty() {
                            form.poll_round_index = (in_idx + 1) % form.registers.len();
                        }
                    }
                } else {
                    form.in_flight_reg_index = None;
                }
            }
            if form.in_flight_reg_index.is_none() && !form.registers.is_empty() {
                let total = form.registers.len();
                let mut attempts = 0;
                let mut r_idx = form.poll_round_index % total;
                while attempts < total {
                    let mut dispatched = false;
                    if let Some(reg) = form.registers.get_mut(r_idx) {
                        if reg.role == EntryRole::Master
                            && now >= reg.next_poll_at
                            && reg.pending_requests.is_empty()
                        {
                            let qty = reg.length.min(125);
                            let mode_val = reg.mode; // avoid borrow issues
                            let gen_res = match mode_val {
                                RegisterMode::Coils => {
                                    generate_pull_get_coils_request(reg.slave_id, reg.address, qty)
                                }
                                RegisterMode::DiscreteInputs => {
                                    generate_pull_get_discrete_inputs_request(
                                        reg.slave_id,
                                        reg.address,
                                        qty,
                                    )
                                }
                                RegisterMode::Holding => generate_pull_get_holdings_request(
                                    reg.slave_id,
                                    reg.address,
                                    qty,
                                ),
                                RegisterMode::Input => {
                                    generate_pull_get_inputs_request(reg.slave_id, reg.address, qty)
                                }
                            };
                            if let Ok((req_obj, raw)) = gen_res {
                                if let Some(rt_some) = rt_opt.as_ref() {
                                    // If skip_self_send_candidate was computed true above,
                                    // avoid sending to the port because the app is also
                                    // simulating registers for this port.
                                    if skip_self_send_candidate {
                                        // Treat as dispatched so polling advances, but do not
                                        // send on the wire.
                                        if let Some(reg) = form.registers.get_mut(r_idx) {
                                            reg.req_total = reg.req_total.saturating_add(1);
                                            reg.next_poll_at =
                                                now + std::time::Duration::from_millis(1000);
                                        }
                                        dispatched = true;
                                    } else if rt_some
                                        .cmd_tx
                                        .send(RuntimeCommand::Write(raw.clone()))
                                        .is_ok()
                                    {
                                        // Log the sent frame so UI/status shows the outgoing request
                                        let hex = raw
                                            .iter()
                                            .map(|b| format!("{b:02x}"))
                                            .collect::<Vec<_>>()
                                            .join(" ");
                                        let sid = reg.slave_id;
                                        let func = match mode_val {
                                            RegisterMode::Coils => 0x01,
                                            RegisterMode::DiscreteInputs => 0x02,
                                            RegisterMode::Holding => 0x03,
                                            RegisterMode::Input => 0x04,
                                        };
                                        let entry = LogEntry {
                                            when: chrono::Local::now(),
                                            raw: format!(
                                                "{}: {hex}",
                                                lang().protocol.modbus.log_sent_frame
                                            ),
                                            parsed: Some(ParsedRequest {
                                                origin: "master".into(),
                                                rw: "W".into(),
                                                command: format!("func_{func:02X}"),
                                                slave_id: sid,
                                                address: reg.address,
                                                length: qty,
                                            }),
                                        };
                                        let target = if idx == self.ui.selected {
                                            None
                                        } else {
                                            p_name.clone()
                                        };
                                        deferred_logs.push((target, entry));

                                        reg.req_total = reg.req_total.saturating_add(1);
                                        let func = match mode_val {
                                            RegisterMode::Coils => 0x01,
                                            RegisterMode::DiscreteInputs => 0x02,
                                            RegisterMode::Holding => 0x03,
                                            RegisterMode::Input => 0x04,
                                        };
                                        reg.pending_requests.push(PendingRequest::new(
                                            func,
                                            reg.address,
                                            qty,
                                            now,
                                            req_obj,
                                        ));
                                        reg.next_poll_at =
                                            now + std::time::Duration::from_millis(1000);
                                        form.in_flight_reg_index = Some(r_idx);
                                        dispatched = true;
                                    }
                                }
                            }
                        }
                    }
                    if dispatched {
                        break;
                    }
                    attempts += 1;
                    r_idx = (r_idx + 1) % total;
                }
            }
        }
        for (target, le) in deferred_logs {
            if let Some(pn) = target {
                if let Some(ps) = self.per_port.states.get_mut(&pn) {
                    ps.logs.push(le);
                }
            } else {
                self.append_log(le);
            }
        }
    }
}
