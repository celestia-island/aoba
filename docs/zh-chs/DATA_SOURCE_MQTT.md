# 自定义数据源 — MQTT

## 快速开始 — 启动一个 CLI 接收器示例

启动应用的 CLI，使其订阅指定的 MQTT 话题并作为接收端。示例（在仓库根目录执行）：

```bash
# 开发期间使用 cargo（推荐）
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# 或者使用已构建的二进制：
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

`mqtt://.../<topic>` URL 包含话题路径（例如 `aoba/data/in`），CLI 会订阅该话题。

## 概述

本文档说明如何通过 MQTT 将消息发布到应用的自定义数据源，包含 Broker 配置建议、Topic 规范与用于下发的 `mosquitto_pub` 示例。

## Broker / 连接

- Host: `mqtt.example.com` 或 `localhost`
- Port: `1883`（明文）或 `8883`（TLS）
- 用户名/密码：可选，根据 Broker 要求提供
- TLS：若使用 `8883`，请配置 CA 证书及客户端证书/秘钥（如需要）

## 推荐 Topic

- 上行（发往应用）：`aoba/data/in` — 应用订阅此 Topic 以接收上报或命令
- 下发（应用发往设备/端口）：`aoba/data/out/<port>` — 应用将下发消息发布到指定端口的 Topic，例如 `aoba/data/out/tmp_vcom1`

## 消息格式

应用接收 JSON 格式消息。下面是一个通用且实用的示例，用于下发（downlink）或寄存器写入：

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

## 使用 `mosquitto_pub` 发布下发示例

下面示例向应用可订阅的 `aoba/data/in` Topic 发布下发消息，应用接收后会处理并对指定 `port` 执行实际下发操作。

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## 注意事项与提示

- 采用可预测的 Topic 命名，便于权限与过滤管理。
- 如果使用端口路径（如 `/tmp/vcom1`）作为标识，请确保在 Topic 映射中处理可能的特殊字符（或使用安全的端口别名）。
- 留意 retained 消息：如果 Broker 使用 retained，下发消息可能在客户端重连时被再次消费，谨慎使用。

如果需要我可以帮助生成完整的测试脚本（例如一个循环下发并轮询状态的脚本），请告诉我你希望使用的工具（`mosquitto_pub`/`python-paho-mqtt` 等）。
