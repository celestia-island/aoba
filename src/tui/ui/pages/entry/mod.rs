pub mod components;
pub mod input;
pub mod render;

pub use input::handle_input;
pub use render::{page_bottom_hints, render};

/// Number of special action items at the bottom of the port list
/// (Refresh, Manual Specify, About)
pub const SPECIAL_ITEMS_COUNT: usize = 3;

/// Calculate the appropriate view_offset for the last special items
/// based on the number of ports and typical viewport constraints.
///
/// This function determines whether scrolling is needed by checking if
/// all content (ports + special items) would fit in a typical terminal.
/// 
/// Returns:
/// - 0 if content fits without scrolling (few ports)
/// - ports_count if scrolling is needed (many ports)
pub fn calculate_special_items_offset(ports_count: usize) -> usize {
    // A typical terminal viewport has about 20-25 lines available
    // after accounting for borders, title, and bottom hints.
    // We use a conservative estimate of 18 lines of usable space.
    const TYPICAL_VIEWPORT_LINES: usize = 18;
    
    // Total content = ports + special items
    let total_items = ports_count + SPECIAL_ITEMS_COUNT;
    
    // If all items fit in viewport, no offset needed
    if total_items <= TYPICAL_VIEWPORT_LINES {
        0
    } else {
        // Content requires scrolling, set offset to keep special items visible at bottom
        ports_count
    }
}
