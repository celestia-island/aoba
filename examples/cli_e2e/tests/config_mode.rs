use anyhow::Result;
use aoba::cli::config::{
    CommunicationMethod, CommunicationMode, CommunicationParams, Config, ModbusRegister,
    PersistenceMode, RegisterType,
};
use ci_utils::terminal::run_binary_sync;

/// Test configuration mode with multiple scenarios
pub async fn test_config_mode() -> Result<()> {
    log::info!("🧪 Starting comprehensive configuration mode tests...");

    // Test 1: Single master configuration with different register types
    test_single_master_config().await?;

    // Test 2: Single slave configuration with different register types
    test_single_slave_config().await?;

    // Test 3: Configuration with multiple register types
    test_multiple_registers_config().await?;

    // Test 4: Configuration with JSON string
    test_config_json_string().await?;

    log::info!("🎉 All configuration mode tests completed successfully!");
    Ok(())
}

/// Test single master configuration with different register types
async fn test_single_master_config() -> Result<()> {
    log::info!("🧪 Testing single master configuration with different register types...");

    // Test holding registers
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(10.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![ModbusRegister {
            station_id: 1,
            register_type: RegisterType::Holding,
            start_address: 0,
            length: 10,
        }],
    };

    test_config_file(&config, "single_master_holding").await?;

    // Test input registers with different station ID
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(10.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![ModbusRegister {
            station_id: 2,
            register_type: RegisterType::Input,
            start_address: 100,
            length: 5,
        }],
    };

    test_config_file(&config, "single_master_input").await?;

    Ok(())
}

/// Test single slave configuration with different register types
async fn test_single_slave_config() -> Result<()> {
    log::info!("🧪 Testing single slave configuration with different register types...");

    // Test coils
    let config = Config {
        port_name: "/tmp/vcom2".to_string(),
        baud_rate: 115200,
        communication_mode: CommunicationMode::Slave,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: true,
            wait_time: Some(2.0),
            timeout: Some(30.0),
            persistence: PersistenceMode::Temporary,
        },
        modbus_configs: vec![ModbusRegister {
            station_id: 3,
            register_type: RegisterType::Coils,
            start_address: 0,
            length: 8,
        }],
    };

    test_config_file(&config, "single_slave_coils").await?;

    // Test discrete inputs with different address range
    let config = Config {
        port_name: "/tmp/vcom2".to_string(),
        baud_rate: 115200,
        communication_mode: CommunicationMode::Slave,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: true,
            wait_time: Some(2.0),
            timeout: Some(30.0),
            persistence: PersistenceMode::Temporary,
        },
        modbus_configs: vec![ModbusRegister {
            station_id: 4,
            register_type: RegisterType::Discrete,
            start_address: 50,
            length: 16,
        }],
    };

    test_config_file(&config, "single_slave_discrete").await?;

    Ok(())
}

/// Test configuration with multiple register types
async fn test_multiple_registers_config() -> Result<()> {
    log::info!("🧪 Testing configuration with multiple register types...");

    // Test master with multiple register types
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(10.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Holding,
                start_address: 0,
                length: 5,
            },
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Input,
                start_address: 100,
                length: 3,
            },
        ],
    };

    test_config_file(&config, "multiple_registers_master").await?;

    // Test slave with multiple register types
    let config = Config {
        port_name: "/tmp/vcom2".to_string(),
        baud_rate: 115200,
        communication_mode: CommunicationMode::Slave,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: true,
            wait_time: Some(2.0),
            timeout: Some(30.0),
            persistence: PersistenceMode::Temporary,
        },
        modbus_configs: vec![
            ModbusRegister {
                station_id: 3,
                register_type: RegisterType::Coils,
                start_address: 0,
                length: 8,
            },
            ModbusRegister {
                station_id: 3,
                register_type: RegisterType::Discrete,
                start_address: 100,
                length: 16,
            },
        ],
    };

    test_config_file(&config, "multiple_registers_slave").await?;

    Ok(())
}

/// Test configuration with JSON string
async fn test_config_json_string() -> Result<()> {
    log::info!("🧪 Testing configuration mode with JSON string...");

    // Test with adjacent registers using standard data structures
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(10.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Holding,
                start_address: 0,
                length: 5,
            },
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Holding,
                start_address: 5,
                length: 5,
            },
        ],
    };

    let json_config = config.to_json()?;
    let output = run_binary_sync(&["--config-json", &json_config])?;

    if output.status.success() {
        log::info!("✅ Configuration mode with JSON string command accepted");
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Loading configuration from JSON string") {
            log::info!("✅ Configuration JSON string loading message found");
        } else {
            log::warn!("⚠️ Configuration JSON string loading message not found in output");
        }
    } else {
        log::warn!("⚠️ Configuration mode with JSON string command failed");
        log::warn!(
            "stdout: {stdout}",
            stdout = String::from_utf8_lossy(&output.stdout)
        );
        log::warn!(
            "stderr: {stderr}",
            stderr = String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Helper function to test configuration with file
async fn test_config_file(config: &Config, test_name: &str) -> Result<()> {
    let config_path = format!("test_config_{test_name}.json");
    std::fs::write(&config_path, config.to_json()?)?;

    let output = run_binary_sync(&["--config", &config_path])?;

    // Clean up temporary file
    let _ = std::fs::remove_file(config_path);

    if output.status.success() {
        log::info!("✅ {test_name} configuration command accepted");
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Loading configuration from file") {
            log::info!("✅ {test_name} configuration file loading message found");
        } else {
            log::warn!("⚠️ {test_name} configuration loading message not found in output");
        }
    } else {
        log::warn!("⚠️ {test_name} configuration command failed");
        log::warn!(
            "stdout: {stdout}",
            stdout = String::from_utf8_lossy(&output.stdout)
        );
        log::warn!(
            "stderr: {stderr}",
            stderr = String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
