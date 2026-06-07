# Hook Install

## 职责

Hook Install 记录 Builder Panel 管理 Codex CLI 和 Claude Code CLI hook 配置的基础设施事实。

本文档不记录真实用户配置内容，不声明 Windows 本机验收已经完成。

## 安装边界

hook 安装器只操作显式传入的配置路径和 `builder-panel-hook` 可执行文件路径。

安装前可生成预览，列出将被修改的配置文件、备份文件和 manifest 文件。

安装器可查询 Codex CLI hook 和 Claude Code CLI hook 的当前安装状态。

安装状态包含未安装、已安装、需要修复和读取失败。

状态查询会校验 manifest 和实际配置文件。

Codex CLI 状态查询会校验 `hooks.json` 的 handler 和 `config.toml` 的 `[features].hooks`。

安装前会备份已存在的第三方配置文件。

配置文件不存在时，安装器会创建最小 JSON 配置。

安装器只写入命令型 hook handler。

安装器会移除旧的 Builder Panel hook handler，避免重复安装。

旧 handler 与用户 handler 位于同一 hook group 时，安装器只移除 Builder Panel handler。

安装器不会移除其他工具或用户已有的 hook handler。

重复 agent 输入会在写文件前去重。

安装器先读取并构造所有目标配置，再开始写入文件。

安装器写配置文件和 manifest 时使用临时文件替换目标文件。

安装过程中 manifest 写入失败时，已写配置会回滚到安装前状态。

安装完成后会写入 manifest，记录安装前配置是否存在和备份位置。

单项安装会保留 manifest 中其它 agent 的记录。

卸载时按 manifest 恢复安装前已存在的配置文件。

卸载可按 agent 单项执行。

单项卸载会保留 manifest 中其它 agent 的记录。

单项卸载写回剩余 manifest 失败时，会回滚目标配置文件和旧 manifest。

安装前配置不存在时，卸载会删除本次创建的配置文件。

最后一个 agent 卸载成功后会删除 manifest，避免陈旧 manifest 再次生效。

严格已安装状态下，重复安装不会再次写入配置。

没有目标 manifest 记录时，重复卸载不会改写配置。

设置页以列表展示 Codex CLI hook 和 Claude CLI hook 的当前状态。

设置页每个 hook 项只提供安装和卸载入口。

设置页不会因 agent 开关变化自动写入第三方 hook 配置。

Tauri hook 安装 command 默认使用当前应用同目录下的 `builder-panel-hook` 作为 helper 路径。

环境变量 `BUILDER_PANEL_HOOK_PATH` 存在时，Tauri hook 安装 command 使用该路径作为 helper 路径。

## Agent 配置

Codex CLI 当前写入 `hooks.json` 形式，不写入 inline `config.toml` hooks。

Codex CLI 当前覆盖 `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse` 和 `Stop`。

Codex CLI `PermissionRequest` 使用长 timeout，避免用户审批前 hook 被提前杀死。

Claude Code CLI 当前写入 `settings.json` 的 `hooks` 字段。

Claude Code CLI 当前覆盖 `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse`、`Notification`、`Stop` 和 `SessionEnd`。

Claude Code CLI `PermissionRequest` 使用长 timeout，避免用户审批前 hook 被提前杀死。

## 安全限制

安装器不静默提权。

安装器不绕过 Codex hook trust review。

Codex 非 managed command hook 仍由 Codex 自身执行 `/hooks` review 和 trust 流程。

安装器不执行真实 Codex 或 Claude Code 进程。

自动化测试只使用临时目录 fixture。

## 代码入口

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 状态查询、安装预览、安装、备份、manifest 和卸载入口。

`src-tauri/src/tauri_api/commands.rs` 是 hook 状态查询、安装预览、安装和卸载 command 入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是被安装的 hook helper CLI 入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询和安装 command 调用入口。

`src/components/SettingsPanel.tsx` 是设置页 hook 安装入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 hook payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 stdout directive 编码入口。

## 相关测试

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖状态查询、安装预览、Codex hook 写入、重复安装跳过、混合 group 保留用户 handler、重复 agent 去重、失败回滚、单项安装 manifest 保留、单项卸载回滚、备份恢复、manifest 删除和缺失配置卸载删除。

`cargo test --manifest-path src-tauri/Cargo.toml hook_install` 验证 hook 安装器。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装按钮禁用规则。

`src/components/SettingsPanel.test.tsx` 覆盖设置页 hook 状态列表展示和单项安装卸载按钮。
