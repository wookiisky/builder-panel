//! Agent hook 安装与卸载 adapter。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use toml_edit::{value as toml_value, DocumentMut, Item, Table};

use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};

const CODEX_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        name: "SessionStart",
        matcher: Some("startup|resume|clear|compact"),
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "UserPromptSubmit",
        matcher: None,
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "PreToolUse",
        matcher: Some("*"),
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "PermissionRequest",
        matcher: Some("*"),
        timeout_seconds: 3600,
    },
    HookEventSpec {
        name: "PostToolUse",
        matcher: Some("*"),
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "Stop",
        matcher: None,
        timeout_seconds: 30,
    },
];

const CLAUDE_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        name: "SessionStart",
        matcher: None,
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "UserPromptSubmit",
        matcher: None,
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "PreToolUse",
        matcher: Some("*"),
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "PermissionRequest",
        matcher: Some("*"),
        timeout_seconds: 3600,
    },
    HookEventSpec {
        name: "PostToolUse",
        matcher: Some("*"),
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "Notification",
        matcher: None,
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "Stop",
        matcher: None,
        timeout_seconds: 30,
    },
    HookEventSpec {
        name: "SessionEnd",
        matcher: None,
        timeout_seconds: 30,
    },
];

/// hook 安装目标 agent。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookInstallAgent {
    /// Codex CLI。
    Codex,
    /// Claude Code CLI。
    Claude,
}

impl HookInstallAgent {
    /// 返回 hook helper `--source` 参数。
    fn source_arg(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    /// 返回当前支持的 hook 事件。
    fn events(self) -> &'static [HookEventSpec] {
        match self {
            Self::Codex => CODEX_EVENTS,
            Self::Claude => CLAUDE_EVENTS,
        }
    }
}

/// hook 安装路径集合。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallPaths {
    /// Codex hooks.json 路径。
    pub codex_hooks_path: PathBuf,
    /// Codex config.toml 路径。
    pub codex_config_path: PathBuf,
    /// Claude settings.json 路径。
    pub claude_settings_path: PathBuf,
    /// 安装 manifest 路径。
    pub manifest_path: PathBuf,
    /// builder-panel-hook 可执行文件路径。
    pub hook_executable_path: PathBuf,
}

impl HookInstallPaths {
    /// 使用用户目录默认路径创建安装路径集合。
    pub fn user_defaults(hook_executable_path: PathBuf) -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".config"));

        Self {
            codex_hooks_path: home.join(".codex").join("hooks.json"),
            codex_config_path: home.join(".codex").join("config.toml"),
            claude_settings_path: home.join(".claude").join("settings.json"),
            manifest_path: config_home
                .join("builder-panel")
                .join("hook-install-manifest.json"),
            hook_executable_path,
        }
    }

    /// 返回指定 agent 的配置文件路径。
    fn config_path(&self, agent: HookInstallAgent) -> &Path {
        match agent {
            HookInstallAgent::Codex => &self.codex_hooks_path,
            HookInstallAgent::Claude => &self.claude_settings_path,
        }
    }

    /// 返回指定 agent 的所有受管配置文件路径。
    fn config_paths(&self, agent: HookInstallAgent) -> Vec<&Path> {
        match agent {
            HookInstallAgent::Codex => vec![&self.codex_hooks_path, &self.codex_config_path],
            HookInstallAgent::Claude => vec![&self.claude_settings_path],
        }
    }
}

/// hook 安装预览。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallPreview {
    /// 将被修改的配置文件。
    pub files_to_modify: Vec<PathBuf>,
    /// 将被创建的备份文件。
    pub backup_files: Vec<PathBuf>,
    /// 将被写入的 manifest 文件。
    pub manifest_path: PathBuf,
}

/// hook 安装总状态。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallStatus {
    /// 各 agent hook 安装状态。
    pub agents: Vec<HookInstallAgentStatus>,
}

/// 单个 agent hook 安装状态。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallAgentStatus {
    /// 目标 agent。
    pub agent: HookInstallAgent,
    /// 当前状态。
    pub state: HookInstallStateKind,
    /// 用户可读状态文案。
    pub message: String,
    /// 状态原因。
    pub reasons: Vec<String>,
    /// 当前是否允许安装。
    pub can_install: bool,
    /// 当前是否允许卸载。
    pub can_uninstall: bool,
}

/// hook 安装状态类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookInstallStateKind {
    /// 未安装。
    NotInstalled,
    /// 已完整安装。
    Installed,
    /// 配置存在但不完整或与当前规则不一致。
    Partial,
    /// 状态读取或配置解析失败。
    Error,
}

/// hook 安装 manifest。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallManifest {
    /// 已安装的 agent 配置记录。
    pub entries: Vec<HookInstallManifestEntry>,
}

/// hook 安装 manifest 单项。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookInstallManifestEntry {
    /// 已安装 agent。
    pub agent: HookInstallAgent,
    /// 被修改的配置文件路径。
    pub config_path: PathBuf,
    /// 修改前配置是否存在。
    pub existed_before_install: bool,
    /// 修改前备份文件路径。
    pub backup_path: PathBuf,
}

/// hook 安装器。
pub struct HookInstaller {
    /// hook 安装路径集合。
    paths: HookInstallPaths,
}

impl HookInstaller {
    /// 创建 hook 安装器。
    pub fn new(paths: HookInstallPaths) -> Self {
        Self { paths }
    }

    /// 生成安装预览，不修改文件。
    pub fn preview(&self, agents: &[HookInstallAgent]) -> HookInstallPreview {
        let agents = unique_agents(agents);
        HookInstallPreview {
            files_to_modify: agents
                .iter()
                .flat_map(|agent| self.paths.config_paths(*agent))
                .map(Path::to_path_buf)
                .collect(),
            backup_files: agents
                .iter()
                .flat_map(|agent| self.paths.config_paths(*agent))
                .map(backup_path)
                .collect(),
            manifest_path: self.paths.manifest_path.clone(),
        }
    }

    /// 查询当前 hook 安装状态，不修改文件。
    pub fn status(&self) -> HookInstallStatus {
        let manifest = match read_manifest_optional(
            &self.paths.manifest_path,
            AppErrorCode::HookInstallFailed,
        ) {
            Ok(manifest) => manifest,
            Err(error) => {
                return HookInstallStatus {
                    agents: vec![
                        manifest_error_status(HookInstallAgent::Codex, &error.user_message),
                        manifest_error_status(HookInstallAgent::Claude, &error.user_message),
                    ],
                };
            }
        };

        HookInstallStatus {
            agents: vec![
                self.agent_status(HookInstallAgent::Codex, manifest.as_ref()),
                self.agent_status(HookInstallAgent::Claude, manifest.as_ref()),
            ],
        }
    }

    /// 安装 hook 配置并写入 manifest。
    pub fn install(&self, agents: &[HookInstallAgent]) -> Result<HookInstallManifest, AppError> {
        let agents = unique_agents(agents);
        let status = self.status();
        let agents_to_install = installable_agents(&agents, &status)?;
        if agents_to_install.is_empty() {
            return Ok(self.current_manifest_or_empty()?);
        }

        let mut plans = Vec::new();

        for agent in &agents_to_install {
            match agent {
                HookInstallAgent::Codex => {
                    plans.push(self.codex_hooks_plan(*agent)?);
                    plans.push(self.codex_config_plan(*agent)?);
                }
                HookInstallAgent::Claude => {
                    plans.push(self.json_hooks_plan(*agent, self.paths.config_path(*agent))?);
                }
            }
        }
        apply_existing_manifest_backup_policy(&mut plans, &self.paths.manifest_path)?;

        let manifest = self.merge_manifest_after_install(&agents_to_install, &plans)?;
        let manifest_value = serde_json::to_value(&manifest).map_err(|error| {
            hook_error(
                AppErrorCode::HookInstallFailed,
                "hook 安装 manifest 编码失败",
                error.to_string(),
            )
        })?;

        let mut written_configs = Vec::new();
        let protected_files = protect_install_files(&plans, &self.paths.manifest_path)?;
        let write_result = (|| {
            for plan in &plans {
                ensure_parent_dir(&plan.entry.config_path, AppErrorCode::HookInstallFailed)?;
                ensure_parent_dir(&plan.entry.backup_path, AppErrorCode::HookInstallFailed)?;

                if let Some(original_text) = &plan.original_text {
                    fs::write(&plan.entry.backup_path, original_text).map_err(|error| {
                        hook_error(
                            AppErrorCode::HookInstallFailed,
                            "hook 配置备份失败",
                            error.to_string(),
                        )
                    })?;
                } else if plan.entry.backup_path.exists() {
                    fs::remove_file(&plan.entry.backup_path).map_err(|error| {
                        hook_error(
                            AppErrorCode::HookInstallFailed,
                            "旧 hook 备份清理失败",
                            error.to_string(),
                        )
                    })?;
                }

                match &plan.next_content {
                    HookInstallPlanContent::Json(value) => {
                        write_json_file(
                            &plan.entry.config_path,
                            value,
                            AppErrorCode::HookInstallFailed,
                        )?;
                    }
                    HookInstallPlanContent::Text(text) => {
                        write_text_file(
                            &plan.entry.config_path,
                            text,
                            AppErrorCode::HookInstallFailed,
                        )?;
                    }
                }
                written_configs.push(plan);
            }

            write_json_file(
                &self.paths.manifest_path,
                &manifest_value,
                AppErrorCode::HookInstallFailed,
            )
        })();

        if let Err(error) = write_result {
            rollback_written_configs(written_configs);
            restore_protected_files(&protected_files);
            return Err(error);
        }

        Ok(manifest)
    }

    /// 根据 manifest 卸载指定 agent hook。
    pub fn uninstall_agents(&self, agents: &[HookInstallAgent]) -> Result<(), AppError> {
        let agents = unique_agents(agents);
        if agents.is_empty() || !self.paths.manifest_path.exists() {
            return Ok(());
        }

        let manifest = read_manifest_required(
            &self.paths.manifest_path,
            AppErrorCode::HookUninstallFailed,
        )?;
        let (target_entries, remaining_entries): (Vec<_>, Vec<_>) = manifest
            .entries
            .into_iter()
            .partition(|entry| agents.contains(&entry.agent));
        if target_entries.is_empty() {
            return Ok(());
        }

        let protected_files = protect_uninstall_files(&target_entries, &self.paths.manifest_path)?;
        let result = (|| {
            for entry in &target_entries {
                if entry.existed_before_install {
                    ensure_parent_dir(&entry.config_path, AppErrorCode::HookUninstallFailed)?;
                    fs::copy(&entry.backup_path, &entry.config_path).map_err(|error| {
                        hook_error(
                            AppErrorCode::HookUninstallFailed,
                            "hook 配置恢复失败",
                            error.to_string(),
                        )
                    })?;
                } else if entry.config_path.exists() {
                    fs::remove_file(&entry.config_path).map_err(|error| {
                        hook_error(
                            AppErrorCode::HookUninstallFailed,
                            "hook 配置删除失败",
                            error.to_string(),
                        )
                    })?;
                }
            }

            write_remaining_manifest(&self.paths.manifest_path, remaining_entries)
        })();

        if let Err(error) = result {
            restore_protected_files(&protected_files);
            return Err(error);
        }

        Ok(())
    }

    /// 根据 manifest 卸载 hook 并恢复安装前配置。
    pub fn uninstall(&self) -> Result<(), AppError> {
        self.uninstall_agents(&[HookInstallAgent::Codex, HookInstallAgent::Claude])
    }

    fn agent_status(
        &self,
        agent: HookInstallAgent,
        manifest: Option<&HookInstallManifest>,
    ) -> HookInstallAgentStatus {
        let manifest_entries = manifest
            .map(|item| {
                item.entries
                    .iter()
                    .filter(|entry| entry.agent == agent)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let can_uninstall = !manifest_entries.is_empty();
        let expected_paths = self.paths.config_paths(agent);
        let mut reasons = Vec::new();
        let mut has_managed_config = can_uninstall;
        let mut has_error = false;

        if !manifest_entries.is_empty() {
            for path in &expected_paths {
                if !manifest_entries
                    .iter()
                    .any(|entry| entry.config_path == path.to_path_buf())
                {
                    reasons.push(format!("manifest 缺少 {}", path.display()));
                }
            }
        }

        match self.agent_config_check(agent) {
            Ok(check) => {
                has_managed_config = has_managed_config || check.has_builder_panel_hook;
                reasons.extend(check.reasons);
            }
            Err(reason) => {
                has_error = true;
                reasons.push(reason);
            }
        }

        if has_error {
            return HookInstallAgentStatus {
                agent,
                state: HookInstallStateKind::Error,
                message: "状态读取失败".to_string(),
                reasons,
                can_install: false,
                can_uninstall,
            };
        }

        if manifest_entries.is_empty() && !has_managed_config {
            return HookInstallAgentStatus {
                agent,
                state: HookInstallStateKind::NotInstalled,
                message: "未安装".to_string(),
                reasons,
                can_install: true,
                can_uninstall: false,
            };
        }

        if !manifest_entries.is_empty() && reasons.is_empty() {
            return HookInstallAgentStatus {
                agent,
                state: HookInstallStateKind::Installed,
                message: "已安装".to_string(),
                reasons,
                can_install: false,
                can_uninstall: true,
            };
        }

        HookInstallAgentStatus {
            agent,
            state: HookInstallStateKind::Partial,
            message: "需要修复".to_string(),
            reasons,
            can_install: true,
            can_uninstall,
        }
    }

    fn agent_config_check(
        &self,
        agent: HookInstallAgent,
    ) -> Result<HookConfigCheck, String> {
        match agent {
            HookInstallAgent::Codex => self.codex_config_check(),
            HookInstallAgent::Claude => self.json_config_check(agent, self.paths.config_path(agent)),
        }
    }

    fn codex_config_check(&self) -> Result<HookConfigCheck, String> {
        let mut check = self.json_config_check(HookInstallAgent::Codex, &self.paths.codex_hooks_path)?;
        check
            .reasons
            .extend(codex_feature_check(&self.paths.codex_config_path)?);
        Ok(check)
    }

    fn json_config_check(
        &self,
        agent: HookInstallAgent,
        config_path: &Path,
    ) -> Result<HookConfigCheck, String> {
        if !config_path.exists() {
            return Ok(HookConfigCheck {
                has_builder_panel_hook: false,
                reasons: vec![format!("{} 不存在", config_path.display())],
            });
        }

        let config = read_json_object(config_path).map_err(|error| error.user_message)?;
        Ok(json_hook_config_check(
            &config,
            agent,
            &self.paths.hook_executable_path,
            config_path,
        ))
    }

    fn current_manifest_or_empty(&self) -> Result<HookInstallManifest, AppError> {
        read_manifest_optional(&self.paths.manifest_path, AppErrorCode::HookInstallFailed)
            .map(|manifest| manifest.unwrap_or(HookInstallManifest { entries: Vec::new() }))
    }

    fn merge_manifest_after_install(
        &self,
        agents: &[HookInstallAgent],
        plans: &[HookInstallPlan],
    ) -> Result<HookInstallManifest, AppError> {
        let mut entries = read_manifest_optional(
            &self.paths.manifest_path,
            AppErrorCode::HookInstallFailed,
        )?
        .map(|manifest| manifest.entries)
        .unwrap_or_default();
        entries.retain(|entry| !agents.contains(&entry.agent));
        entries.extend(plans.iter().map(|plan| plan.entry.clone()));

        Ok(HookInstallManifest { entries })
    }

    fn codex_hooks_plan(&self, agent: HookInstallAgent) -> Result<HookInstallPlan, AppError> {
        self.json_hooks_plan(agent, &self.paths.codex_hooks_path)
    }

    fn json_hooks_plan(
        &self,
        agent: HookInstallAgent,
        config_path: &Path,
    ) -> Result<HookInstallPlan, AppError> {
        let backup_path = backup_path(config_path);
        let existed_before_install = config_path.exists();
        let original_text = if existed_before_install {
            Some(read_text(config_path, AppErrorCode::HookInstallFailed)?)
        } else {
            None
        };
        let mut config = read_json_object(config_path)?;
        install_agent_hooks(&mut config, agent, &self.paths.hook_executable_path)?;

        Ok(HookInstallPlan {
            entry: HookInstallManifestEntry {
                agent,
                config_path: config_path.to_path_buf(),
                existed_before_install,
                backup_path,
            },
            original_text,
            next_content: HookInstallPlanContent::Json(Value::Object(config)),
        })
    }

    fn codex_config_plan(&self, agent: HookInstallAgent) -> Result<HookInstallPlan, AppError> {
        let config_path = &self.paths.codex_config_path;
        let backup_path = backup_path(config_path);
        let existed_before_install = config_path.exists();
        let original_text = if existed_before_install {
            Some(read_text(config_path, AppErrorCode::HookInstallFailed)?)
        } else {
            None
        };
        let current = original_text.clone().unwrap_or_default();
        let next_text = enable_codex_hooks_feature(&current)?;

        Ok(HookInstallPlan {
            entry: HookInstallManifestEntry {
                agent,
                config_path: config_path.to_path_buf(),
                existed_before_install,
                backup_path,
            },
            original_text,
            next_content: HookInstallPlanContent::Text(next_text),
        })
    }
}

/// hook 安装写入计划。
#[derive(Clone, Debug)]
struct HookInstallPlan {
    /// manifest 单项。
    entry: HookInstallManifestEntry,
    /// 安装前配置原文。
    original_text: Option<String>,
    /// 将写入的配置内容。
    next_content: HookInstallPlanContent,
}

/// 安装前需要保护的文件。
#[derive(Clone, Debug)]
struct ProtectedInstallFile {
    /// 文件路径。
    path: PathBuf,
    /// 安装前内容。
    content: Option<Vec<u8>>,
}

/// hook 安装写入内容。
#[derive(Clone, Debug)]
enum HookInstallPlanContent {
    /// JSON 文件。
    Json(Value),
    /// 文本文件。
    Text(String),
}

/// hook 配置检查结果。
#[derive(Clone, Debug, Eq, PartialEq)]
struct HookConfigCheck {
    /// 配置中是否存在 Builder Panel hook handler。
    has_builder_panel_hook: bool,
    /// 不满足当前安装规则的原因。
    reasons: Vec<String>,
}

/// 单个 hook 事件配置。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HookEventSpec {
    /// hook 事件名。
    name: &'static str,
    /// matcher 表达式。
    matcher: Option<&'static str>,
    /// handler 超时秒数。
    timeout_seconds: u64,
}

fn install_agent_hooks(
    config: &mut Map<String, Value>,
    agent: HookInstallAgent,
    hook_executable_path: &Path,
) -> Result<(), AppError> {
    let command = hook_command(hook_executable_path, agent);
    let hooks_value = config
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(hooks_object) = hooks_value.as_object_mut() else {
        return Err(hook_error(
            AppErrorCode::HookInstallFailed,
            "hook 配置格式无效",
            "hooks 字段不是对象".to_string(),
        ));
    };

    for event in agent.events() {
        let event_value = hooks_object
            .entry(event.name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(groups) = event_value.as_array_mut() else {
            return Err(hook_error(
                AppErrorCode::HookInstallFailed,
                "hook 事件配置格式无效",
                format!("{} 字段不是数组", event.name),
            ));
        };

        remove_builder_panel_handlers(groups);
        groups.push(hook_group(event, &command));
    }

    Ok(())
}

fn hook_group(event: &HookEventSpec, command: &str) -> Value {
    let mut group = Map::new();
    if let Some(matcher) = event.matcher {
        group.insert("matcher".to_string(), Value::String(matcher.to_string()));
    }
    group.insert(
        "hooks".to_string(),
        Value::Array(vec![json!({
            "type": "command",
            "command": command,
            "timeout": event.timeout_seconds,
            "statusMessage": "连接 Builder Panel"
        })]),
    );

    Value::Object(group)
}

fn remove_builder_panel_handlers(groups: &mut Vec<Value>) {
    for group in groups.iter_mut() {
        let Some(group_object) = group.as_object_mut() else {
            continue;
        };
        let Some(hooks) = group_object.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };

        hooks.retain(|hook| !is_builder_panel_hook(hook));
    }

    groups.retain(|group| {
        let Some(group_object) = group.as_object() else {
            return true;
        };
        let Some(hooks) = group_object.get("hooks").and_then(Value::as_array) else {
            return true;
        };

        !hooks.is_empty()
    });
}

fn is_builder_panel_hook(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains("builder-panel-hook"))
}

fn json_hook_config_check(
    config: &Map<String, Value>,
    agent: HookInstallAgent,
    hook_executable_path: &Path,
    config_path: &Path,
) -> HookConfigCheck {
    let mut reasons = Vec::new();
    let command = hook_command(hook_executable_path, agent);
    let has_builder_panel_hook = config
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(object_has_builder_panel_hook);
    let Some(hooks_object) = config.get("hooks").and_then(Value::as_object) else {
        reasons.push(format!("{} 缺少 hooks 对象", config_path.display()));
        return HookConfigCheck {
            has_builder_panel_hook,
            reasons,
        };
    };

    for event in agent.events() {
        let Some(groups) = hooks_object.get(event.name).and_then(Value::as_array) else {
            reasons.push(format!("{} 缺少 {} hook", config_path.display(), event.name));
            continue;
        };
        let expected_group = hook_group(event, &command);
        if !groups.iter().any(|group| group == &expected_group) {
            reasons.push(format!(
                "{} 的 {} hook 与当前安装规则不一致",
                config_path.display(),
                event.name
            ));
        }
    }

    HookConfigCheck {
        has_builder_panel_hook,
        reasons,
    }
}

fn object_has_builder_panel_hook(hooks_object: &Map<String, Value>) -> bool {
    hooks_object.values().any(|event_value| {
        event_value.as_array().is_some_and(|groups| {
            groups.iter().any(|group| {
                group
                    .as_object()
                    .and_then(|group_object| group_object.get("hooks"))
                    .and_then(Value::as_array)
                    .is_some_and(|hooks| hooks.iter().any(is_builder_panel_hook))
            })
        })
    })
}

fn codex_feature_check(config_path: &Path) -> Result<Vec<String>, String> {
    if !config_path.exists() {
        return Ok(vec![format!("{} 不存在", config_path.display())]);
    }

    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("Codex 配置读取失败：{}", error))?;
    let document = text
        .parse::<DocumentMut>()
        .map_err(|error| format!("Codex 配置 TOML 无效：{}", error))?;
    let hooks_enabled = document
        .get("features")
        .and_then(Item::as_table)
        .and_then(|features| features.get("hooks"))
        .and_then(Item::as_value)
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if hooks_enabled {
        return Ok(Vec::new());
    }

    Ok(vec![format!(
        "{} 未启用 [features].hooks",
        config_path.display()
    )])
}

fn enable_codex_hooks_feature(contents: &str) -> Result<String, AppError> {
    let mut document = parse_codex_config_toml(contents)?;
    let features = document
        .as_table_mut()
        .entry("features")
        .or_insert_with(|| Item::Table(Table::new()));
    let Some(features_table) = features.as_table_mut() else {
        return Err(hook_error(
            AppErrorCode::HookInstallFailed,
            "Codex 配置格式无效",
            "features 不是 TOML table".to_string(),
        ));
    };

    features_table["hooks"] = toml_value(true);
    features_table.remove("codex_hooks");

    let next_text = document.to_string();
    parse_codex_config_toml(&next_text)?;
    Ok(next_text)
}

fn parse_codex_config_toml(contents: &str) -> Result<DocumentMut, AppError> {
    contents.parse::<DocumentMut>().map_err(|error| {
        hook_error(
            AppErrorCode::HookInstallFailed,
            "Codex 配置 TOML 无效",
            error.to_string(),
        )
    })
}

fn hook_command(path: &Path, agent: HookInstallAgent) -> String {
    let escaped_path = path.to_string_lossy().replace('"', "\\\"");
    format!("\"{escaped_path}\" --source {}", agent.source_arg())
}

fn read_json_object(path: &Path) -> Result<Map<String, Value>, AppError> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let text = read_text(path, AppErrorCode::HookInstallFailed)?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        hook_error(
            AppErrorCode::HookInstallFailed,
            "hook 配置 JSON 无效",
            error.to_string(),
        )
    })?;
    value.as_object().cloned().ok_or_else(|| {
        hook_error(
            AppErrorCode::HookInstallFailed,
            "hook 配置格式无效",
            "根节点不是对象".to_string(),
        )
    })
}

fn read_text(path: &Path, code: AppErrorCode) -> Result<String, AppError> {
    fs::read_to_string(path)
        .map_err(|error| hook_error(code, "hook 配置读取失败", error.to_string()))
}

fn read_manifest_optional(
    path: &Path,
    code: AppErrorCode,
) -> Result<Option<HookInstallManifest>, AppError> {
    if !path.exists() {
        return Ok(None);
    }

    read_manifest_required(path, code).map(Some)
}

fn read_manifest_required(
    path: &Path,
    code: AppErrorCode,
) -> Result<HookInstallManifest, AppError> {
    let text = fs::read_to_string(path).map_err(|error| {
        hook_error(
            code.clone(),
            "hook 安装 manifest 读取失败",
            error.to_string(),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        hook_error(
            code,
            "hook 安装 manifest 格式无效",
            error.to_string(),
        )
    })
}

fn write_json_file(path: &Path, value: &Value, code: AppErrorCode) -> Result<(), AppError> {
    ensure_parent_dir(path, code.clone())?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| hook_error(code.clone(), "hook 配置编码失败", error.to_string()))?;
    write_text_file(path, &text, code)
}

fn write_text_file(path: &Path, text: &str, code: AppErrorCode) -> Result<(), AppError> {
    ensure_parent_dir(path, code.clone())?;
    let temp_path = atomic_temp_path(path);
    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| {
            hook_error(code.clone(), "hook 配置临时文件创建失败", error.to_string())
        })?;
    temp_file.write_all(text.as_bytes()).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        hook_error(code.clone(), "hook 配置写入失败", error.to_string())
    })?;
    temp_file.sync_all().map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        hook_error(code.clone(), "hook 配置刷盘失败", error.to_string())
    })?;
    drop(temp_file);

    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        hook_error(code, "hook 配置替换失败", error.to_string())
    })
}

fn ensure_parent_dir(path: &Path, code: AppErrorCode) -> Result<(), AppError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)
        .map_err(|error| hook_error(code, "hook 配置目录创建失败", error.to_string()))
}

fn backup_path(config_path: &Path) -> PathBuf {
    let file_name = config_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");

    config_path.with_file_name(format!("{file_name}.builder-panel.bak"))
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}

fn hook_error(code: AppErrorCode, message: &str, detail: String) -> AppError {
    AppError::new(
        code,
        message,
        Some(detail),
        true,
        Some(FallbackAction::OpenSettings),
    )
}

fn protect_install_files(
    plans: &[HookInstallPlan],
    manifest_path: &Path,
) -> Result<Vec<ProtectedInstallFile>, AppError> {
    let mut paths = Vec::new();
    for plan in plans {
        if !paths.contains(&plan.entry.config_path) {
            paths.push(plan.entry.config_path.clone());
        }
        if !paths.contains(&plan.entry.backup_path) {
            paths.push(plan.entry.backup_path.clone());
        }
    }
    if !paths.iter().any(|path| path == manifest_path) {
        paths.push(manifest_path.to_path_buf());
    }

    paths
        .into_iter()
        .map(|path| {
            let content = if path.exists() {
                Some(fs::read(&path).map_err(|error| {
                    hook_error(
                        AppErrorCode::HookInstallFailed,
                        "hook 安装保护文件读取失败",
                        error.to_string(),
                    )
                })?)
            } else {
                None
            };
            Ok(ProtectedInstallFile { path, content })
        })
        .collect()
}

fn apply_existing_manifest_backup_policy(
    plans: &mut [HookInstallPlan],
    manifest_path: &Path,
) -> Result<(), AppError> {
    if !manifest_path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(manifest_path).map_err(|error| {
        hook_error(
            AppErrorCode::HookInstallFailed,
            "旧 hook 安装 manifest 读取失败",
            error.to_string(),
        )
    })?;
    let manifest: HookInstallManifest = serde_json::from_str(&text).map_err(|error| {
        hook_error(
            AppErrorCode::HookInstallFailed,
            "旧 hook 安装 manifest 格式无效",
            error.to_string(),
        )
    })?;

    for plan in plans {
        let Some(existing_entry) = manifest
            .entries
            .iter()
            .find(|entry| entry.config_path == plan.entry.config_path)
        else {
            continue;
        };
        plan.entry.existed_before_install = existing_entry.existed_before_install;
        plan.entry.backup_path = existing_entry.backup_path.clone();
        plan.original_text = if existing_entry.existed_before_install {
            Some(read_text(
                &existing_entry.backup_path,
                AppErrorCode::HookInstallFailed,
            )?)
        } else {
            None
        };
    }

    Ok(())
}

fn restore_protected_files(files: &[ProtectedInstallFile]) {
    for file in files {
        match &file.content {
            Some(content) => {
                if let Some(parent) = file.path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&file.path, content);
            }
            None => {
                if file.path.exists() {
                    let _ = fs::remove_file(&file.path);
                }
            }
        }
    }
}

fn rollback_written_configs(plans: Vec<&HookInstallPlan>) {
    for plan in plans.into_iter().rev() {
        if let Some(original_text) = &plan.original_text {
            let _ = fs::write(&plan.entry.config_path, original_text);
        } else {
            let _ = fs::remove_file(&plan.entry.config_path);
        }
    }
}

fn protect_uninstall_files(
    entries: &[HookInstallManifestEntry],
    manifest_path: &Path,
) -> Result<Vec<ProtectedInstallFile>, AppError> {
    let mut paths = Vec::new();
    for entry in entries {
        if !paths.contains(&entry.config_path) {
            paths.push(entry.config_path.clone());
        }
    }
    if !paths.iter().any(|path| path == manifest_path) {
        paths.push(manifest_path.to_path_buf());
    }

    paths
        .into_iter()
        .map(|path| {
            let content = if path.exists() {
                Some(fs::read(&path).map_err(|error| {
                    hook_error(
                        AppErrorCode::HookUninstallFailed,
                        "hook 卸载保护文件读取失败",
                        error.to_string(),
                    )
                })?)
            } else {
                None
            };
            Ok(ProtectedInstallFile { path, content })
        })
        .collect()
}

fn write_remaining_manifest(
    manifest_path: &Path,
    entries: Vec<HookInstallManifestEntry>,
) -> Result<(), AppError> {
    if entries.is_empty() {
        if manifest_path.exists() {
            fs::remove_file(manifest_path).map_err(|error| {
                hook_error(
                    AppErrorCode::HookUninstallFailed,
                    "hook 安装 manifest 删除失败",
                    error.to_string(),
                )
            })?;
        }
        return Ok(());
    }

    let manifest = HookInstallManifest { entries };
    let value = serde_json::to_value(&manifest).map_err(|error| {
        hook_error(
            AppErrorCode::HookUninstallFailed,
            "hook 安装 manifest 编码失败",
            error.to_string(),
        )
    })?;
    write_json_file(
        manifest_path,
        &value,
        AppErrorCode::HookUninstallFailed,
    )
}

fn manifest_error_status(agent: HookInstallAgent, reason: &str) -> HookInstallAgentStatus {
    HookInstallAgentStatus {
        agent,
        state: HookInstallStateKind::Error,
        message: "状态读取失败".to_string(),
        reasons: vec![reason.to_string()],
        can_install: false,
        can_uninstall: false,
    }
}

fn installable_agents(
    agents: &[HookInstallAgent],
    status: &HookInstallStatus,
) -> Result<Vec<HookInstallAgent>, AppError> {
    let mut installable = Vec::new();
    for agent in agents {
        let Some(agent_status) = status.agents.iter().find(|item| item.agent == *agent) else {
            return Err(hook_error(
                AppErrorCode::HookInstallFailed,
                "hook 状态缺失",
                format!("{agent:?} 未返回安装状态"),
            ));
        };

        if agent_status.can_install {
            installable.push(*agent);
            continue;
        }
        if agent_status.state == HookInstallStateKind::Installed {
            continue;
        }

        return Err(hook_error(
            AppErrorCode::HookInstallFailed,
            "hook 状态异常，无法安装",
            agent_status.reasons.join("；"),
        ));
    }

    Ok(installable)
}

fn unique_agents(agents: &[HookInstallAgent]) -> Vec<HookInstallAgent> {
    let mut unique = Vec::new();
    for agent in agents {
        if unique.contains(agent) {
            continue;
        }

        unique.push(*agent);
    }

    unique
}

#[cfg(test)]
mod tests {
    use super::{
        backup_path, enable_codex_hooks_feature, write_text_file, HookInstallAgent,
        HookInstallManifest, HookInstallPaths, HookInstallStateKind, HookInstaller,
    };
    use crate::domain::app_error::AppErrorCode;
    use serde_json::Value;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use toml_edit::DocumentMut;

    #[test]
    fn preview_lists_target_configs_backups_and_manifest() {
        let root = fixture_root("preview");
        let installer = HookInstaller::new(paths(&root));

        let preview = installer.preview(&[
            HookInstallAgent::Codex,
            HookInstallAgent::Codex,
            HookInstallAgent::Claude,
        ]);

        assert_eq!(preview.files_to_modify.len(), 3);
        assert!(preview
            .files_to_modify
            .contains(&root.join("codex").join("hooks.json")));
        assert!(preview
            .files_to_modify
            .contains(&root.join("codex").join("config.toml")));
        assert!(preview
            .backup_files
            .contains(&root.join("codex").join("hooks.json.builder-panel.bak")));
        assert!(preview
            .backup_files
            .contains(&root.join("codex").join("config.toml.builder-panel.bak")));
        assert_eq!(
            preview.manifest_path,
            root.join("builder-panel")
                .join("hook-install-manifest.json")
        );
        cleanup(root);
    }

    #[test]
    fn install_writes_codex_hooks_and_manifest_with_backup() {
        let root = fixture_root("codex-install");
        let hook_path = root.join("bin").join("builder-panel-hook");
        let codex_path = root.join("codex").join("hooks.json");
        let codex_config_path = root.join("codex").join("config.toml");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(
            &codex_path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo old"}]}]}}"#,
        )
        .expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        let manifest = installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        let value = read_json(&codex_path);

        assert_eq!(manifest.entries.len(), 2);
        assert!(codex_path
            .with_file_name("hooks.json.builder-panel.bak")
            .exists());
        assert!(codex_config_path.exists());
        assert!(fs::read_to_string(&codex_config_path)
            .expect("config should read")
            .contains("hooks = true"));
        assert_eq!(
            value["hooks"]["PermissionRequest"][0]["hooks"][0]["command"],
            format!("\"{}\" --source codex", hook_path.to_string_lossy())
        );
        assert_eq!(
            value["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            3600
        );
        assert!(root
            .join("builder-panel")
            .join("hook-install-manifest.json")
            .exists());
        cleanup(root);
    }

    #[test]
    fn status_reports_not_installed_when_no_managed_config_exists() {
        let root = fixture_root("status-not-installed");
        let installer = HookInstaller::new(paths(&root));

        let status = installer.status();
        let codex = status_for(&status, HookInstallAgent::Codex);

        assert_eq!(codex.state, HookInstallStateKind::NotInstalled);
        assert!(codex.can_install);
        assert!(!codex.can_uninstall);
        cleanup(root);
    }

    #[test]
    fn status_reports_installed_when_manifest_and_configs_match() {
        let root = fixture_root("status-installed");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        let status = installer.status();
        let codex = status_for(&status, HookInstallAgent::Codex);

        assert_eq!(codex.state, HookInstallStateKind::Installed);
        assert!(!codex.can_install);
        assert!(codex.can_uninstall);
        cleanup(root);
    }

    #[test]
    fn status_reports_partial_when_codex_feature_is_disabled() {
        let root = fixture_root("status-feature-disabled");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        fs::write(&hook_paths.codex_config_path, "[features]\nhooks = false\n")
            .expect("config should drift");
        let status = installer.status();
        let codex = status_for(&status, HookInstallAgent::Codex);

        assert_eq!(codex.state, HookInstallStateKind::Partial);
        assert!(codex.can_install);
        assert!(codex.can_uninstall);
        cleanup(root);
    }

    #[test]
    fn status_reports_partial_when_hook_command_drifted() {
        let root = fixture_root("status-command-drift");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        let mut drifted_paths = hook_paths.clone();
        drifted_paths.hook_executable_path = root.join("bin").join("new-builder-panel-hook");
        let drifted_installer = HookInstaller::new(drifted_paths);
        let status = drifted_installer.status();
        let codex = status_for(&status, HookInstallAgent::Codex);

        assert_eq!(codex.state, HookInstallStateKind::Partial);
        assert!(codex.can_install);
        cleanup(root);
    }

    #[test]
    fn single_agent_install_preserves_other_agent_manifest_entries() {
        let root = fixture_root("single-install-preserve-manifest");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("codex hook should install");
        installer
            .install(&[HookInstallAgent::Claude])
            .expect("claude hook should install");
        let manifest = read_manifest(&hook_paths.manifest_path);

        assert_eq!(
            manifest
                .entries
                .iter()
                .filter(|entry| entry.agent == HookInstallAgent::Codex)
                .count(),
            2
        );
        assert_eq!(
            manifest
                .entries
                .iter()
                .filter(|entry| entry.agent == HookInstallAgent::Claude)
                .count(),
            1
        );
        cleanup(root);
    }

    #[test]
    fn single_agent_uninstall_keeps_other_agent_manifest_entries() {
        let root = fixture_root("single-uninstall-keep-manifest");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex, HookInstallAgent::Claude])
            .expect("hooks should install");
        installer
            .uninstall_agents(&[HookInstallAgent::Codex])
            .expect("codex hook should uninstall");
        let manifest = read_manifest(&hook_paths.manifest_path);

        assert!(!hook_paths.codex_hooks_path.exists());
        assert!(!hook_paths.codex_config_path.exists());
        assert!(hook_paths.claude_settings_path.exists());
        assert!(manifest
            .entries
            .iter()
            .all(|entry| entry.agent == HookInstallAgent::Claude));
        cleanup(root);
    }

    #[test]
    #[cfg(unix)]
    fn single_agent_uninstall_rolls_back_when_manifest_write_fails() {
        let root = fixture_root("single-uninstall-rollback");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex, HookInstallAgent::Claude])
            .expect("hooks should install");
        let codex_hooks = fs::read_to_string(&hook_paths.codex_hooks_path)
            .expect("codex hooks should read");
        let codex_config = fs::read_to_string(&hook_paths.codex_config_path)
            .expect("codex config should read");
        let manifest = fs::read_to_string(&hook_paths.manifest_path)
            .expect("manifest should read");
        let manifest_parent = hook_paths
            .manifest_path
            .parent()
            .expect("manifest parent should exist");
        let mut permissions = fs::metadata(manifest_parent)
            .expect("manifest parent metadata should read")
            .permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(manifest_parent, permissions)
            .expect("manifest parent permissions should set");

        let result = installer.uninstall_agents(&[HookInstallAgent::Codex]);

        let mut permissions = fs::metadata(manifest_parent)
            .expect("manifest parent metadata should reread")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(manifest_parent, permissions)
            .expect("manifest parent permissions should restore");
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&hook_paths.codex_hooks_path)
                .expect("codex hooks should reread"),
            codex_hooks
        );
        assert_eq!(
            fs::read_to_string(&hook_paths.codex_config_path)
                .expect("codex config should reread"),
            codex_config
        );
        assert_eq!(
            fs::read_to_string(&hook_paths.manifest_path)
                .expect("manifest should reread"),
            manifest
        );
        cleanup(root);
    }

    #[test]
    fn install_replaces_existing_builder_panel_handler_only() {
        let root = fixture_root("replace-existing");
        let codex_path = root.join("codex").join("hooks.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(
            &codex_path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"builder-panel-hook --source old"}]},{"hooks":[{"type":"command","command":"echo other"}]}]}}"#,
        )
        .expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        let value = read_json(&codex_path);
        let stop_groups = value["hooks"]["Stop"]
            .as_array()
            .expect("Stop should be array");

        assert_eq!(stop_groups.len(), 2);
        assert_eq!(stop_groups[0]["hooks"][0]["command"], "echo other");
        cleanup(root);
    }

    #[test]
    fn repeated_successful_install_uninstalls_to_original_config() {
        let root = fixture_root("repeat-success-uninstall");
        let hook_paths = paths(&root);
        fs::create_dir_all(
            hook_paths
                .codex_hooks_path
                .parent()
                .expect("codex parent should exist"),
        )
        .expect("codex parent should create");
        let original_hooks = r#"{"hooks":{"Stop":[]}}"#;
        let original_config = "[features]\nhooks = false\n";
        fs::write(&hook_paths.codex_hooks_path, original_hooks)
            .expect("hooks fixture should write");
        fs::write(&hook_paths.codex_config_path, original_config)
            .expect("config fixture should write");
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("first install should succeed");
        installer
            .install(&[HookInstallAgent::Codex])
            .expect("second install should succeed");
        installer.uninstall().expect("uninstall should succeed");

        assert_eq!(
            fs::read_to_string(&hook_paths.codex_hooks_path).expect("hooks should read"),
            original_hooks
        );
        assert_eq!(
            fs::read_to_string(&hook_paths.codex_config_path).expect("config should read"),
            original_config
        );
        cleanup(root);
    }

    #[test]
    fn install_keeps_user_handler_when_group_also_contains_builder_panel_handler() {
        let root = fixture_root("keep-mixed-group");
        let codex_path = root.join("codex").join("hooks.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(
            &codex_path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"builder-panel-hook --source old"},{"type":"command","command":"echo user"}]}]}}"#,
        )
        .expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        let value = read_json(&codex_path);
        let stop_groups = value["hooks"]["Stop"]
            .as_array()
            .expect("Stop should be array");

        assert_eq!(stop_groups[0]["hooks"][0]["command"], "echo user");
        assert_eq!(stop_groups.len(), 2);
        cleanup(root);
    }

    #[test]
    fn install_deduplicates_agent_input_before_backup() {
        let root = fixture_root("deduplicate-agent");
        let codex_path = root.join("codex").join("hooks.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[]}}"#).expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        let manifest = installer
            .install(&[HookInstallAgent::Codex, HookInstallAgent::Codex])
            .expect("hook should install");
        installer.uninstall().expect("hook should uninstall");
        let restored = fs::read_to_string(&codex_path).expect("config should restore");

        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(restored, r#"{"hooks":{"Stop":[]}}"#);
        cleanup(root);
    }

    #[test]
    fn uninstall_restores_existing_config_from_backup() {
        let root = fixture_root("restore-existing");
        let codex_path = root.join("codex").join("hooks.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[]}}"#).expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        installer.uninstall().expect("hook should uninstall");
        let restored = fs::read_to_string(&codex_path).expect("config should restore");

        assert_eq!(restored, r#"{"hooks":{"Stop":[]}}"#);
        assert!(!root
            .join("builder-panel")
            .join("hook-install-manifest.json")
            .exists());
        cleanup(root);
    }

    #[test]
    fn uninstall_removes_config_that_did_not_exist_before_install() {
        let root = fixture_root("remove-new");
        let claude_path = root.join("claude").join("settings.json");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Claude])
            .expect("hook should install");
        assert!(claude_path.exists());
        installer.uninstall().expect("hook should uninstall");

        assert!(!claude_path.exists());
        cleanup(root);
    }

    #[test]
    fn uninstall_does_not_reuse_deleted_manifest() {
        let root = fixture_root("manifest-expired");
        let codex_path = root.join("codex").join("hooks.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[]}}"#).expect("fixture should write");
        let installer = HookInstaller::new(paths(&root));

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("hook should install");
        installer.uninstall().expect("hook should uninstall");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[{"hooks":[]}]}}"#)
            .expect("new config should write");
        let result = installer.uninstall();
        let current = fs::read_to_string(&codex_path).expect("config should read");

        assert!(result.is_ok());
        assert_eq!(current, r#"{"hooks":{"Stop":[{"hooks":[]}]}}"#);
        cleanup(root);
    }

    #[test]
    fn install_rolls_back_written_configs_when_manifest_write_fails() {
        let root = fixture_root("manifest-failure-rollback");
        let codex_path = root.join("codex").join("hooks.json");
        let claude_path = root.join("claude").join("settings.json");
        let manifest_path = root
            .join("builder-panel")
            .join("hook-install-manifest.json");
        fs::create_dir_all(codex_path.parent().expect("parent should exist"))
            .expect("codex parent should create");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[]}}"#).expect("codex fixture should write");
        fs::create_dir_all(&manifest_path).expect("manifest blocker should create");
        let installer = HookInstaller::new(paths(&root));

        let result = installer.install(&[HookInstallAgent::Codex, HookInstallAgent::Claude]);
        let restored = fs::read_to_string(&codex_path).expect("codex config should restore");

        assert!(result.is_err());
        assert_eq!(restored, r#"{"hooks":{"Stop":[]}}"#);
        assert!(!claude_path.exists());
        cleanup(root);
    }

    #[test]
    #[cfg(unix)]
    fn repeated_install_skips_when_hook_is_already_current() {
        let root = fixture_root("repeat-install-backup-rollback");
        let hook_paths = paths(&root);
        fs::create_dir_all(
            hook_paths
                .codex_hooks_path
                .parent()
                .expect("codex parent should exist"),
        )
        .expect("codex parent should create");
        fs::write(&hook_paths.codex_hooks_path, r#"{"hooks":{"Stop":[]}}"#)
            .expect("hooks fixture should write");
        fs::write(&hook_paths.codex_config_path, "[features]\nhooks = false\n")
            .expect("config fixture should write");
        let installer = HookInstaller::new(hook_paths.clone());
        installer
            .install(&[HookInstallAgent::Codex])
            .expect("first install should succeed");
        let hooks_backup_path = backup_path(&hook_paths.codex_hooks_path);
        let config_backup_path = backup_path(&hook_paths.codex_config_path);
        let hooks_backup =
            fs::read_to_string(&hooks_backup_path).expect("hooks backup should read");
        let config_backup =
            fs::read_to_string(&config_backup_path).expect("config backup should read");
        let manifest = fs::read_to_string(&hook_paths.manifest_path).expect("manifest should read");
        let mut permissions = fs::metadata(
            hook_paths
                .manifest_path
                .parent()
                .expect("manifest parent should exist"),
        )
        .expect("manifest parent metadata should read")
        .permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(
            hook_paths
                .manifest_path
                .parent()
                .expect("manifest parent should exist"),
            permissions,
        )
        .expect("manifest parent permissions should set");

        let result = installer.install(&[HookInstallAgent::Codex]);

        let mut permissions = fs::metadata(
            hook_paths
                .manifest_path
                .parent()
                .expect("manifest parent should exist"),
        )
        .expect("manifest parent metadata should read")
        .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(
            hook_paths
                .manifest_path
                .parent()
                .expect("manifest parent should exist"),
            permissions,
        )
        .expect("manifest parent permissions should restore");
        assert!(result.is_ok());
        assert_eq!(
            fs::read_to_string(&hooks_backup_path).expect("hooks backup should reread"),
            hooks_backup
        );
        assert_eq!(
            fs::read_to_string(&config_backup_path).expect("config backup should reread"),
            config_backup
        );
        assert_eq!(
            fs::read_to_string(&hook_paths.manifest_path).expect("manifest should reread"),
            manifest
        );
        cleanup(root);
    }

    #[test]
    #[cfg(unix)]
    fn repair_install_failure_restores_config_from_operation_start() {
        let root = fixture_root("repair-install-rollback");
        let hook_paths = paths(&root);
        let installer = HookInstaller::new(hook_paths.clone());

        installer
            .install(&[HookInstallAgent::Codex])
            .expect("first install should succeed");
        let drifted_hooks = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo user drift"}]}]}}"#;
        fs::write(&hook_paths.codex_hooks_path, drifted_hooks)
            .expect("drifted hooks should write");
        let manifest = fs::read_to_string(&hook_paths.manifest_path)
            .expect("manifest should read");
        let manifest_parent = hook_paths
            .manifest_path
            .parent()
            .expect("manifest parent should exist");
        let mut permissions = fs::metadata(manifest_parent)
            .expect("manifest parent metadata should read")
            .permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(manifest_parent, permissions)
            .expect("manifest parent permissions should set");

        let result = installer.install(&[HookInstallAgent::Codex]);

        let mut permissions = fs::metadata(manifest_parent)
            .expect("manifest parent metadata should reread")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(manifest_parent, permissions)
            .expect("manifest parent permissions should restore");
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&hook_paths.codex_hooks_path)
                .expect("codex hooks should reread"),
            drifted_hooks
        );
        assert_eq!(
            fs::read_to_string(&hook_paths.manifest_path)
                .expect("manifest should reread"),
            manifest
        );
        cleanup(root);
    }

    #[test]
    fn install_does_not_write_first_agent_when_second_agent_is_invalid() {
        let root = fixture_root("second-agent-invalid");
        let codex_path = root.join("codex").join("hooks.json");
        let claude_path = root.join("claude").join("settings.json");
        fs::create_dir_all(codex_path.parent().expect("codex parent should exist"))
            .expect("codex parent should create");
        fs::create_dir_all(claude_path.parent().expect("claude parent should exist"))
            .expect("claude parent should create");
        fs::write(&codex_path, r#"{"hooks":{"Stop":[]}}"#).expect("codex fixture should write");
        fs::write(&claude_path, r#"[]"#).expect("claude fixture should write");
        let installer = HookInstaller::new(paths(&root));

        let result = installer.install(&[HookInstallAgent::Codex, HookInstallAgent::Claude]);
        let current = fs::read_to_string(&codex_path).expect("codex config should read");

        assert!(result.is_err());
        assert_eq!(current, r#"{"hooks":{"Stop":[]}}"#);
        assert!(!root
            .join("builder-panel")
            .join("hook-install-manifest.json")
            .exists());
        cleanup(root);
    }

    #[test]
    #[cfg(unix)]
    fn text_write_failure_keeps_existing_config() {
        let root = fixture_root("text-write-failure");
        let config_path = root.join("codex").join("config.toml");
        fs::create_dir_all(config_path.parent().expect("parent should exist"))
            .expect("parent should create");
        fs::write(&config_path, "[features]\nhooks = false\n").expect("fixture should write");
        let mut permissions = fs::metadata(config_path.parent().expect("parent should exist"))
            .expect("metadata should read")
            .permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(
            config_path.parent().expect("parent should exist"),
            permissions,
        )
        .expect("permissions should set");

        let result = write_text_file(
            &config_path,
            "[features]\nhooks = true\n",
            AppErrorCode::HookInstallFailed,
        );

        let mut permissions = fs::metadata(config_path.parent().expect("parent should exist"))
            .expect("metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(
            config_path.parent().expect("parent should exist"),
            permissions,
        )
        .expect("permissions should restore");
        let current = fs::read_to_string(&config_path).expect("config should read");

        assert!(result.is_err());
        assert_eq!(current, "[features]\nhooks = false\n");
        cleanup(root);
    }

    #[test]
    fn codex_config_feature_edit_handles_toml_format_variants() {
        let edited =
            enable_codex_hooks_feature("# user config\n[ features ]\n\"codex_hooks\" = false\n")
                .expect("config should edit");

        edited
            .parse::<DocumentMut>()
            .expect("edited config should remain valid TOML");
        assert!(edited.contains("hooks = true"));
        assert!(!edited.contains("codex_hooks"));
    }

    #[test]
    fn codex_config_feature_edit_rejects_invalid_toml() {
        let result = enable_codex_hooks_feature("[features\nhooks = false\n");

        assert!(result.is_err());
    }

    fn paths(root: &PathBuf) -> HookInstallPaths {
        HookInstallPaths {
            codex_hooks_path: root.join("codex").join("hooks.json"),
            codex_config_path: root.join("codex").join("config.toml"),
            claude_settings_path: root.join("claude").join("settings.json"),
            manifest_path: root
                .join("builder-panel")
                .join("hook-install-manifest.json"),
            hook_executable_path: root.join("bin").join("builder-panel-hook"),
        }
    }

    fn fixture_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "builder-panel-hook-install-{name}-{}",
            std::process::id()
        ));
        cleanup(root.clone());
        root
    }

    fn read_json(path: &PathBuf) -> Value {
        let text = fs::read_to_string(path).expect("json should read");
        serde_json::from_str(&text).expect("json should parse")
    }

    fn read_manifest(path: &PathBuf) -> HookInstallManifest {
        let text = fs::read_to_string(path).expect("manifest should read");
        serde_json::from_str(&text).expect("manifest should parse")
    }

    fn status_for(
        status: &super::HookInstallStatus,
        agent: HookInstallAgent,
    ) -> super::HookInstallAgentStatus {
        status
            .agents
            .iter()
            .find(|item| item.agent == agent)
            .cloned()
            .expect("agent status should exist")
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
