//! 通用事件日志 adapter。
//!
//! 单一全局单例，按 JSON 行追加写入本地文件，支持启用开关、大小滚动和复用脱敏。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::adapters::log_sanitizer::sanitize_log_value;

/// 日志单文件大小上限（5 MB）。
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 滚动保留份数（app.log.1 / 2 / 3）。
const ROTATION_KEEP: usize = 3;
/// 默认日志文件名。
const DEFAULT_LOG_FILE: &str = "app.log";

/// 日志级别。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    /// 普通业务事件。
    Info,
    /// 异常或失败事件。
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Error => "error",
        }
    }
}

/// 日志器内部状态。
struct LoggerInner {
    /// 是否启用。
    enabled: bool,
    /// 当前日志文件路径。
    path: PathBuf,
}

/// 进程内事件日志器。
pub struct EventLogger {
    inner: Mutex<LoggerInner>,
}

impl EventLogger {
    fn new() -> Self {
        Self {
            inner: Mutex::new(LoggerInner {
                enabled: false,
                path: default_log_path(),
            }),
        }
    }

    /// 配置启用状态与日志路径。
    pub fn configure(&self, enabled: bool, path: Option<PathBuf>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.enabled = enabled;
            if let Some(path) = path {
                inner.path = path;
            }
        }
    }

    /// 返回当前日志文件路径（无论启用与否）。
    pub fn current_path(&self) -> PathBuf {
        self.inner
            .lock()
            .map(|inner| inner.path.clone())
            .unwrap_or_else(|_| default_log_path())
    }

    /// 返回日志目录（父目录）。
    pub fn current_dir(&self) -> PathBuf {
        self.current_path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// 写入一条事件。关闭时 no-op；任何 IO 错误吞掉。
    pub fn log(&self, level: LogLevel, event: &str, payload: Value) {
        let Ok(inner) = self.inner.lock() else {
            return;
        };
        if !inner.enabled {
            return;
        }
        let path = inner.path.clone();
        drop(inner);

        let sanitized = sanitize_log_value(&payload);
        let line = json!({
            "ts": current_timestamp_iso(),
            "level": level.as_str(),
            "event": event,
            "pid": std::process::id(),
            "payload": sanitized,
        });
        let serialized = match serde_json::to_string(&line) {
            Ok(text) => text,
            Err(_) => return,
        };

        let _ = ensure_parent_dir(&path);
        let _ = rotate_if_needed(&path);

        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = file.write_all(serialized.as_bytes());
            let _ = file.write_all(b"\n");
        }
    }
}

/// 返回全局日志器。
pub fn event_logger() -> &'static EventLogger {
    static LOGGER: OnceLock<EventLogger> = OnceLock::new();
    LOGGER.get_or_init(EventLogger::new)
}

/// 便捷方法：写入 info 事件。
pub fn log_info(event: &str, payload: Value) {
    event_logger().log(LogLevel::Info, event, payload);
}

/// 便捷方法：写入 error 事件。
pub fn log_error(event: &str, payload: Value) {
    event_logger().log(LogLevel::Error, event, payload);
}

/// 返回当前平台默认日志目录。
pub fn default_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BUILDER_PANEL_LOG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "macos")]
    {
        return std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Library")
            .join("Application Support")
            .join("Builder Panel")
            .join("logs");
    }

    #[cfg(target_os = "windows")]
    {
        return std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Builder Panel")
            .join("logs");
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
            .join("logs")
    }
}

/// 返回当前平台默认日志文件路径。
pub fn default_log_path() -> PathBuf {
    default_log_dir().join(DEFAULT_LOG_FILE)
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
    } else {
        Ok(())
    }
}

fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => return Ok(()),
    };
    if metadata.len() < MAX_FILE_BYTES {
        return Ok(());
    }

    // 删除最旧的一份。
    let oldest = rotated_path(path, ROTATION_KEEP);
    let _ = fs::remove_file(&oldest);

    // 依次将 N-1 -> N。
    for index in (1..ROTATION_KEEP).rev() {
        let from = rotated_path(path, index);
        let to = rotated_path(path, index + 1);
        if from.exists() {
            let _ = fs::rename(&from, &to);
        }
    }

    // 当前文件 -> .1。
    let first = rotated_path(path, 1);
    let _ = fs::rename(path, &first);
    Ok(())
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(DEFAULT_LOG_FILE);
    path.with_file_name(format!("{file_name}.{index}"))
}

fn current_timestamp_iso() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let secs = (millis / 1000) as i64;
    let sub_millis = (millis % 1000) as u32;
    format_unix_millis(secs, sub_millis)
}

/// 将 Unix 秒数格式化为 `YYYY-MM-DDTHH:MM:SS.mmmZ`（UTC，避免引入 chrono 依赖）。
fn format_unix_millis(unix_secs: i64, millis: u32) -> String {
    let (year, month, day, hour, minute, second) = unix_to_utc(unix_secs);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
    )
}

fn unix_to_utc(unix_secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let hour = (secs_of_day / 3600) as u32;
    let minute = ((secs_of_day % 3600) / 60) as u32;
    let second = (secs_of_day % 60) as u32;

    // 1970-01-01 是 Unix 纪元。
    let mut year = 1970i32;
    let mut remaining_days = days;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 } as i64;
        if remaining_days >= year_days {
            remaining_days -= year_days;
            year += 1;
        } else if remaining_days < 0 {
            year -= 1;
            let prev_days = if is_leap_year(year) { 366 } else { 365 } as i64;
            remaining_days += prev_days;
        } else {
            break;
        }
    }

    let months = [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    let mut day_in_year = remaining_days as u32;
    for (index, length) in months.iter().enumerate() {
        let mut length = *length;
        if index == 1 && is_leap_year(year) {
            length = 29;
        }
        if day_in_year < length {
            month = (index as u32) + 1;
            break;
        }
        day_in_year -= length;
    }
    let day = day_in_year + 1;

    (year, month, day, hour, minute, second)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "builder-panel-logging-{}-{}",
            std::process::id(),
            // 使用计数器代替随机。
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn disabled_logger_writes_nothing() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");
        let logger = EventLogger::new();
        logger.configure(false, Some(path.clone()));

        logger.log(LogLevel::Info, "测试事件", json!({"foo": "bar"}));

        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enabled_logger_appends_json_line() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");
        let logger = EventLogger::new();
        logger.configure(true, Some(path.clone()));

        logger.log(LogLevel::Info, "设置保存", json!({"prompt": "secret-prompt"}));
        logger.log(LogLevel::Error, "hook 安装失败", json!({"code": 7}));

        let content = fs::read_to_string(&path).expect("log file should exist");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["event"], "设置保存");
        assert_eq!(first["level"], "info");
        // 敏感字段已脱敏。
        assert_eq!(first["payload"]["prompt"], "[已脱敏]");
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["level"], "error");
        assert_eq!(second["payload"]["code"], 7);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotates_when_file_exceeds_threshold() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");
        let big_blob = "x".repeat((MAX_FILE_BYTES as usize) + 16);
        fs::write(&path, big_blob).unwrap();

        let logger = EventLogger::new();
        logger.configure(true, Some(path.clone()));
        logger.log(LogLevel::Info, "触发滚动", json!({}));

        let first_rotated = path.with_file_name("app.log.1");
        assert!(first_rotated.exists());
        let new_size = fs::metadata(&path).unwrap().len();
        assert!(new_size < MAX_FILE_BYTES);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn formats_utc_timestamp() {
        // 2026-06-08T00:00:00Z 对应 1780 + 6 * 365 +... 直接断言一段已知值。
        let formatted = format_unix_millis(1_700_000_000, 123);
        // 1700000000 -> 2023-11-14T22:13:20Z
        assert_eq!(formatted, "2023-11-14T22:13:20.123Z");
    }
}
