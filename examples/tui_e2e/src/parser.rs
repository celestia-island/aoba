//! TOML workflow parser

use anyhow::{Context, Result};
use crate::workflow::Workflow;

/// Parse a workflow from TOML string
pub fn parse_workflow(toml_content: &str) -> Result<Workflow> {
    toml::from_str(toml_content)
        .context("Failed to parse workflow TOML")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let toml = r#"
[manifest]
id = "test_workflow"
description = "Test workflow"
station_id = 1
register_type = "Coils"
start_address = 0x0000
register_count = 10
is_master = true
init_order = ["step1", "step2"]
recycle_order = ["step3"]

[[workflow.step1]]
description = "Press enter"
key = "enter"

[[workflow.step2]]
verify = "Hello"
at_line = 1
"#;

        let workflow = parse_workflow(toml).unwrap();
        assert_eq!(workflow.manifest.id, "test_workflow");
        assert_eq!(workflow.manifest.init_order.len(), 2);
        assert!(workflow.workflow.contains_key("step1"));
        assert!(workflow.workflow.contains_key("step2"));
    }

    #[test]
    fn test_parse_multi_station_workflow() {
        let toml = r#"
[manifest]
id = "multi_test"
description = "Multi-station test"
is_master = true
init_order = ["step1"]

[[manifest.stations]]
station_id = 1
register_type = "Coils"
start_address = 0x0000
register_count = 10

[[manifest.stations]]
station_id = 2
register_type = "Holding"
start_address = 0x0010
register_count = 5

[[workflow.step1]]
key = "enter"
"#;

        let workflow = parse_workflow(toml).unwrap();
        assert_eq!(workflow.manifest.stations.as_ref().unwrap().len(), 2);
        assert_eq!(workflow.manifest.stations.as_ref().unwrap()[0].station_id, 1);
        assert_eq!(workflow.manifest.stations.as_ref().unwrap()[1].station_id, 2);
    }
}
