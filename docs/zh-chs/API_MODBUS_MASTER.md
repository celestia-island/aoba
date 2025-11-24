# Modbus 主站 API 使用指南

本文档介绍如何在 Rust 程序中通过 Aoba 提供的 Modbus 主站（Master） API 采集和监控工业现场设备数据，例如生产线设备状态监控、环境监测、过程控制等场景。

示例代码参考 `examples/api_master` crate。

## 1. 总体概览

Aoba 暴露了一套基于 trait 的 Modbus 主站 API，适合嵌入到你的工控 / 监控应用中，用于：

- 周期性轮询 Modbus 从设备（串口 / 虚拟串口）
- 采集线圈 / 寄存器数据并转换为工程量（压力、温度、阀门状态等）
- 通过 Hook 机制统一做日志、告警、统计等

核心入口类型是 `_main::api::modbus::ModbusBuilder`：

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> 说明：示例工程中 crate 根叫 `_main`，在你自己的项目中通常是 `aoba` 或在 `Cargo.toml` 里配置的别名。

---

## 2. 主站基本生命周期

一个最小可用的主站轮询程序大致如下：

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    // 创建主站实例
    let master = ModbusBuilder::new_master(1) // 从站地址（站号）
        .with_port("/dev/ttyUSB0")          // 或 `/tmp/vcom1` 等
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // 超时：毫秒
        .build_master()?;

    // 简单轮询读取
    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### 关键参数说明

- **串口名称**：
  - 真实串口：如 `/dev/ttyUSB0`、`/dev/ttyS1` 等；
  - 虚拟串口：如 `/tmp/vcom1`，可用 `socat` 或系统工具创建，用于联调 / 仿真。
- **站点 ID（Station ID）**：Modbus 从站地址，通常在 1–247 范围内，需要与现场设备配置一致。
- **寄存器模式（RegisterMode）**：
  - `Coils`（01 线圈）
  - `DiscreteInputs`（02 离散量输入，可写模式在部分场景会特殊处理）
  - `Holding`（03 保持寄存器）
  - `Input`（04 输入寄存器，可写模式同上）
- **寄存器地址 / 长度**：起始地址和读取长度，对应现场协议文档（例如某生产线 PLC 的 Modbus 地址表）。
- **超时时间**：单次请求的超时（毫秒），建议根据总线长度和波特率适当放宽。

主站内部维护请求发送逻辑并将响应推入内部通道，你的代码只需要定期调用 `recv_timeout` 即可获得最新数据。

---

## 3. 使用 Hook 做日志和监控

在大多数工业监控或过程控制场景中，通常需要：

- 记录每次成功的 Modbus 响应（便于追溯）
- 统计错误和超时次数，触发告警
- 将数据推送到外部系统（MQTT、HTTP、数据库等）

`ModbusHook` trait 提供了统一的插桩入口：

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("sending request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, resp: &ModbusResponse) -> Result<()> {
        log::info!(
            "resp {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            resp.station_id,
            resp.register_address,
            resp.values,
        );
        Ok(())
    }

    fn on_error(&self, port: &str, err: &anyhow::Error) {
        log::warn!("modbus error on {}: {}", port, err);
    }
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // 后续使用 recv_timeout 轮询即可
    # let _ = master;
    Ok(())
}
```

你可以注册多个 Hook，例如：

- 一个负责写日志；
- 一个负责将数据转成 JSON 并推送到 MQTT；
- 一个负责统计错误率并触发告警。

---

## 4. 面向工业现场 / 过程设备的推荐集成模式

结合一般工业现场（如产线设备、过程装置、环境监测装置等）的实践经验，一个比较稳妥的集成模式是：

1. **确定通信映射**：
   - 明确每台设备对应的串口（或虚拟串口）；
   - 明确每台设备的站点 ID 和寄存器地址表（参考设备提供的 Modbus 通信协议文档）。
2. **每个物理/虚拟串口创建一个主站实例**：
   - 使用 `ModbusBuilder::new_master(station_id)`；
   - 或在外层维护一个 `(port, station_id, register_region)` 的表，循环创建多个主站。
3. **在 Tokio 运行时中为每个主站起一个任务**：
   - 在任务中使用 `recv_timeout` 轮询数据；
   - 将 `ModbusResponse::values` 按协议文档换算为工程量，写入内存共享状态或通道；
   - 再由其他任务负责数据汇聚（MQTT/HTTP 上报、历史记录等）。
4. **通过 Hook 统一监控健康度**：
   - 在 `on_error` 中维护错误计数；
   - 如果连续错误超过阈值（如 10 次），更新全局“设备离线”状态；
   - 可在 `on_after_response` 中记录响应延时，做性能监控。

这种结构下，Modbus 主站层只关心通信本身，而业务层则只面对清洗后的工程量数据，职责分离清晰，便于后续扩展（增加数据源、切换上报通道等）。

---

## 5. 错误与超时处理建议

- `build_master()` 失败通常表示：
  - 串口不存在或权限不足；
  - 配置参数非法（例如寄存器长度为 0）。
- `recv_timeout()` 返回 `None` 表示在指定时间内没有拿到响应，**不一定是致命错误**，在现场环境（线缆较长、电磁干扰、大量设备共享总线）下偶发超时是常态。
- 协议级错误（CRC 错误、异常码、IO 错误等）会通过 `ModbusHook::on_error` 回调暴露出来。

实践建议：

- **区分偶发错误与持续错误**：
  - 使用 Hook 里的计数器统计连续失败次数；
  - 连续失败超过 N 次（比如 5～10）再标记“设备疑似离线”。
- **注意节流日志**：
  - 在高频轮询（< 1s 周期）下，避免对每次请求都打印大量日志，建议使用 Info 打点 + Debug 级别详细日志的方式。

---

## 6. 运行示例程序

在仓库根目录执行：

```bash
cargo run --package api_master -- /tmp/vcom1
```

其中 `/tmp/vcom1` 可以是：

- 真实的串口设备（在 Linux/WSL 下需要用实际设备名替换）；
- 或通过 `socat` 创建的虚拟串口对，用于与 `examples/modbus_slave` 或现场设备仿真程序联动。

在典型的工业联调过程中，常见步骤是：

1. 先用 Aoba CLI/TUI 或 `examples/modbus_slave` 搭一个从站仿真；
2. 再运行 `api_master`，确认 Modbus 配置和协议映射都正确；
3. 在此基础上，将示例代码中的采集逻辑改造成自己的业务服务（写数据库、推 MQTT 等）。

---

## 7. 延伸阅读

- 从站侧 API 使用示例：`examples/api_slave`；
- CLI 级别的 Modbus 使用说明：`docs/zh-chs/CLI_MODBUS.md`；
- HTTP / MQTT / IPC 等数据源对接文档：同目录下 `DATA_SOURCE_*.md`；
- 若要结合 TUI 做 E2E 测试，请参考 `examples/tui_e2e` 及根目录下 `CLAUDE.md` 中关于 IPC 与状态监控的介绍。
