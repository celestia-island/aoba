# Python 脚本数据源指南

## 概述

Aoba 提供基于 RustPython 的嵌入式运行器，用于驱动 Python 数据源。脚本在独立的解释器线程内执行，并将结构化站点数据返回给 Modbus 主站。

> 如果需要完整的 CPython 生态或原生扩展，请将脚本保持在外部进程中运行，并通过 `--data-source ipc:<路径>` 选项向 Aoba 流式推送 JSON，复用现有的 IPC 数据源即可。

## RustPython 嵌入式模式

- **命令格式**：`--data-source python:<路径>` 或 `--data-source python:embedded:<路径>`。
- 脚本在独立的 RustPython 解释器线程中运行。
- 运行器会向主站写入 `PythonOutput` 结构。目前实现仍属实验阶段：stdout 捕获能力有限，脚本需配合辅助函数（参见 `examples/cli_e2e/scripts/test_embedded.py`）。
- 脚本抛出的异常会在 Aoba 日志中显示。

### 示例

```bash
cargo run -- --enable-virtual-ports \
  --master-provide-persist /tmp/vcom1 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --data-source python:embedded:$(pwd)/examples/cli_e2e/scripts/test_embedded.py
```

## 为什么不再集成 CPython？

现在移除 CPython 子进程运行器，是因为 IPC 数据源已经能够从任意外部进程流式读取 JSON；继续维护第二套运行时只会增加复杂度，却不会带来额外能力。

## 迁移说明

- `python:external:<路径>` 现在会提示使用 `ipc:<路径>`。
- CLI E2E 测试已移除 CPython 专用模块。
- 文档与示例脚本已更新，聚焦 RustPython 与 IPC 工作流。

## 限制

- RustPython 执行仍在不断完善；stdout 捕获、更多集成功能后续会补充。
- 依赖原生扩展或 CPython 专用模块的脚本建议继续通过 `--data-source ipc:<路径>` 推送 JSON。
