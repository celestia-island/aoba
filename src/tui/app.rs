use chrono::{DateTime, Local};

use serialport::SerialPortInfo;

use crate::protocol::tty::available_ports_sorted;

#[derive(Debug, PartialEq, Eq)]
pub enum Focus {
    Left,
    Right,
}

pub struct App {
    pub ports: Vec<SerialPortInfo>,
    pub selected: usize,
    pub focus: Focus,
    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,
    // Error message with timestamp when it was set
    pub error: Option<(String, DateTime<Local>)>,
}

impl App {
    pub fn new() -> Self {
        let ports = available_ports_sorted();
        Self {
            ports,
            selected: 0,
            focus: Focus::Left,
            auto_refresh: true,
            last_refresh: None,
            error: None,
        }
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some((msg.into(), Local::now()));
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Create an App with provided ports (useful for tests)
    #[cfg(test)]
    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        Self {
            ports,
            selected: 0,
            focus: Focus::Left,
            auto_refresh: false,
            last_refresh: None,
            error: None,
        }
    }

    /// Re-scan available ports and reset selection if needed
    pub fn refresh(&mut self) {
        self.ports = available_ports_sorted();
        if self.ports.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.ports.len() {
            self.selected = 0;
        }
        self.last_refresh = Some(Local::now());
    }

    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh = !self.auto_refresh;
    }
    pub fn next(&mut self) {
        if !self.ports.is_empty() {
            self.selected = (self.selected + 1) % self.ports.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.ports.is_empty() {
            if self.selected == 0 {
                self.selected = self.ports.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn fake_port(name: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type: SerialPortType::Unknown,
        }
    }

    #[test]
    fn test_navigation() {
        let ports = vec![fake_port("COM1"), fake_port("COM2")];
        let mut app = App::with_ports(ports);
        assert_eq!(app.selected, 0);
        app.next();
        assert_eq!(app.selected, 1);
        app.next();
        assert_eq!(app.selected, 0);
        app.prev();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_focus_and_refresh() {
        let ports = vec![fake_port("COM1")];
        let mut app = App::with_ports(ports);
        assert_eq!(app.focus, Focus::Left);
    // Call refresh (may change ports depending on environment)
        app.refresh();
    // Ensure selected is in bounds
        if !app.ports.is_empty() {
            assert!(app.selected < app.ports.len());
        }
    }
}
