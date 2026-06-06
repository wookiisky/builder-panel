//! Agent hook 安装与卸载 adapter。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

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
                .map(|agent| self.paths.config_path(*agent).to_path_buf())
                .collect(),
            backup_files: agents
                .iter()
                .map(|agent| backup_path(self.paths.config_path(*agent)))
                .collect(),
            manifest_path: self.paths.manifest_path.clone(),
        }
    }

    /// 安装 hook 配置并写入 manifest。
    pub fn install(&self, agents: &[HookInstallAgent]) -> Result<HookInstallManifest, AppError> {
        let agents = unique_agents(agents);
        let mut plans = Vec::new();

        for agent in agents {
            let config_path = self.paths.config_path(agent);
            let backup_path = backup_path(config_path);
            let existed_before_install = config_path.exists();
            let original_text = if existed_before_install {
                Some(read_text(config_path, AppErrorCode::HookInstallFailed)?)
            } else {
                None
            };
            let mut config = read_json_object(config_path)?;
            install_agent_hooks(&mut config, agent, &self.paths.hook_executable_path)?;

            plans.push(HookInstallPlan {
                entry: HookInstallManifestEntry {
                    agent,
                    config_path: config_path.to_path_buf(),
                    existed_before_install,
                    backup_path,
                },
                original_text,
                next_value: Value::Object(config),
            });
        }

        let manifest = HookInstallManifest {
            entries: plans.iter().map(|plan| plan.entry.clone()).collect(),
        };
        let manifest_value = serde_json::to_value(&manifest).map_err(|error| {
            hook_error(
                AppErrorCode::HookInstallFailed,
                "hook 安装 manifest 编码失败",
                error.to_string(),
            )
        })?;

        let mut written_configs = Vec::new();
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

                write_json_file(
                    &plan.entry.config_path,
                    &plan.next_value,
                    AppErrorCode::HookInstallFailed,
                )?;
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
            return Err(error);
        }

        Ok(manifest)
    }

    /// 根据 manifest 卸载 hook 并恢复安装前配置。
    pub fn uninstall(&self) -> Result<(), AppError> {
        let text = fs::read_to_string(&self.paths.manifest_path).map_err(|error| {
            hook_error(
                AppErrorCode::HookUninstallFailed,
                "hook 安装 manifest 读取失败",
                error.to_string(),
            )
        })?;
        let manifest: HookInstallManifest = serde_json::from_str(&text).map_err(|error| {
            hook_error(
                AppErrorCode::HookUninstallFailed,
                "hook 安装 manifest 格式无效",
                error.to_string(),
            )
        })?;

        for entry in manifest.entries {
            if entry.existed_before_install {
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

        fs::remove_file(&self.paths.manifest_path).map_err(|error| {
            hook_error(
                AppErrorCode::HookUninstallFailed,
                "hook 安装 manifest 删除失败",
                error.to_string(),
            )
        })?;

        Ok(())
    }
}

/// hook 安装写入计划。
#[derive(Clone, Debug)]
struct HookInstallPlan {
    /// manifest 单项。
    entry: HookInstallManifestEntry,
    /// 安装前配置原文。
    original_text: Option<String>,
    /// 将写入的配置 JSON。
    next_value: Value,
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

fn write_json_file(path: &Path, value: &Value, code: AppErrorCode) -> Result<(), AppError> {
    ensure_parent_dir(path, code.clone())?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| hook_error(code.clone(), "hook 配置编码失败", error.to_string()))?;
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

fn rollback_written_configs(plans: Vec<&HookInstallPlan>) {
    for plan in plans.into_iter().rev() {
        if let Some(original_text) = &plan.original_text {
            let _ = fs::write(&plan.entry.config_path, original_text);
        } else {
            let _ = fs::remove_file(&plan.entry.config_path);
        }
    }
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
    use super::{HookInstallAgent, HookInstallPaths, HookInstaller};
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn preview_lists_target_configs_backups_and_manifest() {
        let root = fixture_root("preview");
        let installer = HookInstaller::new(paths(&root));

        let preview = installer.preview(&[
            HookInstallAgent::Codex,
            HookInstallAgent::Codex,
            HookInstallAgent::Claude,
        ]);

        assert_eq!(preview.files_to_modify.len(), 2);
        assert!(preview
            .files_to_modify
            .contains(&root.join("codex").join("hooks.json")));
        assert!(preview
            .backup_files
            .contains(&root.join("codex").join("hooks.json.builder-panel.bak")));
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

        assert_eq!(manifest.entries.len(), 1);
        assert!(codex_path
            .with_file_name("hooks.json.builder-panel.bak")
            .exists());
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

        assert_eq!(manifest.entries.len(), 1);
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

        assert!(result.is_err());
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

    fn paths(root: &PathBuf) -> HookInstallPaths {
        HookInstallPaths {
            codex_hooks_path: root.join("codex").join("hooks.json"),
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

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
