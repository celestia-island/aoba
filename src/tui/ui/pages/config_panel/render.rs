use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::{i18n::lang, tui::ui::pages::config_panel::components::render_kv_data};

pub fn page_bottom_hints() -> Vec<Vec<String>> {
    vec![
        vec![lang().hotkeys.hint_move_vertical.as_str().to_string()],
        vec![lang().hotkeys.press_enter_modify.as_str().to_string()],
    ]
}

pub fn render(frame: &mut Frame, area: Rect) -> Result<()> {
    // Get the label and value data
    let (labels, values) = render_kv_data()?;

    // Create a 4:6 ratio layout (40% labels, 60% values)
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    let left_area = chunks[0];
    let right_area = chunks[1];

    // Create bordered blocks
    let left_block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::left(1));

    let right_block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::left(1));

    // Render labels in left column
    let labels_paragraph = Paragraph::new(labels).block(left_block);

    // Render values in right column
    let values_paragraph = Paragraph::new(values).block(right_block);

    frame.render_widget(labels_paragraph, left_area);
    frame.render_widget(values_paragraph, right_area);

    Ok(())
}
