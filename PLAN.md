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

---

## 待补充的接口

### 1️⃣ 串口异步读写包装（tokio stream）

**现状**：`open_serial_port()` 返回 `Box<dyn serialport::SerialPort>`（同步）。
**需求**：提供或导出 async read/write 能力，使 evernight `serial` 模块可以在 tokio 运行时中非阻塞读写。

**选项 A**：aoba 直接包装 tokio-serial，提供 `tokio::io::AsyncRead + AsyncWrite` 类型：

```rust
// 期望的 API 示意
pub fn open_serial_port_async(
    port: &str,
    baud_rate: u32,
    timeout: Duration,
) -> Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite>;
```

**选项 B**：aoba 导出 `serialport::SerialPort` trait，由 evernight 自行包装。

**推荐**：**选项 A**，因为 aoba 已经依赖 serialport，在它之上封装 async 对下游更友好。

### 2️⃣ 端口占用检测

**现状**：`check_ports_occupied()` 通过 spawn 自身 CLI 子进程实现，不适合库调用。
**需求**：提供纯库函数的端口占用检测，不依赖 CLI 子进程。

```rust
// 期望的 API 示意
pub fn is_port_occupied(port: &str) -> Result<bool>;
```

### 3️⃣ Modbus RTU 波特率扫描（用于协议自动探测）

**现状**：aoba 已有 Modbus RTU master（`cli/modbus/master.rs`），但嵌入在 CLI 中。
**需求**：暴露为库函数，供 evernight `protocol::probe` 模块在串口上自动探测 Modbus RTU 设备。

```rust
// 期望的 API 示意
pub fn probe_modbus_rtu_baud(
    port: &str,
    baud_rates: &[u32],
    timeout: Duration,
) -> Result<Option<u32>>;
```

### 4️⃣ 虚拟端口检测能力

**现状**：`api::utils::open_serial_port()` 内部使用 `PortType::detect(port)` 判断虚拟端口并拒绝打开。
**需求**：将 `PortType::detect()` 导出为公开函数，使 evernight 可以在打开前预先判断。

```rust
// 期望的 API 示意
pub fn is_virtual_port(port: &str) -> bool;
```

---

## 实现优先级

| 优先级 | 需求 | 对应 evernight 阶段 |
|--------|------|---------------------|
| P0 | 异步读写包装 | 2D.3 |
| P1 | 端口占用检测 | 2D.1 |
| P2 | Modbus RTU 扫描 | 2D.5 |
| P3 | 虚拟端口检测导出 | 2D.1 |
