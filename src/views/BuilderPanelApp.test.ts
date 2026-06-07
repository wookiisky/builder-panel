import { describe, expect, it } from "vitest";

import {
  actionLabel,
  aggregateToolUsage,
  applyPanelWindowControlError,
  canExpandSessionRow,
  canJumpOnSessionClick,
  canSelectOnSessionClick,
  canShowTimelineEntry,
  countSessionsByStatus,
  fetchSessionsForSource,
  isLatestSettingsSaveResponse,
  isCodexCliRuntime,
  isCodexAppRuntime,
  defaultHookAgentStatuses,
  isHookActionDisabled,
  mergePanelWindowStateUpdate,
  panelSessionToId,
  selectPanelSession,
  selectFirstSessionWhenMissing,
  shouldSubmitReplyOnKeyDown,
  shouldUseFollowupShortcut,
  sortPanelSessions,
  type PanelSessionListItem,
} from "./BuilderPanelApp";
import type { SessionKey } from "../api/mockPanelContract";
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

  it("only shows timeline entry when capability exists", () => {
    expect(canShowTimelineEntry(["jump", "view_process_timeline"])).toBe(true);
    expect(canShowTimelineEntry(["jump", "send_reply"])).toBe(false);
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

  it("sorts waiting sessions before running sessions and then by updated time", () => {
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

    const sorted = sortPanelSessions([running, olderWaiting, newerWaiting]);

    expect(sorted.map((session) => session.conversation_label)).toEqual([
      "newer-waiting",
      "older-waiting",
      "running",
    ]);
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

  it("expands failed sessions that can create follow-up turns", () => {
    expect(canExpandSessionRow("failed", true)).toBe(true);
    expect(canExpandSessionRow("completed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("failed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("completed", true)).toBe(true);
    expect(canExpandSessionRow("failed", false)).toBe(false);
    expect(shouldUseFollowupShortcut("running", true)).toBe(false);
  });

  it("uses stable labels for executable capability actions", () => {
    expect(actionLabel("resolve_approval")).toBe("审批");
    expect(actionLabel("send_reply")).toBe("回复");
    expect(actionLabel("view_process_timeline")).toBe("过程");
  });

  it("ignores stale settings save responses", () => {
    expect(isLatestSettingsSaveResponse(1, 2)).toBe(false);
    expect(isLatestSettingsSaveResponse(2, 2)).toBe(true);
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
  conversation_label: sessionKey.conversation_id.value,
  status_label: statusLabel,
  status_kind: statusKind,
  summary: {
    text: "等待 Codex 审批",
    truncated: false,
    max_chars: 120,
  },
  updated_at_label: updatedAt.toString(),
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
    can_view_process_timeline: false,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
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
