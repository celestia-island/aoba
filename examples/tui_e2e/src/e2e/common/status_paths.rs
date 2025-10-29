use serde_json::to_string;

/// JSON path to the current page "type" field in the status dump.
pub(super) const PAGE_TYPE_PATH: &str = "page.type";

/// Build a JSONPath segment that filters ports by name.
pub(super) fn port_selector(port_name: &str) -> String {
    // serde_json::to_string already adds surrounding quotes and escapes special characters
    let literal = to_string(port_name).expect("Port name serialization should not fail");
    format!("ports[?(@.name == {literal})]")
}

/// Build a JSONPath for a field under the selected port.
pub(super) fn port_field_path(port_name: &str, field: &str) -> String {
    let selector = port_selector(port_name);
    if field.is_empty() {
        selector
    } else {
        format!("{selector}.{field}")
    }
}

/// Return the JSON key for master vs slave station collections.
pub(super) fn station_collection(is_master: bool) -> &'static str {
    if is_master {
        "modbus_masters"
    } else {
        "modbus_slaves"
    }
}

/// Build a JSONPath to a specific station entry under a port.
pub(super) fn station_path(port_name: &str, is_master: bool, station_index: usize) -> String {
    let collection = station_collection(is_master);
    let base = port_field_path(port_name, collection);
    format!("{base}[{station_index}]")
}

/// Build a JSONPath to a specific field of a station entry.
pub(super) fn station_field_path(
    port_name: &str,
    is_master: bool,
    station_index: usize,
    field: &str,
) -> String {
    let station = station_path(port_name, is_master, station_index);
    if field.is_empty() {
        station
    } else {
        format!("{station}.{field}")
    }
}

/// Return the JSONPath to the page type field.
pub(super) fn page_type_path() -> &'static str {
    PAGE_TYPE_PATH
}
