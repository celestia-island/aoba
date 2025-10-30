use serde_json::json;

use super::super::status_paths::page_type_path;
use ci_utils::CursorAction;

/// Validate that the UI reports the expected page type.
#[allow(dead_code)]
pub fn check_page(expected_page: &str) -> Vec<CursorAction> {
    vec![CursorAction::CheckStatus {
        description: format!("Page is {expected_page}"),
        path: page_type_path().to_string(),
        expected: json!(expected_page),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }]
}
