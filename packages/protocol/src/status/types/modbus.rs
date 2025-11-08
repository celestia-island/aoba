use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt};
use strum::{EnumIter, FromRepr};

use crate::i18n::lang;

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ModbusConnectionMode {
    Master,
    Slave {
        current_request_at_station_index: usize,
    },
}

impl ModbusConnectionMode {
    pub fn is_master(&self) -> bool {
        matches!(self, ModbusConnectionMode::Master)
    }

    pub fn is_slave(&self) -> bool {
        matches!(self, ModbusConnectionMode::Slave { .. })
    }

    pub fn default_master() -> Self {
        ModbusConnectionMode::Master
    }

    pub fn default_slave() -> Self {
        ModbusConnectionMode::Slave {
            current_request_at_station_index: 0,
        }
    }

    // Helper methods for UI compatibility
    pub fn all_variants() -> Vec<Self> {
        vec![Self::default_master(), Self::default_slave()]
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::default_master(),
            1 => Self::default_slave(),
            _ => Self::default_master(),
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            ModbusConnectionMode::Master => 0,
            ModbusConnectionMode::Slave { .. } => 1,
        }
    }
}

impl std::fmt::Display for ModbusConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusConnectionMode::Master => {
                write!(f, "{}", lang().protocol.modbus.role_master)
            }
            ModbusConnectionMode::Slave { .. } => {
                write!(f, "{}", lang().protocol.modbus.role_slave)
            }
        }
    }
}

/// Station-level configuration primitive shared by CLI, TUI and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationMode {
    Master,
    Slave,
}

impl StationMode {
    pub fn is_master(self) -> bool {
        matches!(self, StationMode::Master)
    }

    pub fn is_slave(self) -> bool {
        matches!(self, StationMode::Slave)
    }
}

impl fmt::Display for StationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StationMode::Master => write!(f, "master"),
            StationMode::Slave => write!(f, "slave"),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    // Note: custom conversion helpers removed per request. Use `FromRepr::from_repr` and
    // direct casts where needed (e.g. `as u8` / `as usize`).
}

impl TryFrom<&str> for RegisterMode {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.eq_ignore_ascii_case("holding") || value.eq_ignore_ascii_case("holdings") {
            Ok(Self::Holding)
        } else if value.eq_ignore_ascii_case("input") || value.eq_ignore_ascii_case("inputs") {
            Ok(Self::Input)
        } else if value.eq_ignore_ascii_case("coil") || value.eq_ignore_ascii_case("coils") {
            Ok(Self::Coils)
        } else if value.eq_ignore_ascii_case("discrete")
            || value.eq_ignore_ascii_case("discrete_input")
            || value.eq_ignore_ascii_case("discrete_inputs")
            || value.eq_ignore_ascii_case("discreteinputs")
        {
            Ok(Self::DiscreteInputs)
        } else {
            Err(())
        }
    }
}

impl std::fmt::Display for RegisterMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterMode::Coils => write!(f, "{}", lang().protocol.modbus.reg_type_coils),
            RegisterMode::DiscreteInputs => {
                write!(f, "{}", lang().protocol.modbus.reg_type_discrete_inputs)
            }
            RegisterMode::Holding => write!(f, "{}", lang().protocol.modbus.reg_type_holding),
            RegisterMode::Input => write!(f, "{}", lang().protocol.modbus.reg_type_input),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRange {
    pub address_start: u16,
    pub length: u16,
    #[serde(default)]
    pub initial_values: Vec<u16>,
}

impl Default for RegisterRange {
    fn default() -> Self {
        Self {
            address_start: 0,
            length: 10,
            initial_values: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegisterMap {
    #[serde(default)]
    pub coils: Vec<RegisterRange>,
    #[serde(default)]
    pub discrete_inputs: Vec<RegisterRange>,
    #[serde(default)]
    pub holding: Vec<RegisterRange>,
    #[serde(default)]
    pub input: Vec<RegisterRange>,
}

/// Canonical Modbus station configuration shared across the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    #[serde(rename = "id")]
    pub station_id: u8,
    pub mode: StationMode,
    pub map: RegisterMap,
}

impl Default for StationConfig {
    fn default() -> Self {
        Self {
            station_id: 1,
            mode: StationMode::Master,
            map: RegisterMap::default(),
        }
    }
}

impl StationConfig {
    /// Convenience constructor for the common single-register-range case.
    pub fn single_range(
        station_id: u8,
        mode: StationMode,
        register_mode: RegisterMode,
        start_address: u16,
        register_count: u16,
        register_values: Option<Vec<u16>>,
    ) -> Self {
        let mut map = RegisterMap::default();
        let range = RegisterRange {
            address_start: start_address,
            length: register_count,
            initial_values: register_values.unwrap_or_default(),
        };
        match register_mode {
            RegisterMode::Coils => map.coils.push(range),
            RegisterMode::DiscreteInputs => map.discrete_inputs.push(range),
            RegisterMode::Holding => map.holding.push(range),
            RegisterMode::Input => map.input.push(range),
        }

        Self {
            station_id,
            mode,
            map,
        }
    }

    pub fn station_id(&self) -> u8 {
        self.station_id
    }

    pub fn is_master(&self) -> bool {
        self.mode.is_master()
    }

    pub fn is_slave(&self) -> bool {
        self.mode.is_slave()
    }

    fn first_range(&self) -> Option<(RegisterMode, &RegisterRange)> {
        self.map
            .coils
            .first()
            .map(|r| (RegisterMode::Coils, r))
            .or_else(|| {
                self.map
                    .discrete_inputs
                    .first()
                    .map(|r| (RegisterMode::DiscreteInputs, r))
            })
            .or_else(|| self.map.holding.first().map(|r| (RegisterMode::Holding, r)))
            .or_else(|| self.map.input.first().map(|r| (RegisterMode::Input, r)))
    }

    fn first_range_mut(&mut self) -> Option<(RegisterMode, &mut RegisterRange)> {
        if let Some(range) = self.map.coils.first_mut() {
            return Some((RegisterMode::Coils, range));
        }
        if let Some(range) = self.map.discrete_inputs.first_mut() {
            return Some((RegisterMode::DiscreteInputs, range));
        }
        if let Some(range) = self.map.holding.first_mut() {
            return Some((RegisterMode::Holding, range));
        }
        if let Some(range) = self.map.input.first_mut() {
            return Some((RegisterMode::Input, range));
        }
        None
    }

    pub fn register_mode(&self) -> RegisterMode {
        self.first_range()
            .map(|(mode, _)| mode)
            .expect("StationConfig must contain at least one register range")
    }

    pub fn start_address(&self) -> u16 {
        self.first_range()
            .map(|(_, range)| range.address_start)
            .expect("StationConfig must contain at least one register range")
    }

    pub fn register_count(&self) -> u16 {
        self.first_range()
            .map(|(_, range)| range.length)
            .expect("StationConfig must contain at least one register range")
    }

    pub fn register_values(&self) -> Option<&[u16]> {
        self.first_range().and_then(|(_, range)| {
            if range.initial_values.is_empty() {
                None
            } else {
                Some(range.initial_values.as_slice())
            }
        })
    }

    pub fn register_values_owned(&self) -> Option<Vec<u16>> {
        self.register_values().map(|slice| slice.to_vec())
    }

    /// Total number of register ranges across all register modes.
    pub fn range_count(&self) -> usize {
        self.map.coils.len()
            + self.map.discrete_inputs.len()
            + self.map.holding.len()
            + self.map.input.len()
    }

    /// Whether the station is defined with exactly one register range.
    pub fn is_single_range(&self) -> bool {
        self.range_count() == 1
    }

    pub fn set_station_id(&mut self, station_id: u8) {
        self.station_id = station_id;
    }

    pub fn set_mode(&mut self, mode: StationMode) {
        self.mode = mode;
    }

    pub fn set_register_values(&mut self, values: Option<Vec<u16>>) {
        let (_, range) = self
            .first_range_mut()
            .expect("StationConfig must contain at least one register range");
        range.initial_values = values.unwrap_or_default();
    }

    pub fn set_single_range(
        &mut self,
        register_mode: RegisterMode,
        start_address: u16,
        register_count: u16,
    ) {
        let mut map = RegisterMap::default();
        let range = RegisterRange {
            address_start: start_address,
            length: register_count,
            initial_values: self.register_values_owned().unwrap_or_default(),
        };

        match register_mode {
            RegisterMode::Coils => map.coils.push(range),
            RegisterMode::DiscreteInputs => map.discrete_inputs.push(range),
            RegisterMode::Holding => map.holding.push(range),
            RegisterMode::Input => map.input.push(range),
        }

        self.map = map;
    }
}

#[derive(Debug, Clone)]
pub struct ModbusRegisterItem {
    pub station_id: u8,
    pub register_mode: RegisterMode,
    pub register_address: u16,
    pub register_length: u16,
    pub last_values: Vec<u16>,

    pub req_success: u32,
    pub req_total: u32,
    pub next_poll_at: std::time::Instant,
    pub last_request_time: Option<std::time::Instant>, // For timeout tracking in slave mode
    pub last_response_time: Option<std::time::Instant>, // For throttling responses in master mode
    pub pending_requests: Vec<u8>,                     // simplified type for now
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter, FromRepr)]
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
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter, FromRepr)]
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

// Custom conversion helpers removed. Use direct casts and `FromRepr::from_repr` as needed.
impl DataBitsOption {}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
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
            BaudRateSelector::Custom { baud } => baud,
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
            BaudRateOption::Custom(v) => write!(f, "{v}"),
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

// Custom conversion helpers removed. Use direct casts and `FromRepr::from_repr` as needed.
impl StopBitsOption {}
