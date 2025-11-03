# JSON-Based Screenshot Verification

## Overview

This document describes the refactored TUI E2E screenshot capture and verification system, which uses JSON-based rule definitions with named steps instead of array indices.

## Key Changes

### 1. Named Steps Instead of Indices

Previously, tests referenced screenshot steps by array index (0, 1, 2, ...). This was error-prone when adding or removing steps. The new system uses explicit step names:

**Old (index-based):**

```rust
verify_screen_against_definitions(&screen, &definitions, 5)?; // What's step 5?
```

**New (name-based):**

```rust
verify_screen_with_json_rules(&screen, RULES, "step_05_configure_station")?;
```

### 2. JSON Structure

Each step now has a `name` field in addition to `description`:

```json
{
  "name": "step_00_entry_page",
  "description": "开头先快照一次，/tmp/vcom1 与 /tmp/vcom2 应当在屏幕上",
  "line": { "from": 2, "to": 3 },
  "search": [
    {
      "type": "text",
      "value": "/tmp/vcom1"
    }
  ]
}
```

### 3. Capture and Verify Modes

#### Capture Mode (Screenshot Generation)

When running in `ExecutionMode::GenerateScreenshots`:

1. Write mock global status to `/tmp/status.json`
2. Spawn TUI with `--debug-screen-capture` flag
3. Wait 3 seconds for rendering
4. Capture terminal output via expectrl + vt100
5. Kill TUI process

```rust
let ctx = SnapshotContext::new(
    ExecutionMode::GenerateScreenshots,
    "single_station/master_modes/coils".to_string(),
    "test".to_string(),
);

ctx.capture_or_verify(&mut session, &mut cap, predicted_state, "step_name").await?;
```

#### Verify Mode (Test Execution)

When running in `ExecutionMode::Normal`:

1. Capture actual terminal state
2. Load JSON rule definitions
3. Find the named step
4. Verify screen content matches all search conditions

```rust
let ctx = SnapshotContext::new(
    ExecutionMode::Normal,
    "single_station/master_modes/coils".to_string(),
    "test".to_string(),
);

ctx.capture_or_verify(&mut session, &mut cap, state, "step_name").await?;
```

### 4. Standalone Verification

For tests that embed JSON rules via `include_str!`, use the standalone function:

```rust
const RULES: &str = include_str!("../screenshots/single_station/master_modes/coils.json");

#[tokio::test]
async fn test_coils_configuration() -> Result<()> {
    let mut session = spawn_tui_session()?;
    let mut cap = TerminalCapture::with_size(TerminalSize::Large);
    
    // Perform actions...
    let screen = cap.capture(&mut session, "after_config").await?;
    
    // Verify against specific step by name
    verify_screen_with_json_rules(&screen, RULES, "step_05_configure_station")?;
    
    Ok(())
}
```

## Search Condition Types

### Text Search

Match exact text on specified lines:

```json
{
  "type": "text",
  "value": "/tmp/vcom1"
}
```

With negation (text should NOT be present):

```json
{
  "type": "text",
  "value": "Error",
  "negate": true
}
```

### Cursor Line

Verify cursor position on a specific line:

```json
{
  "type": "cursor_line",
  "value": 2
}
```

### Placeholder

Match placeholder values (for dynamic content):

```json
{
  "type": "placeholder",
  "value": "{{0b#001}}",
  "pattern": "exact"
}
```

Available patterns:

- `exact`: Match exact placeholder
- `any_boolean`: Match any boolean placeholder ({{0b#...}})
- `any_decimal`: Match any decimal placeholder ({{#...}})
- `any_hexadecimal`: Match any hex placeholder ({{0x#...}})
- `any`: Match any placeholder format

## Migration Guide

### Step 1: Add Names to JSON Files

Run the `add_name_field.py` script to automatically add names to existing JSON files:

```bash
python3 scripts/add_name_field.py
```

### Step 2: Update Test Code

Replace index-based calls with name-based calls:

**Before:**

```rust
ctx.verify_screen_against_definitions(&screen, &definitions, 5)?;
```

**After:**

```rust
verify_screen_with_json_rules(&screen, RULES, "step_05_configure_station")?;
```

### Step 3: Remove CheckStatus Usage

The old `CursorAction::CheckStatus` has been removed. For status tree verification, use direct calls to status monitoring functions:

**Before:**

```rust
CursorAction::CheckStatus {
    description: "Port enabled".to_string(),
    path: "ports[0].enabled".to_string(),
    expected: json!(true),
    timeout_secs: Some(5),
    retry_interval_ms: Some(500),
}
```

**After:**

```rust
// Direct status verification
wait_for_port_enabled("/tmp/vcom1", 5, Some(500)).await?;
```

## API Reference

### Functions

#### `verify_screen_with_json_rules`

```rust
pub fn verify_screen_with_json_rules(
    screen_content: &str,
    json_rules: &str,
    step_name: &str,
) -> Result<()>
```

Standalone verification function for use with embedded JSON via `include_str!`.

#### `SnapshotContext::capture_or_verify`

```rust
pub async fn capture_or_verify<T: ExpectSession>(
    &self,
    session: &mut T,
    cap: &mut TerminalCapture,
    predicted_state: TuiStatus,
    step_name: &str,
) -> Result<String>
```

Main entry point for screenshot/state management in both capture and verify modes.

#### `SnapshotContext::verify_screen_by_step_name`

```rust
pub fn verify_screen_by_step_name(
    screen_content: &str,
    definitions: &[SnapshotDefinition],
    step_name: &str,
) -> Result<()>
```

Verify screen content against a specific named snapshot step.

#### `SnapshotContext::find_definition_by_name`

```rust
pub fn find_definition_by_name<'a>(
    definitions: &'a [SnapshotDefinition],
    step_name: &str,
) -> Result<&'a SnapshotDefinition>
```

Find a snapshot definition by its step name.

## Examples

See:

- `packages/ci_utils/tests/json_verification.rs` - Unit tests demonstrating all features
- `examples/tui_e2e/src/test_snapshot_json.rs` - Integration test example

## Troubleshooting

### Step Not Found

If you get "Snapshot definition not found for step 'xxx'":

1. Check the step name matches exactly (including Chinese characters)
2. Verify the JSON file contains the step
3. Ensure the JSON file is being loaded correctly

### Text Not Found

If verification fails with "Text '...' not found":

1. Check line numbers are 0-indexed
2. Verify the specified lines actually contain the text
3. Consider using broader line ranges if content shifts

### Placeholder Not Matching

If placeholder patterns don't match:

1. Verify placeholder format ({{0b#001}}, {{#001}}, {{0x#001}})
2. Check pattern type matches the placeholder format
3. Use `any` pattern for more flexible matching
