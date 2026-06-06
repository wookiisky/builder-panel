//! 系统通知 adapter。

use std::cell::RefCell;

use crate::domain::app_error::AppError;
use crate::ports::notification_port::NotificationPort;
use crate::services::notification_service::NotificationPlan;

/// 记录型通知 adapter，用于测试和未接入系统通知时的降级验证。
pub struct RecordingNotificationAdapter {
    /// 已发送通知计划。
    sent: RefCell<Vec<NotificationPlan>>,
}

impl RecordingNotificationAdapter {
    /// 创建记录型通知 adapter。
    pub fn new() -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
        }
    }

    /// 读取已记录通知。
    pub fn sent_notifications(&self) -> Vec<NotificationPlan> {
        self.sent.borrow().clone()
    }
}

impl Default for RecordingNotificationAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationPort for RecordingNotificationAdapter {
    fn show_notification(&self, plan: &NotificationPlan) -> Result<(), AppError> {
        self.sent.borrow_mut().push(plan.clone());
        Ok(())
    }
}
