# TUI E2E 测试重构 - 快速参考

## 📚 文档导航

| 文档 | 用途 | 行数 |
|------|------|------|
| **TEST_FRAMEWORK_SUMMARY.md** | 完整的测试框架分析和技术细节 | ~655 |
| **REFACTORING_GUIDE.md** | 重构实施指南和最佳实践 | ~275 |
| **SUMMARY.md** | 工作总结和成果概览 | ~296 |

## 🎯 快速开始

### 1. 了解现有框架

```bash
# 阅读测试框架总结
cat TEST_FRAMEWORK_SUMMARY.md
```

**重点章节**:

- 第二章：通用测试流程（完整的测试步骤）
- 第三章：核心工具与技术（CursorAction、状态监控）
- 第四章：关键技术要点（时序控制、状态验证）

### 2. 规划重构

```bash
# 阅读重构指南
cat REFACTORING_GUIDE.md
```

**重点章节**:

- 阶段 1-4：渐进式重构计划
- 测试实现优先级：按优先级实现测试
- 注意事项：避免常见陷阱

### 3. 开始实现

#### Step 1: 创建通用模块

```bash
# 创建 common.rs
touch src/e2e/common.rs
```

在 `common.rs` 中实现：

```rust
// 配置结构体
pub struct StationConfig { ... }
pub enum RegisterMode { ... }
pub enum ConnectionMode { ... }

// 通用流程函数
pub async fn setup_tui_test(...) -> Result<...> { ... }
pub async fn configure_station(...) -> Result<()> { ... }
pub async fn verify_master_data(...) -> Result<()> { ... }
pub async fn verify_slave_data(...) -> Result<()> { ... }
```

#### Step 2: 实现第一个测试

```bash
# 编辑 master_modes.rs
vim src/e2e/single_station/master_modes.rs
```

替换 `todo!` 为实际实现：

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

#### Step 3: 测试和迭代

```bash
# 运行测试
cargo run --package tui_e2e -- --module tui_master_coils

# 启用调试模式
cargo run --package tui_e2e -- --module tui_master_coils --debug
```

## 🔧 核心工具速查

### CursorAction 常用操作

```rust
// 导航
PressArrow { direction: ArrowKey::Down, count: 1 }
PressCtrlPageUp  // 回到顶部
PressPageDown    // 下一个站点

// 编辑
PressEnter       // 进入编辑模式/确认
PressEscape      // 取消
PressCtrlA       // 全选
PressBackspace   // 删除
TypeString(s)    // 输入文本

// 控制
PressCtrlS       // 保存配置
Sleep { ms }     // 等待

// 验证
CheckStatus {    // 检查状态
    path: "ports[0].enabled",
    expected: json!(true),
    ...
}
```

### 状态文件位置

```bash
# TUI 状态
/tmp/ci_tui_status.json

# CLI 状态
/tmp/ci_cli_vcom1_status.json
/tmp/ci_cli_vcom2_status.json

# 调试截图
/tmp/tui_e2e_debug/
```

## 📝 测试模板

### 单站 Master 测试模板

```rust
pub async fn test_tui_master_xxx(port1: &str, port2: &str) -> Result<()> {
    // 1. 准备
    let config = StationConfig { ... };
    let test_data = generate_test_data();
    
    // 2. 启动和配置
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;
    configure_station(&mut session, &mut cap, config).await?;
    
    // 3. 验证
    verify_master_data(port2, &test_data, &config).await?;
    
    // 4. 清理
    drop(session);
    Ok(())
}
```

### 单站 Slave 测试模板

```rust
pub async fn test_tui_slave_xxx(port1: &str, port2: &str) -> Result<()> {
    // 1. 准备
    let config = StationConfig { 
        connection_mode: ConnectionMode::Slave,
        ...
    };
    let test_data = generate_test_data();
    
    // 2. 启动和配置
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;
    configure_station(&mut session, &mut cap, config).await?;
    
    // 3. CLI Master 写入数据
    send_data_via_cli_master(port2, &test_data, &config)?;
    
    // 4. 验证 TUI 接收
    verify_slave_data(&mut session, &mut cap, &test_data, &config).await?;
    
    // 5. 清理
    drop(session);
    Ok(())
}
```

## ⚠️ 常见陷阱

| 问题 | 原因 | 解决方案 |
|------|------|----------|
| 配置未生效 | 未等待状态同步 | 使用 `CheckStatus` 验证 |
| 测试间干扰 | 配置缓存残留 | 调用 `cleanup_tui_config_cache()` |
| 超时失败 | sleep 时间不够 | 增加超时或使用状态轮询 |
| 光标位置错误 | 未移动到正确位置 | 使用 `PressCtrlPageUp` 复位 |

## 🎯 实现优先级

### 第一批（核心功能）

1. ✅ `test_tui_master_coils`
2. ✅ `test_tui_slave_coils`
3. ✅ `test_tui_master_holding_registers`

### 第二批（扩展覆盖）

4. ⏳ 其他单站测试
5. ⏳ 多站混合类型测试

### 第三批（边界场景）

6. ⏳ 多站地址间隔测试
7. ⏳ 多站混合站号测试

## 🐛 调试技巧

```bash
# 1. 启用调试模式
cargo run --package tui_e2e -- --module xxx --debug

# 2. 检查状态文件
cat /tmp/ci_tui_status.json | jq

# 3. 查看调试截图
ls -lh /tmp/tui_e2e_debug/

# 4. 实时监控状态
watch -n 0.5 'cat /tmp/ci_tui_status.json | jq ".ports[0]"'
```

## 📊 进度追踪

```bash
# 查看待实现的测试
rg "todo!" src/e2e/ --count

# 统计代码行数
wc -l src/e2e/**/*.rs
```

## 🔗 相关资源

- `../ci_utils/` - 测试工具库
- `../../src/protocol/status/` - 状态结构定义
- `../../docs/zh-chs/CLI_MODBUS.md` - CLI 使用文档
- `../../CLAUDE.md` - 项目上下文

---

**提示**: 始终参考 TEST_FRAMEWORK_SUMMARY.md 了解详细的技术细节和最佳实践。
