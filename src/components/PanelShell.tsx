import type { ReactNode } from "react";

/// 浮动 panel 外壳属性。
export interface PanelShellProps {
  /// 顶部拖动区展示的标题。
  readonly title: string;
  /// 标题右侧的状态摘要。
  readonly titleMeta?: ReactNode;
  /// 顶部拖动区右侧操作。
  readonly actions?: ReactNode;
  /// panel 主体内容。
  readonly children: ReactNode;
}

/// Builder Panel 的基础窗口外壳。
export const PanelShell = ({
  title,
  titleMeta,
  actions,
  children,
}: PanelShellProps) => {
  return (
    <section className="panel-shell" aria-label={title}>
      <header className="panel-drag-region" data-tauri-drag-region>
        <div className="panel-title-block" data-tauri-drag-region>
          <span
            className="panel-status-dot"
            aria-hidden="true"
            data-tauri-drag-region
          />
          <h1 data-tauri-drag-region>{title}</h1>
          {titleMeta}
        </div>
        {actions}
      </header>
      <div className="panel-content">
        <div className="panel-natural-content">{children}</div>
      </div>
    </section>
  );
};
