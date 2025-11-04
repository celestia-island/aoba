//! Workflow executor
//!
//! Executes TOML workflows in either screen-capture or drill-down mode.

use crate::mock_state::{
    init_mock_state, save_mock_state_to_file, set_mock_state, verify_mock_state,
};
use crate::placeholder::{
    clear_placeholders, generate_value, replace_placeholders, set_placeholder,
};
use crate::renderer::render_tui_to_string;
use crate::workflow::{Workflow, WorkflowStep};
use anyhow::Result;
use std::time::Duration;

/// Path to snapshots directory relative to executor
const SNAPSHOT_PATH: &str = "../snapshots";

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Screen capture only - test rendering with mocked state
    ScreenCaptureOnly,
    /// Drill down - test real TUI with keyboard input
    /// 
    /// **Deprecated**: This mode is no longer supported. Use ScreenCaptureOnly instead.
    #[deprecated(note = "DrillDown mode is no longer supported. Use ScreenCaptureOnly mode with TestBackend rendering.")]
    DrillDown,
}

/// Execution context
pub struct ExecutionContext {
    pub mode: ExecutionMode,
    pub port1: String,
    pub port2: String,
    pub debug: bool,
}

/// Execute a complete workflow
pub async fn execute_workflow(ctx: &ExecutionContext, workflow: &Workflow) -> Result<()> {
    log::info!("🚀 Starting workflow execution: {}", workflow.manifest.id);
    log::info!("   Mode: {:?}", ctx.mode);

    // Clear placeholders from previous runs
    clear_placeholders();

    // Initialize mock state if in screen-capture mode
    if ctx.mode == ExecutionMode::ScreenCaptureOnly {
        init_mock_state();
        log::info!("🔧 Initialized mock state for screen-capture testing");
    } else {
        anyhow::bail!(
            "DrillDown mode is deprecated. Please use --screen-capture-only mode with TestBackend rendering"
        );
    }

    // Execute init_order steps
    log::info!("📋 Executing init_order steps...");
    for step_name in &workflow.manifest.init_order {
        log::info!("  ▶️  Step: {}", step_name);

        let steps = workflow
            .workflow
            .get(step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in workflow", step_name))?;

        execute_step_sequence(ctx, &workflow.manifest.id, steps).await?;
    }

    // Execute recycle_order steps if any
    if !workflow.manifest.recycle_order.is_empty() {
        log::info!("📋 Executing recycle_order steps...");
        for step_name in &workflow.manifest.recycle_order {
            log::info!("  ▶️  Step: {}", step_name);

            let steps = workflow
                .workflow
                .get(step_name)
                .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in workflow", step_name))?;

            execute_step_sequence(ctx, &workflow.manifest.id, steps).await?;
        }
    }

    // Save mock state if in debug mode
    if ctx.debug {
        save_mock_state_to_file("/tmp/tui_e2e_new_mock_state.json")?;
    }

    log::info!("✅ Workflow execution completed successfully");
    Ok(())
}

/// Execute a sequence of workflow steps
async fn execute_step_sequence(
    ctx: &ExecutionContext,
    workflow_id: &str,
    steps: &[WorkflowStep],
) -> Result<()> {
    for (i, step) in steps.iter().enumerate() {
        if let Some(desc) = &step.description {
            log::debug!("    [{}] {}", i, desc);
        }

        execute_single_step(ctx, workflow_id, step).await?;
    }
    Ok(())
}

/// Execute a single workflow step
async fn execute_single_step(
    _ctx: &ExecutionContext,
    workflow_id: &str,
    step: &WorkflowStep,
) -> Result<()> {
    // Handle input generation and storage
    if let Some(input_type) = &step.input {
        if let Some(index) = step.index {
            // Generate value based on input type
            let value = if let Some(val) = &step.value {
                // Use provided value
                match val {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => val.to_string(),
                }
            } else {
                // Generate random value
                generate_value(input_type, None)
            };

            set_placeholder(index, value.clone());
        }
    }

    // Handle mock state operations (screen-capture mode only)
    // Set mock state value
    if let Some(path) = &step.mock_path {
        let value = if let Some(template) = &step.mock_set_value_with_placeholder {
            let value_str = replace_placeholders(template)?;
            serde_json::json!(value_str)
        } else if let Some(value) = &step.mock_set_value {
            value.clone()
        } else {
            anyhow::bail!("mock_path specified but no value provided");
        };

        set_mock_state(path, value)?;
    }

    // Verify mock state value
    if let Some(path) = &step.mock_verify_path {
        let expected = step.mock_verify_value.as_ref().ok_or_else(|| {
            anyhow::anyhow!("mock_verify_path specified but no expected value")
        })?;
        verify_mock_state(path, expected)?;
    }

    // Handle screen verification - render to TestBackend and create snapshot
    if step.verify.is_some() || step.verify_with_placeholder.is_some() {
        // Render the TUI to a string
        let screen_content = render_tui_to_string(120, 40)?;

        // Determine expected text
        let expected_text = if let Some(template) = &step.verify_with_placeholder {
            replace_placeholders(template)?
        } else if let Some(text) = &step.verify {
            text.clone()
        } else {
            String::new()
        };

        // Create snapshot name from workflow ID and step description
        let snapshot_name = if let Some(desc) = &step.description {
            format!("{}_{}", workflow_id, sanitize_snapshot_name(desc))
        } else {
            workflow_id.to_string()
        };

        // Use insta to assert snapshot
        insta::with_settings!({
            snapshot_path => SNAPSHOT_PATH,
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(snapshot_name.as_str(), screen_content);
        });

        // Also verify the expected text is present
        if let Some(line_num) = step.at_line {
            let lines: Vec<&str> = screen_content.lines().collect();
            if line_num >= lines.len() {
                anyhow::bail!(
                    "Line {} out of bounds (screen has {} lines)",
                    line_num,
                    lines.len()
                );
            }
            let actual_line = lines[line_num];
            if !actual_line.contains(&expected_text) {
                anyhow::bail!(
                    "Screen verification failed at line {}:\n  Expected: '{}'\n  Actual: '{}'",
                    line_num,
                    expected_text,
                    actual_line
                );
            }
        } else if !screen_content.contains(&expected_text) {
            anyhow::bail!(
                "Screen verification failed: expected text '{}' not found",
                expected_text
            );
        }

        log::debug!("✅ Screen snapshot verified: {}", &snapshot_name);
    }

    // Handle sleep
    if let Some(sleep_ms) = step.sleep_ms {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }

    Ok(())
}

/// Sanitize a description string to create a valid snapshot name
fn sanitize_snapshot_name(desc: &str) -> String {
    desc.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

