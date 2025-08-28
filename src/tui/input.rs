use crossterm::event::KeyCode;

/// High-level user actions
pub enum Action {
    Quit,
    FocusLeft,
    FocusRight,
    MoveNext,
    MovePrev,
    Refresh,
    ClearError,
    SwitchMode(usize),
    SwitchNext,
    SwitchPrev,
    TogglePort,
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
        KeyCode::Char('c') => Action::ClearError,
        KeyCode::Char('1') => Action::SwitchMode(0),
        KeyCode::Char('2') => Action::SwitchMode(1),
        KeyCode::Char('3') => Action::SwitchMode(2),
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::Enter => Action::TogglePort,
        _ => Action::None,
    }
}
