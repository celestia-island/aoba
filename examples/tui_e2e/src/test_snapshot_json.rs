//! Test script to verify the new JSON snapshot format with step names
use aoba_ci_utils::{
    ExecutionMode, SearchCondition, SnapshotContext, SnapshotDefinition,
    verify_screen_with_json_rules,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Test 1: Create a snapshot context for testing
    let ctx = SnapshotContext::new(
        ExecutionMode::Normal,
        "single_station/master_modes/coils".to_string(),
        "test_basic_configuration".to_string(),
    );

    // Test loading snapshot definitions
    match ctx.load_snapshot_definitions() {
        Ok(definitions) => {
            println!(
                "✅ Successfully loaded {} snapshot definitions",
                definitions.len()
            );

            // Print the first few definitions for verification
            for (i, def) in definitions.iter().take(3).enumerate() {
                println!("Definition {}: {}", i, def.description);
                println!("  Name: {}", def.name);
                println!("  Lines: {:?}", def.line);
                println!("  Search conditions: {}", def.search.len());
                for condition in &def.search {
                    match condition {
                        SearchCondition::Text { value, negate } => {
                            println!("    - Text: '{}' (negate: {})", value, negate);
                        }
                        SearchCondition::CursorLine { value } => {
                            println!("    - Cursor line: {}", value);
                        }
                        SearchCondition::Placeholder { value, pattern } => {
                            println!("    - Placeholder: '{}' (pattern: {:?})", value, pattern);
                        }
                    }
                }
            }

            // Test finding by name
            println!("\n📌 Testing find by name...");
            if let Ok(found) = SnapshotContext::find_definition_by_name(
                &definitions,
                "step_00_snapshot一次tmpvcom1_与_tmpvcom2_应当在屏幕上",
            ) {
                println!("✅ Found step by name: {}", found.description);
            } else {
                println!("❌ Failed to find step by name");
            }
        }
        Err(e) => {
            println!("❌ Failed to load snapshot definitions: {}", e);
            println!("   This is expected if the JSON files haven't been updated yet");
        }
    }

    // Test 2: Test the standalone verification function
    println!("\n📌 Testing standalone verification function...");
    const TEST_JSON: &str = r#"[
        {
            "name": "test_step_01",
            "description": "Test step 1",
            "line": [0, 1],
            "search": [
                {
                    "type": "text",
                    "value": "Hello World"
                }
            ]
        }
    ]"#;

    let test_screen = "Hello World\nLine 2\n";
    match verify_screen_with_json_rules(test_screen, TEST_JSON, "test_step_01") {
        Ok(()) => println!("✅ Standalone verification passed"),
        Err(e) => println!("❌ Standalone verification failed: {}", e),
    }

    // Test 3: Verify mismatch detection
    println!("\n📌 Testing mismatch detection...");
    let bad_screen = "Goodbye World\nLine 2\n";
    match verify_screen_with_json_rules(bad_screen, TEST_JSON, "test_step_01") {
        Ok(()) => println!("❌ Should have failed but passed"),
        Err(e) => println!("✅ Correctly detected mismatch: {}", e),
    }

    // Test 4: Save snapshot definitions (for generation mode) with names
    println!("\n📌 Testing save with names...");
    let test_definitions = vec![
        SnapshotDefinition {
            name: "test_entry_page".to_string(),
            description: "Test definition 1".to_string(),
            line: vec![0, 1],
            search: vec![
                SearchCondition::Text {
                    value: "Test Text".to_string(),
                    negate: false,
                },
                SearchCondition::CursorLine { value: 1 },
            ],
        },
        SnapshotDefinition {
            name: "test_config_page".to_string(),
            description: "Test definition 2".to_string(),
            line: vec![2, 3],
            search: vec![SearchCondition::Placeholder {
                value: "{{0b#001}}".to_string(),
                pattern: aoba_ci_utils::snapshot::PlaceholderPattern::Exact,
            }],
        },
    ];

    if let Err(e) = ctx.save_snapshot_definitions(&test_definitions) {
        println!("❌ Failed to save test definitions: {}", e);
    } else {
        println!("✅ Successfully saved test definitions with names");
    }

    Ok(())
}
