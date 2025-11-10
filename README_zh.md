<p align="center">
  <img src="./packages/tui/res/logo.png" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">
    青叶（Aoba）
  </h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/basic-checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/basic-checks.yml/badge.svg?branch=master" alt="Basic Checks status" />
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

## 快速开始

1. 安装 Rust 工具链
2. 安装工具：`cargo install aoba`
3. 运行工具：执行已安装的 `aoba` 二进制（或通过你的包管理器/路径运行）

说明：

- 具体文档尚在编写中。
  - 示例与部分参考材料位于 `examples/` 和 `docs/` 目录，但尚不完整。
  - 若需 CI 或自动化脚本，请查看 `./scripts/`。

## 贡献

欢迎贡献 — 请提交 issue 或 PR。仓库中包含代码风格与 CI 配置说明。
