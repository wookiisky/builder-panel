//! 过程事件时间线应用服务。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::AppError;
use crate::ports::process_timeline_port::{
    ProcessTimelineEventKind, ProcessTimelineItem, ProcessTimelineReaderPort,
};

/// 时间线单页最大条目数。
pub const MAX_TIMELINE_PAGE_SIZE: usize = 50;

/// 时间线查询请求。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimelineQuery {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 页码，从 0 开始。
    pub page: usize,
    /// 每页条目数。
    pub page_size: usize,
    /// 搜索关键词。
    pub search: Option<String>,
    /// 类型筛选。
    pub kind: Option<ProcessTimelineEventKind>,
}

/// 时间线分页结果。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimelinePage {
    /// 当前页条目。
    pub items: Vec<ProcessTimelineItem>,
    /// 页码，从 0 开始。
    pub page: usize,
    /// 每页条目数。
    pub page_size: usize,
    /// 过滤后的总条目数。
    pub total: usize,
    /// 是否还有下一页。
    pub has_next: bool,
    /// 当前启用过滤器数量。
    pub filter_count: usize,
}

/// 过程事件时间线应用服务。
pub struct ProcessTimelineService<'a, R: ProcessTimelineReaderPort> {
    /// 时间线读取端口。
    reader: &'a R,
}

impl<'a, R: ProcessTimelineReaderPort> ProcessTimelineService<'a, R> {
    /// 创建过程事件时间线应用服务。
    pub fn new(reader: &'a R) -> Self {
        Self { reader }
    }

    /// 查询过程事件时间线。
    pub fn query_timeline(&self, query: TimelineQuery) -> Result<TimelinePage, AppError> {
        let page_size = query.page_size.clamp(1, MAX_TIMELINE_PAGE_SIZE);
        let search = normalized_search(query.search.as_deref());
        let mut filter_count = 0;
        if search.is_some() {
            filter_count += 1;
        }
        if query.kind.is_some() {
            filter_count += 1;
        }

        let filtered = self
            .reader
            .read_timeline(&query.session_key)?
            .into_iter()
            .filter(|item| matches_kind(item, query.kind.as_ref()))
            .filter(|item| matches_search(item, search.as_deref()))
            .collect::<Vec<_>>();

        let total = filtered.len();
        let start = query.page.saturating_mul(page_size);
        let items = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .collect::<Vec<_>>();
        let has_next = start.saturating_add(items.len()) < total;

        Ok(TimelinePage {
            items,
            page: query.page,
            page_size,
            total,
            has_next,
            filter_count,
        })
    }
}

/// 标准化搜索关键词。
fn normalized_search(value: Option<&str>) -> Option<String> {
    let text = value?.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }

    Some(text)
}

/// 判断条目类型是否匹配。
fn matches_kind(item: &ProcessTimelineItem, kind: Option<&ProcessTimelineEventKind>) -> bool {
    match kind {
        Some(kind) => &item.kind == kind,
        None => true,
    }
}

/// 判断条目搜索是否匹配。
fn matches_search(item: &ProcessTimelineItem, search: Option<&str>) -> bool {
    let Some(search) = search else {
        return true;
    };
    let title = item.title.to_lowercase();
    let body = item.body.to_lowercase();

    title.contains(search) || body.contains(search)
}

#[cfg(test)]
mod tests {
    use super::{ProcessTimelineService, TimelineQuery};
    use crate::adapters::mock_agent::MockAgentRuntime;
    use crate::ports::process_timeline_port::ProcessTimelineEventKind;

    #[test]
    fn query_paginates_timeline() {
        let runtime = MockAgentRuntime::stage3_default();
        let key = runtime
            .session_list()
            .into_iter()
            .find(|item| item.conversation_label == "审批闭环")
            .expect("approval session should exist")
            .session_key;
        let service = ProcessTimelineService::new(&runtime);
        let page = service
            .query_timeline(TimelineQuery {
                session_key: key,
                page: 0,
                page_size: 1,
                search: None,
                kind: None,
            })
            .expect("timeline should load");

        assert_eq!(page.items.len(), 1);
        assert!(page.has_next);
    }

    #[test]
    fn query_filters_by_search_and_kind() {
        let runtime = MockAgentRuntime::stage3_default();
        let key = runtime
            .session_list()
            .into_iter()
            .find(|item| item.conversation_label == "审批闭环")
            .expect("approval session should exist")
            .session_key;
        let service = ProcessTimelineService::new(&runtime);
        let page = service
            .query_timeline(TimelineQuery {
                session_key: key,
                page: 0,
                page_size: 20,
                search: Some("等待".to_string()),
                kind: Some(ProcessTimelineEventKind::Approval),
            })
            .expect("timeline should load");

        assert_eq!(page.total, 1);
        assert_eq!(page.filter_count, 2);
        assert_eq!(page.items[0].kind, ProcessTimelineEventKind::Approval);
    }
}
