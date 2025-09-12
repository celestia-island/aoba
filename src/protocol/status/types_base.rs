use chrono::{DateTime, Local};
use std::sync::{Arc, RwLock};
use yuuka::derive_struct;

use serialport::{SerialPort, SerialPortInfo};

use crate::protocol::{runtime::PortRuntimeHandle, tty::PortExtra};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Clone)]
pub struct SerialPortWrapper(Arc<RwLock<Box<dyn SerialPort + Send>>>);

impl std::fmt::Debug for SerialPortWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SerialPortWrapper").finish()
    }
}

derive_struct! {
    pub Status {
        ports: [{
            info?: SerialPortInfo,
            extra: PortExtra,
            state: enum PortState {
                Free,
                OccupiedByThis,
                OccupiedByOther,
            },
            handle?: SerialPortWrapper,
            runtimes?: PortRuntimeHandle,
        }] = vec![],

        page: enum Page {
            Entry {
                cursor?: enum EntryCursor {
                    Com { idx: usize },
                    About,
                },
            },
            ModbusConfig {
                selected_port: usize,
            },
            ModbusDashboard {
                selected_port: usize,

                cursor: usize,
                editing_field?: enum EditingField {
                    Loop,
                    Baud,
                    Parity,
                    StopBits,
                    DataBits,
                    GlobalInterval,
                    GlobalTimeout,
                    RegisterField {
                        idx: usize,
                        field: enum RegisterField {
                            SlaveId,
                            Mode,
                            Address,
                            Length,
                        },
                    },
                },
                input_buffer: String,
                edit_choice_index: Option<usize>,
                edit_confirmed: bool,

                master_cursor: usize,
                master_field_selected: bool,
                master_field_editing: bool,
                master_edit_field?: enum MasterEditField {
                    Role,
                    Id,
                    Type,
                    Start,
                    End,
                    Counter,
                    Value(u16),
                },
                master_edit_index: Option<usize>,
                master_input_buffer: String,
                poll_round_index: usize,
                in_flight_reg_index: Option<usize>,
            },
            ModbusLog {
                selected_port: usize,
                logs: [LogEntry {
                    when: DateTime<Local>,
                    raw: String,
                    parsed?: ParsedRequest {
                        origin: String,
                        rw: String,
                        command: String,
                        slave_id: u8,
                        address: u16,
                        length: u16,
                    },
                }],
                log_selected: usize,
                log_view_offset: usize,
                log_auto_scroll: bool,
                log_clear_pending: bool,
                input_mode: InputMode,
                input_editing: bool,
                input_buffer: String,
            },
            About {
                view_offset: usize,
            }
        },
        temporarily: {
            input_raw_buffer: String,
            input_mode: InputMode = InputMode::Ascii,
        }
    }
}
