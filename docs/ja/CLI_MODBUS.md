# CLI Modbus 機能

本ドキュメントでは、aoba プロジェクトに追加された Modbus 操作用の新しい CLI 機能について説明します。

## 機能

### 1. ポート検出と一覧表示

#### 全ポートの一覧表示

`--list-ports` コマンドは、`--json` と組み合わせることでより詳細な情報を提供します。

```bash
aoba --list-ports --json
```

出力には以下が含まれます。

- `path`: ポートパス（例: COM1, /dev/ttyUSB0）
- `status`: "Free" または "Occupied"
- `guid`: Windows デバイス GUID（利用可能な場合）
- `vid`: USB ベンダー ID（利用可能な場合）
- `pid`: USB プロダクト ID（利用可能な場合）
- `serial`: シリアル番号（利用可能な場合）

出力例:

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

#### 単一ポートの占有状態確認

`--check-port` コマンドは、特定のポートが占有されているかどうかを検出するために使用します。スクリプトの自動化やポートステータスの監視に便利です。

```bash
aoba --check-port COM3
```

**終了コード:**

- `0` - ポートは空き、使用可能です
- `1` - ポートは他のプログラムに占有されています

**通常出力:**

```
Port COM3 is free
```

または

```
Port COM3 is occupied
```

**JSON 形式の出力:**

```bash
aoba --check-port COM3 --json
```

出力例:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

または

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**使用例:**

シェルスクリプトでの使用:

```bash
# Bash の例
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # Perform your operations
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# PowerShell の例
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. スレーブリッスンモード

#### 一時モード

1つの Modbus リクエストを受信し、応答して終了します。

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

単一の JSON レスポンスを出力して終了します。

#### 永続モード

継続的にリクエストを受信し JSONL を出力します。

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

処理されたリクエストごとに1行の JSON を出力します。

### 3. マスタープロバイドモード

- 一時モード: データを1回提供して終了します。

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

データソースから1行を読み取り、送信して終了します。

- 永続モード: 継続的にデータを提供します。

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

データソースから行を読み取り、継続的に送信します。

### データソース形式

マスターモードでは、データソースファイルは JSONL 形式である必要があります。

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

各行はスレーブに送信される更新を表します。

#### ファイルをデータソースとして使用

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Unix 名前付きパイプをデータソースとして使用

Unix 名前付きパイプ（FIFO）はリアルタイムデータストリーミングに使用できます。

```bash
# Create named pipe
mkfifo /tmp/modbus_input

# Start master in one terminal
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# Write data in another terminal
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### 出力先

スレーブモードでは、出力先を指定できます。

#### 標準出力への出力（デフォルト）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### ファイルへの出力（追記モード）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Unix 名前付きパイプへの出力

```bash
# Create named pipe
mkfifo /tmp/modbus_output

# Start slave in one terminal
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# Read data in another terminal
cat /tmp/modbus_output
```

## デーモンモード（永続動作）

CLI は**永続モード**を通じてデーモン的な継続動作をサポートします。

- **スレーブデーモン**: `--slave-listen-persist` で継続的なリッスンと応答
- **マスターデーモン**: `--master-provide-persist` で継続的なデータ提供

これらのモードは中断される（Ctrl+C）まで無期限で実行され、各操作ごとに JSONL（1行に1つの JSON オブジェクト）を出力します。以下の用途に最適です。

- 長時間実行される監視アプリケーション
- データロギングシステム
- パイプやファイルを介した他ツールとの統合
- TUI サブプロセスとの通信（`--ipc-channel` と組み合わせた場合）

デーモン使用例:

```bash
# Run as slave daemon with file output logging
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# Run as master daemon with pipe input
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**注**: TUI モードは、CLI サブプロセスとの双方向通信のために、これらの永続モードを `--ipc-channel` と共に内部的に使用します。

## パラメータ

| パラメータ | 説明 | デフォルト |
|-----------|------|-----------|
| `--station-id` | Modbus ステーション ID（スレーブアドレス） | 1 |
| `--register-address` | 開始レジスタアドレス | 0 |
| `--register-length` | レジスタ数 | 10 |
| `--register-mode` | レジスタタイプ: holding, input, coils, discrete | holding |
| `--data-source` | データソース: `file:<path>` または `pipe:<name>` | - |
| `--output` | 出力先: `file:<path>` または `pipe:<name>`（デフォルト: stdout） | stdout |
| `--baud-rate` | シリアルポートのボーレート | 9600 |
| `--debounce-seconds` | 重複 JSON 出力のデバウンス間隔（秒、浮動小数点） | 1.0 |
| `--ipc-channel` | TUI 通信用の IPC チャネル UUID（内部使用） | - |

## レジスタモード

- `holding`: ホールディングレジスタ（読み取り/書き込み）
- `input`: インプットレジスタ（読み取り専用）
- `coils`: コイル（読み取り/書き込みビット）
- `discrete`: ディスクリート入力（読み取り専用ビット）

## 統合テスト

統合テストは `examples/cli_e2e/` にあります。以下のコマンドで実行してください。

```bash
cd examples/cli_e2e
cargo run
```

### ループモードでのテスト実行

安定性テストとデバッグのため、`--loop-count` コマンドライン引数を使用してテストを複数回実行できます。

```bash
# Run tests 5 times consecutively
cargo run --example cli_e2e -- --loop-count 5

# Run tests 10 times to verify port cleanup and stability
cargo run --example cli_e2e -- --loop-count 10
```

これは以下の用途に役立ちます。

- テスト実行間のポートクリーンアップの確認
- 安定性と再現性のテスト
- 断続的な問題のデバッグ
- socat 仮想ポートのリセットが正しく動作することの確認

テストで確認される内容:

- ステータス付きの拡張ポート一覧
- スレーブリッスン一時モード
- スレーブリッスン永続モード
- マスタープロバイド一時モード
- マスタープロバイド永続モード
- 継続接続テスト（ファイルデータソースとファイル出力）
- 継続接続テスト（Unix パイプデータソースとパイプ出力）

### 継続接続テスト

継続接続テストは、マスターとスレーブ間の長時間データ送信を確認します。

1. **ファイルをデータソースと出力として使用**: マスターがファイルからデータを読み取り送信し、スレーブが受信してファイルに追記する
2. **Unix パイプをデータソースと出力として使用**: マスターが名前付きパイプからリアルタイムデータを読み取り、スレーブが名前付きパイプに出力する
3. **ランダムデータ生成**: 各テスト実行で異なるランダムデータを生成し、テストの信頼性を確保する

## 今後の拡張予定

- 仮想シリアルポートを使用したリアルタイム Modbus 通信テスト
- 追加のレジスタモードのサポート
