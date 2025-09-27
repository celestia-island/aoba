# CI 设置说明 / CI Setup Documentation

本项目包含三种类型的 CI 任务，满足不同的测试需求。

This project includes three types of CI tasks to meet different testing requirements.

## 1. 基本检查矩阵 / Basic Check Matrix

### 功能 / Features
- 多平台支持：Ubuntu, Windows, macOS
- 多 Rust 版本：stable, beta
- 代码格式检查 (`cargo fmt`)
- Clippy 静态分析
- 单元测试执行

### 运行条件 / Run Conditions
- 每次 push 到 master/main 分支
- 每次 Pull Request

### 配置 / Configuration
```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    rust: [stable, beta]
```

## 2. 烟雾测试环境 / Smoke Test Environment

### 功能 / Features
- 虚拟串口设置 (使用 `socat`)
- 快速黑箱测试
- CLI 功能验证
- 二进制文件存在性检查

### 虚拟串口设置 / Virtual Serial Port Setup
```bash
# 创建虚拟串口对
socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2 &
```

### 测试内容 / Test Coverage
- `aoba --help` - 帮助信息
- `aoba --list-ports` - 串口列表
- `aoba --list-ports --json` - JSON 输出
- TUI 快速启动/关闭测试

### 专用二进制 / Dedicated Binary
项目包含专用的烟雾测试二进制文件：`src/bin/smoke_test.rs`

```bash
cargo build --release --bin smoke_test
./target/release/smoke_test
```

## 3. TUI 集成测试环境 / TUI Integration Test Environment

### 功能 / Features
- 完整 TUI 用户行为模拟
- 终端自动化测试 (使用 `expectrl` crate)
- 动态内容过滤 (旋转指示器、时间戳)
- 键盘输入模拟

### 技术栈 / Technology Stack
- **expectrl**: Rust 实现的 expect-like 功能
- **regex**: 动态内容过滤
- **tokio-test**: 异步测试支持

### 测试类型 / Test Types
1. **基础启动/关闭测试** - TUI 启动和退出
2. **导航测试** - 键盘导航功能
3. **串口交互测试** - 虚拟串口连接和操作

### 动态内容过滤 / Dynamic Content Filtering
```rust
fn filter_dynamic_content(content: &str) -> String {
    // 过滤旋转指示器
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    
    // 过滤时间戳
    let re = regex::Regex::new(r"\d{2}:\d{2}:\d{2}").unwrap();
    
    // ... 更多过滤逻辑
}
```

## 运行方式 / How to Run

### 本地运行 / Local Execution

```bash
# 基础检查
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test

# 烟雾测试 (需要先构建)
cargo build --release
cargo build --release --bin smoke_test
./target/release/smoke_test

# 集成测试
cargo test --test integration_tests

# TUI 集成测试 (需要 socat)
sudo apt-get install -y socat  # Linux
cargo test --test tui_integration_tests
```

### CI 环境 / CI Environment

CI 会自动：
1. 安装系统依赖 (libudev-dev, socat, etc.)
2. 设置虚拟串口
3. 运行所有测试
4. 清理测试资源

## 依赖要求 / Dependencies

### 系统依赖 / System Dependencies
- **Linux**: `libudev-dev`, `pkg-config`, `libx11-dev`, `socat`
- **Windows**: 自动处理
- **macOS**: 自动处理

### Rust 依赖 / Rust Dependencies
```toml
[dev-dependencies]
expectrl = "^0.7"
tokio-test = "^0.4"
regex = "^1"
```

## 注意事项 / Notes

1. **虚拟串口**: Linux 环境使用 `socat` 创建，其他平台可能需要不同工具
2. **TUI 测试**: 可能在某些终端环境中不稳定，CI 中有超时保护
3. **动态内容**: TUI 中的旋转指示器和时间戳会被自动过滤以确保测试稳定性
4. **权限**: 虚拟串口创建后会自动设置适当权限 (666)

## 故障排除 / Troubleshooting

### 常见问题 / Common Issues

1. **socat 未找到**
   ```bash
   sudo apt-get install -y socat
   ```

2. **权限问题**
   ```bash
   sudo chmod 666 /tmp/vcom*
   ```

3. **TUI 测试挂起**
   - CI 中有自动超时机制
   - 本地测试可以使用 `timeout` 命令

4. **依赖编译失败**
   ```bash
   sudo apt-get install -y libudev-dev pkg-config libx11-dev
   ```