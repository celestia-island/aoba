# Modbus 从站 API 使用指南

本文档介绍如何在 Rust 程序中通过 Aoba 提供的 Modbus 从站（Slave） API 暴露现场设备或仿真设备的数据，例如在工业产线、过程控制或测试平台中作为被采集端使用。

示例代码参考 `examples/api_slave` crate。

## 1. 总体概览

Aoba 提供了与主站 API 风格一致的从站 API，通过 Builder + Hook 的形式，便于：

- 将你的进程包装成一个 Modbus 从站，对外暴露寄存器/线圈数据；
- 在测试/仿真环境中快速搭建一个可配置的 Modbus 设备；
- 通过 Hook 中间件链路统一处理日志、统计、告警等逻辑。

核心入口类型同样是 `_main::api::modbus::ModbusBuilder`，但使用 `new_slave` / `build_slave`：

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. 从站基本生命周期

示例中的从站程序大致结构如下：

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct ResponseLoggingHook;

impl ModbusHook for ResponseLoggingHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "sent response on {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, error: &anyhow::Error) {
        log::warn!("error: {}", error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "/tmp/vcom2" };

    let hook: Arc<dyn ModbusHook> = Arc::new(ResponseLoggingHook);

    let _slave = ModbusBuilder::new_slave(1)
        .with_port(port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(hook)
        .build_slave()?;

    // 从站启动后会持续监听主站请求
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### 核心配置项

- **串口名称**：与主站一致，例如 `/dev/ttyUSB0`、`/dev/ttyS1`、`/tmp/vcom2` 等；
- **站点 ID（Station ID）**：需要与主站访问的目标站点一致；
- **寄存器模式与地址区间**：决定从站对外暴露的寄存器/线圈范围；
- **超时时间**：从站内部使用，用于控制内部 IO/处理过程的超时（通常保持与主站设置一致即可）。

---

## 3. Hook 中间件链路

从站端同样可以注册多个 Hook 形成中间件链路，用于：

- 在请求到达前检查状态（例如是否允许当前时段访问）；
- 在响应发送后记录日志、做统计；
- 在出现错误时统一告警。

`examples/api_slave` 中的示例展示了 3 个 Hook 串联的模式：

- `RequestMonitorHook`：监控请求并在出错时告警；
- `ResponseLoggingHook`：详细记录每次响应的寄存器地址和值；
- `StatisticsHook`：统计请求次数等指标。

这种“中间件链路”的好处是，可以把与业务无关的通用逻辑（日志、统计、限流、权限校验等）抽离出来，按需组合挂载在从站实例上。

---

## 4. 典型使用场景

在工业现场或测试平台中，从站 API 典型用法包括：

1. **软从站仿真设备**：
   - 在没有真实设备时，用 Rust 程序模拟一个 Modbus 设备；
   - 根据测试需要定期更新内部寄存器值，供上位主站轮询；
   - 便于在 CI 中做自动化联调测试。
2. **为其他协议/总线提供 Modbus 适配层**：
   - 例如底层是 CAN、以太网或自定义协议，上层系统要求通过 Modbus 访问；
   - 可以用从站 API 将这些数据映射到 Modbus 寄存器空间，对外暴露统一接口。
3. **生产线/装置的边缘网关**：
   - 将现场设备数据先接入你的网关进程；
   - 再通过 Modbus 从站 API 暴露给历史系统或第三方上位机。

---

## 5. 与主站 API 的配合

主站与从站 API 在 Builder 和 Hook 设计上保持一致，方便你在同一进程中：

- 同时作为若干上游设备的主站；
- 又作为其他系统的从站，对外统一暴露整理后的数据。

典型结构：

1. 使用主站 API 轮询多个现场设备，汇聚并清洗为统一数据模型；
2. 在同一进程内使用从站 API，将这些数据映射到一块连续的 Modbus 寄存器空间；
3. 其他系统只需要按这块“汇总寄存器”进行读取即可。

---

## 6. 运行从站示例

在仓库根目录执行：

```bash
cargo run --package api_slave -- /tmp/vcom2
```

可以配合主站示例或 Aoba CLI/TUI 进行联调：

- 先启动从站示例监听 `/tmp/vcom2`；
- 再使用主站示例或 CLI/TUI 轮询该端口，验证寄存器读写是否符合预期。

---

## 7. 延伸阅读

- 主站侧 API 使用示例：`docs/zh-chs/API_MODBUS_MASTER.md`；
- CLI 级别 Modbus 使用说明：`docs/zh-chs/CLI_MODBUS.md`；
- 数据源/数据出口能力（HTTP、MQTT、IPC 等）：同目录下 `DATA_SOURCE_*.md` 文档；
- 更多综合示例可参考 `examples` 目录中的其他子项目。
