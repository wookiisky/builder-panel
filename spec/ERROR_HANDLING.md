# 错误处理

## 职责

本文档记录错误来源、收敛方式、降级策略和外部表现。

本文档记录 Domain 已定义的业务错误类型和阶段 0 错误收敛方式。

## 错误来源

前端调用 Tauri command 可能失败。

Tauri 应用启动可能失败。

hook helper 未来连接 bridge 可能失败。

hook helper 读取到的 stdin 可能为空或不是合法 JSON。

hook payload 可能缺少必填字段或字段类型错误。

bridge codec 可能收到半包、空行或非法 JSON。

bridge 连接可能失败、超时或返回 error。

bridge response 可能与当前 request ID 或 agent source 不匹配。

hook stdout directive 可能编码失败。

Codex CLI runtime 锁可能损坏。

Codex CLI approval 等待可能超时。

Codex APP app-server schema 探针可能执行失败或缺少关键 schema 文件。

Codex APP app-server 可能启动失败、stdin/stdout 不可用、写入失败、response 超时或返回 error。

Codex APP hook payload 可能被识别为 Codex APP 后无法转换为归一事件。

Codex APP notification 可能缺少必填字段或字段类型错误。

Codex APP app-server server request 可能缺少必填字段或字段类型错误。

Codex APP app-server `thread/loaded/list` 与 `thread/list` response 可能缺少必填字段或字段类型错误。

Codex APP rollout 文件可能不存在、过旧、过大、格式无效或缺少 session 元数据。

Codex rollout tail 目标可能不存在、被截断、被替换、超出大小上限、出现超长行或出现无效 JSON 行。

Tauri session 更新事件可能发送失败。

阶段 0 本地验证可能因缺少 Rust 工具链失败。

Domain 失败事件可能携带统一 `AppError`。

Codex APP 审批提交可能遇到会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

Codex APP 选项提交可能遇到空选择、非法选项值、单选提交多个值、会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

Codex APP 文本回复提交可能遇到空内容、超长内容、会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

跳回终端可能失败。

session 跳回目标可能缺失或不支持。

预设命令可能降级为复制命令。

设置文件可能不存在、损坏或保存失败。

自定义快捷输入配置可能包含非法项或重复 ID。

panel 窗口状态恢复、置顶偏好应用、监听或保存可能失败。

通知发送端口可能失败。

hook 安装可能遇到配置读取失败、JSON 格式无效、备份失败、写入失败或 manifest 写入失败。

hook 卸载可能遇到 manifest 读取失败、manifest 格式无效、备份恢复失败或新建配置删除失败。

hook 卸载成功后删除 manifest 可能失败。

## 收敛规则

前端读取基础探针失败时，使用默认探针状态渲染空 panel。

hook helper 阶段 0 直接退出，不输出阻塞 directive。

hook helper 阶段 2 在空 stdin、非法 JSON、payload 校验失败、bridge 不可用、bridge error 和 directive 编码失败时 fail-open。

hook helper 阶段 2 在 response request ID 或 agent source 不匹配时 fail-open。

bridge codec 对半包只缓存，不写错误事实。

bridge codec 对空行忽略。

bridge codec 对非法 JSON 返回 codec 错误。

Tauri 应用启动失败时，进程直接失败，不写入业务状态。

阶段 0 不建立重试队列。

阶段 0 不写入错误持久化事实。

阶段 2 不写入错误持久化事实。

`Failed` 事件写入 session 最近错误，并清理不可继续的 pending interaction。

Codex APP 审批、选项和回复校验失败返回应用错误，不写 session 状态。

Codex APP 审批、选项和回复回写失败返回可重试应用错误，不写 `InteractionCompleted`。

Codex APP 审批、选项和回复回写失败不清理 pending interaction。

Codex CLI 非阻塞 hook 处理成功后返回 ack。

Codex CLI approval 等待超时时返回 bridge error，hook helper 继续按 fail-open 收敛。

Codex CLI 允许并记住当前收敛为 allow directive，不声明真实记忆持久化。

Codex CLI runtime 锁损坏时返回 bridge error，不写业务状态。

Codex APP schema 探针失败时返回不可用结果，不写 session 状态。

Codex APP 未识别 notification 被忽略。

Codex APP malformed notification 返回 adapter 错误，不写 Domain 状态。

Codex APP malformed server request 回写 JSON-RPC error；能识别 thread 时写失败状态。

Codex APP 未识别 server request 回写 JSON-RPC error；能识别 thread 时写失败状态。

Codex APP hook approval 等待超时时返回 bridge error，hook helper 继续按 fail-open 收敛。

Codex APP app-server request 超时时返回可重试应用错误。

Codex APP app-server 启动或初始化失败时必须尝试回收子进程。

Codex APP app-server 启动或初始化失败后，后端在退避窗口内返回最近一次错误，不重复启动子进程。

Codex APP app-server 正在启动时，其它需要 app-server client 的写入 command 返回“正在启动”，不等待全局 slot 锁。

Codex APP app-server 启动失败时，已进入 runtime 的 hook session 和 hook approval 仍可读取和处理。

Codex APP app-server 子进程退出后，后端清理 app-server client，后续请求进入重启或退避流程。

Codex APP app-server 写入失败时，不清理对应 pending interaction。

Codex APP app-server 审批、文本回复或选项回写成功时，只清理 pending interaction，不把 turn 标记为完成。

Codex APP 未识别或畸形 app-server server request 会回写 JSON-RPC error，避免对端等待悬挂。

Codex APP app-server `notLoaded` 会清理 pending interaction 和对应 RPC 回写上下文。

Codex APP app-server `thread/loaded/list` 与 `thread/list` 读取使用短超时；读取失败不写 session 失败状态。

Codex APP app-server `thread/list` 单条 thread 清洗失败时跳过该条，保留同批其它有效 thread。

Codex APP app-server `thread/list` 中缺 cwd 但带 path 的 thread 不直接写 session 状态；若后续 rollout 读取失败，则保持待识别项目。

Codex APP app-server `thread/list` 中 `status.type` 类型错误时跳过该条，避免把脏数据折叠成完成态。

Codex APP rollout 历史读取失败不写 session 失败状态。

Codex rollout tail 读取失败不写 session 失败状态。

Codex rollout tail 遇到无效 JSON 行时跳过该行。

Codex rollout tail 遇到超长行时丢弃当前半行缓存。

Codex rollout tail 遇到文件截断或替换时重置该目标本地读取状态，不回放旧历史。

Codex rollout tail 目标不在 Codex sessions root 内、文件名不匹配、不是普通文件或文件过大时丢弃该目标。

Codex rollout tail 清洗出的事件写入 runtime 失败时丢弃该事件，不阻塞 watcher 后续轮询。

Tauri session 更新事件发送失败时不写 session 失败状态，前端继续依赖定时刷新兜底。

Codex APP rollout 命中不存在且不可迁移的 session 时丢弃该快照，不创建新的当前 session。

Codex APP recent rollout 命中非候选 thread 时丢弃该历史快照，不创建新的当前 session。

Codex APP thread path 读取到错配 session ID 时丢弃该快照，不创建新的当前 session。

Codex APP thread path 不在 Codex sessions root 内、文件名不匹配、不是普通文件或文件过大时丢弃该快照。

Codex APP 缺少可信 cwd 时写入待识别项目占位，不生成跳回目标。

Codex APP follow-up 写入失败时，不写入用户输入原文事件。

Codex APP follow-up 在 app-server client 获取、loaded thread 查询、thread resume 或 turn/start 任一阶段失败时，释放 follow-up 提交占位并返回可重试错误。

Codex APP `thread/loaded/list` 缺失 `data`、`data` 非数组或包含非字符串 id 时视为 loaded thread 查询失败；空白 id 会跳过，重复 id 会去重，不阻断同批有效 id。

Codex APP follow-up 若 session 仍在运行或存在 pending interaction，后端拒绝创建后续 turn。

设置文件不存在时返回默认设置，不写错误状态。

设置文件缺失字段时补默认值，不写错误状态。

设置文件未知字段被丢弃，不写错误状态。

自定义快捷输入非法项被丢弃，不写错误状态。

自定义快捷输入重复 ID 保留第一项，不写错误状态。

设置文件损坏时返回默认设置和用户可读提示。

设置保存失败时返回应用错误，不声明设置已保存。

session 跳回 command 不因业务不可跳回场景抛出前端异常。

session 跳回 command 返回是否已跳回、用户可读消息和可选复制降级文本。

panel 窗口状态恢复、监听或保存失败时返回前端可展示错误，不写 session 状态。

通知发送失败时返回应用错误，不写 session 状态。

hook 状态读取失败时，前端保留已有 hook 状态并展示错误提示。

hook 安装失败时返回应用错误，不声明 hook 已安装。

hook 安装失败时，已写配置会尽量回滚到安装前状态。

hook 卸载失败时返回应用错误，不声明 hook 已卸载。

hook 单项卸载写回剩余 manifest 失败时，已恢复或删除的目标配置会尽量回滚。

## 降级规则

基础探针不可用时，前端仍展示置顶 panel 的默认文案。

真实 agent 不可用时，占位会话保持未连接状态。

bridge 不可用时，hook helper 不输出阻塞 directive。

directive 编码失败时，hook helper 不输出 stdout。

Codex APP 回写失败时，前端展示错误并允许用户继续处理当前 pending interaction。

Codex APP 选项回写失败时，前端保留当前 interaction 的已选选项。

Codex APP 回复回写失败时，前端保留当前 session 草稿。

快捷回复回写失败时，前端将快捷回复内容写回当前 session 草稿。

跳回失败时，降级动作为复制到剪贴板。

没有跳回目标或不支持跳回时，点击 session 不请求跳回、不更新选中态且不展示错误。

全局跳回开关关闭时，有跳回目标的 session 点击只更新选中态，不请求跳回且不展示错误。

预设命令无法可靠创建时，降级为复制命令。

Codex CLI bridge 不可用或审批超时时，Codex CLI hook 不输出阻塞 directive。

Codex APP schema 或 app-server 不可用时，不展示 Codex APP 结构化控制能力。

Codex APP session 来源读取失败时，前端跳过 Codex APP 来源，不阻断 Codex CLI session 展示。

Codex APP app-server 启动失败时，不通过已加载 thread id 或 thread 元数据补齐 session，但仍可读取当前进程内已捕捉的 hook runtime 状态。

Codex APP app-server `thread/loaded/list` 或 `thread/list` 后台读取被节流或短超时打断时，仅跳过本轮元数据补齐，不阻断当前 session 列表返回。

Codex rollout tail 不可用时，对应 session 仍保留 hook、app-server 和定时刷新可见能力。

Tauri session 更新事件缺失时，前端仍通过定时刷新展示 session 最新状态。

Codex APP rollout 读取不可用时，不补齐历史项目名或历史输出，已捕捉实时 session 继续展示。

Codex APP rollout 读取遇到超长 JSONL 行时跳过该行，继续读取后续有效行。

Codex APP recent rollout 目录遍历达到硬上限时停止本轮发现，已发现的候选仍可继续补齐。

Codex APP 待识别项目不提供跳回；用户点击该 session 行无反应。

Rust 工具链不可用时，只能完成前端测试、架构脚本和静态文件检查。

设置读取失败时，前端使用默认设置继续展示 panel。

浏览器 fallback 设置结构无效时，前端使用默认设置并提示配置损坏。

浏览器 fallback 自定义快捷输入结构无效时，前端按默认值或清洗结果继续展示。

panel 窗口状态恢复或置顶偏好应用失败时，前端继续使用当前窗口展示。

panel 窗口移动、尺寸保存或置顶偏好应用失败时，不影响 session 刷新、审批和回复。

真实系统通知不可用时，当前仅保留记录型通知 adapter 验证路径。

hook 安装失败时，用户可根据状态原因和备份路径人工检查配置。

hook 安装 manifest 写入失败时，不应留下可被卸载流程当作成功安装使用的新 manifest，且旧 manifest 和旧备份不得被污染。

## 用户侧表现

前端 command 失败不会弹出错误通知。

hook helper fail-open 时不输出 stdout。

hook helper 可输出简短 stderr 诊断。

阶段 0 不展示真实 agent 状态。

产品运行时不展示 mock agent 状态。

Codex APP 审批或回复失败时，前端显示错误提示。

Codex APP 选项失败时，前端显示错误提示，并保留已选选项。

Codex CLI session 出现后按真实 hook 状态展示。

Codex APP app-server schema 探针失败不会显示为已支持能力。

Codex APP app-server 启动或连接失败时，Codex APP session 读取仍返回当前进程内已捕捉到的 hook runtime 状态。

设置保存失败时，前端保留用户当前选择并展示错误提示。

panel 窗口状态恢复、置顶偏好应用、监听或保存失败时，前端在设置状态提示中展示错误。

hook 安装或卸载失败时，调用方应展示用户可读错误消息。

## 代码入口

`src/views/BuilderPanelApp.tsx` 是前端 command 失败降级入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是 hook fail-open 入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 错误收敛入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 malformed payload 收敛入口。

`src-tauri/src/adapters/bridge/codec.rs` 是 malformed envelope 收敛入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 bridge 不可用和超时收敛入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 directive 编码失败收敛入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI runtime 和审批等待错误收敛入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、runtime 和审批等待错误收敛入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、schema 探针和 notification 错误收敛入口。

`src-tauri/src/lib.rs` 是 Tauri 启动失败入口。

`src-tauri/src/domain/app_error.rs` 是统一应用错误入口。

`src-tauri/src/domain/session_state.rs` 是失败事件收敛入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP 审批、选项和回复错误收敛入口。

`src-tauri/src/services/interaction_service.rs` 是 mock 测试基线审批和选项错误收敛入口。

`src-tauri/src/services/reply_service.rs` 是 mock 测试基线回复错误收敛入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令复制降级入口。

`src-tauri/src/adapters/terminal/mod.rs` 是跳回失败降级入口。

`src/api/sessionJumpApi.ts` 是前端跳回失败收敛入口。

`src-tauri/src/services/settings_service.rs` 是设置读取和保存错误收敛入口。

`src-tauri/src/adapters/config_file/mod.rs` 是设置文件 IO 和 JSON 解析错误入口。

`src/api/panelWindowApi.ts` 是前端 panel 窗口状态恢复和保存错误入口。

`src-tauri/src/services/notification_service.rs` 是通知发送错误收敛入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 安装和卸载错误收敛入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

`src/api/settingsApi.ts` 是浏览器 fallback 设置结构校验入口。

`src/views/BuilderPanelApp.tsx` 是 Codex CLI 和 Codex APP 前端错误提示入口。

## 相关测试

阶段 0 没有错误通知测试。

`src-tauri/src/domain/app_error.rs` 覆盖错误对象测试。

`src-tauri/src/domain/session_state.rs` 覆盖失败事件测试。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 hook helper fail-open。

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 malformed payload。

`src-tauri/src/adapters/bridge/codec.rs` 覆盖 malformed envelope。

`src-tauri/src/adapters/bridge/transport.rs` 覆盖 bridge 不存在。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI ack、approval 等待和超时等待器。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 Codex APP hook 分流、schema 探针、app-server request 和 notification 字段校验。

`src-tauri/src/services/interaction_service.rs` 覆盖 mock 测试基线审批回写失败不清理 pending。

`src-tauri/src/services/interaction_service.rs` 覆盖 mock 测试基线选项校验失败和回写失败不清理 pending。

`src-tauri/src/services/reply_service.rs` 覆盖 mock 测试基线回复校验失败和回写失败不清理 pending。

`src-tauri/src/services/preset_command_service.rs` 覆盖复制降级计划。

`src-tauri/src/adapters/terminal/mod.rs` 覆盖跳回失败的复制降级。

`src-tauri/src/domain/view_model.rs` 覆盖缺少跳回目标时不生成跳回动作。

`src/stores/mockPanelStore.test.ts` 覆盖回复失败时草稿保留和选项失败时选择保留所需的状态隔离。

`src-tauri/src/services/settings_service.rs` 覆盖设置缺失、损坏和保存失败边界。

`src/api/settingsApi.test.ts` 覆盖自定义快捷输入清洗边界。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖损坏 JSON 设置文件。

`src-tauri/src/services/notification_service.rs` 覆盖通知抑制、合并和点击定位。

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖 hook 状态查询、安装预览、备份恢复和卸载失败边界。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 覆盖敏感日志字段脱敏和长文本截断。
