pub mod components;
pub mod input;
pub mod render;

pub use input::handle_input;
pub use render::{page_bottom_hints, render};

/// Number of special action items at the bottom of the port list
/// (Refresh, Manual Specify, About)
pub const SPECIAL_ITEMS_COUNT: usize = 3;

/// Calculate the appropriate view_offset for the last special items
/// based on the number of ports and viewport height.
///
/// This function determines whether scrolling is needed by checking if
/// all content (ports + special items + padding) would fit in the viewport.
/// 
/// Parameters:
/// - `ports_count`: Number of serial ports
/// - `viewport_height`: Height of the viewport in lines. When called from navigation
///   handlers without access to actual viewport, use a conservative estimate like 20.
///
/// Returns:
/// - 0 if content fits without scrolling (few ports)
/// - ports_count if scrolling is needed (many ports)
pub fn calculate_special_items_offset(ports_count: usize, viewport_height: usize) -> usize {
    // Total content = ports + special items
    let total_items = ports_count + SPECIAL_ITEMS_COUNT;
    
    // If all items fit in viewport, no offset needed
    if total_items <= viewport_height {
        0
    } else {
        // Content requires scrolling, set offset to keep special items visible at bottom
        ports_count
    }
}

/// Conservative estimate of viewport height for navigation calculations
/// We use a very conservative estimate to handle even small terminals (15-20 lines).
/// A 15-line terminal minus borders (2) and bottom hints (2) leaves about 11 lines.
/// We use 12 as a safe minimum to ensure proper behavior on small terminals.
pub const CONSERVATIVE_VIEWPORT_HEIGHT: usize = 12;

/// Check if scrollbar is needed based on content size and viewport height
///
/// Parameters:
/// - `ports_count`: Number of serial ports
/// - `viewport_height`: Actual height of the viewport (area.height - 2 for borders)
///
/// Returns:
/// - true if scrollbar should be shown (content exceeds viewport)
/// - false if scrollbar should be hidden (content fits in viewport)
pub fn should_show_scrollbar(ports_count: usize, viewport_height: usize) -> bool {
    let total_items = ports_count + SPECIAL_ITEMS_COUNT;
    total_items > viewport_height
}
