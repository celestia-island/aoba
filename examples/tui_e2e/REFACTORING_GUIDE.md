# TUI E2E 测试重构指南

## 当前状态

所有测试模块（14 个）已被清空，仅保留 `todo!` 宏占位符，以确保测试失败并提示后续实现。

### 已清空的测试模块

**单站 Master 模式** (`src/e2e/single_station/master_modes.rs`):

- ✅ `test_tui_master_coils`
- ✅ `test_tui_master_discrete_inputs`
- ✅ `test_tui_master_holding_registers`
- ✅ `test_tui_master_input_registers`

**单站 Slave 模式** (`src/e2e/single_station/slave_modes.rs`):

- ✅ `test_tui_slave_coils`
- ✅ `test_tui_slave_discrete_inputs`
- ✅ `test_tui_slave_holding_registers`
- ✅ `test_tui_slave_input_registers`

**多站 Master 模式** (`src/e2e/multi_station/master_modes.rs`):

- ✅ `test_tui_multi_master_mixed_register_types`
- ✅ `test_tui_multi_master_spaced_addresses`
- ✅ `test_tui_multi_master_mixed_station_ids`

**多站 Slave 模式** (`src/e2e/multi_station/slave_modes.rs`):

- ✅ `test_tui_multi_slave_mixed_register_types`
- ✅ `test_tui_multi_slave_spaced_addresses`
- ✅ `test_tui_multi_slave_mixed_station_ids`

---

## 重构参考资源

### 核心文档

1. **TEST_FRAMEWORK_SUMMARY.md** - 完整的测试框架总结
   - 包含所有现有测试的详细分析
   - 通用测试流程的完整描述
   - 核心工具和技术的使用指南
   - 数据流和验证策略

### 现有工具库 (`examples/ci_utils`)

**关键模块**:

- `auto_cursor.rs` - 自动化光标操作和验证
- `status_monitor.rs` - 全局状态监控和检查
- `snapshot.rs` - 终端截图和调试
- `tui.rs` - TUI 导航辅助函数
- `terminal.rs` - 进程管理和构建
- `data.rs` - 测试数据生成
- `ports.rs` - 串口工具

---

## 重构建议

### 阶段 1: 提取通用流程函数

创建 `src/e2e/common.rs` 模块，包含以下通用函数：

```rust
// 完整的测试设置流程
async fn setup_tui_test(
    port1: &str,
    port2: &str,
) -> Result<(ExpectSession, TerminalCapture)>;

// 通用站点配置函数
async fn configure_station(
    session: &mut ExpectSession,
    cap: &mut TerminalCapture,
    config: StationConfig,
) -> Result<()>;

// 数据验证流程（Master 模式）
async fn verify_master_data(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()>;

// 数据验证流程（Slave 模式）
async fn verify_slave_data(
    session: &mut ExpectSession,
    cap: &mut TerminalCapture,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()>;
```

### 阶段 2: 配置驱动测试

定义测试配置结构体：

```rust
#[derive(Debug, Clone)]
pub struct StationConfig {
    pub station_id: u8,
    pub register_mode: RegisterMode,
    pub start_address: u16,
    pub register_count: u16,
    pub connection_mode: ConnectionMode,
    pub register_values: Option<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub enum RegisterMode {
    Coils,
    DiscreteInputs,
    Holding,
    Input,
}

#[derive(Debug, Clone)]
pub enum ConnectionMode {
    Master,
    Slave,
}
```

使用配置简化测试：

```rust
pub async fn test_tui_master_coils(port1: &str, port2: &str) -> Result<()> {
    let config = StationConfig {
        station_id: 1,
        register_mode: RegisterMode::Coils,
        start_address: 0x0000,
        register_count: 10,
        connection_mode: ConnectionMode::Master,
        register_values: None,
    };
    
    run_single_station_test(port1, port2, config).await
}
```

### 阶段 3: 增强错误诊断

添加更详细的错误信息：

```rust
#[derive(Debug)]
pub struct TestFailure {
    pub test_name: String,
    pub phase: String,
    pub expected: String,
    pub actual: String,
    pub screenshot_path: Option<PathBuf>,
    pub status_dump: Option<Value>,
}
```

### 阶段 4: 改进时序控制

减少硬编码的 sleep，更多使用状态检查：

```rust
// 替代：sleep(5s)
// 改用：
wait_for_status(
    "ports[0].enabled",
    json!(true),
    timeout_secs: 10,
)?;
```

---

## 测试实现优先级

### 高优先级（核心功能）

1. ✅ 单站 Master Coils（最简单的场景）
2. ✅ 单站 Slave Coils（验证双向通信）
3. ✅ 单站 Master Holding（最常用的类型）

### 中优先级（扩展覆盖）

4. ✅ 单站 Master/Slave Discrete Inputs
5. ✅ 单站 Master/Slave Input Registers
6. ✅ 多站 Master/Slave 混合类型

### 低优先级（边界场景）

7. ✅ 多站地址间隔测试
8. ✅ 多站混合站号测试

---

## 验证清单

每个测试实现后应验证：

- [ ] 测试能独立运行（不依赖外部状态）
- [ ] 测试失败时提供清晰的错误信息
- [ ] 测试有适当的超时设置
- [ ] 测试清理了临时文件和进程
- [ ] 测试有充分的日志输出
- [ ] 测试截图在失败时自动保存

---

## 运行测试

```bash
# 运行单个模块
cargo run --package tui_e2e -- --module tui_master_coils

# 查看可用模块列表
cargo run --package tui_e2e

# 启用调试模式
cargo run --package tui_e2e -- --module tui_master_coils --debug

# 自定义端口
cargo run --package tui_e2e -- \
    --module tui_master_coils \
    --port1 /tmp/vcom1 \
    --port2 /tmp/vcom2
```

---

## 相关文档

- `TEST_FRAMEWORK_SUMMARY.md` - 完整的框架总结和技术细节
- `../CLAUDE.md` - 项目上下文和测试需求
- `../../docs/zh-chs/CLI_MODBUS.md` - CLI 使用文档

---

## 注意事项

### 测试隔离

- 每个测试前清理配置缓存 (`cleanup_tui_config_cache`)
- 删除旧的状态文件
- 确保端口未被占用

### 调试技巧

- 使用 `--debug` 标志启用调试模式
- 检查 `/tmp/tui_e2e_debug/` 目录中的截图
- 查看 `/tmp/ci_*_status.json` 文件了解实时状态
- 使用 `DebugBreakpoint` 动作暂停测试

### 常见陷阱

- ❌ 硬编码 sleep 时间可能不够
- ✅ 使用 `CheckStatus` 等待状态变化
- ❌ 忘记等待配置提交到状态树
- ✅ 在关键操作后使用 `CheckStatus` 验证
- ❌ 测试间共享状态
- ✅ 每个测试完全独立运行

---

## 下一步行动

1. 阅读 `TEST_FRAMEWORK_SUMMARY.md` 了解完整的测试框架
2. 创建 `src/e2e/common.rs` 模块实现通用函数
3. 按优先级重新实现测试模块
4. 添加新的测试场景（错误处理、并发等）
5. 优化测试性能和可靠性

---

**准备就绪！** 现在可以开始从头重构测试，构建更清晰、更可维护的测试框架。
