# Multi-Station Drill-Down Mode Testing & Fix Plan

## Current Status (Post Structural Corrections)

### ✅ Completed
1. **Structural Corrections** (Commit f79ccc1)
   - Station configuration fixes (IDs, addresses)
   - Header and manifest updates
   - TOML workflow file structure complete

2. **Files Status**
   - master/mixed_types.toml - Structure corrected
   - master/mixed_ids.toml - Structure corrected
   - master/spaced_addresses.toml - Structure corrected
   - slave/mixed_types.toml - Structure corrected
   - slave/mixed_ids.toml - Structure corrected
   - slave/spaced_addresses.toml - Structure corrected

### ⏳ Pending: Drill-Down Mode Validation

**What is Drill-Down Mode?**
- Real TUI process with `--debug-ci` flag
- Full integration testing via IPC
- Keyboard input simulation
- Actual CLI subprocess spawning
- Screen content verification
- Tests complete user workflows

**vs Rendering Mode** (which already passes for most tests):
- Fast execution without process spawning
- Direct mock state manipulation
- Uses `ratatui::TestBackend` for rendering
- Ideal for UI regression tests

## Testing Strategy

### Phase 1: Batch Test Execution (Current)
Run all 6 multi-station drill-down tests and collect logs:

```bash
# Master mode tests (3)
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_master_mixed_types
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_master_mixed_ids
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_master_spaced_addresses

# Slave mode tests (3)
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_slave_mixed_types
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_slave_mixed_ids
./scripts/run_ci_locally.sh --workflow tui-drilldown --module multi_station_slave_spaced_addresses
```

### Phase 2: Error Analysis
Common drill-down mode issues:
1. **IPC Communication Failures**
   - TUI process not receiving keyboard events
   - Screen content not being captured
   - Connection timeouts

2. **CLI Subprocess Issues**
   - Multiple CLI instances for multi-station
   - Port conflicts
   - Configuration file vs command-line args (especially for slave mode)

3. **Timing/Race Conditions**
   - Menu navigation timing
   - Page transition delays
   - Port enable/disable synchronization

4. **Keyboard Navigation Errors**
   - Incorrect cursor positioning
   - Missing or extra keystrokes
   - Field navigation issues

### Phase 3: Targeted Fixes
Based on error logs, fix issues in priority order:
1. Critical blockers (test can't run at all)
2. Configuration/setup issues
3. Keyboard navigation adjustments
4. Timing adjustments

## Known Challenges for Multi-Station Tests

### Master Mode
- **Easier:** Can spawn 2 separate CLI instances
  ```bash
  # Station 1
  cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --station-id 1 ...
  
  # Station 2  
  cargo run --package aoba -- --slave-listen-persist /tmp/vcom2 --station-id 2 ...
  ```

### Slave Mode  
- **More Complex:** Multiple stations in single TUI instance
- **Solution:** Use temporary configuration file instead of CLI args
  ```bash
  # Create temp config with both stations
  cargo run --package aoba -- --tui --config /tmp/multi_station_config.json
  ```
- **Requires:** TUI E2E test framework updates to:
  1. Generate temporary config files
  2. Pass config file path to TUI
  3. Manage config lifecycle (create/cleanup)

## Expected Outcomes

### Success Criteria
- All 6 multi-station drill-down tests passing
- Both rendering and drill-down modes working
- Stable/reliable test execution (>95% pass rate)

### Deliverables
1. Updated workflow TOML files (if keyboard navigation needs adjustment)
2. TUI E2E framework updates (if slave mode needs config file support)
3. CLI subprocess spawning logic (if multi-station coordination needs fixes)
4. Documentation of any drill-down-specific patterns

## Next Steps

1. ✅ **Compile tui_e2e** (in progress)
2. ⏳ **Run all 6 drill-down tests** - collect logs
3. ⏳ **Analyze failure patterns** - categorize errors
4. ⏳ **Implement fixes** - iterate until all passing
5. ⏳ **Verify stability** - run 3x to ensure consistency

## Timeline Estimate

- Test execution: 10-15 min per test × 6 = 60-90 min
- Error analysis: 30-60 min
- Fixes implementation: 2-6 hours (depending on complexity)
- Verification: 30-60 min

**Total: 4-8 hours** (assuming no major TUI E2E framework changes needed)

If slave mode requires config file support, add 2-4 hours for framework updates.
