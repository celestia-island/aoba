# 自定义数据源 — HTTP

## 快速开始 — 启动一个 CLI 接收器示例

启动应用的 CLI，使其在本地托管一个 HTTP 接口并接收 POST 的 JSON。示例（在仓库根目录执行）：

```bash
# 开发期间使用 cargo（推荐）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# 或者使用已构建的二进制：
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

上述命令会在 `127.0.0.1:8080` 启动 HTTP 服务并接受对根路径 `/` 的 `POST` 请求。可使用下面的 `curl` 示例进行测试。

## 概述

本文档说明如何通过 HTTP 将自定义数据下发/上报到应用。包含请求格式、常用 header，以及用于快速验证的 `curl` 示例。

## 接口

- 方法：`POST`
- URL：`http://<host>:<port>/`（示例：`http://localhost:8080/`）
- Content-Type：`application/json`

## 请求格式

服务接收 JSON 请求体。最小示例：

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

说明：

- 推荐使用 ISO 8601 时间戳。
- `payload` 内容根据业务而定，上例为常见的寄存器写入示例。

## curl 调试示例

将下列命令中的 `localhost:8080` 替换为你的服务地址：

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

## 预期行为

- 成功接收时返回 `200 OK` 或 `202 Accepted`。
- 若返回 4xx/5xx，请查看响应体中的错误信息以定位问题。

## 排查要点

- 确保 `Content-Type: application/json` 存在。
- 若服务需要认证，请添加 `Authorization: Bearer <token>` 或其它所需 header。
- 大体量数据可尝试 `--data-binary` 并调整服务器超时配置。

如果需要针对内部 schema 的样例，请贴出具体 JSON，我可以帮你生成匹配的请求样例。
