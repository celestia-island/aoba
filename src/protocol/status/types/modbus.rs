use strum::EnumIter;

use crate::i18n::lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusConnectionMode {
    Master,
    Slave,
}

impl std::fmt::Display for ModbusConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusConnectionMode::Master => write!(f, "Master"),
            ModbusConnectionMode::Slave => write!(f, "Slave"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}

impl RegisterMode {
    pub const fn all() -> &'static [RegisterMode] {
        &[
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => unimplemented!("Invalid RegisterMode value: {v}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModbusRegisterItem {
    pub slave_id: u8,
    pub role: ModbusConnectionMode,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,

    pub req_success: u32,
    pub req_total: u32,
    pub next_poll_at: std::time::Instant,
    pub pending_requests: Vec<u8>, // simplified type for now
    pub values: Vec<u16>,          // Register values
}

#[derive(EnumIter, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityOption {
    None,
    Odd,
    Even,
}

impl std::fmt::Display for ParityOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParityOption::None => write!(f, "{}", lang().protocol.common.parity_none),
            ParityOption::Odd => write!(f, "{}", lang().protocol.common.parity_odd),
            ParityOption::Even => write!(f, "{}", lang().protocol.common.parity_even),
        }
    }
}
