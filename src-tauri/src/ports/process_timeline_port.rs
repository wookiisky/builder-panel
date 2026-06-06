//! 过程事件时间线抽象边界。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::AppError;
use crate::domain::usage::UnixMillis;

/// 过程事件类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessTimelineEventKind {
    /// 普通运行活动。
    Activity,
    /// 工具调用。
    Tool,
    /// 审批相关事件。
    Approval,
    /// 回复相关事件。
    Reply,
    /// 系统边界事件。
    System,
}

/// 过程事件时间线条目。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessTimelineItem {
    /// 条目唯一标识。
    pub item_id: String,
    /// 所属会话。
    pub session_key: SessionKey,
    /// 事件类型。
    pub kind: ProcessTimelineEventKind,
    /// 条目标题。
    pub title: String,
    /// 已清洗正文。
    pub body: String,
    /// 创建时间。
    pub created_at: UnixMillis,
}

/// 过程事件时间线写入端口。
pub trait ProcessTimelineWriterPort {
    /// 接收已清洗的时间线条目，返回是否写入新条目。
    fn record_timeline_item(&mut self, item: ProcessTimelineItem) -> Result<bool, AppError>;
}

/// 过程事件时间线读取端口。
pub trait ProcessTimelineReaderPort {
    /// 读取指定会话的时间线条目。
    fn read_timeline(&self, session_key: &SessionKey)
        -> Result<Vec<ProcessTimelineItem>, AppError>;
}

/// 过程事件时间线缓存释放端口。
pub trait ProcessTimelineReleasePort {
    /// 释放指定会话中已经缓存的大文本正文。
    fn release_large_texts(&mut self, session_key: &SessionKey) -> Result<usize, AppError>;
}
