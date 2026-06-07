import { useEffect, useMemo, useRef, useState } from "react";

import {
  createCodexAppFollowupTurn,
  fetchCodexAppSessions,
  fetchCodexAppTimeline,
  releaseCodexAppTimelineCache,
  submitCodexAppApproval,
  submitCodexAppChoice,
  submitCodexAppReply,
} from "../api/codexAppPanelApi";
import {
  fetchCodexCliSessions,
  fetchCodexCliTimeline,
  releaseCodexCliTimelineCache,
  submitCodexCliApproval,
} from "../api/codexCliPanelApi";
import {
  getHookInstallStatus,
  installHooks,
  uninstallHooks,
  type HookInstallAgent,
  type HookInstallAgentStatus,
} from "../api/hookInstallApi";
import {
  applyPanelWindowGeometry,
  closePanelWindow,
  minimizePanelWindow,
  savePanelWindowState,
  subscribePanelWindowGeometry,
  type PanelWindowStateUpdate,
} from "../api/panelWindowApi";
import { jumpToSession } from "../api/sessionJumpApi";
import type {
  ApprovalDecision,
  InteractionId,
  ProcessTimelineItem,
  SessionDetailViewModel,
  SessionKey,
  SessionListItemViewModel,
  TimelinePage,
  TimelineQuery,
  UiAction,
} from "../api/mockPanelContract";
import {
  defaultSettings,
  fetchPanelSettings,
  savePanelSettings,
} from "../api/settingsApi";
import type {
  BuilderPanelSettings,
  CustomShortcutInput,
  SettingsViewModel,
} from "../api/settingsContract";
import { PanelShell } from "../components/PanelShell";
import { SettingsPanel } from "../components/SettingsPanel";
import {
  beginSubmit,
  beginTimelineLoad,
  clearChoiceSelection,
  clearDraft,
  closeTimeline,
  countReplyChars,
  createDefaultMockPanelUiState,
  endSubmit,
  failTimelineLoad,
  isReplyDraftInvalid,
  openTimeline,
  selectSession,
  sessionKeyToId,
  setTimelinePage,
  timelinePageToCopyText,
  timelineVisibleRange,
  toggleChoiceSelection,
  updateDraft,
  updateTimelineKind,
  updateTimelineSearch,
  type MockPanelUiState,
  type TimelineKindFilter,
} from "../stores/mockPanelStore";
/// 会话运行时来源。
type RuntimeSource = "codex_cli" | "codex_app";

/// hook 安装前端状态。
interface HookInstallUiState {
  /// 当前 agent hook 安装状态。
  readonly agentStatuses: readonly HookInstallAgentStatus[];
  /// 当前状态提示。
  readonly statusMessage: string | null;
  /// 正在执行操作的 agent。
  readonly workingAgent: HookInstallAgent | null;
  /// 是否正在刷新 hook 状态。
  readonly refreshing: boolean;
}

/// 前端合并后的 session 列表项。
export type PanelSessionListItem = SessionListItemViewModel & {
  /// 该 session 来源，用于隔离不同真实运行时。
  readonly runtimeSource: RuntimeSource;
};

/// Codex CLI hook 写入 runtime 后，前端刷新 session 的间隔。
export const SESSION_REFRESH_INTERVAL_MS = 1000;

/// Timeline 单次读取页大小。
const TIMELINE_PAGE_SIZE = 50;
/// Timeline 虚拟列表固定行高。
const TIMELINE_ROW_HEIGHT = 82;
/// Timeline 虚拟列表视口高度。
const TIMELINE_VIEWPORT_HEIGHT = 330;
/// Timeline 虚拟列表额外渲染行数。
const TIMELINE_OVERSCAN = 4;

/// session 状态排序权重。
const SESSION_STATUS_ORDER: Record<
  SessionListItemViewModel["status_kind"],
  number
> = {
  waiting_for_approval: 0,
  waiting_for_answer: 0,
  running: 1,
  failed: 2,
  completed: 3,
  detached: 4,
};

/// Builder Panel 首屏应用。
export const BuilderPanelApp = () => {
  const [mockUiState, setMockUiState] = useState<MockPanelUiState>(() =>
    createDefaultMockPanelUiState(),
  );
  const [sessions, setSessions] = useState<readonly PanelSessionListItem[]>([]);
  const [settingsModalOpen, setSettingsModalOpen] = useState(false);
  const [settingsView, setSettingsView] = useState<SettingsViewModel>(() => ({
    settings: defaultSettings(),
    status_message: null,
  }));
  const [settingsHydrated, setSettingsHydrated] = useState(false);
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [hookInstallState, setHookInstallState] = useState<HookInstallUiState>(
    () => ({
      agentStatuses: defaultHookAgentStatuses(),
      statusMessage: null,
      workingAgent: null,
      refreshing: false,
    }),
  );
  const settingsSaveVersion = useRef(0);
  const panelGeometryApplied = useRef(false);
  const panelGeometrySaveTimer = useRef<number | null>(null);
  const pendingPanelWindowUpdate = useRef<PanelWindowStateUpdate>({});

  useEffect(() => {
    let disposed = false;

    fetchPanelSettings()
      .then((view) => {
        if (!disposed) {
          setSettingsView({
            ...view,
            settings: forceExpandedPanelSettings(view.settings),
          });
          setSettingsHydrated(true);
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          const fallbackView = {
            settings: defaultSettings(),
            status_message: readableError(error, "读取设置失败"),
          };
          setSettingsView({
            ...fallbackView,
            settings: forceExpandedPanelSettings(fallbackView.settings),
          });
          setSettingsHydrated(true);
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!settingsHydrated || panelGeometryApplied.current) {
      return;
    }

    panelGeometryApplied.current = true;
    applyPanelWindowGeometry(settingsView.settings.panel).catch(
      (error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "恢复 panel 窗口状态失败"),
        }));
      },
    );
  }, [settingsHydrated, settingsView.settings.panel]);

  useEffect(() => {
    if (!settingsHydrated) {
      return;
    }

    let disposed = false;
    let unsubscribe: (() => void) | null = null;
    subscribePanelWindowGeometry((update) => {
      if (!disposed) {
        schedulePanelWindowStateSave(update);
      }
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        unsubscribe = unlisten;
      })
      .catch((error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "监听 panel 窗口状态失败"),
        }));
      });

    return () => {
      disposed = true;
      if (unsubscribe !== null) {
        unsubscribe();
      }
      if (panelGeometrySaveTimer.current !== null) {
        window.clearTimeout(panelGeometrySaveTimer.current);
      }
    };
  }, [settingsHydrated]);

  useEffect(() => {
    let disposed = false;
    let refreshing = false;

    const refresh = () => {
      if (refreshing) {
        return;
      }

      refreshing = true;
      fetchAllSessions(settingsView.settings)
        .then((items) => {
          if (disposed) {
            return;
          }
          setSessions(items);
          setMockUiState((current) =>
            selectFirstSessionWhenMissing(current, items),
          );
        })
        .catch((error: unknown) => {
          if (!disposed) {
            setMockUiState((current) =>
              endSubmit(current, readableError(error, "读取 session 失败")),
            );
          }
        })
        .finally(() => {
          refreshing = false;
        });
    };

    refresh();
    const timer = window.setInterval(refresh, SESSION_REFRESH_INTERVAL_MS);

    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [settingsView.settings]);

  const selectedSession = useMemo(() => {
    return (
      sessions.find(
        (session) =>
          panelSessionToId(session) === mockUiState.selectedSessionId,
      ) ?? null
    );
  }, [mockUiState.selectedSessionId, sessions]);

  useEffect(() => {
    let disposed = false;

    if (!mockUiState.timeline.open || selectedSession === null) {
      return;
    }

    setMockUiState((current) => beginTimelineLoad(current));
    fetchTimelinePage(selectedSession, {
      session_key: selectedSession.session_key,
      page: 0,
      page_size: TIMELINE_PAGE_SIZE,
      search:
        mockUiState.timeline.search.trim().length === 0
          ? null
          : mockUiState.timeline.search,
      kind:
        mockUiState.timeline.kind === "all" ? null : mockUiState.timeline.kind,
    })
      .then((page) => {
        if (!disposed) {
          setMockUiState((current) => setTimelinePage(current, page));
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setMockUiState((current) =>
            failTimelineLoad(
              current,
              readableError(error, "读取 timeline 失败"),
            ),
          );
        }
      });

    return () => {
      disposed = true;
    };
  }, [
    mockUiState.timeline.kind,
    mockUiState.timeline.open,
    mockUiState.timeline.search,
    selectedSession,
  ]);

  const refreshSessions = async (): Promise<void> => {
    const nextSessions = await fetchAllSessions(settingsView.settings);
    setSessions(nextSessions);
  };

  const updateSettings = async (
    settings: BuilderPanelSettings,
  ): Promise<void> => {
    const saveVersion = settingsSaveVersion.current + 1;
    settingsSaveVersion.current = saveVersion;
    setSettingsView({
      settings,
      status_message: settingsView.status_message,
    });
    setSettingsSaving(true);
    try {
      const nextView = await savePanelSettings(settings);
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsView(nextView);
      }
    } catch (error: unknown) {
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsView({
          settings,
          status_message: readableError(error, "保存设置失败"),
        });
      }
    } finally {
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsSaving(false);
      }
    }
  };

  const schedulePanelWindowStateSave = (
    update: PanelWindowStateUpdate,
  ): void => {
    if (panelGeometrySaveTimer.current !== null) {
      window.clearTimeout(panelGeometrySaveTimer.current);
    }
    pendingPanelWindowUpdate.current = mergePanelWindowStateUpdate(
      pendingPanelWindowUpdate.current,
      update,
    );
    panelGeometrySaveTimer.current = window.setTimeout(() => {
      const nextUpdate = pendingPanelWindowUpdate.current;
      pendingPanelWindowUpdate.current = {};
      void savePanelWindowState(nextUpdate).catch((error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "保存 panel 窗口状态失败"),
        }));
      });
    }, 400);
  };

  const refreshHookInstallStatus = async (): Promise<void> => {
    setHookInstallState((current) => ({
      ...current,
      refreshing: true,
      statusMessage: null,
    }));
    try {
      const status = await getHookInstallStatus();
      setHookInstallState((current) => ({
        ...current,
        agentStatuses: status.agents,
        refreshing: false,
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        refreshing: false,
        statusMessage: readableError(error, "hook 状态读取失败"),
      }));
    }
  };

  const runHookInstall = async (agent: HookInstallAgent): Promise<void> => {
    const status = hookInstallState.agentStatuses.find(
      (item) => item.agent === agent,
    );
    if (status === undefined || !status.can_install) {
      setHookInstallState((current) => ({
        ...current,
        statusMessage: "当前 hook 状态不允许安装",
      }));
      return;
    }

    setHookInstallState((current) => ({
      ...current,
      workingAgent: agent,
      statusMessage: null,
    }));
    try {
      await installHooks({ agents: [agent] });
      const nextStatus = await getHookInstallStatus();
      setHookInstallState((current) => ({
        ...current,
        agentStatuses: nextStatus.agents,
        workingAgent: null,
        statusMessage: "hook 状态已更新",
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        workingAgent: null,
        statusMessage: readableError(error, "hook 安装失败"),
      }));
    }
  };

  const runHookUninstall = async (agent: HookInstallAgent): Promise<void> => {
    const status = hookInstallState.agentStatuses.find(
      (item) => item.agent === agent,
    );
    if (status === undefined || !status.can_uninstall) {
      setHookInstallState((current) => ({
        ...current,
        statusMessage: "当前 hook 状态不允许卸载",
      }));
      return;
    }

    setHookInstallState((current) => ({
      ...current,
      workingAgent: agent,
      statusMessage: null,
    }));
    try {
      await uninstallHooks({ agents: [agent] });
      const nextStatus = await getHookInstallStatus();
      setHookInstallState((current) => ({
        ...current,
        agentStatuses: nextStatus.agents,
        workingAgent: null,
        statusMessage: "hook 状态已更新",
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        workingAgent: null,
        statusMessage: readableError(error, "hook 卸载失败"),
      }));
    }
  };

  const resolveApprovalForSession = async (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    decision: ApprovalDecision,
  ): Promise<void> => {
    setMockUiState((current) => beginSubmit(current, interactionId.value));
    try {
      if (isCodexCliRuntime(session)) {
        await submitCodexCliApproval({
          session_key: session.session_key,
          interaction_id: interactionId,
          decision,
        });
      } else if (isCodexAppRuntime(session)) {
        await submitCodexAppApproval({
          session_key: session.session_key,
          interaction_id: interactionId,
          decision,
        });
      }
      await refreshSessions();
      setMockUiState((current) => endSubmit(current, null));
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "审批提交失败")),
      );
    }
  };

  const sendReplyForSession = async (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    contentOverride: string | null = null,
  ): Promise<void> => {
    const draft =
      mockUiState.draftsBySessionId[sessionKeyToId(session.session_key)] ?? "";
    const effectiveContent = (contentOverride ?? draft).trim();
    if (isReplyDraftInvalid(effectiveContent, 1000)) {
      return;
    }

    setMockUiState((current) => beginSubmit(current, interactionId.value));
    try {
      if (isCodexAppRuntime(session)) {
        await submitCodexAppReply({
          session_key: session.session_key,
          interaction_id: interactionId,
          content: effectiveContent,
        });
      }
      await refreshSessions();
      setMockUiState((current) =>
        endSubmit(clearDraft(current, session.session_key), null),
      );
    } catch (error: unknown) {
      if (contentOverride !== null) {
        setMockUiState((current) =>
          updateDraft(current, session.session_key, contentOverride),
        );
      }
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "回复发送失败")),
      );
    }
  };

  const submitChoiceForSession = async (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    selectedValues: readonly string[],
  ): Promise<void> => {
    if (selectedValues.length === 0) {
      return;
    }

    setMockUiState((current) => beginSubmit(current, interactionId.value));
    try {
      if (isCodexAppRuntime(session)) {
        await submitCodexAppChoice({
          session_key: session.session_key,
          interaction_id: interactionId,
          selected_values: selectedValues,
        });
      }
      await refreshSessions();
      setMockUiState((current) =>
        endSubmit(clearChoiceSelection(current, interactionId.value), null),
      );
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "选项提交失败")),
      );
    }
  };

  const createFollowupTurnForSession = async (
    session: PanelSessionListItem,
    content: string,
  ): Promise<void> => {
    if (!isCodexAppRuntime(session)) {
      return;
    }

    const prompt = content.trim();
    if (isReplyDraftInvalid(prompt, 1000)) {
      return;
    }

    const submitId = `followup-${sessionKeyToId(session.session_key)}`;
    setMockUiState((current) => beginSubmit(current, submitId));
    try {
      await createCodexAppFollowupTurn({
        session_key: session.session_key,
        prompt,
      });
      await refreshSessions();
      setMockUiState((current) =>
        endSubmit(clearDraft(current, session.session_key), null),
      );
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "follow-up 创建失败")),
      );
    }
  };

  const jumpToPanelSession = async (
    session: PanelSessionListItem,
  ): Promise<void> => {
    setMockUiState((current) => selectPanelSession(current, session));
    if (
      !settingsView.settings.terminal.jump_enabled ||
      !session.inline_interaction.can_jump
    ) {
      return;
    }

    const result = await jumpToSession({
      runtime_source: session.runtimeSource,
      session_key: session.session_key,
    });
    if (result.jumped) {
      setMockUiState((current) => endSubmit(current, null));
      return;
    }

    if (
      result.fallback_text !== null &&
      settingsView.settings.terminal.copy_fallback_enabled
    ) {
      await copyText(result.fallback_text);
    }
    setMockUiState((current) => endSubmit(current, result.message));
  };

  const closeWindow = async (): Promise<void> => {
    try {
      await closePanelWindow();
    } catch (error: unknown) {
      reportPanelWindowError(readableError(error, "关闭窗口失败"));
    }
  };

  const minimizeWindow = async (): Promise<void> => {
    try {
      await minimizePanelWindow();
    } catch (error: unknown) {
      reportPanelWindowError(readableError(error, "最小化窗口失败"));
    }
  };

  /// 显示窗口控制错误，不改变交互提交状态。
  const reportPanelWindowError = (errorMessage: string): void => {
    setMockUiState((current) =>
      applyPanelWindowControlError(current, errorMessage),
    );
  };

  useEffect(() => {
    if (!settingsModalOpen) {
      return;
    }

    void refreshHookInstallStatus();
  }, [settingsModalOpen]);

  return (
    <main className={appSurfaceClassName(settingsView.settings)}>
      <PanelShell title="Builder Panel">
        <PanelTopBar
          sessions={sessions}
          settings={settingsView.settings}
          onClose={() => {
            void closeWindow();
          }}
          onMinimize={() => {
            void minimizeWindow();
          }}
          onOpenSettings={() => {
            setSettingsModalOpen(true);
          }}
        />
        {mockUiState.errorMessage !== null && (
          <div className="panel-error" role="alert">
            {mockUiState.errorMessage}
          </div>
        )}
        <SessionStream
          mockUiState={mockUiState}
          sessions={sessions}
          settings={settingsView.settings}
          selectedSessionId={mockUiState.selectedSessionId}
          onCreateFollowupTurn={(session, content) => {
            void createFollowupTurnForSession(session, content);
          }}
          onDraftChange={(session, draft) => {
            setMockUiState((current) =>
              updateDraft(current, session.session_key, draft),
            );
          }}
          onJump={(session) => {
            void jumpToPanelSession(session);
          }}
          onOpenTimeline={(session) => {
            setMockUiState((current) =>
              openTimeline(
                selectPanelSession(current, session),
                session.session_key,
              ),
            );
          }}
          onResolveApproval={(session, interactionId, decision) => {
            void resolveApprovalForSession(session, interactionId, decision);
          }}
          onSendReply={(session, interactionId, content) => {
            void sendReplyForSession(session, interactionId, content);
          }}
          onSubmitChoice={(session, interactionId, values) => {
            void submitChoiceForSession(session, interactionId, values);
          }}
          onToggleChoice={(interactionId, choiceValue, allowsMultiple) => {
            setMockUiState((current) =>
              toggleChoiceSelection(
                current,
                interactionId.value,
                choiceValue,
                allowsMultiple,
              ),
            );
          }}
        />
        {settingsModalOpen && (
          <div className="overlay-backdrop" role="presentation">
            <section className="overlay-panel settings-modal" aria-label="设置">
              <header>
                <div>
                  <strong>设置</strong>
                  <p>配置会立即保存</p>
                </div>
                <button
                  type="button"
                  onClick={() => {
                    setSettingsModalOpen(false);
                  }}
                >
                  关闭
                </button>
              </header>
              <SettingsPanel
                hookInstall={hookInstallState}
                saving={settingsSaving}
                settings={settingsView.settings}
                statusMessage={settingsView.status_message}
                onChange={(settings) => {
                  void updateSettings(settings);
                }}
                onInstallHook={(agent) => {
                  void runHookInstall(agent);
                }}
                onUninstallHook={(agent) => {
                  void runHookUninstall(agent);
                }}
              />
            </section>
          </div>
        )}
        {mockUiState.timeline.open && selectedSession !== null && (
          <TimelineOverlay
            state={mockUiState}
            selectedSession={selectedSession}
            onClose={() => {
              releaseTimelineCache(selectedSession);
              setMockUiState((current) => closeTimeline(current));
            }}
            onSearch={(search) => {
              setMockUiState((current) =>
                updateTimelineSearch(current, search),
              );
            }}
            onKind={(kind) => {
              setMockUiState((current) => updateTimelineKind(current, kind));
            }}
          />
        )}
      </PanelShell>
    </main>
  );
};

/// 记录窗口控制错误，不结束正在提交的交互。
export const applyPanelWindowControlError = (
  state: MockPanelUiState,
  errorMessage: string,
): MockPanelUiState => ({
  ...state,
  errorMessage,
});

/// 生成应用根节点样式类。
const appSurfaceClassName = (settings: BuilderPanelSettings): string => {
  const classNames = [
    "app-surface",
    `app-theme-${settings.display.theme}`,
    settings.display.density === "compact" ? "app-surface-compact" : null,
  ];

  return classNames.filter((className) => className !== null).join(" ");
};

/// 移除收缩入口后，运行时始终强制展开 panel。
const forceExpandedPanelSettings = (
  settings: BuilderPanelSettings,
): BuilderPanelSettings => ({
  ...settings,
  panel: {
    ...settings.panel,
    collapsed: false,
  },
});

/// 读取所有当前可展示 session。
const fetchAllSessions = async (
  settings: BuilderPanelSettings,
): Promise<readonly PanelSessionListItem[]> => {
  const [codexCliSessions, codexAppSessions] = await Promise.all([
    fetchSessionsForSource(
      settings.agents.codex_cli_enabled,
      fetchCodexCliSessions,
    ),
    fetchSessionsForSource(
      settings.agents.codex_app_enabled,
      fetchCodexAppSessions,
    ),
  ]);

  return sortPanelSessions([
    ...codexAppSessions.map((session) =>
      withRuntimeSource(session, "codex_app"),
    ),
    ...codexCliSessions.map((session) =>
      withRuntimeSource(session, "codex_cli"),
    ),
  ]);
};

/// 读取单一来源 session，来源失败时不阻断其它来源。
export const fetchSessionsForSource = async (
  enabled: boolean,
  fetcher: () => Promise<readonly SessionListItemViewModel[]>,
): Promise<readonly SessionListItemViewModel[]> => {
  if (!enabled) {
    return [];
  }

  try {
    return await fetcher();
  } catch {
    return [];
  }
};

/// 排序合并后的 session 列表。
export const sortPanelSessions = (
  sessions: readonly PanelSessionListItem[],
): readonly PanelSessionListItem[] => {
  return [...sessions].sort((left, right) => {
    const leftStatusOrder = SESSION_STATUS_ORDER[left.status_kind];
    const rightStatusOrder = SESSION_STATUS_ORDER[right.status_kind];
    if (leftStatusOrder !== rightStatusOrder) {
      return leftStatusOrder - rightStatusOrder;
    }

    const leftUpdatedAt = Number(left.updated_at_label);
    const rightUpdatedAt = Number(right.updated_at_label);
    if (Number.isFinite(leftUpdatedAt) && Number.isFinite(rightUpdatedAt)) {
      return rightUpdatedAt - leftUpdatedAt;
    }

    return panelSessionToId(left).localeCompare(panelSessionToId(right));
  });
};

/// 统计等待和运行中的 session 数量。
export const countSessionsByStatus = (
  sessions: readonly PanelSessionListItem[],
): { readonly waiting: number; readonly running: number } => {
  return sessions.reduce(
    (counts, session) => {
      if (
        session.status_kind === "waiting_for_approval" ||
        session.status_kind === "waiting_for_answer"
      ) {
        return {
          waiting: counts.waiting + 1,
          running: counts.running,
        };
      }
      if (session.status_kind === "running") {
        return {
          waiting: counts.waiting,
          running: counts.running + 1,
        };
      }

      return counts;
    },
    { waiting: 0, running: 0 },
  );
};

/// 判断设置保存响应是否仍是最新请求。
export const isLatestSettingsSaveResponse = (
  responseVersion: number,
  currentVersion: number,
): boolean => {
  return responseVersion === currentVersion;
};

/// 创建默认 hook 安装状态。
export const defaultHookAgentStatuses = (): readonly HookInstallAgentStatus[] => [
  defaultHookAgentStatus("codex"),
  defaultHookAgentStatus("claude"),
];

/// 判断 hook 安装按钮是否应该禁用。
export const isHookActionDisabled = (
  status: HookInstallAgentStatus,
  action: "install" | "uninstall",
  workingAgent: HookInstallAgent | null,
): boolean => {
  if (workingAgent !== null) {
    return true;
  }

  if (action === "install") {
    return !status.can_install;
  }

  return !status.can_uninstall;
};

/// 合并 panel 窗口状态局部更新。
export const mergePanelWindowStateUpdate = (
  current: PanelWindowStateUpdate,
  next: PanelWindowStateUpdate,
): PanelWindowStateUpdate => {
  return {
    ...current,
    ...next,
  };
};

/// 创建默认单 agent hook 安装状态。
const defaultHookAgentStatus = (
  agent: HookInstallAgent,
): HookInstallAgentStatus => ({
  agent,
  state: "not_installed",
  message: "未安装",
  reasons: [],
  can_install: true,
  can_uninstall: false,
});

/// 首次或空选中时自动选择第一条 session。
export const selectFirstSessionWhenMissing = (
  current: MockPanelUiState,
  items: readonly PanelSessionListItem[],
): MockPanelUiState => {
  if (items.length === 0) {
    return current;
  }

  const selectedStillExists =
    current.selectedSessionId !== null &&
    items.some((item) => panelSessionToId(item) === current.selectedSessionId);
  if (selectedStillExists) {
    return current;
  }

  return selectPanelSession(current, items[0]);
};

/// 生成包含运行时来源的前端 session ID。
export const panelSessionToId = (session: PanelSessionListItem): string => {
  return [session.runtimeSource, sessionKeyToId(session.session_key)].join(
    "::",
  );
};

/// 选择合并后的前端 session。
export const selectPanelSession = (
  current: MockPanelUiState,
  session: PanelSessionListItem,
): MockPanelUiState => ({
  ...selectSession(current, session.session_key),
  selectedSessionId: panelSessionToId(session),
});

/// 为列表项附加显式运行时来源。
const withRuntimeSource = (
  session: SessionListItemViewModel,
  runtimeSource: RuntimeSource,
): PanelSessionListItem => ({
  ...session,
  runtimeSource,
});

/// 顶部状态区属性。
interface PanelTopBarProps {
  /// 当前 session 列表。
  readonly sessions: readonly PanelSessionListItem[];
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 打开设置回调。
  readonly onOpenSettings: () => void;
  /// 最小化窗口回调。
  readonly onMinimize: () => void;
  /// 关闭窗口回调。
  readonly onClose: () => void;
}

/// 顶部状态区。
const PanelTopBar = ({
  sessions,
  settings,
  onOpenSettings,
  onMinimize,
  onClose,
}: PanelTopBarProps) => {
  const counts = countSessionsByStatus(sessions);

  return (
    <div className="panel-topbar" aria-label="session 状态">
      <strong>
        运行中 {counts.running} / {sessions.length}
      </strong>
      {settings.display.show_usage && <ToolUsageSummary sessions={sessions} />}
      <div className="panel-topbar-actions">
        <button type="button" onClick={onMinimize}>
          最小化
        </button>
        <button type="button" onClick={onOpenSettings}>
          设置
        </button>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
    </div>
  );
};

/// 判断 session 是否来自真实 Codex CLI runtime。
export const isCodexCliRuntime = (session: PanelSessionListItem): boolean => {
  return session.runtimeSource === "codex_cli";
};

/// 判断 session 是否来自真实 Codex APP runtime。
export const isCodexAppRuntime = (session: PanelSessionListItem): boolean => {
  return session.runtimeSource === "codex_app";
};

/// 判断是否展示 timeline 入口。
export const canShowTimelineEntry = (actions: readonly UiAction[]): boolean => {
  return actions.includes("view_process_timeline");
};

/// 判断回复输入键盘事件是否应提交。
export const shouldSubmitReplyOnKeyDown = (
  enterToSend: boolean,
  key: string,
  shiftKey: boolean,
): boolean => enterToSend && key === "Enter" && !shiftKey;

/// 判断 session 行是否应展开。
export const canExpandSessionRow = (
  statusKind: PanelSessionListItem["status_kind"],
  canCreateFollowupTurn: boolean,
): boolean => {
  return (
    statusKind === "waiting_for_approval" ||
    statusKind === "waiting_for_answer" ||
    ((statusKind === "completed" || statusKind === "failed") &&
      canCreateFollowupTurn)
  );
};

/// 判断快捷输入是否应创建 follow-up。
export const shouldUseFollowupShortcut = (
  statusKind: PanelSessionListItem["status_kind"],
  canCreateFollowupTurn: boolean,
): boolean => {
  return (
    canCreateFollowupTurn &&
    (statusKind === "completed" || statusKind === "failed")
  );
};

/// 返回 UI 动作标签。
export const actionLabel = (action: UiAction): string => {
  switch (action) {
    case "jump":
      return "跳回";
    case "send_reply":
      return "回复";
    case "resolve_approval":
      return "审批";
    case "create_followup_turn":
      return "新对话";
    case "view_process_timeline":
      return "过程";
  }
};

/// 按运行时来源读取 timeline。
const fetchTimelinePage = async (
  session: PanelSessionListItem,
  query: TimelineQuery,
): Promise<TimelinePage> => {
  if (isCodexCliRuntime(session)) {
    return fetchCodexCliTimeline(query);
  }
  if (isCodexAppRuntime(session)) {
    return fetchCodexAppTimeline(query);
  }

  return {
    items: [],
    page: query.page,
    page_size: query.page_size,
    total: 0,
    has_next: false,
    filter_count: 0,
  };
};

/// 按运行时来源释放 timeline 大文本缓存。
const releaseTimelineCache = (session: PanelSessionListItem): void => {
  const releaseTask = isCodexCliRuntime(session)
    ? releaseCodexCliTimelineCache(session.session_key)
    : releaseCodexAppTimelineCache(session.session_key);

  releaseTask.catch(() => {
    // 关闭弹层不能被释放失败阻塞；下次查询仍可重新读取可用缓存。
  });
};

/// Session 流属性。
interface SessionStreamProps {
  /// 会话列表。
  readonly sessions: readonly PanelSessionListItem[];
  /// 当前选中 session ID。
  readonly selectedSessionId: string | null;
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// mock UI 状态。
  readonly mockUiState: MockPanelUiState;
  /// 跳回回调。
  readonly onJump: (session: PanelSessionListItem) => void;
  /// 草稿变化回调。
  readonly onDraftChange: (
    session: PanelSessionListItem,
    draft: string,
  ) => void;
  /// 审批回调。
  readonly onResolveApproval: (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    decision: ApprovalDecision,
  ) => void;
  /// 回复回调。
  readonly onSendReply: (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    content: string | null,
  ) => void;
  /// 切换选项回调。
  readonly onToggleChoice: (
    interactionId: InteractionId,
    choiceValue: string,
    allowsMultiple: boolean,
  ) => void;
  /// 提交选项回调。
  readonly onSubmitChoice: (
    session: PanelSessionListItem,
    interactionId: InteractionId,
    values: readonly string[],
  ) => void;
  /// 打开时间线回调。
  readonly onOpenTimeline: (session: PanelSessionListItem) => void;
  /// 创建后续 turn 回调。
  readonly onCreateFollowupTurn: (
    session: PanelSessionListItem,
    content: string,
  ) => void;
}

/// Session 流。
const SessionStream = ({
  sessions,
  selectedSessionId,
  settings,
  mockUiState,
  onJump,
  onDraftChange,
  onResolveApproval,
  onSendReply,
  onToggleChoice,
  onSubmitChoice,
  onOpenTimeline,
  onCreateFollowupTurn,
}: SessionStreamProps) => (
  <div className="session-list" aria-label="agent 会话列表">
    {sessions.length === 0 && (
      <div className="session-list-empty">暂无可展示 session</div>
    )}
    {sessions.map((session) => (
      <SessionRow
        key={panelSessionToId(session)}
        mockUiState={mockUiState}
        selected={selectedSessionId === panelSessionToId(session)}
        session={session}
        settings={settings}
        onCreateFollowupTurn={onCreateFollowupTurn}
        onDraftChange={onDraftChange}
        onJump={onJump}
        onOpenTimeline={onOpenTimeline}
        onResolveApproval={onResolveApproval}
        onSendReply={onSendReply}
        onSubmitChoice={onSubmitChoice}
        onToggleChoice={onToggleChoice}
      />
    ))}
  </div>
);

/// 工具用量摘要。
interface ToolUsageSummaryItem {
  /// 工具族。
  readonly family: "Codex" | "Claude";
  /// 5 小时窗口展示。
  readonly usage5h: string;
  /// 周窗口展示。
  readonly weekly: string;
  /// tooltip 文本。
  readonly tooltip: string;
}

/// 工具用量摘要。
const ToolUsageSummary = ({
  sessions,
}: {
  readonly sessions: readonly PanelSessionListItem[];
}) => {
  const items = aggregateToolUsage(sessions);
  if (items.length === 0) {
    return <span className="tool-usage-empty">用量 --</span>;
  }

  return (
    <div className="tool-usage" aria-label="工具用量">
      {items.map((item) => (
        <span key={item.family} title={item.tooltip}>
          {item.family} 5H {item.usage5h} / 1W {item.weekly}
        </span>
      ))}
    </div>
  );
};

/// 聚合工具整体用量。
export const aggregateToolUsage = (
  sessions: readonly PanelSessionListItem[],
): readonly ToolUsageSummaryItem[] => {
  return (["Codex", "Claude"] as const)
    .map((family) => {
      const familySessions = sessions.filter(
        (session) => toolFamily(session) === family,
      );
      const usage5h = latestUsageValue(
        familySessions.map((session) => session.usage_5h),
      );
      const weekly = latestUsageValue(
        familySessions.map((session) => session.usage_weekly),
      );
      if (usage5h === null && weekly === null) {
        return null;
      }

      return {
        family,
        usage5h: usage5h?.label ?? "--",
        weekly: weekly?.label ?? "--",
        tooltip: [usage5h?.tooltip, weekly?.tooltip]
          .filter((item) => item !== undefined)
          .join("\n"),
      };
    })
    .filter((item) => item !== null);
};

/// 工具用量候选。
interface LatestUsageCandidate {
  /// 展示标签。
  readonly label: string;
  /// 更新时间。
  readonly updatedAt: number;
  /// tooltip。
  readonly tooltip: string;
}

/// 取最新用量值。
const latestUsageValue = (
  values: readonly PanelSessionListItem["usage_5h"][],
): LatestUsageCandidate | null => {
  const bySource = new Map<string, LatestUsageCandidate>();
  for (const value of values) {
    if (
      value.amount_label === null ||
      value.source_key === null ||
      value.scope !== "account_window"
    ) {
      continue;
    }
    const updatedAt = value.updated_at?.value ?? 0;
    const candidate = {
      label: value.value_label,
      updatedAt,
      tooltip: `${value.source_label ?? value.source_key} ${value.value_label}`,
    };
    const current = bySource.get(value.source_key);
    if (current === undefined || current.updatedAt <= updatedAt) {
      bySource.set(value.source_key, candidate);
    }
  }

  return (
    [...bySource.values()].sort(
      (left, right) => right.updatedAt - left.updatedAt,
    )[0] ?? null
  );
};

/// 返回工具族。
const toolFamily = (
  session: PanelSessionListItem,
): "Codex" | "Claude" | null => {
  switch (session.session_key.agent_kind) {
    case "codex_app":
    case "codex_cli":
      return "Codex";
    case "claude_code_app":
    case "claude_code_cli":
      return "Claude";
  }
};

/// Session 行属性。
interface SessionRowProps {
  /// 当前 session。
  readonly session: PanelSessionListItem;
  /// 是否选中。
  readonly selected: boolean;
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// mock UI 状态。
  readonly mockUiState: MockPanelUiState;
  /// 跳回回调。
  readonly onJump: (session: PanelSessionListItem) => void;
  /// 草稿变化回调。
  readonly onDraftChange: (
    session: PanelSessionListItem,
    draft: string,
  ) => void;
  /// 审批回调。
  readonly onResolveApproval: SessionStreamProps["onResolveApproval"];
  /// 回复回调。
  readonly onSendReply: SessionStreamProps["onSendReply"];
  /// 切换选项回调。
  readonly onToggleChoice: SessionStreamProps["onToggleChoice"];
  /// 提交选项回调。
  readonly onSubmitChoice: SessionStreamProps["onSubmitChoice"];
  /// 打开时间线回调。
  readonly onOpenTimeline: (session: PanelSessionListItem) => void;
  /// 创建后续 turn 回调。
  readonly onCreateFollowupTurn: SessionStreamProps["onCreateFollowupTurn"];
}

/// Session 行。
const SessionRow = ({
  session,
  selected,
  settings,
  mockUiState,
  onJump,
  onDraftChange,
  onResolveApproval,
  onSendReply,
  onToggleChoice,
  onSubmitChoice,
  onOpenTimeline,
  onCreateFollowupTurn,
}: SessionRowProps) => {
  const interaction = session.inline_interaction;
  const interactionId = interaction.interaction_id;
  const draft =
    mockUiState.draftsBySessionId[sessionKeyToId(session.session_key)] ?? "";
  const selectedChoiceValues =
    interactionId === null
      ? []
      : (mockUiState.selectedChoicesByInteractionId[interactionId.value] ?? []);
  const submitting =
    interactionId !== null &&
    mockUiState.submittingInteractionId === interactionId.value;
  const followupSubmitId = `followup-${sessionKeyToId(session.session_key)}`;
  const followupSubmitting =
    mockUiState.submittingInteractionId === followupSubmitId;
  const expanded = canExpandSessionRow(
    session.status_kind,
    interaction.can_create_followup_turn,
  );
  const shortcuts = sortedEnabledShortcuts(settings.replies.custom_shortcuts);

  return (
    <article
      className={
        selected ? "session-card session-card-selected" : "session-card"
      }
      onClick={() => {
        onJump(session);
      }}
    >
      <div className="session-row-main">
        <span
          className={`session-status session-status-${session.status_kind}`}
        >
          {session.status_label}
        </span>
        <span className="session-source">{sourceTag(session)}</span>
        <strong>{session.project_label}</strong>
        <p title={session.summary.text}>
          {lastParagraph(session.summary.text)}
        </p>
      </div>
      {expanded && (
        <div
          className="session-row-action"
          onClick={(event) => {
            event.stopPropagation();
          }}
        >
          <span>
            {interaction.summary ?? lastParagraph(session.summary.text)}
          </span>
          {interaction.can_resolve_approval && interactionId !== null && (
            <div className="button-row">
              <button
                disabled={submitting}
                type="button"
                onClick={() => {
                  onResolveApproval(session, interactionId, "allow");
                }}
              >
                允许
              </button>
              <button
                disabled={submitting}
                type="button"
                onClick={() => {
                  onResolveApproval(session, interactionId, "deny");
                }}
              >
                拒绝
              </button>
              <button
                disabled={submitting}
                type="button"
                onClick={() => {
                  onResolveApproval(
                    session,
                    interactionId,
                    "allow_and_remember",
                  );
                }}
              >
                允许并记住
              </button>
            </div>
          )}
          {interaction.kind === "choice" && interactionId !== null && (
            <div className="choice-box">
              <div className="choice-list">
                {interaction.choice_box.choices.map((choice) => (
                  <label
                    className="choice-row"
                    key={choice.value}
                    title={choice.tooltip ?? choice.label}
                  >
                    <input
                      checked={selectedChoiceValues.includes(choice.value)}
                      name={interactionId.value}
                      type={
                        interaction.choice_box.allows_multiple
                          ? "checkbox"
                          : "radio"
                      }
                      value={choice.value}
                      onChange={() => {
                        onToggleChoice(
                          interactionId,
                          choice.value,
                          interaction.choice_box.allows_multiple,
                        );
                      }}
                    />
                    <span>{choice.label}</span>
                  </label>
                ))}
              </div>
              <div className="button-row">
                <button
                  disabled={selectedChoiceValues.length === 0 || submitting}
                  type="button"
                  onClick={() => {
                    onSubmitChoice(
                      session,
                      interactionId,
                      selectedChoiceValues,
                    );
                  }}
                >
                  提交选择
                </button>
              </div>
            </div>
          )}
          {interaction.kind !== "choice" &&
            (interaction.can_send_reply ||
              interaction.can_create_followup_turn) &&
            settings.replies.shortcut_replies_enabled &&
            shortcuts.length > 0 && (
              <div className="shortcut-row" aria-label="快捷输入">
                {shortcuts.map((shortcut) => (
                  <button
                    disabled={submitting || followupSubmitting}
                    key={shortcut.id}
                    title={shortcut.content}
                    type="button"
                    onClick={() => {
                      if (
                        shouldUseFollowupShortcut(
                          session.status_kind,
                          interaction.can_create_followup_turn,
                        )
                      ) {
                        onCreateFollowupTurn(session, shortcut.content);
                        return;
                      }
                      if (interactionId !== null) {
                        onSendReply(session, interactionId, shortcut.content);
                      }
                    }}
                  >
                    {shortcut.label}
                  </button>
                ))}
              </div>
            )}
          {interaction.kind === "text_reply" && interactionId !== null && (
            <div className="inline-reply">
              <textarea
                disabled={submitting}
                value={draft}
                placeholder="输入回复"
                onChange={(event) => {
                  onDraftChange(session, event.target.value);
                }}
                onKeyDown={(event) => {
                  if (
                    shouldSubmitReplyOnKeyDown(
                      settings.replies.enter_to_send,
                      event.key,
                      event.shiftKey,
                    ) &&
                    !submitting
                  ) {
                    event.preventDefault();
                    onSendReply(session, interactionId, null);
                  }
                }}
              />
              <button
                disabled={isReplyDraftInvalid(draft, 1000) || submitting}
                type="button"
                onClick={() => {
                  onSendReply(session, interactionId, null);
                }}
              >
                发送
              </button>
            </div>
          )}
          {interaction.can_view_process_timeline && (
            <button
              type="button"
              onClick={() => {
                onOpenTimeline(session);
              }}
            >
              Timeline
            </button>
          )}
        </div>
      )}
    </article>
  );
};

/// 返回来源标签。
const sourceTag = (session: PanelSessionListItem): string => {
  switch (session.runtimeSource) {
    case "codex_app":
      return "codex";
    case "codex_cli":
      return "codex-cli";
  }
};

/// 返回最后一段文本。
const lastParagraph = (text: string): string => {
  const paragraphs = text
    .split(/\n\s*\n|\n/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
  return paragraphs.at(-1) ?? "";
};

/// 返回排序后的启用快捷输入。
const sortedEnabledShortcuts = (
  shortcuts: readonly CustomShortcutInput[],
): readonly CustomShortcutInput[] => {
  return [...shortcuts]
    .filter((shortcut) => shortcut.enabled)
    .sort((left, right) => {
      if (left.order !== right.order) {
        return left.order - right.order;
      }
      if (left.label !== right.label) {
        return left.label.localeCompare(right.label);
      }
      return left.id.localeCompare(right.id);
    });
};

/// Session 详情属性。
interface SessionDetailProps {
  /// Session 详情。
  readonly detail: SessionDetailViewModel | null;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem | null;
  /// 当前 session 回复草稿。
  readonly draft: string;
  /// 当前交互已选选项。
  readonly selectedChoiceValues: readonly string[];
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 当前提交中的交互 ID。
  readonly submittingInteractionId: string | null;
  /// 草稿更新回调。
  readonly onDraftChange: (draft: string) => void;
  /// 审批回调。
  readonly onResolveApproval: (decision: ApprovalDecision) => void;
  /// 回复回调。
  readonly onSendReply: () => void;
  /// 快捷回复回调。
  readonly onUseShortcutReply: (content: string) => void;
  /// 切换选项回调。
  readonly onToggleChoice: (
    choiceValue: string,
    allowsMultiple: boolean,
  ) => void;
  /// 提交选项回调。
  readonly onSubmitChoice: () => void;
  /// 打开时间线回调。
  readonly onOpenTimeline: (sessionKey: SessionKey) => void;
  /// 创建后续 turn 回调。
  readonly onCreateFollowupTurn: (content: string) => void;
}

/// Session 详情。
export const SessionDetail = ({
  detail,
  selectedSession,
  draft,
  selectedChoiceValues,
  settings,
  submittingInteractionId,
  onDraftChange,
  onResolveApproval,
  onSendReply,
  onUseShortcutReply,
  onToggleChoice,
  onSubmitChoice,
  onOpenTimeline,
  onCreateFollowupTurn,
}: SessionDetailProps) => {
  const [detailOverlayOpen, setDetailOverlayOpen] = useState(false);
  const [replyComposerOpen, setReplyComposerOpen] = useState(false);
  const [followupComposerOpen, setFollowupComposerOpen] = useState(false);

  if (selectedSession === null || detail === null) {
    return <section className="session-detail-empty">暂无 session</section>;
  }

  const pendingId = detail.pending_interaction_id?.value ?? null;
  const submitting =
    pendingId !== null && submittingInteractionId === pendingId;
  const canResolveApproval =
    detail.toolbar_actions.includes("resolve_approval") &&
    detail.pending_interaction_kind === "approval";
  const canSendReply =
    detail.toolbar_actions.includes("send_reply") &&
    detail.pending_interaction_kind === "text_reply";
  const canSubmitChoice =
    detail.toolbar_actions.includes("send_reply") &&
    detail.pending_interaction_kind === "choice" &&
    detail.choice_box.enabled;
  const canOpenTimeline = canShowTimelineEntry(detail.toolbar_actions);
  const canCreateFollowup =
    selectedSession.runtimeSource === "codex_app" &&
    detail.toolbar_actions.includes("create_followup_turn");
  const replyCharCount = countReplyChars(draft);
  const replyInvalid = isReplyDraftInvalid(draft, 1000);

  return (
    <>
      <section className="session-detail" aria-label="session 详情">
        <header>
          <div>
            <strong>{detail.header}</strong>
            <p>{detail.identity}</p>
          </div>
          <span>{detail.execution_info}</span>
        </header>
        <div className="detail-summary-line">
          <p className="detail-summary">{detail.summary.text}</p>
          <button
            type="button"
            onClick={() => {
              setDetailOverlayOpen(true);
            }}
          >
            详情
          </button>
        </div>
        {settings.display.show_usage && (
          <div className="detail-metrics">
            <span>{detail.usage}</span>
            <span>{selectedSession.usage_5h.value_label}</span>
          </div>
        )}
        {detail.pending_interaction !== null && (
          <div className="pending-box">
            <strong>{detail.pending_interaction}</strong>
            {canResolveApproval && (
              <div className="button-row">
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("allow");
                  }}
                >
                  允许
                </button>
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("deny");
                  }}
                >
                  拒绝
                </button>
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("allow_and_remember");
                  }}
                >
                  允许并记住
                </button>
              </div>
            )}
            {canSendReply && (
              <div className="reply-inline">
                <div className="shortcut-row" aria-label="快捷回复">
                  {settings.replies.shortcut_replies_enabled &&
                    sortedEnabledShortcuts(
                      settings.replies.custom_shortcuts,
                    ).map((shortcut) => (
                      <button
                        key={shortcut.id}
                        type="button"
                        disabled={submitting}
                        onClick={() => {
                          onUseShortcutReply(shortcut.content);
                        }}
                      >
                        {shortcut.label}
                      </button>
                    ))}
                  <button
                    type="button"
                    disabled={submitting}
                    onClick={() => {
                      setReplyComposerOpen(true);
                    }}
                  >
                    打开回复
                  </button>
                </div>
                <span className={replyInvalid ? "reply-count-invalid" : ""}>
                  {replyCharCount}/1000
                </span>
              </div>
            )}
            {canSubmitChoice && (
              <div className="choice-box">
                <div className="choice-list">
                  {detail.choice_box.choices.map((choice) => {
                    const checked = selectedChoiceValues.includes(choice.value);
                    return (
                      <label className="choice-row" key={choice.value}>
                        <input
                          checked={checked}
                          name={pendingId ?? "choice"}
                          type={
                            detail.choice_box.allows_multiple
                              ? "checkbox"
                              : "radio"
                          }
                          value={choice.value}
                          onChange={() => {
                            onToggleChoice(
                              choice.value,
                              detail.choice_box.allows_multiple,
                            );
                          }}
                        />
                        <span>{choice.label}</span>
                      </label>
                    );
                  })}
                </div>
                <div className="button-row">
                  <button
                    type="button"
                    disabled={selectedChoiceValues.length === 0 || submitting}
                    onClick={() => {
                      onSubmitChoice();
                    }}
                  >
                    提交选择
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
        {(canOpenTimeline || canCreateFollowup) && (
          <footer>
            {canCreateFollowup && (
              <button
                type="button"
                onClick={() => {
                  setFollowupComposerOpen(true);
                }}
              >
                Follow-up
              </button>
            )}
            {canOpenTimeline && (
              <button
                type="button"
                onClick={() => {
                  onOpenTimeline(selectedSession.session_key);
                }}
              >
                Timeline
              </button>
            )}
          </footer>
        )}
      </section>
      {detailOverlayOpen && (
        <SessionDetailOverlay
          detail={detail}
          selectedSession={selectedSession}
          onClose={() => {
            setDetailOverlayOpen(false);
          }}
        />
      )}
      {replyComposerOpen && canSendReply && (
        <ReplyComposerOverlay
          draft={draft}
          replyInvalid={replyInvalid}
          settings={settings}
          submitting={submitting}
          onClose={() => {
            setReplyComposerOpen(false);
          }}
          onDraftChange={onDraftChange}
          onSendReply={onSendReply}
        />
      )}
      {followupComposerOpen && canCreateFollowup && (
        <FollowupComposerOverlay
          draft={draft}
          replyInvalid={replyInvalid}
          submitting={submittingInteractionId !== null}
          onClose={() => {
            setFollowupComposerOpen(false);
          }}
          onDraftChange={onDraftChange}
          onCreateFollowupTurn={onCreateFollowupTurn}
        />
      )}
    </>
  );
};

/// Session 详情弹层属性。
interface SessionDetailOverlayProps {
  /// Session 详情。
  readonly detail: SessionDetailViewModel;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem;
  /// 关闭回调。
  readonly onClose: () => void;
}

/// Session 详情弹层。
const SessionDetailOverlay = ({
  detail,
  selectedSession,
  onClose,
}: SessionDetailOverlayProps) => (
  <div className="overlay-backdrop" role="presentation">
    <section className="overlay-panel session-detail-overlay" aria-label="详情">
      <header>
        <div>
          <strong>{detail.header}</strong>
          <p>
            {selectedSession.agent_label} / {selectedSession.conversation_label}
          </p>
        </div>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </header>
      <dl>
        <div>
          <dt>身份</dt>
          <dd>{detail.identity}</dd>
        </div>
        <div>
          <dt>执行</dt>
          <dd>{detail.execution_info}</dd>
        </div>
        <div>
          <dt>摘要</dt>
          <dd>{detail.summary.text}</dd>
        </div>
      </dl>
    </section>
  </div>
);

/// 回复输入弹层属性。
interface ReplyComposerOverlayProps {
  /// 当前草稿。
  readonly draft: string;
  /// 当前草稿是否非法。
  readonly replyInvalid: boolean;
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 是否正在提交。
  readonly submitting: boolean;
  /// 关闭回调。
  readonly onClose: () => void;
  /// 草稿更新回调。
  readonly onDraftChange: (draft: string) => void;
  /// 回复发送回调。
  readonly onSendReply: () => void;
}

/// 回复输入弹层。
const ReplyComposerOverlay = ({
  draft,
  replyInvalid,
  settings,
  submitting,
  onClose,
  onDraftChange,
  onSendReply,
}: ReplyComposerOverlayProps) => (
  <div className="overlay-backdrop" role="presentation">
    <section className="overlay-panel reply-composer" aria-label="回复输入">
      <header>
        <div>
          <strong>Reply</strong>
          <p>{settings.replies.enter_to_send ? "Enter 发送" : "手动发送"}</p>
        </div>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </header>
      <textarea
        value={draft}
        autoFocus={true}
        placeholder={
          settings.replies.enter_to_send
            ? "输入回复，Enter 发送，Shift+Enter 换行"
            : "输入回复"
        }
        onChange={(event) => {
          onDraftChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (
            !settings.replies.enter_to_send ||
            event.key !== "Enter" ||
            event.shiftKey
          ) {
            return;
          }
          event.preventDefault();
          if (!replyInvalid && !submitting) {
            onSendReply();
          }
        }}
      />
      <div className="button-row">
        <span className={replyInvalid ? "reply-count-invalid" : ""}>
          {countReplyChars(draft)}/1000
        </span>
        <button
          type="button"
          disabled={replyInvalid || submitting}
          onClick={() => {
            onSendReply();
          }}
        >
          发送
        </button>
      </div>
    </section>
  </div>
);

/// Follow-up 输入弹层属性。
interface FollowupComposerOverlayProps {
  /// 当前草稿。
  readonly draft: string;
  /// 当前草稿是否非法。
  readonly replyInvalid: boolean;
  /// 是否正在提交。
  readonly submitting: boolean;
  /// 关闭回调。
  readonly onClose: () => void;
  /// 草稿更新回调。
  readonly onDraftChange: (draft: string) => void;
  /// 创建 follow-up 回调。
  readonly onCreateFollowupTurn: (content: string) => void;
}

/// Follow-up 输入弹层。
const FollowupComposerOverlay = ({
  draft,
  replyInvalid,
  submitting,
  onClose,
  onDraftChange,
  onCreateFollowupTurn,
}: FollowupComposerOverlayProps) => (
  <div className="overlay-backdrop" role="presentation">
    <section className="overlay-panel reply-composer" aria-label="Follow-up">
      <header>
        <div>
          <strong>Follow-up</strong>
          <p>Codex APP</p>
        </div>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </header>
      <textarea
        value={draft}
        autoFocus={true}
        placeholder="输入下一轮指令"
        onChange={(event) => {
          onDraftChange(event.target.value);
        }}
      />
      <div className="button-row">
        <span className={replyInvalid ? "reply-count-invalid" : ""}>
          {countReplyChars(draft)}/1000
        </span>
        <button
          type="button"
          disabled={replyInvalid || submitting}
          onClick={() => {
            onCreateFollowupTurn(draft);
          }}
        >
          发送
        </button>
      </div>
    </section>
  </div>
);

/// Timeline 弹层属性。
interface TimelineOverlayProps {
  /// Mock panel UI 状态。
  readonly state: MockPanelUiState;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem;
  /// 关闭回调。
  readonly onClose: () => void;
  /// 搜索回调。
  readonly onSearch: (search: string) => void;
  /// 类型筛选回调。
  readonly onKind: (kind: TimelineKindFilter) => void;
}

/// Timeline 弹层。
const TimelineOverlay = ({
  state,
  selectedSession,
  onClose,
  onSearch,
  onKind,
}: TimelineOverlayProps) => {
  const listRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const items = state.timeline.page?.items ?? [];
  const range = timelineVisibleRange(
    items.length,
    scrollTop,
    TIMELINE_VIEWPORT_HEIGHT,
    TIMELINE_ROW_HEIGHT,
    TIMELINE_OVERSCAN,
  );
  const visibleItems = items.slice(range.start, range.end);
  const topSpacerHeight = range.start * TIMELINE_ROW_HEIGHT;
  const bottomSpacerHeight = (items.length - range.end) * TIMELINE_ROW_HEIGHT;
  const canCopyFiltered =
    state.timeline.page !== null && state.timeline.page.items.length > 0;

  return (
    <div className="overlay-backdrop" role="presentation">
      <section
        className="overlay-panel timeline-overlay"
        aria-label="过程时间线"
      >
        <header>
          <div>
            <strong>Timeline</strong>
            <p>
              {selectedSession.project_label} /{" "}
              {selectedSession.conversation_label}
            </p>
          </div>
          <button type="button" onClick={onClose}>
            关闭
          </button>
        </header>
        <div className="timeline-filters">
          <input
            value={state.timeline.search}
            placeholder="搜索"
            onChange={(event) => {
              onSearch(event.target.value);
            }}
          />
          <select
            value={state.timeline.kind}
            onChange={(event) => {
              onKind(event.target.value as TimelineKindFilter);
            }}
          >
            <option value="all">全部</option>
            <option value="activity">活动</option>
            <option value="tool">工具</option>
            <option value="approval">审批</option>
            <option value="reply">回复</option>
            <option value="system">系统</option>
          </select>
        </div>
        <div className="timeline-actions">
          <span>
            {state.timeline.page === null
              ? "0 条"
              : `${state.timeline.page.total} 条`}
          </span>
          <button
            type="button"
            disabled={!canCopyFiltered}
            onClick={() => {
              if (state.timeline.page !== null) {
                void copyText(timelinePageToCopyText(state.timeline.page));
              }
            }}
          >
            复制筛选结果
          </button>
          <button
            type="button"
            disabled={items.length === 0}
            onClick={() => {
              const list = listRef.current;
              if (list !== null) {
                list.scrollTop = list.scrollHeight;
              }
            }}
          >
            跳到最新
          </button>
        </div>
        {state.timeline.loading && <p className="timeline-empty">读取中</p>}
        {state.timeline.errorMessage !== null && (
          <p className="timeline-empty">{state.timeline.errorMessage}</p>
        )}
        <div
          className="timeline-list"
          ref={listRef}
          onScroll={(event) => {
            setScrollTop(event.currentTarget.scrollTop);
          }}
        >
          <div style={{ height: topSpacerHeight }} />
          {visibleItems.map((item) => (
            <TimelineRow item={item} key={item.item_id} />
          ))}
          <div style={{ height: bottomSpacerHeight }} />
          {state.timeline.page !== null &&
            state.timeline.page.items.length === 0 &&
            !state.timeline.loading && (
              <p className="timeline-empty">没有匹配的过程事件</p>
            )}
        </div>
      </section>
    </div>
  );
};

/// Timeline 行属性。
interface TimelineRowProps {
  /// Timeline 条目。
  readonly item: ProcessTimelineItem;
}

/// Timeline 行。
const TimelineRow = ({ item }: TimelineRowProps) => (
  <article className="timeline-row">
    <div>
      <strong>{item.title}</strong>
      <p>{item.body}</p>
    </div>
    <button
      type="button"
      onClick={() => {
        void copyTimelineItem(item);
      }}
    >
      复制
    </button>
  </article>
);

/// 复制单条 timeline 文本。
const copyTimelineItem = async (item: ProcessTimelineItem): Promise<void> => {
  await copyText(`${item.title}
${item.body}`);
};

/// 复制纯文本。
const copyText = async (text: string): Promise<void> => {
  const clipboard = navigator.clipboard;
  if (clipboard === undefined) {
    return;
  }

  await clipboard.writeText(text);
};

/// 归一错误消息。
const readableError = (error: unknown, fallback: string): string => {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }
  if (typeof error === "string" && error.length > 0) {
    return error;
  }

  return fallback;
};
