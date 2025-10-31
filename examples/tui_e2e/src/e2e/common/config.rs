use aoba_ci_utils::ArrowKey;

/// Re-export the canonical Modbus configuration primitives used by the binary so
/// tests operate on the exact same data structures.
pub use aoba::protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode};

/// Construct a single-range station configuration shared across the test suite.
///
/// The runtime supports arbitrarily complex `StationConfig` values with multiple
/// register blocks per station. The majority of TUI integration tests exercise
/// the common single-range case, so this helper keeps call-sites concise while
/// still emitting the canonical protocol structure.
pub fn make_station_config(
    station_id: u8,
    register_mode: RegisterMode,
    start_address: u16,
    register_count: u16,
    is_master: bool,
    register_values: Option<Vec<u16>>,
) -> StationConfig {
    StationConfig::single_range(
        station_id,
        if is_master {
            StationMode::Master
        } else {
            StationMode::Slave
        },
        register_mode,
        start_address,
        register_count,
        register_values,
    )
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
///
/// Extension helpers for the shared RegisterMode enum that are only needed by
/// the test harness (e.g. translating into CLI strings or navigation hints).
pub trait RegisterModeExt {
    fn as_cli_mode(&self) -> &'static str;
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

// TODO: The legacy harness used bespoke accessors; callers should migrate to the
// inherent `StationConfig` helpers (`station_id()`, `register_values_owned()`, etc.).
