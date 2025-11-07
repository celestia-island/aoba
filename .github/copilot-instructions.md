# Agent 项目指南

## 迁移后包布局

- `packages/tui`：拥有终端 UI、全局状态管理和 TUI 驱动的 IPC 前端
- `packages/cli`：托管 CLI 二进制文件、命令分派和长期运行的 Modbus 工作进程
- `packages/protocol`：共享 IPC 定义、状态模式和 Modbus 传输原语
- `packages/ci_utils`：每个分层测试套件使用的共享测试工具实用程序

所有四个 crate 都保留在顶级工作区中。共享依赖项必须在根 `Cargo.toml` 中声明；单个包清单仅添加特定于包的额外内容。

## 测试套件分段

- `TUI E2E`：针对模拟的 TUI→CLI IPC 端点测试 TUI 全局状态，断言命令发出和入站寄存器差异。在 `examples/tui_e2e` 中实现。
- `CLI E2E`：涵盖虚拟化 TUI 传输与实时 CLI 运行时之间的双向 IPC，加上从原始 CLI 套件移动的传统 stdio 场景。在 `examples/cli_e2e` 中实现。

当前 CI 工作流反映了这种分割：`e2e-tests-cli.yml` 执行 CLI 矩阵，`e2e-tests-tui.yml` 驱动 TUI E2E 测试套件。每个工作流在执行模块之前构建关联的示例 crate 和主要包。

本文档的其余部分重点关注 `TUI E2E` 测试层，其中状态监控继续是主要的验证策略。有关特定于层的约定，请参考每个套件的 README（创建时）。

## 工作区依赖策略

- 在根 `Cargo.toml` 中一次性声明共享版本
- 通过 `[workspace.dependencies]` 重新导出工作区成员
- 仅在包是唯一消费者时在包内添加 `[dependencies]` 条目

## Rust `use` 语句指南

为了保持工作区中导入部分的一致性，请应用以下规则：

1. **组顺序**
    - **组 1 – 共享实用程序 crate：** `std`、核心语言 crate 和广泛可重用的第三方库，如 `anyhow`、`serde`、`regex` 等。
    - **组 2 – 特定领域 crate：** 针对终端/Modbus/UI 场景的外部 crate（例如 `serialport`、`rmodbus`、`ratatui`）或任何不是明确通用实用程序的库。
    - **组 3 – 工作区/内部 crate：** 以 `crate`、`super` 开头或此工作区中任何包的导入（例如 `aoba_ci_utils`）。
2. **间距**
    - 用单个空行分隔组。
    - 在最终组后，在代码前添加一个空行。
3. **模块声明**
    - 在 `mod.rs`、`lib.rs` 和 `main.rs` 中，在第一个 `use` 块之前发出每个 `mod`/`pub mod` 声明。
4. **去重路径**
    - 将重复的前缀合并到大括号组中。例如，连续的 `use std::collections::HashMap;` 和 `use std::sync::Arc;` 变为 `use std::{collections::HashMap, sync::Arc};`。

### 导入强制执行脚本

- 助手脚本 `scripts/enforce_use_groups.py` 自动执行上述每个规则。在发送 PR 或调用标准格式化工具链之前运行它。
- `basic-check` CI 工作流必须在 `cargo fmt`、`cargo check` 和 `cargo clippy` **之前** 执行此脚本，以便差异保持一致。
- 因为它纯粹用于格式化，`scripts/` 下的 Python 助手通过 `.gitattributes` 排除在源语言统计之外。

## 基于 IPC 架构的 TUI E2E 测试

### 概述

TUI E2E 测试框架使用现代基于 IPC 的架构，提供可靠、快速和确定性的测试：

1. **基于 IPC 的通信**：通过 Unix 域套接字直接进行进程通信
   - 消除对终端仿真的需要（移除了 expectrl/vt100）
   - 测试进程与 TUI 进程之间的双向消息传递
   - 基于 JSON 的键盘事件和屏幕内容协议
   - 自动连接重试和超时处理

2. **两种测试模式**：
   - **屏幕捕获模式**（`--screen-capture-only`）：快速 UI 测试，无需进程生成，直接操作模拟状态
   - **DrillDown 模式**（默认）：使用 IPC 对真实 TUI 进程进行完整集成测试

3. **基于 TOML 的工作流**：`workflow/**/*.toml` 文件中的测试定义
   - 声明式测试步骤，具有键盘输入和屏幕验证
   - 无需快照文件或终端转义序列解析
   - 易于阅读、编写和维护

### 测试模式

#### 屏幕捕获模式

- 执行速度快（比 DrillDown 快 5-10 倍）
- 无需进程生成
- 直接模拟状态操作
- 理想的 UI 回归测试和快速开发
- 使用 `ratatui::TestBackend` 进行渲染

**用法：**

```bash
cargo run --package tui_e2e -- --screen-capture-only --module single_station_master_coils
```

#### DrillDown 模式

- 带有 `--debug-ci` 标志的真实 TUI 进程
- 通过 IPC 进行完整集成测试
- 键盘输入模拟
- 实际屏幕内容验证
- 测试完整用户工作流

**用法：**

```bash
cargo run --package tui_e2e -- --module single_station_master_coils
```

### IPC 通信流程

```raw
┌─────────────┐                    ┌──────────────┐
│   TUI E2E   │                    │  TUI Process │
│   Test      │                    │  (--debug-ci)│
│             │                    │              │
│  IpcSender  │ ◄──── IPC ─────►   │  IpcReceiver │
│             │   Unix Socket      │              │
└─────────────┘                    └──────────────┘
      │                                   │
      │ 1. KeyPress                       │ 2. Process Input
      │ 2. RequestScreen                  │ 3. Render to TestBackend
      │ 3. Receive ScreenContent          │ 4. Send Screen Content
      └───────────────────────────────────┘
```

### 关键优势

- **无终端依赖**：移除了 expectrl 和 vt100，直接使用 IPC
- **确定性**：无终端渲染导致的时序问题或竞争条件
- **快速**：屏幕捕获比终端仿真快 10-40 倍
- **可靠**：99%+ 测试可靠性 vs 旧方法的 70-80%
- **可维护**：TOML 工作流比快照测试更容易编写和理解
- **真正集成**：DrillDown 模式测试真实 TUI 进程，而不是模拟

有关详细文档，请参见：

- [TUI E2E 测试 README](../examples/tui_e2e/README.md)
- [IPC 架构详情](../examples/tui_e2e/IPC_ARCHITECTURE.md)

### 调试模式激活

#### 对于 TUI 进程

使用 `--debug-ci-e2e-test` 标志启动 TUI：

```bash
cargo run --package aoba -- --tui --debug-ci-e2e-test
```

这将创建 `/tmp/ci_tui_status.json`，每 500ms 进行定期状态转储。

**对于 E2E 测试**，还添加 `--no-config-cache` 以防止配置持久化：

```bash
cargo run --package aoba -- --tui --debug-ci-e2e-test --no-config-cache
```

`--no-config-cache` 标志禁用 `aoba_tui_config.json` 的加载和保存，
确保每个测试以干净状态开始，而不受之前运行的干扰。
这由 TUI E2E 测试框架在 `setup_tui_test()` 中**自动使用**。

#### 对于 CLI 子进程

当 TUI 进程在调试模式下生成时，CLI 子进程自动继承调试模式。`--debug-ci-e2e-test` 标志自动注入。

手动 CLI 调用：

```bash
cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --debug-ci-e2e-test
```

这将创建 `/tmp/ci_cli_vcom1_status.json`，使用端口基本名称进行定期状态转储（例如 "/tmp/vcom1" → "vcom1"）。

### 注意：在 Windows（非 CI）上运行命令

如果您在本地 Windows 机器（非 CI）上运行这些命令，并且使用 WSL（Windows Subsystem for Linux），我们建议将必须在 Unix 式环境中运行的命令包装在 `wsl bash -lc '...'` 中，以便在 WSL 中正确解析路径和临时文件位置（例如 `/tmp`）。

例如：

```bash
# 在 WSL 中以调试模式启动 TUI
wsl bash -lc 'cargo run --package aoba -- --tui --debug-ci-e2e-test'

# 在 WSL 中手动启动 CLI 子进程（调试模式）
wsl bash -lc 'cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --debug-ci-e2e-test'
```

如果您在本机 Windows shell（PowerShell / cmd）中运行上述命令，您可能会遇到路径或权限问题，因为调试状态文件写入 Unix 风格的临时目录（例如 `/tmp`）。使用 `wsl bash -lc '...'` 明确在 WSL 中运行命令，避免这些问题。

### 状态文件格式

#### TUI 状态（`/tmp/ci_tui_status.json`）

```json
{
  "ports": [
    {
      "name": "/tmp/vcom1",
      "enabled": true,
      "state": "OccupiedByThis",
      "modbus_masters": [
        {
          "station_id": 1,
          "register_type": "Holding",
          "start_address": 0,
          "register_count": 10
        }
      ],
      "modbus_slaves": [],
      "log_count": 5
    }
  ],
  "page": "ModbusDashboard",
  "timestamp": "2025-10-19T16:41:40.123+00:00"
}
```

#### CLI 状态（`/tmp/ci_cli_{port}_status.json`）

```json
{
  "port_name": "/tmp/vcom1",
  "station_id": 1,
  "register_mode": "Holding",
  "register_address": 0,
  "register_length": 10,
  "mode": "SlaveListen",
  "timestamp": "2025-10-19T16:41:40.456+00:00"
}
```

### 使用状态监控进行测试

#### 示例测试结构

```rust
use ci_utils::{
    spawn_expect_process,
    wait_for_tui_page,
    wait_for_port_enabled,
    wait_for_modbus_config,
    read_tui_status,
};

async fn test_tui_master_configuration() -> Result<()> {
    // 使用调试模式启用生成 TUI
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;

    // 等待 TUI 初始化并开始写入状态
    // 注意：在生产测试中，更喜欢使用 wait_for_tui_page() 而不是 sleep
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 等待 TUI 到达 Entry 页面
    wait_for_tui_page("Entry", 10, None).await?;

    // 执行 UI 操作（导航、配置等）
    // ... 配置端口的光标操作 ...

    // 等待端口启用
    wait_for_port_enabled("/tmp/vcom1", 10, None).await?;

    // 等待 modbus 主配置
    wait_for_modbus_config("/tmp/vcom1", true, 1, 10, None).await?;

    // 读取当前状态进行详细验证
    let status = read_tui_status()?;
    assert_eq!(status.page, "ModbusDashboard");

    Ok(())
}
```

#### 可用的监控函数

##### 等待函数（带超时和重试）

- `wait_for_tui_page(page, timeout_secs, retry_interval_ms)` - 等待 TUI 到达特定页面
- `wait_for_port_enabled(port_name, timeout_secs, retry_interval_ms)` - 等待端口启用
- `wait_for_modbus_config(port_name, is_master, station_id, timeout_secs, retry_interval_ms)` - 等待 modbus 配置
- `wait_for_cli_status(port_name, timeout_secs, retry_interval_ms)` - 等待 CLI 子进程状态

##### 直接读取函数

- `read_tui_status()` - 从 `/tmp/tui_e2e_status.json` 读取当前 TUI 状态
- `read_cli_status(port)` - 从 `/tmp/cli_e2e_{port}.log` 读取当前 CLI 状态
- `port_exists_in_tui(port_name)` - 检查端口是否存在于 TUI 中
- `get_port_log_count(port_name)` - 获取端口的日志数量

### TUI 端口启用机制（关键）

**重要**：理解 TUI 中如何启用/禁用端口对于编写正确的 E2E 测试至关重要。

#### 端口启用工作原理

**使用 `Ctrl + S` 保存 Modbus 配置时，端口自动启用：**

```rust
// 配置站点（站点 ID、寄存器类型、地址、长度）
// ... 创建站点 1、站点 2 等 ...

// 保存配置 - 这会自动启用端口
let actions = vec![
    CursorAction::PressCtrlS,
    CursorAction::Sleep { ms: 5000 }, // 等待端口启用和稳定
];
```

**关键点：**

1. **`Ctrl + S` 触发端口启用**：在 Modbus 面板中按 `Ctrl + S` 时，TUI 保存配置并**自动启用端口**
2. **端口状态从 `Disabled` → `Running`**：Ctrl + S 后，标题栏中的状态指示器更改为显示 `Running ●`
3. **无需手动切换**：您**不必**手动切换"Enable Port"字段或在其上按右箭头
4. **Escape 不会启用端口**：仅按 Escape 离开 Modbus 面板**不会**触发端口启用（这是之前的误解）

#### 常见错误：冗余端口重启

**错误 - 更新寄存器后冗余离开/返回/验证：**

```rust
// Ctrl + S 后，端口已经运行 ●
update_tui_registers(&mut session, &mut cap, &data, false).await?;

// ❌ 错误：无需离开并返回来触发重启
let actions = vec![
    CursorAction::PressEscape,  // ❌ 这不会重启端口
    CursorAction::Sleep { ms: 3000 },
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 },
    CursorAction::PressEnter,  // ❌ 不必要的返回面板
];
```

**正确 - Ctrl + S 后端口已启用：**

```rust
// 保存配置（自动启用端口）
let actions = vec![
    CursorAction::PressCtrlS,
    CursorAction::Sleep { ms: 5000 },
];
execute_cursor_actions(&mut session, &mut cap, &actions, "save_and_enable").await?;

// 验证端口已启用（我们已经在 Modbus 面板中，状态指示器可见）
let status = verify_port_enabled(&mut session, &mut cap, "verify_enabled").await?;

// 更新寄存器值（端口保持运行）
update_tui_registers(&mut session, &mut cap, &data, false).await?;

// ✅ 正确：端口仍在运行，直接继续测试
test_modbus_communication(...).await?;
```

#### 端口何时被禁用

端口被禁用（状态更改为 `Disabled` 或 `Not Started ×`）时：

1. 用户手动禁用它（E2E 测试中通常不这样做）
2. TUI 进程退出
3. 配置被 `Ctrl+Esc` 丢弃

**重要**：仅按 Escape (Esc) **不会**启用端口。您**必须**使用 `Ctrl + S` 保存配置并触发端口启用。`Ctrl + S` 后，您可以按 `Esc` 返回上一页（ConfigPanel）。

#### 验证最佳实践

始终在 Ctrl + S **之后**验证端口状态，同时仍在 Modbus 面板中：

```rust
// 保存配置
execute_cursor_actions(&mut session, &mut cap, &save_actions, "save_config").await?;

// 立即验证（状态指示器在 Modbus 面板标题栏中可见）
let status = verify_port_enabled(&mut session, &mut cap, "verify_after_save").await?;
// 状态应为 "Running ●" 或 "Applied ✔"
```

### 多站点配置工作流

配置多个 Modbus 站点时，请遵循两阶段方法：

#### 阶段 1：站点创建

在配置任何站点之前先创建所有站点：

```ignore
// 在"Create Station"上按 Enter station_count 次，在迭代之间重置为 Ctrl+PgUp。
// 创建后，通过 CursorAction::MatchPattern 匹配由井号后跟 station_count 形成的文字字符串来确认最终站点。
```

**连接模式配置：**

创建站点后，按一次向下箭头移动到"Connection Mode"字段。TUI 默认为**Master**模式：

- 如果需要 Master 模式：无需操作（已在默认值）
- 如果需要 Slave 模式：按 `Enter`、`Left`、`Enter` 从 Master 切换到 Slave

**注意**：中文需求中可能存在一些歧义，建议按 `Right` 切换到 Master 模式，但代码检查和现有测试确认默认值为 Master，按 `Left` 切换到 Slave。

#### 阶段 2：站点配置

使用绝对定位逐个配置每个站点：

```rust
for (i, station_config) in station_configs.iter().enumerate() {
    let station_number = i + 1; // 1-索引

    // 使用 Ctrl+PgUp + PgDown 导航到站点
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    for _ in 0..=i {
        actions.push(CursorAction::PressPageDown);
    }

    // 配置站点 ID
    actions.extend(vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,     // 全选
        CursorAction::PressBackspace, // 清除
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
    ]);

    // 配置寄存器类型（字段 2，使用 Down count: 2）
    // ... 其他字段的类似模式 ...
}
```

**关键点：**

- 在导航到站点之前，始终使用 `Ctrl+PgUp` 重置到顶部
- 使用 `PgDown` 跳转到站点部分（从顶部每个站点一个 PgDown）
- 在站点内使用 `Down` 箭头键在字段之间导航
- 配置所有站点后，使用 `Ctrl + S` 一次保存所有配置并启用端口
- 在每个站点配置结束时使用 `Ctrl+PgUp` 返回顶部（确保一致状态）

### 寄存器值配置工作流

配置站点字段（ID、类型、地址、计数）后，您可以选择配置单个寄存器值：

#### 详细的分步过程

1. **导航到第一个寄存器**：设置寄存器长度并确认后，光标移动到寄存器网格区域
2. **对于每个寄存器**：
   - 如果寄存器不需要值：按 `Right` 跳到下一个寄存器
   - 如果寄存器需要值：
     - 按 `Enter` 进入编辑模式
     - 输入十六进制值（不带 0x 前缀）
     - 按 `Enter` 确认
     - **验证状态树中的值**：使用 `CheckStatus` 操作验证值已写入全局状态
     - 按 `Right` 移动到下一个寄存器
3. **配置所有寄存器后**：按 `Ctrl+PgUp` 返回顶部

#### 状态验证路径格式

对于主站点：

```text
ports[0].modbus_masters[station_index].registers[register_index]
```

对于从站点：

```text
ports[0].modbus_slaves[station_index].registers[register_index]
```

#### 重要注意事项

- **站点索引**：0-基础（站点 1 的索引为 0）
- **寄存器索引**：0-基础（第一个寄存器的索引为 0）
- **值格式**：不带 0x 前缀的十六进制（例如 "1234" 而不是 "0x1234"）
- **状态验证**：在继续之前始终验证关键值已提交到状态树
- **干净状态**：在运行测试之前删除 JSON 缓存文件（`rm -f ~/.config/aoba/*.json`），以避免来自之前测试运行的干扰

### 最佳实践

#### 何时使用 UI 测试 vs 状态监控

**使用 UI 测试（终端捕获）用于：**

- 验证 UI 渲染和布局
- 检查视觉指示器（状态符号、颜色）
- 验证编辑模式括号和格式
- 测试键盘导航和光标移动

**使用状态监控用于：**

- 验证端口状态（启用/禁用）
- 检查 modbus 配置（站点、寄存器）
- 等待状态转换
- 验证通信日志
- 测试多进程场景

#### 结合两种方法

对于全面测试，结合两种方法：

```rust
// 1. 使用 UI 测试进行配置
execute_cursor_actions(&mut session, &mut cap, &actions, "configure").await?;

// 2. 使用状态监控进行验证
wait_for_port_enabled("/tmp/vcom1", 10, None).await?;

// 3. 使用 UI 测试验证视觉反馈
let screen = cap.capture(&mut session, "after_enable").await?;
assert!(screen.contains("●")); // 绿色圆点指示器
```

#### 超时和重试配置

默认重试间隔为 500ms。根据预期操作持续时间进行调整：

```rust
// 快速操作（页面导航）
wait_for_tui_page("Entry", 5, Some(200)).await?;

// 慢速操作（端口初始化）
wait_for_port_enabled("/tmp/vcom1", 30, Some(1000)).await?;
```

### 迁移指南

#### 旧方法（仅终端捕获）

```rust
// 旧：等待终端内容出现
let screen = cap.capture(&mut session, "after_enable").await?;
assert!(screen.contains("Enable Port: Yes"));
```

#### 新方法（状态监控）

```rust
// 新：等待状态反映变化
wait_for_port_enabled("/tmp/vcom1", 10, None).await?;
let status = read_tui_status()?;
assert!(status.ports.iter().any(|p| p.name == "/tmp/vcom1" && p.enabled));
```

#### 新方法的优势

1. **可靠性**：状态监控不受终端渲染时序影响
2. **精确性**：直接访问应用程序状态，而不是视觉表示
3. **速度**：无需等待 UI 刷新周期
4. **可调试性**：JSON 转储可以独立检查
5. **简单性**：对结构化数据的清晰断言，而不是文本匹配

### 调试 TUI E2E 测试

**重要原则**：UI 和逻辑测试的分离并不意味着放弃终端模拟。终端对于调试仍然至关重要。

#### 何时使用 DebugBreakpoint

虽然 TUI E2E 测试主要使用状态监控（CheckStatus）进行验证，但终端捕获对于调试仍然至关重要：

1. **开发期间**：插入 `DebugBreakpoint` 操作以捕获和检查出现问题时的当前终端状态
2. **故障排除失败**：如果 `CheckStatus` 断言失败，在它之前添加断点以查看 UI 实际显示的内容
3. **验证 UI 状态**：使用断点确认 TUI 在执行操作之前处于预期状态

**示例用法：**

```rust
let actions = vec![
    // 导航到端口
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
    CursorAction::Sleep { ms: 500 },

    // 调试：检查终端显示的内容
    CursorAction::DebugBreakpoint {
        description: "verify_port_selection".to_string(),
    },

    // 然后通过状态监控验证
    CursorAction::CheckStatus {
        description: "Port should be selected".to_string(),
        path: "current_selection".to_string(),
        expected: json!("/tmp/vcom1"),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    },
];
```

**关键点**：不要仅使用状态检查进行"盲目"调试。使用 `DebugBreakpoint` 直观确认 UI 行为，然后在理解正在发生的事情后添加适当的 `CheckStatus` 断言。

### 菜单导航时序和竞争条件

#### 理解 TUI 架构

TUI 使用多线程架构，可能在 E2E 测试中导致时序问题：

1. **输入线程**：同步捕获键盘事件并更新全局状态
2. **核心线程**：处理子进程管理，每 ~50ms 轮询 UI 消息
3. **渲染线程**：基于全局状态绘制 UI，以 100ms 超时轮询

当发生菜单操作（如在"Enter Business Configuration"上按 Enter）时：

1. 输入处理器立即更新状态（`Page::ConfigPanel` → `Page::ModbusDashboard`）
2. 通过通道向渲染线程发送 `Refresh` 消息
3. 渲染线程在下一次轮询周期处理消息（最多 100ms 延迟）
4. 使用新页面内容重绘终端

**关键问题**：使用终端捕获的 E2E 测试可能会在渲染线程完成绘制周期之前看到过时内容。

#### 菜单导航最佳实践

**要这样做**：对页面转换使用状态树验证

```rust
// 导航到菜单项并按 Enter
execute_cursor_actions(&mut session, &mut cap, &actions, "press_enter").await?;

// 等待状态树反映页面更改
wait_for_tui_page("ModbusDashboard", 5, Some(300)).await?;

// 现在安全地验证终端内容
let screen = cap.capture(&mut session, "after_navigation").await?;
assert!(screen.contains("ModBus Master/Slave Set"));
```

**不要这样做**：仅依赖导航后的终端模式匹配

```rust
// ❌ 错误：可能在渲染完成之前捕获
let actions = vec![
    CursorAction::PressEnter,
    CursorAction::Sleep { ms: 1000 },
    CursorAction::MatchPattern {
        pattern: Regex::new(r"ModBus Master/Slave Set")?,
        // ... 可能间歇性失败
    },
];
```

#### 实现健壮的菜单导航

对于可靠的菜单导航 E2E 测试，使用多尝试策略与状态验证：

```rust
pub async fn enter_menu_with_retry<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    menu_item: &str,
    expected_page: &str,
    max_attempts: usize,
) -> Result<()> {
    for attempt in 1..=max_attempts {
        // 导航并按 Enter
        navigate_to_menu_item(session, cap, menu_item).await?;

        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 1000 },
        ];
        execute_cursor_actions(session, cap, &actions, "press_enter").await?;

        // 等待状态树更新
        match wait_for_tui_page(expected_page, 3, Some(300)).await {
            Ok(()) => {
                // 验证终端也更新了
                tokio::time::sleep(Duration::from_millis(500)).await;
                let screen = cap.capture(session, "verify").await?;
                if screen.contains(expected_page) {
                    return Ok(());
                }
            }
            Err(_) if attempt < max_attempts => {
                log::warn!("尝试 {} 失败，重试中...", attempt);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(anyhow!("{} 次尝试后失败", max_attempts))
}
```

#### 同步点

始终在这些同步点使用状态树验证：

- **页面导航**：在菜单项上按 Enter 后
- **端口启用/禁用**：切换端口状态后
- **配置保存**：按 Ctrl + S 后
- **站点创建**：创建新的 Modbus 站点后

这确保 TUI 的内部状态在继续后续操作之前完全更新。

### 故障排除

#### 找不到状态文件

确保通过传递 `--debug-ci-e2e-test` 标志启用调试模式来生成 TUI 或 CLI 进程：

```rust
spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
```

#### 状态文件未更新

检查状态转储线程是否正在运行。查找日志消息：

```text
Started status dump thread, writing to /tmp/ci_tui_status.json
```

#### 等待状态超时

- 增加超时值
- 如果文件 I/O 较慢，则增加重试间隔
- 检查预期状态是否实际可达
- 手动检查 `/tmp/tui_e2e_status.json` 以查看当前状态

#### 间歇性菜单导航失败

如果菜单导航（例如"Enter Business Configuration"）间歇性失败：

- **根本原因**：状态更新与终端渲染之间的竞争条件
- **解决方案**：使用多尝试重试与状态树验证（参见"菜单导航时序"部分）
- **实现**：`enter_modbus_panel` 函数现在包括：
  - 最多 10 次重试尝试，间隔 1 秒
  - 启用调试模式时的状态树轮询或终端验证回退
  - 失败时自动重新导航
- **预防**：始终在检查终端内容之前通过状态树验证页面更改
- **调试**：添加 DebugBreakpoint 操作以查看故障期间的实际终端状态

#### 多站点配置问题

配置多个 Modbus 站点时：

- **导航**：使用 `Ctrl+PgUp` 重置到顶部，然后 `PgDown` 跳转到特定站点
- **时序**：在 Enter 退出编辑模式后允许足够的延迟（Enter 后 1000-2000ms）
- **验证**：在按 Ctrl + S 保存之前检查每个站点的配置
- **已知问题**：PgDown 导航可能无法正确定位所有站点；使用调试断点验证

## E2E 测试矩阵结构

### 测试矩阵概述

E2E 测试套件组织成涵盖 CLI 和 TUI 模式以及各种寄存器类型和站点配置的综合矩阵。所有测试遵循独立原则 - 每个测试都是独立的单元，不应与其他测试组合。

### 测试组织

#### CLI E2E 测试（`examples/cli_e2e`）

**单站点测试**（`e2e/single_station/register_modes.rs`）

- 通过 stdio 管道使用主/从通信测试所有 4 种 Modbus 寄存器模式
- 模式：01 线圈、02 离散输入（可写）、03 保持、04 输入（可写）
- 地址范围：0x0000-0x0030（间隔 0x0010）
- 模式 02 和 04 的双向写入测试

**多站点测试**（`e2e/multi_station/two_stations.rs`）

- 测试具有各种场景的 2 站点配置
- 混合寄存器类型（线圈 + 保持）
- 间隔地址（0x0000 和 0x00A0）
- 混合站点 ID（1 和 5）

#### TUI E2E 测试（`examples/tui_e2e`）

**单站点主模式**（`e2e/single_station/master_modes.rs`）

- TUI 充当 Modbus 主，CLI 充当从
- 测试所有 4 种寄存器模式
- 包括遵循 CLAUDE.md 工作流的 `configure_tui_station` 助手
- 完整状态监控和验证

**单站点从模式**（`e2e/single_station/slave_modes.rs`）

- TUI 充当 Modbus 从，CLI 充当主
- 测试所有 4 种寄存器模式
- 可写模式的双向写入测试

**多站点主模式**（`e2e/multi_station/master_modes.rs`）

- TUI 主与 2 个站点
- 混合类型、间隔地址、混合 ID

**多站点从模式**（`e2e/multi_station/slave_modes.rs`）

- TUI 从与 2 个站点
- 混合类型（WritableCoils + WritableRegisters）
- 间隔地址，混合 ID（2 和 6）

### 运行测试

```bash
# CLI 单站点测试
cargo run --package cli_e2e -- --module modbus_single_station_coils
cargo run --package cli_e2e -- --module modbus_single_station_discrete_inputs
cargo run --package cli_e2e -- --module modbus_single_station_holding
cargo run --package cli_e2e -- --module modbus_single_station_input

# CLI 多站点测试
cargo run --package cli_e2e -- --module modbus_multi_station_mixed_types
cargo run --package cli_e2e -- --module modbus_multi_station_spaced_addresses
cargo run --package cli_e2e -- --module modbus_multi_station_mixed_ids

# TUI 单站点主测试
cargo run --package tui_e2e -- --module tui_master_coils
cargo run --package tui_e2e -- --module tui_master_discrete_inputs
cargo run --package tui_e2e -- --module tui_master_holding
cargo run --package tui_e2e -- --module tui_master_input

# TUI 单站点从测试
cargo run --package tui_e2e -- --module tui_slave_coils
cargo run --package tui_e2e -- --module tui_slave_discrete_inputs
cargo run --package tui_e2e -- --module tui_slave_holding
cargo run --package tui_e2e -- --module tui_slave_input

# TUI 多站点主测试
cargo run --package tui_e2e -- --module tui_multi_master_mixed_types
cargo run --package tui_e2e -- --module tui_multi_master_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_master_mixed_ids

# TUI 多站点从测试
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_types
cargo run --package tui_e2e -- --module tui_multi_slave_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_ids
```

### 测试实现指南

1. **站点配置工作流**（TUI 测试）
   - 遵循 CLAUDE.md 中的分步过程
   - 为一致性使用 `configure_tui_station` 助手
   - 配置步骤后始终验证状态树

2. **寄存器值配置**
   - 使用十六进制格式设置单个寄存器值（不带 0x 前缀）
   - 使用 `CheckStatus` 操作验证每个值
   - 配置后使用 `Ctrl+PgUp` 返回顶部

3. **端口启用机制**
   - 使用 `Ctrl + S` 保存配置时自动启用端口
   - 保存后状态从 `Disabled` → `Running`
   - `Ctrl + S` 后至少等待 5 秒以进行端口稳定

4. **数据验证**
   - CLI 测试：使用 stdio 管道和 JSON 解析
   - TUI 测试：结合状态监控与 CLI 从/主验证
   - 始终验证可写模式的双向通信

5. **干净状态**
   - 每个测试前删除 TUI 配置缓存：`~/.config/aoba/*.json`
   - 清理调试状态文件：`/tmp/ci_tui_status.json`、`/tmp/ci_cli_*_status.json`
   - 运行 `socat_init.sh` 以重置虚拟串行端口（如需要）
