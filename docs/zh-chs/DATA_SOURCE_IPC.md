# IPC 通信（自定义数据源）

## 快速开始 — 启动一个 CLI 接收器示例

对于数据源模式 `ipc:<path>`，CLI 会从命名管道（FIFO）或文件按行读取 JSON。要启动一个简单的 CLI 接收器并从 FIFO 接收数据，请按下列步骤操作：

```bash
# 创建 FIFO（一次性操作）
mkfifo /tmp/aoba_ipc.pipe

# 启动 CLI 接收器（它将从 FIFO 路径读取行）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# 然后在另一个 shell 中向管道写入一行 JSON：
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

注意：仓库还在其它场景使用 Unix 域套接字 / 命名管道（用于 TUI↔CLI 通信）。数据源的 `ipc:<path>` 模式专用于 FIFO/文件路径，CLI 会按行读取并解析 JSON。

## 概述

本文档说明如何通过 IPC（进程间通信）将自定义数据发送到应用。按项目约定，应用在本机上作为 IPC 的监听者（server）运行；外部集成或辅助程序应作为客户端连接并发送 JSON 消息。下面给出客户端（发送端）示例（Rust / Python / Node）。

## 适用场景

- 本机集成，网络开销不必要时使用
- 需要低延迟、快速通信的进程间通信
- 测试或 E2E 场景中由主进程派生的辅助进程

## 推荐消息格式

建议使用 JSON，示例：

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Unix 域套接字：Rust 示例（使用 `interprocess`）

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
interprocess = "*"
```

应用端创建并绑定套接字（例如 `/tmp/aoba_ipc.sock`）。下面给出 Rust 客户端示例，说明如何连接并发送一条消息：

客户端（连接并发送）：

```rust
use std::io::{Read, Write};
use interprocess::local_socket::LocalSocketStream;

fn main() -> std::io::Result<()> {
    let mut stream = LocalSocketStream::connect("/tmp/aoba_ipc.sock")?;
    let msg = r#"{"source":"ipc","type":"downlink","body":{"command":"ping"}}"#;
    stream.write_all(msg.as_bytes())?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    println!("Response: {}", resp);
    Ok(())
}
```

注意：

- 应用负责创建并监听 socket；客户端只需连接并发送。
- 如果用于本地测试且需要创建临时监听进程，请确保监听端与生产环境的绑定路径一致。
- Windows 平台请使用命名管道（例如 `\\.\pipe\aoba_ipc`）或使用跨平台库。

## Python 示例（AF_UNIX）

应用端已创建并绑定 Unix 域套接字；下面示例仅展示客户端如何连接并发送 JSON 消息：

客户端：

```python
import socket
PATH = '/tmp/aoba_ipc.sock'
cli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
cli.connect(PATH)
cli.sendall(b'{"source":"ipc","type":"downlink","body":{"command":"ping"}}')
resp = cli.recv(65536)
print('Response:', resp)
cli.close()
```

## Node.js（ES6）示例 — UNIX 域套接字

应用端监听该 socket 路径；下面示例为客户端连接并发送消息的最小示例：

客户端：

```javascript
import net from 'net';

const PATH = '/tmp/aoba_ipc.sock';
const client = net.createConnection({ path: PATH }, () => {
  client.write(JSON.stringify({ source: 'ipc', type: 'downlink', body: { command: 'ping' } }));
});

client.on('data', (data) => {
  console.log('Response:', data.toString());
  client.end();
});
```

## 跨平台说明

- Windows 使用命名管道（`\\.\pipe\<name>`）；Node/Python 与 Rust 均有支持或替代库。
- 确保 socket 文件权限允许对应进程连接。

如需，我可以再提供一个小型测试脚本，自动启动服务端并从客户端做 JSON 收发验证（选择 prefer 的语言）。
