# Settings Service

## 模块职责

Settings Service 负责读取、默认化和保存 Builder Panel 设置。

Settings Service 不直接读写文件系统，不决定具体配置路径，不处理 UI 组件状态。

## 代码入口

`src-tauri/src/services/settings_service.rs` 是设置模型、默认值和设置用例入口。

`src-tauri/src/ports/config_store_port.rs` 是设置存储端口入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 入口。

`src-tauri/src/tauri_api/commands.rs` 是设置读写 command 入口。

## 对外接口

设置模型显式分为 General、Display、Panel、Agents、Replies、Presets、Terminal 和 Advanced。

Display 设置控制用量展示、浅色或深色主题、UI 密度和动画等级。

Panel 设置控制收缩状态、上次窗口位置和上次窗口尺寸。

Agents 设置控制 mock agent、Codex CLI、Codex APP、Claude Code CLI 和 Claude Code APP 开关。

当前 UI 只允许修改 mock agent 和 Codex CLI 开关。

Codex APP、Claude Code CLI 和 Claude Code APP 开关当前保存在模型中，但 UI 禁用，不驱动 session 读取。

Replies 设置控制 Enter 发送和快捷回复入口。

设置模型不包含自动更新配置项。

panel 窗口状态可以通过局部保存 command 更新，不要求前端重写完整设置模型。

## 错误与降级

配置文件不存在时返回默认设置。

配置文件缺失字段时使用对应字段默认值。

配置文件未知字段不会进入设置模型。

配置文件损坏或无法解析时返回默认设置，并附带用户可读提示。

配置保存失败时返回应用错误。

Settings Service 不吞掉保存失败。

JSON 设置文件 adapter 使用同目录临时文件写入后替换目标文件。

每次保存会生成唯一临时文件路径，避免同进程并发保存共享临时文件。

临时文件写入失败时不得覆盖旧配置。

## 相关测试

`src-tauri/src/services/settings_service.rs` 覆盖缺失配置、损坏配置和保存。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖设置文件缺失、读写和损坏 JSON。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖缺字段默认化和临时文件写入失败不覆盖旧配置。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖同一路径并发保存不会共享临时文件。

`src/api/settingsApi.test.ts` 覆盖前端默认设置和默认 panel 状态事实。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装目标选择的前端纯状态转换。
