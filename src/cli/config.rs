use serde::{Deserialize, Serialize};
use std::fmt;

/// 通信模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMode {
    /// Modbus 主站模式
    Master,
    /// Modbus 从站模式
    Slave,
}

/// 通信方式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMethod {
    /// IPC 通信
    Ipc,
    /// 标准输入输出通信
    Stdio,
    /// 文件通信
    File,
}

/// 持久化模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceMode {
    /// 持久化运行
    Persistent,
    /// 临时运行
    Temporary,
    /// 一次性运行
    OneShot,
}

/// 寄存器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// 保持寄存器
    Holding,
    /// 输入寄存器
    Input,
    /// 线圈寄存器
    Coils,
    /// 离散输入寄存器
    Discrete,
}

impl fmt::Display for RegisterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterType::Holding => write!(f, "holding"),
            RegisterType::Input => write!(f, "input"),
            RegisterType::Coils => write!(f, "coils"),
            RegisterType::Discrete => write!(f, "discrete"),
        }
    }
}

/// 通信线程参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationParams {
    /// 通信方式
    pub mode: CommunicationMethod,
    /// 从外部动态拉取数据
    pub dynamic_pull: bool,
    /// 等待时间（秒）
    pub wait_time: Option<f64>,
    /// 超时时间（秒）
    pub timeout: Option<f64>,
    /// 持久化模式
    pub persistence: PersistenceMode,
}

/// Modbus 寄存器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusRegister {
    /// 站点 ID
    pub station_id: u8,
    /// 寄存器类型
    pub register_type: RegisterType,
    /// 起始地址
    pub start_address: u16,
    /// 寄存器数量
    pub length: u16,
}

/// 根配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 端口名称
    pub port_name: String,
    /// 端口通信配置（波特率）
    pub baud_rate: u32,
    /// 端口通信模式
    pub communication_mode: CommunicationMode,
    /// 通信线程参数
    pub communication_params: CommunicationParams,
    /// Modbus 配置列表
    pub modbus_configs: Vec<ModbusRegister>,
}

impl Default for CommunicationParams {
    fn default() -> Self {
        Self {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            persistence: PersistenceMode::Persistent,
        }
    }
}

impl Default for ModbusRegister {
    fn default() -> Self {
        Self {
            station_id: 1,
            register_type: RegisterType::Holding,
            start_address: 0,
            length: 10,
        }
    }
}

impl Config {
    /// 从 JSON 字符串解析配置
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// 从文件读取配置
    pub fn from_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(file_path)?;
        Self::from_json(&content).map_err(|e| e.into())
    }

    /// 转换为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = Config {
            port_name: "COM1".to_string(),
            baud_rate: 9600,
            communication_mode: CommunicationMode::Master,
            communication_params: CommunicationParams::default(),
            modbus_configs: vec![
                ModbusRegister {
                    station_id: 1,
                    register_type: RegisterType::Holding,
                    start_address: 0,
                    length: 10,
                },
                ModbusRegister {
                    station_id: 2,
                    register_type: RegisterType::Input,
                    start_address: 100,
                    length: 5,
                },
            ],
        };

        let json = config.to_json().unwrap();
        println!("{json}");

        let parsed_config = Config::from_json(&json).unwrap();
        assert_eq!(parsed_config.port_name, "COM1");
        assert_eq!(parsed_config.modbus_configs.len(), 2);
        assert_eq!(parsed_config.modbus_configs[0].station_id, 1);
        assert_eq!(parsed_config.modbus_configs[1].station_id, 2);
    }
}
