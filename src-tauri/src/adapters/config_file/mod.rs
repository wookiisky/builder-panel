//! 本地配置文件 adapter。

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::config_store_port::SettingsStorePort;
use crate::services::settings_service::BuilderPanelSettings;

static SETTINGS_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 本地 JSON 设置文件存储。
pub struct JsonSettingsStore {
    /// 设置文件路径。
    path: PathBuf,
}

impl JsonSettingsStore {
    /// 使用默认路径创建设置存储。
    pub fn default_path() -> Self {
        let path = std::env::var("BUILDER_PANEL_SETTINGS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_settings_path());

        Self { path }
    }

    /// 使用指定路径创建设置存储。
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 返回本次保存使用的临时文件路径。
    fn temp_path(&self) -> PathBuf {
        let sequence = SETTINGS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
        self.temp_path_for_sequence(sequence)
    }

    /// 返回指定序号对应的临时文件路径。
    fn temp_path_for_sequence(&self, sequence: u64) -> PathBuf {
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.json");
        self.path.with_file_name(format!(
            ".{file_name}.{}.{sequence}.tmp",
            std::process::id()
        ))
    }
}

impl SettingsStorePort for JsonSettingsStore {
    fn load_settings(&self) -> Result<Option<BuilderPanelSettings>, AppError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let text = fs::read_to_string(&self.path).map_err(|error| {
            AppError::new(
                AppErrorCode::ConfigLoadFailed,
                "配置读取失败",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })?;
        let settings = serde_json::from_str(&text).map_err(|error| {
            AppError::new(
                AppErrorCode::ConfigLoadFailed,
                "配置格式无效",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })?;

        Ok(Some(settings))
    }

    fn save_settings(&self, settings: &BuilderPanelSettings) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::new(
                    AppErrorCode::ConfigSaveFailed,
                    "配置目录创建失败",
                    Some(error.to_string()),
                    true,
                    Some(FallbackAction::OpenSettings),
                )
            })?;
        }

        let text = serde_json::to_string_pretty(settings).map_err(|error| {
            AppError::new(
                AppErrorCode::ConfigSaveFailed,
                "配置序列化失败",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })?;

        let temp_path = self.temp_path();
        let mut temp_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                AppError::new(
                    AppErrorCode::ConfigSaveFailed,
                    "配置保存失败",
                    Some(error.to_string()),
                    true,
                    Some(FallbackAction::OpenSettings),
                )
            })?;

        temp_file.write_all(text.as_bytes()).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            AppError::new(
                AppErrorCode::ConfigSaveFailed,
                "配置写入失败",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })?;
        temp_file.sync_all().map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            AppError::new(
                AppErrorCode::ConfigSaveFailed,
                "配置刷盘失败",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })?;
        drop(temp_file);

        fs::rename(&temp_path, &self.path).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            AppError::new(
                AppErrorCode::ConfigSaveFailed,
                "配置替换失败",
                Some(error.to_string()),
                true,
                Some(FallbackAction::OpenSettings),
            )
        })
    }
}

/// 返回当前平台默认设置文件路径。
fn default_settings_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        return std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Library")
            .join("Application Support")
            .join("Builder Panel")
            .join("settings.json");
    }

    #[cfg(target_os = "windows")]
    {
        return std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Builder Panel")
            .join("settings.json");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".config")
            })
            .join("builder-panel")
            .join("settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::JsonSettingsStore;
    use crate::ports::config_store_port::SettingsStorePort;
    use crate::services::settings_service::BuilderPanelSettings;
    use serde_json::json;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn missing_file_returns_empty_settings() {
        let path =
            std::env::temp_dir().join(format!("builder-panel-missing-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let store = JsonSettingsStore::new(path);

        let settings = store.load_settings().expect("missing file should load");

        assert_eq!(settings, None);
    }

    #[test]
    fn saves_and_loads_settings_file() {
        let path = std::env::temp_dir().join(format!(
            "builder-panel-settings-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let store = JsonSettingsStore::new(path.clone());
        let mut settings = BuilderPanelSettings::defaults();
        settings.general.notify_on_completion = false;

        store
            .save_settings(&settings)
            .expect("settings should save");
        let loaded = store.load_settings().expect("settings should load");

        assert_eq!(loaded, Some(settings));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_fields_use_default_values() {
        let path =
            std::env::temp_dir().join(format!("builder-panel-partial-{}.json", std::process::id()));
        fs::write(
            &path,
            json!({
                "display": {
                    "show_usage": false
                },
                "agents": {
                    "unknown_agent": true
                },
                "unknown_section": {
                    "value": true
                }
            })
            .to_string(),
        )
        .expect("fixture should write");
        let store = JsonSettingsStore::new(path.clone());

        let settings = store
            .load_settings()
            .expect("partial settings should load")
            .expect("settings should exist");

        assert_eq!(settings.display.show_usage, false);
        assert_eq!(settings.display.theme, Default::default());
        assert_eq!(settings.display.density, Default::default());
        assert_eq!(settings.display.summary_tooltip_paragraphs, 5);
        assert_eq!(settings.general, Default::default());
        assert_eq!(settings.agents, Default::default());
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn failed_temp_write_keeps_old_settings_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "builder-panel-readonly-settings-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).expect("settings dir should create");
        let path = dir.join("settings.json");
        let store = JsonSettingsStore::new(path.clone());
        fs::write(&path, "{\"general\":{\"notify_on_completion\":false}}")
            .expect("old settings should write");
        let mut permissions = fs::metadata(&dir)
            .expect("settings dir metadata should read")
            .permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(&dir, permissions).expect("settings dir should become readonly");
        let settings = BuilderPanelSettings::defaults();

        let result = store.save_settings(&settings);
        let mut permissions = fs::metadata(&dir)
            .expect("settings dir metadata should read")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&dir, permissions).expect("settings dir should become writable");
        let old_text = fs::read_to_string(&path).expect("old settings should remain");

        assert!(result.is_err());
        assert_eq!(old_text, "{\"general\":{\"notify_on_completion\":false}}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_saves_use_distinct_temp_files() {
        let dir = std::env::temp_dir().join(format!(
            "builder-panel-concurrent-settings-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).expect("settings dir should create");
        let path = dir.join("settings.json");
        let barrier = Arc::new(Barrier::new(2));
        let mut first = BuilderPanelSettings::defaults();
        first.general.notify_on_completion = false;
        let mut second = BuilderPanelSettings::defaults();
        second.display.show_usage = false;

        let first_handle = {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            let settings = first.clone();
            thread::spawn(move || {
                let store = JsonSettingsStore::new(path);
                barrier.wait();
                store.save_settings(&settings)
            })
        };
        let second_handle = {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            let settings = second.clone();
            thread::spawn(move || {
                let store = JsonSettingsStore::new(path);
                barrier.wait();
                store.save_settings(&settings)
            })
        };

        first_handle
            .join()
            .expect("first thread should finish")
            .expect("first save should succeed");
        second_handle
            .join()
            .expect("second thread should finish")
            .expect("second save should succeed");
        let store = JsonSettingsStore::new(path);
        let loaded = store
            .load_settings()
            .expect("settings should load")
            .expect("settings should exist");
        let leftover_temp_files = fs::read_dir(&dir)
            .expect("settings dir should read")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".settings.json.")
            })
            .count();

        assert!(loaded == first || loaded == second);
        assert_eq!(leftover_temp_files, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupted_file_returns_load_error() {
        let path = std::env::temp_dir().join(format!(
            "builder-panel-corrupted-{}.json",
            std::process::id()
        ));
        fs::write(&path, "{invalid").expect("fixture should write");
        let store = JsonSettingsStore::new(path.clone());

        let result = store.load_settings();

        assert!(result.is_err());
        let _ = fs::remove_file(path);
    }
}
