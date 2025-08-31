//! TUI 相关的可调常量集中管理，便于后续统一调整与文档化。
/// 日志面板中每个逻辑分组（时间戳 / 原始 / 解析）占用的行数。
pub const LOG_GROUP_HEIGHT: usize = 3;
/// PageUp/PageDown 的基准跳跃组数（逻辑分组）。
pub const LOG_PAGE_JUMP: usize = 5; // 保守值；实际可视高度由渲染计算。