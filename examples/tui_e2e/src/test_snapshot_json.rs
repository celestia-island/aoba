//! Test script to verify the new JSON snapshot format
use aoba_ci_utils::{ExecutionMode, SearchCondition, SnapshotContext, SnapshotDefinition};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a snapshot context for testing
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
                        SearchCondition::Placeholder { value } => {
                            println!("    - Placeholder: '{}'", value);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to load snapshot definitions: {}", e);
            println!("   This is expected if --generate-screenshots hasn't been run yet");
        }
    }

    // Test saving snapshot definitions (for generation mode)
    let test_definitions = vec![
        SnapshotDefinition {
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
            description: "Test definition 2".to_string(),
            line: vec![2, 3],
            search: vec![SearchCondition::Placeholder {
                value: "{{0b#001}}".to_string(),
            }],
        },
    ];

    if let Err(e) = ctx.save_snapshot_definitions(&test_definitions) {
        println!("❌ Failed to save test definitions: {}", e);
    } else {
        println!("✅ Successfully saved test definitions");
    }

    Ok(())
}
