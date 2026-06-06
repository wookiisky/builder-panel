# Release Quality

## 职责

Release Quality 记录阶段 8 发布质量、日志脱敏、性能预算和文档门禁事实。

本文档不替代 CI 配置，不记录人工验收结论。

## 日志脱敏

日志脱敏发生在 adapter 边界。

默认日志不得记录 prompt 全文。

默认日志不得记录 transcript 全文。

默认日志不得记录 timeline 全文。

敏感字段名包含 prompt、transcript、timeline、token、secret、password 或 api_key 时，日志载荷输出为脱敏占位。

长字符串日志会被截断，避免把大文本过程事件写入默认日志。

日志业务事件名使用中文。

当前日志脱敏工具不接真实日志框架。

## 性能预算

性能预算脚本覆盖 10 session、1000 event、1 万 timeline、虚拟列表可见范围、单 session timeline 淘汰和大文本释放静态场景。

性能预算脚本不替代 10 分钟空闲 CPU 人工采样。

性能预算脚本不声明 Windows 本机结果。

Mac 人工性能验收结果需要另行记录。

## 文档门禁

spec 文档门禁检查 `spec/00_INDEX.md` 是否覆盖实际 spec 文档。

spec 文档门禁检查职责说明、代码入口、测试或验收入口。

spec 文档门禁禁止 spec 文档包含代码块、流程图语法和 Markdown 表格。

`spec/00_INDEX.md`、`spec/DECISION_LOG.md` 和 `spec/TEST.md` 是特殊文档，不要求代码入口和测试入口。

## CI 行为

CI 执行架构检查、spec 文档门禁、性能预算静态场景、前端 lint、前端测试和 Rust 测试。

CI 当前不执行 Windows 本机人工验收。

## 代码入口

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

`scripts/check-spec-docs.mjs` 是 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 是性能预算静态场景入口。

`.github/workflows/ci.yml` 是发布质量 CI 入口。

`package.json` 是本地发布质量命令入口。

## 相关测试

`src-tauri/src/adapters/log_sanitizer/mod.rs` 覆盖敏感字段脱敏、长文本截断和中文业务事件名。

`pnpm spec:check` 验证 spec 文档质量门禁。

`pnpm performance:check` 验证性能预算静态场景。
