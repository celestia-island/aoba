# Action Items from @langyo Feedback

## Summary of Required Changes

Based on feedback in comment #3474125490, the following changes are needed to properly implement screenshot generation mode:

## 1. Remove Serial Port Initialization from Runtime ✅ (Low Priority)
**Action**: Make serial port setup user's responsibility, not automatic in test runtime
**Impact**: Tests need explicit port setup before running
**Files**: `examples/tui_e2e/src/main.rs` - `setup_virtual_serial_ports()`

## 2. Screenshot Mode Behavior Changes ⚠️ (CRITICAL - Breaking Change)
**Current Problem**: In `--generate-screenshots` mode, tests still execute keyboard actions
**Required**: In screenshot mode, ALL keyboard actions should be skipped, only screenshots processed

**Impact**: This is a fundamental architectural change affecting the entire test execution model

## 3. Remove CursorAction Variants ⚠️ (BREAKING CHANGE)
**Remove**:
- `CursorAction::MatchPattern` - Currently used in 22+ places in tui_e2e
- `CursorAction::CheckStatus` - Currently used in tui_e2e

**Impact**: 
- Breaking change affecting all existing tests
- Requires updating all test files that use these actions
- Estimated 20-30 files need modification

## 4. Simplify MatchScreenCapture ✅ (Partially Done)
**Remove**: `line_range` and `col_range` parameters from `MatchScreenCapture`
**New Structure**:
```rust
MatchScreenCapture {
    test_name: String,  // Can be hierarchical path
    step_name: String,
    description: String,
}
```

**Status**: Structure defined but execution logic needs implementation

## 5. Add AssertUpdateStatus Action ⚠️ (New Feature)
**Purpose**: Update mock global status in screenshot mode
**Signature**:
```rust
AssertUpdateStatus {
    description: String,
    updater: fn(&mut TuiStatus),
}
```

**Behavior**:
- **Screenshot mode**: Updates internal mock TuiStatus
- **Normal mode**: Ignored (no-op)

**Challenge**: Requires passing mock state through entire execution chain

## 6. MatchScreenCapture Dual Behavior ⚠️ (Complex Change)
**Non-screenshot mode**:
- Read reference file from `$pwd/examples/tui_e2e/screenshots/$test_name/$step_name.txt`
- Compare with actual terminal output
- Fail if mismatch

**Screenshot mode**:
- Spawn TUI with mock status
- Capture terminal output
- Write to `$pwd/examples/tui_e2e/screenshots/$test_name/$step_name.txt`
- Create directories as needed

## 7. Mock State Initialization ✅ (Design Complete)
**Requirement**: Initialize mock TuiStatus with:
- `/tmp/vcom1` port
- `/tmp/vcom2` port
- Default disabled state

**Implementation**: Create initial state in screenshot mode at test start

## Implementation Challenges

### Challenge 1: Breaking Changes
Removing `MatchPattern` and `CheckStatus` breaks 20+ test files. Each needs manual update.

### Challenge 2: Mode-Dependent Execution
Current `execute_cursor_actions` is 900+ lines and tightly coupled. Needs refactoring to:
- Check mode at every action
- Skip keyboard input in screenshot mode
- Handle mock state updates
- Support dual screenshot behavior

### Challenge 3: State Threading
Mock TuiStatus needs to be:
- Created at test start
- Passed through all action executions
- Updated by `AssertUpdateStatus`
- Used by `MatchScreenCapture` for rendering

This requires changing function signatures throughout the call chain.

### Challenge 4: Screenshot Generation
In screenshot mode, `MatchScreenCapture` needs to:
1. Serialize mock status to `/tmp/status.json`
2. Spawn TUI with `--debug-screen-capture`
3. Wait for rendering
4. Capture output
5. Write to file
6. Clean up TUI process

This is essentially what `tui_ui_e2e` does - needs integration.

## Recommended Approach

### Option 1: Complete Rewrite (4-6 hours)
1. Create new `auto_cursor_v2.rs` with clean implementation
2. Update all test files to new API
3. Test thoroughly
4. Remove old implementation

### Option 2: Incremental Migration (8-10 hours)
1. Keep old `CursorAction` variants deprecated
2. Add new variants alongside
3. Migrate tests one by one
4. Remove old variants when done

### Option 3: Separate Screenshot System (2-3 hours)
1. Keep `CursorAction` for normal tests
2. Create separate `ScreenshotAction` enum
3. Separate execution functions
4. Screenshot mode uses different code path

## Current Status

**Completed**:
- ✅ Enum structure updated (partially)
- ✅ Helper functions for file I/O
- ✅ Design documented

**Blocked**:
- ⚠️ Execute function needs complete rewrite
- ⚠️ 20+ test files need updates
- ⚠️ Mock state threading not implemented
- ⚠️ Screenshot generation logic not integrated

## Recommendation

Given the scope of breaking changes and the complexity involved, I recommend:

1. **Create separate PR** for these changes with dedicated time
2. **Use Option 3** (separate screenshot system) to minimize breaking changes
3. **Test thoroughly** before merging - this affects all E2E tests

The current PR successfully fixes the terminal capture issue (original goal). These architectural changes for screenshot generation should be a separate focused effort.

## Files Requiring Changes

If proceeding with implementation:

1. `packages/ci_utils/src/auto_cursor.rs` - Complete rewrite of execute function
2. `packages/ci_utils/src/lib.rs` - Export new types
3. `examples/tui_e2e/src/e2e/**/*.rs` - Update all test files (20+ files)
4. `examples/tui_e2e/src/main.rs` - Add mock state initialization
5. Integration with `screenshot.rs` infrastructure

**Estimated Total Effort**: 6-10 hours of focused development + testing
