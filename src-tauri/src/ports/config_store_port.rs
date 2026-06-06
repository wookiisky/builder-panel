//! 本地配置存储抽象边界。

use crate::domain::app_error::AppError;
use crate::services::settings_service::BuilderPanelSettings;

/// Builder Panel 设置存储端口。
pub trait SettingsStorePort {
    /// 读取设置；配置不存在时返回空值。
    fn load_settings(&self) -> Result<Option<BuilderPanelSettings>, AppError>;

    /// 保存设置。
    fn save_settings(&self, settings: &BuilderPanelSettings) -> Result<(), AppError>;
}
