# 自定義資料來源 — HTTP

## 快速開始 — 啟動一個 CLI 接收器範例

啟動應用的 CLI，使其在本機託管一個 HTTP 介面並接收 POST 的 JSON。範例（在倉庫根目錄執行）：

```bash
# 開發期間使用 cargo（推薦）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# 或者使用已建構的二進位檔：
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

上述命令會在 `127.0.0.1:8080` 啟動 HTTP 服務並接受對根路徑 `/` 的 `POST` 請求。可使用下面的 `curl` 範例進行測試。

## 概述

本文件說明如何透過 HTTP 將自定義資料下發/上報到應用。包含請求格式、常用 header，以及用於快速驗證的 `curl` 範例。

## 介面

- 方法：`POST`
- URL：`http://<host>:<port>/`（範例：`http://localhost:8080/`）
- Content-Type：`application/json`

## 請求格式

服務接收 JSON 請求體。最小範例：

```json
{
  "source": "http",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "payload": {
    "type": "register_update",
    "registers": [
      {"address": 0, "value": "1234"},
      {"address": 1, "value": "abcd"}
    ]
  }
}
```

說明：

- 推薦使用 ISO 8601 時間戳。
- `payload` 內容根據業務而定，上例為常見的暫存器寫入範例。

## curl 除錯範例

將下列命令中的 `localhost:8080` 替換為你的服務位址：

```bash
curl -v -X POST "http://localhost:8080/" \
  -H "Content-Type: application/json" \
  -d '{
    "source":"http",
    "timestamp":"2025-11-15T12:34:56Z",
    "port":"/tmp/vcom1",
    "payload":{
      "type":"register_update",
      "registers":[{"address":0,"value":"1234"}]
    }
  }'
```

## 預期行為

- 成功接收時回傳 `200 OK` 或 `202 Accepted`。
- 若回傳 4xx/5xx，請檢視回應體中的錯誤資訊以定位問題。

## 排查要點

- 確保 `Content-Type: application/json` 存在。
- 若服務需要認證，請新增 `Authorization: Bearer <token>` 或其它所需 header。
- 大體量資料可嘗試 `--data-binary` 並調整伺服器逾時配置。

如果需要針對內部 schema 的範例，請貼出具體 JSON，我可以幫你生成匹配的請求範例。
