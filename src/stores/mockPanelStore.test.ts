import { describe, expect, it } from "vitest";

import type { SessionKey } from "../api/mockPanelContract";
import {
  beginSubmit,
  clearChoiceSelection,
  clearDraft,
  clearFollowupSessionExpansion,
  countReplyChars,
  createDefaultMockPanelUiState,
  endSubmit,
  isFollowupSessionExpanded,
  isReplyDraftInvalid,
  selectSession,
  sessionKeyToId,
  toggleChoiceSelection,
  toggleFollowupSessionExpansion,
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

  it("切换 session 只更新选择，不影响 follow-up 展开状态", () => {
    const firstKey = sessionKey("project-a", "conversation-a");
    const secondKey = sessionKey("project-b", "conversation-b");
    const expanded = toggleFollowupSessionExpansion(
      createDefaultMockPanelUiState(),
      firstKey,
    );
    const selected = selectSession(expanded, secondKey);

    expect(selected.selectedSessionId).toBe(sessionKeyToId(secondKey));
    expect(isFollowupSessionExpanded(selected, firstKey)).toBe(true);
  });

  it("可切换并清理 follow-up 展开状态", () => {
    const key = sessionKey("project-a", "conversation-a");
    const state = createDefaultMockPanelUiState();
    const expanded = toggleFollowupSessionExpansion(state, key);
    const collapsed = toggleFollowupSessionExpansion(expanded, key);
    const cleared = clearFollowupSessionExpansion(expanded, key);

    expect(isFollowupSessionExpanded(state, key)).toBe(false);
    expect(isFollowupSessionExpanded(expanded, key)).toBe(true);
    expect(isFollowupSessionExpanded(collapsed, key)).toBe(false);
    expect(isFollowupSessionExpanded(cleared, key)).toBe(false);
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
