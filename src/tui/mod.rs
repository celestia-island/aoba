use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    widgets::*,
};
use std::io;

pub fn start() -> Result<()> {
    log::info!("[TUI] aoba TUI starting...");
    let mut stdout = io::stdout();
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

    terminal.clear().ok();
    terminal
        .draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Length(5),
                    Constraint::Percentage(40),
                ])
                .split(area);

            let block = Block::default()
                .title("Aoba - Multi-protocol Debug & Simulation Tool")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL);
            let paragraph = Paragraph::new("Welcome to the TUI mode!")
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(paragraph, chunks[1]);

            let exit_paragraph =
                Paragraph::new("Press Enter to exit...").alignment(Alignment::Center);
            f.render_widget(exit_paragraph, chunks[2]);
        })
        .ok();

    let _ = io::stdin().read_line(&mut String::new());
    // TODO: Listen for user input

    Ok(())
}
