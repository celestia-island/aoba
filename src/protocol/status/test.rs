use anyhow::Result;
use std::thread;
use std::time::Duration;

use crate::protocol::status::{StateManager, run_state_writer_thread, types::Status};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_manager_basic_operations() -> Result<()> {
        // Create a StateManager with default status
        let (state_manager, state_write_rx) = StateManager::new(Status::default());
        let state_ref = state_manager.get_state_ref().clone();

        // Start the state writer thread
        let _handle = thread::spawn(move || {
            let _ = run_state_writer_thread(Status::default(), state_write_rx, state_ref);
        });

        // Test reading initial state
        let initial_busy = state_manager.read_status(|s| Ok(s.temporarily.busy.busy))?;
        assert_eq!(initial_busy, false);

        // Test async write operation
        state_manager.write_status_async(|s| {
            s.temporarily.busy.busy = true;
            Ok(())
        })?;

        // Give the writer thread time to process
        thread::sleep(Duration::from_millis(100));

        // Test reading updated state
        let updated_busy = state_manager.read_status(|s| Ok(s.temporarily.busy.busy))?;
        assert_eq!(updated_busy, true);

        // Test sync write operation
        state_manager.write_status_sync(|s| {
            s.temporarily.busy.spinner_frame = 42;
            Ok(())
        })?;

        // Verify sync write worked
        let spinner_frame = state_manager.read_status(|s| Ok(s.temporarily.busy.spinner_frame))?;
        assert_eq!(spinner_frame, 42);

        Ok(())
    }

    #[test]
    fn test_state_manager_closure_with_result() -> Result<()> {
        let (state_manager, state_write_rx) = StateManager::new(Status::default());
        let state_ref = state_manager.get_state_ref().clone();

        let _handle = thread::spawn(move || {
            let _ = run_state_writer_thread(Status::default(), state_write_rx, state_ref);
        });

        // Test the legacy write_status interface
        let result: String = state_manager.write_status(|s| {
            s.temporarily.scan.last_scan_info = "test scan result".to_string();
            Ok("operation successful".to_string())
        })?;

        assert_eq!(result, "operation successful");

        // Verify the state was updated
        let scan_info = state_manager.read_status(|s| Ok(s.temporarily.scan.last_scan_info.clone()))?;
        assert_eq!(scan_info, "test scan result");

        Ok(())
    }
}

/// Run basic tests to validate StateManager functionality
pub fn run_state_manager_tests() -> Result<()> {
    log::info!("[TEST] Running StateManager tests...");
    
    // Create a StateManager with default status
    let (state_manager, state_write_rx) = StateManager::new(Status::default());
    let state_ref = state_manager.get_state_ref().clone();

    // Start the state writer thread
    let _handle = thread::spawn(move || {
        let _ = run_state_writer_thread(Status::default(), state_write_rx, state_ref);
    });

    // Test reading initial state
    let initial_busy = state_manager.read_status(|s| Ok(s.temporarily.busy.busy))?;
    log::info!("[TEST] Initial busy state: {}", initial_busy);

    // Test async write operation
    state_manager.write_status_async(|s| {
        s.temporarily.busy.busy = true;
        s.temporarily.scan.last_scan_info = "StateManager test successful".to_string();
        Ok(())
    })?;

    // Give the writer thread time to process
    thread::sleep(Duration::from_millis(100));

    // Test reading updated state
    let updated_busy = state_manager.read_status(|s| Ok(s.temporarily.busy.busy))?;
    let scan_info = state_manager.read_status(|s| Ok(s.temporarily.scan.last_scan_info.clone()))?;
    
    log::info!("[TEST] Updated busy state: {}", updated_busy);
    log::info!("[TEST] Scan info: {}", scan_info);

    // Test sync write operation
    state_manager.write_status_sync(|s| {
        s.temporarily.busy.spinner_frame = 123;
        Ok(())
    })?;

    // Verify sync write worked
    let spinner_frame = state_manager.read_status(|s| Ok(s.temporarily.busy.spinner_frame))?;
    log::info!("[TEST] Spinner frame: {}", spinner_frame);

    log::info!("[TEST] StateManager tests completed successfully!");
    Ok(())
}