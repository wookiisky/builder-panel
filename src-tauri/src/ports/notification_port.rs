//! 系统通知抽象边界。

use crate::domain::app_error::AppError;
use crate::services::notification_service::NotificationPlan;

/// 系统通知发送端口。
pub trait NotificationPort {
    /// 展示通知计划。
    fn show_notification(&self, plan: &NotificationPlan) -> Result<(), AppError>;
}
