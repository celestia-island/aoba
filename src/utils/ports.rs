/// Return a sorted list of available ports as (`port_name`, `port_type_string`).
#[must_use]
pub fn enumerate_ports() -> Vec<(String, String)> {
    // Use unified platform-specific enumeration which includes virtual port
    // detection when CI/debug hints are enabled.
    let mut ports = crate::protocol::tty::available_ports_sorted();
    ports.sort_by(|a, b| a.port_name.cmp(&b.port_name));
    ports
        .into_iter()
        .map(|p| (p.port_name, format!("{:?}", p.port_type)))
        .collect()
}
