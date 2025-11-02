//! Integration test for JSON-based screenshot verification
//!
//! This test demonstrates the new screenshot verification mechanism that uses
//! JSON rule definitions with named steps instead of array indices.

use anyhow::Result;
use aoba_ci_utils::{verify_screen_with_json_rules, SearchCondition, SnapshotContext, SnapshotDefinition};

#[test]
fn test_simple_text_matching() -> Result<()> {
    const RULES: &str = r#"[
        {
            "name": "test_entry_page",
            "description": "Verify entry page shows port options",
            "line": [2, 3],
            "search": [
                {
                    "type": "text",
                    "value": "/tmp/vcom1"
                },
                {
                    "type": "text",
                    "value": "/tmp/vcom2"
                }
            ]
        }
    ]"#;
    
    let screen = "Header Line\n\n/tmp/vcom1\n/tmp/vcom2\nFooter";
    
    // Should succeed when both texts are found
    verify_screen_with_json_rules(screen, RULES, "test_entry_page")?;
    
    // Should fail when text is missing
    let bad_screen = "Header Line\n\n/tmp/vcom1\nFooter";
    assert!(verify_screen_with_json_rules(bad_screen, RULES, "test_entry_page").is_err());
    
    Ok(())
}

#[test]
fn test_cursor_line_matching() -> Result<()> {
    const RULES: &str = r#"[
        {
            "name": "test_cursor_position",
            "description": "Verify cursor is on the correct line",
            "line": [2],
            "search": [
                {
                    "type": "cursor_line",
                    "value": 2
                },
                {
                    "type": "text",
                    "value": "Enter Business Configuration"
                }
            ]
        }
    ]"#;
    
    let screen = "Header\nLine 1\n> Enter Business Configuration\nLine 3";
    
    // Should succeed when cursor is on line 2 (0-indexed)
    verify_screen_with_json_rules(screen, RULES, "test_cursor_position")?;
    
    // Should fail when cursor is on wrong line
    let bad_screen = "Header\n> Line 1\nEnter Business Configuration\nLine 3";
    assert!(verify_screen_with_json_rules(bad_screen, RULES, "test_cursor_position").is_err());
    
    Ok(())
}

#[test]
fn test_placeholder_matching() -> Result<()> {
    const RULES: &str = r#"[
        {
            "name": "test_register_values",
            "description": "Verify register placeholder values",
            "line": [0, 1, 2],
            "search": [
                {
                    "type": "placeholder",
                    "value": "{{0b#001}}",
                    "pattern": "exact"
                }
            ]
        }
    ]"#;
    
    let screen = "Register 1: {{0b#001}}\nRegister 2: {{0b#002}}\nRegister 3: {{0b#003}}";
    
    // Should succeed when placeholder is found
    verify_screen_with_json_rules(screen, RULES, "test_register_values")?;
    
    // Should fail when placeholder is missing
    let bad_screen = "Register 1: OFF\nRegister 2: {{0b#002}}\nRegister 3: {{0b#003}}";
    assert!(verify_screen_with_json_rules(bad_screen, RULES, "test_register_values").is_err());
    
    Ok(())
}

#[test]
fn test_negation() -> Result<()> {
    const RULES: &str = r#"[
        {
            "name": "test_negation",
            "description": "Verify certain text is NOT present",
            "line": [0, 1],
            "search": [
                {
                    "type": "text",
                    "value": "Error",
                    "negate": true
                }
            ]
        }
    ]"#;
    
    let screen = "Success message\nOperation completed";
    
    // Should succeed when text is NOT found
    verify_screen_with_json_rules(screen, RULES, "test_negation")?;
    
    // Should fail when text IS found (because negate is true)
    let bad_screen = "Error occurred\nOperation failed";
    assert!(verify_screen_with_json_rules(bad_screen, RULES, "test_negation").is_err());
    
    Ok(())
}

#[test]
fn test_step_not_found() {
    const RULES: &str = r#"[
        {
            "name": "test_step_1",
            "description": "Step 1",
            "line": [0],
            "search": [
                {
                    "type": "text",
                    "value": "test"
                }
            ]
        }
    ]"#;
    
    let screen = "test";
    
    // Should fail when step name doesn't exist
    let result = verify_screen_with_json_rules(screen, RULES, "nonexistent_step");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_load_definitions_from_str() -> Result<()> {
    const RULES: &str = r#"[
        {
            "name": "step_1",
            "description": "First step",
            "line": [0],
            "search": [
                {
                    "type": "text",
                    "value": "test"
                }
            ]
        },
        {
            "name": "step_2",
            "description": "Second step",
            "line": [1],
            "search": [
                {
                    "type": "text",
                    "value": "test2"
                }
            ]
        }
    ]"#;
    
    let definitions = SnapshotContext::load_snapshot_definitions_from_str(RULES)?;
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].name, "step_1");
    assert_eq!(definitions[1].name, "step_2");
    
    Ok(())
}

#[test]
fn test_find_definition_by_name() -> Result<()> {
    let definitions = vec![
        SnapshotDefinition {
            name: "step_a".to_string(),
            description: "Step A".to_string(),
            line: vec![0],
            search: vec![SearchCondition::Text {
                value: "A".to_string(),
                negate: false,
            }],
        },
        SnapshotDefinition {
            name: "step_b".to_string(),
            description: "Step B".to_string(),
            line: vec![1],
            search: vec![SearchCondition::Text {
                value: "B".to_string(),
                negate: false,
            }],
        },
    ];
    
    let found = SnapshotContext::find_definition_by_name(&definitions, "step_b")?;
    assert_eq!(found.name, "step_b");
    assert_eq!(found.description, "Step B");
    
    // Should fail when name doesn't exist
    assert!(SnapshotContext::find_definition_by_name(&definitions, "step_c").is_err());
    
    Ok(())
}
