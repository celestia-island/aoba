<p align="center">
  <img src="./res/logo.png" alt="Aoba Logo" width="240" />
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

专用于 Modbus RTU 协议的调试与转换工具，支持硬件串口与网络端口的转发协议，提供 TUI 界面与 CLI 接口。

## 功能

- 支持 Modbus RTU（主/从）协议的调试与模拟，支持四种寄存器类型：保持寄存器 (holding)、输入寄存器 (input)、线圈 (coils) 与离散输入 (discrete)。
- 提供功能丰富的 CLI：端口检测/查询（`--list-ports` / `--check-port`）、主/从模式（`--master-provide` / `--slave-listen`）及其持久化模式（`--*-persist`），输出可为 JSON/JSONL，适用于脚本与 CI 集成。
- 交互式 TUI：通过终端 UI 进行端口、站点与寄存器的图形化配置；支持保存/加载配置（`Ctrl+S` 保存并自动启用端口），并与 CLI 通过 IPC 通信进行集成测试与自动化使用。
- 多种数据源与协议支持：支持物理/虚拟串口（通过 `socat` 管理）、HTTP、MQTT、IPC（Unix socket / 命名管道）、文件与管道（FIFO）作为数据下行/上行来源或输出目标。
- 端口转发（Port Forwarding / 透明转发）：在 TUI 内部配置源端口与目标端口实现数据转发或数据复制，用于监控、桥接或测试场景。
- 守护进程模式（daemon）：以非交互方式运行，自动加载 TUI 保存的配置并启动所有已配置的端口/站点，适用于嵌入式部署与 CI 环境。
- 虚拟端口与测试工具：包含 `scripts/socat_init.sh`（用于创建虚拟串口 vcom）及广泛的 E2E 示例（`examples/cli_e2e`、`examples/tui_e2e`），方便本地与 CI 测试。
- 可扩展与集成：支持将串口数据通过 HTTP/MQTT/IPC 转发或接收，便于与其他服务集成与远程控制。

> 注意：使用 `--no-config-cache` 标志可以禁用保存/加载 TUI 配置（即不使用配置缓存），`--config-file <FILE>` 与 `--no-config-cache` 互斥。

## 快速开始

1. 安装 Rust 工具链
2. 克隆本仓库并进入目录
3. 运行 `cargo install --path .` 安装 Aoba
4. 运行 `aoba`，默认启动 TUI 界面进行配置与操作；如需在 TUI 模式下保存配置并留到后续使用，请参考下文“持久配置文件”章节

## 持久配置文件

`--config-file <FILE>` 用于显式指定 TUI 的配置文件路径（守护进程可通过 `--daemon-config` 指定），该选项与 `--no-config-cache` 冲突。`--no-config-cache` 会禁用配置的加载与保存（即不开启配置缓存），因此不能与 `--config-file <FILE>` 同时使用，命令行会拒绝此组合。

示例：

```bash
# 启动 TUI 并显式使用自定义配置文件（允许加载/保存）
aoba --tui --config-file /path/to/config.json

# 启动 TUI 并禁用配置缓存（不加载/保存配置），这是默认选项
aoba --tui --no-config-cache
```

在此之后，可以以非交互式方式运行 Aoba 守护进程，加载之前保存的配置文件：

```bash
# 启动 Aoba 守护进程，加载之前保存的配置文件
aoba --daemon --config-file /path/to/config.json
```

建议使用 systemd 或其他进程管理工具来管理守护进程，以下是一个简单的 systemd 服务单元示例：

```ini
# 写入 /etc/systemd/system/aoba.service 并启用服务
sudo tee /etc/systemd/system/aoba.service <<EOF
[Unit]
Description=Aoba Modbus RTU Daemon
Wants=network.target
After=network.target network-service
StartLimitIntervalSec=0

[Service]
Type=simple
WorkingDirectory=/home/youruser
ExecStart=/usr/local/bin/aoba --daemon --config-file /home/youruser/config.json
Restart=always
RestartSec=1s

[Install]
WantedBy=multi-user.target
EOF
```

## 典型使用场景

- **自动化测试**：CI/CD 环境中自动启动 Modbus 模拟器
- **嵌入式系统**：在诸如树莓派这样的开发板上运行 Modbus 守护进程，配合 CH340 等 USB 转串口模块工作
