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
    pub connection_mode: ModbusConnectionMode,
    pub station_id: u8,
    pub register_mode: RegisterMode,
    pub register_address: u16,
    pub register_length: u16,

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

/// UI enums for DataBits and StopBits so they can be used with selector_spans
#[derive(EnumIter, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBitsOption {
    Five,
    Six,
    Seven,
    Eight,
}

impl std::fmt::Display for DataBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataBitsOption::Five => write!(f, "5"),
            DataBitsOption::Six => write!(f, "6"),
            DataBitsOption::Seven => write!(f, "7"),
            DataBitsOption::Eight => write!(f, "8"),
        }
    }
}

impl DataBitsOption {
    pub fn as_u8(self) -> u8 {
        match self {
            DataBitsOption::Five => 5u8,
            DataBitsOption::Six => 6u8,
            DataBitsOption::Seven => 7u8,
            DataBitsOption::Eight => 8u8,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            5 => DataBitsOption::Five,
            6 => DataBitsOption::Six,
            7 => DataBitsOption::Seven,
            _ => DataBitsOption::Eight,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            DataBitsOption::Five => 0usize,
            DataBitsOption::Six => 1usize,
            DataBitsOption::Seven => 2usize,
            DataBitsOption::Eight => 3usize,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => DataBitsOption::Five,
            1 => DataBitsOption::Six,
            2 => DataBitsOption::Seven,
            _ => DataBitsOption::Eight,
        }
    }
}

#[derive(EnumIter, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBitsOption {
    One,
    Two,
}

impl std::fmt::Display for StopBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopBitsOption::One => write!(f, "1"),
            StopBitsOption::Two => write!(f, "2"),
        }
    }
}

/// Baud rate presets including a Custom placeholder. Custom does not carry
/// the numeric value here; the actual runtime baud is stored in the port
/// configuration as a `u32`. This enum is used for selector rendering.
#[derive(EnumIter, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaudRateSelector {
    B110,
    B300,
    B600,
    B1200,
    B2400,
    B4800,
    B9600,
    B14400,
    B19200,
    B38400,
    B57600,
    B115200,
    B230400,
    B460800,
    B921600,
    B1000000,
    B2000000,
    Custom { baud: u32 },
}

impl std::fmt::Display for BaudRateSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaudRateSelector::Custom { .. } => write!(f, "{}", lang().protocol.common.custom),
            other => write!(f, "{}", other.as_u32()),
        }
    }
}

impl BaudRateSelector {
    pub fn as_u32(self) -> u32 {
        match self {
            BaudRateSelector::B110 => 110u32,
            BaudRateSelector::B300 => 300u32,
            BaudRateSelector::B600 => 600u32,
            BaudRateSelector::B1200 => 1200u32,
            BaudRateSelector::B2400 => 2400u32,
            BaudRateSelector::B4800 => 4800u32,
            BaudRateSelector::B9600 => 9600u32,
            BaudRateSelector::B14400 => 14400u32,
            BaudRateSelector::B19200 => 19200u32,
            BaudRateSelector::B38400 => 38400u32,
            BaudRateSelector::B57600 => 57600u32,
            BaudRateSelector::B115200 => 115200u32,
            BaudRateSelector::B230400 => 230400u32,
            BaudRateSelector::B460800 => 460800u32,
            BaudRateSelector::B921600 => 921600u32,
            BaudRateSelector::B1000000 => 1000000u32,
            BaudRateSelector::B2000000 => 2000000u32,
            BaudRateSelector::Custom { .. } => 0u32,
        }
    }

    pub fn from_u32(v: u32) -> Self {
        match v {
            110 => BaudRateSelector::B110,
            300 => BaudRateSelector::B300,
            600 => BaudRateSelector::B600,
            1200 => BaudRateSelector::B1200,
            2400 => BaudRateSelector::B2400,
            4800 => BaudRateSelector::B4800,
            9600 => BaudRateSelector::B9600,
            14400 => BaudRateSelector::B14400,
            19200 => BaudRateSelector::B19200,
            38400 => BaudRateSelector::B38400,
            57600 => BaudRateSelector::B57600,
            115200 => BaudRateSelector::B115200,
            230400 => BaudRateSelector::B230400,
            460800 => BaudRateSelector::B460800,
            921600 => BaudRateSelector::B921600,
            1000000 => BaudRateSelector::B1000000,
            2000000 => BaudRateSelector::B2000000,
            _ => BaudRateSelector::Custom { baud: v },
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            BaudRateSelector::B110 => 0usize,
            BaudRateSelector::B300 => 1usize,
            BaudRateSelector::B600 => 2usize,
            BaudRateSelector::B1200 => 3usize,
            BaudRateSelector::B2400 => 4usize,
            BaudRateSelector::B4800 => 5usize,
            BaudRateSelector::B9600 => 6usize,
            BaudRateSelector::B14400 => 7usize,
            BaudRateSelector::B19200 => 8usize,
            BaudRateSelector::B38400 => 9usize,
            BaudRateSelector::B57600 => 10usize,
            BaudRateSelector::B115200 => 11usize,
            BaudRateSelector::B230400 => 12usize,
            BaudRateSelector::B460800 => 13usize,
            BaudRateSelector::B921600 => 14usize,
            BaudRateSelector::B1000000 => 15usize,
            BaudRateSelector::B2000000 => 16usize,
            BaudRateSelector::Custom { .. } => 17usize,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => BaudRateSelector::B110,
            1 => BaudRateSelector::B300,
            2 => BaudRateSelector::B600,
            3 => BaudRateSelector::B1200,
            4 => BaudRateSelector::B2400,
            5 => BaudRateSelector::B4800,
            6 => BaudRateSelector::B9600,
            7 => BaudRateSelector::B14400,
            8 => BaudRateSelector::B19200,
            9 => BaudRateSelector::B38400,
            10 => BaudRateSelector::B57600,
            11 => BaudRateSelector::B115200,
            12 => BaudRateSelector::B230400,
            13 => BaudRateSelector::B460800,
            14 => BaudRateSelector::B921600,
            15 => BaudRateSelector::B1000000,
            16 => BaudRateSelector::B2000000,
            _ => BaudRateSelector::Custom { baud: 0 },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaudRateOption {
    Preset(BaudRateSelector),
    Custom(u32),
}

impl std::fmt::Display for BaudRateOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaudRateOption::Preset(p) => write!(f, "{}", p.as_u32()),
            BaudRateOption::Custom(v) => write!(f, "{}", v),
        }
    }
}

impl BaudRateOption {
    pub fn from_u32(v: u32) -> Self {
        match BaudRateSelector::from_u32(v) {
            BaudRateSelector::Custom { .. } => BaudRateOption::Custom(v),
            s => BaudRateOption::Preset(s),
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            BaudRateOption::Preset(s) => s.as_u32(),
            BaudRateOption::Custom(v) => v,
        }
    }
}

impl StopBitsOption {
    pub fn as_u8(self) -> u8 {
        match self {
            StopBitsOption::One => 1u8,
            StopBitsOption::Two => 2u8,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            2 => StopBitsOption::Two,
            _ => StopBitsOption::One,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            StopBitsOption::One => 0usize,
            StopBitsOption::Two => 1usize,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            1 => StopBitsOption::Two,
            _ => StopBitsOption::One,
        }
    }
}
