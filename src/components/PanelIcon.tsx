import {
  Bolt,
  Check,
  CircleEllipsis,
  LoaderCircle,
  Minus,
  OctagonX,
  SendHorizontal,
  X,
  type LucideIcon,
} from "lucide-react";

import type { SessionStatus } from "../api/mockPanelContract";

/// Panel 当前可复用的图标用途。
export type PanelIconName =
  | "window-minimize"
  | "window-settings"
  | "window-close"
  | "send"
  | "stop-placeholder";

/// Panel 图标基础属性。
export interface PanelIconProps {
  /// 图标用途。
  readonly name: PanelIconName;
  /// 额外样式类名。
  readonly className?: string;
}

/// Session 状态图标基础属性。
export interface PanelSessionStatusIconProps {
  /// Session 运行状态。
  readonly status: SessionStatus;
  /// 额外样式类名。
  readonly className?: string;
}

const panelIconByName: Record<PanelIconName, LucideIcon> = {
  "window-minimize": Minus,
  "window-settings": Bolt,
  "window-close": X,
  send: SendHorizontal,
  "stop-placeholder": OctagonX,
};

const sessionStatusIconByStatus: Record<SessionStatus, LucideIcon> = {
  running: LoaderCircle,
  waiting_for_approval: CircleEllipsis,
  waiting_for_answer: CircleEllipsis,
  completed: Check,
  failed: X,
  detached: Minus,
};

/// 渲染开源图标资源中的通用 panel 图标。
export const PanelIcon = ({ name, className }: PanelIconProps) => {
  const Icon = panelIconByName[name];

  return (
    <Icon
      absoluteStrokeWidth
      aria-hidden="true"
      className={className}
      focusable="false"
      size={14}
      strokeWidth={2}
    />
  );
};

/// 渲染开源图标资源中的 session 状态图标。
export const PanelSessionStatusIcon = ({
  status,
  className,
}: PanelSessionStatusIconProps) => {
  const Icon = sessionStatusIconByStatus[status];

  return (
    <Icon
      absoluteStrokeWidth
      aria-hidden="true"
      className={className}
      focusable="false"
      size={13}
      strokeWidth={2.4}
    />
  );
};
