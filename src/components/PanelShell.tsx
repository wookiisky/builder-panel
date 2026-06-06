import type { ReactNode } from "react";

/// 浮动 panel 外壳属性。
export interface PanelShellProps {
  /// 顶部拖动区展示的标题。
  readonly title: string;
  /// panel 主体内容。
  readonly children: ReactNode;
}

/// Builder Panel 的基础窗口外壳。
export const PanelShell = ({
  title,
  children,
}: PanelShellProps) => {
  return (
    <section className="panel-shell" aria-label={title}>
      <header className="panel-drag-region" data-tauri-drag-region>
        <div className="panel-title-block">
          <span className="panel-status-dot" aria-hidden="true" />
          <h1>{title}</h1>
        </div>
      </header>
      <div className="panel-content">{children}</div>
    </section>
  );
};
