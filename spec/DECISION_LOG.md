# 决策记录

## 职责

本文档记录当前仍然生效的核心技术和产品决策。

本文档不记录临时调试结论。

## 决策 1

决策：首版工程采用 Tauri v2、Rust、React、TypeScript 和 Vite。

原因：该组合满足 Mac 和 Windows 跨平台桌面应用、Rust 后端边界和 React 前端开发效率。

代码影响：根目录保存 Vite 前端，`src-tauri/` 保存 Rust 后端。

测试影响：前端使用 Vitest，Rust 使用 Cargo test，边界使用静态脚本检查。

排障影响：前端问题先查 `src/`，Tauri 边界问题先查 `src-tauri/src/tauri_api/`，纯规则问题先查 `src-tauri/src/domain/`。

状态：生效。

## 决策 2

决策：阶段 0 只实现扩展模式，不实现 mini 模式入口。

原因：需求要求首版实现扩展模式，mini 模式只作为未来边界保留。

代码影响：`PanelMode` 当前只包含 `Expanded`。

测试影响：阶段 0 只验证扩展模式默认探针。

排障影响：若 UI 出现 mini 入口，应视为范围外行为。

状态：生效。

## 决策 3

决策：hook helper 在 bridge 不可用或 payload 无法处理时保持 fail-open 行为。

原因：hook helper 位于 agent 生命周期边界，任何本地 panel 故障都不得阻塞调用方 agent。

代码影响：`builder-panel-hook` 空 stdin 直接退出，payload 或 bridge 失败时不输出 stdout directive。

测试影响：hook CLI 测试覆盖空 stdin、非法 JSON、bridge 不可用和 directive 输出。

排障影响：若 hook 在 bridge 不可用时输出阻塞内容，应检查 `src-tauri/src/adapters/bridge/hook_cli.rs` 和 `src-tauri/src/adapters/bridge/hook_output.rs`。

状态：生效。

## 决策 4

决策：阶段 0 建立 `spec/` 文档骨架，不建立 `spec/STORE/`。

原因：当前没有数据库、缓存或持久化主事实。

代码影响：不新增持久化模块。

测试影响：文档检查只覆盖当前实际存在的 spec 文档。

排障影响：出现持久化主事实时必须补建 `spec/STORE/` 并登记索引。

状态：生效。

## 决策 5

决策：Domain 时间使用 `UnixMillis` 值对象。

原因：阶段 1 需要显式时间排序和序列化，但 Domain 不应读取系统时间或依赖运行时。

代码影响：`src-tauri/src/domain/usage.rs` 定义 `UnixMillis`，事件和 session 更新时间使用该值对象。

测试影响：Domain 测试直接构造确定时间戳。

排障影响：若时间展示错误，先检查 adapter 或 service 传入 Domain 的时间值。

状态：生效。

## 决策 6

决策：仓库不强制替换 crates.io registry。

原因：registry 镜像属于本地或 CI 网络策略，不应默认影响所有开发者和发布环境。

代码影响：仓库不保存 `.cargo/config.toml` registry 替换配置。

测试影响：本地网络无法直连 crates.io 时，可在命令环境中临时指定镜像完成依赖解析。

排障影响：若 Cargo 依赖解析失败，先区分是代码编译问题还是本地 registry 网络问题。

状态：生效。

## 决策 7

决策：本地 bridge 使用 NDJSON envelope，并在 Mac UDS 与 Windows Named Pipe 间共享同一 schema。

原因：hook helper 需要跨平台发送本地事件，同时保持协议测试可复用。

代码影响：`src-tauri/src/adapters/bridge/codec.rs` 定义 request、response、schema version、command type、result type 和错误结构。

测试影响：codec 测试覆盖单行、多行、半包、空行、非法 JSON 和 response schema。

排障影响：若 UDS 和 Named Pipe 行为不一致，先检查是否复用了同一 codec。

状态：生效。

## 决策 8

决策：阶段 2 只对 hook payload 做最低限度结构校验，不把真实 Codex 或 Claude 私有协议写入 Domain。

原因：当前目标是打通本地 bridge 和 hook helper 链路，真实 agent adapter 仍需后续独立验证。

代码影响：`src-tauri/src/adapters/bridge/hook_payload.rs` 在 adapter 边界清洗 JSON，Domain 不接收 `serde_json::Value`。

测试影响：payload 测试覆盖必填字段、事件范围和 JSON 类型边界。

排障影响：若真实 hook payload 被拒绝，先检查该事件是否属于阶段 2 支持范围。

状态：生效。

## 决策 9

决策：阶段 3 使用进程内 mock agent runtime 打通 session、审批、回复和 timeline 闭环。

原因：核心状态、回写和 UI 交互需要先在不依赖真实 Codex 或 Claude Code 私有协议的条件下可验证。

代码影响：`src-tauri/src/adapters/mock_agent/mod.rs` 保存 mock adapter、runtime、directive 记录和 timeline 数据源，Tauri command 通过进程内锁收口访问。

测试影响：Rust 测试覆盖 mock event、service 校验、回写成功、回写失败和 timeline 查询，前端测试覆盖草稿隔离、提交中状态和 timeline 缓存释放。

排障影响：若 mock 闭环失败，先检查 service 校验和 mock runtime；若真实 agent 接入失败，不应修改 mock 基线来掩盖真实 adapter 问题。

状态：生效。

## 决策 10

决策：阶段 4 先实现 Codex 真实接入，Claude Code 真实接入不冒充完成。

原因：Codex CLI hook 与 Codex APP app-server 已具备本机可验证入口，Claude Code 仍需后续独立验证。

代码影响：新增 `codex_cli_hook` runtime、Codex APP app-server schema 探针和前端 Codex CLI API。

测试影响：Rust 测试覆盖 Codex CLI hook 转换、审批等待、Codex APP request 编码和 notification 转换。

排障影响：若 Claude Code 行为异常，不应从 Codex adapter 推断 Claude 协议。

状态：生效。

## 决策 11

决策：Codex APP app-server 字段以本机 schema 探针结果为准，不把 experimental 字段当作跨版本稳定事实。

原因：app-server schema 随 Codex 版本生成，直接写死未验证字段会造成错误能力承诺。

代码影响：`src-tauri/src/adapters/codex_app/mod.rs` 提供 schema 探针和最小 notification 转换。

测试影响：测试固定本项目依赖的关键 schema 文件和已使用 notification 字段。

排障影响：若 Codex APP 接入失败，先运行 schema 探针确认当前 Codex 版本是否仍提供所需入口。

状态：生效。

## 决策 12

决策：阶段 5 先补完整交互契约和 mock 可验证闭环，真实终端跳回和新对话创建只建立端口与降级模型。

原因：审批、选项、回复、快捷回复和预设命令需要稳定状态机和失败保留规则；真实终端控制和 Windows 托管流程尚未完成人工验证，不能作为已支持能力声明。

代码影响：`InteractionService` 处理审批和选项；`ReplyService` 继续处理文本回复；`ShortcutReplyService` 和 `PresetCommandService` 提供纯规则；`JumpTargetPort` 与 `ReplySenderPort` 分离。

测试影响：Rust 测试覆盖选项、快捷回复、预设命令和跳回降级；前端测试覆盖选项选择状态。

排障影响：若 UI 展示发送入口，先检查 `can_send_reply` 和 pending interaction；若跳回失败，不应触发文本回写补偿。

状态：生效。

## 决策 13

决策：阶段 6 的 process timeline 只使用进程内内存缓存，不写入文件或数据库，也不从 transcript 或 JSONL 反向读取。

原因：timeline 是托管事件流的观察入口，不应把历史文件解析伪装成实时托管能力；进程内缓存能满足当前查询、搜索、筛选和释放需求。

代码影响：`src-tauri/src/adapters/timeline/mod.rs` 保存内存缓存、去重、淘汰和大文本释放逻辑；Codex CLI hook runtime 在写 session state 的同时写 timeline 缓存。

测试影响：Rust 测试覆盖去重、单 session 上限、全局上限、优先级淘汰、大文本释放和 Codex CLI hook 接收。

排障影响：若 timeline 缺失，应先检查托管 hook 事件是否到达 runtime，不应通过读取 transcript 或 JSONL 补数据。

状态：生效。

## 决策 14

决策：阶段 7 设置页建立显式设置模型，但不提供自动更新配置项。

原因：当前阶段目标是完成 panel 体验、agent 开关、用量展示和回复行为配置；自动更新涉及发布和安全边界，不属于本阶段可验证能力。

代码影响：`src-tauri/src/services/settings_service.rs` 定义设置模型，`src/components/SettingsPanel.tsx` 展示设置分组。

测试影响：前端测试验证默认设置不包含自动更新配置项，Rust 测试验证设置缺失、损坏和保存。

排障影响：若 UI 出现自动更新设置，应视为范围外行为。

状态：生效。

## 决策 15

决策：阶段 7 只实现可测试的通知计划和记录型通知 adapter，不声明真实系统通知已接入。

原因：通知合并、当前 session 抑制和点击定位是稳定业务规则；真实 Mac 和 Windows 系统通知需要额外平台能力验证，不能用 mock 结果冒充完成。

代码影响：`src-tauri/src/services/notification_service.rs` 负责通知计划，`src-tauri/src/adapters/notification/mod.rs` 提供记录型 adapter。

测试影响：Rust 测试覆盖通知抑制、合并和点击不打开 timeline。

排障影响：若用户没有看到系统通知，当前应先确认是否已有真实通知 adapter，而不是调整通知业务规则。

状态：生效。

## 决策 16

决策：阶段 8 配置保存使用同目录临时文件写入后替换目标文件，并在反序列化边界补齐缺失字段。

原因：配置文件属于用户本地状态，写入失败不能破坏旧配置；缺字段常见于旧版本配置，不应被当作整份配置损坏。

代码影响：`src-tauri/src/adapters/config_file/mod.rs` 负责默认路径、临时文件写入和替换；`src-tauri/src/services/settings_service.rs` 负责设置模型默认值。

测试影响：Rust 测试覆盖缺字段默认化、未知字段丢弃和临时文件写入失败不覆盖旧配置。

排障影响：若设置保存后丢失旧配置，先检查临时文件写入和替换路径；若旧配置缺字段导致整体降级，先检查 serde 默认化。

状态：生效。

## 决策 17

决策：阶段 8 hook 安装器只写入显式 JSON hook 配置，不绕过 Codex hook trust review，设置页只提供用户显式触发的预览、安装和卸载入口。

原因：hook 安装会修改第三方配置，必须先有可预览、可备份、可卸载的基础设施能力；Codex trust review 是外部 agent 自身安全边界，不能由 Builder Panel 绕过；agent 开关变化不应隐式写入第三方配置。

代码影响：`src-tauri/src/adapters/hook_install/mod.rs` 提供安装预览、备份、manifest、安装和卸载；`src/components/SettingsPanel.tsx` 提供显式入口。

测试影响：Rust fixture 测试覆盖 Codex 和 Claude 配置写入、重复安装替换、备份恢复和缺失配置删除；前端测试覆盖 hook 安装目标选择。

排障影响：若 hook 安装后未运行，先检查 Codex 或 Claude 自身 hook review、trust 和配置加载状态，而不是绕过安全检查。

状态：生效。

## 决策 18

决策：阶段 8 性能预算先建立可重复静态场景脚本，不把它声明为 Mac 或 Windows 人工性能验收。

原因：静态脚本能防止基础容量规则回退，但空闲 CPU、系统内存和平台交互仍需要真实环境人工采样。

代码影响：`scripts/check-performance-budget.mjs` 覆盖 10 session、1000 event、1 万 timeline、虚拟列表范围、淘汰和释放场景。

测试影响：CI 执行性能预算静态场景。

排障影响：若真实环境卡顿，应补充系统监控采样，不能只依赖静态脚本结论。

状态：生效。
