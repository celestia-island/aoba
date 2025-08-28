use chrono::{DateTime, Local};

use serialport::SerialPort;
use serialport::SerialPortInfo;
use std::collections::HashMap;
use std::time::Duration;

use crate::protocol::tty::available_ports_sorted;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RightMode {
    Master,
    SlaveStack,
    Listen,
}

#[derive(Debug)]
pub struct Status {
    pub ports: Vec<SerialPortInfo>,
    /// occupancy state for each port (same index as `ports`)
    pub port_states: Vec<PortState>,
    /// optional open handle when this app occupies the port
    pub port_handles: Vec<Option<Box<dyn SerialPort>>>,
    pub selected: usize,
    pub focus: Focus,
    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub error: Option<(String, DateTime<Local>)>,
    pub right_mode: RightMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

impl Status {
    pub fn new() -> Self {
        let ports = available_ports_sorted();
        let port_states = Self::detect_port_states(&ports);
        let port_handles = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            selected: 0,
            focus: Focus::Left,
            auto_refresh: true,
            last_refresh: None,
            error: None,
            right_mode: RightMode::Master,
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
        let port_states = ports.iter().map(|_| PortState::Free).collect();
        let port_handles = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            selected: 0,
            focus: Focus::Left,
            auto_refresh: false,
            last_refresh: None,
            error: None,
            right_mode: RightMode::Master,
        }
    }

    /// Re-scan available ports and reset selection if needed
    pub fn refresh(&mut self) {
        let new_ports = available_ports_sorted();
        // preserve known states and handles by port name
        let mut name_to_state: HashMap<String, PortState> = HashMap::new();
        let mut name_to_handle: HashMap<String, Option<Box<dyn SerialPort>>> = HashMap::new();
        for (i, p) in self.ports.iter().enumerate() {
            if let Some(s) = self.port_states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            // take ownership of existing handle if any
            if let Some(h) = self.port_handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
        }
        self.ports = new_ports;
        // rebuild port_states and port_handles preserving by name
        let mut new_states: Vec<PortState> = Vec::with_capacity(self.ports.len());
        let mut new_handles: Vec<Option<Box<dyn SerialPort>>> =
            Vec::with_capacity(self.ports.len());
        for p in self.ports.iter() {
            if let Some(s) = name_to_state.remove(&p.port_name) {
                new_states.push(s);
            } else if Self::probe_port_free(&p.port_name) {
                new_states.push(PortState::Free);
            } else {
                new_states.push(PortState::OccupiedByOther);
            }
            // move back handle if existed
            if let Some(h) = name_to_handle.remove(&p.port_name) {
                new_handles.push(h);
            } else {
                new_handles.push(None);
            }
        }
        self.port_states = new_states;
        self.port_handles = new_handles;
        if self.ports.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.ports.len() {
            self.selected = 0;
        }
        self.last_refresh = Some(Local::now());
    }

    fn probe_port_free(port_name: &str) -> bool {
        // try to open the port briefly; if succeed it's free (we immediately drop it)
        match serialport::new(port_name, 9600)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn detect_port_states(ports: &Vec<SerialPortInfo>) -> Vec<PortState> {
        ports
            .iter()
            .map(|p| {
                if Self::probe_port_free(&p.port_name) {
                    PortState::Free
                } else {
                    PortState::OccupiedByOther
                }
            })
            .collect()
    }

    /// Toggle the selected port's occupancy by this app. No-op if other program occupies the port.
    pub fn toggle_selected_port(&mut self) {
        if self.ports.is_empty() {
            return;
        }
        let i = self.selected;
        if let Some(state) = self.port_states.get_mut(i) {
            match state {
                PortState::Free => {
                    // try to open and hold the port
                    let port_name = self.ports[i].port_name.clone();
                    match serialport::new(&port_name, 9600)
                        .timeout(Duration::from_millis(200))
                        .open()
                    {
                        Ok(handle) => {
                            if let Some(hslot) = self.port_handles.get_mut(i) {
                                *hslot = Some(handle);
                            }
                            *state = PortState::OccupiedByThis;
                        }
                        Err(e) => {
                            // cannot open -> likely occupied by other
                            *state = PortState::OccupiedByOther;
                            self.set_error(format!("failed to open {}: {}", port_name, e));
                        }
                    }
                }
                PortState::OccupiedByThis => {
                    // drop handle
                    if let Some(hslot) = self.port_handles.get_mut(i) {
                        *hslot = None;
                    }
                    *state = PortState::Free;
                }
                PortState::OccupiedByOther => {
                    // don't change
                }
            }
        }
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
        let mut app = Status::with_ports(ports);
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
        let mut app = Status::with_ports(ports);
        assert_eq!(app.focus, Focus::Left);
        // Call refresh (may change ports depending on environment)
        app.refresh();
        // Ensure selected is in bounds
        if !app.ports.is_empty() {
            assert!(app.selected < app.ports.len());
        }
    }
}
