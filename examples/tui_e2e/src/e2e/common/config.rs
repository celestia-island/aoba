use ci_utils::ArrowKey;

/// Re-export the canonical RegisterMode used by the aoba binary so tests
/// operate on the same enumeration as the runtime.
pub use _bin::protocol::status::types::modbus::RegisterMode;

/// Station configuration for TUI tests.
///
/// This structure encapsulates all parameters needed to configure a Modbus station
/// in the TUI environment, supporting both Master and Slave roles with various
/// register types.
///
/// # Fields
///
/// - **`station_id`**: Unique station identifier (1-247 for Modbus)
///   - Used to identify the station in the TUI and CLI
///   - Master stations typically use ID 1
///   - Slave stations use IDs 2-247
///
/// - **`register_mode`**: Type of registers to configure (Coils, DiscreteInputs, Holding, Input)
///   - Determines read/write operations and data type (bit vs 16-bit word)
///   - See [`RegisterMode`] for detailed mode descriptions
///
/// - **`start_address`**: Starting address for register block (0-65535)
///   - Modbus address space varies by register type
///   - Common ranges: 0-9999 for Coils, 30000-39999 for Inputs, etc.
///
/// - **`register_count`**: Number of registers to allocate (1-2000)
///   - Limited by Modbus protocol (max 2000 coils, 125 registers per read)
///   - Affects memory usage and read/write performance
///
/// - **`is_master`**: Whether this station acts as a Master (true) or Slave (false)
///   - Master stations initiate requests
///   - Slave stations respond to requests
///   - Role determines available operations in TUI
///
/// - **`register_values`**: Optional initial register values for Slave stations
///   - `Some(vec![...])`: Pre-populate registers with specific values
///   - `None`: Use default values (0 for all registers)
///   - Only applicable for Slave stations with writable register types
///
/// # Example 1: Master Station with Coils
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::*;
/// let master_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 100,
///     is_master: true,
///     register_values: None, // Master doesn't need initial values
/// };
/// ```
///
/// # Example 2: Slave Station with Pre-populated Holdings
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::*;
/// let slave_config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 10,
///     is_master: false,
///     register_values: Some(vec![1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000]),
/// };
/// ```
///
/// # Example 3: Input Registers (Read-Only from Master Perspective)
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::*;
/// let input_config = StationConfig {
///     station_id: 3,
///     register_mode: RegisterMode::Input,
///     start_address: 30000,
///     register_count: 50,
///     is_master: false,
///     register_values: Some(vec![100; 50]), // All registers initialized to 100
/// };
/// ```
///
/// # Usage with Configuration Functions
///
/// This structure is typically used with:
/// - [`configure_tui_station`]: Apply configuration in TUI environment
/// - [`setup_tui_test`]: Initialize test environment with station
/// - [`navigate_to_modbus_panel`]: Navigate to station configuration page
///
/// # See Also
///
/// - [`RegisterMode`]: Enum defining the four Modbus register types
/// - [`configure_tui_station`]: Function to apply this configuration in TUI
#[derive(Debug, Clone)]
pub struct StationConfig {
    pub station_id: u8,
    pub register_mode: RegisterMode,
    pub start_address: u16,
    pub register_count: u16,
    pub is_master: bool,
    pub register_values: Option<Vec<u16>>,
}

/// Register mode enumeration for Modbus operations.
///
/// Modbus defines four distinct register types, each with different addressing,
/// access patterns, and data representations. This enum provides type-safe
/// selection of register modes for TUI configuration and CLI commands.
///
/// # Register Types
///
/// | Mode              | Modbus Code | Data Type | Access      | Address Range (Standard) |
/// |-------------------|-------------|-----------|-------------|--------------------------|
/// | `Coils`           | 01          | Bit       | Read/Write  | 0-9999                   |
/// | `DiscreteInputs`  | 02          | Bit       | Read-Only*  | 10000-19999              |
/// | `Holding`         | 03          | 16-bit    | Read/Write  | 40000-49999              |
/// | `Input`           | 04          | 16-bit    | Read-Only*  | 30000-39999              |
///
/// *Note: In this TUI implementation, `DiscreteInputs` and `Input` are writable
/// on the Slave side for testing purposes, but appear read-only to Masters.
///
/// # Variants
///
/// ## `Coils`
/// - **Modbus Function**: 01 (Read Coils), 05 (Write Single Coil), 15 (Write Multiple Coils)
/// - **Data Type**: Single bit (0 or 1)
/// - **Use Case**: Digital outputs, relay control, on/off states
/// - **CLI Mode String**: `"coils"`
/// - **TUI Display**: Shows as checkboxes or binary values
///
/// ## `DiscreteInputs`
/// - **Modbus Function**: 02 (Read Discrete Inputs)
/// - **Data Type**: Single bit (0 or 1)
/// - **Use Case**: Digital inputs, sensor states, read-only flags
/// - **CLI Mode String**: `"discrete_inputs"`
/// - **TUI Display**: Shows as read-only checkboxes (Slave can modify for testing)
///
/// ## `Holding`
/// - **Modbus Function**: 03 (Read Holding Registers), 06 (Write Single Register), 16 (Write Multiple Registers)
/// - **Data Type**: 16-bit unsigned integer (0-65535)
/// - **Use Case**: Configuration values, setpoints, general read/write data
/// - **CLI Mode String**: `"holding"`
/// - **TUI Display**: Shows as numeric fields with hex/decimal format
///
/// ## `Input`
/// - **Modbus Function**: 04 (Read Input Registers)
/// - **Data Type**: 16-bit unsigned integer (0-65535)
/// - **Use Case**: Sensor readings, measurement data, read-only values
/// - **CLI Mode String**: `"input"`
/// - **TUI Display**: Shows as read-only numeric fields (Slave can modify for testing)
///
/// # Example 1: CLI Mode Strings
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::RegisterMode;
/// let mode = RegisterMode::Holding;
/// assert_eq!(mode.as_cli_mode(), "holding");
///
/// let mode = RegisterMode::Coils;
/// assert_eq!(mode.as_cli_mode(), "coils");
/// ```
///
/// # Example 2: Pattern Matching for Operation Logic
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::RegisterMode;
/// fn get_data_size(mode: RegisterMode, count: u16) -> usize {
///     match mode {
///         RegisterMode::Coils | RegisterMode::DiscreteInputs => {
///             (count as usize + 7) / 8 // Bits packed into bytes
///         }
///         RegisterMode::Holding | RegisterMode::Input => {
///             count as usize * 2 // 16-bit words = 2 bytes each
///         }
///     }
/// }
/// ```
///
/// # Example 3: Configuration with Different Register Types
///
/// ```rust,no_run
/// # use aoba::protocol::modbus::*;
/// // Coils: Binary sensors
/// let coil_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 16,
///     is_master: false,
///     register_values: None,
/// };
///
/// // Holdings: Numeric configuration
/// let holding_config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Holding,
///     start_address: 1000,
///     register_count: 10,
///     is_master: false,
///     register_values: Some(vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]),
/// };
/// ```
///
/// # See Also
///
/// - [`StationConfig`]: Uses this enum to specify register type
/// - [`as_cli_mode`]: Convert to CLI mode string for command-line operations
/// Extension helpers for the shared RegisterMode enum that are only needed by
/// the test harness (e.g. translating into CLI strings or navigation hints).
pub trait RegisterModeExt {
    fn as_cli_mode(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn status_value(&self) -> &'static str;
    fn arrow_from_default(&self) -> (ArrowKey, usize);
}

impl RegisterModeExt for RegisterMode {
    fn as_cli_mode(&self) -> &'static str {
        match self {
            RegisterMode::Coils => "coils",
            RegisterMode::DiscreteInputs => "discrete_inputs",
            RegisterMode::Holding => "holding",
            RegisterMode::Input => "input",
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            RegisterMode::Coils => "Coils",
            RegisterMode::DiscreteInputs => "Discrete Inputs",
            RegisterMode::Holding => "Holding",
            RegisterMode::Input => "Input",
        }
    }

    fn status_value(&self) -> &'static str {
        match self {
            RegisterMode::Coils => "Coils",
            RegisterMode::DiscreteInputs => "DiscreteInputs",
            RegisterMode::Holding => "Holding",
            RegisterMode::Input => "Input",
        }
    }

    fn arrow_from_default(&self) -> (ArrowKey, usize) {
        match self {
            RegisterMode::Coils => (ArrowKey::Left, 2),
            RegisterMode::DiscreteInputs => (ArrowKey::Left, 1),
            RegisterMode::Holding => (ArrowKey::Down, 0),
            RegisterMode::Input => (ArrowKey::Right, 1),
        }
    }
}
