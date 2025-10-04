# GitHub Actions Workflows

This document describes the GitHub Actions workflows used in this repository.

## Auto-Fix Workflows

The repository includes three CI workflows that automatically detect and fix Rust code issues:

### 1. Basic Checks (`basic-checks.yml`)

This workflow runs on every push and pull request to the `master` and `main` branches. It performs:

- **Formatting check**: `cargo fmt --all -- --check`
- **Compilation check**: `cargo check --all-targets --all-features`
- **Clippy lints**: `cargo clippy --all-targets --all-features -- -D warnings`
- **Unit tests**: `cargo test --lib`

**Auto-fix behavior (only on push to master/main with stable Rust)**:
- If formatting issues are detected, runs `cargo fmt --all` to auto-fix
- If compilation errors are found, runs `cargo fix --all-targets --allow-dirty --allow-staged`
- If clippy warnings are detected, runs `cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged`
- Commits all fixes with the message: **"🎨 Clippy."**
- Pushes the commit to the current branch

### 2. CI Basic Checks (`ci-basic-checks.yml`)

This workflow checks the example packages (`aoba_cli_integration_tests`, `aoba_tui_e2e_tests`) with similar auto-fix behavior:

- Runs formatting, compilation, and clippy checks for each example package
- Auto-fixes issues when detected (only on push to master/main)
- Commits fixes with the message: **"🎨 Clippy."**

### 3. Cargo Auto Fix (`cargo-auto-fix.yml`)

A dedicated workflow that focuses solely on auto-fixing issues:

- Can be triggered manually via `workflow_dispatch`
- Runs comprehensive checks: `cargo check`, `cargo clippy`, `cargo fmt`
- Attempts to fix all detected issues
- Commits and pushes changes with the message: **"🎨 Clippy."**

## How Auto-Fix Works

The auto-fix mechanism follows these steps:

1. **Detection**: Each check step (fmt, check, clippy) is run with `continue-on-error: true` and an `id` is assigned
2. **Conditional Fixes**: Auto-fix steps only run if:
   - The corresponding check step failed (`steps.<id>.outcome == 'failure'`)
   - The workflow is running on a push event (not on pull requests)
   - For `basic-checks.yml`, only runs on stable Rust toolchain
3. **Fixing**:
   - `cargo fix` for compilation errors
   - `cargo clippy --fix` for clippy warnings
   - `cargo fmt` for formatting issues
4. **Committing**: All changes are staged, committed with the message "🎨 Clippy.", and pushed to the current branch

## Permissions

All auto-fix workflows require `contents: write` permission to push commits back to the repository.

## Notes

- Auto-fix only runs on direct pushes to `master` or `main` branches, not on pull requests
- The workflows use `github-actions[bot]` as the commit author
- All fix attempts use `continue-on-error: true` to prevent workflow failures
- The `--allow-dirty` and `--allow-staged` flags allow cargo to modify files even with uncommitted changes
