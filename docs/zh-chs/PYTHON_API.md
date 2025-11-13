# Python 脚本数据源 API

## 概述

Python 脚本数据源允许您使用 Python 脚本为 Modbus 主站提供动态数据。脚本会定期执行，其输出用于更新 Modbus 寄存器值。

## 执行模式

Python 脚本有两种执行模式：

### 1. 外部 CPython 模式（推荐）

使用系统的 Python 解释器（`python` 或 `python3`）在单独的进程中执行脚本。

**优点：**
- 支持所有 Python 库和模块
- 完整的 Python 标准库支持
- 兼容 Python 2.7 和 Python 3.x
- 无需额外依赖

**用法：**
```bash
--data-source python:external:/path/to/script.py
```

### 2. 内嵌 RustPython 模式（当前已禁用）

原计划使用 RustPython VM 在 Aoba 进程内执行脚本。

**状态：** 由于与 RustPython 0.4 的线程兼容性问题，此模式当前已禁用。一旦问题解决，将在未来版本中重新启用。

## JSON 输出格式

您的 Python 脚本必须以以下两种格式之一向 stdout 输出 JSON：

### 格式 1：站点数组（推荐）

```json
{
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]
          }
        ]
      }
    }
  ],
  "reboot_interval": 1000
}
```

### 格式 2：逐行 JSON

您的脚本也可以每行输出一个 JSON 对象（JSON Lines 格式）：

```json
{"stations": [{"id": 1, "mode": "master", "map": {"holding": [{"address_start": 0, "length": 5, "initial_values": [1, 2, 3, 4, 5]}]}}], "reboot_interval": 2000}
{"stations": [{"id": 1, "mode": "master", "map": {"holding": [{"address_start": 0, "length": 5, "initial_values": [6, 7, 8, 9, 10]}]}}]}
```

## JSON 模式

### Station 对象

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `id` | integer | 是 | 站点 ID (1-247) |
| `mode` | string | 是 | 站点模式：`"master"` 或 `"slave"` |
| `map` | object | 是 | 包含寄存器范围的寄存器映射 |

### Register Map 对象

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `coils` | array | 否 | 线圈寄存器范围数组 |
| `discrete_inputs` | array | 否 | 离散输入寄存器范围数组 |
| `holding` | array | 否 | 保持寄存器范围数组 |
| `input` | array | 否 | 输入寄存器范围数组 |

### Register Range 对象

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `address_start` | integer | 是 | 起始寄存器地址 (0-65535) |
| `length` | integer | 是 | 寄存器数量 (1-65536) |
| `initial_values` | array | 否 | 初始寄存器值数组 (u16) |

### 根输出对象

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `stations` | array | 是 | 站点配置数组 |
| `reboot_interval` | integer | 否 | 脚本再次执行前的等待时间（毫秒） |

## 标准错误流 (stderr)

写入 stderr 的任何输出都将被 Aoba 捕获并记录为警告。这对于调试脚本很有用：

```python
import sys
sys.stderr.write("调试：正在处理站点 1\n")
```

## 示例脚本

### 示例 1：简单静态数据

```python
#!/usr/bin/env python3
import json

# 定义站点配置
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 10,
                    "initial_values": [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]
                }
            ]
        }
    }
]

# 输出 JSON
output = {
    "stations": stations,
    "reboot_interval": 1000  # 每秒执行一次
}

print(json.dumps(output))
```

### 示例 2：来自传感器的动态数据

```python
#!/usr/bin/env python3
import json
import random
import time

# 模拟读取传感器数据
temperature = random.randint(20, 30)
humidity = random.randint(40, 60)
pressure = random.randint(1000, 1020)

# 创建包含传感器读数的站点
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 3,
                    "initial_values": [temperature, humidity, pressure]
                }
            ]
        }
    }
]

# 输出 JSON
output = {
    "stations": stations,
    "reboot_interval": 5000  # 每 5 秒更新一次
}

print(json.dumps(output))
```

### 示例 3：多个站点

```python
#!/usr/bin/env python3
import json

# 定义多个站点
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 5,
                    "initial_values": [1, 2, 3, 4, 5]
                }
            ]
        }
    },
    {
        "id": 2,
        "mode": "master",
        "map": {
            "coils": [
                {
                    "address_start": 0,
                    "length": 8,
                    "initial_values": [1, 0, 1, 0, 1, 0, 1, 0]
                }
            ]
        }
    }
]

# 输出 JSON
print(json.dumps({"stations": stations, "reboot_interval": 2000}))
```

### 示例 4：从数据库读取

```python
#!/usr/bin/env python3
import json
import sqlite3
import sys

try:
    # 连接到数据库
    conn = sqlite3.connect('/path/to/sensors.db')
    cursor = conn.cursor()
    
    # 查询最新的传感器读数
    cursor.execute('''
        SELECT station_id, register_address, value 
        FROM sensor_readings 
        WHERE timestamp > datetime('now', '-1 minute')
        ORDER BY station_id, register_address
    ''')
    
    # 按站点分组读数
    stations_data = {}
    for row in cursor.fetchall():
        station_id, address, value = row
        if station_id not in stations_data:
            stations_data[station_id] = []
        stations_data[station_id].append((address, value))
    
    # 构建站点配置
    stations = []
    for station_id, readings in stations_data.items():
        min_addr = min(addr for addr, _ in readings)
        max_addr = max(addr for addr, _ in readings)
        length = max_addr - min_addr + 1
        
        # 填充值数组
        values = [0] * length
        for addr, value in readings:
            values[addr - min_addr] = value
        
        stations.append({
            "id": station_id,
            "mode": "master",
            "map": {
                "holding": [{
                    "address_start": min_addr,
                    "length": length,
                    "initial_values": values
                }]
            }
        })
    
    conn.close()
    
    # 输出 JSON
    print(json.dumps({
        "stations": stations,
        "reboot_interval": 60000  # 每分钟更新一次
    }))

except Exception as e:
    # 将错误记录到 stderr
    sys.stderr.write(f"从数据库读取时出错：{e}\n")
    sys.exit(1)
```

## 最佳实践

1. **始终输出有效的 JSON** - 无效的 JSON 将导致脚本被忽略
2. **使用 stderr 进行调试** - 所有 stderr 输出都会被记录为警告
3. **设置适当的 reboot_interval** - 在更新频率和系统负载之间取得平衡
4. **优雅地处理错误** - 使用 try-except 块，致命错误时以非零状态退出
5. **保持脚本快速执行** - 长时间运行的脚本会阻塞 Modbus 通信
6. **验证您的输出** - 在与 Aoba 一起使用之前，独立测试您的脚本

## 测试您的脚本

您可以在与 Aoba 一起使用之前独立测试 Python 脚本：

```bash
# 运行脚本
python3 /path/to/script.py

# 验证 JSON 输出
python3 /path/to/script.py | python3 -m json.tool

# 检查错误
python3 /path/to/script.py 2>&1 | grep -i error
```

## 故障排除

### 找不到脚本

确保脚本路径是绝对路径且文件存在：
```bash
ls -l /path/to/script.py
```

### 找不到 Python

确保 Python 已安装并在您的 PATH 中：
```bash
which python3
python3 --version
```

### 权限被拒绝

使脚本可执行（Linux/macOS）：
```bash
chmod +x /path/to/script.py
```

### 无效的 JSON 输出

测试您的 JSON 输出：
```bash
python3 /path/to/script.py | jq .
```

### 脚本执行失败

检查 Aoba 日志中的 stderr 输出以获取错误消息。启用调试日志记录以获取更多详细信息。
