use anyhow::Result;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    tui::{
        status::{
            port::{PortData, PortState},
            read_status, Page,
        },
        ui::pages::entry::components::derive_selection_from_page,
    },
    utils::i18n::lang,
};

pub fn page_bottom_hints() -> Result<Vec<Vec<String>>> {
    // Check if we're in creation mode
    let in_creation = read_status(|status| Ok(status.temporarily.new_port_creation.active))?;

    if in_creation {
        // Show creation mode hints using "Press xxx to yyy" format
        Ok(vec![vec![
            lang().hotkeys.press_enter_submit.to_string(),
            lang().hotkeys.press_esc_cancel.to_string(),
        ]])
    } else {
        // Show normal mode hints using "Press xxx to yyy" format
        Ok(vec![vec![
            lang().index.hint_press_n_new_port.to_string(),
            lang().index.hint_press_d_delete_port.to_string(),
            lang().index.hint_press_a_about.to_string(),
            lang().hotkeys.press_q_quit.to_string(),
        ]])
    }
}

/// Render the entry page as a full-screen node grid layout.
pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    let selection = read_status(|app| derive_selection_from_page(&app.page, &app.ports.order))?;

    // Get port list, cursor state, and new port creation state
    let (ports_order, _cursor_opt, view_offset, in_creation, port_type_index) =
        read_status(|status| {
            let cursor = if let Page::Entry { cursor, .. } = &status.page {
                *cursor
            } else {
                None
            };
            let offset = if let Page::Entry { view_offset, .. } = &status.page {
                *view_offset
            } else {
                0
            };
            Ok((
                status.ports.order.clone(),
                cursor,
                offset,
                status.temporarily.new_port_creation.active,
                status.temporarily.new_port_creation.port_type_index,
            ))
        })?;

    // Render the full-screen canvas with nodes
    render_node_grid(
        frame,
        area,
        &ports_order,
        selection,
        view_offset,
        in_creation,
        port_type_index,
    )?;

    Ok(())
}

/// Render nodes in a horizontal single-row layout with scrolling support
fn render_node_grid(
    frame: &mut Frame,
    area: Rect,
    ports_order: &[String],
    selection: usize,
    _view_offset: usize,
    in_creation: bool,
    port_type_index: usize,
) -> Result<()> {
    // Create outer block for the canvas
    let canvas_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let inner_area = canvas_block.inner(area);
    frame.render_widget(canvas_block, area);

    // Node dimensions: two lines (name + type), horizontal only
    let node_width = 20u16; // Total width including borders
    let node_height = 4u16; // Two lines + borders (4 total)
    let spacing = 1u16; // Spacing between nodes

    // Calculate total number of nodes (ports + editing node if in creation mode)
    let total_nodes = if in_creation {
        ports_order.len() + 1
    } else {
        ports_order.len()
    };

    // Get port data for status indicators
    let ports_map = read_status(|status| Ok(status.ports.map.clone()))?;

    // Calculate how many nodes fit in the viewport
    let viewport_width = inner_area.width;
    let nodes_fit_in_viewport = (viewport_width / (node_width + spacing)).max(1) as usize;

    // Smart viewport rendering strategy based on selection position
    let (start_index, end_index) = calculate_visible_range(
        selection,
        total_nodes,
        nodes_fit_in_viewport,
    );

    // Render position indicator in top-right corner (green to match other UI elements)
    if total_nodes > 0 {
        let indicator_text = format!(" {} / {} ", selection + 1, total_nodes);
        let indicator_width = indicator_text.len() as u16;
        let indicator_area = Rect {
            x: area.x + area.width.saturating_sub(indicator_width + 1),
            y: area.y,
            width: indicator_width,
            height: 1,
        };
        let indicator_widget = Paragraph::new(indicator_text)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
        frame.render_widget(indicator_widget, indicator_area);
    }

    // Render only the visible nodes based on calculated range
    for i in start_index..end_index.min(ports_order.len()) {
        let node_position_in_view = i - start_index;
        let x = inner_area.x + (node_position_in_view as u16 * (node_width + spacing));
        let y = inner_area.y;

        // Skip if no room
        if x + node_width > inner_area.x + inner_area.width {
            break;
        }

        let port_name = &ports_order[i];
        let node_area = Rect {
            x,
            y,
            width: node_width,
            height: node_height,
        };

        // Render the node
        render_node(frame, node_area, port_name, i == selection, &ports_map)?;
    }

    // Render "editing" node if in creation mode and it's in visible range
    if in_creation && end_index > ports_order.len() {
        let edit_node_index = ports_order.len();
        if edit_node_index >= start_index {
            let node_position_in_view = edit_node_index - start_index;
            let x = inner_area.x + (node_position_in_view as u16 * (node_width + spacing));
            let y = inner_area.y;

            if x + node_width <= inner_area.x + inner_area.width {
                let node_area = Rect {
                    x,
                    y,
                    width: node_width,
                    height: node_height,
                };

                render_editing_node(frame, node_area, port_type_index)?;
            }
        }
    }

    // Render horizontal scrollbar if total nodes exceed viewport capacity (using ratatui's Scrollbar)
    if total_nodes > nodes_fit_in_viewport {
        let scrollbar_area = Rect {
            x: area.x + 1,
            y: area.y + area.height - 1,
            width: area.width - 2,
            height: 1,
        };
        // Calculate scrollbar max based on actual content vs viewport ratio
        // This ensures the thumb size reflects the actual visible portion
        let scrollbar_max = total_nodes.saturating_sub(1).max(1);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .thumb_symbol("━")  // Use thick horizontal line for occupied part
            .track_symbol(Some("─"));  // Use thin horizontal line for unoccupied part
        frame.render_stateful_widget(
            scrollbar,
            scrollbar_area,
            &mut ScrollbarState::new(scrollbar_max)
                .position(selection)
                .viewport_content_length(nodes_fit_in_viewport),
        );
    }

    Ok(())
}

/// Calculate the visible range of nodes based on selection position
/// Strategy:
/// - First 2 positions: show from start
/// - Last 2 positions: show from end  
/// - Middle positions: selected node at 3rd position (index 2 in view)
fn calculate_visible_range(
    selection: usize,
    total_nodes: usize,
    nodes_fit: usize,
) -> (usize, usize) {
    if total_nodes <= nodes_fit {
        // All nodes fit, show everything
        return (0, total_nodes);
    }

    // Selection at beginning (first 2 positions)
    if selection < 2 {
        return (0, nodes_fit);
    }

    // Selection at end (last 2 positions)
    if selection >= total_nodes.saturating_sub(2) {
        return (total_nodes.saturating_sub(nodes_fit), total_nodes);
    }

    // Selection in middle: place selected node at position 2 (3rd from left)
    // This ensures at least 2 nodes before the selected one
    let start = selection.saturating_sub(2);
    let end = (start + nodes_fit).min(total_nodes);
    (start, end)
}

/// Truncate text to max 10 characters with ellipsis
fn truncate_text(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        text.to_string()
    } else {
        // Take first 7 chars + "..."
        let truncated: String = chars.iter().take(7).collect();
        format!("{}...", truncated)
    }
}

/// Render a single port node
fn render_node(
    frame: &mut Frame,
    area: Rect,
    port_name: &str,
    is_selected: bool,
    ports_map: &std::collections::HashMap<String, PortData>,
) -> Result<()> {
    // Get port state
    let port_state = ports_map
        .get(port_name)
        .map(|p| p.state.clone())
        .unwrap_or(PortState::Free);

    // Determine status indicator
    let status_indicator = match port_state {
        PortState::OccupiedByThis => "●", // Filled circle
        _ => "○",                         // Empty circle
    };

    // Create node block with border
    let node_border_style = if is_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let node_block = Block::default()
        .borders(Borders::ALL)
        .border_style(node_border_style);

    let inner = node_block.inner(area);
    frame.render_widget(node_block, area);

    // Render status indicator on the border (top-right corner on the actual border)
    // When selected, show with angle brackets: " > ● < " (7 chars total)
    // When not selected, show with spaces: "   ●   " (7 chars total)
    if area.width >= 7 {
        let indicator_x = area.x + area.width.saturating_sub(8);
        let indicator_y = area.y;
        let indicator_area = Rect {
            x: indicator_x,
            y: indicator_y,
            width: 7,
            height: 1,
        };
        let indicator_color = match port_state {
            PortState::OccupiedByThis => Color::Green,
            _ => Color::Gray,
        };

        // Selection indicator: angle brackets around the circle when selected
        let indicator_text = if is_selected {
            format!(" > {} < ", status_indicator)
        } else {
            format!("   {}   ", status_indicator)
        };

        let indicator_style = if is_selected {
            Style::default()
                .fg(indicator_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(indicator_color)
        };

        let indicator_widget = Paragraph::new(indicator_text).style(indicator_style);
        frame.render_widget(indicator_widget, indicator_area);
    }

    // Build node content - two lines (node height is 4: top border + 2 content lines + bottom border)
    // No angle brackets in the text - selection is shown via the indicator above
    if inner.height >= 2 && inner.width >= 3 {
        // Get port type from port data
        let port_type = ports_map
            .get(port_name)
            .map(|p| p.port_type.as_str())
            .unwrap_or("");
        
        // Determine port type display label
        let port_type_label = if port_type.contains("http") || port_type.contains("HTTP") {
            if lang().index.title.contains("中") {
                "HTTP 服务器"
            } else {
                "HTTP Server"
            }
        } else if port_type.contains("ipc") || port_type.contains("IPC") {
            if lang().index.title.contains("中") {
                "IPC 管道"
            } else {
                "IPC Pipe"
            }
        } else {
            // Default to serial port
            lang().index.port_suffix.as_str()
        };

        let text_style = if is_selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        // Line 1: Port name (truncated to 10 chars max)
        let name_display = truncate_text(port_name, 10);
        let name_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let name_widget = Paragraph::new(name_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(name_widget, name_area);

        // Line 2: Port type (truncated to 10 chars max)
        let type_display = truncate_text(port_type_label, 10);
        let type_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        let type_widget = Paragraph::new(type_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(type_widget, type_area);
    }

    Ok(())
}

/// Render the "editing" node for new port creation
fn render_editing_node(frame: &mut Frame, area: Rect, port_type_index: usize) -> Result<()> {
    // Create node block with border - always highlighted as it's being edited
    let node_border_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let node_block = Block::default()
        .borders(Borders::ALL)
        .border_style(node_border_style);

    let inner = node_block.inner(area);
    frame.render_widget(node_block, area);

    // Build node content - two lines (node height is 4: top border + 2 content lines + bottom border)
    if inner.height >= 2 && inner.width >= 3 {
        let text_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        // Line 1: "新建" label
        let new_label = if lang().index.title.contains("中") {
            "新建"
        } else {
            "New"
        };
        let new_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let new_widget = Paragraph::new(new_label)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(new_widget, new_area);

        // Line 2: Port type selector with outward-pointing brackets < option >
        let port_types = [
            lang().index.port_creation_ipc_pipe.as_str(),
            lang().index.port_creation_http_server.as_str(),
        ];

        let type_display = if port_type_index < port_types.len() {
            format!("< {} >", truncate_text(port_types[port_type_index], 10))
        } else {
            format!("< {} >", truncate_text(port_types[0], 10))
        };

        let type_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };

        let type_widget = Paragraph::new(type_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(type_widget, type_area);
    }

    Ok(())
}
