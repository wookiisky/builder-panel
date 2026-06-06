# Preset Command Service

## 模块职责

Preset Command Service 负责把预设命令配置转换为创建新对话计划。

Preset Command Service 只生成计划，不直接启动真实 agent，不打开终端，不写用户配置。

## 代码入口

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划生成入口。

`src-tauri/src/ports/managed_process_port.rs` 是后续托管进程边界预留入口。

`src-tauri/src/adapters/terminal/mod.rs` 是终端交互 adapter 入口。

## 核心流程

调用方提供预设命令和当前创建能力。

支持结构化创建时，计划优先选择结构化创建。

不支持结构化创建但支持托管进程时，计划选择托管进程。

两者都不支持时，计划降级为复制命令。

自动发送回车只来自用户配置，不由服务默认开启。

命令行由启动命令和非空初始 prompt 组成。

## 边界与错误

阶段 5 不声明真实终端启动和 Windows 本机托管进程验证已完成。

复制降级必须携带明确原因。

预设命令计划不代表 session 已创建。

## 相关测试

`src-tauri/src/services/preset_command_service.rs` 覆盖结构化创建优先、托管进程降级和复制降级。
