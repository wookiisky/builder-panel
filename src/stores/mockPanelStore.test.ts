import { describe, expect, it } from "vitest";

import type { SessionKey, TimelinePage } from "../api/mockPanelContract";
import {
  beginSubmit,
  clearChoiceSelection,
  clearDraft,
  closeTimeline,
  countReplyChars,
  createDefaultMockPanelUiState,
  endSubmit,
  isReplyDraftInvalid,
  openTimeline,
  selectSession,
  sessionKeyToId,
  setTimelinePage,
  timelinePageToCopyText,
  timelineVisibleRange,
  toggleChoiceSelection,
  updateDraft,
} from "./mockPanelStore";

describe("mockPanelStore", () => {
  it("按 session 独立保留草稿", () => {
    const firstKey = sessionKey("project-a", "conversation-a");
    const secondKey = sessionKey("project-a", "conversation-b");
    const state = createDefaultMockPanelUiState();
    const withFirstDraft = updateDraft(state, firstKey, "第一段");
    const withSecondDraft = updateDraft(withFirstDraft, secondKey, "第二段");

    expect(withSecondDraft.draftsBySessionId[sessionKeyToId(firstKey)]).toBe(
      "第一段",
    );
    expect(withSecondDraft.draftsBySessionId[sessionKeyToId(secondKey)]).toBe(
      "第二段",
    );
    expect(state.draftsBySessionId[sessionKeyToId(firstKey)]).toBeUndefined();
  });

  it("发送成功后只清空当前 session 草稿", () => {
    const firstKey = sessionKey("project-a", "conversation-a");
    const secondKey = sessionKey("project-a", "conversation-b");
    const state = updateDraft(
      updateDraft(createDefaultMockPanelUiState(), firstKey, "第一段"),
      secondKey,
      "第二段",
    );
    const nextState = clearDraft(state, firstKey);

    expect(
      nextState.draftsBySessionId[sessionKeyToId(firstKey)],
    ).toBeUndefined();
    expect(nextState.draftsBySessionId[sessionKeyToId(secondKey)]).toBe(
      "第二段",
    );
  });

  it("提交中状态防止同一交互重复提交", () => {
    const submitting = beginSubmit(createDefaultMockPanelUiState(), "reply-1");
    const finished = endSubmit(submitting, null);

    expect(submitting.submittingInteractionId).toBe("reply-1");
    expect(finished.submittingInteractionId).toBeNull();
  });

  it("单选只保留最后一次选择", () => {
    const first = toggleChoiceSelection(
      createDefaultMockPanelUiState(),
      "choice-1",
      "first",
      false,
    );
    const second = toggleChoiceSelection(first, "choice-1", "second", false);

    expect(second.selectedChoicesByInteractionId["choice-1"]).toEqual([
      "second",
    ]);
  });

  it("多选失败路径可保留选择，成功后只清当前交互", () => {
    const first = toggleChoiceSelection(
      createDefaultMockPanelUiState(),
      "choice-1",
      "first",
      true,
    );
    const second = toggleChoiceSelection(first, "choice-1", "second", true);
    const withOther = toggleChoiceSelection(second, "choice-2", "other", true);
    const cleared = clearChoiceSelection(withOther, "choice-1");

    expect(withOther.selectedChoicesByInteractionId["choice-1"]).toEqual([
      "first",
      "second",
    ]);
    expect(cleared.selectedChoicesByInteractionId["choice-1"]).toBeUndefined();
    expect(cleared.selectedChoicesByInteractionId["choice-2"]).toEqual([
      "other",
    ]);
  });

  it("关闭时间线会释放缓存页", () => {
    const key = sessionKey("project-a", "conversation-a");
    const page: TimelinePage = {
      items: [],
      page: 0,
      page_size: 20,
      total: 0,
      has_next: false,
      filter_count: 0,
    };
    const opened = openTimeline(createDefaultMockPanelUiState(), key);
    const cached = setTimelinePage(opened, page);
    const closed = closeTimeline(cached);

    expect(cached.timeline.page).toBe(page);
    expect(closed.timeline.open).toBe(false);
    expect(closed.timeline.page).toBeNull();
    expect(closed.timeline.loading).toBe(false);
    expect(closed.timeline.errorMessage).toBeNull();
  });

  it("切换 session 时关闭已打开的 timeline", () => {
    const firstKey = sessionKey("project-a", "conversation-a");
    const secondKey = sessionKey("project-b", "conversation-b");
    const page: TimelinePage = {
      items: [],
      page: 0,
      page_size: 20,
      total: 0,
      has_next: false,
      filter_count: 0,
    };
    const opened = setTimelinePage(
      openTimeline(createDefaultMockPanelUiState(), firstKey),
      page,
    );
    const selected = selectSession(opened, secondKey);

    expect(selected.selectedSessionId).toBe(sessionKeyToId(secondKey));
    expect(selected.timeline.open).toBe(false);
    expect(selected.timeline.page).toBeNull();
  });

  it("复制筛选结果只包含当前页正文", () => {
    const page: TimelinePage = {
      items: [
        {
          item_id: "item-1",
          session_key: sessionKey("project-a", "conversation-a"),
          kind: "activity",
          title: "读取任务",
          body: "读取用户输入",
          created_at: { value: 1 },
        },
        {
          item_id: "item-2",
          session_key: sessionKey("project-a", "conversation-a"),
          kind: "tool",
          title: "执行工具",
          body: "运行测试",
          created_at: { value: 2 },
        },
      ],
      page: 0,
      page_size: 20,
      total: 2,
      has_next: false,
      filter_count: 1,
    };

    expect(timelinePageToCopyText(page)).toBe("读取用户输入\n\n运行测试");
  });

  it("虚拟列表只计算可见范围", () => {
    const range = timelineVisibleRange(10_000, 4_000, 320, 80, 3);

    expect(range.start).toBe(47);
    expect(range.end).toBe(57);
  });

  it("虚拟列表在滚动位置超过筛选结果时仍返回有效范围", () => {
    const range = timelineVisibleRange(2, 40_000, 320, 80, 3);

    expect(range.start).toBe(0);
    expect(range.end).toBe(2);
  });

  it("回复长度按 Unicode 字符而不是 UTF-16 单元计算", () => {
    const thousandEmoji = "🙂".repeat(1000);
    const overLimit = `${thousandEmoji}🙂`;

    expect(thousandEmoji.length).toBe(2000);
    expect(countReplyChars(thousandEmoji)).toBe(1000);
    expect(isReplyDraftInvalid(thousandEmoji, 1000)).toBe(false);
    expect(isReplyDraftInvalid(overLimit, 1000)).toBe(true);
  });

  const sessionKey = (
    projectId: string,
    conversationId: string,
  ): SessionKey => ({
    agent_kind: "codex_cli",
    project_id: { value: projectId },
    conversation_id: { value: conversationId },
  });
});
