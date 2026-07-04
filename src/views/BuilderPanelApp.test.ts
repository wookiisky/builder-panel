import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";

import {
  actionLabel,
  applyLatestSavedPanelWindowPreferences,
  aggregateToolUsage,
  applyPanelWindowControlError,
  canToggleFollowupRow,
  canJumpOnSessionClick,
  canSelectOnSessionClick,
  countSessionsByStatus,
  createSessionRefreshScheduler,
  elapsedDurationLabel,
  fetchSessionsForSource,
  isLatestSettingsSaveResponse,
  isCodexCliRuntime,
  isCodexAppRuntime,
  defaultHookAgentStatuses,
  isHookActionDisabled,
  mergePanelSessionsByCaptureOrder,
  mergePanelWindowStateUpdate,
  panelSessionToId,
  parseTooltipMarkdown,
  positionTooltipPanel,
  relativePastLabel,
  selectPanelSession,
  selectFirstSessionWhenMissing,
  SessionDetail,
  SessionStream,
  sessionActionRowClassName,
  shouldAutoShowSessionActionRow,
  shouldShowSessionActionRow,
  shouldSubmitReplyOnKeyDown,
  shouldUseFollowupShortcut,
  sourceTag,
  stopTooltipPortalEvent,
  sessionSideTimeLabel,
  textDisplayParagraph,
  type PanelSessionListItem,
} from "./BuilderPanelApp";
import type {
  SessionDetailViewModel,
  SessionKey,
} from "../api/mockPanelContract";
import { defaultSettings } from "../api/settingsApi";
import { createDefaultMockPanelUiState } from "../stores/mockPanelStore";

const codexSessionKey: SessionKey = {
  agent_kind: "codex_cli",
  project_id: { value: "/tmp/builder-panel" },
  conversation_id: { value: "codex-session-1" },
};

const codexAppSessionKey: SessionKey = {
  agent_kind: "codex_app",
  project_id: { value: "/tmp/builder-panel-app" },
  conversation_id: { value: "codex-app-session-1" },
};

describe("BuilderPanelApp session refresh", () => {
  it("selects a codex session that appears after the initial empty refresh", () => {
    const emptyState = createDefaultMockPanelUiState();
    const stateAfterEmptyRefresh = selectFirstSessionWhenMissing(
      emptyState,
      [],
    );

    const stateAfterCodexRefresh = selectFirstSessionWhenMissing(
      stateAfterEmptyRefresh,
      [sessionItem(codexSessionKey, "codex_cli")],
    );

    expect(stateAfterEmptyRefresh.selectedSessionId).toBeNull();
    expect(stateAfterCodexRefresh.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexSessionKey, "codex_cli")),
    );
  });

  it("selects a codex app session that appears after the initial empty refresh", () => {
    const emptyState = createDefaultMockPanelUiState();
    const stateAfterEmptyRefresh = selectFirstSessionWhenMissing(
      emptyState,
      [],
    );

    const stateAfterCodexAppRefresh = selectFirstSessionWhenMissing(
      stateAfterEmptyRefresh,
      [sessionItem(codexAppSessionKey, "codex_app")],
    );

    expect(stateAfterEmptyRefresh.selectedSessionId).toBeNull();
    expect(stateAfterCodexAppRefresh.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexAppSessionKey, "codex_app")),
    );
  });

  it("keeps the user selected session during later refreshes", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(codexAppSessionKey, "codex_app"),
    );

    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
      sessionItem(codexAppSessionKey, "codex_app"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexAppSessionKey, "codex_app")),
    );
  });

  it("selects the first available session when the previous selection disappears", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(codexAppSessionKey, "codex_app"),
    );
    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexSessionKey, "codex_cli")),
    );
  });

  it("routes by runtime source instead of agent kind", () => {
    const codexAppRuntimeSession = sessionItem(codexSessionKey, "codex_app");
    const codexCliRuntimeSession = sessionItem(codexSessionKey, "codex_cli");
    const codexAppSession = sessionItem(
      {
        ...codexSessionKey,
        agent_kind: "codex_app",
      },
      "codex_app",
    );

    expect(isCodexCliRuntime(codexAppRuntimeSession)).toBe(false);
    expect(isCodexCliRuntime(codexCliRuntimeSession)).toBe(true);
    expect(isCodexAppRuntime(codexAppSession)).toBe(true);
    expect(isCodexCliRuntime(codexAppSession)).toBe(false);
  });

  it("shows runtime source labels with Codex app branding", () => {
    expect(sourceTag(sessionItem(codexSessionKey, "codex_app"))).toBe("Codex");
    expect(sourceTag(sessionItem(codexSessionKey, "codex_cli"))).toBe(
      "Codex CLI",
    );
  });

  it("keeps runtime source in selected session identity", () => {
    const codexAppRuntimeSession = sessionItem(codexSessionKey, "codex_app");
    const codexCliRuntimeSession = sessionItem(codexSessionKey, "codex_cli");
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      codexAppRuntimeSession,
    );
    const sessions = [codexCliRuntimeSession, codexAppRuntimeSession];

    const selectedSession = sessions.find(
      (session) =>
        panelSessionToId(session) === selectedState.selectedSessionId,
    );

    expect(panelSessionToId(codexAppRuntimeSession)).not.toBe(
      panelSessionToId(codexCliRuntimeSession),
    );
    expect(selectedSession?.runtimeSource).toBe("codex_app");
  });

  it("only jumps on row click when jump action exists", () => {
    const settings = defaultSettings();

    expect(canJumpOnSessionClick(["jump"], settings)).toBe(true);
    expect(canJumpOnSessionClick(["send_reply"], settings)).toBe(false);
    expect(
      canJumpOnSessionClick(["jump"], {
        ...settings,
        terminal: {
          ...settings.terminal,
          jump_enabled: false,
        },
      }),
    ).toBe(false);
  });

  it("can select a jump-capable row even when terminal jump is disabled", () => {
    expect(canSelectOnSessionClick(["jump"])).toBe(true);
    expect(canSelectOnSessionClick(["send_reply"])).toBe(false);
  });

  it("returns empty source sessions when one runtime fetch fails", async () => {
    const sessions = await fetchSessionsForSource(true, async () => {
      throw new Error("Codex APP 不可用");
    });
    const disabledSessions = await fetchSessionsForSource(false, async () => [
      sessionItem(codexAppSessionKey, "codex_app"),
    ]);

    expect(sessions).toEqual([]);
    expect(disabledSessions).toEqual([]);
  });

  it("keeps captured session order stable and prepends newly captured sessions", () => {
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      3000,
    );
    const olderWaiting = sessionItem(
      sessionKey("project-a", "older-waiting"),
      "codex_app",
      "waiting_for_answer",
      "等待回复",
      1000,
    );
    const newerWaiting = sessionItem(
      sessionKey("project-a", "newer-waiting"),
      "codex_app",
      "waiting_for_approval",
      "等待审批",
      2000,
    );

    const initiallyCaptured = mergePanelSessionsByCaptureOrder(
      [],
      [running, olderWaiting],
    );
    const refreshed = mergePanelSessionsByCaptureOrder(initiallyCaptured, [
      newerWaiting,
      olderWaiting,
      running,
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "newer-waiting",
      "running",
      "older-waiting",
    ]);
  });

  it("updates an existing session summary without changing capture order", () => {
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const waiting = sessionItem(
      sessionKey("project-a", "waiting"),
      "codex_app",
      "waiting_for_answer",
      "等待回复",
      1000,
    );
    const captured = mergePanelSessionsByCaptureOrder([], [running, waiting]);
    const refreshed = mergePanelSessionsByCaptureOrder(captured, [
      {
        ...waiting,
        summary: textDisplay("新的 Agent 摘要"),
      },
      {
        ...running,
        summary: textDisplay("运行中 Agent 摘要"),
      },
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "running",
      "waiting",
    ]);
    expect(refreshed[1].summary.full_text).toBe("新的 Agent 摘要");
  });

  it("keeps Codex APP parent-child block together without reordering unrelated CLI sessions", () => {
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const cli = sessionItem(
      sessionKey("project-b", "cli"),
      "codex_cli",
      "running",
      "运行中",
      1000,
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      indent_level: 1,
    };
    const previous = mergePanelSessionsByCaptureOrder([], [parent, cli]);
    const refreshed = mergePanelSessionsByCaptureOrder(previous, [
      parent,
      child,
      cli,
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "parent",
      "child",
      "cli",
    ]);
    expect(refreshed[1].indent_level).toBe(1);
  });

  it("renders the refreshed list row summary for the same session", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: textDisplay("旧摘要"),
      actions: ["jump"] as const,
    };
    const refreshed = {
      ...session,
      summary: textDisplay("新的 Agent 摘要"),
    };
    const container = document.createElement("div");
    const root = createRoot(container);
    const renderStream = (items: readonly PanelSessionListItem[]) =>
      createElement(SessionStream, {
        mockUiState: createDefaultMockPanelUiState(),
        currentTimeMs: 2000,
        sessions: items,
        settings: defaultSettings(),
        selectedSessionId: null,
        onCreateFollowupTurn: () => undefined,
        onDraftChange: () => undefined,
        onJump: () => undefined,
        onResolveApproval: () => undefined,
        onSendReply: () => undefined,
        onSubmitChoice: () => undefined,
        onToggleChoice: () => undefined,
      });

    await act(async () => {
      root.render(renderStream([session]));
    });
    expect(container.textContent).toContain("旧摘要");

    await act(async () => {
      root.render(renderStream([refreshed]));
    });
    expect(container.textContent).toContain("新的 Agent 摘要");

    await act(async () => {
      root.unmount();
    });
  });

  it("renders child session with one-level indent", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "running",
        "运行中",
      ),
      indent_level: 1,
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [parent, child],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const rows = container.querySelectorAll<HTMLElement>(".session-table-row");
    expect(rows[0].style.getPropertyValue("--session-indent")).toBe("0px");
    expect(rows[1].style.getPropertyValue("--session-indent")).toBe("14px");

    await act(async () => {
      root.unmount();
    });
  });

  it("renders full tooltip paragraph even when the row summary is truncated", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: {
        text: "截断",
        full_text: "第一段完整内容\n\n第二段完整内容很长",
        truncated: true,
        max_chars: 3,
      },
    };
    const settings = {
      ...defaultSettings(),
      display: {
        ...defaultSettings().display,
        summary_tooltip_paragraphs: 1,
      },
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings,
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const summaryTooltip = container.querySelector(".session-summary-tooltip");
    expect(summaryTooltip?.textContent).toBe("第二段");

    await act(async () => {
      summaryTooltip?.dispatchEvent(
        new MouseEvent("mouseover", { bubbles: true }),
      );
    });

    expect(document.body.textContent).toContain("第二段完整内容很长");

    await act(async () => {
      root.unmount();
    });
  });

  it("runs a queued refresh after the current refresh finishes", async () => {
    const resolvers: Array<() => void> = [];
    const refresh = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolvers.push(resolve);
        }),
    );
    const scheduler = createSessionRefreshScheduler({
      eventDelayMs: 350,
      refresh,
      reportError: () => undefined,
      scheduleTimeout: window.setTimeout,
      clearTimeout: window.clearTimeout,
    });

    scheduler.requestImmediate();
    scheduler.requestEvent();

    expect(refresh).toHaveBeenCalledTimes(1);
    resolvers.shift()?.();
    await flushPromises();

    expect(refresh).toHaveBeenCalledTimes(2);
    resolvers.shift()?.();
    await flushPromises();
    scheduler.dispose();
  });

  it("throttles continuous session update events without waiting for silence", async () => {
    vi.useFakeTimers();
    const refresh = vi.fn<() => Promise<void>>(async () => undefined);
    const scheduler = createSessionRefreshScheduler({
      eventDelayMs: 350,
      refresh,
      reportError: () => undefined,
      scheduleTimeout: window.setTimeout,
      clearTimeout: window.clearTimeout,
    });

    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    expect(refresh).not.toHaveBeenCalled();

    vi.advanceTimersByTime(150);
    await flushPromises();
    expect(refresh).toHaveBeenCalledTimes(1);

    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    vi.advanceTimersByTime(250);
    await flushPromises();
    expect(refresh).toHaveBeenCalledTimes(2);

    scheduler.dispose();
    vi.useRealTimers();
  });

  it("counts waiting and running sessions for the expanded panel summary", () => {
    const counts = countSessionsByStatus([
      sessionItem(
        sessionKey("project-a", "approval"),
        "codex_app",
        "waiting_for_approval",
        "等待审批",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "answer"),
        "codex_app",
        "waiting_for_answer",
        "等待回复",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "running"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "completed"),
        "codex_app",
        "completed",
        "已完成",
        1000,
      ),
    ]);

    expect(counts).toEqual({ waiting: 2, running: 1 });
  });

  it("aggregates tool usage by latest source value without summing sessions", () => {
    const olderCodex = {
      ...sessionItem(
        sessionKey("project-a", "codex-old"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      usage_5h: verifiedUsage("10 tokens", "codex-account", 1000),
    };
    const newerCodex = {
      ...sessionItem(
        sessionKey("project-a", "codex-new"),
        "codex_app",
        "running",
        "运行中",
        2000,
      ),
      usage_5h: verifiedUsage("20 tokens", "codex-account", 2000),
    };
    const usage = aggregateToolUsage([olderCodex, newerCodex]);

    expect(usage).toEqual([
      {
        family: "Codex",
        usage5h: "20 tokens",
        weekly: "--",
        tooltip: "test 20 tokens",
      },
    ]);
  });

  it("ignores session scoped usage in tool usage summary", () => {
    const codexSessionUsage = {
      ...sessionItem(
        sessionKey("project-a", "codex-session"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      usage_5h: verifiedUsage("99 tokens", "codex-session", 1000, "session"),
    };

    const usage = aggregateToolUsage([codexSessionUsage]);

    expect(usage).toEqual([]);
  });

  it("submits reply on Enter but keeps Shift Enter for multiline input", () => {
    expect(shouldSubmitReplyOnKeyDown(true, "Enter", false)).toBe(true);
    expect(shouldSubmitReplyOnKeyDown(true, "Enter", true)).toBe(false);
    expect(shouldSubmitReplyOnKeyDown(false, "Enter", false)).toBe(false);
    expect(shouldSubmitReplyOnKeyDown(true, "A", false)).toBe(false);
  });

  it("builds default tooltip from the most recent five full paragraphs", () => {
    const display = textDisplayParagraph({
      text: "第六段",
      full_text:
        "第一段\n\n第二段\n\n第三段\n\n第四段\n\n第五段\n\n第六段内容很长",
      truncated: false,
      max_chars: 4,
    });

    expect(display.visibleText).toBe("第六段内");
    expect(display.fullParagraph).toBe("第六段内容很长");
    expect(display.paragraphTruncated).toBe(true);
    expect(display.tooltipText).toBe(
      "第二段\n\n第三段\n\n第四段\n\n第五段\n\n第六段内容很长",
    );
  });

  it("keeps line breaks inside the full tooltip paragraphs", () => {
    const display = textDisplayParagraph({
      text: "第一段",
      full_text: "第一段\n\n- 第一项\n- **第二项**\n继续说明",
      truncated: false,
      max_chars: 8,
    });

    expect(display.visibleText).toBe("- 第一项\n- ");
    expect(display.fullParagraph).toBe("- 第一项\n- **第二项**\n继续说明");
    expect(display.tooltipText).toBe(
      "第一段\n\n- 第一项\n- **第二项**\n继续说明",
    );
  });

  it("keeps tooltip text independent from display text and max-char truncation", () => {
    const display = textDisplayParagraph({
      text: "截断展示",
      full_text: "第一段完整内容\n\n第二段完整内容很长",
      truncated: true,
      max_chars: 3,
    });

    expect(display.visibleText).toBe("第二段");
    expect(display.paragraphTruncated).toBe(true);
    expect(display.tooltipText).toBe("第一段完整内容\n\n第二段完整内容很长");
  });

  it("keeps tooltip text even when the paragraph is not max-char truncated", () => {
    const display = textDisplayParagraph({
      text: "短文本",
      full_text: "短文本但可能被行宽省略",
      truncated: false,
      max_chars: 100,
    });

    expect(display.visibleText).toBe("短文本但可能被行宽省略");
    expect(display.paragraphTruncated).toBe(false);
    expect(display.tooltipText).toBe("短文本但可能被行宽省略");
  });

  it("builds tooltip from the most recent paragraphs while showing only the last", () => {
    const display = textDisplayParagraph(
      {
        text: "第三段",
        full_text: "第一段\n\n第二段\n\n第三段",
        truncated: false,
        max_chars: 100,
      },
      2,
    );

    expect(display.visibleText).toBe("第三段");
    expect(display.fullParagraph).toBe("第三段");
    expect(display.tooltipText).toBe("第二段\n\n第三段");
  });

  it("returns all paragraphs when fewer than the requested tooltip count", () => {
    const display = textDisplayParagraph(
      {
        text: "第二段",
        full_text: "第一段\n\n第二段",
        truncated: false,
        max_chars: 100,
      },
      5,
    );

    expect(display.visibleText).toBe("第二段");
    expect(display.tooltipText).toBe("第一段\n\n第二段");
  });

  it("parses tooltip markdown blocks for rendered session tooltips", () => {
    expect(
      parseTooltipMarkdown(
        "## 标题\n\n- **允许**\n- `拒绝`\n\n> 保留\n> 换行\n\n```txt\nline 1\nline 2\n```",
      ),
    ).toEqual([
      { kind: "heading", level: 2, text: "标题" },
      { kind: "unordered-list", items: ["**允许**", "`拒绝`"] },
      { kind: "blockquote", lines: ["保留", "换行"] },
      { kind: "code", text: "line 1\nline 2" },
    ]);
  });

  it("positions tooltip above the anchor when the bottom would be clipped", () => {
    expect(
      positionTooltipPanel(
        { bottom: 290, left: 120, top: 260 },
        { height: 90, width: 240 },
        { height: 300, width: 500 },
      ),
    ).toMatchObject({
      left: 120,
      placement: "top",
      top: 164,
    });
  });

  it("keeps tooltip inside the right viewport edge", () => {
    expect(
      positionTooltipPanel(
        { bottom: 40, left: 470, top: 20 },
        { height: 90, width: 220 },
        { height: 300, width: 500 },
      ),
    ).toMatchObject({
      left: 272,
      placement: "bottom",
    });
  });

  it("limits tooltip width to the available viewport width", () => {
    expect(
      positionTooltipPanel(
        { bottom: 40, left: 20, top: 20 },
        { height: 90, width: 520 },
        { height: 300, width: 300 },
      ),
    ).toMatchObject({
      left: 8,
      maxWidth: 284,
    });
  });

  it("keeps an oversized tooltip inside the top viewport edge", () => {
    expect(
      positionTooltipPanel(
        { bottom: 160, left: 20, top: 140 },
        { height: 500, width: 240 },
        { height: 180, width: 360 },
      ),
    ).toMatchObject({
      left: 20,
      top: 8,
    });
  });

  it("stops tooltip portal events from bubbling to the session row", () => {
    const stopPropagation = vi.fn();

    stopTooltipPortalEvent({ stopPropagation });

    expect(stopPropagation).toHaveBeenCalledTimes(1);
  });

  it("renders full thread label in session detail", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const longThreadLabel = "一二三四五六七八九十十一完整线程标题";
    const selectedSession = {
      ...sessionItem(codexAppSessionKey, "codex_app"),
      thread_label: longThreadLabel,
    };
    const detail: SessionDetailViewModel = {
      header: longThreadLabel,
      identity: `/tmp/builder-panel-app / ${longThreadLabel}`,
      usage: "5H --，本周 --",
      summary: {
        text: "摘要",
        full_text: "摘要",
        truncated: false,
        max_chars: 240,
      },
      execution_info: "完成",
      pending_interaction: null,
      pending_interaction_id: null,
      pending_interaction_kind: null,
      reply_box: {
        enabled: false,
        disabled_reason: "当前会话不支持回复",
      },
      choice_box: {
        enabled: false,
        allows_multiple: false,
        choices: [],
        disabled_reason: "当前会话没有选项交互",
      },
      toolbar_actions: [],
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionDetail, {
          detail,
          selectedSession,
          draft: "",
          selectedChoiceValues: [],
          settings: defaultSettings(),
          submittingInteractionId: null,
          onDraftChange: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onUseShortcutReply: () => undefined,
          onToggleChoice: () => undefined,
          onSubmitChoice: () => undefined,
          onCreateFollowupTurn: () => undefined,
        }),
      );
    });

    expect(container.querySelector(".session-detail strong")?.textContent).toBe(
      longThreadLabel,
    );
    expect(container.querySelector(".session-detail p")?.textContent).toBe(
      `/tmp/builder-panel-app / ${longThreadLabel}`,
    );

    const detailButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "详情",
    );
    await act(async () => {
      detailButton?.click();
    });

    expect(
      container.querySelector(".session-detail-overlay p")?.textContent,
    ).toBe(`Codex / ${longThreadLabel}`);

    await act(async () => {
      root.unmount();
    });
  });

  it("only auto-shows action rows for waiting sessions", () => {
    expect(shouldAutoShowSessionActionRow("waiting_for_approval")).toBe(true);
    expect(shouldAutoShowSessionActionRow("waiting_for_answer")).toBe(true);
    expect(shouldAutoShowSessionActionRow("completed")).toBe(false);
    expect(shouldAutoShowSessionActionRow("failed")).toBe(false);
  });

  it("renders completed and failed follow-up rows for hover expansion", () => {
    expect(canToggleFollowupRow("failed", true)).toBe(true);
    expect(canToggleFollowupRow("completed", true)).toBe(true);
    expect(canToggleFollowupRow("failed", false)).toBe(false);
    expect(shouldShowSessionActionRow("completed", true)).toBe(true);
    expect(shouldShowSessionActionRow("failed", true)).toBe(true);
    expect(shouldShowSessionActionRow("completed", false)).toBe(false);
    expect(shouldUseFollowupShortcut("failed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("completed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("running", true)).toBe(false);
  });

  it("uses the compact two-column action row for manual follow-up expansion", () => {
    expect(sessionActionRowClassName("completed")).toContain(
      "session-row-action-followup",
    );
    expect(sessionActionRowClassName("failed")).toContain(
      "session-row-action-followup",
    );
    expect(sessionActionRowClassName("waiting_for_answer")).toBe(
      "session-row-action",
    );
  });

  it("uses stable labels for executable capability actions", () => {
    expect(actionLabel("resolve_approval")).toBe("审批");
    expect(actionLabel("send_reply")).toBe("回复");
    expect(actionLabel("create_followup_turn")).toBe("新对话");
  });

  it("ignores stale settings save responses", () => {
    expect(isLatestSettingsSaveResponse(1, 2)).toBe(false);
    expect(isLatestSettingsSaveResponse(2, 2)).toBe(true);
  });

  it("applies window preferences for the latest settings save response", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();
    const settings = {
      ...defaultSettings(),
      general: {
        ...defaultSettings().general,
        keep_panel_on_top: false,
      },
    };

    const applied = await applyLatestSavedPanelWindowPreferences(
      true,
      2,
      2,
      settings,
      applyPreferences,
    );

    expect(applied).toBe(true);
    expect(applyPreferences).toHaveBeenCalledTimes(1);
    expect(applyPreferences).toHaveBeenCalledWith(settings);
  });

  it("does not apply window preferences for stale settings save responses", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();

    const applied = await applyLatestSavedPanelWindowPreferences(
      true,
      1,
      2,
      defaultSettings(),
      applyPreferences,
    );

    expect(applied).toBe(false);
    expect(applyPreferences).not.toHaveBeenCalled();
  });

  it("keeps window preferences untouched when settings save fails", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();

    const applied = await applyLatestSavedPanelWindowPreferences(
      false,
      2,
      2,
      defaultSettings(),
      applyPreferences,
    );

    expect(applied).toBe(false);
    expect(applyPreferences).not.toHaveBeenCalled();
  });

  it("creates default hook install statuses for the settings list", () => {
    const statuses = defaultHookAgentStatuses();

    expect(statuses.map((status) => status.agent)).toEqual(["codex", "claude"]);
    expect(statuses.every((status) => status.can_install)).toBe(true);
    expect(statuses.every((status) => status.can_uninstall)).toBe(false);
  });

  it("disables duplicate hook install and uninstall actions", () => {
    const installed = {
      ...defaultHookAgentStatuses()[0],
      state: "installed" as const,
      message: "已安装",
      can_install: false,
      can_uninstall: true,
    };
    const notInstalled = defaultHookAgentStatuses()[1];

    expect(isHookActionDisabled(installed, "install", null)).toBe(true);
    expect(isHookActionDisabled(installed, "uninstall", null)).toBe(false);
    expect(isHookActionDisabled(notInstalled, "install", null)).toBe(false);
    expect(isHookActionDisabled(notInstalled, "uninstall", null)).toBe(true);
    expect(isHookActionDisabled(installed, "uninstall", "codex")).toBe(true);
  });

  it("merges panel window updates during the debounce window", () => {
    const merged = mergePanelWindowStateUpdate(
      {
        window_position: { x: 12, y: 20 },
      },
      {
        window_size: { width: 860, height: 640 },
      },
    );

    expect(merged).toEqual({
      window_position: { x: 12, y: 20 },
      window_size: { width: 860, height: 640 },
    });
  });

  it("keeps submitting state when panel window controls fail", () => {
    const current = {
      ...createDefaultMockPanelUiState(),
      submittingInteractionId: "approval-1",
    };

    const next = applyPanelWindowControlError(current, "最小化窗口失败");

    expect(next.errorMessage).toBe("最小化窗口失败");
    expect(next.submittingInteractionId).toBe("approval-1");
  });

  it("formats active session elapsed time with hour rollover", () => {
    expect(elapsedDurationLabel(1000, 62_000)).toBe("01:01");
    expect(elapsedDurationLabel(1000, 3_663_000)).toBe("01:01:02");
    expect(elapsedDurationLabel(3000, 1000)).toBe("00:00");
  });

  it("formats completed session relative time", () => {
    expect(relativePastLabel(10_000, 59_000)).toBe("刚刚");
    expect(relativePastLabel(10_000, 130_000)).toBe("2 分钟前");
    expect(relativePastLabel(10_000, 7_210_000)).toBe("2 小时前");
  });

  it("uses started and completed timestamps for the session side label", () => {
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      10_000,
    );
    const completed = {
      ...sessionItem(
        sessionKey("project-a", "completed"),
        "codex_app",
        "completed",
        "已完成",
        30_000,
      ),
      completed_at: { value: 30_000 },
    };

    expect(sessionSideTimeLabel(running, 72_000)).toBe("01:02");
    expect(sessionSideTimeLabel(completed, 150_000)).toBe("2 分钟前");
  });
});

const sessionItem = (
  sessionKey: SessionKey,
  runtimeSource: PanelSessionListItem["runtimeSource"],
  statusKind: PanelSessionListItem["status_kind"] = "waiting_for_approval",
  statusLabel = "等待审批",
  updatedAt = 1000,
): PanelSessionListItem => ({
  runtimeSource,
  session_key: sessionKey,
  agent_label: sessionKey.agent_kind,
  project_label: sessionKey.project_id.value,
  thread_label: `Thread ${sessionKey.conversation_id.value}`,
  conversation_label: sessionKey.conversation_id.value,
  status_label: statusLabel,
  status_kind: statusKind,
  summary: {
    text: "等待 Codex 审批",
    full_text: "等待 Codex 审批",
    truncated: false,
    max_chars: 120,
  },
  updated_at_label: updatedAt.toString(),
  started_at: { value: updatedAt },
  completed_at:
    statusKind === "completed" || statusKind === "failed"
      ? { value: updatedAt }
      : null,
  usage_5h: {
    value_label: "--",
    amount_label: null,
    unit: null,
    source_key: null,
    source_label: null,
    scope: null,
    updated_at: null,
  },
  usage_weekly: {
    value_label: "--",
    amount_label: null,
    unit: null,
    source_key: null,
    source_label: null,
    scope: null,
    updated_at: null,
  },
  actions: ["resolve_approval"],
  inline_interaction: {
    summary: "等待 Codex 审批",
    interaction_id: { value: "approval-1" },
    kind: "approval",
    can_jump: false,
    can_send_reply: false,
    can_resolve_approval: true,
    can_create_followup_turn: false,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
  indent_level: 0,
});

const sessionKey = (projectId: string, conversationId: string): SessionKey => ({
  agent_kind: "codex_cli",
  project_id: { value: projectId },
  conversation_id: { value: conversationId },
});

const verifiedUsage = (
  valueLabel: string,
  sourceKey: string,
  updatedAt: number,
  scope: PanelSessionListItem["usage_5h"]["scope"] = "account_window",
): PanelSessionListItem["usage_5h"] => {
  const [amountLabel, unit] = valueLabel.split(" ");
  return {
    value_label: valueLabel,
    amount_label: amountLabel,
    unit: unit ?? null,
    source_key: sourceKey,
    source_label: "test",
    scope,
    updated_at: { value: updatedAt },
  };
};

const textDisplay = (text: string): PanelSessionListItem["summary"] => ({
  text,
  full_text: text,
  truncated: false,
  max_chars: 120,
});

const flushPromises = async (): Promise<void> => {
  await Promise.resolve();
  await Promise.resolve();
};
