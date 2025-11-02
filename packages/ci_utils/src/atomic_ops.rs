/// Atomic operations with state-based verification for TUI E2E tests
///
/// This module provides robust atomic operations that verify state changes
/// after each action, eliminating race conditions and timing issues.
///
/// Key principles:
/// - Every keypress is followed by state verification
/// - State must stabilize (3 consecutive identical readings) before proceeding  
/// - Failed keypresses are retried (up to 3 attempts)
/// - All operations fail fast with clear error messages

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::fmt::Debug;
use tokio::time::{sleep, Duration};

use crate::{read_tui_status, ArrowKey, ExpectKeyExt, ExpectSession};

/// Maximum number of retry attempts for a single keypress
const MAX_KEYPRESS_RETRIES: usize = 3;

/// Number of consecutive stable readings required
const STABILITY_CHECKS: usize = 3;

/// Interval between state checks in milliseconds
const STATE_CHECK_INTERVAL_MS: u64 = 1000;

/// Extract a value from TUI status using a JSON path
/// 
/// # Arguments
/// * `status` - The TUI status object
/// * `path` - JSON path like "ports[0].enabled" or "page"
fn extract_state_value(status: &crate::TuiStatus, path: &str) -> Result<Value> {
    let json = serde_json::to_value(status)?;
    
    // Simple path parser - supports:
    // - "page" -> direct field access
    // - "ports[0].enabled" -> array index and field access
    let mut current = &json;
    
    for segment in path.split('.') {
        if segment.contains('[') {
            // Handle array index like "ports[0]"
            let parts: Vec<&str> = segment.split('[').collect();
            let field = parts[0];
            let index_str = parts[1].trim_end_matches(']');
            let index: usize = index_str.parse()
                .map_err(|_| anyhow!("Invalid array index: {}", index_str))?;
            
            current = current.get(field)
                .ok_or_else(|| anyhow!("Field not found: {}", field))?;
            current = current.get(index)
                .ok_or_else(|| anyhow!("Array index out of bounds: {}", index))?;
        } else {
            current = current.get(segment)
                .ok_or_else(|| anyhow!("Field not found: {}", segment))?;
        }
    }
    
    Ok(current.clone())
}

/// Wait for a state value to stabilize (same value for STABILITY_CHECKS consecutive readings)
///
/// # Arguments
/// * `path` - JSON path to the state value to monitor
/// * `max_checks` - Maximum number of checks before giving up
///
/// # Returns
/// The stabilized value, or error if state doesn't stabilize
async fn wait_for_state_stability(path: &str, max_checks: usize) -> Result<Value> {
    let mut stable_value: Option<Value> = None;
    let mut stable_count = 0;
    
    for _check_num in 0..max_checks {
        sleep(Duration::from_millis(STATE_CHECK_INTERVAL_MS)).await;
        
        let status = read_tui_status()?;
        let current_value = extract_state_value(&status, path)?;
        
        if let Some(ref prev_value) = stable_value {
            if *prev_value == current_value {
                stable_count += 1;
                log::debug!(
                    "State stability check {}/{} for '{}': {:?}",
                    stable_count,
                    STABILITY_CHECKS,
                    path,
                    current_value
                );
                
                if stable_count >= STABILITY_CHECKS {
                    log::info!("✅ State stabilized at '{}': {:?}", path, current_value);
                    return Ok(current_value);
                }
            } else {
                log::debug!(
                    "State changed at '{}': {:?} -> {:?}, resetting stability counter",
                    path,
                    prev_value,
                    current_value
                );
                stable_value = Some(current_value);
                stable_count = 1;
            }
        } else {
            stable_value = Some(current_value);
            stable_count = 1;
        }
    }
    
    Err(anyhow!(
        "State did not stabilize after {} checks (path: {})",
        max_checks,
        path
    ))
}

/// Verify that a state value changed from an initial value
///
/// # Arguments
/// * `path` - JSON path to monitor
/// * `initial_value` - The value before the action
/// * `timeout_checks` - Maximum number of checks
///
/// # Returns
/// true if state changed, false if it remained the same
async fn verify_state_changed(
    path: &str,
    initial_value: &Value,
    timeout_checks: usize,
) -> Result<bool> {
    for _ in 0..timeout_checks {
        sleep(Duration::from_millis(STATE_CHECK_INTERVAL_MS)).await;
        
        let status = read_tui_status()?;
        let current_value = extract_state_value(&status, path)?;
        
        if current_value != *initial_value {
            log::debug!("State changed at '{}': {:?} -> {:?}", path, initial_value, current_value);
            return Ok(true);
        }
    }
    
    log::warn!("State did not change at '{}' after {} checks", path, timeout_checks);
    Ok(false)
}

/// Atomic text input operation with state verification
///
/// Types text and verifies that the corresponding state field updates correctly.
///
/// # Arguments
/// * `session` - The TUI session
/// * `text` - Text to type
/// * `state_path` - JSON path to the field being edited (e.g., "ports[0].modbus_masters[0].station_id")
/// * `expected_value` - Optional expected value after typing (for verification)
///
/// # Returns
/// Ok if typing succeeded and state updated, Err otherwise
pub async fn atomic_type_text<T: ExpectSession>(
    session: &mut T,
    text: &str,
    state_path: &str,
    expected_value: Option<Value>,
) -> Result<()> {
    log::info!("📝 Atomic type text: '{}' into '{}'", text, state_path);
    
    // Read initial state
    let initial_status = read_tui_status()?;
    let initial_value = extract_state_value(&initial_status, state_path)?;
    
    for attempt in 1..=MAX_KEYPRESS_RETRIES {
        // Type the text
        for ch in text.chars() {
            session.send_char(ch)?;
        }
        
        // Wait for state to stabilize
        match wait_for_state_stability(state_path, STABILITY_CHECKS * 3).await {
            Ok(final_value) => {
                // Verify the value changed
                if final_value == initial_value {
                    log::warn!(
                        "Attempt {}: State unchanged after typing, retrying...",
                        attempt
                    );
                    continue;
                }
                
                // If expected value provided, verify it matches
                if let Some(ref expected) = expected_value {
                    if final_value != *expected {
                        return Err(anyhow!(
                            "State value mismatch: expected {:?}, got {:?}",
                            expected,
                            final_value
                        ));
                    }
                }
                
                log::info!(
                    "✅ Text typed successfully: '{}' -> {:?}",
                    text,
                    final_value
                );
                return Ok(());
            }
            Err(e) => {
                if attempt < MAX_KEYPRESS_RETRIES {
                    log::warn!("Attempt {}: Failed to stabilize state: {}, retrying...", attempt, e);
                    continue;
                } else {
                    return Err(anyhow!(
                        "Failed to type text after {} attempts: {}",
                        MAX_KEYPRESS_RETRIES,
                        e
                    ));
                }
            }
        }
    }
    
    Err(anyhow!("Unexpected: exceeded retry loop"))
}

/// Atomic cursor movement operation with state verification
///
/// Moves cursor and verifies that the cursor position state updates.
///
/// # Arguments
/// * `session` - The TUI session
/// * `direction` - Arrow key direction
/// * `count` - Number of times to press the arrow key
/// * `cursor_state_path` - JSON path to cursor position field
/// * `expected_position` - Optional expected cursor position enum value
///
/// # Returns
/// Ok if cursor moved and state updated, Err otherwise
pub async fn atomic_move_cursor<T: ExpectSession>(
    session: &mut T,
    direction: ArrowKey,
    count: usize,
    cursor_state_path: &str,
    expected_position: Option<Value>,
) -> Result<()> {
    log::info!(
        "🔄 Atomic cursor move: {:?} x{} (monitoring '{}')",
        direction,
        count,
        cursor_state_path
    );
    
    for i in 0..count {
        // Read current cursor state
        let initial_status = read_tui_status()?;
        let initial_cursor = extract_state_value(&initial_status, cursor_state_path)?;
        
        for attempt in 1..=MAX_KEYPRESS_RETRIES {
            // Press arrow key
            session.send_arrow(direction)?;
            
            // Verify cursor state changed
            match verify_state_changed(cursor_state_path, &initial_cursor, STABILITY_CHECKS).await {
                Ok(true) => {
                    // Wait for stability
                    match wait_for_state_stability(cursor_state_path, STABILITY_CHECKS * 2).await {
                        Ok(new_cursor) => {
                            log::debug!(
                                "Cursor moved ({}/{}): {:?} -> {:?}",
                                i + 1,
                                count,
                                initial_cursor,
                                new_cursor
                            );
                            break; // Success, move to next iteration
                        }
                        Err(e) => {
                            if attempt < MAX_KEYPRESS_RETRIES {
                                log::warn!("Cursor state didn't stabilize, retrying: {}", e);
                                continue;
                            } else {
                                return Err(anyhow!("Failed after {} attempts: {}", MAX_KEYPRESS_RETRIES, e));
                            }
                        }
                    }
                }
                Ok(false) => {
                    if attempt < MAX_KEYPRESS_RETRIES {
                        log::warn!("Cursor didn't move, retrying arrow key press...");
                        continue;
                    } else {
                        return Err(anyhow!(
                            "Cursor didn't move after {} attempts",
                            MAX_KEYPRESS_RETRIES
                        ));
                    }
                }
                Err(e) => {
                    return Err(anyhow!("Failed to verify cursor movement: {}", e));
                }
            }
        }
    }
    
    // If expected position provided, verify final position
    if let Some(ref expected) = expected_position {
        let final_status = read_tui_status()?;
        let final_cursor = extract_state_value(&final_status, cursor_state_path)?;
        
        if final_cursor != *expected {
            return Err(anyhow!(
                "Final cursor position mismatch: expected {:?}, got {:?}",
                expected,
                final_cursor
            ));
        }
    }
    
    log::info!("✅ Cursor moved successfully {} times", count);
    Ok(())
}

/// Atomic selection/option change operation with state verification
///
/// Changes a selection (like switching between Master/Slave) and verifies state update.
///
/// # Arguments
/// * `session` - The TUI session
/// * `key` - The key to press (typically Enter, Left, Right arrow)
/// * `state_path` - JSON path to the selection field
/// * `expected_value` - Expected value after the change
///
/// # Returns
/// Ok if selection changed and state updated, Err otherwise
pub async fn atomic_change_selection<T: ExpectSession>(
    session: &mut T,
    key: SelectionKey,
    state_path: &str,
    expected_value: Value,
) -> Result<()> {
    log::info!(
        "🔀 Atomic selection change: {:?} on '{}' -> {:?}",
        key,
        state_path,
        expected_value
    );
    
    // Read initial state
    let initial_status = read_tui_status()?;
    let initial_value = extract_state_value(&initial_status, state_path)?;
    
    for attempt in 1..=MAX_KEYPRESS_RETRIES {
        // Press the key
        match key {
            SelectionKey::Enter => session.send_enter()?,
            SelectionKey::Left => session.send_arrow(ArrowKey::Left)?,
            SelectionKey::Right => session.send_arrow(ArrowKey::Right)?,
            SelectionKey::Escape => session.send_escape()?,
        }
        
        // Wait for state to change
        match verify_state_changed(state_path, &initial_value, STABILITY_CHECKS).await {
            Ok(true) => {
                // Wait for stability
                match wait_for_state_stability(state_path, STABILITY_CHECKS * 2).await {
                    Ok(final_value) => {
                        if final_value != expected_value {
                            return Err(anyhow!(
                                "Selection value mismatch: expected {:?}, got {:?}",
                                expected_value,
                                final_value
                            ));
                        }
                        
                        log::info!("✅ Selection changed successfully: {:?}", final_value);
                        return Ok(());
                    }
                    Err(e) => {
                        if attempt < MAX_KEYPRESS_RETRIES {
                            log::warn!("State didn't stabilize, retrying: {}", e);
                            continue;
                        } else {
                            return Err(anyhow!("Failed after {} attempts: {}", MAX_KEYPRESS_RETRIES, e));
                        }
                    }
                }
            }
            Ok(false) => {
                if attempt < MAX_KEYPRESS_RETRIES {
                    log::warn!("Selection didn't change, retrying...");
                    continue;
                } else {
                    return Err(anyhow!(
                        "Selection didn't change after {} attempts",
                        MAX_KEYPRESS_RETRIES
                    ));
                }
            }
            Err(e) => {
                return Err(anyhow!("Failed to verify selection change: {}", e));
            }
        }
    }
    
    Err(anyhow!("Unexpected: exceeded retry loop"))
}

/// Key types for selection operations
#[derive(Debug, Clone, Copy)]
pub enum SelectionKey {
    Enter,
    Left,
    Right,
    Escape,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_simple_field() {
        let status = crate::TuiStatus {
            page: crate::TuiPage::Entry,
            ports: vec![],
            timestamp: "2025-11-02T00:00:00Z".to_string(),
        };
        
        let value = extract_state_value(&status, "page").unwrap();
        assert_eq!(value, json!("Entry"));
    }

    #[test]
    fn test_extract_array_field() {
        let status = crate::TuiStatus {
            page: crate::TuiPage::Entry,
            ports: vec![
                crate::TuiPort {
                    name: "/tmp/vcom1".to_string(),
                    enabled: true,
                    state: crate::PortState::Free,
                    modbus_masters: vec![],
                    modbus_slaves: vec![],
                    log_count: 0,
                },
            ],
            timestamp: "2025-11-02T00:00:00Z".to_string(),
        };
        
        let value = extract_state_value(&status, "ports[0].enabled").unwrap();
        assert_eq!(value, json!(true));
    }
}
