import type { SessionKey } from "../api/mockPanelContract";

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
  /// 已手动展开 follow-up 输入区的 session。
  readonly expandedFollowupSessionIds: Readonly<Record<string, true>>;
}

/// 创建默认 mock panel UI 状态。
export const createDefaultMockPanelUiState = (): MockPanelUiState => ({
  selectedSessionId: null,
  draftsBySessionId: {},
  submittingInteractionId: null,
  selectedChoicesByInteractionId: {},
  errorMessage: null,
  expandedFollowupSessionIds: {},
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

/// 切换 completed 或 failed session 的 follow-up 输入区。
export const toggleFollowupSessionExpansion = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): MockPanelUiState => {
  const sessionId = sessionKeyToId(sessionKey);
  const nextExpanded = { ...state.expandedFollowupSessionIds };
  if (nextExpanded[sessionId]) {
    delete nextExpanded[sessionId];
  } else {
    nextExpanded[sessionId] = true;
  }

  return {
    ...state,
    expandedFollowupSessionIds: nextExpanded,
  };
};

/// 清理指定 session 的 follow-up 展开状态。
export const clearFollowupSessionExpansion = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): MockPanelUiState => {
  const sessionId = sessionKeyToId(sessionKey);
  if (!state.expandedFollowupSessionIds[sessionId]) {
    return state;
  }

  const nextExpanded = { ...state.expandedFollowupSessionIds };
  delete nextExpanded[sessionId];

  return {
    ...state,
    expandedFollowupSessionIds: nextExpanded,
  };
};

/// 判断指定 session 的 follow-up 输入区是否已展开。
export const isFollowupSessionExpanded = (
  state: MockPanelUiState,
  sessionKey: SessionKey,
): boolean => {
  return state.expandedFollowupSessionIds[sessionKeyToId(sessionKey)] === true;
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
