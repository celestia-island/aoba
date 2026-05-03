# IPC 通信（カスタムデータソース）

## クイックスタート — 小規模な CLI レシーバーの実行

データソースモード `ipc:<path>` では、CLI は名前付きパイプ（FIFO）または通常のファイルから JSON 行を読み取ります。FIFO から読み取る小規模な CLI レシーバーを起動するには、以下の手順に従ってください。

```bash
# create a FIFO (one-time)
mkfifo /tmp/aoba_ipc.pipe

# start the CLI receiver (it will read lines from the FIFO path)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# then, from another shell, write a JSON line into the pipe:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

注: リポジトリでは他の IPC（TUI↔CLI）にも Unix ドメインソケット / 名前付きパイプを使用しています。`ipc:<path>` データソースモードは、具体的には CLI が行単位でオープンして読み取れる FIFO/ファイルパスを想定しています。

## 概要

本ドキュメントでは、アプリケーションが IPC（プロセス間通信）経由でカスタムデータを受け取る方法について説明します。リポジリ/アプリケーションの設計では、アプリケーションが IPC リスナー（サーバー）として動作し、サードパーティの統合やヘルパープログラムがクライアントとして動作して、アプリケーションのソケットに JSON メッセージを送信します。以下に、接続とメッセージ送信の方法を示すクライアントのみの例（Rust/Python/Node）を示します。

## IPC の使いどころ

- ネットワークのオーバーヘッドが不要なローカル統合
- 同一ホスト上のプロセス間での高速・低レイテンシ通信
- ヘルパープロセスを生成するテストハーネスや E2E セットアップ

## メッセージ形式（推奨）

ポータビリティのため JSON を使用してください。メッセージ例:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Unix ドメインソケット: Rust の例（`interprocess` 使用）

`Cargo.toml` に依存関係を追加:

```toml
[dependencies]
interprocess = "*"
```

アプリケーションは Unix ドメインソケット（例: `/tmp/aoba_ipc.sock`）でリッスンします。以下の Rust の例は、クライアントがそのソケットに接続して単一の JSON メッセージを送信する方法を示しています。

クライアント（接続 & 送信）:

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

注:

- アプリケーションがソケットの作成とバインドを行います（リスナー）。クライアントプログラムは同じパスをバインドしようとせず、接続のみを行ってください。
- テストのために両側を制御する場合、ローカルで小さなリスナーを実行できます。本番ではアプリケーションがソケットパスを提供します。
- Windows では名前付きパイプ（`\\.\pipe\aoba_ipc` のようなパス）を使用するか、`interprocess` のクロスプラットフォーム API を使用してください。

## Python の例（AF_UNIX）

アプリケーションが Unix ドメインソケットを作成してバインドします。以下の Python スニペットは、クライアントが接続してアプリケーションのソケットパスに JSON メッセージを送信する方法を示しています。

クライアント:

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

## Node.js（ES6）の例 — Unix ドメインソケット

アプリケーションはソケットパスでリッスンします。以下の Node.js スニペットは、クライアントが接続して JSON メッセージを送信する方法を示しています。

クライアント:

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

## クロスプラットフォームの注意事項

- Windows では名前付きパイプ（`\\.\pipe\<name>`）を使用してください。Node と Python にはどちらも名前付きパイプを操作するライブラリがあります。Rust ではクロスプラットフォーム対応の `interprocess` を使用できます。
- プロセスが接続できるように、ソケットファイルのパーミッションを適切に設定してください。
