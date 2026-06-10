#![allow(clippy::wildcard_enum_match_arm)]
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt};
use strum::{EnumIter, FromRepr};

use crate::utils::i18n::lang;

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ModbusConnectionMode {
    Master,
    Slave {
        current_request_at_station_index: usize,
    },
}

impl ModbusConnectionMode {
    #[must_use]
    pub const fn is_master(&self) -> bool {
        matches!(self, Self::Master)
    }

    #[must_use]
    pub const fn is_slave(&self) -> bool {
        matches!(self, Self::Slave { .. })
    }

    #[must_use]
    pub const fn default_master() -> Self {
        Self::Master
    }

    #[must_use]
    pub const fn default_slave() -> Self {
        Self::Slave {
            current_request_at_station_index: 0,
        }
    }

    // Helper methods for UI compatibility
    #[must_use]
    pub fn all_variants() -> Vec<Self> {
        vec![Self::default_master(), Self::default_slave()]
    }

    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::default_slave(),
            _ => Self::default_master(),
        }
    }

    #[must_use]
    pub const fn to_index(&self) -> usize {
        match self {
            Self::Master => 0,
            Self::Slave { .. } => 1,
        }
    }
}

impl std::fmt::Display for ModbusConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Master => {
                write!(f, "{}", lang().protocol.modbus.role_master)
            }
            Self::Slave { .. } => {
                write!(f, "{}", lang().protocol.modbus.role_slave)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum ModbusMasterDataSourceKind {
    Manual,
    MqttServer,
    HttpServer,
    IpcPipe,
    PortForwarding,
}

impl ModbusMasterDataSourceKind {
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Manual,
            Self::MqttServer,
            Self::HttpServer,
            Self::IpcPipe,
            Self::PortForwarding,
        ]
    }

    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Self::all().get(index).copied().unwrap_or(Self::Manual)
    }

    #[must_use]
    pub fn to_index(self) -> usize {
        Self::all()
            .iter()
            .position(|kind| *kind == self)
            .unwrap_or(0)
    }

    #[must_use]
    pub const fn value_kind(self) -> ModbusMasterDataSourceValueKind {
        match self {
            Self::Manual => ModbusMasterDataSourceValueKind::None,
            Self::MqttServer => ModbusMasterDataSourceValueKind::Url,
            Self::HttpServer => ModbusMasterDataSourceValueKind::Port,
            Self::IpcPipe => ModbusMasterDataSourceValueKind::Path,
            Self::PortForwarding => ModbusMasterDataSourceValueKind::PortName,
        }
    }
}

impl fmt::Display for ModbusMasterDataSourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Manual => lang().protocol.modbus.data_source_manual.clone(),
            Self::MqttServer => {
                lang().protocol.modbus.data_source_mqtt.clone()
            }
            Self::HttpServer => {
                lang().protocol.modbus.data_source_http.clone()
            }
            Self::IpcPipe => lang().protocol.modbus.data_source_ipc.clone(),
            Self::PortForwarding => {
                lang().protocol.modbus.data_source_port_forwarding.clone()
            }
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusMasterDataSourceValueKind {
    None,
    Port,
    Url,
    Path,
    PortName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum ModbusMasterDataSource {
    #[default]
    Manual,
    MqttServer {
        url: String,
    },
    HttpServer {
        port: u16,
    },
    IpcPipe {
        path: String,
    },
    PortForwarding {
        source_port: String,
    },
}

impl ModbusMasterDataSource {
    #[must_use]
    pub const fn kind(&self) -> ModbusMasterDataSourceKind {
        match self {
            Self::Manual => ModbusMasterDataSourceKind::Manual,
            Self::MqttServer { .. } => ModbusMasterDataSourceKind::MqttServer,
            Self::HttpServer { .. } => ModbusMasterDataSourceKind::HttpServer,
            Self::IpcPipe { .. } => ModbusMasterDataSourceKind::IpcPipe,
            Self::PortForwarding { .. } => {
                ModbusMasterDataSourceKind::PortForwarding
            }
        }
    }

    #[must_use]
    pub const fn value_kind(&self) -> ModbusMasterDataSourceValueKind {
        self.kind().value_kind()
    }

    #[must_use]
    pub const fn with_kind(kind: ModbusMasterDataSourceKind) -> Self {
        match kind {
            ModbusMasterDataSourceKind::Manual => Self::Manual,
            ModbusMasterDataSourceKind::MqttServer => Self::MqttServer { url: String::new() },
            ModbusMasterDataSourceKind::HttpServer => Self::HttpServer { port: 8080 },
            ModbusMasterDataSourceKind::IpcPipe => Self::IpcPipe {
                path: String::new(),
            },
            ModbusMasterDataSourceKind::PortForwarding => Self::PortForwarding {
                source_port: String::new(),
            },
        }
    }

    pub fn set_kind(&mut self, kind: ModbusMasterDataSourceKind) {
        *self = Self::with_kind(kind);
    }

    #[must_use]
    pub const fn get_port(&self) -> Option<u16> {
        match self {
            Self::HttpServer { port } => Some(*port),
                    _ => None,
        }
    }

    pub const fn set_port(&mut self, new_port: u16) {
        if let Self::HttpServer { port } = self {
            *port = new_port;
        }
    }

    #[must_use]
    pub const fn get_text(&self) -> Option<&str> {
        match self {
            Self::MqttServer { url } => Some(url.as_str()),
            Self::IpcPipe { path } => Some(path.as_str()),
            Self::PortForwarding { source_port } => Some(source_port.as_str()),
                    _ => None,
        }
    }

    pub fn set_text(&mut self, value: String) {
        match self {
            Self::MqttServer { url } => {
                *url = value;
            }
            Self::IpcPipe { path } => {
                *path = value;
            }
            Self::PortForwarding { source_port } => {
                *source_port = value;
            }
            Self::Manual | Self::HttpServer { .. } => {}
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
    #[must_use]
    pub const fn is_master(self) -> bool {
        matches!(self, Self::Master)
    }

    #[must_use]
    pub const fn is_slave(self) -> bool {
        matches!(self, Self::Slave)
    }
}

impl fmt::Display for StationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Master => write!(f, "master"),
            Self::Slave => write!(f, "slave"),
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
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }

    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => {
                log::warn!("Invalid RegisterMode value: {v}, defaulting to Holding");
                Self::Holding
            }
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
            Self::Coils => write!(f, "{}", lang().protocol.modbus.reg_type_coils),
            Self::DiscreteInputs => {
                write!(f, "{}", lang().protocol.modbus.reg_type_discrete_inputs)
            }
            Self::Holding => write!(f, "{}", lang().protocol.modbus.reg_type_holding),
            Self::Input => write!(f, "{}", lang().protocol.modbus.reg_type_input),
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
    #[must_use]
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

    #[must_use]
    pub const fn station_id(&self) -> u8 {
        self.station_id
    }

    #[must_use]
    pub const fn is_master(&self) -> bool {
        self.mode.is_master()
    }

    #[must_use]
    pub const fn is_slave(&self) -> bool {
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

    #[must_use]
    pub fn register_mode(&self) -> RegisterMode {
        self.first_range()
            .map_or(RegisterMode::Holding, |(mode, _)| mode)
    }

    #[must_use]
    pub fn start_address(&self) -> u16 {
        self.first_range()
            .map_or(0, |(_, range)| range.address_start)
    }

    #[must_use]
    pub fn register_count(&self) -> u16 {
        self.first_range()
            .map_or(0, |(_, range)| range.length)
    }

    #[must_use]
    pub fn register_values(&self) -> Option<&[u16]> {
        self.first_range().and_then(|(_, range)| {
            if range.initial_values.is_empty() {
                None
            } else {
                Some(range.initial_values.as_slice())
            }
        })
    }

    #[must_use]
    pub fn register_values_owned(&self) -> Option<Vec<u16>> {
        self.register_values().map(<[u16]>::to_vec)
    }

    /// Total number of register ranges across all register modes.
    #[must_use]
    pub const fn range_count(&self) -> usize {
        self.map.coils.len()
            + self.map.discrete_inputs.len()
            + self.map.holding.len()
            + self.map.input.len()
    }

    /// Whether the station is defined with exactly one register range.
    #[must_use]
    pub const fn is_single_range(&self) -> bool {
        self.range_count() == 1
    }

    pub const fn set_station_id(&mut self, station_id: u8) {
        self.station_id = station_id;
    }

    pub const fn set_mode(&mut self, mode: StationMode) {
        self.mode = mode;
    }

    pub fn set_register_values(&mut self, values: Option<Vec<u16>>) {
        if let Some((_, range)) = self.first_range_mut() {
            range.initial_values = values.unwrap_or_default();
        }
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

/// Standardized response used for HTTP server POST /stations replies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationsResponse {
    pub success: bool,
    pub message: String,
    pub stations: Vec<StationConfig>,
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

    /// Tracks which register indices have pending writes: (`register_index`, `new_value`)
    pub pending_writes: std::collections::HashMap<usize, u16>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
pub enum ParityOption {
    None,
    Odd,
    Even,
}

impl std::fmt::Display for ParityOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", lang().protocol.common.parity_none),
            Self::Odd => write!(f, "{}", lang().protocol.common.parity_odd),
            Self::Even => write!(f, "{}", lang().protocol.common.parity_even),
        }
    }
}

/// UI enums for `DataBits` and `StopBits` so they can be used with `selector_spans`
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
pub enum DataBitsOption {
    Five,
    Six,
    Seven,
    Eight,
}

impl std::fmt::Display for DataBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Five => write!(f, "5"),
            Self::Six => write!(f, "6"),
            Self::Seven => write!(f, "7"),
            Self::Eight => write!(f, "8"),
        }
    }
}

// Custom conversion helpers removed. Use direct casts and `FromRepr::from_repr` as needed.

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
pub enum StopBitsOption {
    One,
    Two,
}

impl std::fmt::Display for StopBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One => write!(f, "1"),
            Self::Two => write!(f, "2"),
        }
    }
}

/// Baud rate presets including a Custom placeholder.
///
/// Custom does not carry the numeric value here; the actual runtime baud
/// is stored in the port configuration as a `u32`. This enum is used for
/// selector rendering.
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
            Self::Custom { .. } => write!(f, "{}", lang().protocol.common.custom),
                    other => write!(f, "{}", other.as_u32()),
        }
    }
}

impl BaudRateSelector {
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::B110 => 110u32,
            Self::B300 => 300u32,
            Self::B600 => 600u32,
            Self::B1200 => 1200u32,
            Self::B2400 => 2400u32,
            Self::B4800 => 4800u32,
            Self::B9600 => 9600u32,
            Self::B14400 => 14400u32,
            Self::B19200 => 19200u32,
            Self::B38400 => 38400u32,
            Self::B57600 => 57600u32,
            Self::B115200 => 115_200_u32,
            Self::B230400 => 230_400_u32,
            Self::B460800 => 460_800_u32,
            Self::B921600 => 921_600_u32,
            Self::B1000000 => 1_000_000_u32,
            Self::B2000000 => 2_000_000_u32,
            Self::Custom { baud } => baud,
        }
    }

    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            110 => Self::B110,
            300 => Self::B300,
            600 => Self::B600,
            1200 => Self::B1200,
            2400 => Self::B2400,
            4800 => Self::B4800,
            9600 => Self::B9600,
            14400 => Self::B14400,
            19200 => Self::B19200,
            38400 => Self::B38400,
            57600 => Self::B57600,
            115_200 => Self::B115200,
            230_400 => Self::B230400,
            460_800 => Self::B460800,
            921_600 => Self::B921600,
            1_000_000 => Self::B1000000,
            2_000_000 => Self::B2000000,
            _ => Self::Custom { baud: v },
        }
    }

    #[must_use]
    pub const fn to_index(self) -> usize {
        match self {
            Self::B110 => 0usize,
            Self::B300 => 1usize,
            Self::B600 => 2usize,
            Self::B1200 => 3usize,
            Self::B2400 => 4usize,
            Self::B4800 => 5usize,
            Self::B9600 => 6usize,
            Self::B14400 => 7usize,
            Self::B19200 => 8usize,
            Self::B38400 => 9usize,
            Self::B57600 => 10usize,
            Self::B115200 => 11usize,
            Self::B230400 => 12usize,
            Self::B460800 => 13usize,
            Self::B921600 => 14usize,
            Self::B1000000 => 15usize,
            Self::B2000000 => 16usize,
            Self::Custom { .. } => 17usize,
        }
    }

    #[must_use]
    pub const fn from_index(i: usize) -> Self {
        match i {
            0 => Self::B110,
            1 => Self::B300,
            2 => Self::B600,
            3 => Self::B1200,
            4 => Self::B2400,
            5 => Self::B4800,
            6 => Self::B9600,
            7 => Self::B14400,
            8 => Self::B19200,
            9 => Self::B38400,
            10 => Self::B57600,
            11 => Self::B115200,
            12 => Self::B230400,
            13 => Self::B460800,
            14 => Self::B921600,
            15 => Self::B1000000,
            16 => Self::B2000000,
            _ => Self::Custom { baud: 0 },
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
            Self::Preset(p) => write!(f, "{}", p.as_u32()),
            Self::Custom(v) => write!(f, "{v}"),
        }
    }
}

impl BaudRateOption {
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match BaudRateSelector::from_u32(v) {
            BaudRateSelector::Custom { .. } => Self::Custom(v),
                    s => Self::Preset(s),
        }
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::Preset(s) => s.as_u32(),
            Self::Custom(v) => v,
        }
    }
}

/// Register mode for `ModbusResponse`, supports standard types and custom mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseRegisterMode {
    /// 0x01 - Read Coils
    Coils,
    /// 0x02 - Read Discrete Inputs
    DiscreteInputs,
    /// 0x03 - Read Holding Registers
    Holding,
    /// 0x04 - Read Input Registers
    Input,
    /// Custom mode with user-defined function code
    Custom { function_code: u8 },
}

impl ResponseRegisterMode {
    /// Create from `RegisterMode`
    #[must_use]
    pub const fn from_register_mode(mode: RegisterMode) -> Self {
        match mode {
            RegisterMode::Coils => Self::Coils,
            RegisterMode::DiscreteInputs => Self::DiscreteInputs,
            RegisterMode::Holding => Self::Holding,
            RegisterMode::Input => Self::Input,
        }
    }

    /// Create from function code (0x01-0x04 standard, others custom)
    #[must_use]
    pub const fn from_function_code(code: u8) -> Self {
        match code {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Custom {
                function_code: code,
            },
        }
    }

    /// Get function code
    #[must_use]
    pub const fn function_code(self) -> u8 {
        match self {
            Self::Coils => 1,
            Self::DiscreteInputs => 2,
            Self::Holding => 3,
            Self::Input => 4,
            Self::Custom { function_code } => function_code,
        }
    }

    /// Check if this is a standard mode
    #[must_use]
    pub const fn is_standard(self) -> bool {
        !matches!(self, Self::Custom { .. })
    }

    /// Check if this is a custom mode
    #[must_use]
    pub const fn is_custom(self) -> bool {
        matches!(self, Self::Custom { .. })
    }

    /// Try to convert to `RegisterMode` (only works for standard types)
    #[must_use]
    pub const fn to_register_mode(self) -> Option<RegisterMode> {
        match self {
            Self::Coils => Some(RegisterMode::Coils),
            Self::DiscreteInputs => Some(RegisterMode::DiscreteInputs),
            Self::Holding => Some(RegisterMode::Holding),
            Self::Input => Some(RegisterMode::Input),
            Self::Custom { .. } => None,
        }
    }
}

impl fmt::Display for ResponseRegisterMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coils => write!(f, "Coils"),
            Self::DiscreteInputs => write!(f, "DiscreteInputs"),
            Self::Holding => write!(f, "Holding"),
            Self::Input => write!(f, "Input"),
            Self::Custom { function_code } => write!(f, "Custom(0x{function_code:02X})"),
        }
    }
}

impl From<RegisterMode> for ResponseRegisterMode {
    fn from(mode: RegisterMode) -> Self {
        Self::from_register_mode(mode)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusResponse {
    pub station_id: u8,
    pub register_address: u16,
    pub register_mode: ResponseRegisterMode,
    pub values: Vec<u16>,
    pub timestamp: String,
}
