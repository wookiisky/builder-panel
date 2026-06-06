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

阶段 0 本地验证可能因缺少 Rust 工具链失败。

Domain 失败事件可能携带统一 `AppError`。

mock 审批提交可能遇到会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

mock 选项提交可能遇到空选择、非法选项值、单选提交多个值、会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

mock 文本回复提交可能遇到空内容、超长内容、会话不存在、状态不匹配、交互类型不匹配、交互 ID 不匹配或回写失败。

mock timeline 查询可能遇到 reader 端口错误。

Codex CLI timeline 查询可能遇到 reader 端口错误。

timeline 接收过程事件可能遇到缓存写入错误。

timeline 大文本释放可能失败。

跳回终端可能失败。

session 跳回目标可能缺失或不支持。

预设命令可能降级为复制命令。

设置文件可能不存在、损坏或保存失败。

自定义快捷输入配置可能包含非法项或重复 ID。

panel 窗口状态恢复、监听或保存可能失败。

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

mock 审批、选项和回复校验失败返回应用错误，不写 session 状态。

mock 审批、选项和回复回写失败返回可重试应用错误，不写 `TurnCompleted`。

mock 审批、选项和回复回写失败不清理 pending interaction。

mock timeline 查询失败不写 session 状态。

Codex CLI timeline 查询失败不写 session 状态。

timeline 接收失败不写 session 状态。

timeline 大文本释放失败不阻塞关闭弹层。

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

Codex APP app-server 启动、初始化或同步失败时必须尝试回收子进程。

Codex APP app-server 启动、初始化或同步失败后，后端在退避窗口内返回最近一次错误，不重复启动子进程。

Codex APP app-server 正在启动或同步时，其它需要 app-server client 的写入 command 返回“正在启动”，不等待全局 slot 锁。

Codex APP app-server 启动失败时，已进入 runtime 的 hook session 和 hook approval 仍可读取和处理。

Codex APP app-server 子进程退出后，后端清理 app-server client，后续请求进入重启或退避流程。

Codex APP app-server 写入失败时，不清理对应 pending interaction。

Codex APP app-server 审批、文本回复或选项回写成功时，只清理 pending interaction，不把 turn 标记为完成。

Codex APP 未识别或畸形 app-server server request 会回写 JSON-RPC error，避免对端等待悬挂。

Codex APP app-server `notLoaded` 会清理 pending interaction 和对应 RPC 回写上下文。

Codex APP follow-up 写入失败时，不写入“已提交”activity。

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

hook 安装失败时返回应用错误，不声明 hook 已安装。

hook 安装失败时，已写配置会尽量回滚到安装前状态。

hook 卸载失败时返回应用错误，不声明 hook 已卸载。

## 降级规则

基础探针不可用时，前端仍展示置顶 panel 的默认文案。

真实 agent 不可用时，占位会话保持未连接状态。

bridge 不可用时，hook helper 不输出阻塞 directive。

directive 编码失败时，hook helper 不输出 stdout。

mock 回写失败时，前端展示错误并允许用户继续处理当前 pending interaction。

mock 选项回写失败时，前端保留当前 interaction 的已选选项。

mock 回复回写失败时，前端保留当前 session 草稿。

快捷回复回写失败时，前端将快捷回复内容写回当前 session 草稿。

跳回失败时，降级动作为复制到剪贴板。

没有跳回目标或不支持跳回时，前端只保留选中态或展示可复制降级提示。

预设命令无法可靠创建时，降级为复制命令。

Codex CLI bridge 不可用或审批超时时，Codex CLI hook 不输出阻塞 directive。

timeline 不可用时，对应入口不应展示；查询失败时只展示 timeline 错误状态。

timeline 大文本释放失败时，用户仍可关闭弹层。

Codex APP schema 或 app-server 不可用时，不展示 Codex APP 结构化控制能力。

Codex APP session 来源读取失败时，前端跳过 Codex APP 来源，不阻断 mock 和 Codex CLI session 展示。

Rust 工具链不可用时，只能完成前端测试、架构脚本和静态文件检查。

设置读取失败时，前端使用默认设置继续展示 panel。

浏览器 fallback 设置结构无效时，前端使用默认设置并提示配置损坏。

浏览器 fallback 自定义快捷输入结构无效时，前端按默认值或清洗结果继续展示。

panel 窗口状态恢复失败时，前端继续使用当前窗口展示。

panel 窗口移动或尺寸保存失败时，不影响 session 刷新、审批和回复。

真实系统通知不可用时，当前仅保留记录型通知 adapter 验证路径。

hook 安装失败时，用户可根据预览和备份路径人工检查配置。

hook 安装 manifest 写入失败时，不应留下可被卸载流程当作成功安装使用的新 manifest，且旧 manifest 和旧备份不得被污染。

默认日志脱敏 prompt、transcript、timeline、token、secret、password 和 api_key 字段。

## 用户侧表现

前端 command 失败不会弹出错误通知。

hook helper fail-open 时不输出 stdout。

hook helper 可输出简短 stderr 诊断。

阶段 0 不展示真实 agent 状态。

阶段 3 展示 mock agent 状态，但不声明真实 agent 已连接。

mock 审批或回复失败时，前端显示错误提示。

mock 选项失败时，前端显示错误提示，并保留已选选项。

timeline 查询失败时，前端在 timeline 弹层中展示错误状态。

Codex CLI session 出现后按真实 hook 状态展示。

Codex APP app-server schema 探针失败不会显示为已支持能力。

Codex APP app-server 启动或连接失败时，Codex APP session 读取仍返回已进入 hook runtime 的状态。

设置保存失败时，前端保留用户当前选择并展示错误提示。

panel 窗口状态恢复、监听或保存失败时，前端在设置状态提示中展示错误。

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

`src-tauri/src/adapters/timeline/mod.rs` 是 timeline 缓存写入、淘汰和释放入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、schema 探针和 notification 错误收敛入口。

`src-tauri/src/lib.rs` 是 Tauri 启动失败入口。

`src-tauri/src/domain/app_error.rs` 是统一应用错误入口。

`src-tauri/src/domain/session_state.rs` 是失败事件收敛入口。

`src-tauri/src/services/interaction_service.rs` 是 mock 审批错误收敛入口。

`src-tauri/src/services/interaction_service.rs` 是 mock 选项错误收敛入口。

`src-tauri/src/services/reply_service.rs` 是 mock 回复错误收敛入口。

`src-tauri/src/services/process_timeline_service.rs` 是 mock timeline 错误收敛入口。

`src-tauri/src/services/process_timeline_service.rs` 是 Codex CLI timeline 查询错误收敛入口。

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

`src/views/BuilderPanelApp.tsx` 是 mock 前端错误提示入口。

## 相关测试

阶段 0 没有错误通知测试。

`src-tauri/src/domain/app_error.rs` 覆盖错误对象测试。

`src-tauri/src/domain/session_state.rs` 覆盖失败事件测试。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 hook helper fail-open。

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 malformed payload。

`src-tauri/src/adapters/bridge/codec.rs` 覆盖 malformed envelope。

`src-tauri/src/adapters/bridge/transport.rs` 覆盖 bridge 不存在。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI ack、approval 等待和超时等待器。

`src-tauri/src/adapters/timeline/mod.rs` 覆盖 timeline 去重、淘汰和释放。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 Codex APP hook 分流、schema 探针、app-server request 和 notification 字段校验。

`src-tauri/src/services/interaction_service.rs` 覆盖 mock 审批回写失败不清理 pending。

`src-tauri/src/services/interaction_service.rs` 覆盖 mock 选项校验失败和回写失败不清理 pending。

`src-tauri/src/services/reply_service.rs` 覆盖 mock 回复校验失败和回写失败不清理 pending。

`src-tauri/src/services/preset_command_service.rs` 覆盖复制降级计划。

`src-tauri/src/adapters/terminal/mod.rs` 覆盖跳回失败的复制降级。

`src-tauri/src/domain/view_model.rs` 覆盖缺少跳回目标时不生成跳回动作。

`src/stores/mockPanelStore.test.ts` 覆盖 mock 回复失败时草稿保留和选项失败时选择保留所需的状态隔离。

`src-tauri/src/services/settings_service.rs` 覆盖设置缺失、损坏和保存失败边界。

`src/api/settingsApi.test.ts` 覆盖自定义快捷输入清洗边界。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖损坏 JSON 设置文件。

`src-tauri/src/services/notification_service.rs` 覆盖通知抑制、合并和点击定位。

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖 hook 安装预览、备份恢复和卸载失败边界。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 覆盖敏感日志字段脱敏和长文本截断。
