export type ShortcutScope =
  | "global"
  | "tabs"
  | "editor"
  | "grid"
  | "table"
  | "redis"
  | "menu";

export interface ShortcutEntry {
  scope: ShortcutScope;
  action: string;
  keys: string;
  context?: string;
}

export const SHORTCUT_SCOPE_ORDER: ShortcutScope[] = [
  "global",
  "tabs",
  "editor",
  "grid",
  "table",
  "redis",
  "menu",
];

export const SHORTCUT_SCOPE_LABELS: Record<ShortcutScope, string> = {
  global: "全局与文件",
  tabs: "标签与布局",
  editor: "SQL 编辑器",
  grid: "结果网格",
  table: "表数据",
  redis: "Redis",
  menu: "菜单",
};

export const SHORTCUTS: ShortcutEntry[] = [
  { scope: "global", action: "新建连接", keys: "Mod+N" },
  { scope: "global", action: "新建查询", keys: "Mod+T" },
  { scope: "global", action: "打开 SQL 文件", keys: "Mod+O" },
  { scope: "global", action: "保存 SQL", keys: "Mod+S" },
  { scope: "global", action: "另存为", keys: "Shift+Mod+S" },
  { scope: "global", action: "重新打开关闭的标签", keys: "Shift+Mod+T" },
  { scope: "global", action: "关闭标签", keys: "Mod+W" },
  { scope: "global", action: "关闭窗口", keys: "Shift+Mod+W" },
  { scope: "global", action: "刷新对象", keys: "Mod+R" },
  { scope: "global", action: "执行历史", keys: "Mod+Y" },
  { scope: "global", action: "撤销", keys: "Mod+Z" },
  { scope: "global", action: "重做", keys: "Shift+Mod+Z" },
  { scope: "global", action: "剪切", keys: "Mod+X" },
  { scope: "global", action: "复制", keys: "Mod+C" },
  { scope: "global", action: "粘贴", keys: "Mod+V" },
  { scope: "global", action: "全选", keys: "Mod+A" },
  { scope: "global", action: "最小化窗口", keys: "Mod+M" },
  {
    scope: "tabs",
    action: "反向循环切换标签",
    keys: "Shift+Ctrl+Tab",
    context: "在查询/表/ER 标签之间反向循环",
  },
  {
    scope: "tabs",
    action: "连接侧栏",
    keys: "Mod+B",
    context: "布局",
  },
  {
    scope: "tabs",
    action: "对象树",
    keys: "Shift+Mod+B",
    context: "布局",
  },
  {
    scope: "tabs",
    action: "结果面板",
    keys: "Mod+J",
    context: "布局",
  },
  {
    scope: "tabs",
    action: "右侧面板",
    keys: "Shift+Mod+H",
    context: "布局",
  },
  {
    scope: "tabs",
    action: "循环切换标签",
    keys: "Ctrl+Tab",
    context: "在查询/表/ER 标签之间循环",
  },
  {
    scope: "editor",
    action: "运行查询或选中 SQL",
    keys: "Mod+Enter",
    context: "编辑器聚焦时",
  },
  {
    scope: "editor",
    action: "查找和替换",
    keys: "Mod+F",
    context: "编辑器聚焦时",
  },
  {
    scope: "editor",
    action: "查找下一个",
    keys: "F3 / Mod+G",
    context: "查找面板打开后",
  },
  {
    scope: "editor",
    action: "查找上一个",
    keys: "Shift+F3 / Shift+Mod+G",
    context: "查找面板打开后",
  },
  {
    scope: "editor",
    action: "转到行",
    keys: "Mod+Alt+G",
    context: "编辑器聚焦时",
  },
  {
    scope: "editor",
    action: "注释 / 取消注释",
    keys: "Mod+/",
    context: "编辑器聚焦时",
  },
  {
    scope: "grid",
    action: "切换列排序",
    keys: "Enter / Space",
    context: "表头按钮聚焦时",
  },
  {
    scope: "grid",
    action: "关闭单元格菜单",
    keys: "Escape",
  },
  {
    scope: "table",
    action: "执行 WHERE 筛选",
    keys: "Enter",
    context: "筛选输入框聚焦时",
  },
  {
    scope: "table",
    action: "提交单元格编辑",
    keys: "Enter",
    context: "单元格编辑中",
  },
  {
    scope: "table",
    action: "取消单元格编辑",
    keys: "Escape",
    context: "单元格编辑中",
  },
  {
    scope: "table",
    action: "切换列排序",
    keys: "Enter / Space",
    context: "表头按钮聚焦时",
  },
  {
    scope: "redis",
    action: "扫描键",
    keys: "Enter",
    context: "键模式输入框聚焦时",
  },
  {
    scope: "redis",
    action: "执行 Redis 命令",
    keys: "Enter",
    context: "命令输入框聚焦时",
  },
  {
    scope: "redis",
    action: "关闭逻辑库下拉",
    keys: "Escape",
    context: "下拉打开时",
  },
  {
    scope: "menu",
    action: "进入菜单栏",
    keys: "Tab",
    context: "焦点在窗口内容时",
  },
  {
    scope: "menu",
    action: "切换顶层菜单",
    keys: "← / →",
    context: "菜单栏或菜单打开时",
  },
  {
    scope: "menu",
    action: "移动菜单项",
    keys: "↑ / ↓",
    context: "菜单打开时",
  },
  {
    scope: "menu",
    action: "执行菜单项",
    keys: "Enter / Space",
    context: "菜单打开时",
  },
  {
    scope: "menu",
    action: "关闭菜单并返回焦点",
    keys: "Escape",
  },
  {
    scope: "menu",
    action: "首字母跳转",
    keys: "字母键",
    context: "菜单打开时，500ms 内连续输入",
  },
];

export function formatShortcut(keys: string, isMac: boolean): string {
  if (isMac) {
    return keys
      .replace(/Mod\+/g, "⌘")
      .replace(/Shift\+/g, "⇧")
      .replace(/Alt\+/g, "⌥")
      .replace(/Ctrl\+/g, "⌃")
      .replace(/Enter/g, "↩")
      .replace(/\+/g, "");
  }
  return keys.replace(/Mod\+/g, "Ctrl+");
}

/** Returns human-readable duplicate warnings; empty when all unique. */
export function findDuplicateShortcuts(
  entries: ShortcutEntry[] = SHORTCUTS,
): string[] {
  const seen = new Map<string, string>();
  const warnings: string[] = [];
  for (const entry of entries) {
    const key = `${entry.scope}|${entry.keys}|${entry.context ?? ""}`;
    const existing = seen.get(key);
    if (existing) {
      warnings.push(
        `${SHORTCUT_SCOPE_LABELS[entry.scope]}：${existing} 与 ${entry.action} 都使用 ${entry.keys}`,
      );
    } else {
      seen.set(key, entry.action);
    }
  }
  return warnings;
}