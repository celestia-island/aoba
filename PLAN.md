# aoba 开发计划

## 下游依赖：evernight（长夜月）

`evernight` 是一个跨平台远程控制库，需要 aoba 提供串口通信能力。
以下是 evernight `serial` 模块（`/mnt/sdb1/evernight/src/serial/`）对 aoba 的依赖清单。

---

## 已满足的依赖

evernight 需要 aoba 提供以下接口，均已实现：

| 接口 | aoba 路径 | 用途 |
|------|-----------|------|
| `available_ports_sorted()` | `protocol::tty::available_ports_sorted` | 枚举串口列表 |
| `available_ports_enriched()` | `protocol::tty::available_ports_enriched` | 枚举串口（含 VID/PID/序列号等元信息） |
| `open_serial_port()` | `api::utils::open_serial_port` | 打开串口（带超时、互斥访问） |
| `VidPidSerial` | `protocol::tty::VidPidSerial` | 端口元信息类型 |
| `PortExtra` | `protocol::tty::PortExtra` | 额外端口属性 |
| `PortType` | `api::PortType` | 端口类型枚举（Physical / IPC / HTTP） |
| `is_virtual_port()` | `api::is_virtual_port` | 虚拟端口判断 |
| `is_port_occupied()` | `api::utils::is_port_occupied` | 端口占用检测（纯库函数，不依赖 CLI 子进程） |
| `AsyncSerialPort` | `api::async_serial::AsyncSerialPort`（feature `async-serial`） | 异步串口读写（AsyncRead + AsyncWrite） |
| `probe_modbus_rtu_baud()` | `api::modbus::probe::probe_modbus_rtu_baud` | Modbus RTU 波特率自动探测 |

---

## 实现详情

### 1️⃣ 串口异步读写包装（tokio stream）— 已实现 ✅

**实现方案**：选项 A — aoba 封装 `tokio-serial`。

- 新增 `async-serial` feature flag，可选依赖 `tokio-serial ^5`
- `api::async_serial::AsyncSerialPort` 实现 `tokio::io::AsyncRead + AsyncWrite`
- 使用 `unsafe` pin projection 委托给底层 `tokio_serial::SerialStream`
- 在 Unix 上保留 exclusive access 语义

```rust
// aoba 公开 API
#[cfg(feature = "async-serial")]
pub fn open_async(port: &str, baud_rate: u32, timeout: Duration) -> Result<AsyncSerialPort>;
```

**文件**：`src/api/async_serial.rs`、`Cargo.toml`（feature `async-serial`）

### 2️⃣ 端口占用检测 — 已实现 ✅

**实现方案**：从 `cli::actions::check_port_occupation()` 提取纯逻辑至 `api::utils::is_port_occupied()`。

- Windows：`CreateFileW` + `FILE_SHARE_NONE`，检查 `ERROR_SHARING_VIOLATION` / `ERROR_ACCESS_DENIED`
- Unix：解析 canonical 设备路径 → 获取 `rdev` → 遍历 `/proc/{pid}/fd` 匹配
- CLI 侧保留薄包装调用公共函数

```rust
pub fn is_port_occupied(port_name: &str) -> bool;
```

**文件**：`src/api/utils.rs`（新函数）、`src/cli/actions.rs`（改为调用公共 API）

### 3️⃣ Modbus RTU 波特率扫描 — 已实现 ✅

**实现方案**：新增 `api::modbus::probe` 模块。

- 遍历候选波特率列表，每个波特率打开串口、发送单次 read-holding 请求
- 复用 `execute_single_poll_internal()` 作为探针核心
- 第一个成功响应即返回对应波特率

```rust
pub fn probe_modbus_rtu_baud(
    port: &str,
    station_id: u8,
    baud_rates: &[u32],
    timeout: Duration,
) -> Result<Option<u32>>;

pub const DEFAULT_BAUD_RATES: &[u32] = &[2400, 4800, 9600, 19200, 38400, 57600, 115200];
```

**文件**：`src/api/modbus/probe.rs`

### 4️⃣ 虚拟端口检测能力 — 已实现 ✅

**实现方案**：导出 `PortType` 和 `is_virtual_port()` 至公共 API。

- `api::mod.rs` 中 `pub use PortType` 和 `pub fn is_virtual_port()`
- CLI 侧的 `is_virtual_port()` 调用不变（现在通过公共路径访问）

```rust
pub use crate::protocol::status::types::port::PortType;
pub fn is_virtual_port(port_name: &str) -> bool;
```

**文件**：`src/api/mod.rs`

---

## evernight 侧集成

evernight `src/serial/mod.rs` 已更新，新增以下公共接口：

| evernight API | 底层 aoba API |
|---------------|---------------|
| `serial::open_serial_port_async()` | `aoba::api::async_serial::AsyncSerialPort::open()` |
| `serial::is_port_occupied()` | `aoba::api::utils::is_port_occupied()` |
| `serial::is_virtual_port()` | `aoba::api::is_virtual_port()` |
| `serial::probe_modbus_rtu_baud()` | `aoba::api::modbus::probe::probe_modbus_rtu_baud()` |

evernight `Cargo.toml` 已启用 aoba 的 `async-serial` feature，并添加 `tokio-serial` 依赖。
