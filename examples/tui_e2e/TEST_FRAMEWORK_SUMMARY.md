# TUI E2E 测试框架总结

本文档总结了 TUI E2E 测试的完整测试流程、核心工具和测试方法论，为后续重构提供参考。

## 一、测试架构概览

### 1.1 测试目录结构

```
tui_e2e/
├── src/
│   ├── main.rs              # 测试入口，模块调度
│   └── e2e/
│       ├── mod.rs           # 模块导出
│       ├── single_station/  # 单站测试
│       │   ├── mod.rs
│       │   ├── master_modes.rs  # Master 模式 4 个测试
│       │   └── slave_modes.rs   # Slave 模式 4 个测试
│       └── multi_station/   # 多站测试（2 站）
│           ├── mod.rs
│           ├── master_modes.rs  # Master 模式 3 个测试
│           └── slave_modes.rs   # Slave 模式 3 个测试
```

### 1.2 测试模块清单

**单站 Master 模式测试** (4 个):

1. `tui_master_coils` - 01 线圈寄存器
2. `tui_master_discrete_inputs` - 02 可写线圈寄存器
3. `tui_master_holding` - 03 保持寄存器
4. `tui_master_input` - 04 可写寄存器

**单站 Slave 模式测试** (4 个):

1. `tui_slave_coils` - 01 线圈寄存器
2. `tui_slave_discrete_inputs` - 02 可写线圈寄存器
3. `tui_slave_holding` - 03 保持寄存器
4. `tui_slave_input` - 04 可写寄存器

**多站 Master 模式测试** (3 个):

1. `tui_multi_master_mixed_types` - 混合寄存器类型（站1 Coils + 站2 Holding）
2. `tui_multi_master_spaced_addresses` - 地址间隔配置
3. `tui_multi_master_mixed_ids` - 混合站号（不同 station_id）

**多站 Slave 模式测试** (3 个):

1. `tui_multi_slave_mixed_types` - 混合寄存器类型
2. `tui_multi_slave_spaced_addresses` - 地址间隔配置
3. `tui_multi_slave_mixed_ids` - 混合站号

**总计**: 14 个测试模块

---

## 二、通用测试流程

### 2.1 测试前准备（在 `main.rs` 中）

```rust
// 1. 清理 TUI 配置缓存
cleanup_tui_config_cache()?;
// 删除以下文件：
// - aoba_tui_config.json
// - /tmp/aoba_tui_config.json
// - ~/.config/aoba/aoba_tui_config.json
// - /tmp/ci_tui_status.json
// - /tmp/ci_cli_vcom1_status.json
// - /tmp/ci_cli_vcom2_status.json

// 2. 设置虚拟串口（可选，通过 socat_init.sh）
setup_virtual_serial_ports()?;
```

### 2.2 核心测试流程（以单站 Master Coils 为例）

```rust
async fn test_tui_master_coils(port1: &str, port2: &str) -> Result<()> {
    // === 阶段 0: 准备工作 ===
    // 0.1 验证虚拟串口存在
    port_exists(port1) && port_exists(port2)?;
    
    // 0.2 生成测试数据
    let test_data = generate_random_coils(10);
    
    // === 阶段 1: 启动 TUI 进程 ===
    // 1.1 以调试模式启动 TUI
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);
    sleep_seconds(3).await;
    
    // === 阶段 2: 导航到配置面板 ===
    // 2.1 等待 TUI 到达 Entry 页面
    CheckStatus { path: "page.type", expected: json!("Entry") };
    
    // 2.2 导航到 ConfigPanel
    PressEnter → CheckStatus { path: "page.type", expected: json!("ConfigPanel") };
    
    // 2.3 进入 Modbus 面板
    enter_modbus_panel();
    
    // 2.4 验证端口初始状态（未启用）
    CheckStatus { path: "ports[0].enabled", expected: json!(false) };
    
    // === 阶段 3: 配置站点 ===
    configure_tui_station(
        session, cap,
        station_id: 1,
        register_mode: "coils",
        start_address: 0x0000,
        register_count: 10,
        register_values: None  // 或 Some(&[...])
    );
    // 内部流程详见 2.3 节
    
    // === 阶段 4: 保存配置并启用端口 ===
    // 4.1 Ctrl+S 保存配置（自动触发端口启用）
    PressCtrlS → sleep_seconds(5);
    
    // 4.2 验证 CLI 子进程已启动
    // 检查 /tmp/ci_cli_vcom1_status.json 文件是否存在
    
    // === 阶段 5: 数据验证 ===
    // 5.1 Master 模式：启动 CLI Slave 读取数据
    Command::new("aoba")
        .args(["--slave-poll", port2, "--station-id", "1", ...])
        .output()?;
    
    // 5.2 Slave 模式：启动 CLI Master 写入数据
    // Command::new("aoba")
    //     .args(["--master-provide", port2, "--data-source", ...])
    //     .output()?;
    
    // 5.3 解析 JSON 响应并验证数据
    let received_values = parse_json_response(output);
    assert_eq!(test_data, received_values);
    
    // === 阶段 6: 清理 ===
    drop(tui_session);
    
    Ok(())
}
```

### 2.3 配置站点的详细流程

```rust
async fn configure_tui_station(
    session, cap,
    station_id: u8,
    register_mode: &str,  // "coils", "discrete_inputs", "holding", "input"
    start_address: u16,
    register_count: u16,
    register_values: Option<&[u16]>
) -> Result<()> {
    // === 步骤 1: 创建站点 ===
    PressEnter → sleep(2s);
    MatchPattern { pattern: r"#1(?:\D|$)" };  // 验证站点 #1 已创建
    
    // === 步骤 2: 配置连接模式 ===
    // Master 模式：默认，无需操作
    // Slave 模式：PressEnter → ArrowLeft(1) → PressEnter
    
    // === 步骤 3: 移动到站点 ===
    PressCtrlPageUp → PressPageDown;
    
    // === 步骤 4: 配置字段 ===
    // 4.1 配置 Station ID（字段 0）
    PressEnter → PressCtrlA → PressBackspace
        → TypeString(station_id) → PressEnter
        → ArrowDown(1);
    
    // 4.2 配置 Register Type（字段 1）
    // 默认: Holding (索引 2)
    // - "coils":           PressEnter → ArrowLeft(2) → PressEnter
    // - "discrete_inputs": PressEnter → ArrowLeft(1) → PressEnter
    // - "holding":         无操作（默认值）
    // - "input":           PressEnter → ArrowRight(1) → PressEnter
    → CheckStatus { path: "ports[0].modbus_masters[0].register_type", expected: ... }
    → ArrowDown(1);
    
    // 4.3 配置 Start Address（字段 2）
    PressEnter → PressCtrlA → PressBackspace
        → TypeString(format!("{:x}", start_address))
        → PressEnter → ArrowDown(1);
    
    // 4.4 配置 Register Count（字段 3）
    PressEnter → sleep(1s) → PressCtrlA → PressBackspace
        → TypeString(register_count) → PressEnter
        → sleep(5s);  // 关键：等待值提交到状态树
    CheckStatus { 
        path: "ports[0].modbus_masters[0].register_count",
        expected: json!(register_count),
        timeout: 15s
    };
    
    // 4.5 配置寄存器值（可选）
    if let Some(values) = register_values {
        ArrowDown(1);  // 进入寄存器网格
        for (i, value) in values.iter().enumerate() {
            PressEnter → TypeString(format!("{:x}", value))
                → PressEnter → sleep(500ms);
            if i < values.len() - 1 {
                ArrowRight(1);
            }
        }
    }
    
    // === 步骤 5: 移动到安全位置 ===
    PressCtrlPageUp → sleep(500ms);
    
    // === 步骤 6: 保存并启用 ===
    sleep(1s) → PressCtrlS → sleep(5s);
    // Ctrl+S 自动调用 ToggleRuntime 启用端口
    
    Ok(())
}
```

### 2.4 多站配置流程差异

```rust
async fn configure_multiple_tui_master_stations(
    session, cap,
    stations: &[(u8, &str, u16, u16, Option<Vec<u16>>)]
) -> Result<()> {
    // === 阶段 1: 批量创建站点 ===
    for i in 0..stations.len() {
        PressEnter → sleep(1s) → PressCtrlPageUp;
    }
    
    // === 阶段 2: 逐个配置站点 ===
    for (i, station_config) in stations.iter().enumerate() {
        // 2.1 导航到站点 i
        PressCtrlPageUp;
        for _ in 0..=i {
            PressPageDown;
        }
        
        // 2.2 配置站点（同单站流程）
        // ... (同 configure_tui_station 的步骤 4)
        
        // 2.3 返回顶部
        PressCtrlPageUp;
    }
    
    Ok(())
}
```

---

## 三、核心工具与技术

### 3.1 仿真终端操作 (ci_utils)

#### 3.1.1 核心枚举：`CursorAction`

```rust
pub enum CursorAction {
    // 基础按键
    PressArrow { direction: ArrowKey, count: usize },
    PressEnter,
    PressEscape,
    PressTab,
    CtrlC,
    PressCtrlS,
    PressCtrlA,
    PressBackspace,
    PressPageUp,
    PressPageDown,
    PressCtrlPageUp,
    PressCtrlPageDown,
    
    // 输入操作
    TypeChar(char),
    TypeString(String),
    
    // 时序控制
    Sleep { ms: u64 },
    
    // 模式匹配验证
    MatchPattern {
        pattern: Regex,
        description: String,
        line_range: Option<(usize, usize)>,   // 行范围（0 索引）
        col_range: Option<(usize, usize)>,    // 列范围（0 索引）
        retry_action: Option<Vec<CursorAction>>,  // 失败后重试动作
    },
    
    // 调试断点
    DebugBreakpoint { description: String },
    
    // 状态检查（核心功能）
    CheckStatus {
        description: String,
        path: String,                          // JSON 路径，如 "ports[0].enabled"
        expected: Value,                       // 期望值（serde_json::Value）
        timeout_secs: Option<u64>,             // 超时（默认 10s）
        retry_interval_ms: Option<u64>,        // 重试间隔（默认 500ms）
    },
}
```

#### 3.1.2 执行引擎：`execute_cursor_actions`

```rust
pub async fn execute_cursor_actions<T: Expect>(
    session: &mut T,          // expect 会话（TUI 进程）
    cap: &mut TerminalCapture,  // 终端截图工具
    actions: &[CursorAction],   // 动作序列
    session_name: &str          // 会话名称（用于日志）
) -> Result<()>
```

**特性**:

- 顺序执行动作列表
- `MatchPattern` 支持嵌套重试（内循环 3 次 × 外循环 3 次 = 总共 9 次尝试）
- `CheckStatus` 通过读取 `/tmp/ci_tui_status.json` 验证状态
- 失败时自动捕获屏幕截图并保存到 `/tmp/tui_e2e_debug/`

### 3.2 全局状态监控

#### 3.2.1 状态文件位置

**TUI 状态**:

- 路径: `/tmp/ci_tui_status.json`
- 启用: `--debug-ci-e2e-test` 标志
- 更新频率: 实时（每次状态变更）

**CLI 状态**:

- 路径: `/tmp/ci_cli_{port_basename}_status.json`
- 示例: `/tmp/ci_cli_vcom1_status.json`
- 启用: TUI 子进程自动生成

#### 3.2.2 状态结构（简化版）

```rust
// TUI 状态
struct TuiStatus {
    ports: Vec<TuiPort>,
    page: TuiPage,  // Entry, ConfigPanel, ModbusDashboard, LogPanel, About
    timestamp: String,
}

struct TuiPort {
    name: String,
    enabled: bool,
    state: PortState,  // Free, OccupiedByThis, OccupiedByOther
    modbus_masters: Vec<TuiModbusMaster>,
    modbus_slaves: Vec<TuiModbusSlave>,
    log_count: usize,
}

struct TuiModbusMaster {
    station_id: u8,
    register_type: String,
    start_address: u16,
    register_count: usize,
}

// CLI 状态
struct CliStatus {
    port_name: String,
    station_id: u8,
    register_mode: RegisterMode,
    register_address: u16,
    register_length: u16,
    mode: CliMode,  // SlaveListen, SlavePoll, MasterProvide
    timestamp: String,
}
```

#### 3.2.3 状态读取工具

```rust
// 读取 TUI 状态
pub fn read_tui_status() -> Result<TuiStatus>;

// 读取 CLI 状态
pub fn read_cli_status(port: &str) -> Result<CliStatus>;

// 等待 TUI 到达指定页面
pub async fn wait_for_tui_page(
    expected_page: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>
) -> Result<TuiStatus>;

// 等待端口启用
pub async fn wait_for_port_enabled(
    port_name: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>
) -> Result<TuiStatus>;

// 通用 JSON 路径检查
pub async fn check_status_value(
    path: &str,
    expected: &Value,
    timeout_secs: u64,
    retry_interval_ms: u64
) -> Result<Value>;
```

### 3.3 终端截图与日志

#### 3.3.1 终端捕获

```rust
pub struct TerminalCapture {
    session_count: usize,
    size: TerminalSize,  // Small (80×24), Medium (120×40), Large (160×50)
}

impl TerminalCapture {
    // 捕获当前终端内容
    pub async fn capture_with_logging(
        &mut self,
        session: &mut impl Expect,
        session_name: &str,
        log_content: bool  // 是否打印内容到日志
    ) -> Result<String>;
    
    // 保存调试截图
    pub fn save_debug_snapshot(
        &self,
        screen: &str,
        name: &str
    ) -> Result<()>;
}
```

**保存位置**: `/tmp/tui_e2e_debug/{name}_{timestamp}.txt`

#### 3.3.2 日志解析（未充分使用）

```rust
// 位于 ci_utils/src/log_parser.rs
// 目前测试中未大量使用日志解析，主要依赖状态文件
```

### 3.4 辅助工具

```rust
// 端口工具 (ports.rs)
pub fn port_exists(port: &str) -> bool;
pub fn vcom_matchers_with_ports(port1: &str, port2: &str) -> VcomMatchers;

// 数据生成 (data.rs)
pub fn generate_random_coils(count: usize) -> Vec<u16>;
pub fn generate_random_registers(count: usize) -> Vec<u16>;

// TUI 导航 (tui.rs)
pub async fn enter_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture
) -> Result<()>;

// 进程管理 (terminal.rs)
pub fn spawn_expect_process(args: &[&str]) -> Result<impl Expect>;
pub fn build_debug_bin(name: &str) -> Result<PathBuf>;

// 时间工具 (helpers.rs)
pub async fn sleep_seconds(secs: u64);
```

---

## 四、关键技术要点

### 4.1 时序控制

**关键时序点**:

1. **启动等待**: TUI 启动后等待 3 秒
2. **字段编辑**: 进入编辑模式后等待 500ms-1s
3. **值提交**: 按 Enter 后等待 1-5 秒（Register Count 需要 5 秒）
4. **状态同步**: Ctrl+S 后等待 5 秒
5. **进程通信**: CLI 命令后等待 2 秒

**原因**: TUI 状态更新是异步的，需要足够时间让数据同步到状态文件。

### 4.2 状态验证策略

**优先级顺序**:

1. **CheckStatus**: 最可靠，直接读取进程状态
2. **MatchPattern**: 屏幕内容匹配（可能受渲染影响）
3. **文件存在性**: 检查 `/tmp/ci_cli_*.json`（子进程启动验证）

### 4.3 错误处理

**失败时的调试流程**:

1. 自动捕获屏幕截图
2. 保存到 `/tmp/tui_e2e_debug/`
3. 打印最后的屏幕内容
4. 返回详细错误信息

### 4.4 测试隔离

**每个测试前的清理**:

- 删除 TUI 配置缓存 (`aoba_tui_config.json`)
- 删除旧的状态文件 (`/tmp/ci_*_status.json`)
- 重置虚拟串口（可选）

---

## 五、测试数据流

### 5.1 Master 模式数据流

```
TUI Master (port1) ←→ CLI Slave (port2)
      ↓ 配置                  ↓
   register_values     读取并验证
      ↓                       ↓
   内部存储              JSON 响应
      ↓                       ↓
   Modbus 协议 ——————→   解析验证
```

### 5.2 Slave 模式数据流

```
CLI Master (port2) ←→ TUI Slave (port1)
      ↓ 写入                  ↓
   test_data          接收并存储
      ↓                       ↓
   Modbus 协议 ——————→   register 数组
      ↓                       ↓
   命令完成         CheckStatus 验证
                   (ports[0].modbus_slaves[0].registers)
```

---

## 六、已知的测试模式

### 6.1 寄存器类型覆盖

- **Coils** (0x01): 布尔值数组
- **Discrete Inputs** (0x02): 只读布尔值
- **Holding Registers** (0x03): 可读写 16 位寄存器
- **Input Registers** (0x04): 只读 16 位寄存器

### 6.2 地址测试覆盖

- **起始地址**: 0x0000, 0x0010, 0x0020, 0x0030
- **地址间隔**: 测试非连续地址（多站场景）
- **数据长度**: 主要测试 10 个寄存器

### 6.3 多站场景覆盖

- **混合寄存器类型**: 不同站点使用不同类型
- **混合站号**: 不同 station_id
- **地址间隔**: 测试地址空间不重叠

---

## 七、改进建议（为重构做准备）

### 7.1 代码复用

**问题**: 大量重复代码（单站/多站、Master/Slave 流程相似）

**建议**:

- 提取通用流程函数
- 使用配置驱动测试（JSON/TOML 描述测试场景）
- 创建 DSL 简化测试编写

### 7.2 时序可靠性

**问题**: 硬编码的 sleep 时间可能不够稳定

**建议**:

- 更多使用 `CheckStatus` 而非 `sleep`
- 实现自动重试机制
- 添加可配置的超时时间

### 7.3 日志与诊断

**问题**: 失败时难以定位问题

**建议**:

- 增强日志输出
- 捕获更多中间状态
- 添加性能监控（测试执行时间）

### 7.4 测试覆盖

**问题**: 某些边界场景未覆盖

**建议**:

- 添加错误场景测试（无效配置、通信失败）
- 添加并发测试（多端口同时操作）
- 添加压力测试（大量寄存器、长时间运行）

---

## 八、重要文件参考

### 8.1 测试相关

- `examples/tui_e2e/src/main.rs` - 测试入口
- `examples/tui_e2e/src/e2e/**/*.rs` - 测试实现

### 8.2 工具库

- `examples/ci_utils/src/auto_cursor.rs` - 自动化操作
- `examples/ci_utils/src/status_monitor.rs` - 状态监控
- `examples/ci_utils/src/snapshot.rs` - 终端捕获
- `examples/ci_utils/src/tui.rs` - TUI 导航辅助
- `examples/ci_utils/src/terminal.rs` - 进程管理

### 8.3 状态定义

- `src/protocol/status/` - 状态结构定义（主程序）
- `examples/ci_utils/src/status_monitor.rs` - 状态结构定义（测试侧）

---

## 九、总结

TUI E2E 测试框架通过以下核心技术实现了自动化集成测试：

1. **仿真终端操作**: 通过 `expect` 库和 `CursorAction` 枚举模拟用户输入
2. **全局状态监控**: 通过读取 `/tmp/ci_*_status.json` 文件验证程序状态
3. **终端截图**: 捕获终端内容用于调试和模式匹配
4. **模块化测试**: 14 个独立测试模块覆盖不同场景
5. **数据验证**: 通过 CLI 工具验证 Modbus 通信的正确性

**核心优势**:

- 端到端测试，覆盖真实使用场景
- 自动化程度高，无需人工干预
- 状态监控机制提供可靠的验证手段

**待改进点**:

- 代码复用率低，存在大量重复
- 时序控制依赖硬编码 sleep
- 错误诊断能力有待增强

本文档将作为后续测试重构的基础参考。
