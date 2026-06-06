/// 空骨架中展示的会话占位数据。
export interface SessionPlaceholderProps {
  /// Agent 来源名称。
  readonly agentName: string;
  /// 当前状态文案。
  readonly statusText: string;
  /// 当前能力文案。
  readonly capabilityText: string;
}

/// 阶段 0 的会话占位行。
export const SessionPlaceholder = ({
  agentName,
  statusText,
  capabilityText,
}: SessionPlaceholderProps) => {
  return (
    <article className="session-row">
      <div>
        <strong>{agentName}</strong>
        <p>{capabilityText}</p>
      </div>
      <span>{statusText}</span>
    </article>
  );
};
