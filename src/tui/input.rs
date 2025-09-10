use crossterm::event::KeyCode;

/// High-level user actions
#[derive(Clone)]
pub enum Action {
    Quit,
    LeavePage,
    EnterPage,
    EditToggle,
    AddRegister,
    DeleteRegister,
    PageUp,
    PageDown,
    JumpTop,
    JumpBottom,
    MoveNext,
    MovePrev,
    ClearError,
    EnterSubpage(char),
    ExitSubpage,
    SwitchNext,
    SwitchPrev,
    TogglePort,
    ToggleFollow,
    QuickScan,
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
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::JumpTop,
        KeyCode::End => Action::JumpBottom,
        KeyCode::Char('c') => Action::ClearError,
        KeyCode::Char('e') => Action::EditToggle,
        KeyCode::Char('n') => Action::AddRegister,
        KeyCode::Char('d') => Action::DeleteRegister,
        KeyCode::Char('p') => Action::ToggleFollow,
        KeyCode::Char('r') => Action::QuickScan,
        KeyCode::Tab => Action::SwitchNext,
        KeyCode::BackTab => Action::SwitchPrev,
        KeyCode::Enter => Action::TogglePort,

        _ => Action::None,
    }
}
