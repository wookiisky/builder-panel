import { describe, expect, it } from "vitest";

import {
  actionLabel,
  canShowTimelineEntry,
  countSessionsByStatus,
  isLatestSettingsSaveResponse,
  isCodexCliRuntime,
  canInstallHooksAfterPreview,
  mergePanelWindowStateUpdate,
  panelSessionToId,
  selectPanelSession,
  selectFirstSessionWhenMissing,
  sortPanelSessions,
  toggleHookInstallAgent,
  type PanelSessionListItem,
} from "./BuilderPanelApp";
import type { SessionKey } from "../api/mockPanelContract";
import { createDefaultMockPanelUiState } from "../stores/mockPanelStore";

const codexSessionKey: SessionKey = {
  agent_kind: "codex_cli",
  project_id: { value: "/tmp/builder-panel" },
  conversation_id: { value: "codex-session-1" },
};

const mockSessionKey: SessionKey = {
  agent_kind: "claude_code_cli",
  project_id: { value: "mock-project-1" },
  conversation_id: { value: "mock-session-1" },
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

  it("keeps the user selected session during later refreshes", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(mockSessionKey, "mock"),
    );

    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
      sessionItem(mockSessionKey, "mock"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(mockSessionKey, "mock")),
    );
  });

  it("selects the first available session when the previous selection disappears", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(mockSessionKey, "mock"),
    );
    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexSessionKey, "codex_cli")),
    );
  });

  it("routes by runtime source instead of agent kind", () => {
    const mockCodexLikeSession = sessionItem(codexSessionKey, "mock");
    const realCodexSession = sessionItem(codexSessionKey, "codex_cli");

    expect(isCodexCliRuntime(mockCodexLikeSession)).toBe(false);
    expect(isCodexCliRuntime(realCodexSession)).toBe(true);
  });

  it("keeps runtime source in selected session identity", () => {
    const mockCodexLikeSession = sessionItem(codexSessionKey, "mock");
    const realCodexSession = sessionItem(codexSessionKey, "codex_cli");
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      mockCodexLikeSession,
    );
    const sessions = [realCodexSession, mockCodexLikeSession];

    const selectedSession = sessions.find(
      (session) =>
        panelSessionToId(session) === selectedState.selectedSessionId,
    );

    expect(panelSessionToId(mockCodexLikeSession)).not.toBe(
      panelSessionToId(realCodexSession),
    );
    expect(selectedSession?.runtimeSource).toBe("mock");
  });

  it("only shows timeline entry when capability exists", () => {
    expect(canShowTimelineEntry(["jump", "view_process_timeline"])).toBe(true);
    expect(canShowTimelineEntry(["jump", "send_reply"])).toBe(false);
  });

  it("sorts waiting sessions before running sessions and then by updated time", () => {
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "mock",
      "running",
      "运行中",
      3000,
    );
    const olderWaiting = sessionItem(
      sessionKey("project-a", "older-waiting"),
      "mock",
      "waiting_for_answer",
      "等待回复",
      1000,
    );
    const newerWaiting = sessionItem(
      sessionKey("project-a", "newer-waiting"),
      "mock",
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
        "mock",
        "waiting_for_approval",
        "等待审批",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "answer"),
        "mock",
        "waiting_for_answer",
        "等待回复",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "running"),
        "mock",
        "running",
        "运行中",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "completed"),
        "mock",
        "completed",
        "已完成",
        1000,
      ),
    ]);

    expect(counts).toEqual({ waiting: 2, running: 1 });
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

  it("toggles hook install agents without mutating the original selection", () => {
    const selected = ["codex", "claude"] as const;

    const withoutCodex = toggleHookInstallAgent(selected, "codex");
    const withCodexAgain = toggleHookInstallAgent(withoutCodex, "codex");

    expect(selected).toEqual(["codex", "claude"]);
    expect(withoutCodex).toEqual(["claude"]);
    expect(withCodexAgain).toEqual(["claude", "codex"]);
  });

  it("requires preview for the current hook install selection before install", () => {
    const preview = {
      files_to_modify: ["/tmp/hooks.json"],
      backup_files: ["/tmp/hooks.json.builder-panel.bak"],
      manifest_path: "/tmp/manifest.json",
    };

    expect(
      canInstallHooksAfterPreview({
        selectedAgents: ["codex"],
        previewAgents: null,
        preview: null,
      }),
    ).toBe(false);
    expect(
      canInstallHooksAfterPreview({
        selectedAgents: ["codex", "claude"],
        previewAgents: ["codex"],
        preview,
      }),
    ).toBe(false);
    expect(
      canInstallHooksAfterPreview({
        selectedAgents: ["claude", "codex"],
        previewAgents: ["codex", "claude"],
        preview,
      }),
    ).toBe(true);
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
    source_label: null,
  },
  usage_weekly: {
    value_label: "--",
    source_label: null,
  },
  actions: ["resolve_approval"],
});

const sessionKey = (projectId: string, conversationId: string): SessionKey => ({
  agent_kind: "codex_cli",
  project_id: { value: projectId },
  conversation_id: { value: conversationId },
});
