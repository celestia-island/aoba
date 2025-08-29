use crossterm::event::KeyCode;

/// High-level user actions
pub enum Action {
    Quit,
    FocusLeft,
    FocusRight,
    EditToggle,
    /// Start editing a specific field (index-based): 0=baud,1=parity,2=stopbits,>=3 -> register index
    StartEditField(usize),
    EditCancel,
    AddRegister,
    DeleteRegister,
    MoveNext,
    MovePrev,
    Refresh,
    ClearError,
    SwitchMode(usize),
    CycleMode,
    EnterSubpage(char),
    ExitSubpage,
    ShowModeSelector,
    SwitchNext,
    SwitchPrev,
    TogglePort,
    None,
}

/// Map a KeyCode to a high-level Action
pub fn map_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') => Action::Quit,
        // Left/h always moves focus left or exits subpage
        KeyCode::Left | KeyCode::Char('h') => Action::FocusLeft,
        // Only Right/l triggers entering the right area / subpage
        KeyCode::Right | KeyCode::Char('l') => Action::FocusRight,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveNext,
        KeyCode::Up | KeyCode::Char('k') => Action::MovePrev,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('c') => Action::ClearError,
        // Enter used for toggling port state (open/close)
        // Use 'e' to toggle edit mode in subpage forms
        KeyCode::Char('e') => Action::EditToggle,
        KeyCode::Esc => Action::EditCancel,
        KeyCode::Char('n') => Action::AddRegister,
        KeyCode::Char('d') => Action::DeleteRegister,
        KeyCode::Char('m') => Action::ShowModeSelector,
        KeyCode::Char('1') => Action::SwitchMode(0),
        KeyCode::Char('2') => Action::SwitchMode(1),
        KeyCode::Char('3') => Action::SwitchMode(2),
        // explicit subpage letter shortcuts removed; use Right/l to enter the current mode's page
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::Enter => Action::TogglePort,
        // keep Back key explicit exit optional (unused) - map to ExitSubpage for safety
        KeyCode::Char('b') => Action::ExitSubpage,
        _ => Action::None,
    }
}
