//! Tauri 事件出口。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::ports::session_update_port::{
    SessionUpdateNotification, SessionUpdateSinkPort, SESSION_UPDATED_EVENT,
};

/// panel 探针变化事件名。
pub const PANEL_PROBE_CHANGED_EVENT: &str = "panel_probe_changed";

const SESSION_UPDATE_DEBOUNCE: Duration = Duration::from_millis(200);

/// Tauri session 更新发布器。
pub struct TauriSessionUpdateSink {
    /// Tauri app 句柄。
    app_handle: AppHandle,
    /// 按 session 合并的最近通知。
    pending: Arc<Mutex<BTreeMap<String, SessionUpdateNotification>>>,
    /// 已安排发送任务的 session。
    scheduled: Arc<Mutex<BTreeSet<String>>>,
}

impl TauriSessionUpdateSink {
    /// 创建 Tauri session 更新发布器。
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            scheduled: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }
}

impl SessionUpdateSinkPort for TauriSessionUpdateSink {
    fn publish_session_update(&self, notification: SessionUpdateNotification) {
        let key = notification_key(&notification);
        let Ok(mut pending) = self.pending.lock() else {
            return;
        };
        pending.insert(key.clone(), notification);
        drop(pending);

        let Ok(mut scheduled) = self.scheduled.lock() else {
            return;
        };
        if !scheduled.insert(key.clone()) {
            return;
        }
        drop(scheduled);

        let app_handle = self.app_handle.clone();
        let pending = Arc::clone(&self.pending);
        let scheduled = Arc::clone(&self.scheduled);
        thread::spawn(move || {
            thread::sleep(SESSION_UPDATE_DEBOUNCE);
            let notification = pending
                .lock()
                .ok()
                .and_then(|mut pending| pending.remove(&key));
            if let Ok(mut scheduled) = scheduled.lock() {
                scheduled.remove(&key);
            }
            if let Some(notification) = notification {
                let _ = app_handle.emit(SESSION_UPDATED_EVENT, notification);
            }
        });
    }
}

/// 返回 session 级节流键。
fn notification_key(notification: &SessionUpdateNotification) -> String {
    format!(
        "{:?}:{}:{}",
        notification.runtime_source,
        notification.session_key.project_id.value,
        notification.session_key.conversation_id.value
    )
}
