# Local Bridge

## 职责

本文档记录本地 bridge 的协议语义、传输边界、错误收敛和验收入口。

本文档不复制完整 JSON 字段清单，不记录真实 agent 私有协议。

## 协议事实

本地 bridge 使用 newline-delimited JSON。

每一行只包含一个 request 或 response envelope。

request envelope 固定包含协议版本、command 类型、请求 ID 和已清洗 payload。

response envelope 固定包含协议版本、请求 ID、结果类型、可选 directive payload 和可选错误 payload。

当前协议版本为 1。

当前 command 类型只登记 `process_agent_hook`。

Mac 和 Windows 使用同一套 envelope schema。

hook helper 只接受 request ID 与当前 request 一致的 response。

hook helper 只接受 directive 目标 agent 与当前 hook source 一致的 response。

半包数据只进入 decoder buffer，不提前生成 envelope。

空行被 decoder 忽略。

非法 JSON 被 codec 拒绝，不进入 hook command。

## 传输事实

Mac 使用 Unix Domain Socket。

Windows 使用 Named Pipe。

hook helper 作为 client 连接本地 bridge。

APP 作为 server 接收 hook command 并返回 ack、directive 或 error。

Mac socket 路径可通过 `BUILDER_PANEL_BRIDGE_PATH` 覆盖。

未覆盖时，Mac 默认使用用户 Application Support 下的 Builder Panel socket。

Windows 默认使用 `builder-panel-bridge` Named Pipe。

Windows client 使用调用方传入的 timeout 收敛连接和读写等待。

Windows Named Pipe 代码已按平台条件隔离，但本阶段没有在 Windows 本机完成验证。

## 错误语义

bridge 不存在、连接失败或超时时，hook helper fail-open。

response request ID 或 directive 目标 agent 不匹配时，hook helper fail-open。

codec 解析失败时，请求不进入 APP 主流程。

response 为 error 时，hook helper 不输出阻塞 directive。

directive 编码失败时，hook helper fail-open。

fail-open 不写 stdout，可以写简短 stderr 诊断。

fail-open 退出码保持为 0。

## 代码入口

`src-tauri/src/adapters/bridge/codec.rs` 是协议 schema 和 NDJSON 编解码入口。

`src-tauri/src/adapters/bridge/codec_tests.rs` 是 codec 测试入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 UDS 和 Named Pipe 传输入口。

`src-tauri/src/adapters/bridge/transport/windows_transport.rs` 是 Windows Named Pipe 具体实现入口。

`src-tauri/src/ports/local_bridge_port.rs` 是本地 bridge client 抽象边界入口。

## 相关测试

`src-tauri/src/adapters/bridge/codec_tests.rs` 覆盖单行、多行、半包、空行、非法 JSON 和 schema 一致性。

`src-tauri/src/adapters/bridge/transport.rs` 覆盖 Mac Unix Domain Socket 单请求往返和 bridge 不存在错误。
