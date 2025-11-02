use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use expectrl::Expect;

use super::super::config::{RegisterMode, RegisterModeExt};
use super::super::status_paths::{page_type_path, station_field_path};
use aoba_ci_utils::{
    atomic_edit_steps, atomic_type_text, read_tui_status, wait_for_state_value, AtomicEditStep,
    EditKeyCommand, ExpectSession, TerminalCapture,
};

const CONFIG_EDIT_ACTIVE_PATH: &str = "temporaries.config_edit.active";
const CONFIG_EDIT_BUFFER_PATH: &str = "temporaries.config_edit.buffer";
const CONFIG_EDIT_FIELD_KEY_PATH: &str = "temporaries.config_edit.field_key";
const CONFIG_EDIT_CURSOR_POS_PATH: &str = "temporaries.config_edit.cursor_pos";

fn extract_status_value(status: &aoba_ci_utils::TuiStatus, path: &str) -> Result<Value> {
    let json = serde_json::to_value(status)?;
    let mut current = &json;

    for segment in path.split('.') {
        if let Some((field, rest)) = segment.split_once('[') {
            let index_str = rest.trim_end_matches(']');
            let index: usize = index_str
                .parse()
                .map_err(|_| anyhow!("Invalid array index '{index_str}'"))?;

            current = current
                .get(field)
                .ok_or_else(|| anyhow!("Field '{field}' not found when reading '{path}'"))?;
            current = current
                .get(index)
                .ok_or_else(|| anyhow!("Index {index} out of bounds for '{field}'"))?;
        } else {
            current = current
                .get(segment)
                .ok_or_else(|| anyhow!("Field '{segment}' not found when reading '{path}'"))?;
        }
    }

    Ok(current.clone())
}

fn read_status_value(path: &str) -> Result<Value> {
    let status = read_tui_status()?;
    extract_status_value(&status, path)
}

async fn ensure_modbus_dashboard() -> Result<()> {
    wait_for_state_value(page_type_path(), json!("modbus_dashboard"), 5).await
}

async fn enter_config_edit<T: Expect + ExpectSession>(
    session: &mut T,
    description: &str,
    expected_field_key: Option<&str>,
) -> Result<String> {
    ensure_modbus_dashboard().await?;

    let steps = vec![AtomicEditStep {
        key: EditKeyCommand::Enter,
        monitor_paths: vec![CONFIG_EDIT_ACTIVE_PATH],
        description: format!("{description}_enter"),
    }];
    atomic_edit_steps(session, &steps).await?;

    wait_for_state_value(CONFIG_EDIT_ACTIVE_PATH, json!(true), 5).await?;

    let field_key_value = read_status_value(CONFIG_EDIT_FIELD_KEY_PATH)?;
    let field_key = field_key_value.as_str().unwrap_or_default().to_string();

    if let Some(expected) = expected_field_key {
        if field_key != expected {
            return Err(anyhow!(
                "Config edit focused field '{field_key}', expected '{expected}'"
            ));
        }
    }

    Ok(field_key)
}

async fn clear_config_edit_buffer<T: Expect + ExpectSession>(
    session: &mut T,
    description: &str,
) -> Result<()> {
    let mut guard = 0usize;

    loop {
        let buffer_value = read_status_value(CONFIG_EDIT_BUFFER_PATH)?;
        let buffer = buffer_value.as_str().unwrap_or_default();

        if buffer.is_empty() {
            break;
        }

        guard += 1;
        if guard > 64 {
            return Err(anyhow!(
                "Failed to clear config edit buffer after {guard} iterations"
            ));
        }

        let steps = vec![AtomicEditStep {
            key: EditKeyCommand::Backspace,
            monitor_paths: vec![CONFIG_EDIT_BUFFER_PATH, CONFIG_EDIT_CURSOR_POS_PATH],
            description: format!("{description}_clear_{guard}"),
        }];
        atomic_edit_steps(session, &steps).await?;
    }

    Ok(())
}

async fn commit_config_edit<T: Expect + ExpectSession>(
    session: &mut T,
    field_path: &str,
    expected_value: Value,
    description: &str,
) -> Result<()> {
    ensure_modbus_dashboard().await?;

    let steps = vec![AtomicEditStep {
        key: EditKeyCommand::Enter,
        monitor_paths: vec![CONFIG_EDIT_ACTIVE_PATH],
        description: format!("{description}_commit"),
    }];
    atomic_edit_steps(session, &steps).await?;

    wait_for_state_value(CONFIG_EDIT_ACTIVE_PATH, json!(false), 5).await?;
    ensure_modbus_dashboard().await?;
    wait_for_state_value(field_path, expected_value, 5).await
}

fn register_type_value(mode: RegisterMode) -> String {
    format!("{:?}", mode)
}

/// Configures the Station ID for a given station.
///
/// This function performs the following steps:
/// 1. Navigates to the Station ID field.
/// 2. Enters edit mode.
/// 3. Clears the existing value.
/// 4. Types the new station ID.
/// 5. Commits the change and verifies it via a status check.
pub async fn configure_station_id<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    station_id: u8,
    is_master: bool,
) -> Result<()> {
    let _ = cap;

    let path = station_field_path(port_name, is_master, station_index, "station_id");

    enter_config_edit(session, "station_id", Some("station_id")).await?;
    clear_config_edit_buffer(session, "station_id").await?;

    let id_text = station_id.to_string();
    atomic_type_text(
        session,
        &id_text,
        CONFIG_EDIT_BUFFER_PATH,
        Some(json!(id_text)),
    )
    .await?;

    commit_config_edit(session, &path, json!(station_id), "station_id").await
}

/// Configures the Register Type for a given station.
///
/// This function performs the following steps:
/// 1. Navigates to the Register Type field.
/// 2. Enters the selection mode.
/// 3. Selects the new register type.
/// 4. Commits the change and verifies it via a status check.
pub async fn configure_register_type<T: Expect + ExpectSession>(
    session: &mut T,
    _cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    register_mode: RegisterMode,
    is_master: bool,
) -> Result<()> {
    let field_path = station_field_path(port_name, is_master, station_index, "register_type");

    let field_key = enter_config_edit(session, "register_type", None).await?;
    if field_key != "register_type" {
        return Err(anyhow!(
            "Unexpected field key '{field_key}' while editing register type"
        ));
    }

    let (direction, count) = register_mode.arrow_from_default();

    if count > 0 {
        let steps = (0..count)
            .map(|idx| AtomicEditStep {
                key: EditKeyCommand::Arrow(direction),
                monitor_paths: vec![CONFIG_EDIT_BUFFER_PATH],
                description: format!("register_type_move_{idx}"),
            })
            .collect::<Vec<_>>();
        atomic_edit_steps(session, &steps).await?;
    }

    commit_config_edit(
        session,
        &field_path,
        json!(register_type_value(register_mode)),
        "register_type",
    )
    .await
}

/// Configures the Start Address for a given station.
pub async fn configure_start_address<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    start_address: u16,
    is_master: bool,
) -> Result<()> {
    let _ = cap;

    configure_numeric_field(
        session,
        port_name,
        station_index,
        start_address,
        is_master,
        "start_address",
        "start_address",
        "start_address",
    )
    .await
}

/// Configures the Register Count for a given station.
pub async fn configure_register_count<T: Expect + ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port_name: &str,
    station_index: usize,
    register_count: u16,
    is_master: bool,
) -> Result<()> {
    let _ = cap;

    configure_numeric_field(
        session,
        port_name,
        station_index,
        register_count,
        is_master,
        "register_count",
        "register_count",
        "register_count",
    )
    .await
}

/// Configures a numeric field (like Start Address or Register Count) for a station.
async fn configure_numeric_field<T: Expect + ExpectSession>(
    session: &mut T,
    port_name: &str,
    station_index: usize,
    value: u16,
    is_master: bool,
    field_name: &str,
    config_field_key: &str,
    description: &str,
) -> Result<()> {
    let path = station_field_path(port_name, is_master, station_index, field_name);

    enter_config_edit(session, description, Some(config_field_key)).await?;
    clear_config_edit_buffer(session, description).await?;

    let text = value.to_string();
    atomic_type_text(session, &text, CONFIG_EDIT_BUFFER_PATH, Some(json!(text))).await?;

    commit_config_edit(session, &path, json!(value), description).await
}
