use crossterm::event::KeyCode;

/// High-level user actions
pub enum Action {
    Quit,
    LeavePage,
    EnterPage,
    EditToggle,
    AddRegister,
    DeleteRegister,
    MoveNext,
    MovePrev,
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
        KeyCode::Esc | KeyCode::Char('h') => Action::LeavePage,
        KeyCode::Char('l') => Action::EnterPage,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveNext,
        KeyCode::Up | KeyCode::Char('k') => Action::MovePrev,
        KeyCode::Char('c') => Action::ClearError,

        KeyCode::Char('e') => Action::EditToggle,
        KeyCode::Char('n') => Action::AddRegister,
        KeyCode::Char('d') => Action::DeleteRegister,
        KeyCode::Char('m') => Action::ShowModeSelector,
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::Enter => Action::TogglePort,

        _ => Action::None,
    }
}
