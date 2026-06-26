import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import {
  fetchCodexAppSessions,
  injectCodexAppFollowup,
  submitCodexAppApproval,
  submitCodexAppChoice,
  submitCodexAppReply,
} from "../api/codexAppPanelApi";
import {
  fetchCodexCliSessions,
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
  applyPanelWindowPreferences,
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
  SessionDetailViewModel,
  SessionListItemViewModel,
  TextDisplay,
  UiAction,
} from "../api/mockPanelContract";
import {
  defaultSettings,
  fetchLogInfo,
  fetchPanelSettings,
  openLogFolder,
  savePanelSettings,
} from "../api/settingsApi";
import { subscribeSessionUpdates } from "../api/sessionUpdateApi";
import type {
  BuilderPanelSettings,
  CustomShortcutInput,
  SettingsViewModel,
} from "../api/settingsContract";
import { PanelShell } from "../components/PanelShell";
import { SettingsPanel } from "../components/SettingsPanel";
import {
  beginSubmit,
  clearChoiceSelection,
  clearDraft,
  countReplyChars,
  createDefaultMockPanelUiState,
  endSubmit,
  clearFollowupSessionExpansion,
  isFollowupSessionExpanded,
  isReplyDraftInvalid,
  selectSession,
  sessionKeyToId,
  toggleFollowupSessionExpansion,
  toggleChoiceSelection,
  updateDraft,
  type MockPanelUiState,
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
/// 后端实时更新触发前端刷新时的短延迟。
export const SESSION_UPDATE_REFRESH_DELAY_MS = 350;

/// Builder Panel 首屏应用。
export const BuilderPanelApp = () => {
  const [mockUiState, setMockUiState] = useState<MockPanelUiState>(() =>
    createDefaultMockPanelUiState(),
  );
  const [sessions, setSessions] = useState<readonly PanelSessionListItem[]>([]);
  const sessionsRef = useRef<readonly PanelSessionListItem[]>([]);
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
  const [logPath, setLogPath] = useState<string | null>(null);
  const settingsSaveVersion = useRef(0);
  const panelWindowPreferencesApplied = useRef(false);
  const panelGeometrySaveTimer = useRef<number | null>(null);
  const pendingPanelWindowUpdate = useRef<PanelWindowStateUpdate>({});

  const applySessionRefresh = (
    items: readonly PanelSessionListItem[],
  ): void => {
    const nextSessions = mergePanelSessionsByCaptureOrder(
      sessionsRef.current,
      items,
    );
    sessionsRef.current = nextSessions;
    setSessions(nextSessions);
    setMockUiState((current) =>
      selectFirstSessionWhenMissing(current, nextSessions),
    );
  };

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
    let disposed = false;
    fetchLogInfo()
      .then((info) => {
        if (!disposed && info !== null) {
          setLogPath(info.path);
        }
      })
      .catch(() => {
        // 日志信息获取失败不影响主流程。
      });
    return () => {
      disposed = true;
    };
  }, [settingsView.settings.logging.enabled]);

  useEffect(() => {
    if (!settingsHydrated || panelWindowPreferencesApplied.current) {
      return;
    }

    panelWindowPreferencesApplied.current = true;
    applyPanelWindowPreferences(settingsView.settings).catch(
      (error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "应用 panel 窗口设置失败"),
        }));
      },
    );
  }, [settingsHydrated, settingsView.settings]);

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
    let unsubscribe: (() => void) | null = null;
    const scheduler = createSessionRefreshScheduler({
      eventDelayMs: SESSION_UPDATE_REFRESH_DELAY_MS,
      refresh: async () => {
        const items = await fetchAllSessions(settingsView.settings);
        if (!disposed) {
          applySessionRefresh(items);
        }
      },
      reportError: (error) => {
        if (!disposed) {
          setMockUiState((current) =>
            endSubmit(current, readableError(error, "读取 session 失败")),
          );
        }
      },
      scheduleTimeout: (handler, delayMs) =>
        window.setTimeout(handler, delayMs),
      clearTimeout: (timerId) => {
        window.clearTimeout(timerId);
      },
    });

    scheduler.requestImmediate();
    const timer = window.setInterval(() => {
      scheduler.requestImmediate();
    }, SESSION_REFRESH_INTERVAL_MS);

    subscribeSessionUpdates(() => {
      scheduler.requestEvent();
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        unsubscribe = unlisten;
      })
      .catch((error: unknown) => {
        setMockUiState((current) =>
          endSubmit(current, readableError(error, "监听 session 更新失败")),
        );
      });

    return () => {
      disposed = true;
      scheduler.dispose();
      window.clearInterval(timer);
      if (unsubscribe !== null) {
        unsubscribe();
      }
    };
  }, [settingsView.settings]);

  const refreshSessions = async (): Promise<void> => {
    const nextSessions = await fetchAllSessions(settingsView.settings);
    applySessionRefresh(nextSessions);
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
      void applyLatestSavedPanelWindowPreferences(
        true,
        saveVersion,
        settingsSaveVersion.current,
        nextView.settings,
        applyPanelWindowPreferences,
      ).catch((error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "应用 panel 窗口设置失败"),
        }));
      });
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
      await injectCodexAppFollowup({
        session_key: session.session_key,
        prompt,
      });
      await refreshSessions();
      setMockUiState((current) =>
        endSubmit(
          clearFollowupSessionExpansion(
            clearDraft(current, session.session_key),
            session.session_key,
          ),
          null,
        ),
      );
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "follow-up 注入失败")),
      );
    }
  };

  const jumpToPanelSession = async (
    session: PanelSessionListItem,
  ): Promise<void> => {
    if (!canSelectOnSessionClick(session.actions)) {
      return;
    }

    setMockUiState((current) => selectPanelSession(current, session));
    if (!canJumpOnSessionClick(session.actions, settingsView.settings)) {
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
          onToggleFollowupExpansion={(session) => {
            setMockUiState((current) =>
              toggleFollowupSessionExpansion(current, session.session_key),
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
                logPath={logPath}
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
                onOpenLogFolder={() => {
                  void openLogFolder().catch(() => {
                    // 失败提示由后端写入日志。
                  });
                }}
              />
            </section>
          </div>
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

/// Session 刷新调度器依赖。
interface SessionRefreshSchedulerOptions<TimerId> {
  /// 后端事件到前端刷新之间的短延迟。
  readonly eventDelayMs: number;
  /// 执行一次真实 session 刷新。
  readonly refresh: () => Promise<void>;
  /// 刷新失败收敛。
  readonly reportError: (error: unknown) => void;
  /// 注册延迟任务。
  readonly scheduleTimeout: (handler: () => void, delayMs: number) => TimerId;
  /// 取消延迟任务。
  readonly clearTimeout: (timerId: TimerId) => void;
}

/// Session 刷新调度器。
export interface SessionRefreshScheduler {
  /// 立即请求刷新，用于启动和定时兜底。
  readonly requestImmediate: () => void;
  /// 后端事件触发的短延迟刷新。
  readonly requestEvent: () => void;
  /// 释放未执行的延迟任务。
  readonly dispose: () => void;
}

/// 创建 session 列表刷新调度器。
export const createSessionRefreshScheduler = <TimerId,>({
  eventDelayMs,
  refresh,
  reportError,
  scheduleTimeout,
  clearTimeout,
}: SessionRefreshSchedulerOptions<TimerId>): SessionRefreshScheduler => {
  let disposed = false;
  let running = false;
  let queued = false;
  let eventTimer: TimerId | null = null;

  const clearEventTimer = (): void => {
    if (eventTimer === null) {
      return;
    }
    clearTimeout(eventTimer);
    eventTimer = null;
  };

  const run = (): void => {
    if (disposed) {
      return;
    }
    if (running) {
      queued = true;
      return;
    }

    running = true;
    void refresh()
      .catch(reportError)
      .finally(() => {
        running = false;
        if (!queued || disposed) {
          return;
        }
        queued = false;
        run();
      });
  };

  return {
    requestImmediate: () => {
      clearEventTimer();
      run();
    },
    requestEvent: () => {
      if (disposed) {
        return;
      }
      if (running) {
        queued = true;
        return;
      }
      if (eventTimer !== null) {
        return;
      }
      eventTimer = scheduleTimeout(() => {
        eventTimer = null;
        run();
      }, eventDelayMs);
    },
    dispose: () => {
      disposed = true;
      clearEventTimer();
      queued = false;
    },
  };
};

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

  return [
    ...codexAppSessions.map((session) =>
      withRuntimeSource(session, "codex_app"),
    ),
    ...codexCliSessions.map((session) =>
      withRuntimeSource(session, "codex_cli"),
    ),
  ];
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

/// 按首次捕捉顺序合并 session，新的 session 放到列表顶部。
export const mergePanelSessionsByCaptureOrder = (
  previousSessions: readonly PanelSessionListItem[],
  nextSessions: readonly PanelSessionListItem[],
): readonly PanelSessionListItem[] => {
  const previousIds = new Set(previousSessions.map(panelSessionToId));
  const nextById = new Map(
    nextSessions.map((session) => [panelSessionToId(session), session]),
  );
  const newlyCaptured = nextSessions.filter(
    (session) => !previousIds.has(panelSessionToId(session)),
  );
  const existingInCaptureOrder = previousSessions
    .map((session) => nextById.get(panelSessionToId(session)) ?? null)
    .filter((session) => session !== null);

  return [...newlyCaptured, ...existingInCaptureOrder];
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

/// 保存响应仍是最新请求时，应用当前窗口偏好。
export const applyLatestSavedPanelWindowPreferences = async (
  saveSucceeded: boolean,
  responseVersion: number,
  currentVersion: number,
  settings: BuilderPanelSettings,
  applyPreferences: (settings: BuilderPanelSettings) => Promise<void>,
): Promise<boolean> => {
  if (!saveSucceeded) {
    return false;
  }
  if (!isLatestSettingsSaveResponse(responseVersion, currentVersion)) {
    return false;
  }

  await applyPreferences(settings);
  return true;
};

/// 创建默认 hook 安装状态。
export const defaultHookAgentStatuses =
  (): readonly HookInstallAgentStatus[] => [
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

/// 判断点击 session 行是否应跳回。
export const canJumpOnSessionClick = (
  actions: readonly UiAction[],
  settings: BuilderPanelSettings,
): boolean => settings.terminal.jump_enabled && actions.includes("jump");

/// 判断点击 session 行是否可选中。
export const canSelectOnSessionClick = (
  actions: readonly UiAction[],
): boolean => actions.includes("jump");

/// 判断回复输入键盘事件是否应提交。
export const shouldSubmitReplyOnKeyDown = (
  enterToSend: boolean,
  key: string,
  shiftKey: boolean,
): boolean => enterToSend && key === "Enter" && !shiftKey;

/// 判断 session 行是否因待处理交互自动展示第二行。
export const shouldAutoShowSessionActionRow = (
  statusKind: PanelSessionListItem["status_kind"],
): boolean => {
  return (
    statusKind === "waiting_for_approval" || statusKind === "waiting_for_answer"
  );
};

/// 判断 session 是否可手动展开 follow-up 输入区。
export const canToggleFollowupRow = (
  statusKind: PanelSessionListItem["status_kind"],
  canCreateFollowupTurn: boolean,
): boolean => {
  return (
    canCreateFollowupTurn &&
    (statusKind === "completed" || statusKind === "failed")
  );
};

/// 判断 session 第二行是否应显示。
export const shouldShowSessionActionRow = (
  statusKind: PanelSessionListItem["status_kind"],
  canCreateFollowupTurn: boolean,
  followupExpanded: boolean,
): boolean => {
  if (shouldAutoShowSessionActionRow(statusKind)) {
    return true;
  }

  return (
    canToggleFollowupRow(statusKind, canCreateFollowupTurn) && followupExpanded
  );
};

/// 返回 session 第二行样式类名。
export const sessionActionRowClassName = (
  statusKind: PanelSessionListItem["status_kind"],
): string => {
  if (shouldAutoShowSessionActionRow(statusKind)) {
    return "session-row-action";
  }

  return "session-row-action session-row-action-followup";
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
  }
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
  /// 创建后续 turn 回调。
  readonly onCreateFollowupTurn: (
    session: PanelSessionListItem,
    content: string,
  ) => void;
  /// 切换 follow-up 输入区回调。
  readonly onToggleFollowupExpansion: (session: PanelSessionListItem) => void;
}

/// Session 流。
export const SessionStream = ({
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
  onCreateFollowupTurn,
  onToggleFollowupExpansion,
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
        onResolveApproval={onResolveApproval}
        onSendReply={onSendReply}
        onSubmitChoice={onSubmitChoice}
        onToggleChoice={onToggleChoice}
        onToggleFollowupExpansion={onToggleFollowupExpansion}
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
  /// 创建后续 turn 回调。
  readonly onCreateFollowupTurn: SessionStreamProps["onCreateFollowupTurn"];
  /// 切换 follow-up 输入区回调。
  readonly onToggleFollowupExpansion: SessionStreamProps["onToggleFollowupExpansion"];
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
  onCreateFollowupTurn,
  onToggleFollowupExpansion,
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
  const canToggleFollowup = canToggleFollowupRow(
    session.status_kind,
    interaction.can_create_followup_turn,
  );
  const followupExpanded =
    canToggleFollowup &&
    isFollowupSessionExpanded(mockUiState, session.session_key);
  const expanded = shouldShowSessionActionRow(
    session.status_kind,
    interaction.can_create_followup_turn,
    followupExpanded,
  );
  const canCreateFollowup = shouldUseFollowupShortcut(
    session.status_kind,
    interaction.can_create_followup_turn,
  );
  const showActionSummary = shouldAutoShowSessionActionRow(session.status_kind);
  const shortcuts = sortedEnabledShortcuts(settings.replies.custom_shortcuts);
  const summaryParagraph = textDisplayParagraph(session.summary);
  const actionSummary = interaction.summary ?? summaryParagraph.visibleText;
  const actionSummaryTooltip =
    interaction.summary ?? summaryParagraph.tooltipText;

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
        <MarkdownTooltip
          className="session-project"
          content={session.project_label}
        >
          <strong>{session.project_label}</strong>
        </MarkdownTooltip>
        <MarkdownTooltip
          className="session-thread"
          content={session.thread_label}
        >
          <strong>{session.thread_label}</strong>
        </MarkdownTooltip>
        <MarkdownTooltip
          className="session-summary-tooltip"
          content={summaryParagraph.tooltipText}
        >
          {summaryParagraph.visibleText}
        </MarkdownTooltip>
        {canToggleFollowup && (
          <button
            aria-expanded={followupExpanded}
            className="session-expand-button"
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onToggleFollowupExpansion(session);
            }}
          >
            {followupExpanded ? "收起" : "展开"}
          </button>
        )}
      </div>
      {expanded && (
        <div
          className={sessionActionRowClassName(session.status_kind)}
          onClick={(event) => {
            event.stopPropagation();
          }}
        >
          {showActionSummary && (
            <MarkdownTooltip
              className="session-action-tooltip"
              content={actionSummaryTooltip}
            >
              {actionSummary}
            </MarkdownTooltip>
          )}
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
          {canCreateFollowup && (
            <div className="inline-reply">
              <textarea
                disabled={followupSubmitting}
                value={draft}
                placeholder="继续输入"
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
                    !followupSubmitting
                  ) {
                    event.preventDefault();
                    onCreateFollowupTurn(session, draft);
                  }
                }}
              />
              <button
                disabled={
                  isReplyDraftInvalid(draft, 1000) || followupSubmitting
                }
                type="button"
                onClick={() => {
                  onCreateFollowupTurn(session, draft);
                }}
              >
                发送
              </button>
            </div>
          )}
        </div>
      )}
    </article>
  );
};

/// 返回来源标签。
export const sourceTag = (session: PanelSessionListItem): string => {
  switch (session.runtimeSource) {
    case "codex_app":
      return "Codex";
    case "codex_cli":
      return "Codex CLI";
  }
};

/// 返回最后一段文本。
const lastParagraph = (text: string): string => {
  const paragraphs = text
    .split(/\n\s*\n/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
  return paragraphs.at(-1) ?? "";
};

/// 单段文本展示模型。
export interface ParagraphDisplay {
  /// 当前可见文本。
  readonly visibleText: string;
  /// 当前完整段落。
  readonly fullParagraph: string;
  /// 当前段落是否被截断。
  readonly paragraphTruncated: boolean;
  /// 当前完整段落 tooltip 文本。
  readonly tooltipText: string | null;
}

/// 从 TextDisplay 中生成当前段落展示。
export const textDisplayParagraph = (
  display: TextDisplay,
): ParagraphDisplay => {
  const fullParagraph = lastParagraph(display.full_text);
  const visibleText = truncateText(fullParagraph, display.max_chars);
  const paragraphTruncated =
    Array.from(fullParagraph).length > display.max_chars;

  return {
    visibleText,
    fullParagraph,
    paragraphTruncated,
    tooltipText: fullParagraph.length > 0 ? fullParagraph : null,
  };
};

/// 按字符数截断文本。
const truncateText = (text: string, maxChars: number): string => {
  const characters = Array.from(text);
  if (characters.length <= maxChars) {
    return text;
  }

  return characters.slice(0, maxChars).join("");
};

interface MarkdownTooltipProps {
  readonly children: ReactNode;
  readonly className?: string;
  readonly content: string | null | undefined;
}

interface TooltipAnchorRect {
  readonly bottom: number;
  readonly left: number;
  readonly top: number;
}

interface TooltipPanelSize {
  readonly height: number;
  readonly width: number;
}

interface TooltipViewportSize {
  readonly height: number;
  readonly width: number;
}

interface TooltipPanelPosition {
  readonly left: number;
  readonly maxWidth: number;
  readonly placement: "bottom" | "top";
  readonly top: number;
}

const TOOLTIP_EDGE_GAP = 8;
const TOOLTIP_ANCHOR_GAP = 6;
const TOOLTIP_MAX_WIDTH = 520;

export const positionTooltipPanel = (
  anchor: TooltipAnchorRect,
  panel: TooltipPanelSize,
  viewport: TooltipViewportSize,
): TooltipPanelPosition => {
  const maxWidth = tooltipMaxWidthForViewport(viewport.width);
  const panelWidth = Math.min(panel.width, maxWidth);
  const belowTop = anchor.bottom + TOOLTIP_ANCHOR_GAP;
  const aboveTop = anchor.top - panel.height - TOOLTIP_ANCHOR_GAP;
  const fitsBelow =
    belowTop + panel.height + TOOLTIP_EDGE_GAP <= viewport.height;
  const canFitAbove = aboveTop >= TOOLTIP_EDGE_GAP;
  const placement = fitsBelow || !canFitAbove ? "bottom" : "top";
  const unclampedTop = placement === "bottom" ? belowTop : aboveTop;
  const top = clamp(
    unclampedTop,
    TOOLTIP_EDGE_GAP,
    Math.max(
      TOOLTIP_EDGE_GAP,
      viewport.height - panel.height - TOOLTIP_EDGE_GAP,
    ),
  );
  const left = clamp(
    anchor.left,
    TOOLTIP_EDGE_GAP,
    Math.max(TOOLTIP_EDGE_GAP, viewport.width - panelWidth - TOOLTIP_EDGE_GAP),
  );

  return { left, maxWidth, placement, top };
};

const tooltipMaxWidthForViewport = (viewportWidth: number): number =>
  Math.max(
    0,
    Math.min(TOOLTIP_MAX_WIDTH, viewportWidth - TOOLTIP_EDGE_GAP * 2),
  );

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), max);

const tooltipPositionsEqual = (
  left: TooltipPanelPosition | null,
  right: TooltipPanelPosition,
): boolean =>
  left !== null &&
  left.left === right.left &&
  left.maxWidth === right.maxWidth &&
  left.placement === right.placement &&
  left.top === right.top;

export const stopTooltipPortalEvent = (event: {
  stopPropagation: () => void;
}): void => {
  event.stopPropagation();
};

const MarkdownTooltip = ({
  children,
  className,
  content,
}: MarkdownTooltipProps) => {
  const hasTooltip =
    content !== null && content !== undefined && content !== "";
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<TooltipPanelPosition | null>(null);
  const anchorRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const portalTarget =
    typeof document === "undefined"
      ? null
      : (anchorRef.current?.closest(".app-surface") ?? document.body);

  const updatePosition = (): void => {
    if (!hasTooltip || anchorRef.current === null) {
      return;
    }

    const anchorRect = anchorRef.current.getBoundingClientRect();
    const panelRect = panelRef.current?.getBoundingClientRect();
    const nextPosition = positionTooltipPanel(
      {
        bottom: anchorRect.bottom,
        left: anchorRect.left,
        top: anchorRect.top,
      },
      {
        height: panelRect?.height ?? 0,
        width:
          panelRect?.width ?? tooltipMaxWidthForViewport(window.innerWidth),
      },
      {
        height: window.innerHeight,
        width: window.innerWidth,
      },
    );
    setPosition((current) =>
      tooltipPositionsEqual(current, nextPosition) ? current : nextPosition,
    );
  };

  const initialMaxWidth =
    typeof window === "undefined"
      ? TOOLTIP_MAX_WIDTH
      : tooltipMaxWidthForViewport(window.innerWidth);

  useLayoutEffect(() => {
    if (!open) {
      return;
    }

    updatePosition();
    const frame = window.requestAnimationFrame(() => {
      updatePosition();
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [open, content]);

  useEffect(() => {
    if (!open || panelRef.current === null) {
      return;
    }

    const panel = panelRef.current;
    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      updatePosition();
    });
    observer.observe(panel);

    return () => {
      observer.disconnect();
    };
  }, [open, content]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handleViewportChange = (): void => {
      updatePosition();
    };
    window.addEventListener("resize", handleViewportChange);
    window.addEventListener("scroll", handleViewportChange, true);

    return () => {
      window.removeEventListener("resize", handleViewportChange);
      window.removeEventListener("scroll", handleViewportChange, true);
    };
  }, [open, hasTooltip, content]);

  useEffect(() => {
    return () => {
      if (closeTimerRef.current !== null) {
        window.clearTimeout(closeTimerRef.current);
      }
    };
  }, []);

  const cancelClose = (): void => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  };

  const openTooltip = (): void => {
    if (hasTooltip) {
      cancelClose();
      setOpen(true);
    }
  };

  const scheduleCloseTooltip = (): void => {
    cancelClose();
    closeTimerRef.current = window.setTimeout(() => {
      setOpen(false);
      closeTimerRef.current = null;
    }, 80);
  };

  return (
    <div
      ref={anchorRef}
      className={[
        "markdown-tooltip",
        hasTooltip ? "markdown-tooltip-enabled" : "",
        className ?? "",
      ]
        .filter((item) => item.length > 0)
        .join(" ")}
      tabIndex={hasTooltip ? 0 : undefined}
      onBlur={scheduleCloseTooltip}
      onFocus={openTooltip}
      onMouseEnter={openTooltip}
      onMouseLeave={scheduleCloseTooltip}
    >
      <span className="markdown-tooltip-label">{children}</span>
      {hasTooltip &&
        open &&
        portalTarget !== null &&
        createPortal(
          <div
            ref={panelRef}
            className={`markdown-tooltip-panel markdown-tooltip-panel-${position?.placement ?? "bottom"}`}
            role="tooltip"
            style={{
              left: position?.left ?? TOOLTIP_EDGE_GAP,
              maxWidth: position?.maxWidth ?? initialMaxWidth,
              top: position?.top ?? TOOLTIP_EDGE_GAP,
            }}
            onClick={stopTooltipPortalEvent}
            onMouseEnter={cancelClose}
            onMouseLeave={scheduleCloseTooltip}
            onMouseDown={stopTooltipPortalEvent}
            onPointerDown={stopTooltipPortalEvent}
          >
            {renderTooltipMarkdown(content)}
          </div>,
          portalTarget,
        )}
    </div>
  );
};

type TooltipMarkdownBlock =
  | {
      readonly kind: "blockquote";
      readonly lines: readonly string[];
    }
  | {
      readonly kind: "code";
      readonly text: string;
    }
  | {
      readonly kind: "heading";
      readonly level: number;
      readonly text: string;
    }
  | {
      readonly kind: "ordered-list" | "unordered-list";
      readonly items: readonly string[];
    }
  | {
      readonly kind: "paragraph";
      readonly lines: readonly string[];
    };

export const parseTooltipMarkdown = (
  markdown: string,
): readonly TooltipMarkdownBlock[] => {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const blocks: TooltipMarkdownBlock[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    if (line.trim().length === 0) {
      index += 1;
      continue;
    }

    const fenceMatch = line.match(/^\s*```/);
    if (fenceMatch !== null) {
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length && !/^\s*```/.test(lines[index])) {
        codeLines.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) {
        index += 1;
      }
      blocks.push({ kind: "code", text: codeLines.join("\n") });
      continue;
    }

    const headingMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch !== null) {
      blocks.push({
        kind: "heading",
        level: headingMatch[1].length,
        text: headingMatch[2].trim(),
      });
      index += 1;
      continue;
    }

    if (/^\s*>\s?/.test(line)) {
      const quoteLines: string[] = [];
      while (index < lines.length && /^\s*>\s?/.test(lines[index])) {
        quoteLines.push(lines[index].replace(/^\s*>\s?/, ""));
        index += 1;
      }
      blocks.push({ kind: "blockquote", lines: quoteLines });
      continue;
    }

    if (/^\s*[-*+]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^\s*[-*+]\s+/.test(lines[index])) {
        items.push(lines[index].replace(/^\s*[-*+]\s+/, ""));
        index += 1;
      }
      blocks.push({ kind: "unordered-list", items });
      continue;
    }

    if (/^\s*\d+[.)]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^\s*\d+[.)]\s+/.test(lines[index])) {
        items.push(lines[index].replace(/^\s*\d+[.)]\s+/, ""));
        index += 1;
      }
      blocks.push({ kind: "ordered-list", items });
      continue;
    }

    const paragraphLines: string[] = [];
    while (
      index < lines.length &&
      lines[index].trim().length > 0 &&
      !/^\s*```/.test(lines[index]) &&
      !/^(#{1,6})\s+/.test(lines[index]) &&
      !/^\s*>\s?/.test(lines[index]) &&
      !/^\s*[-*+]\s+/.test(lines[index]) &&
      !/^\s*\d+[.)]\s+/.test(lines[index])
    ) {
      paragraphLines.push(lines[index]);
      index += 1;
    }
    blocks.push({ kind: "paragraph", lines: paragraphLines });
  }

  return blocks;
};

const renderTooltipMarkdown = (markdown: string): ReactNode => {
  const blocks = parseTooltipMarkdown(markdown);
  if (blocks.length === 0) {
    return null;
  }

  return blocks.map((block, index) => {
    const key = `block-${index}`;
    switch (block.kind) {
      case "blockquote":
        return (
          <blockquote key={key}>
            {renderInlineLines(block.lines, `${key}-quote`)}
          </blockquote>
        );
      case "code":
        return (
          <pre key={key}>
            <code>{block.text}</code>
          </pre>
        );
      case "heading": {
        const HeadingTag = `h${Math.min(block.level, 6)}` as
          | "h1"
          | "h2"
          | "h3"
          | "h4"
          | "h5"
          | "h6";
        return (
          <HeadingTag key={key}>
            {renderInlineMarkdown(block.text, `${key}-heading`)}
          </HeadingTag>
        );
      }
      case "ordered-list":
        return (
          <ol key={key}>
            {block.items.map((item, itemIndex) => (
              <li key={`${key}-${itemIndex}`}>
                {renderInlineMarkdown(item, `${key}-${itemIndex}`)}
              </li>
            ))}
          </ol>
        );
      case "paragraph":
        return (
          <p key={key}>{renderInlineLines(block.lines, `${key}-paragraph`)}</p>
        );
      case "unordered-list":
        return (
          <ul key={key}>
            {block.items.map((item, itemIndex) => (
              <li key={`${key}-${itemIndex}`}>
                {renderInlineMarkdown(item, `${key}-${itemIndex}`)}
              </li>
            ))}
          </ul>
        );
    }
  });
};

const renderInlineLines = (
  lines: readonly string[],
  keyPrefix: string,
): ReactNode => {
  return lines.flatMap((line, index) => [
    ...(index === 0 ? [] : [<br key={`${keyPrefix}-br-${index}`} />]),
    ...renderInlineMarkdown(line, `${keyPrefix}-line-${index}`),
  ]);
};

const renderInlineMarkdown = (text: string, keyPrefix: string): ReactNode[] => {
  const nodes: ReactNode[] = [];
  const tokenPattern =
    /(`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*]+)\*|_([^_]+)_)/g;
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenPattern.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(text.slice(cursor, match.index));
    }

    const key = `${keyPrefix}-${match.index}`;
    if (match[2] !== undefined) {
      nodes.push(<code key={key}>{match[2]}</code>);
    } else if (match[3] !== undefined && match[4] !== undefined) {
      const href = safeMarkdownHref(match[4]);
      nodes.push(
        href === null ? (
          <span key={key}>{match[3]}</span>
        ) : (
          <a href={href} key={key} rel="noreferrer" target="_blank">
            {match[3]}
          </a>
        ),
      );
    } else if (match[5] !== undefined || match[6] !== undefined) {
      nodes.push(<strong key={key}>{match[5] ?? match[6]}</strong>);
    } else {
      nodes.push(<em key={key}>{match[7] ?? match[8]}</em>);
    }

    cursor = match.index + match[0].length;
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }

  return nodes;
};

const safeMarkdownHref = (href: string): string | null => {
  if (/^(https?:|mailto:)/i.test(href) || href.startsWith("#")) {
    return href;
  }

  return null;
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
          <p className="detail-summary" title={detail.summary.full_text}>
            {detail.summary.full_text}
          </p>
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
        {canCreateFollowup && (
          <footer>
            <button
              type="button"
              onClick={() => {
                setFollowupComposerOpen(true);
              }}
            >
              Follow-up
            </button>
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
            {sourceTag(selectedSession)} / {selectedSession.thread_label}
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
          <dd>{detail.summary.full_text}</dd>
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
