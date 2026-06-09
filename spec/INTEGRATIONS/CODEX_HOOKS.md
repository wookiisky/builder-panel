# Codex Hooks

## 职责

本文档记录 Builder Panel 对 Codex CLI hook 的当前接入边界。

本文档不记录 Codex APP app-server 接入。Codex CLI 阶段 4 真实 hook 闭环事实见 `spec/INTEGRATIONS/CODEX_CLI.md`。

## 当前边界

Codex CLI 通过 `builder-panel-hook --source codex` 进入本地 bridge。

hook helper 只在 payload 基础校验通过后发送 bridge command。

当前默认支持低噪声生命周期事件和权限请求事件。

当前支持事件包括 `SessionStart`、`UserPromptSubmit`、`PermissionRequest` 和 `Stop`。

当前可解析 `PreToolUse` 和 `PostToolUse`，但不把它们作为默认安装承诺。

`PermissionRequest` 可返回 allow 或 deny directive。

非阻塞事件返回 ack 时不输出 stdout。

Codex hook 只处理 Builder Panel APP 启动后送达 bridge 的实时 hook payload。


Codex hook payload 的模型字段不作为 thread 标题展示。

Codex hook 工具调用、工具参数和工具结束事件不写活动摘要。

Codex hook 不从 transcript、JSONL 或其它历史记录恢复 session。

APP 启动后仍在运行的 Codex 任务如果继续触发 hook payload，可以进入当前 session 状态。

## 官方资料边界

Codex hooks 当前使用 `hooks` 作为规范 feature key。

`codex_hooks` 只作为旧别名存在。

Codex 支持 `commandWindows` 作为 Windows 专用 hook command 覆盖。

非托管 command hook 需要 Codex trust review。

本阶段只实现 hook helper 和 bridge 链路，不实现 hook 安装器和 trust review UI。

## 参考实现边界

`open-vibe-island` 的 hook CLI、bridge codec 和 Codex stdout directive 作为实现思路参考。

参考实现不作为当前 Codex 版本的官方协议证明。

stdout directive 格式必须由本项目测试固定。

## 限制

当前不承诺 Codex APP 接入。

Codex APP app-server 当前事实见 `spec/INTEGRATIONS/CODEX_APP.md`。


当前不承诺文件编辑审批一定通过 `PreToolUse` 覆盖。

当前不承诺 Windows Codex hook 已完成本机验收。

## 代码入口

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 Codex hook payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 Codex stdout directive 编码入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook adapter 预留入口。

## 相关测试

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 Codex `PermissionRequest` payload 清洗。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 Codex `PermissionRequest` 长等待和 allow directive。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 Codex allow 和 deny stdout。
