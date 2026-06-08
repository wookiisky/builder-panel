import type {
  SessionKey,
  TimelineEventKind,
  TimelinePage,
} from "../api/mockPanelContract";

/// 时间线筛选类型。
export type TimelineKindFilter = TimelineEventKind | "all";

/// 时间线 UI 状态。
export interface TimelineUiState {
  /// 是否打开时间线弹层。
  readonly open: boolean;
  /// 当前时间线所属 session ID。
  readonly sessionId: string | null;
  /// 搜索关键词。
  readonly search: string;
  /// 类型筛选。
  readonly kind: TimelineKindFilter;
  /// 当前缓存页。
  readonly page: TimelinePage | null;
  /// 是否正在读取当前筛选页。
  readonly loading: boolean;
  /// 当前时间线错误消息。
  readonly errorMessage: string | null;
}

/// Mock panel UI 状态。
export interface MockPanelUiState {
  /// 当前选中 session ID。
  readonly selectedSessionId: string | null;
  /// 每个 session 独立保存的回复草稿。
  readonly draftsBySessionId: Readonly<Record<string, string>>;
  /// 当前提交中的交互 ID。
  readonly submittingInteractionId: string | null;
  /// 每个交互独立保存的选项选择。
  readonly selectedChoicesByInteractionId: Readonly<
    Record<string, readonly string[]>
  >;
  /// 最近一次错误消息。
  readonly errorMessage: string | null;
  /// 时间线 UI 状态。
  readonly timeline: TimelineUiState;
}

/// 创建默认 mock panel UI 状态。
export const createDefaultMockPanelUiState = (): MockPanelUiState => ({
  selectedSessionId: null,
  draftsBySessionId: {},
  submittingInteractionId: null,
  selectedChoicesByInteractionId: {},
  errorMessage: null,
  timeline: {
    open: false,
    sessionId: null,
    search: "",
    kind: "all",
    page: null,
    loading: false,
    errorMessage: null,
  },
});

/// 切换单选或多选选项。
export const toggleChoiceSelection = (
  state: MockPanelUiState,
  interactionId: string,
  choiceValue: string,
  allowsMultiple: boolean,
): MockPanelUiState => {
  const currentValues =
    state.selectedChoicesByInteractionId[interactionId] ?? [];
  const nextValues = allowsMultiple
    ? toggleMultipleChoice(currentValues, choiceValue)
    : [choiceValue];

  return {
    ...state,
    selectedChoicesByInteractionId: {
      ...state.selectedChoicesByInteractionId,
      [interactionId]: nextValues,
    },
  };
};

/// 清空指定交互的选项选择。
export const clearChoiceSelection = (
  state: MockPanelUiState,
  interactionId: string,
): MockPanelUiState => {
  const nextChoices = { ...state.selectedChoicesByInteractionId };
  delete nextChoices[interactionId];

  return {
    ...state,
    selectedChoicesByInteractionId: nextChoices,
  };
};

/// 将 session key 转成前端稳定 ID。
export const sessionKeyToId = (sessionKey: SessionKey): string => {
  return [
    sessionKey.agent_kind,
    sessionKey.project_id.value,
    sessionKey.conversation_id.value,
  ].join("::");
};

/// 选择 session。
export const selectSession = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): MockPanelUiState => ({
  ...state,
  selectedSessionId: sessionKeyToId(sessionKey),
  errorMessage: null,
  timeline: {
    open: false,
    sessionId: null,
    search: "",
    kind: "all",
    page: null,
    loading: false,
    errorMessage: null,
  },
});

/// 更新指定 session 草稿。
export const updateDraft = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
  draft: string,
): MockPanelUiState => ({
  ...state,
  draftsBySessionId: {
    ...state.draftsBySessionId,
    [sessionKeyToId(sessionKey)]: draft,
  },
});

/// 清空指定 session 草稿。
export const clearDraft = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): MockPanelUiState => {
  const nextDrafts = { ...state.draftsBySessionId };
  delete nextDrafts[sessionKeyToId(sessionKey)];

  return {
    ...state,
    draftsBySessionId: nextDrafts,
  };
};

/// 标记交互提交开始。
export const beginSubmit = (
  state: MockPanelUiState,
  interactionId: string,
): MockPanelUiState => ({
  ...state,
  submittingInteractionId: interactionId,
  errorMessage: null,
});

/// 标记交互提交结束。
export const endSubmit = (
  state: MockPanelUiState,
  errorMessage: string | null,
): MockPanelUiState => ({
  ...state,
  submittingInteractionId: null,
  errorMessage,
});

/// 打开指定 session 的时间线。
export const openTimeline = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): MockPanelUiState => ({
  ...state,
  timeline: {
    open: true,
    sessionId: sessionKeyToId(sessionKey),
    search: "",
    kind: "all",
    page: null,
    loading: false,
    errorMessage: null,
  },
});

/// 关闭时间线并释放缓存页。
export const closeTimeline = (state: MockPanelUiState): MockPanelUiState => ({
  ...state,
  timeline: {
    open: false,
    sessionId: null,
    search: "",
    kind: "all",
    page: null,
    loading: false,
    errorMessage: null,
  },
});

/// 更新时间线搜索关键词。
export const updateTimelineSearch = (
  state: MockPanelUiState,
  search: string,
): MockPanelUiState => ({
  ...state,
  timeline: {
    ...state.timeline,
    search,
    page: null,
    errorMessage: null,
  },
});

/// 更新时间线类型筛选。
export const updateTimelineKind = (
  state: MockPanelUiState,
  kind: TimelineKindFilter,
): MockPanelUiState => ({
  ...state,
  timeline: {
    ...state.timeline,
    kind,
    page: null,
    errorMessage: null,
  },
});

/// 标记时间线开始加载。
export const beginTimelineLoad = (
  state: MockPanelUiState,
): MockPanelUiState => ({
  ...state,
  timeline: {
    ...state.timeline,
    loading: true,
    errorMessage: null,
  },
});

/// 标记时间线加载失败。
export const failTimelineLoad = (
  state: MockPanelUiState,
  errorMessage: string,
): MockPanelUiState => ({
  ...state,
  timeline: {
    ...state.timeline,
    loading: false,
    errorMessage,
  },
});

/// 写入时间线当前页缓存。
export const setTimelinePage = (
  state: MockPanelUiState,
  page: TimelinePage,
): MockPanelUiState => ({
  ...state,
  timeline: {
    ...state.timeline,
    page,
    loading: false,
    errorMessage: null,
  },
});

/// 将当前筛选结果格式化为可复制文本。
export const timelinePageToCopyText = (page: TimelinePage): string => {
  return page.items.map((item) => item.body).join("\n\n");
};

/// 计算虚拟列表可见范围。
export const timelineVisibleRange = (
  itemCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  overscan: number,
): { readonly start: number; readonly end: number } => {
  if (itemCount <= 0 || rowHeight <= 0 || viewportHeight <= 0) {
    return { start: 0, end: 0 };
  }

  const visibleCount = Math.ceil(viewportHeight / rowHeight);
  const maxFirstVisible = Math.max(0, itemCount - visibleCount);
  const firstVisible = Math.min(
    Math.floor(Math.max(0, scrollTop) / rowHeight),
    maxFirstVisible,
  );
  const start = Math.max(0, firstVisible - overscan);
  const end = Math.min(itemCount, firstVisible + visibleCount + overscan);

  return { start, end };
};

/// 计算与 Rust chars().count() 对齐的前端字符数。
export const countReplyChars = (value: string): number => {
  return Array.from(value.trim()).length;
};

/// 判断回复草稿是否不能发送。
export const isReplyDraftInvalid = (
  value: string,
  maxChars: number,
): boolean => {
  const charCount = countReplyChars(value);

  return charCount === 0 || charCount > maxChars;
};

/// 切换多选集合。
const toggleMultipleChoice = (
  currentValues: readonly string[],
  choiceValue: string,
): readonly string[] => {
  if (currentValues.includes(choiceValue)) {
    return currentValues.filter((value) => value !== choiceValue);
  }

  return [...currentValues, choiceValue];
};
