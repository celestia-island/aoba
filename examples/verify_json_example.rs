#!/usr/bin/env rust-script
//! Simple example demonstrating the new JSON-based screenshot verification
//!
//! This example shows how to use the `verify_screen_with_json_rules` function
//! to verify terminal screenshots against JSON rule definitions loaded via `include_str!`.

use anyhow::Result;

fn main() -> Result<()> {
    // Example 1: Simple text matching
    println!("📝 Example 1: Simple text matching");
    
    const RULES_1: &str = r#"[
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
    
    let screen_1 = "Header Line\n\n/tmp/vcom1\n/tmp/vcom2\nFooter";
    
    match aoba_ci_utils::verify_screen_with_json_rules(screen_1, RULES_1, "test_entry_page") {
        Ok(()) => println!("✅ Example 1 passed: Found both ports"),
        Err(e) => println!("❌ Example 1 failed: {}", e),
    }
    
    // Example 2: Cursor line matching
    println!("\n📝 Example 2: Cursor line matching");
    
    const RULES_2: &str = r#"[
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
    
    let screen_2 = "Header\nLine 1\n> Enter Business Configuration\nLine 3";
    
    match aoba_ci_utils::verify_screen_with_json_rules(screen_2, RULES_2, "test_cursor_position") {
        Ok(()) => println!("✅ Example 2 passed: Cursor found at correct position"),
        Err(e) => println!("❌ Example 2 failed: {}", e),
    }
    
    // Example 3: Placeholder matching
    println!("\n📝 Example 3: Placeholder matching");
    
    const RULES_3: &str = r#"[
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
    
    let screen_3 = "Register 1: {{0b#001}}\nRegister 2: {{0b#002}}\nRegister 3: {{0b#003}}";
    
    match aoba_ci_utils::verify_screen_with_json_rules(screen_3, RULES_3, "test_register_values") {
        Ok(()) => println!("✅ Example 3 passed: Found placeholder"),
        Err(e) => println!("❌ Example 3 failed: {}", e),
    }
    
    // Example 4: Negation (text should NOT be present)
    println!("\n📝 Example 4: Negation test");
    
    const RULES_4: &str = r#"[
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
    
    let screen_4 = "Success message\nOperation completed";
    
    match aoba_ci_utils::verify_screen_with_json_rules(screen_4, RULES_4, "test_negation") {
        Ok(()) => println!("✅ Example 4 passed: No error message found"),
        Err(e) => println!("❌ Example 4 failed: {}", e),
    }
    
    // Example 5: Step not found error
    println!("\n📝 Example 5: Invalid step name");
    
    match aoba_ci_utils::verify_screen_with_json_rules(screen_1, RULES_1, "nonexistent_step") {
        Ok(()) => println!("❌ Example 5 should have failed"),
        Err(e) => println!("✅ Example 5 correctly reported error: {}", e),
    }
    
    println!("\n✨ All examples completed!");
    Ok(())
}
