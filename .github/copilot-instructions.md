# Aoba Project - Copilot Instructions

This document provides GitHub Copilot with context about the aoba project.

## Key Documents

- **CLAUDE.md** (project root): Comprehensive agent guide covering module layout, testing architecture, IPC communication, E2E testing workflows, and best practices. Always read this first for detailed implementation guidance.

## Quick Reference

### Module Layout
- `src/cli` — CLI argument parsing, command dispatch, long-lived Modbus worker processes
- `src/core` — Cross-CLI/TUI shared business logic and process orchestration
- `src/protocol` — IPC definitions, state models, Modbus transport primitives
- `src/tui` — Terminal UI (ratatui), global state management, IPC front-end
- `src/utils` — Reusable utility functions
- `src/api` — Public API for external consumers (ModbusBuilder, etc.)

### E2E TUI Testing

Two modes:
- **Screen capture** (`--screen-capture-only`): Fast UI tests using `ratatui::TestBackend` with mocked state
- **DrillDown** (default): Full integration tests with real TUI process via IPC (Unix sockets)

Modules defined in `examples/tui_e2e/workflow/*.toml`. Run with:
```
cargo run --package tui_e2e -- [--screen-capture-only] --module <name>
```

### Rust Conventions
- Use groups: std/core → domain crates → workspace/internal
- Single blank line between groups
- Always run `scripts/enforce_use_groups.py` before `cargo fmt`
- Use workspace dependencies from `[workspace.dependencies]` in root `Cargo.toml`

### API Public Surface
The `api` module is the only public API. Re-exports:
- `ModbusBuilder`, `ModbusMaster`, `ModbusSlave`
- `open_serial_port()`, `is_port_occupied()`, `probe_modbus_rtu_baud()`
- `AsyncSerialPort` (behind `async-serial` feature)
