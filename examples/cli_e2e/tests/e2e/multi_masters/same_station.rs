use anyhow::Result;
use aoba::cli::config::{
    CommunicationMethod, CommunicationMode, CommunicationParams, Config, ModbusRegister,
    PersistenceMode, RegisterType,
};
use std::process::{Command, Stdio};
use std::time::Duration;

/// 测试同站点不同寄存器类型的多个主站配置
pub async fn test_multi_masters_same_station() -> Result<()> {
    log::info!("🧪 Testing multiple masters with same station ID but different register types...");

    // 使用类型安全的 Config struct 创建配置
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Holding,
                start_address: 0,
                length: 10,
            },
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Input,
                start_address: 100,
                length: 5,
            },
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Coils,
                start_address: 200,
                length: 8,
            },
        ],
    };

    // 将配置转换为 JSON 字符串
    let config_json = serde_json::to_string_pretty(&config)?;

    // 将配置写入临时文件
    let temp_dir = std::env::temp_dir();
    let config_file = temp_dir.join("test_multi_masters_same_station.json");
    std::fs::write(&config_file, config_json)?;

    log::info!("🧪 Created configuration file for same station test");

    // 构建二进制文件
    let binary = ci_utils::build_debug_bin("aoba")?;

    // 启动配置模式
    log::info!("🧪 Starting multi-masters with same station configuration...");
    let process = Command::new(&binary)
        .arg("--config")
        .arg(&config_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // 等待一段时间让进程启动
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 等待进程完成
    let output = process.wait_with_output()?;

    // 检查进程是否成功退出
    if output.status.success() {
        log::info!("✅ Multi-masters with same station configuration completed successfully");

        // 检查输出中是否包含配置加载成功的消息
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stdout.contains("Configuration loaded successfully")
            || stderr.contains("Configuration loaded successfully")
        {
            log::info!("✅ Configuration loading message found");
        } else {
            log::warn!("⚠️ Configuration loading message not found in output");
            log::debug!("stdout: {stdout}");
            log::debug!("stderr: {stderr}");
        }
    } else {
        log::warn!(
            "⚠️ Multi-masters with same station configuration failed with status: {}",
            output.status
        );
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::anyhow!(
            "Multi-masters with same station configuration failed"
        ));
    }

    // 清理临时文件
    std::fs::remove_file(&config_file)?;

    log::info!("✅ Multi-masters same station test completed successfully");
    Ok(())
}
