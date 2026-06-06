执行任务时，首先阅读项目文档索引 spec/00_INDEX.md，然后按需阅读相关文档获取当前实现、限制、需求等，结合已有代码完成任务。任务结束后更新所有相关文件。特别注意是否新增了约束和限制。
文档更新必须覆盖横向稳定事实：
- 新增、删除、重命名文档，或调整文档定位、事实入口、索引结构时，必须评估并更新 spec/00_INDEX.md。
- 系统职责、事实分层、运行时分层、组合根、请求收口、焦点规则或跨层依赖边界变化时，必须评估并更新 spec/SYSTEM_OVERVIEW.md。
- 主链路、恢复链路、异常收敛链路，或入口、路由、执行、工具、输出、记忆之间的跨模块流程变化时，必须评估并更新 spec/SYSTEM_FLOWS.md。
- 外部可见行为、Gateway 协议、产品限制、用户验收口径变化时，必须评估并更新 spec/EXTERNAL_BEHAVIOR.md。
- 内部模块协作、调度规则、状态不变量、异常收敛、恢复和并发约束变化时，必须评估并更新 spec/INTERNAL_BEHAVIOR.md。
- 入口、Router、Executor、Response Gate、恢复、静默失败路径的错误分类、异常收敛、降级、资源回收策略变化时，必须评估并更新 spec/ERROR_HANDLING.md。
- 架构边界、核心设计取舍、稳定事实来源、关键流程原则变化时，必须评估并更新 spec/DECISION_LOG.md。
- 即使最终判断这些非具体模块文档不需要更新，也要在交付说明中说明已检查。
测试时，根据 tests/TEST_README.md 的说明执行测试。修改代码逻辑时需要先分析说明，经过用户确认后才能修改代码。
定位错误时，开发环境：使用环境变量 JP_PG_URL链接数据库，数据库前缀dev_。本地环境：使用postgresql+asyncpg://postgres:postgres@localhost:5432/postgres 连接本地数据库，数据库前缀cortex_

## Codex 开发完成 Review 流程
- 主 agent 在开始修改前必须记录本轮任务范围；完成代码、测试、配置或文档修改后，最终交付前必须启动一次 Codex 原生 subagent 做独立 review。
- review 触发必须使用 Codex App 可展示的原生 subagent，使用spawn_agent方式触发。
- 若 subagent 工具暴露 `fork_context` 参数，必须显式设为 `false`；若当前环境不能指定无上下文启动，必须说明独立 reviewer 未执行，不得用普通同上下文 review 冒充。
- reviewer 的输入只允许包含仓库路径、本轮任务相关文件清单、review 范围和输出要求；不得把主任务需求、方案、实现解释、测试结论或中间推理粘贴给 reviewer。
- reviewer 只 review 本轮任务相关修改。工作区里已有的其他 dirty 文件只能列为“未审查的既有改动”，不得作为默认 review 范围或 finding 来源，除非用户明确要求。
- 主 agent 必须等待 reviewer 完成，先处理或回应 reviewer 的 P0/P1/P2 问题，再给最终交付说明。
- reviewer 只做代码审查和工作区修改扫描，不修改文件、不提交、不声明修复完成；reviewer 结论不是“已修复”证明。
