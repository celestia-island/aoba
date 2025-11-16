# 自定义数据源 — HTTP 与 IPC 通道

本文档介绍用于虚拟串口和真实端口监控的 HTTP 服务器和 IPC 通道功能，以实现与外部系统的数据交换。

## 概述

Aoba CLI 支持两种数据交换模式：

1. **HTTP 服务器模式**：通过 HTTP GET/POST 端点检索和上传站点数据
2. **IPC 通道模式**：使用半双工 JSON 请求-响应协议的 Unix 套接字服务器

## HTTP 服务器模式

### 描述

当配合 `--master-provide-persist` 使用 `--data-source http://<端口>` 时，会在指定端口启动 HTTP 服务器。该服务器在根端点 `/` 接受 GET 和 POST 请求。

### 端点

#### GET / - 检索站点数据

从 Modbus 存储中检索当前站点配置及实时寄存器值。

**请求：**
```bash
curl http://localhost:8080/
```

**响应：**
```json
{
  "success": true,
  "message": "Retrieved 2 stations",
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 101, 102, 103, 104, 105, 106, 107, 108, 109]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]
}
```

#### POST / - 上传站点配置

上传新的站点配置并更新内部存储。

**请求：**
```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '[
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]'
```

**响应：**
```json
{
  "success": true,
  "message": "Stations queued",
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]
}
```

### 使用示例

启动持久化主站模式并使用 HTTP 数据源：

```bash
cargo run -- \
  --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --data-source http://8080 \
  --baud-rate 9600
```

在另一个终端上传配置：

```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '[{"id":1,"mode":"master","map":{"holding":[{"address_start":0,"length":10,"initial_values":[1,2,3,4,5,6,7,8,9,10]}],"coils":[],"discrete_inputs":[],"input":[]}}]'
```

查询当前数据：

```bash
curl http://localhost:8080/
```

## IPC 通道模式

### 描述

IPC 通道模式创建一个 Unix 域套接字服务器，可接受多个并发连接。每个连接以半双工模式运行：客户端发送 JSON 请求，服务器处理一个 Modbus 事务，然后返回 JSON 响应。

### 协议

- **传输方式**：Unix 域套接字（基于文件或抽象命名空间）
- **格式**：基于行的 JSON（每行一个请求/响应，以 `\n` 结尾）
- **模式**：半双工（一个请求 → 一个响应）
- **并发性**：多个客户端可以同时连接

### 消息格式

#### 请求

任何有效的 JSON 对象。服务器验证 JSON 但不要求特定字段：

```json
{"action": "read"}
```

或者仅：

```json
{}
```

#### 成功响应

```json
{
  "success": true,
  "data": {
    "station_id": 1,
    "register_address": 0,
    "register_mode": "Holding",
    "values": [100, 101, 102, 103, 104, 105, 106, 107, 108, 109],
    "timestamp": "2025-01-15T10:30:45.123Z"
  }
}
```

#### 错误响应

```json
{
  "success": false,
  "error": "No data received"
}
```

### 使用示例

启动持久化从站模式并使用 IPC 套接字：

```bash
cargo run -- \
  --slave-listen-persist /dev/ttyUSB0 \
  --ipc-socket-path /tmp/modbus.sock \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

使用 `nc`（netcat）连接并发送请求：

```bash
echo '{"action":"read"}' | nc -U /tmp/modbus.sock
```

或使用 `socat`：

```bash
echo '{"action":"read"}' | socat - UNIX-CONNECT:/tmp/modbus.sock
```

### 多连接支持

IPC 服务器可以处理多个并发连接。每个连接在独立的异步任务中处理：

**终端 1：**
```bash
socat - UNIX-CONNECT:/tmp/modbus.sock
{"action":"read"}
# 等待响应...
```

**终端 2（同时）：**
```bash
socat - UNIX-CONNECT:/tmp/modbus.sock
{"action":"read"}
# 等待响应...
```

两个客户端将在 Modbus 事务完成时独立接收响应。

## 架构

### HTTP 服务器

- 在由 `http_daemon_registry` 管理的后台异步任务中运行
- 通过 `Arc<Mutex<ModbusStorageSmall>>` 共享 Modbus 存储
- 在 `Arc<Mutex<Vec<StationConfig>>>` 中跟踪站点配置
- GET 读取已配置站点的存储中的当前值
- POST 更新跟踪的配置和存储值

### IPC 通道服务器

- 在主循环中运行，使用 `listener.accept()` 接受连接
- 每个接受的连接通过 `task_manager::spawn_task()` 作为独立任务生成
- 连接处理器在 `BufReader` 上使用基于行的 JSON
- 调用 `listen_for_one_request()` 处理每个请求的 Modbus 事务
- Unix 系统上自动清理套接字文件

## 故障排除

### HTTP 服务器

**问题**：端口已被占用
```
Failed to bind HTTP server to 127.0.0.1:8080: Address already in use
```
**解决方案**：选择不同的端口或终止占用该端口的进程

**问题**：GET 未返回数据
```json
{"success": true, "message": "Retrieved 0 stations", "stations": []}
```
**解决方案**：首先发送 POST 请求以配置站点

### IPC 通道

**问题**：套接字文件已存在
```
Socket address already in use: /tmp/modbus.sock
```
**解决方案**：套接字文件会自动删除，但如果持续存在，请手动删除：
```bash
rm /tmp/modbus.sock
```

**问题**：套接字权限被拒绝
```
Failed to create listener for /tmp/modbus.sock: Permission denied
```
**解决方案**：确保对套接字目录有写权限

**问题**：连接立即关闭
**解决方案**：检查日志中的 JSON 解析错误或 Modbus 事务失败

## 参考资料

- Axum HTTP 框架：https://docs.rs/axum/
- Interprocess crate（Unix 套接字）：https://docs.rs/interprocess/
- Modbus 协议：https://modbus.org/
