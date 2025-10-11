use clap::ArgMatches;
use serde::Serialize;

/// Helper to establish IPC connection if requested
pub fn setup_ipc(matches: &ArgMatches) -> Option<crate::protocol::ipc::IpcServer> {
    if let Some(channel_id) = matches.get_one::<String>("ipc-channel") {
        log::info!("IPC: Attempting to connect to channel: {channel_id}");
        match crate::protocol::ipc::IpcServer::connect(channel_id.clone()) {
            Ok(ipc) => {
                log::info!("IPC: Successfully connected");
                Some(ipc)
            }
            Err(err) => {
                log::warn!("IPC: Failed to connect: {err}");
                None
            }
        }
    } else {
        None
    }
}

#[derive(Serialize)]
struct PortInfo<'a> {
    #[serde(rename = "path")]
    port_name: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    guid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_port_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product: Option<String>,
}

pub fn run_one_shot_actions(matches: &ArgMatches) -> bool {
    if matches.get_flag("list-ports") {
        let ports_enriched = crate::protocol::tty::available_ports_enriched();

        let want_json = matches.get_flag("json");
        if want_json {
            let mut out: Vec<PortInfo> = Vec::new();

            // Get occupied ports from status if available
            let occupied_ports = crate::protocol::status::read_status(|status| {
                let mut ports = std::collections::HashSet::new();
                for (port_name, port_arc) in &status.ports.map {
                    if let Ok(port_data) = port_arc.read() {
                        if matches!(
                            &port_data.state,
                            crate::protocol::status::types::port::PortState::OccupiedByThis {
                                owner: _
                            }
                        ) {
                            ports.insert(port_name.clone());
                        }
                    }
                }
                Ok(ports)
            })
            .unwrap_or_default();

            for (p, extra) in ports_enriched.iter() {
                let status = if occupied_ports.contains(&p.port_name) {
                    "Occupied"
                } else {
                    "Free"
                };

                // Attempt to capture annotation if present in port_name (parenthetical)
                let ann = if p.port_name.contains('(') && p.port_name.contains(')') {
                    Some(p.port_name.clone())
                } else {
                    None
                };
                // Canonical: COMn if present, else basename for unix-like
                let canonical = compute_canonical(&p.port_name);
                let raw_type = Some(format!("{:?}", p.port_type));

                out.push(PortInfo {
                    port_name: &p.port_name,
                    status,
                    guid: extra.guid.clone(),
                    vid: extra.vid,
                    pid: extra.pid,
                    serial: extra.serial.clone(),
                    annotation: ann,
                    canonical,
                    raw_port_type: raw_type,
                    manufacturer: extra.manufacturer.clone(),
                    product: extra.product.clone(),
                });
            }
            if let Ok(s) = serde_json::to_string_pretty(&out) {
                println!("{s}");
            } else {
                // Fallback to plain listing
                for (p, _) in ports_enriched.iter() {
                    println!("{p_port}", p_port = p.port_name);
                }
            }
        } else {
            for (p, _) in ports_enriched.iter() {
                println!("{p_port}", p_port = p.port_name);
            }
        }
        return true;
    }

    // Handle modbus slave listen
    if let Some(port) = matches.get_one::<String>("slave-listen") {
        if let Err(err) = super::modbus::slave::handle_slave_listen(matches, port) {
            eprintln!("Error in slave-listen: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus slave listen persist
    if let Some(port) = matches.get_one::<String>("slave-listen-persist") {
        if let Err(err) = super::modbus::slave::handle_slave_listen_persist(matches, port) {
            eprintln!("Error in slave-listen-persist: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus slave poll (client mode - sends request)
    if let Some(port) = matches.get_one::<String>("slave-poll") {
        if let Err(err) = super::modbus::slave::handle_slave_poll(matches, port) {
            eprintln!("Error in slave-poll: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus slave poll persist (client mode - continuous polling)
    if let Some(port) = matches.get_one::<String>("slave-poll-persist") {
        if let Err(err) = super::modbus::slave::handle_slave_poll_persist(matches, port) {
            eprintln!("Error in slave-poll-persist: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus master provide
    if let Some(port) = matches.get_one::<String>("master-provide") {
        if let Err(err) = super::modbus::master::handle_master_provide(matches, port) {
            eprintln!("Error in master-provide: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus master provide persist
    if let Some(port) = matches.get_one::<String>("master-provide-persist") {
        if let Err(err) = super::modbus::master::handle_master_provide_persist(matches, port) {
            eprintln!("Error in master-provide-persist: {err}");
            std::process::exit(1);
        }
        return true;
    }

    false
}

fn compute_canonical(name: &str) -> Option<String> {
    // Try to find COM<number> anywhere (case-insensitive)
    let up = name.to_uppercase();
    if let Some(pos) = up.find("COM") {
        let tail = &up[pos + 3..];
        let mut num = String::new();
        for c in tail.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                break;
            }
        }
        if !num.is_empty() {
            return Some(format!("COM{num}"));
        }
    }
    // Fallback: take basename after last '/'
    if let Some(b) = name.rsplit('/').next() {
        return Some(b.to_string());
    }
    None
}
