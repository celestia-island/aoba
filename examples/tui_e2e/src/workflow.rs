//! Workflow data structures
//!
//! Defines the structure of TOML workflow files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete workflow definition from TOML file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub manifest: Manifest,
    pub workflow: HashMap<String, Vec<WorkflowStep>>,
}

/// Workflow manifest - metadata and execution order
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub id: String,
    pub description: String,

    /// For single-station tests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station_id: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub register_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_address: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub register_count: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_master: Option<bool>,

    /// For multi-station tests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stations: Option<Vec<StationConfig>>,

    /// Execution order for initial setup
    pub init_order: Vec<String>,

    /// Execution order for repeated verification cycles
    #[serde(default)]
    pub recycle_order: Vec<String>,
}

/// Station configuration for multi-station tests
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StationConfig {
    pub station_id: u8,
    pub register_type: String,
    pub start_address: u16,
    pub register_count: u16,
}

/// Individual workflow step
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowStep {
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Key to press (e.g., "enter", "down", "ctrl-s")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Number of times to repeat the key press
    #[serde(skip_serializing_if = "Option::is_none")]
    pub times: Option<u32>,

    /// Input type and value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,

    /// Screen verification - exact text match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<String>,

    /// Line number to verify text at
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_line: Option<usize>,

    /// Screen verification with placeholder substitution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_with_placeholder: Option<String>,

    /// Cursor position verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_at_line: Option<usize>,

    /// Sleep duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep_ms: Option<u64>,

    /// Mock state path (e.g., "ports['/tmp/vcom1'].enabled")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_path: Option<String>,

    /// Mock state value to set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_set_value: Option<serde_json::Value>,

    /// Mock state value with placeholder
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_set_value_with_placeholder: Option<String>,

    /// Mock state verification path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_verify_path: Option<String>,

    /// Expected mock state value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_verify_value: Option<serde_json::Value>,

    /// Trigger name for custom actions (e.g., "match_master_registers")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,

    /// Trigger parameters (passed as JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_params: Option<serde_json::Value>,
}
