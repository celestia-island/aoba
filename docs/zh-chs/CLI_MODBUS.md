# CLI Modbus 功能

本文档描述了 aoba 项目新增的 Modbus 操作 CLI 功能。

## 功能

### 1. 增强的端口列表

`--list-ports` 命令现在可以与 `--json` 一起使用，提供更详细的端口信息：

```bash
aoba --list-ports --json
```

输出包括：

- `path`: 端口路径（例如 COM1, /dev/ttyUSB0）
- `status`: "Free"（空闲）或 "Occupied"（占用）
- `guid`: Windows 设备 GUID（如果可用）
- `vid`: USB 厂商 ID（如果可用）
- `pid`: USB 产品 ID（如果可用）
- `serial`: 序列号（如果可用）

示例输出：

```json
[
  {
    "path": "COM1",
    "status": "Free",
    "guid": "{...}",
    "vid": 1234,
    "pid": 5678
  }
]
```

### 2. 从站监听模式

#### 临时模式

监听一个 Modbus 请求，响应后退出：

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

输出单个 JSON 响应后退出。

#### 常驻模式

持续监听请求并输出 JSONL：

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

每处理一个请求输出一行 JSON。

### 3. 主站提供模式

#### 临时模式

提供一次数据后退出：

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

从数据源读取一行，发送数据后退出。

#### 常驻模式

持续提供数据：

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

依次从数据源读取行并发送，每次操作输出一行 JSON。

### 数据源格式

对于主站模式，数据源文件应包含 JSONL 格式：

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

每行代表一次要发送给从站的更新。

## 参数

| 参数 | 说明 | 默认值 |
|-----------|-------------|---------|
| `--station-id` | Modbus 站点 ID（从站地址） | 1 |
| `--register-address` | 起始寄存器地址 | 0 |
| `--register-length` | 寄存器数量 | 10 |
| `--register-mode` | 寄存器类型：holding、input、coils、discrete | holding |
| `--data-source` | 数据源：`file:<path>` 或 `pipe:<name>` | - |
| `--baud-rate` | 串口波特率 | 9600 |

## 寄存器模式

- `holding`: 保持寄存器（可读写）
- `input`: 输入寄存器（只读）
- `coils`: 线圈（可读写位）
- `discrete`: 离散输入（只读位）

## 集成测试

集成测试位于 `examples/cli_e2e_tests/`。运行测试：

```bash
cd examples/cli_e2e_tests
cargo run
```

测试验证：

- 带状态的增强端口列表
- 从站临时监听模式
- 从站常驻监听模式
- 主站临时提供模式
- 主站常驻提供模式

## 未来增强

- 数据源的命名管道支持（当前已预留接口）
- 虚拟串口的实时 Modbus 通信测试
- 额外的寄存器模式支持
