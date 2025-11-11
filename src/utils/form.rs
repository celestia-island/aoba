//! Generic form editing helpers that are UI-agnostic.
//!
//! These provide tiny building blocks for toggling editing state and
//! running a reset closure when editing ends. Keeping them generic
//! lets various frontends reuse the same small helpers without depending
//! on UI-specific types.

/// Set editing flag to true.
pub fn begin_edit(editing: &mut bool) {
    *editing = true;
}

/// End editing: set editing to false and run a reset closure to clear UI-specific fields.
pub fn end_edit_with_reset<F>(editing: &mut bool, reset: F)
where
    F: FnOnce(),
{
    *editing = false;
    reset();
}
