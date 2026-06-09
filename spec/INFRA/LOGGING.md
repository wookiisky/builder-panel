# Logging

## 职责

Logging 记录 Builder Panel 本地事件日志的基础设施事实。

本文档不记录具体业务事件清单的逐条枚举，不约束未来新增事件名。

本文档不记录真实用户日志内容。

## 启用边界

事件日志默认关闭。

事件日志仅在设置中显式启用后写入文件。

事件日志开关属于 Builder Panel 设置的 `logging.enabled` 字段。

设置变化保存成功后日志器配置同步刷新。

应用进程启动时按持久化设置恢复日志器启用状态。

`builder-panel-hook` helper 进程启动时按持久化设置恢复日志器启用状态。

日志关闭时所有事件写入操作为 no-op，不打开文件，不构造 JSON。

日志启用时事件以 JSON 行形式追加写入当前日志文件。

日志写入失败、序列化失败、目录创建失败均被吞掉，不影响主流程。

## 文件位置

日志文件默认位于平台用户配置目录下的 `Builder Panel/logs/app.log`。

日志文件路径与设置文件 `settings.json` 同根。

环境变量 `BUILDER_PANEL_LOG_DIR` 存在时，日志器使用该路径作为日志目录。

设置页展示当前日志文件绝对路径。

设置页提供"打开日志目录"操作，调用平台文件管理器打开日志所在目录。

打开日志目录前若目录不存在会先创建目录。

## 文件格式

每条事件占一行 JSON。

每条事件包含 UTC 时间戳、事件级别、事件名、进程 ID 和载荷字段。

事件级别取值 `info` 或 `error`。

事件名使用中文业务事件描述。

载荷在写入前复用日志脱敏规则，敏感字段被替换为 `[已脱敏]`，长字符串被截断。

载荷不包含原始用户 prompt、transcript、token、API key 或其他敏感字段名。

## 滚动策略

单个日志文件大小阈值为 5 MB。

写入前若当前文件超过阈值，会先执行滚动。

滚动时已存在的 `app.log.N` 依次重命名为 `app.log.N+1`。

滚动后当前文件被重命名为 `app.log.1`。

最多保留 3 份历史文件，超过的最旧文件被删除。

滚动失败被吞掉，不影响后续事件写入尝试。

## 记录范围

事件日志记录设置保存成功和失败。

事件日志记录 hook 安装和卸载成功和失败。

事件日志记录 Codex CLI bridge 启动成功和失败。

事件日志记录 Codex APP app-server 启动成功和失败。

事件日志记录 Codex APP follow-up 创建成功和失败阶段，不记录用户 prompt 原文。

事件日志记录 hook helper 调用入参摘要、完成和非零退出。

## 安全限制

事件日志不写入用户 prompt、transcript、API key、token、密码或其他敏感字段值。

事件日志不绕过日志脱敏规则。

事件日志不传输到外部服务，仅写入本地文件。

事件日志不记录单条超过日志脱敏字符串上限的原始文本。

## 代码入口

`src-tauri/src/adapters/logging/mod.rs` 是事件日志器、文件滚动、默认路径和日志写入入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

`src-tauri/src/services/settings_service.rs` 是 `LoggingSettings` 默认值入口。

`src-tauri/src/tauri_api/commands.rs` 是日志器初始化、关键事件埋点、`get_log_info` 和 `open_log_folder` command 入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是 hook helper 进程日志初始化和调用日志入口。

`src-tauri/src/lib.rs` 是 Tauri 主进程日志初始化入口。

`src/api/settingsContract.ts` 是前端 `LoggingSettings` 契约入口。

`src/api/settingsApi.ts` 是前端日志路径读取和打开日志目录调用入口。

`src/components/SettingsPanel.tsx` 是设置页 Logging 分组入口。

## 相关测试

`src-tauri/src/adapters/logging/mod.rs` 覆盖关闭时 no-op、JSON 行追加、敏感字段脱敏、超阈值滚动和 UTC 时间戳格式。

`cargo test --manifest-path src-tauri/Cargo.toml logging::` 验证日志器行为。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 覆盖载荷脱敏规则。
