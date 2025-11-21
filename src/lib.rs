//! Aoba — Multi-protocol debugging and simulation tool for Modbus RTU
//!
//! This crate provides the core library for Aoba. It exposes a programmatic
//! API used by the CLI and TUI frontends as well as the daemon runner. For
//! end-user usage see the CLI/TUI examples under `examples/` and the top-level
//! README which documents common usage patterns, daemon configuration, and
//! testing tools.
//!
//! The public modules re-export the main APIs for each domain (protocols,
//! TUI, core helpers, etc.). The internal runtime/boot helpers are placed in
//! a separate, hidden module to keep implementation details out of the generated
//! documentation.

pub mod api;
#[doc(hidden)]
pub mod boot;
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod core;
#[doc(hidden)]
pub mod protocol;
#[doc(hidden)]
pub mod tui;
#[doc(hidden)]
pub mod utils;

pub use api::*;
