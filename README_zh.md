<p align="center">
  <img src="./packages/tui/res/logo.png" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">
    青叶（Aoba）
  </h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg?branch=master" alt="Checks status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master" alt="E2E TUI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master" alt="E2E CLI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/blob/master/LICENSE">
    <img src="https://img.shields.io/github/license/celestia-island/aoba?color=blue" alt="License" />
  </a>
  <a href="https://github.com/celestia-island/aoba/releases/latest">
    <img src="https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver" alt="Latest Version" />
  </a>
</p>

<p align="center">
  <a href="./README.md">EN</a> | ZH
</p>

本项目为多协议调试与模拟 CLI 工具，支持 Modbus RTU、MQTT、TCP 等协议。

> 正在积极开发中

## 功能

- 串口和网络协议调试
- 协议模拟（主/从站、客户端/服务端）
- 自动切换 TUI/GUI
- 创建与管理虚拟串口
- **守护进程模式**：非交互式后台运行，支持自动加载配置

## 快速开始

1. 安装 Rust 工具链
2. 安装工具：`cargo install aoba`
3. 运行工具：执行已安装的 `aoba` 二进制（或通过你的包管理器/路径运行）

说明：

- 具体文档尚在编写中。
  - 示例与部分参考材料位于 `examples/` 和 `docs/` 目录，但尚不完整。
  - 若需 CI 或自动化脚本，请查看 `./scripts/`。

## 守护进程模式

守护进程模式允许 `aoba` 在非交互式环境中运行，适用于需要 TUI 配置功能（如透明端口转发）但不需要交互界面的场景。

### 使用方法

```bash
# 使用默认配置文件（当前目录下的 aoba_tui_config.json）
aoba --daemon

# 或使用简写
aoba -d

# 指定自定义配置文件路径
aoba --daemon --daemon-config /path/to/config.json

# 指定日志文件（同时输出到终端和文件）
aoba --daemon --log-file /path/to/daemon.log
```

### 工作原理

1. **配置加载**：从工作目录或指定路径加载 TUI 配置文件
2. **自动启动**：自动启动所有已配置的端口和站点
3. **双路日志**：同时输出日志到终端和文件
4. **无 UI**：不启动交互式界面，仅运行核心线程

### 准备配置文件

首先使用 TUI 模式创建和配置端口：

```bash
# 启动 TUI 进行配置
aoba --tui

# 在 TUI 中：
# 1. 配置端口和 Modbus 站点
# 2. 按 Ctrl+S 保存配置
# 3. 退出 TUI
```

配置文件将自动保存到 `./aoba_tui_config.json`。

### 错误处理

如果配置文件不存在，守护进程会提示错误并退出：

```
Error: Configuration file not found: ./aoba_tui_config.json

Daemon mode requires a configuration file. You can:
1. Run TUI mode first to create and save a configuration
2. Specify a custom config path with --daemon-config <FILE>
```

### 典型使用场景

- **透明端口转发**：在后台运行透明转发服务
- **自动化测试**：CI/CD 环境中自动启动 Modbus 模拟器
- **远程部署**：在无头服务器上运行 Modbus 服务

## 贡献

欢迎贡献 — 请提交 issue 或 PR。仓库中包含代码风格与 CI 配置说明。
