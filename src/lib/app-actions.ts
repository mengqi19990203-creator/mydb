export type AppActionGroup =
  | "connection"
  | "object"
  | "query"
  | "tools"
  | "view";

export type AppActionIcon =
  | "plus"
  | "code"
  | "structure"
  | "refresh"
  | "diagram"
  | "sync"
  | "history"
  | "download"
  | "upload"
  | "close"
  | "play"
  | "more";

export interface AppAction {
  id: string;
  group: AppActionGroup;
  label: string;
  icon: AppActionIcon;
  keywords?: string[];
  shortcut?: string;
  /** Lower numbers stay in the toolbar longer. */
  priority: number;
  /** "more" actions always live in the overflow menu. */
  placement?: "primary" | "more";
  enabled: boolean;
  disabledReason?: string;
  active?: boolean;
  run: () => void | Promise<void>;
}

export const ACTION_GROUP_ORDER: AppActionGroup[] = [
  "connection",
  "object",
  "query",
  "tools",
  "view",
];

export const ACTION_GROUP_LABELS: Record<AppActionGroup, string> = {
  connection: "连接",
  object: "对象",
  query: "查询",
  tools: "工具",
  view: "视图",
};

export interface MenuBarItem {
  label?: string;
  shortcut?: string;
  disabled?: boolean;
  disabledReason?: string;
  separator?: boolean;
  danger?: boolean;
  action?: () => void | Promise<void>;
}

export interface MenuBarMenu {
  label: string;
  items: MenuBarItem[];
}