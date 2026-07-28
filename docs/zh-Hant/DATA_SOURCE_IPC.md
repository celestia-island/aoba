# IPC 通訊（自定義資料來源）

## 快速開始 — 啟動一個 CLI 接收器範例

對於資料來源模式 `ipc:<path>`，CLI 會從命名管道（FIFO）或檔案按行讀取 JSON。要啟動一個簡單的 CLI 接收器並從 FIFO 接收資料，請按下列步驟操作：

```bash
# 建立 FIFO（一次性操作）
mkfifo /tmp/aoba_ipc.pipe

# 啟動 CLI 接收器（它將從 FIFO 路徑讀取行）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# 然後在另一個 shell 中向管道寫入一行 JSON：
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

注意：倉庫還在其它場景使用 Unix 網域通訊端 / 命名管道（用於 TUI↔CLI 通訊）。資料來源的 `ipc:<path>` 模式專用於 FIFO/檔案路徑，CLI 會按行讀取並解析 JSON。

## 概述

本文件說明如何透過 IPC（程序間通訊）將自定義資料傳送到應用。按專案約定，應用在本機上作為 IPC 的監聽者（server）執行；外部整合或輔助程式應作為客戶端連線並傳送 JSON 訊息。下面給出客戶端（傳送端）範例（Rust / Python / Node）。

## 適用場景

- 本機整合，網路開銷不必要時使用
- 需要低延遲、快速通訊的程序間通訊
- 測試或 E2E 場景中由主程序衍生的輔助程序

## 推薦訊息格式

建議使用 JSON，範例：

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Unix 網域通訊端：Rust 範例（使用 `interprocess`）

在 `Cargo.toml` 中新增依賴：

```toml
[dependencies]
interprocess = "*"
```

應用端建立並綁定通訊端（例如 `/tmp/aoba_ipc.sock`）。下面給出 Rust 客戶端範例，說明如何連線並傳送一條訊息：

客戶端（連線並傳送）：

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

- 應用負責建立並監聽 socket；客戶端只需連線並傳送。
- 如果用於本機測試且需要建立臨時監聽程序，請確保監聽端與生產環境的綁定路徑一致。
- Windows 平台請使用命名管道（例如 `\\.\pipe\aoba_ipc`）或使用跨平台函式庫。

## Python 範例（AF_UNIX）

應用端已建立並綁定 Unix 網域通訊端；下面範例僅展示客戶端如何連線並傳送 JSON 訊息：

客戶端：

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

## Node.js（ES6）範例 — UNIX 網域通訊端

應用端監聽該 socket 路徑；下面範例為客戶端連線並傳送訊息的最小範例：

客戶端：

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

## 跨平台說明

- Windows 使用命名管道（`\\.\pipe\<name>`）；Node/Python 與 Rust 均有支援或替代函式庫。
- 確保 socket 檔案權限允許對應程序連線。

如需，我可以再提供一個小型測試腳本，自動啟動服務端並從客戶端做 JSON 收發驗證（選擇偏好的語言）。
