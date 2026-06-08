# Builder Panel

Builder Panel 是一个本地优先的跨平台桌面控制面板，用于聚合 Coding Agent 会话状态、处理审批和回复，并查看托管会话的过程事件。

项目使用 Tauri + Rust + React + TypeScript 构建。Rust 后端承载领域模型、状态转换、应用服务和系统边界适配；React 前端只负责面板展示和用户交互。

## 当前定位

Builder Panel 首版面向日常使用 Codex、Claude Code 等 Coding Agent 的开发者。

核心目标是减少终端、编辑器和 Agent 桌面端之间的来回切换：

1. 在始终置顶的浮动 panel 中查看会话状态。
2. 在等待审批、等待选择或等待文本回复时快速处理。
3. 通过快捷回复和预设命令减少重复输入。
4. 在弹出层中查看托管会话的过程事件。
5. 保持本地优先，不上传 prompt、transcript、日志或过程事件。

## 当前能力

当前实现以扩展模式工作台为主。

已建立的能力包括：

1. Tauri 桌面应用入口和 `builder-panel-hook` CLI 入口。
2. Rust Domain 强类型事件、会话、交互、用量和错误模型。
3. 纯 reducer 管理 session 状态和排序规则。
4. 本地 bridge codec、Mac Unix Domain Socket 传输和 Windows Named Pipe 代码入口。
5. mock agent adapter 和 mock runtime 测试基线。
6. Codex CLI hook 事件转换、审批 directive 等待和进程内 timeline 写入。
7. Codex APP app-server schema 探针、消息编码、notification 转换、审批、回复和 timeline 查询闭环。
8. 设置页、本地 JSON 设置文件读写、hook 状态查询、安装和卸载入口。
9. 日志脱敏、spec 文档门禁和性能预算静态检查脚本。

当前不声明以下能力已经完成：

1. Claude Code 真实闭环。
2. Codex APP 审批回写或已有会话自动发现。
3. 真实 Mac 或 Windows 系统通知接入。
4. Windows 本机人工验收。
5. 任意已有终端的可靠输入注入。
6. 从 transcript 或 JSONL 文件恢复 timeline。

## 技术栈

前端：

1. pnpm
2. Vite
3. React
4. TypeScript
5. Vitest
6. ESLint

桌面端和后端：

1. Tauri v2
2. Rust stable
3. serde
4. serde_json
5. windows-sys

## 工程结构

主要目录：

1. `src/`：React 前端。
2. `src-tauri/src/domain/`：纯领域模型、纯状态转换和纯规则。
3. `src-tauri/src/ports/`：系统边界抽象接口。
4. `src-tauri/src/adapters/`：外部系统和基础设施 adapter。
5. `src-tauri/src/services/`：应用服务和用例编排。
6. `src-tauri/src/tauri_api/`：Tauri command 和事件边界。
7. `src-tauri/src/bin/builder-panel-hook.rs`：hook helper CLI 入口。
8. `scripts/`：本地检查脚本。
9. `spec/`：长期稳定的系统事实文档。
10. `open-vibe-island/`：参考项目，不是运行时依赖。

## 本地开发

安装依赖：

```bash
pnpm install
```

启动 Tauri 开发环境：

```bash
pnpm dev
```

只启动前端 Vite 服务：

```bash
pnpm dev:web
```

构建前端：

```bash
pnpm build
```

Tauri 配置中的前端开发地址为 `http://127.0.0.1:1420`。

## 验证命令

前端单元测试：

```bash
pnpm test
```

前端 lint、架构检查和 spec 文档门禁：

```bash
pnpm lint
```

前端构建：

```bash
pnpm build
```

spec 文档质量门禁：

```bash
pnpm spec:check
```

性能预算静态场景：

```bash
pnpm performance:check
```

Rust 单元测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

部分 Rust 测试入口：

```bash
cargo test --manifest-path src-tauri/Cargo.toml bridge
cargo test --manifest-path src-tauri/Cargo.toml codex_app
cargo test --manifest-path src-tauri/Cargo.toml codex_cli_hook
cargo test --manifest-path src-tauri/Cargo.toml hook_install
cargo test --manifest-path src-tauri/Cargo.toml log_sanitizer
```

## 文档入口

优先从 `spec/00_INDEX.md` 查找稳定事实。

常用文档：

1. `REQ.md`：产品需求。
2. `tech.md`：技术方案和架构边界。
3. `plan.md`：分阶段开发计划。
4. `SPEC_DOC.md`：文档系统规范。
5. `spec/SYSTEM_OVERVIEW.md`：系统职责、分层和依赖边界。
6. `spec/SYSTEM_FLOWS.md`：主流程和边界流程。
7. `spec/EXTERNAL_BEHAVIOR.md`：外部可见行为和限制。
8. `spec/INTERNAL_BEHAVIOR.md`：内部协作约束和状态不变量。
9. `spec/ERROR_HANDLING.md`：错误分类、降级和外部表现。
10. `spec/TEST.md`：测试分层、断言模型和验收入口。

## 开发约束

核心约束：

1. Domain 层保持纯粹，不依赖 Tauri、React、文件系统、网络、系统通知、终端或第三方裸 payload。
2. 第三方 payload 必须在 adapter 边界完成校验和清洗。
3. 关键状态必须使用强类型表达，禁止魔法字符串散落在 UI 或 adapter 中。
4. 回写能力、跳回能力、过程能力和审批能力必须显式建模。
5. bridge、hook helper、终端回写和过程事件接收必须 fail-open 或可降级，不影响 agent 本身运行。
6. 用量数字只展示已验证来源提供的数据，不可用时展示 `--` 或隐藏。

详细约束以 `spec/00_INDEX.md` 及其登记的事实文档为准。
