# aoba — 项目状态与计划 (PLAN)

> 刷新于 2026-07-14。Modbus RTU 调试 / 模拟 CLI + TUI，socat 虚拟串口配套。

## 1. 项目概述

- **名称**：`aoba`
- **简介**：Modbus RTU 调试/模拟工具（多数据源：serial / HTTP / MQTT / IPC），同时提供 TUI 调试面板；常与 evernight 联用做回归测试。
- **远程仓库**：https://github.com/celestia-island/aoba.git
- **技术栈**：Rust / tokio / tokio-modbus / ratatui / clap
- **类别**：tool（CLI + TUI）

## 2. 当前状态

- **当前分支**：`dev`
- **工作区**：干净
- **最近提交时间**：2026-07-13
- **最近提交**：`🔥 Remove IDE config (.trae/) from git tracking.`
- **本地领先 `origin/dev`**：0

## 3. 未提交改动

无

## 4. 近期进展

- `🔥 Remove IDE config (.trae/) from git tracking.`
- `🔧 Pin script recipes to the resolved Git Bash to survive WSL shadowing.`
- `🔧 Switch the justfile to Git Bash and fetch devtools recipes on demand.`
- `♻️ Standardize windows-shell to pwsh.exe across celestia repos.`
- `🐛 Replace shebang recipes with [script(...)] to fix the Windows cygpath error.`

## 5. 后续计划

1. **多协议扩面**：当前 RTU / TCP；可加 ASCII 与 RTU-over-TCP 桥。
2. **Mock 协约稳定化**：与 evernight `scripts/mock/platforms/modbus_mock.py` 行为保持寄存器布局一致。
3. **WSL/Cygwin 串口路径**：解决 WSL 下 `/dev/ttyUSB*` 与 Windows `COMx` 之间的桥。
4. **记录回放**：把会话日志（请求 / 响应 / 时延）落盘，便于复现。

## 6. 跨仓依赖

- 与 evernight 配套；共享 arona 的日志格式。
- 配套脚本：`scripts/socat-virtual-serial.sh`（WSL 下虚拟串口）。

---

## 既有详细计划（存档）

详细命令、子命令、JSON 输出 schema 在 `docs/en/README.md` 与 `docs/en/`；本文件只承载"当前态 → 后续计划"两部分。
