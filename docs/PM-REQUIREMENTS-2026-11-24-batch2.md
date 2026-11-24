# mydb 第二批需求（PM-03 / PM-06 / PM-07 + PM-01b / PM-05 收口）

> 角色：产品经理（桌面数据库客户端）
> 基线：mydb v0.10.0（2026-11-23）
> 日期：2026-11-24
> 关联文档：`docs/PM-REQUIREMENTS-2026-11-24.md`（第一批）
> 范围：只写第二批可执行需求与第一批的剩余验收细节；本文不修改代码、不提交、不打包、不提升版本号。

---

## 0. 第二批范围与优先项

### 0.1 与第一批的关系

- 第一批文档给出的是主界面/菜单栏的方向、差距和 7 条需求（PM-01～PM-07）。
- 本文不重复第一批的背景、竞品结论和已通过条目；只补「可直接排期的实现细节、边界、验收、涉及文件、风险、依赖」。
- 第一批中 **PM-03（导航联动）、PM-06（编辑器菜单命令）、PM-07（快捷键速查）** 尚未实施；本文给出第二批实现规格。
- 第一批中 **PM-01（菜单双端一致性）与 PM-05（状态栏）** 在工作区已有实现（未提交）；本文只列剩余验收和需要收口的细节，不重复已经写过的功能描述。

### 0.2 当前工作区事实（供排期参考）

> 审计更新（2026-11-24 下午）：PM-03 / PM-06 / PM-07 的第一版也已进入工作区。本节保留“批次规划时”的快照；当前实现结论以 §10 审计为准。

截至本文撰写时（2026-11-24 中午），工作区中可见以下未提交改动；它们不是本文产生的：

- `src/components/AppMenuBar.vue` 已实现 roving tabindex、方向键/Home/End、Enter/Space、Esc 还焦点、首字母 typeahead、ARIA（对应第一批 PM-02）。
- `src/lib/app-actions.ts` 的 `MenuBarItem` 已有 `id`、`checked`；`src/App.vue` 的 `appMenus` 已给大部分菜单项加 id 和勾选状态。
- `src-tauri/src/lib.rs` 已使用 `CheckMenuItemBuilder` 构建四个面板开关，补齐打开/保存 SQL、重新打开标签、导入/导出、编辑/关闭连接、GitHub、全屏入口，并新增 `sync_menu_state` 命令；`App.vue` 已有 80ms 防抖的 `nativeMenuState` 同步。
- `src/lib/closed-tabs.ts` 与 `App.vue` 已接入关闭栈、`Shift+Mod+T`、查询/表/ER/Redis 快照恢复（对应第一批 PM-04）。
- `src/components/StatusBar.vue` 已接收活动标签、连接状态、只读、事务、执行中、结果摘要等 props；`styles.css` 已有状态栏基础样式（对应第一批 PM-05，但响应式优先级仍需收口）。
- **PM-03、PM-06、PM-07 尚未实现**；当前没有侧栏/对象树定位、编辑器菜单命令、快捷键速查弹窗。
- 上述状态可能随并行开发变化；实施前以工作区代码和 `CHANGELOG` 为准。

### 0.3 第二批优先级

| 编号 | 内容 | 优先级 | 依赖 | 建议顺序 |
|---|---|---|---|---|
| PM-06 | SQL 编辑器基础命令接入菜单 | P1 | 第一批菜单模型（已有） | 1（低成本、立即补齐菜单） |
| PM-03 | 导航联动：侧栏/对象树定位、折叠、工作集过滤 | P1 | 第一批布局与标签状态（已有） | 2（主界面价值最高，成本中） |
| PM-07 | 快捷键速查与冲突检查 | P2 | PM-06（编辑器条目）、PM-01b（Help 菜单） | 3（最后做，避免列表反复改） |
| PM-01b-AC | 原生菜单同源/状态同步剩余验收 | P0（收口） | 第一批 PM-01 实现 | 与 PM-06 并行 |
| PM-05-AC | 状态栏上下文与响应式剩余验收 | P0（收口） | 第一批 PM-05 实现 | 与 PM-03 并行 |

> 结论：**第二批 3 个功能优先项是 PM-06、PM-03、PM-07**。按实施顺序建议 PM-06 → PM-03 → PM-07；PM-01b-AC 和 PM-05-AC 作为第一批的发布前收口，不占第二批功能排期。

---

## 1. 共同约束

- 只改前端 Vue/CSS 的条目（PM-03、PM-07）不新增后端命令；PM-06 需要 `package.json` 显式补两个 CodeMirror 包和 Rust 原生菜单项，但不新增驱动。
- 不引入新原生依赖、不引入第三方 UI/图标库；新增图标按 `Icon.vue` 现有线性风格绘制。
- 不改变只读连接、事务、凭据存储的现有语义；不把密码、URL 凭据、SQL 结果写入新的持久化键。
- 新增 localStorage 键必须带版本号、逐项校验、损坏时回退默认；键名在实现时集中定义，避免散落。
- 所有新按钮必须有 `title` 与 `aria-label`；禁用时必须说明原因。
- 验收至少覆盖：SQLite / MySQL / PostgreSQL / Redis、无连接、只读、Redis 连接、零标签、960×600 最小窗口。

---

## 2. PM-06 SQL 编辑器基础命令接入菜单

### 2.1 本批增量（相对第一批）

第一批已经列出菜单项和大致验收；本批补：

- 精确的 CodeMirror 命令桥接方式；
- 菜单/原生菜单的加速键策略（避免抢走 WebView/弹窗按键）；
- 文件改动、类型、依赖版本；
- 边界与测试矩阵。

### 2.2 用户可见行为

在「编辑」菜单中，紧跟现有撤销/重做/剪切/复制/粘贴/全选之后增加：

1. 查找和替换…（`Mod+F`）
2. 查找下一个（`F3` / `Mod+G`）
3. 查找上一个（`Shift+F3` / `Shift+Mod+G`）
4. 转到行…（`Mod+Alt+G`）
5. 注释/取消注释（`Mod+/`）

行为要求：

- 只有活动标签是查询标签（`viewMode === "sql"` 且有 `activeTab`）时可用；表 / ER / 零标签 / Redis 时全部置灰，原因统一为「请先打开查询标签」。
- 点击菜单项先让活动 SQL 编辑器获得焦点，再执行 CodeMirror 命令；不改变 SQL（注释除外）、不执行查询、不写执行历史。
- 快捷键本身继续由 CodeMirror 的 `basicSetup`（`searchKeymap`、`defaultKeymap`）处理；菜单只是同一命令的第二个入口，不新增全局 keydown。
- 查找和替换共用 CodeMirror 自带的 search panel；不实现第二套查找 UI。
- 注释使用 SQL 语言的 `commentTokens`（已验证 `@codemirror/lang-sql` 对 MySQL / PostgreSQL / SQLite 提供 `line: "--"`、`block: "/* */"`）；对选中行或当前行生效。
- 转到行使用 CodeMirror `gotoLine`：合法行号跳转并滚动到居中；非数字/空输入关闭对话框；0、负数、超过总行数会 clamp 到 1..总行数，不崩溃。

### 2.3 原生菜单策略（重要）

第一批 PM-01b 要求双端一致，但编辑器命令有一个特殊约束：

- **不要在 macOS 原生「编辑」菜单给查找/注释/转到行注册全局 accelerator。** 原生 accelerator 会先于 WebView 接收按键，可能抢走模态框、结果网格或其他输入控件的 `Mod+F`/`Mod+/`，也会在无查询标签时误触发。
- 原生「编辑」菜单仍增加同名项，点击时通过 `menu:editor-*` 事件调用同一命令；但菜单项不设置 `.accelerator(...)`。窗口内菜单照常显示快捷键提示，实际按键由 CodeMirror 在编辑器聚焦时处理。
- 在 PM-01b 的验收中把这一条列为**允许的显式例外**：原生菜单项可无加速键，但必须可点击、可用状态正确，且不影响编辑器快捷键。
- 若未来希望原生菜单显示快捷键，应改为「应用级命令面板/自定义快捷键」方案，而不是直接给系统菜单加 accelerator。

### 2.4 实现方式

1. `src/components/SqlEditor.vue`
   - 新增：
     ```ts
     import { findNext, findPrevious, gotoLine, openSearchPanel } from "@codemirror/search";
     import { toggleComment } from "@codemirror/commands";
     ```
   - 定义 `EditorCommand = "find" | "findNext" | "findPrevious" | "gotoLine" | "toggleComment"`。
   - `defineExpose({ runEditorCommand })`：
     ```ts
     function runEditorCommand(command: EditorCommand): boolean {
       if (!view) return false;
       view.focus();
       const commands = {
         find: openSearchPanel,
         findNext,
         findPrevious,
         gotoLine,
         toggleComment,
       };
       return commands[command](view);
     }
     ```
   - 类型注意：`toggleComment` 是 `StateCommand`（接收 `{ state, dispatch }`），`EditorView` 满足该形状；若 TypeScript 报联合签名不兼容，用 `Command | StateCommand` 的显式分支调用，不要用 `any`。
2. `src/components/QueryWorkspace.vue`
   - 给 `SqlEditor` 增加 `ref="sqlEditorRef"`；`defineExpose({ requestExport, runEditorCommand })` 转发。
3. `src/App.vue`
   - 新增 `runEditorCommand(command)`：无活动查询标签时设置 `status` 为「请先打开查询标签」；否则调用 `queryWorkspaceRef.value?.runEditorCommand(command)`；返回 `false` 时设置「当前编辑器不支持该命令」。
   - `appMenus` 的「编辑」菜单新增 5 项，带稳定 id（如 `editor-find`、`editor-find-next`、`editor-find-previous`、`editor-goto-line`、`editor-toggle-comment`）和 `shortcut` 展示；`disabled`/`disabledReason` 按 `viewMode/activeTab` 计算。
   - 现有编辑菜单项补稳定 id（`edit-undo`、`edit-redo`、`edit-cut`、`edit-copy`、`edit-paste`、`edit-select-all`），供 PM-07 生成速查表和 PM-01b 做 id 对照；原生预定义项仍由系统处理。
4. `src-tauri/src/lib.rs`
   - 在原生「编辑」菜单预定义项之后加 5 个 `MenuItemBuilder`：
     `menu-editor-find`、`menu-editor-find-next`、`menu-editor-find-previous`、`menu-editor-goto-line`、`menu-editor-toggle-comment`；**不设置 accelerator**。
   - `on_menu_event` 映射为 `menu:editor-*`；`App.vue` 的 `bindMenuEvents` 增加对应 handler，调用 `runEditorCommand`。
   - `nativeMenuState` 会把这些 id 纳入 enabled 同步；Rust 端已有 `sync_menu_state`，无需新命令。
5. `package.json`
   - 显式加入当前已存在的传递依赖（版本与 lock 一致）：
     - `"@codemirror/commands": "^6.11.1"`
     - `"@codemirror/search": "^6.7.2"`
   - 不新增其他依赖。
6. `src/styles.css`
   - 检查 CodeMirror `searchPanel`/`cm-goto-line` 在暗色主题下的对比度；如默认面板是浅色，补 `.cm-panels`、`.cm-search`、`.cm-goto-line` 的 token 覆盖。只改样式，不改面板结构。

### 2.5 验收标准

- [ ] 打开查询标签后，编辑菜单出现 5 个新项；切到表 / ER / 零标签 / Redis 时全部置灰，悬停/键盘可读到「请先打开查询标签」；新建查询后恢复可用。
- [ ] 点击「查找和替换…」与在编辑器中按 `Mod+F` 打开同一个 CodeMirror 查找面板；不会出现两个面板；Esc 关闭后焦点回到编辑器，继续输入不丢字符。
- [ ] 选中多行（或光标在某行）执行「注释/取消注释」，MySQL / PostgreSQL / SQLite 均生成 `-- ` 行注释，再执行一次恢复；不执行查询、不改变结果、不写历史。
- [ ] 「转到行…」输入合法行号后光标跳到该行并居中；输入 0、负数、超过总行数时按 1 或末行处理且不报错；输入非数字关闭对话框；不触发 SQL 执行。
- [ ] macOS 原生编辑菜单有同名 5 项且可点击；没有注册全局加速键；在连接对话框/确认框/同步对话框打开时点击原生项不会在后台执行编辑器命令（若当前实现尚无模态拦截，按 PM-01b-AC-05 一并补）。

### 2.6 涉及文件

- `src/components/SqlEditor.vue`
- `src/components/QueryWorkspace.vue`
- `src/App.vue`
- `src-tauri/src/lib.rs`
- `package.json`
- `src/styles.css`（仅查找/转到行面板暗色适配）

### 2.7 风险与备注

- **原生 accelerator 冲突**：本批明确不加；不要为了“看起来一致”给 `menu-editor-find` 加 `Mod+F`，否则会破坏弹窗/网格/WebView 的按键行为。
- **类型签名**：`toggleComment` 与 `Command` 的签名不同，需在桥接层显式分支，避免 `vue-tsc` 报错或用 `any` 掩盖。
- **IME**：菜单动作在输入法组合期间不应被触发；菜单点击本身不受影响，快捷键由 CodeMirror 处理，需验证中文输入时不误触。
- **search panel 样式**：CodeMirror 默认面板可能和暗色 UI 不一致；只补 CSS，不改 DOM。
- **不做的范围**：格式化 SQL、跳转声明、悬停信息、重构、代码折叠命令；这些会引入格式化算法/方言语义，另行立项。
- **依赖边界**：`@codemirror/search` 和 `@codemirror/commands` 已是 `basicSetup` 的传递依赖；显式声明只是防止未来依赖树变化导致构建失败，不增加实际包体积。

---

## 3. PM-03 导航联动：侧栏 / 对象树定位、折叠、工作集过滤

### 3.1 本批增量（相对第一批）

第一批已定义「全部 / 有打开标签」过滤、定位当前标签、全部折叠和 5 条验收；本批补：

- 精确的状态模型、DOM 钩子、滚动与焦点策略；
- 连接范围不匹配、搜索/过滤隐藏、对象被删除等边界；
- 现有 `ObjectBrowser` / `ObjectCategoryGroups` 的接入方式；
- localStorage 键与恢复规则。

### 3.2 数据模型

在 `App.vue` 内新增/派生：

- `connectionFilter: Ref<"all" | "tabs">`，默认 `"all"`；持久化到 `mydb.sidebarFilter.v1`（`{version:1, filter:"all"|"tabs"}`）。
- `collapsedGroups` 复用现有 ref；持久化到 `mydb.sidebarGroups.v1`（`{version:1, collapsed: Record<string, boolean>}`）；加载时只保留字符串键/布尔值，未知连接组可保留但不影响显示。
- `openTabConnectionIds: ComputedRef<Set<string>>`：
  - `tabs` 中 `connectionId` 非空的查询标签；
  - `tableTabs` 的 `connectionId`；
  - `erTabs` 的 `connectionId`；
  - Redis：当 `isRedisConnection && redisWorkspaceOpen` 时加入 `selectedId`。
- `activeSourceConnectionId: ComputedRef<string | null>`：
  - 查询标签 → `activeTab.connectionId`；
  - 表标签 → `activeTableTab.connectionId`；
  - ER 标签 → `activeErTab.connectionId`；
  - Redis 控制台打开 → `selectedId`；
  - 零标签 → `null`（不拿“当前选中连接”冒充活动标签来源）。
- `filteredGroups` 改为先按搜索、再按 `connectionFilter` 过滤；搜索与过滤是 AND；分组计数使用过滤后的连接数；零匹配组隐藏。
- `activeObjectLocateState`：
  - 表/视图：`{kind: "table", name, kindLabel}`；
  - ER：`{kind: "scope"}`；
  - 查询/零标签：`null`。
  - 只有 `activeTableTab` 与当前 `selectedId/activeDatabase/activeSchema` 满足现有 `sourceMatches()` 时才允许表定位；否则状态提示，不切换范围。

### 3.3 连接侧栏行为

1. 在搜索框下方增加一行：
   - 左侧分段过滤「全部 / 有标签」（`aria-pressed`，按钮高度 24–26px）；
   - 右侧两个图标按钮：「定位当前标签」「全部折叠」。
   - 若空间不足，保留过滤按钮，定位/折叠可用图标-only + `title`。
2. 每个连接项增加 `:data-connection-id="connection.id"`；分组头增加 `:data-group-name="group.name"`，避免用连接名/组名做 CSS 选择器。
3. 定位当前标签：
   - 若无 `activeSourceConnectionId`：按钮禁用，原因「当前没有可定位的标签」。
   - 若活动连接被当前搜索或「有标签」过滤隐藏：先清空 `connectionSearch`、把 `connectionFilter` 设为 `all`，并在 `status` 提示「已清除筛选以定位当前标签」；不改变选中连接。
   - 展开所在分组（`collapsedGroups[groupName] = false`），`nextTick` + `requestAnimationFrame` 后找到 `[data-connection-id]`，`scrollIntoView({ block: "nearest" })`，加临时 `located` class（约 1.2s 后移除），并 `focus({ preventScroll: true })`。
   - 不调用 `selectConnection`/`refreshTables`，不建立新连接。
4. 全部折叠：
   - 把所有已知分组名写入 `collapsedGroups` 为 `true`（包括空组和未分组）；持久化。
   - 不改变当前选中连接、不关闭标签、不触发查询。
5. Redis：
   - Redis 连接在侧栏同样参与过滤和定位；控制台关闭时（`redisWorkspaceOpen=false`）不计入「有标签」，除非有其他标签引用该连接。
   - 点击 Redis 连接仍按现有逻辑打开控制台；定位只滚动/高亮，不额外打开。

### 3.4 对象树行为

1. 对象树标题栏在 `+` / 刷新之外增加「定位当前标签」和「全部折叠」两个图标按钮；Redis 下对象树不渲染，无需显示。
2. `ObjectBrowser.vue` 增加 `ref` 与 `defineExpose`：
   - `locateTable(kind: string, name: string): Promise<boolean>`
   - `locateScope(): Promise<boolean>`（用于 ER 标签定位到数据库/模式节点）
   - `collapseAll(): void`
3. `locateTable` 步骤：
   - 清空对象搜索 `search = ""`（若搜索隐藏了目标，状态提示「已清除对象搜索以定位」）；
   - `expandedDatabase = activeDatabase`（MySQL/PostgreSQL 当前库）；
   - PostgreSQL：`delete collapsedSchemas[activeSchema]`，确保当前模式展开；
   - `nextTick` 后调用 `ObjectCategoryGroups.revealTable(kind, name)`。
4. `ObjectCategoryGroups.vue`：
   - 暴露 `revealTable(kind, name): Promise<boolean>`：`collapsed[kind] = false`；`nextTick` 后查找 `[data-object-kind][data-object-name]`（或 `data-object-key`），`scrollIntoView({ block: "nearest" })`，加临时 `located` class，`focus({ preventScroll: true })`，返回是否找到。
   - 暴露 `collapseAllCategories()`：所有 `collapsed[kind] = true`，清空 `expandedTables`（收起字段）；保留 `search` 不变。
   - 每个表行增加 `:data-object-kind="table.kind"`、`:data-object-name="table.name"`；分类头增加 `:data-category-kind="group.kind"`。
5. `ObjectBrowser.collapseAll()`：
   - 调用当前 `ObjectCategoryGroups` 的 `collapseAllCategories()`；
   - 保留当前数据库/模式节点展开（让分类标题可见），收起非当前数据库节点；PG 下保留当前模式可见、其余模式收起；
   - 不改变 `activeDatabase/activeSchema`。
6. `locateScope()`（ER）：
   - 清空搜索、展开当前库/模式，滚动到数据库（MySQL）或模式（PG）行并高亮；不选中表。
   - `ObjectBrowser` 的数据库头/模式行增加 `:data-database-name` / `:data-schema-name`，用于安全定位；不要用名称拼 CSS 选择器。
7. 目标不存在：
   - `revealTable` 返回 `false` 时，App 设置 `status` 为「对象不存在，可能已被删除」；不打开标签、不切换数据库/模式。
8. 范围不匹配：
   - 若活动表标签的来源与当前对象树范围不一致，App 不调用 `locateTable`，只设置「当前对象树范围与活动标签不一致，请先切回该标签来源」；不自动 `switch_database`/`switch_schema`。
9. 数据库/模式自动展开：
   - 由于 `ObjectBrowser` 只在当前 `activeDatabase/activeSchema` 下渲染分类，定位只在范围一致时执行；不为定位重建对象树或触发 `list_tables`。

### 3.5 交互与样式

- 两个定位按钮使用现有 `Icon.vue` 新增 `target` 图标；折叠按钮使用 `collapse`/`chevron-up` 风格图标（不要引入图标库）。
- 临时高亮：`.located` 使用 `--accent-soft` 背景 + 1px `--accent-border`，1.2s 后移除；不要持久化。
- 过滤按钮：选中态用现有 accent token；未选中为 muted；禁用态 opacity 0.5 + 原因 title。
- 对象树/侧栏按钮均为 24–26px 图标按钮，保持现有 panel-header 密度；不增加新工具条行。
- 滚动只作用于最近的可滚动容器，避免带动整页；使用 `block: "nearest"`。

### 3.6 验收标准

- [ ] 打开查询/表/ER 标签后切到「有标签」，侧栏只显示有标签的连接（Redis 控制台打开时计入）；切回「全部」恢复；搜索与过滤同时生效，分组计数正确；重启后过滤状态保持。
- [ ] 当前标签来源连接所在分组被折叠，点「定位当前标签」：分组自动展开、连接行滚动到可见并短暂高亮；选中连接不变、不新建连接、不刷新对象。
- [ ] 当前标签连接被搜索/过滤隐藏时，定位会清除搜索与过滤并给出状态提示，然后完成定位；不会误选其他连接。
- [ ] MySQL 表标签点击对象树定位：展开当前库和对应分类，滚动到表并高亮；PostgreSQL 表标签还会展开当前模式；不改变 SQL、事务、查询结果或当前库/模式。
- [ ] 对象已被删除、活动标签为查询/零标签、活动表来源与对象树范围不一致时：按钮给出明确状态/禁用原因，不静默切换数据库/模式，不打开错误标签。
- [ ] 侧栏「全部折叠」收起所有分组；对象树「全部折叠」收起所有分类和字段，但保留当前库/模式节点可见；重启后侧栏折叠状态保持。
- [ ] 20 个连接、300 个对象下操作无明显卡顿；定位/折叠/过滤不产生任何后端/数据库请求；Redis 下对象树按钮不出现。

### 3.7 涉及文件

- `src/App.vue`（过滤/折叠/定位状态、localStorage、按钮模板、`objectBrowserRef`、状态提示）
- `src/components/ObjectBrowser.vue`（expose、data 属性、搜索清空、范围展开）
- `src/components/ObjectCategoryGroups.vue`（expose、`data-object-*`、`collapseAllCategories`）
- `src/components/Icon.vue`（`target`、`collapse` 图标）
- `src/styles.css`（过滤按钮、located 高亮、按钮密度）

### 3.8 实现顺序

1. 先做 App 侧派生数据（`openTabConnectionIds`、`activeSourceConnectionId`、过滤/折叠持久化），不改模板。
2. 侧栏过滤 + 定位 + 全部折叠，先验证不触发后端。
3. ObjectBrowser / ObjectCategoryGroups 暴露方法与 data 属性，做表定位和全折叠。
4. 补 ER 的 scope 定位、范围不匹配/对象不存在状态。
5. 最后补样式、长名称/960px/300 对象性能验收。

### 3.9 风险与备注

- **不要自动切换连接上下文**：定位不等于激活标签；范围不一致时只提示，避免未提交事务被换库/模式。
- **DOM 选择器安全**：使用 `data-connection-id` / `data-object-key`，不要用连接名/表名拼 CSS 选择器；表名可能含引号/空格。
- **搜索与过滤交互**：定位前清空隐藏条件会改变用户视图，必须有状态提示；不要静默清空。
- **滚动抖动**：展开父级后要等 `nextTick` + 一帧再滚动；同时移除上一次的 located class/timer。
- **焦点**：点击定位后把焦点放到目标行；若按钮是被键盘触发，焦点移动是预期行为；不要在 `pointerdown` 阶段抢焦点。
- **性能**：不引入虚拟列表；300 对象足够。若后续对象数上千，再单独评估。
- **与 B-26 的边界**：本项不实现连接星标/收藏；收藏连接仍归 B-26，避免重复。
- **与 B-27 的边界**：本项不做标签拖动/固定/溢出；只做“定位”。
- **Redis**：对象树不渲染；侧栏定位对 Redis 仍有效，但不要为了定位自动打开控制台。

---

## 4. PM-07 快捷键速查与冲突检查

### 4.1 本批增量（相对第一批）

第一批只提出「帮助菜单打开只读速查弹窗、分组、搜索、开发期重复警告」；本批补：

- 快捷键条目来源与数据结构；
- 菜单项 id 补齐；
- 重复检测的 scope 规则和允许例外；
- 弹窗 UI、焦点、验收和风险。

### 4.2 数据模型

新增 `src/lib/shortcuts.ts`（纯函数，不依赖 Vue）：

```ts
export type ShortcutScope =
  | "global"
  | "tabs"
  | "editor"
  | "navigator"
  | "grid"
  | "table"
  | "redis"
  | "dialog";

export interface ShortcutEntry {
  id: string;                 // 稳定 id
  label: string;              // 中文动作名
  keys: string;               // 当前平台展示用，如 "⌘⇧T" 或 "Ctrl+Shift+T"
  scope: ShortcutScope;
  condition?: string;         // 如 "活动标签为查询"
  commandId?: string;         // 同一命令的多个入口共用，重复检测时跳过
  keywords?: string[];
}
```

数据来源分两类：

1. **菜单命令**：从 `appMenus` 扁平化提取带 `shortcut` 且带 `id` 的菜单项；scope 按顶层菜单映射：**编辑 → `editor`，文件/视图/工具/窗口/帮助 → `global`**。同一动作在菜单中只出现一次；若未来出现两个入口，使用同一 `commandId`。
2. **非菜单快捷键**（静态常量，按当前平台生成 `keys`）：
   - `tabs`：`Ctrl+Tab` / `Shift+Ctrl+Tab` 循环标签（注意在 macOS 也是 Ctrl，不是 Cmd）。
   - `editor`：`Mod+Enter` 运行、`Mod+F` 查找和替换、`F3`/`Mod+G` 下一个、`Shift+F3`/`Shift+Mod+G` 上一个、`Mod+Alt+G` 转到行、`Mod+/` 注释。与菜单重复的条目使用同一 `commandId`，不重复展示或在同组内合并。
   - `navigator`：对象树 `↑/↓`、`Home/End`、`←/→`、`Enter`；连接列表 `Enter` 选中。
   - `grid`：结果/表头 `Enter`/`Space` 切换排序；表数据 `Enter` 提交编辑、`Esc` 取消编辑。
   - `redis`：键搜索/命令输入 `Enter` 执行；`Esc` 关闭逻辑库下拉（如当前实现）。
   - `dialog`：`Esc` 关闭当前对话框（各对话框现有行为）；不列尚未在所有对话框统一的 Tab 焦点陷阱。
   - 不列未实现的快捷键；不列系统级 `⌘Q`（mac app menu）和窗口管理快捷键，除非它们在菜单中可见。
   - 弹窗只展示当前平台；不需要同时列出 macOS 与 Windows/Linux 两套。

### 4.3 菜单项 id 补齐

为减少速查表与菜单脱节：

- `App.vue` 的编辑菜单项补 id：`edit-undo`、`edit-redo`、`edit-cut`、`edit-copy`、`edit-paste`、`edit-select-all`。
- PM-06 新增 `editor-find`、`editor-find-next`、`editor-find-previous`、`editor-goto-line`、`editor-toggle-comment`。
- 帮助菜单新增 `shortcuts-help`（键盘快捷键）。
- 原生菜单对应 `menu-shortcuts-help`（无 accelerator），事件 `menu:shortcuts-help`；PM-01b 状态同步把它纳入 enabled。
- `appMenus` 中 `about`/`github` 已有 id；若没有 shortcut 则不进入速查表。

### 4.4 重复检测规则

- 归一化：`⌘`/`Cmd`/`Command` → `mod`，`⇧`/`Shift` → `shift`，`⌃`/`Ctrl`/`Control` → `ctrl`，`⌥`/`Alt`/`Option` → `alt`；按键字母转小写；分隔符统一 `+`。
- 只在**同一平台、同一 scope**内比较；`tabs` 与 `global` 不跨 scope 比较。
- 若两个条目 `commandId` 相同（例如菜单「查找和替换」与编辑器 `Mod+F`），视为同一命令，不报重复。
- 明确的允许例外：
  - macOS 原生菜单项不注册编辑器加速键（见 PM-06）；速查表只展示一次。
  - `Ctrl+Tab` 在 macOS 仍是 Ctrl，不要误判为 `Mod+Tab`。
  - `F1` 不作为默认快捷键；若未来加，需先检查系统和 WebView 占用。
- 开发模式（`import.meta.env.DEV`）启动时运行 `findDuplicateShortcuts(entries)`；发现重复时 `console.warn` 并在弹窗顶部显示一条警告「发现 N 组重复快捷键」，列出条目。生产构建不阻断操作。

### 4.5 弹窗行为

- 入口：帮助 → 键盘快捷键；无默认全局快捷键（避免与系统/编辑器冲突）。
- 弹窗：宽 `min(760px, calc(100vw - 32px))`，最大高度 `min(78vh, 640px)`；遮罩使用现有 `.modal-backdrop`；内容可滚动。
- 顶部：搜索框（按动作名/快捷键/条件过滤）+ 关闭按钮；搜索为空时按 scope 分组显示。
- 分组顺序：全局、标签、编辑器、对象树、结果网格、表数据、Redis、对话框。
- 每行：动作名、快捷键（`kbd` 样式，按平台显示）、条件说明；无快捷键的菜单项不进入速查表。
- 底部：说明「快捷键暂不支持自定义，设置能力见 B-28」；结果计数。
- 焦点：打开时聚焦搜索框；Esc 关闭并返回触发按钮/原焦点；Tab 不离开弹窗（或按现有 ConfirmDialog 的焦点陷阱实现）；背景点击关闭。
- 空状态：「没有匹配的快捷键」；搜索清空后恢复分组。
- 可访问性：`role="dialog" aria-modal="true"`；分组标题 `h3`；快捷键用 `<kbd>`；不使用仅颜色表达。

### 4.6 验收标准

- [ ] 帮助 → 键盘快捷键打开弹窗；默认显示 8 个分组，快捷键按平台显示（macOS `⌘/⇧/⌥`，Windows/Linux `Ctrl/Shift/Alt`）；搜索「注释」「查找」「运行」「标签」均能命中。
- [ ] 逐项抽查至少 8 个快捷键（新建查询、重开标签、切换侧栏、运行、查找、注释、循环标签、对象树导航），实际行为与速查表一致；表中没有未实现或已失效的快捷键。
- [ ] PM-06 的 5 个编辑器命令只出现一次（菜单项与 CodeMirror 命令合并），不会被列成重复条目；PM-07 打开时 PM-06 已落地。
- [ ] 在开发模式人为制造同一 scope 的重复绑定，控制台出现警告，弹窗顶部显示重复组；正常构建无警告、无阻断。
- [ ] 960×600 下弹窗可滚动、不横向溢出；Esc/背景点击关闭后焦点回到原触发按钮；弹窗打开时按 `Mod+F`、`Mod+Enter` 不触发编辑器命令。
- [ ] Redis / 无连接 / 零标签下打开速查表不报错；不触发任何 SQL 或后端请求。

### 4.7 涉及文件

- 新增 `src/components/ShortcutHelpDialog.vue`
- 新增 `src/lib/shortcuts.ts`
- `src/App.vue`（Help 菜单项、弹窗状态、`shortcutEntries` 聚合、平台判断）
- `src/styles.css`（弹窗、分组、kbd、窄屏）
- `src-tauri/src/lib.rs`（原生 Help 菜单 `menu-shortcuts-help` + 事件，无 accelerator）
- `src/components/Icon.vue`（可选 `keyboard` 图标）
- `package.json` 无需新增依赖

### 4.8 风险与备注

- **清单漂移**：CodeMirror 升级可能改变默认键位；速查表中的编辑器条目必须与 `@codemirror/search`、`@codemirror/commands` 的显式导入命令对应，不依赖“默认可能还在”的记忆。
- **误报重复**：菜单项「查找和替换」与 CodeMirror `Mod+F` 是同一命令，必须用 `commandId` 合并；否则会把正常入口报成冲突。
- **平台差异**：macOS 的 `Ctrl+Tab` 不是 `⌘Tab`；原生菜单加速键和窗口 keydown 兜底不要重复列。
- **不做的范围**：自定义快捷键、冲突自动修复、导入/导出快捷键方案；这些归 B-28 设置中心。
- **不做假条目**：没有 handler 的快捷键一律不列；宁可少列，也不要让用户按了没反应。
- **原生菜单**：`menu-shortcuts-help` 无 accelerator；如果给 `F1`，需先验证系统/WebView 占用，第一版不加。

---

## 5. PM-01b 剩余验收（第一批收口）

> 第一批 PM-01b 的功能已在工作区实现；本节只列“发布前必须通过”的剩余验收和失败时的最小修正，不重复菜单信息架构描述。

### 5.1 已知实现状态

- 原生菜单已使用 `CheckMenuItemBuilder` 构建连接侧栏、对象树、结果面板、右侧面板四个开关，初始 `checked(true)`。
- 已补打开/保存 SQL、重新打开关闭的标签、导入数据、导出结果、编辑/关闭连接、GitHub、全屏；全屏从窗口菜单移到视图菜单。
- `sync_menu_state` 接收 `enabled` / `checked` 映射；`App.vue` 以 80ms 防抖在状态变化时调用，并在挂载后主动同步一次。
- 非 macOS 无原生菜单；窗口内菜单有 Windows/Linux 的「退出 mydb」，macOS 走 app menu Quit。

### 5.2 剩余验收项

- [ ] **AC-01 ID 对照**：列出 `appMenus` 中所有带 `id` 的项与 `build_menu` 的 id，逐项对应。允许的系统项/平台项只有：`minimize-window`、`toggle-fullscreen`、`about`（均为系统预定义，无自定义 id）、`quit-app`（仅非 macOS），以及 PM-06 补 id 的 `edit-undo`/`edit-redo`/`edit-cut`/`edit-copy`/`edit-paste`/`edit-select-all`（原生侧使用预定义项，无自定义 id）。除此之外不能有未映射 id；`sync_menu_state` 不应收到未知 id（或 Rust 端记录一次 debug 日志）。
- [ ] **AC-02 初始勾选**：应用启动后 200ms 内，原生四个面板开关的勾选状态与 `layout` 一致；右侧面板默认关闭时显示未勾选，不残留 `checked(true)`。若存在首帧闪烁，改为在菜单创建时读取初始布局或首帧同步前不显示菜单。
- [ ] **AC-03 点击核对**：连续点击原生「连接侧栏」两次，侧栏折叠/展开两次，最终勾选与布局一致；点击后不触发两次 toggle（检查 `menu:*` 事件与窗口 keydown 不重复）。
- [ ] **AC-04 禁用矩阵**：无连接、Redis、只读、无结果、无关闭标签、查询运行中时，窗口内与原生菜单的启用状态一致；原生禁用项点击不执行、不弹错误。至少覆盖：ER 图、结构同步、数据同步、刷新、导入数据、导出结果、编辑/关闭连接、重新打开标签。
- [ ] **AC-05 模态拦截**：打开连接对话框、确认对话框、结构/数据同步对话框、导出对话框时，菜单动作不得在后台执行（不得新建查询、刷新对象、打开 ER、执行同步、关闭连接）。若当前实现只在窗口内菜单防住、原生菜单仍可点，增加 `hasOpenModal` 并纳入 `nativeMenuState.enabled`，或在统一 action 入口加 guard；guard 必须给出状态提示，不能静默。
- [ ] **AC-06 单次执行**：macOS 逐一按 `⌘N/⌘T/⌘O/⌘S/⌘⇧S/⌘⇧T/⌘W/⌘⇧W/⌘R/⌘Y/⌘B/⌘⇧B/⌘J/⌘⇧H`，每个动作只执行一次；不同时触发窗口 keydown 兜底和原生菜单事件。非 macOS 检查 `Ctrl` 对应项与「退出」。
- [ ] **AC-07 同步节流与失败**：快速切换标签/连接/布局 10 次，80ms 窗口内最多一次 `sync_menu_state`；在 SQL 编辑器连续输入不触发同步；`sync_menu_state` 失败/非 macOS 调用不产生未处理 Promise rejection，不影响窗口内菜单。
- [ ] **AC-08 平台语义**：macOS 窗口内无「退出 mydb」；app menu Quit 正常退出；Windows/Linux 的「退出 mydb」调用 `quit_app` 并在有未保存 SQL/事务时二次确认；关闭窗口与退出语义分开。
- [ ] **AC-09 快捷键提示**：窗口内菜单显示的快捷键与 `appMenus`/`layoutShortcuts` 一致；原生菜单允许不显示编辑器命令的快捷键（PM-06 例外），但已注册的 accelerator 必须真实可用。
- [ ] **AC-10 无静默失败**：任何已实现的菜单动作在不可用时必须禁用并（窗口内）显示原因；原生菜单至少置灰。点击后无反应的项必须修复或移除。

### 5.3 失败时的最小修正

- `AC-01`：在 `App.vue` 增加 `NATIVE_MENU_IDS` 白名单，`nativeMenuState` 只发送白名单 id；在 Rust 的 `sync_menu_state` 对未知 id 打 `debug!` 或忽略并计数。
- `AC-02`：Rust 初始 `checked` 从布局默认值改为与 `mydb.workspaceLayout.v1` 默认一致，或前端在 `onMounted` 前同步；不要依赖用户看到的第一帧。
- `AC-05`：新增 `hasOpenModal` computed（连接/确认/建表/导入/导出/同步/数据同步/关于等），在 `nativeMenuState` 中把自定义项 enabled 置 false；窗口内菜单同样使用该状态。
- `AC-06`：检查 `onWindowKeydown` 的 `!isMacPlatform` 条件；不要在 macOS 再处理原生 accelerator 对应的键。

### 5.4 涉及文件

- `src/App.vue`（`nativeMenuState`、`hasOpenModal`、菜单 id、action guard）
- `src-tauri/src/lib.rs`（id 对照/日志、CheckMenuItem、事件映射）
- `src/components/AppMenuBar.vue`（窗口内禁用/勾选渲染，已有）
- `src-tauri/src/commands/data.rs`（`quit_app`，已有）

### 5.5 风险

- `AC-05` 的模态拦截是剩余项里风险最高的：不能只靠 CSS 遮罩，必须阻止原生菜单事件执行；否则会在用户不知情时新建标签或启动同步。
- 原生 CheckMenuItem 点击会先由系统切换勾选，再由事件回调切布局；若两者时序不一致，最终以 `sync_menu_state` 为准，但需要避免闪烁。
- 原生菜单 id 与 TS id 容易漂移；建议在代码中用一张常量表/白名单，不接受“运行时忽略未知 id”作为长期方案。
- 非 macOS 没有原生菜单，AC 只验证窗口内菜单；不要为通过验收伪造原生菜单。

---

## 6. PM-05 剩余验收（第一批收口）

> 第一批 PM-05 的状态栏功能已在工作区实现；本节只列剩余验收和需要调整的响应式优先级，不重复状态栏字段定义。

### 6.1 已知实现状态

> 审计更新：当前 `styles.css` 已不隐藏事务徽标；§6.3 中“隐藏事务”的问题已修复。剩余断点问题见 §10.3 AUD-07。

- `StatusBar.vue` 已接收 `activeLabel`、`activeKind`、`connectionStatus`、`connectionStatusLabel`、`readOnly`、`inTransaction`、`running`、`error`。
- `App.vue` 已传入活动标签/来源、连接状态、只读、事务、运行中、错误；`result` 只在 `viewMode === "sql"` 时传入。
- `styles.css` 已加 `.status-active`、`.status-readonly`、`.status-connection`、`.status-transaction`、`.status-running`、`.status-result` 及 1279/1099/999px 媒体查询。
- 当前媒体查询会：<1280 隐藏结果摘要，<1100 隐藏事务徽标，<1000 隐藏连接状态。**隐藏事务徽标不符合安全优先级**，需要调整。

### 6.2 剩余验收项

- [ ] **AC-01 安全优先级**：状态栏永远不隐藏「事务中 · 未提交」「只读」「执行中」；窄窗口先隐藏结果摘要，再隐藏连接状态，最后压缩活动标签（省略号），不能反过来。960×600 下不换行、不横向滚动、不遮挡内容。
- [ ] **AC-02 断点行为**：≥1200px 显示全部；1000–1199px 隐藏结果摘要（或只保留行数、把耗时放 title）；<1000px 隐藏连接状态；活动标签最大宽度 380/260/200px，长名称省略且 title 可看全文。
- [ ] **AC-03 无残留结果**：在查询/表/ER/零标签/Redis 之间切换，状态栏不显示上一个标签的结果行数/耗时；关闭当前查询标签后，结果摘要立即消失；`result` 只来自活动查询标签（当前 `viewMode === "sql"` 逻辑保留）。
- [ ] **AC-04 事务/只读/失败**：活动查询标签事务中显示徽标，切到非事务标签后消失，提交/回滚后消失；只读连接显示「只读」；连接失败显示失败文案与状态点；Redis 显示 `dbNN`；零标签显示当前连接 + 库/模式或「未连接」。
- [ ] **AC-05 结果矩阵**：查询有行显示「N 行 · X ms」；截断追加「已截断」；零行查询显示「0 行 · X ms」；DML 显示「影响 N 行 · X ms」；查询失败显示「查询失败」而不是旧结果；运行中显示 spinner +「执行中…」，不显示旧结果。
- [ ] **AC-06 后台运行**：在 A 标签执行查询后切到 B 标签，右侧不显示「执行中…」；切回 A 显示运行中/结果；左侧全局 `status` 不得与右侧状态矛盾（切标签后不要保留 A 的“执行中”文案而 B 显示空闲）。
- [ ] **AC-07 长文本与无障碍**：超长连接名/库名/标签名省略并可通过 title 查看；左侧状态消息有 `role="status" aria-live="polite"`；右侧高频变化不使用 live region，避免读屏反复播报；只读/事务/运行中有文字而非仅颜色。
- [ ] **AC-08 安全与请求**：状态栏不显示密码、含密码 URL、SSH 凭据；所有状态更新不触发额外后端/数据库请求；连接地址与现有 `connectionTarget` 一致（不含密码）。
- [ ] **AC-09 Redis / 零标签**：Redis 控制台打开/关闭、切换逻辑库后状态栏 `dbNN` 正确；零标签时显示当前选中连接与库/模式，不显示旧查询结果；无连接时显示「未连接」。
- [ ] **AC-10 与菜单同步**：状态变化（运行中、事务、只读、活动标签）不破坏 PM-01b 的菜单同步；状态栏自身不触发 `sync_menu_state` 的额外风暴。

### 6.3 需要的具体调整

1. 修改 `styles.css` 状态栏断点：
   - `@media (max-width: 1279px)` 改为 `@media (max-width: 1199px)` 隐藏 `.status-result`；
   - 删除 `@media (max-width: 1099px)` 中隐藏 `.status-transaction` 的规则；
   - `<1000px` 只隐藏 `.status-connection`；
   - `.status-transaction`、`.status-readonly`、`.status-running` 设 `flex: 0 0 auto`，永不 `display:none`。
2. 若活动标签在窄屏仍过长，使用 `max-width` + `text-overflow`，不要隐藏整段。
3. `StatusBar.vue` 的 `resultSummary` 可增加短格式（只显示行数）供 <1200px 使用，但第一版可以直接隐藏整个结果摘要；不要用 `visibility:hidden` 占位。
4. 检查 `status` 全局消息在切换标签时是否残留后台标签的“执行中…”；若残留，在 `showQueryTab`/`selectTableTab`/`showErTab` 的入口清空或替换为中性消息。
5. 不给状态栏增加点击动作；本批只做展示，避免与 B-05/B-24 抢范围。

### 6.4 涉及文件

- `src/components/StatusBar.vue`（短格式/aria/title）
- `src/App.vue`（活动标签/状态映射、切标签时 status 清理）
- `src/styles.css`（断点优先级）

### 6.5 风险

- **隐藏事务是最危险的**：窄窗口用户可能看不到未提交事务；本批明确永不隐藏。
- **左侧/右侧状态冲突**：全局 `status` 和活动标签状态可能不同步；切标签时必须清理或使用中性文案。
- **STALE 结果**：不要为了“信息丰富”缓存上一个标签的结果；当前 `viewMode === "sql"` 的隔离要保留。
- **安全**：状态栏只显示 driver/host/port/db/schema，不显示密码或含密码 URL。
- **响应式**：`status-message` 和 `status-meta` 都在 flex 容器内，需在 960px 实测而不是只看 CSS。

---

## 7. 第二批建议顺序与依赖

| 顺序 | 条目 | 原因 | 依赖 |
|---|---|---|---|
| 1 | PM-06 | 成本最低，直接补齐“菜单里有编辑器命令”的缺口；依赖第一批菜单模型已就绪 | 第一批 PM-01a/02；原生菜单 PM-01b 已具备 |
| 2 | PM-03 | 主界面价值最高；不影响事务/连接上下文，但需要 DOM 滚动和树展开细节 | 第一批布局/标签状态；PM-01b 不阻塞 |
| 3 | PM-07 | 最后做，避免 PM-06 条目和 Help 菜单反复改；依赖 PM-06 与 PM-01b | PM-06、PM-01b |
| 并行 | PM-01b-AC | 第一批发布前收口，尤其是模态拦截和 id 对照 | 第一批 PM-01 实现 |
| 并行 | PM-05-AC | 第一批发布前收口，尤其是事务永不可隐藏和 960px | 第一批 PM-05 实现 |

**建议先做的 3 个功能优先项：PM-06 → PM-03 → PM-07。**
PM-01b-AC / PM-05-AC 不占功能排期，但必须在前一批宣称“完成”前通过。

---

## 8. 与第一批的边界（本批不做）

- 不重做第一批已实现的菜单键盘导航、原生菜单同步、重开标签、状态栏基础字段；只做本文列出的剩余验收/补丁。
- 不在 PM-03 中做连接星标/收藏（B-26）、标签拖动/固定/溢出（B-27）、全局命令面板（B-25）。
- 不在 PM-06 中做 SQL 格式化、跳转声明、悬停信息、重构、代码折叠命令。
- 不在 PM-07 中做快捷键自定义、导入/导出、冲突自动修复（B-28）。
- 不在 PM-05 中做查询取消（B-05）、多结果集（B-07）、右侧信息面板（B-24）、点击状态栏跳转。
- 不新增数据库驱动、不新增原生依赖、不改凭据/只读/事务语义、不复制第三方品牌资源。

---

## 9. 参考

- 第一批文档：`docs/PM-REQUIREMENTS-2026-11-24.md`（含 DBX / Navicat 18 / DBeaver 的 28 条来源、主界面与菜单栏调研）。
- 现有实现：
  - `src/components/AppMenuBar.vue`（键盘导航/ARIA/勾选渲染）
  - `src/lib/app-actions.ts`（`MenuBarItem.id/checked`）
  - `src/lib/closed-tabs.ts`（关闭栈，第一批 PM-04）
  - `src/App.vue`（`appMenus`、`nativeMenuState`、`sync_menu_state` 调用、状态栏 props）
  - `src-tauri/src/lib.rs`（`build_menu`、`sync_menu_state`、`CheckMenuItemBuilder`）
  - `src/components/StatusBar.vue`、`src/styles.css`
- CodeMirror 命令来源：`@codemirror/search` 的 `openSearchPanel` / `findNext` / `findPrevious` / `gotoLine`，`@codemirror/commands` 的 `toggleComment`；`@codemirror/lang-sql` 的 `commentTokens` 为 `--` 与 `/* */`。
- 本批不新增外部调研；竞品依据沿用第一批文档的来源清单。

---

## 10. 实现审计（2026-11-24 下午，只读）

### 10.1 审计范围与检查结果

- 对工作区当前未提交的第一版实现做**只读代码审计**；本审计不修改源码。
- 已执行：
  - `npm run check`：通过（`vue-tsc --noEmit` + `cargo check`）。
  - `cargo test --manifest-path src-tauri/Cargo.toml`：57 passed，0 failed。
- **未启动 GUI、未逐项点击 macOS 原生菜单**；涉及原生菜单外观、CheckMenuItem 点击时序、滚动/焦点的手感，属于「代码可判定 / 需实机复核」两类，下文分别标注。
- 行号会随并行开发变化，因此证据以文件 + 函数/组件名为准。

### 10.2 结果总览

| 需求 | 结论 | 主要问题 |
|---|---|---|
| PM-01a 菜单信息架构 | 基本通过 | 菜单职责/去重/退出语义已落地；`editItem` 仍无 id；`close-window` 等系统项未纳入守卫 |
| PM-01b 原生菜单同源/状态 | **部分通过，P0 缺口** | 模态拦截只覆盖 App 级状态和部分 action id；子组件弹窗未纳入；窗口内菜单动作无守卫；`close-window` 可绕过错觉/数据风险 |
| PM-02 菜单键盘导航 | 部分通过 | 方向键/Enter/Esc/typeahead/ARIA 已实现；Tab/执行后焦点未显式恢复 |
| PM-03 导航联动 | **部分通过，需修** | 对象定位 selector 未转义；ER scope 定位缺失；全部折叠会隐藏当前库/模式；过滤未持久化；失败/焦点反馈不足 |
| PM-04 重新打开标签 | 部分通过 | macOS 原生 accelerator 可用；Windows/Linux 未接 `Shift+Mod+T`；无效快照会被 pop 掉 |
| PM-05 状态栏 | 部分通过 | 事务/只读/执行中优先级已修正；结果摘要断点仍是 <1280，早于规格的 1200 |
| PM-06 编辑器菜单命令 | 基本通过 | 命令桥接、原生菜单项、无 accelerator 策略正确；编辑菜单项无 id，查找下一个/上一个只展示 F3 未展示 Mod+G |
| PM-07 快捷键速查 | **不通过/需修** | 静态清单未从菜单聚合；`Ctrl+Tab` 在 macOS 显示为 `CtrlTab`；缺 scope/条目；重复检测未覆盖菜单且无弹窗提示；无焦点陷阱/恢复 |

### 10.3 详细问题

#### AUD-01 [P0] 模态拦截只覆盖了一半入口，存在后台执行与数据丢失风险

**事实（代码可判定）**

- `App.vue` 的 `hasOpenModal` 只聚合 App 自己持有的状态（连接、建表、导入、结构/数据同步、关于、快捷键弹窗、复制连接、分组提示、App 级确认）。
- 以下子组件自己的模态状态**不在** `hasOpenModal`：
  - `QueryWorkspace` 的 `exportOpen` / `ResultExportDialog`；
  - `ObjectCategoryGroups` 的 `renameOpen`、`exportOpen`、`copyOpen`、`deleteOpen`、`maintenanceOpen`（对象重命名/导出/复制/删除/清空）；
  - `TableStructureView` 的字段/索引编辑、删除确认；
  - `TableDataView` 的删除确认；
  - `RedisWorkspace` 的风险命令确认/删除确认。
- `AppMenuBar.vue` 的 `runItem` 直接调用 `item.action()`，没有 `hasOpenModal` 判断；模态守卫只加在 `App.vue` 的 `bindMenuEvents` 原生菜单事件包装器和 `runSqlEditorCommand` 里。也就是说：**窗口内菜单动作不受守卫**。
- `modalBlockedActionIds` 不包含 `close-window`、`quit-app`（非 macOS）、`shortcut-help`、`about`、`toggle-sidebar`/`toggle-objects`/`toggle-result`/`toggle-right`、`reset-layout`、`toggle-fullscreen`、`minimize-window`。
- `closeWindow()` 直接 `getCurrentWindow().close()`，不检查未保存 SQL 文件/未提交事务；`quitApp()` 才有检查。
- `ConnectionDialog`、`ShortcutHelpDialog`、`ResultExportDialog` 等没有统一的 Tab 焦点陷阱；用户可以从非陷阱弹窗把焦点移出，再触发窗口菜单或原生菜单。

**影响**

- 原生 `⌘⇧W`（关闭窗口）/菜单项可以在连接对话框、同步对话框、导出对话框打开时关闭窗口；未保存 SQL 文件或事务可能丢失。
- 子组件弹窗打开时，原生菜单的刷新/ER/同步/新查询/导入/导出等仍可能可点并执行；例如结果导出对话框打开时点“刷新对象”会在后台重载对象树。
- `shortcut-help`/`about` 不在阻断面，可能在另一个模态上再叠一层模态。
- 窗口内菜单完全绕过守卫；只要焦点能从弹窗跑出，或用户用键盘触发菜单，就会执行后台动作。

**验收对照**

- 直接失败：PM-01b AC-05「模态拦截」；部分失败：PM-02/PM-07 的焦点可预测性。
- `hasOpenModal` 只修了“App 知道的那部分”，不能作为完成证据。

**建议修复（不改代码，仅给方案）**

1. 建一个统一的「模态注册/计数」机制（provide/inject 或一个小的 `useModalStack`），所有 `*.vue` 在 `open` 时注册、关闭时注销；App 只读一个 `hasOpenModal`。
2. 在 **两个菜单入口**（`AppMenuBar.runItem` 和原生 `listen` 包装器）统一执行 `guardMenuAction(actionId)`；不要只在一个入口加。
3. `close-window`/`quit-app` 至少统一到未保存文件/事务检查；`shortcut-help`/`about` 在已有模态打开时禁用或关闭当前模态。
4. 给 `ConnectionDialog`、`ShortcutHelpDialog`、`ResultExportDialog` 等补焦点陷阱（可以复用 `ConfirmDialog` 的 Tab 逻辑）。

---

#### AUD-02 [P0/P1] PM-03 对象树定位的 DOM 选择器未转义，特殊表名会报错/误定位

**事实（代码可判定）**

- `ObjectCategoryGroups.vue`：
  - `tableKey(table)` 返回 `` `${table.kind}:${table.name}` ``；
  - `locateTable` 使用 `` document.querySelector(`[data-object-key="${key}"]`) ``；
  - `key` 直接来自表名，没有 `CSS.escape`，也不是在当前组件根节点内查找。
- 表/视图名允许包含 `"`、`]`、`\`、空格、冒号等字符（MySQL 反引号标识符、PostgreSQL 引号标识符都可以）；例如表名 `a"b` 会生成 `[data-object-key="table:a"b"]`，属性选择器在引号处提前结束，可能抛 `SyntaxError` 或匹配到错误节点。
- `locateTable` 返回 `false` 时，`ObjectBrowser.locateActiveObject` 完全忽略返回值；活动表已被删除、或定位目标未渲染时，用户点按钮没有任何可见反馈。
- 定位成功后没有 `focus()` 目标行，只滚动；`locateTarget` 不会在成功后清空。
- `ObjectCategoryGroups` 的 `locateNonce` watcher 是 `{ immediate: true }`；一次定位后切换数据库/模式，组件重挂载时可能用旧的 `locateTarget` 在新范围里再定位一次；若新范围有同名对象，会高亮错误对象。

**影响**

- 合法但特殊的表名（引号/方括号/反斜杠）会让“定位”直接失败或报错。
- 目标不存在时静默失败，用户误以为功能坏了。
- 键盘用户定位后焦点仍在按钮，不在目标行，无法继续树导航。
- 切换范围后可能错误高亮。

**验收对照**

- 失败：PM-03「对象树定位」的“滚动到活动对象并高亮”在特殊名称/对象删除/重挂载场景下不成立；PM-03 的“对象不存在时提示”不成立。

**建议修复**

- 用 `CSS.escape(key)` 或 `root.value?.querySelectorAll("[data-object-key]")` 后按 `dataset.objectKey === key` 过滤；不要用全局 `document.querySelector` 拼值。
- `revealTable` 返回 boolean，`ObjectBrowser` 在 false 时回调 App 设置「对象不存在，可能已被删除」。
- 定位成功后 `element.focus({ preventScroll: true })`，并清空 `locateTarget`。
- 只在 `locateNonce` 真正变化（非初次挂载）时执行定位，或给 `locateNonce`/`locateTarget` 增加 source 校验。

---

#### AUD-03 [P1] PM-03 ER 定位、全部折叠与过滤持久化不符合规格

**事实（代码可判定）**

- `objectLocateReason` 对 ER 标签返回「ER 标签没有具体对象」；`ObjectBrowser.locateActiveObject` 只 `flashToast`，没有 `locateScope`/滚动到数据库或模式。第二批要求 ER 定位到 scope。
- `ObjectBrowser.collapseAllObjects` 设置 `expandedDatabase = null` 并把所有 schema 标为 collapsed；它会隐藏当前数据库/模式节点，而不是“保留当前库/模式可见，只折叠分类”。第二批要求保留当前范围可见。
- `sidebarOpenOnly` 只是 `ref(false)`，没有 localStorage 读写；第二批要求的 `mydb.sidebarFilter.v1` 未实现，重启后过滤状态丢失。
- 侧栏过滤按钮没有 `aria-pressed`；定位连接只滚动，不 `focus()` 目标行。
- 组折叠键实际是 `mydb.connectionGroupsCollapsed.v1`（无版本对象），与第二批建议的 `mydb.sidebarGroups.v1` 不一致；行为可用，文档/实现命名需统一。

**影响**

- ER 标签点击对象树定位只弹提示，不能定位到当前库/模式。
- “全部折叠”后当前库/模式也消失，用户无法继续从当前范围展开分类。
- 重启后「只看有标签」丢失。
- 键盘用户定位后焦点不连续；读屏无法知道过滤按钮状态。

**验收对照**

- 失败：PM-03 的 ER scope、全部折叠保留当前范围、过滤状态持久化、`aria-pressed`、定位焦点条目。

**建议修复**

- 增加 ER 的 `locateScope`，在数据库/模式节点加 `data-database-name`/`data-schema-name`，滚动并高亮。
- 全部折叠只折叠分类与字段，保留当前库/模式展开；非当前库/模式可收起。
- `sidebarOpenOnly` 写入版本化 localStorage，并在启动时校验恢复；按钮加 `aria-pressed`。
- 连接定位成功后 `focus({ preventScroll:true })` 目标行。

---

#### AUD-04 [P1] Windows/Linux 上大量菜单快捷键没有落地

**事实（代码可判定）**

- `onWindowKeydown` 在非 macOS 只处理：`Ctrl+O`、`Ctrl+S`/`Ctrl+Shift+S`、`Ctrl+B`/`Ctrl+Shift+B`、`Ctrl+J`、`Ctrl+Shift+H`，外加 `Ctrl+Tab`/`Shift+Ctrl+Tab`。
- `appMenus` / 菜单中显示但非 macOS 没有处理：`Mod+N`（新建连接）、`Mod+T`（新建查询）、`Mod+W`（关闭标签）、`Mod+Shift+W`（关闭窗口）、`Mod+R`（刷新）、`Mod+Y`（历史）、`Mod+M`（最小化）、`Mod+Shift+T`（重新打开关闭的标签）、`Mod+Q`（退出）。
- 原生菜单只在 `#[cfg(target_os = "macos")]` 构建；Windows/Linux 只有窗口内菜单，因此不存在原生 accelerator 兜底。
- 未处理的组合可能落到 WebView/系统默认行为（例如刷新类快捷键），有状态丢失风险；本审计未在 Windows/Linux 实机验证具体默认行为。

**影响**

- 菜单显示的快捷键在 Windows/Linux 上按了没反应；PM-04 的 `Shift+Mod+T` 在非 macOS 直接缺失。
- 跨平台验收无法通过；用户会认为快捷键“时灵时不灵”。

**验收对照**

- 失败：PM-04 的重新打开快捷键（非 macOS）、PM-01 的菜单元数据与实际行为一致性、PM-07 的速查表在非 macOS 的可用性。

**建议修复**

- 把非 macOS 的应用级快捷键集中到一个 `handleAppShortcut(key, modifiers)`，覆盖菜单中所有注册了 shortcut 的 app action；macOS 继续交给原生菜单，避免双触发。
- 对 `Ctrl+R` 等可能与 WebView 默认行为冲突的组合显式 `preventDefault`。
- 增加平台矩阵验收：macOS / Windows / Linux 各抽 8 个快捷键。

---

#### AUD-05 [P1] PM-07 快捷键速查内容不完整，且有可见格式化错误

**事实（代码可判定）**

- `shortcuts.ts` 的 `SHORTCUTS` 是**静态清单**，没有从 `appMenus`/`appActions` 聚合；菜单后续新增/改键时不会自动同步。
- `formatShortcut("Ctrl+Tab", true)` 返回 `"CtrlTab"`（已用等价 JS 逻辑验证）；原因是 macOS 分支把 `+` 全部删除，却没有先把 `Ctrl+` 转成 `⌃`。`Ctrl+Shift+Tab` 同理。
- 清单缺少第二批要求的：
  - `Shift+Ctrl+Tab`（反向循环）；
  - `Mod+G` / `Shift+Mod+G`（查找下一个/上一个的 CodeMirror 快捷键）；
  - 对象树方向键/Home/End/Enter、连接列表 Enter；
  - 对话框 Esc（没有 `dialog` scope）；
  - 撤销/重做/剪切/复制/粘贴/全选等菜单快捷键。
- `findDuplicateShortcuts` 只接收静态 `SHORTCUTS`，没有菜单条目；key 包含 `context`，无法按 `commandId` 合并“菜单入口 + CodeMirror 命令”；没有实际重复键告警测试。
- `App.vue` 只在 DEV 下 `console.warn`，弹窗没有重复提示条；第二批要求的“弹窗顶部显示重复组”未实现。
- `ShortcutHelpDialog` 没有焦点陷阱，也没有记录/恢复打开前焦点；关闭后焦点去向不确定。
- 清单里的「Tab 进入菜单栏」并不是任何代码保证的全局行为（取决于当前焦点位置），可能误导。

**影响**

- macOS 用户看到 `CtrlTab` 这种错误显示；Windows/Linux 用户看到大量未实际接通的快捷键（与 AUD-04 叠加）。
- 用户按速查表找不到对象树/对话框快捷键；重复检测无法阻止真正的冲突。
- 弹窗打开时 Tab 可离开模态，键盘焦点丢失。

**验收对照**

- 失败：PM-07 的“清单与实际一致、8 个 scope、重复检查、焦点恢复、弹窗提示”大部分条目。

**建议修复**

- 补齐菜单项 id，`buildShortcutEntries(appMenus)` 聚合；静态表只保留非菜单快捷键，并用 `commandId` 去重。
- `formatShortcut` 先处理 `Ctrl+` → `⌃`（macOS），再处理 `Mod/Shift/Alt` 和 `+`；补单测或至少手工验证 8 个组合。
- 增加 `navigator`/`dialog` scope、反向 `Ctrl+Tab`、`Mod+G` 系列、编辑菜单命令。
- 重复检测按平台 + scope + 规范化键 + `commandId`；弹窗顶部显示警告；加焦点陷阱与关闭后恢复焦点。
- 删除无保证的「Tab 进入菜单栏」条目，或改成准确的「菜单栏聚焦时 Tab 离开」。

---

#### AUD-06 [P2] PM-02 菜单关闭后焦点未显式恢复（Tab / 执行动作）

**事实（代码可判定）**

- `AppMenuBar.vue` 的 `onMenuKeydown` 对 `Tab` 调用 `closeMenu(false)`；`runItem` 也调用 `closeMenu(false)`。
- `closeMenu(false)` 只清空 `openIndex`，不把焦点还给触发按钮，也不移动到下一个控件。菜单项 DOM 随后被移除，焦点可能落到 `body`。
- `Escape` 路径是正确的：`closeMenu(true)` 会 `nextTick` 聚焦原按钮。

**影响**

- 键盘用户按 Tab 离开菜单后焦点丢失，不能“继续正常焦点顺序”；执行菜单项后焦点也不可预测。
- 与 PM-02 验收「Tab 关闭菜单并继续正常焦点顺序」不一致。

**建议修复**

- Tab 关闭时阻止默认行为并显式把焦点移到菜单栏之后的下一个可聚焦元素，或先回到触发按钮再让默认 Tab 继续。
- 执行动作后由动作目标决定焦点；导航类动作应聚焦目标，纯展示类动作回到触发按钮。
- 加一条键盘回归清单：Esc、Tab、Enter、方向键、点击外部。

---

#### AUD-07 [P2] PM-05 结果摘要断点早于规格

**事实（代码可判定）**

- `styles.css` 当前在 `@media (max-width: 1279px)` 隐藏 `.status-result`（整个结果摘要），在 `<1000px` 隐藏连接状态。
- 第二批 PM-05 要求「≥1200px 显示全部；1000–1199px 隐藏结果摘要」；现在 1200–1279px 被过早隐藏。
- 事务/只读/执行中已经设为 `flex: 0 0 auto`，不再被媒体查询隐藏；安全优先级已修正。
- 结果按活动查询标签隔离、无残留的代码逻辑正确；`error`/running/DML/截断均有分支。

**影响**

- 1200–1279px 的窗口（接近默认 1280）看不到行数/耗时，信息密度低于规格；不是数据错误，但验收不一致。
- 需确认 960px 下 `status-message` 与 `status-meta` 不互相挤压。

**建议修复**

- 将 `.status-result` 的隐藏断点改为 `max-width: 1199px`；保留事务/只读/执行中不隐藏。
- 如需在 1000–1199px 显示行数，可加 `status-result-short`（仅行数）而不是整个隐藏。

---

#### AUD-08 [P2] PM-04 关闭栈的无效快照会被直接丢弃；部分成功文案可能不准确

**事实（代码可判定）**

- `reopenClosedTab` 先用 `popClosedTab` 移除栈顶，再校验来源连接；连接已删除时直接 `return`，该快照永久丢失，用户无法重试或看到“已跳过”的记录。
- 表/ER/查询恢复在 `selectTableTab`/`showErTab`/`showQueryTab` 可能因事务或连接上下文失败而提前返回，但 `reopenClosedTab` 末尾仍可能设置「已恢复」状态。
- 关闭栈不持久化、最多 10 条、事务不恢复、文件路径只在内存，这些符合规格。

**影响**

- 用户连续按重开时，一个失效条目会“吃掉”一次操作；状态文案可能给出错误的成功反馈。
- 不是高频/高风险，但会影响 PM-04 的“跳过并提示”验收。

**建议修复**

- 用 `peek` + 校验通过后再 `pop`；无效条目移到临时已跳过列表或保留在栈中并加“不可恢复”标记。
- 恢复函数返回 boolean，只有激活成功才设置成功文案；失败时提示原因。

---

#### AUD-09 [P3] PM-01b 的 id/初始状态还有一些收口项

**事实（代码可判定）**

- `appMenus` 的 `editItem`（撤销/重做/剪切/复制/粘贴/全选）仍然没有 `id`；PM-06 文档要求补稳定 id。它们没有自定义原生同步需求（原生用预定义项），但会影响 PM-07 聚合与 id 对照。
- `nativeMenuState` 会发送 `menu-toggle-fullscreen`、`menu-minimize-window`、`menu-about` 等没有自定义原生 id 的项；Rust `sync_menu_state` 静默忽略，没有 `NATIVE_MENU_IDS` 白名单或 dev 日志。
- 四个 CheckMenuItem 初始都是 `checked(true)`；`rightOpen` 默认可能为 false。App 在 `onMounted` 后才同步一次，理论上首帧可能短暂显示错误勾选（未实机计时验证）。
- `modalBlockedActionIds` 是硬编码列表，新增菜单项容易忘记加入；AUD-01 已覆盖其后果。

**影响**

- ID 对照/自动化验收无法完全通过；新增动作时容易出现未知 id 或漏同步。
- 初始勾选闪烁是低风险，但会影响“菜单像系统菜单”的观感。

**建议修复**

- 给编辑菜单项补 id；建立 `NATIVE_MENU_IDS` 白名单并在 dev 下记录未知 id。
- 初始 checked 使用与 `DEFAULT_LAYOUT` 一致的值，或在显示前完成首次同步。
- 把 `modalBlockedActionIds` 改为 `modalBlockedPredicate(actionId)` 或由动作注册表声明 `blockedByModal`，避免新增动作漏加。

---

#### AUD-10 [P3] PM-06 的次要缺口

**事实（代码可判定）**

- `SqlEditor.runCommand` / `QueryWorkspace.runEditorCommand` 返回 void；App 无法区分“命令返回 false（例如无 commentTokens）”并给出状态提示；当前 SQL 方言有 commentTokens，实际风险低。
- `editor-find-next` / `editor-find-previous` 的菜单只展示 `F3` / `Shift+F3`，没有展示 CodeMirror 同时支持的 `Mod+G` / `Shift+Mod+G`；PM-06 文档要求两者可见。
- `editItem` 没有 id（见 AUD-09）。
- 其余核心行为（查找/替换、转到行、注释、原生菜单无 accelerator、依赖显式声明）在代码层面成立；`npm run check` 通过。

**影响**

- 快捷键发现性不完整；命令失败时用户没有反馈。
- 低风险，但属于 PM-06/PM-07 的验收细节。

**建议修复**

- 让 `runCommand` 返回 boolean，App 在 false 时设置状态。
- 菜单项显示 `F3 / ⌘G`（按平台格式化），并同步到 PM-07 清单。
- 给编辑菜单项补 id。

---

### 10.4 验收缺口汇总

| 需求 | 可判定失败/缺口 | 需实机复核 |
|---|---|---|
| PM-01b | AC-05 模态拦截；`close-window` 未检查未保存/事务；`shortcut-help`/`about` 可叠加；窗口内菜单无守卫 | 初始勾选 200ms、CheckMenuItem 点击与 sync 的最终一致性 |
| PM-02 | Tab/Enter 关闭菜单后焦点未显式恢复 | 原生菜单与窗口菜单同时打开时的焦点表现 |
| PM-03 | 对象定位 selector 未转义；目标不存在无提示；无 focus；ER scope 缺失；全部折叠隐藏当前范围；过滤未持久化；无 `aria-pressed` | 300 对象滚动性能、PG 模式切换后 immediate locate 是否误高亮 |
| PM-04 | Windows/Linux 无 `Shift+Mod+T`；无效快照被丢弃；失败时可能误报成功 | mac 原生 ⌘⇧T、Redis 控制台恢复、带脏文件标签恢复 |
| PM-05 | 结果摘要断点 <1280 早于规格 1200 | 960px 布局、后台查询切换、失败/截断结果 |
| PM-06 | 编辑菜单项无 id；查找下一个/上一个只显示 F3；命令失败无状态 | 原生编辑菜单点击、查找面板暗色样式 |
| PM-07 | `Ctrl+Tab` 显示 `CtrlTab`；静态清单非菜单聚合；缺 navigator/dialog/反向 Tab/Mod+G/编辑命令；重复检测不覆盖菜单且无弹窗提示；无焦点陷阱/恢复 | 弹窗 960×600 滚动、Esc 关闭后焦点、真实重复键注入 |

### 10.5 最需要修的 3 个问题

1. **AUD-01 模态拦截不完整（P0）**：子组件弹窗未纳入、窗口内菜单无守卫、`close-window` 可绕过未保存/事务检查。它会直接导致后台动作、嵌套弹窗和数据丢失风险。
2. **AUD-02 + AUD-03 PM-03 定位/折叠（P0/P1）**：未转义的对象 key 会让特殊表名定位报错/误定位；目标不存在无反馈；ER scope 缺失；“全部折叠”隐藏当前库/模式；过滤状态不持久化。PM-03 的 7 条验收里有多条不成立。
3. **AUD-04 Windows/Linux 快捷键未落地（P1）**：菜单显示的 `Ctrl+N/T/W/R/Y/M`、`Ctrl+Shift+T`、`Ctrl+Shift+W`、`Ctrl+Q` 在非 macOS 没有处理；PM-04 的重开标签快捷键在 Windows/Linux 直接不可用，且可能落到 WebView 默认行为。

**紧随其后**：AUD-05（PM-07 速查内容/格式化/焦点）是已实现功能里最明显的用户可见缺口；虽然风险低于前三条，但建议在宣称 PM-07 完成前修完。AUD-06（PM-02 焦点）、AUD-07（PM-05 断点）为一般质量项。

### 10.6 审计结论

- 第一版实现的主体方向正确：菜单双端、键盘导航、关闭栈、状态栏、编辑器命令、导航定位、快捷键速查都已落地，`npm run check` 和 57 个 Rust 测试通过。
- 当前最需要的是**收口而不是扩功能**：先补统一模态守卫、PM-03 定位/折叠的边界与焦点、非 macOS 快捷键；再修 PM-07 速查清单和 PM-05 断点；最后补 PM-02/PM-04 的焦点与失败文案。
- 本审计未修改任何源码、未提交；所有修复建议应在后续独立实现并重新跑 `npm run check`、`cargo test` 和平台手测。

---

## 11. 修复状态（2026-11-24 实现侧）

> 以下由实现侧在审计后更新，仅记录代码状态；仍需实机验收。

- **AUD-01 模态拦截**：已改为 `MutationObserver` 检测任意 `.modal-backdrop` + `domModalOpen`，窗口内/原生菜单共用置灰与执行守卫；补充 `close-window` 未保存/事务确认和顶层对话框 Tab 焦点闭环。已用连接对话框实测原生菜单全部置灰。
- **AUD-02/AUD-03 导航定位**：对象定位改为 data 属性遍历（不再拼接选择器），成功后聚焦目标行、失败提示；ER 标签定位数据库/模式；全部折叠保留当前库/模式；过滤状态持久化并补 `aria-pressed`；`locateTarget` 1.8s 后清理，避免重挂载误高亮。
- **AUD-04 非 macOS 快捷键**：补齐 N/T/W/R/Y/M/Q、Shift+W、Shift+T 等窗口级处理；模态打开时抑制应用快捷键。
- **AUD-05 快捷键速查**：macOS Control 显示为 `⌃`；补充 Shift+Ctrl+Tab、Mod+G/Shift+Mod+G、撤销/重做/剪贴板条目；增加重复键位提示和 Tab 焦点闭环/焦点恢复。列表仍以静态表为主，未从 `appMenus` 运行时聚合（后续可做）。
- **AUD-06 菜单焦点**：Tab 和菜单项执行后显式回到触发按钮，再交由动作或默认 Tab 顺序。
- **AUD-07 状态栏断点**：结果摘要改为 <1200px 隐藏，事务徽标始终可见。
- **AUD-08 恢复失败**：查询/ER 激活后校验活动标签，失败时提示“来源上下文切换被阻止”；失效快照仍按“跳过并提示”处理。
- **AUD-09/AUD-10**：编辑菜单项补稳定 id；初始原生勾选状态已实测；`runCommand` 的逐命令失败反馈尚未补，留作后续小步。
