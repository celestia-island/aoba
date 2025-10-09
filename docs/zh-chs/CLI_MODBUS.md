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

- 临时模式，提供一次数据后退出：

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

- 常驻模式，持续提供数据：

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

#### 使用文件作为数据源

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### 使用 Unix 命名管道作为数据源

Unix 命名管道（FIFO）可用于实时数据流传输：

```bash
# 创建命名管道
mkfifo /tmp/modbus_input

# 在一个终端中启动主站
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# 在另一个终端中写入数据
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### 输出目标

对于从站模式，可以指定输出目标：

#### 输出到标准输出（默认）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### 输出到文件（追加模式）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### 输出到 Unix 命名管道

```bash
# 创建命名管道
mkfifo /tmp/modbus_output

# 在一个终端中启动从站
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# 在另一个终端中读取数据
cat /tmp/modbus_output
```

## 参数

| 参数 | 说明 | 默认值 |
|-----------|-------------|---------|
| `--station-id` | Modbus 站点 ID（从站地址） | 1 |
| `--register-address` | 起始寄存器地址 | 0 |
| `--register-length` | 寄存器数量 | 10 |
| `--register-mode` | 寄存器类型：holding、input、coils、discrete | holding |
| `--data-source` | 数据源：`file:<path>` 或 `pipe:<name>` | - |
| `--output` | 输出目标：`file:<path>` 或 `pipe:<name>`（默认：标准输出） | stdout |
| `--baud-rate` | 串口波特率 | 9600 |

## 寄存器模式

- `holding`: 保持寄存器（可读写）
- `input`: 输入寄存器（只读）
- `coils`: 线圈（可读写位）
- `discrete`: 离散输入（只读位）

## 集成测试

集成测试位于 `examples/cli_e2e/`。运行测试：

```bash
cd examples/cli_e2e
cargo run
```

### 循环模式运行测试

为了进行稳定性测试和调试，可以使用 `TEST_LOOP` 环境变量多次运行测试：

```bash
# 连续运行测试 5 次
TEST_LOOP=5 cargo run --example cli_e2e

# 运行测试 10 次以验证端口清理和稳定性
TEST_LOOP=10 cargo run --example cli_e2e
```

这对于以下场景很有用：
- 验证测试运行之间的端口清理
- 测试稳定性和可重复性
- 调试间歇性问题
- 确保 socat 虚拟端口重置正常工作

测试验证：

- 带状态的增强端口列表
- 从站临时监听模式
- 从站常驻监听模式
- 主站临时提供模式
- 主站常驻提供模式
- 持续连接测试（文件数据源和文件输出）
- 持续连接测试（Unix 管道数据源和管道输出）

### 持续连接测试

持续连接测试验证主站和从站之间的长时间数据传输：

1. **文件作为数据源和输出**：主站从文件读取数据并发送，从站接收数据并追加写入文件
2. **Unix 管道作为数据源和输出**：主站从命名管道读取实时数据，从站输出到命名管道
3. **随机数据生成**：每次测试运行时生成不同的随机数据，确保测试的可靠性

## 未来增强

- 虚拟串口的实时 Modbus 通信测试
- 额外的寄存器模式支持
