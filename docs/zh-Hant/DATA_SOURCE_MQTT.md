# 自定義資料來源 — MQTT

## 快速開始 — 啟動一個 CLI 接收器範例

啟動應用的 CLI，使其訂閱指定的 MQTT 話題並作為接收端。範例（在倉庫根目錄執行）：

```bash
# 開發期間使用 cargo（推薦）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# 或者使用已建構的二進位檔：
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

`mqtt://.../<topic>` URL 包含話題路徑（例如 `aoba/data/in`），CLI 會訂閱該話題。

## 概述

本文件說明如何透過 MQTT 將訊息發佈到應用的自定義資料來源，包含 Broker 配置建議、Topic 規範與用於下發的 `mosquitto_pub` 範例。

## Broker / 連線

- Host: `mqtt.example.com` 或 `localhost`
- Port: `1883`（明文）或 `8883`（TLS）
- 使用者名稱/密碼：可選，根據 Broker 要求提供
- TLS：若使用 `8883`，請配置 CA 憑證及客戶端憑證/金鑰（如需要）

## 推薦 Topic

- 上行（發往應用）：`aoba/data/in` — 應用訂閱此 Topic 以接收上報或命令
- 下發（應用發往設備/埠）：`aoba/data/out/<port>` — 應用將下發訊息發佈到指定埠的 Topic，例如 `aoba/data/out/tmp_vcom1`

## 訊息格式

應用接收 JSON 格式訊息。下面是一個通用且實用的範例，用於下發（downlink）或暫存器寫入：

```json
{
  "source": "mqtt",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": {
    "command": "write_register",
    "registers": [{"address":0, "value":"1234"}]
  }
}
```

## 使用 `mosquitto_pub` 發佈下發範例

下面範例向應用可訂閱的 `aoba/data/in` Topic 發佈下發訊息，應用接收後會處理並對指定 `port` 執行實際下發操作。

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## 注意事項與提示

- 採用可預測的 Topic 命名，便於權限與過濾管理。
- 如果使用埠路徑（如 `/tmp/vcom1`）作為識別，請確保在 Topic 映射中處理可能的特殊字元（或使用安全的埠別名）。
- 留意 retained 訊息：如果 Broker 使用 retained，下發訊息可能在客戶端重連時被再次消費，謹慎使用。

如果需要我可以幫助生成完整的測試腳本（例如一個迴圈下發並輪詢狀態的腳本），請告訴我你希望使用的工具（`mosquitto_pub`/`python-paho-mqtt` 等）。
