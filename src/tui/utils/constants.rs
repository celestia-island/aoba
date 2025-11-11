//! Adjustable TUI related constants centralized for unified tuning and documentation.
/// Number of lines consumed by each logical log group (timestamp / raw / parsed).
pub const LOG_GROUP_HEIGHT: usize = 3;
/// Base number of logical groups jumped for PageUp / PageDown.
pub const LOG_PAGE_JUMP: usize = 5; // Conservative value; actual visible height is computed during rendering.
