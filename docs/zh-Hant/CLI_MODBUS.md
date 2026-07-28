# CLI Modbus 功能

本文件描述了 aoba 專案新增的 Modbus 操作 CLI 功能。

## 功能

### 1. 埠檢測與列表

#### 列出所有埠

`--list-ports` 命令現在可以與 `--json` 一起使用，提供更詳細的埠資訊：

```bash
aoba --list-ports --json
```

輸出包括：

- `path`: 埠路徑（例如 COM1, /dev/ttyUSB0）
- `status`: "Free"（空閒）或 "Occupied"（佔用）
- `guid`: Windows 設備 GUID（如果可用）
- `vid`: USB 廠商 ID（如果可用）
- `pid`: USB 產品 ID（如果可用）
- `serial`: 序號（如果可用）

範例輸出：

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

#### 檢查單個埠佔用狀態

`--check-port` 命令用於檢測特定埠是否被佔用，這對於腳本自動化和埠狀態監控非常有用：

```bash
aoba --check-port COM3
```

**退出碼：**

- `0` - 埠空閒可用
- `1` - 埠被其他程式佔用

**普通輸出：**

```
Port COM3 is free
```

或

```
Port COM3 is occupied
```

**JSON 格式輸出：**

```bash
aoba --check-port COM3 --json
```

輸出範例：

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

或

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**使用範例：**

在 shell 腳本中使用：

```bash
# Bash 範例
if aoba --check-port /dev/ttyUSB0; then
    echo "埠空閒，可以使用"
    # 執行你的操作
else
    echo "埠被佔用，請先關閉佔用該埠的程式"
    exit 1
fi
```

```powershell
# PowerShell 範例
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "埠空閒"
} else {
    Write-Host "埠被佔用"
}
```

### 2. 從站監聽模式

#### 臨時模式

監聽一個 Modbus 請求，回應後退出：

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

輸出單個 JSON 回應後退出。

#### 常駐模式

持續監聽請求並輸出 JSONL：

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

每處理一個請求輸出一行 JSON。

### 3. 主站提供模式

- 臨時模式，提供一次資料後退出：

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

從資料來源讀取一行，傳送資料後退出。

- 常駐模式，持續提供資料：

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

依次從資料來源讀取行並傳送，每次操作輸出一行 JSON。

### 資料來源格式

對於主站模式，資料來源檔案應包含 JSONL 格式：

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

每行代表一次要傳送給從站的更新。

#### 使用檔案作為資料來源

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### 使用 Unix 命名管道作為資料來源

Unix 命名管道（FIFO）可用於即時資料流傳輸：

```bash
# 建立命名管道
mkfifo /tmp/modbus_input

# 在一個終端機中啟動主站
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# 在另一個終端機中寫入資料
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### 輸出目標

對於從站模式，可以指定輸出目標：

#### 輸出到標準輸出（預設）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### 輸出到檔案（追加模式）

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### 輸出到 Unix 命名管道

```bash
# 建立命名管道
mkfifo /tmp/modbus_output

# 在一個終端機中啟動從站
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# 在另一個終端機中讀取資料
cat /tmp/modbus_output
```

## 守護程式模式（常駐執行）

CLI 透過 **常駐模式（persist modes）** 支援類似守護程式的持續執行：

- **從站守護程式**：使用 `--slave-listen-persist` 實現持續監聽和回應
- **主站守護程式**：使用 `--master-provide-persist` 實現持續資料提供

這些模式會無限期執行直到被中斷（Ctrl+C），並以 JSONL 格式輸出（每行一個 JSON 物件）記錄每次操作。它們適用於：

- 長時間執行的監控應用
- 資料記錄系統
- 透過管道或檔案與其他工具整合
- TUI 子程序通訊（與 `--ipc-channel` 配合使用）

守護程式模式使用範例：

```bash
# 作為從站守護程式執行，輸出到檔案日誌
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# 作為主站守護程式執行，從管道讀取輸入
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**注意**：TUI 模式內部使用這些常駐模式配合 `--ipc-channel` 與 CLI 子程序進行雙向通訊。

## 參數

| 參數 | 說明 | 預設值 |
|-----------|-------------|---------|
| `--station-id` | Modbus 站點 ID（從站位址） | 1 |
| `--register-address` | 起始暫存器位址 | 0 |
| `--register-length` | 暫存器數量 | 10 |
| `--register-mode` | 暫存器型別：holding、input、coils、discrete | holding |
| `--data-source` | 資料來源：`file:<path>` 或 `pipe:<name>` | - |
| `--output` | 輸出目標：`file:<path>` 或 `pipe:<name>`（預設：標準輸出） | stdout |
| `--baud-rate` | 串列埠鮑率 | 9600 |
| `--debounce-seconds` | 重複 JSON 輸出的去抖動視窗（秒，浮點數） | 1.0 |
| `--ipc-channel` | TUI 通訊的 IPC 通道 UUID（內部使用） | - |

## 暫存器模式

- `holding`: 保持暫存器（可讀寫）
- `input`: 輸入暫存器（唯讀）
- `coils`: 線圈（可讀寫位元）
- `discrete`: 離散輸入（唯讀位元）

## 整合測試

整合測試位於 `examples/cli_e2e/`。執行測試：

```bash
cd examples/cli_e2e
cargo run
```

### 迴圈模式執行測試

為了進行穩定性測試和除錯，可以使用 `--loop-count` 命令列參數多次執行測試：

```bash
# 連續執行測試 5 次
cargo run --example cli_e2e -- --loop-count 5

# 執行測試 10 次以驗證埠清理和穩定性
cargo run --example cli_e2e -- --loop-count 10
```

這對於以下場景很有用：

- 驗證測試執行之間的埠清理
- 測試穩定性和可重複性
- 除錯間歇性問題
- 確保 socat 虛擬埠重置正常運作

測試驗證：

- 帶狀態的增強埠列表
- 從站臨時監聽模式
- 從站常駐監聽模式
- 主站臨時提供模式
- 主站常駐提供模式
- 持續連線測試（檔案資料來源和檔案輸出）
- 持續連線測試（Unix 管道資料來源和管道輸出）

### 持續連線測試

持續連線測試驗證主站和從站之間的長時間資料傳輸：

1. **檔案作為資料來源和輸出**：主站從檔案讀取資料並傳送，從站接收資料並追加寫入檔案
2. **Unix 管道作為資料來源和輸出**：主站從命名管道讀取即時資料，從站輸出到命名管道
3. **隨機資料生成**：每次測試執行時生成不同的隨機資料，確保測試的可靠性

## 未來增強

- 虛擬串列埠的即時 Modbus 通訊測試
- 額外的暫存器模式支援
