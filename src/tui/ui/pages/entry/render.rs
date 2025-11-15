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
        // Show creation mode hints using i18n
        let confirm_hint = format!("Enter: {}", lang().index.press_enter_confirm.as_str());
        let cancel_hint = format!("Esc: {}", lang().index.press_esc_cancel_action.as_str());
        Ok(vec![vec![confirm_hint, cancel_hint]])
    } else {
        // Show normal mode hints
        Ok(vec![vec![
            format!("n: {}", lang().index.new_action.as_str()),
            format!("d: {}", lang().index.delete_action.as_str()),
            format!("a: {}", lang().index.about_label.as_str()),
            format!(
                "q: {}",
                lang()
                    .hotkeys
                    .press_q_quit
                    .as_str()
                    .replace("Press q to ", "")
            ),
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

/// Render nodes in a horizontal grid layout with scrolling support
fn render_node_grid(
    frame: &mut Frame,
    area: Rect,
    ports_order: &[String],
    selection: usize,
    view_offset: usize,
    in_creation: bool,
    port_type_index: usize,
) -> Result<()> {
    // Create outer block for the canvas
    let canvas_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let inner_area = canvas_block.inner(area);
    frame.render_widget(canvas_block, area);

    // Node dimensions: width includes border, so content width is width - 2
    let node_width = 20u16; // Total width including borders
    let node_height = 5u16; // Total height including borders
    let spacing = 1u16; // Spacing between nodes

    // Calculate how many nodes fit in the viewport
    let viewport_width = inner_area.width;
    let nodes_per_row = ((viewport_width + spacing) / (node_width + spacing)).max(1) as usize;

    // Calculate total content width for scrolling
    let total_nodes = ports_order.len();
    let content_width = (node_width + spacing) * total_nodes as u16;

    // Determine horizontal scroll offset based on selection
    let selected_col = selection % nodes_per_row;
    let horizontal_offset = if content_width > viewport_width {
        let selected_x = selected_col as u16 * (node_width + spacing);
        if selected_x < view_offset as u16 {
            selected_x
        } else if selected_x + node_width > view_offset as u16 + viewport_width {
            (selected_x + node_width).saturating_sub(viewport_width)
        } else {
            view_offset as u16
        }
    } else {
        0
    };

    // Get port data for status indicators (Arc<PortData> is cloned cheaply)
    let ports_map = read_status(|status| Ok(status.ports.map.clone()))?;

    // Render each node
    for (i, port_name) in ports_order.iter().enumerate() {
        let row = i / nodes_per_row;
        let col = i % nodes_per_row;

        // Calculate node position
        let x =
            inner_area.x + (col as u16 * (node_width + spacing)).saturating_sub(horizontal_offset);
        let y = inner_area.y + (row as u16 * (node_height + spacing));

        // Skip nodes outside the viewport
        if x + node_width < inner_area.x || x >= inner_area.x + inner_area.width {
            continue;
        }
        if y + node_height < inner_area.y || y >= inner_area.y + inner_area.height {
            continue;
        }

        // Create node area, clipping to viewport bounds
        let node_x = x.max(inner_area.x);
        let node_width_visible = (x + node_width)
            .min(inner_area.x + inner_area.width)
            .saturating_sub(node_x);

        if node_width_visible == 0 {
            continue;
        }

        let node_area = Rect {
            x: node_x,
            y,
            width: node_width_visible.min(node_width),
            height: node_height.min(inner_area.height.saturating_sub(y - inner_area.y)),
        };

        // Render the node
        render_node(frame, node_area, port_name, i == selection, &ports_map)?;
    }

    // Render "editing" node if in creation mode
    if in_creation {
        let edit_node_index = ports_order.len();
        let row = edit_node_index / nodes_per_row;
        let col = edit_node_index % nodes_per_row;

        let x =
            inner_area.x + (col as u16 * (node_width + spacing)).saturating_sub(horizontal_offset);
        let y = inner_area.y + (row as u16 * (node_height + spacing));

        // Only render if in viewport
        if x + node_width >= inner_area.x
            && x < inner_area.x + inner_area.width
            && y + node_height >= inner_area.y
            && y < inner_area.y + inner_area.height
        {
            let node_x = x.max(inner_area.x);
            let node_width_visible = (x + node_width)
                .min(inner_area.x + inner_area.width)
                .saturating_sub(node_x);

            if node_width_visible > 0 {
                let node_area = Rect {
                    x: node_x,
                    y,
                    width: node_width_visible.min(node_width),
                    height: node_height.min(inner_area.height.saturating_sub(y - inner_area.y)),
                };

                render_editing_node(frame, node_area, port_type_index)?;
            }
        }
    }

    // Render horizontal scrollbar if needed
    if content_width > viewport_width {
        let scrollbar_area = Rect {
            x: area.x + 1,
            y: area.y + area.height - 1,
            width: area.width - 2,
            height: 1,
        };
        let scrollbar_max = content_width.saturating_sub(viewport_width) as usize;
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
            scrollbar_area,
            &mut ScrollbarState::new(scrollbar_max).position(horizontal_offset as usize),
        );
    }

    Ok(())
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

    // Build node content with proper padding
    // Node height is 5, so inner height is 3 (5 - 2 borders), which is enough for 2 lines of text
    // No angle brackets in the text - selection is shown via the indicator above
    if inner.height >= 2 && inner.width >= 3 {
        // Line 1: Port name
        let port_suffix = lang().index.port_suffix.as_str();

        // Calculate vertical centering for two lines of text
        let start_y = inner.y + (inner.height / 2).saturating_sub(1);

        // First line: Port name (no selection brackets)
        let name_display = format!("{}", port_name);

        let name_area = Rect {
            x: inner.x,
            y: start_y,
            width: inner.width,
            height: 1,
        };

        let text_style = if is_selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let name_widget = Paragraph::new(name_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(name_widget, name_area);

        // Second line: Port suffix (串口 or Port, no selection brackets)
        let suffix_display = format!("{}", port_suffix);

        let suffix_area = Rect {
            x: inner.x,
            y: start_y + 1,
            width: inner.width,
            height: 1,
        };

        let suffix_widget = Paragraph::new(suffix_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(suffix_widget, suffix_area);
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

    // Build node content with proper padding
    // Node height is 5, so inner height is 3 (5 - 2 borders), which is enough for 2 lines of text
    if inner.height >= 2 && inner.width >= 3 {
        // Calculate vertical centering for two lines of text
        let start_y = inner.y + (inner.height / 2).saturating_sub(1);

        // First line: new port label using i18n
        let new_label = lang().index.port_creation_new_label.as_str();
        let new_display = new_label.to_string();

        let new_area = Rect {
            x: inner.x,
            y: start_y,
            width: inner.width,
            height: 1,
        };

        let text_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let new_widget = Paragraph::new(new_display)
            .style(text_style)
            .alignment(Alignment::Center);
        frame.render_widget(new_widget, new_area);

        // Second line: Port type selector with outward-pointing brackets < option >
        let port_types = [
            lang().index.port_creation_ipc_pipe.as_str(),
            lang().index.port_creation_http_server.as_str(),
        ];

        let type_display = if port_type_index < port_types.len() {
            format!("< {} >", port_types[port_type_index])
        } else {
            format!("< {} >", port_types[0])
        };

        let type_area = Rect {
            x: inner.x,
            y: start_y + 1,
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
