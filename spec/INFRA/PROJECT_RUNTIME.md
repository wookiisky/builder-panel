# Project Runtime

## 职责

Project Runtime 记录工程运行时、工具链和本地依赖解析事实。

Project Runtime 不记录业务状态，不替代 CI 配置。

## 代码入口

`package.json` 定义前端和 Tauri 命令入口。

`pnpm-workspace.yaml` 将当前仓库固定为独立 pnpm workspace。

`src-tauri/Cargo.toml` 定义 Rust crate 和依赖。

`src-tauri/Cargo.toml` 通过 `default-run` 指定 Tauri dev 默认启动 `builder-panel`。

`pnpm dev` 启动 Tauri 开发环境。

`pnpm dev:web` 只启动前端 Vite 开发服务，并作为 Tauri `beforeDevCommand` 使用。

`pnpm tauri:build` 执行 Tauri release 构建并生成正式桌面程序产物。

`pnpm package` 是正式桌面程序打包入口，等价于 `pnpm tauri:build`。

Tauri release 构建会通过 `beforeBuildCommand` 先执行 `pnpm build`。

正式打包产物位于 `src-tauri/target/release/bundle/`。

`.github/workflows/ci.yml` 定义 CI 验证入口。

`scripts/check-spec-docs.mjs` 定义 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 定义性能预算静态场景入口。

## 工具链事实

前端使用 pnpm、Vite、React、TypeScript、Vitest 和 ESLint。

前端图标资源使用 `lucide-react`。

桌面端使用 Tauri v2 和 Rust。

本地 bridge codec 和 hook helper 运行时依赖 `serde_json`。

Windows Named Pipe 传输在 Windows target 下依赖 `windows-sys`。

本地验证需要 `cargo`、`rustc` 和 Rust stable toolchain。

Codex APP app-server schema 探针需要本机可执行 `codex` CLI。

阶段 8 hook 安装器使用 JSON 配置文件，不新增 TOML 解析依赖。

## 依赖解析

当前项目通过 `pnpm-workspace.yaml` 避免被用户主目录 workspace 配置误收编。

项目不强制替换 crates.io registry。

本地网络无法直连 crates.io 时，可在命令环境中临时指定 registry 镜像。

用户级 Cargo registry 替换可能影响本地测试。

当 Aliyun mirror 缺少 lockfile 依赖时，可用命令级 `--config` 临时覆盖 registry。

## 相关测试

`pnpm test` 验证前端单元测试。

`pnpm lint` 验证前端 lint 和架构检查。

`pnpm build` 验证前端构建。

`pnpm package` 验证当前平台正式桌面程序打包，不属于常规快速验证入口。

`pnpm spec:check` 验证 spec 文档质量门禁。

`pnpm performance:check` 验证性能预算静态场景。

`cargo test --manifest-path src-tauri/Cargo.toml` 验证 Rust 单元测试。

`cargo test --manifest-path src-tauri/Cargo.toml bridge` 验证本地 bridge 和 hook helper 测试。

`cargo test --manifest-path src-tauri/Cargo.toml codex_app` 验证 Codex APP app-server adapter 测试。

`cargo test --manifest-path src-tauri/Cargo.toml codex_cli_hook` 验证 Codex CLI hook adapter 测试。

`cargo test --manifest-path src-tauri/Cargo.toml hook_install` 验证 hook 安装器 fixture 测试。

`cargo test --manifest-path src-tauri/Cargo.toml log_sanitizer` 验证日志脱敏测试。
