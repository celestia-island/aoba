use crossterm::event::KeyCode;

/// High-level user actions
pub enum Action {
    Quit,
    FocusLeft,
    FocusRight,
    MoveNext,
    MovePrev,
    Refresh,
    ToggleAutoRefresh,
    None,
}

/// Map a KeyCode to a high-level Action
pub fn map_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Left | KeyCode::Char('h') => Action::FocusLeft,
        KeyCode::Right | KeyCode::Char('l') => Action::FocusRight,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveNext,
        KeyCode::Up | KeyCode::Char('k') => Action::MovePrev,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('a') => Action::ToggleAutoRefresh,
        _ => Action::None,
    }
}
