# Fine-Grained Validation Migration Guide

This document demonstrates how to migrate TUI E2E tests from screen-capture-based validation to fine-grained status checks using the new `execute_with_status_checks()` factory function.

## Pattern Overview

### Before: Screen Capture Validation (Slow, Unreliable)

```rust
// Old pattern: Perform actions, then capture screen to verify
let actions = vec![
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 },
    CursorAction::PressEnter,
    CursorAction::TypeString("5".to_string()),
    CursorAction::PressEnter,
];
execute_cursor_actions(session, cap, &actions, "edit_station_id").await?;

// Capture screen and manually search for expected text
let screen = cap.capture(session, "after_edit").await?;
if !screen.contains("Station ID: 5") {
    return Err(anyhow!("Station ID not updated"));
}
```

**Problems:**

- Slow (terminal rendering + capture overhead)
- Brittle (depends on exact text format)
- Race conditions (UI may not have rendered yet)
- No intermediate validation (all-or-nothing)

### After: Status Check Validation (Fast, Reliable)

```rust
// New pattern: Atomic action + validation with auto-retry
use serde_json::json;

// Step 1: Navigate to field
execute_with_status_checks(
    session, cap,
    &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 }],
    &[CursorAction::CheckStatus {
        description: "Page is modbus_dashboard".to_string(),
        path: "page.type".to_string(),
        expected: json!("modbus_dashboard"),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }],
    "navigate_to_station_id",
    None,
).await?;

// Step 2: Enter edit mode
execute_with_status_checks(
    session, cap,
    &[CursorAction::PressEnter],
    &[CursorAction::CheckStatus {
        description: "Page is modbus_dashboard".to_string(),
        path: "page.type".to_string(),
        expected: json!("modbus_dashboard"),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }],
    "enter_edit",
    None,
).await?;

// Step 3: Clear and type value
execute_with_status_checks(
    session, cap,
    &[
        CursorAction::PressCtrlA,
        CursorAction::PressBackspace,
        CursorAction::TypeString("5".to_string()),
    ],
    &[CursorAction::CheckStatus {
        description: "Page is modbus_dashboard".to_string(),
        path: "page.type".to_string(),
        expected: json!("modbus_dashboard"),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    }],
    "type_value",
    None,
).await?;

// Step 4: Commit and VERIFY value was written
execute_with_status_checks(
    session, cap,
    &[CursorAction::PressEnter],
    &[
        CursorAction::CheckStatus {
            description: "Station ID updated to 5".to_string(),
            path: "ports[0].modbus_masters[0].station_id".to_string(),
            expected: json!(5),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        },
    ],
    "commit_station_id",
    Some(3), // 3 retries if check fails
).await?;
```

**Benefits:**

- Fast (direct status file read)
- Reliable (checks actual config, not UI representation)
- Fine-grained (validates at each step)
- Auto-retry (transient failures handled automatically)

## Complete Example: Configure Station Fields

### Scenario: Create and configure a Master station

```rust
use ci_utils::*;
use serde_json::json;

pub async fn configure_station_demo<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    // STEP 1: Create station
    execute_with_status_checks(
        session, cap,
        &[
            CursorAction::PressEnter, // Press on "Create Station"
            CursorAction::Sleep1s,
        ],
        &[
            CursorAction::CheckStatus {
                description: "Station #1 created".to_string(),
                path: "ports[0].modbus_masters".to_string(),
                expected: json!([{}]), // Array has 1 element (exact config checked later)
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
        ],
        "create_station",
        Some(3),
    ).await?;

    // STEP 2: Navigate to Station ID field
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }],
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "nav_to_station_id",
        None,
    ).await?;

    // STEP 3: Edit Station ID
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressEnter], // Enter edit mode
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "enter_edit_station_id",
        None,
    ).await?;

    execute_with_status_checks(
        session, cap,
        &[
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString("5".to_string()),
        ],
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "type_station_id",
        None,
    ).await?;

    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressEnter], // Commit
        &[
            CursorAction::CheckStatus {
                description: "Station ID is 5".to_string(),
                path: "ports[0].modbus_masters[0].station_id".to_string(),
                expected: json!(5),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
        ],
        "commit_station_id",
        Some(3),
    ).await?;

    // STEP 4: Navigate to Register Type field
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }],
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "nav_to_register_type",
        None,
    ).await?;

    // STEP 5: Change Register Type from default to Holding
    execute_with_status_checks(
        session, cap,
        &[
            CursorAction::PressEnter,
            CursorAction::PressArrow { direction: ArrowKey::Left, count: 2 },
            CursorAction::PressEnter,
        ],
        &[
            CursorAction::CheckStatus {
                description: "Register type is Holding".to_string(),
                path: "ports[0].modbus_masters[0].register_type".to_string(),
                expected: json!("Holding"),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
        ],
        "set_register_type_holding",
        Some(3),
    ).await?;

    // STEP 6: Set Start Address
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }],
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "nav_to_start_address",
        None,
    ).await?;

    execute_with_status_checks(
        session, cap,
        &[
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString("100".to_string()),
            CursorAction::PressEnter,
        ],
        &[
            CursorAction::CheckStatus {
                description: "Start address is 100".to_string(),
                path: "ports[0].modbus_masters[0].start_address".to_string(),
                expected: json!(100),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
        ],
        "set_start_address",
        Some(3),
    ).await?;

    // STEP 7: Set Register Count
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 }],
        &[CursorAction::CheckStatus {
            description: "Page is modbus_dashboard".to_string(),
            path: "page.type".to_string(),
            expected: json!("modbus_dashboard"),
            timeout_secs: Some(5),
            retry_interval_ms: Some(500),
        }],
        "nav_to_register_count",
        None,
    ).await?;

    execute_with_status_checks(
        session, cap,
        &[
            CursorAction::PressEnter,
            CursorAction::PressCtrlA,
            CursorAction::PressBackspace,
            CursorAction::TypeString("10".to_string()),
            CursorAction::PressEnter,
        ],
        &[
            CursorAction::CheckStatus {
                description: "Register count is 10".to_string(),
                path: "ports[0].modbus_masters[0].register_count".to_string(),
                expected: json!(10),
                timeout_secs: Some(5),
                retry_interval_ms: Some(500),
            },
        ],
        "set_register_count",
        Some(3),
    ).await?;

    // STEP 8: Save configuration
    execute_with_status_checks(
        session, cap,
        &[CursorAction::PressCtrlS, CursorAction::Sleep3s],
        &[
            // Verify port is enabled after save
            CursorAction::CheckStatus {
                description: "Port is enabled".to_string(),
                path: "ports[0].enabled".to_string(),
                expected: json!(true),
                timeout_secs: Some(10),
                retry_interval_ms: Some(500),
            },
        ],
        "save_configuration",
        Some(3),
    ).await?;

    // STEP 9: Final verification - check complete configuration
    let final_checks = check_station_config(
        0, // port_index
        0, // station_index
        true, // is_master
        5, // station_id
        "Holding", // register_type
        100, // start_address
        10, // register_count
    );

    execute_cursor_actions(session, cap, &final_checks, "verify_final_config").await?;

    log::info!("✅ Station configured and verified");
    Ok(())
}
```

## Granularity Principle

As requested by @langyo, validation should be **as fine-grained as possible**:

### ✅ GOOD: Fine-grained steps

```rust
let page_check = check_page("modbus_dashboard");

// Step 1: Navigate
execute_with_status_checks(..., &[navigate_action], &page_check, ...).await?;

// Step 2: Enter edit
execute_with_status_checks(..., &[enter], &page_check, ...).await?;

// Step 3: Type
execute_with_status_checks(..., &[type_action], &page_check, ...).await?;

// Step 4: Commit + VERIFY
execute_with_status_checks(..., &[enter], &[check_config], ...).await?;
```

### ❌ BAD: Coarse-grained (all actions in one step)

```rust
execute_with_status_checks(
    ...,
    &[navigate, enter, type, commit], // Too many actions together
    &[check_config],
    ...
).await?;
```

**Why fine-grained is better:**

- Easier to debug which step failed
- Can retry individual steps
- Better isolation of failures
- Matches the user interaction model

## Helper Functions

Use the validation helpers from `validation.rs`:

```rust
// Check station configuration (4 checks in one call)
let checks = check_station_config(0, 0, true, 5, "Holding", 100, 10);
execute_cursor_actions(session, cap, &checks, "verify").await?;

// Check page
let checks = check_page("modbus_dashboard");
execute_cursor_actions(session, cap, &checks, "verify_page").await?;

// Check port enabled
let checks = check_port_enabled(0, true);
execute_cursor_actions(session, cap, &checks, "verify_enabled").await?;
```

## Migration Checklist for Each Test Module

- [ ] Identify all screen capture validations (`cap.capture` followed by `.contains()`)
- [ ] Replace with `CheckStatus` actions using appropriate JSON path
- [ ] Break down coarse operations into fine-grained steps
- [ ] Add status checks after each value-changing operation
- [ ] Use `execute_with_status_checks` for operations that need retry
- [ ] Keep `DebugBreakpoint` for troubleshooting
- [ ] Test each migrated function individually

## Status Paths Reference

### Common Paths

- `page` - Current TUI page (e.g., `{"type": "modbus_dashboard"}`)
- `ports[N].enabled` - Port enabled state (boolean)
- `ports[N].state` - Port state (e.g., `{"type": "occupied_by_this"}`)
- `ports[N].modbus_masters[M].station_id` - Master station ID (number)
- `ports[N].modbus_masters[M].register_type` - Register type (string like "Holding")
- `ports[N].modbus_masters[M].start_address` - Start address (number)
- `ports[N].modbus_masters[M].register_count` - Register count (number)
- `ports[N].modbus_slaves[M].*` - Same fields for slaves

### Index Conventions

- `N` = Port index (usually 0 for first port)
- `M` = Station index (0 for first station, 1 for second, etc.)

## Expected Values

Use `serde_json::json!()` macro for expected values:

```rust
json!(5)                  // Number
json!("Holding")          // String
json!(true)               // Boolean
json!([1, 2, 3])          // Array
json!({"type": "entry"})  // Object
```

## Performance Impact

**Before** (screen capture):

- 1 operation = 500-1000ms (capture + parse)
- 10 operations = 5-10 seconds

**After** (status check):

- 1 operation = 50-100ms (read JSON file)
- 10 operations = 0.5-1 second

Result: **10x faster tests**
