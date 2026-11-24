# mydb 主界面与菜单栏调研及需求

> 角色：产品经理（桌面数据库客户端）
> 基线：mydb v0.10.0（2026-11-23）
> 日期：2026-11-24
> 范围：只做竞品研究、现状差距分析与可执行需求；本文不包含任何代码修改，不提交、不打包、不提升版本号。
> 阅读顺序建议：先看「0. 结论摘要」和「4. 首批建议」，再回看需求详情。
> 后续批次：第二批实现规格与第一批 PM-01b/PM-05 收口见 `docs/PM-REQUIREMENTS-2026-11-24-batch2.md`。
> 实施状态：截至 2026-11-24 下午，PM-01～PM-07 的第一版均已进入工作区（未提交）；实现审计与剩余问题见 `docs/PM-REQUIREMENTS-2026-11-24-batch2.md` §10。

---

## 0. 结论摘要

1. **DBX 走的是“工具栏 + 命令面板 + 右键菜单 + 设置页”的路线，没有窗口内文本菜单栏。** 它的强项是动作注册表、宽度溢出测量、Quick Open 分类搜索、标签右键管理、设置项搜索和快捷键注册表；这些更适合 mydb 作为后续能力参考，而不是照搬其 100+ 驱动、插件中心和 AI/MCP 体系。
2. **Navicat 18 的主窗口是明确的多面板工作台**：Main Toolbar、Navigation Pane、Tab Bar、Object Toolbar、Object Pane、Information & AI Chat Pane、Status Bar。菜单和面板负责“完整命令目录 + 当前上下文”，导航栏有过滤、星标、定位当前标签、全部折叠，查询结果支持位置切换、结果固定和多结果集。
3. **DBeaver 用 8 个顶层菜单承担完整命令目录**（File / Edit / Navigate / Search / SQL Editor / Database / Window / Help），并把工具栏、菜单、快捷键都做成可定制；Connections 视图提供 Link with editor、Show connected only、Collapse All、过滤模式，Search 视图有搜索历史。它**没有 VS Code 式的统一命令面板**（只有 Find actions `Ctrl+3` 搜命令），官方讨论区仍能看到用户请求。
4. **mydb 的主窗口骨架已经具备**：连接侧栏、对象树、统一查询/表/ER 标签、查询工作区、右侧历史、可调整布局、窗口内菜单和 macOS 原生菜单、连接 chip、状态栏。当前最明显的差距不是“缺一个大功能”，而是**菜单信息架构、菜单状态、导航定位和状态反馈不够一致**。
5. **本文件建议先修菜单与主界面的“可信度”**：窗口内菜单与原生菜单同源、同状态、同语义；菜单可用键盘操作；关闭标签可恢复；状态栏能说明“我现在在哪、在跑什么、事务是否未提交”；导航能一键定位当前连接/对象。完整命令面板、标签拖拽、设置中心、分屏等继续走已有 BACKLOG，不在本文件重复。

---

## 1. 调研结论

### 1.1 调研方法与事实边界

- **DBX**：读取 GitHub 仓库 `t8y2/dbx` 的 `main` 分支（调研提交 `0fbd067138d5e21523b0b35b74d471277246b520`，2026-11-24），直接查看 `apps/desktop/src` 下的 `App.vue`、布局组件、Quick Open、设置页、快捷键注册表和标签菜单源码。以下“事实”均来自该提交的源码；不引用二手截图描述。
- **Navicat**：读取 Navicat 18 官方在线手册的 Main UI、查询编辑器、查询结果、快捷键、配置、收藏、搜索过滤、控制台等页面。以下只写手册明确写出的内容；手册未展开的顶层菜单不作臆测。
- **DBeaver**：读取 DBeaver 官方文档的 Application Window Overview、Database Navigator、SQL Editor、Toolbar Customization、Shortcuts、Search 页面，并核对官方 GitHub 讨论区关于命令面板的讨论。
- 本文把「事实」和「建议」分开写：调研结论先陈述事实，再给「借鉴 / 不借鉴」的产品判断。所有 URL 见文末来源清单。

---

### 1.2 DBX

#### 事实：主界面结构

截至调研提交，DBX 桌面端主窗口的 UI 结构如下（源码路径见来源）：

- **顶部：AppToolbar（单行工具栏）**
  - 左侧：可配置的 primary 区，默认包含新建连接、新建查询、数据传输、驱动管理、插件中心；部分菜单动作可以被用户固定到主栏。
  - 中部：可拖拽的窗口标题区/工作区窗口切换器。
  - 右侧：可配置的 utility 区，包含更新检查、立即同步、置顶、导出进度、SQL 库、SQL 文件树、历史、AI、主题、GitHub、设置等。
  - 溢出：通过测量工具栏实际宽度决定哪些右侧项进入 More，而不是写死隐藏断点；工具栏项支持显示/排序/固定。
- **左侧：AppSidebar（连接/对象导航）**
  - 搜索、连接分组、连接树、多选、批量删除/移动分组、创建分组、全部折叠、定位活动标签。
  - 侧栏可悬停展开、可折叠为 rail；底部显示当前连接 endpoint。
- **中部：AppTabBar + EditorGroup**
  - 查询、数据、对象、设置页等标签；支持标签分组、拖动排序、固定、分离为独立窗口、最近关闭标签、关闭其他/左侧/右侧/全部、保存全部。
  - 标签栏支持纵向/横向位置、分组、换行/滚动等设置。
- **右侧：可切换的右侧面板**
  - AI 助手、历史、SQL 库、SQL 文件/书签等面板，可调整宽度、最大化、关闭。
- **起始页：WelcomeScreen**
  - 最近连接、新建连接、导入配置、最近 SQL、快捷键提示。
- **没有窗口内文本菜单栏**：源码中没有 AppMenuBar 之类的组件；主命令入口是工具栏、Quick Open、右键菜单和设置页。
- **没有发现独立的全局状态栏组件**：连接/执行状态分散在工具栏、标签和内容区；这一点与 mydb 已有底部状态栏不同。

#### 事实：Quick Open / 命令入口

- 默认快捷键 `Mod+P` 打开 Quick Open；`Mod+Alt+F` 打开全局搜索模式；`Mod+,` 打开设置；`Mod+T` 新建查询；`Shift+Mod+T` 重新打开关闭的标签；`Mod+B` 切换侧栏；`Shift+Mod+F12` 进入 Zen 模式。
- Quick Open 分类为：全部、连接、数据库、对象、字段、文件、文件内容、命令。支持按连接范围过滤、结果预览、内容搜索模式、每类计数、键盘上下选择和 `Mod+Enter` 次级动作。
- 它不是简单的“命令列表”，而是把连接、对象、文件、内容和命令统一到一个键盘入口。

#### 事实：设置与快捷键

- 设置页包含约 14 个桌面分类：外观、编辑器、SQL 格式化、导航、数据、备份、隧道、快捷键、代码片段、同步、AI、MCP、更新、关于（Web 端另有安全分类）。
- 设置页提供全局搜索，搜索结果按分类分组并可跳转到对应设置项；部分分类底部有“应用/重置”。
- 快捷键有集中注册表（id / label / scope / defaultShortcut），可在设置中录制和冲突检查；设置页搜索会包含快捷键条目。

#### 事实：标签菜单

`lib/tabs/tabMenu.ts` 明确提供以下菜单项构造函数：重命名/复制标签、定位活动标签、固定/取消固定、关闭、重新打开关闭的标签、关闭其他、关闭右侧、关闭左侧、关闭全部、保存全部。标签菜单是“标签管理”的主要入口，而不是只靠快捷键。

#### 借鉴什么（建议）

| 借鉴点 | 理由 | 在 mydb 的落点 |
|---|---|---|
| 动作注册表与统一可用状态 | 菜单、工具栏、命令面板、右键菜单共用一套动作和禁用原因，避免“同一个动作四处判断” | PM-01；后续 B-25 命令面板复用 |
| 标签右键最小集：重新打开、定位、关闭其他/右侧/全部 | 日常高频，且不依赖重架构；能显著减少误关标签的损失 | PM-04（重新打开）、PM-03（定位）；其余归 B-27 |
| 侧栏/对象树的“定位当前标签” | 连接和对象多时，活动标签的来源不容易找到 | PM-03 |
| 快捷键注册表 + 设置内搜索 | 解决“快捷键只存在代码里”的发现性问题 | PM-07（先做速查），设置化留给 B-28 |
| 设置页搜索、分类导航 | 设置项变多后仍可找到 | B-18/B-28，本文件不展开 |
| 宽度测量式溢出 | 比固定断点更稳，避免按钮被裁切 | 仅作为后续工具栏/菜单参考；本文件不重加工具栏 |

#### 不借鉴什么（建议）

| 不借鉴 | 原因 |
|---|---|
| 100+ 驱动、驱动商店、插件中心、插件运行时 | 与 mydb「轻量、纯 Rust 驱动、四个核心驱动」的定位冲突；需要独立架构评估 |
| AI 助手、MCP Server、Docker/Web 登录 | 涉及外部服务、凭据、隐私和额外依赖；mydb 当前不做 |
| 标签分离为独立窗口、跨窗口拖拽、工作区窗口合并 | 属于多窗口架构（B-29），成本高、状态同步复杂，不作为第一批 |
| 14 个分类的大型设置中心、背景图、Zen 模式、UI 缩放 | 对当前轻量客户端收益有限；设置中心已有 B-18/B-28 |
| 仿 DBX 的整行应用工具栏 | 与 2026-11-24 已做的“移除应用工具栏整行”决策冲突；详见 2.3 文档漂移 |

---

### 1.3 Navicat 18

#### 事实：主窗口结构

Navicat 18 官方手册把主窗口拆成以下部分：

- **Main Toolbar**：访问连接、用户、表、集合、备份、自动化等基础对象和功能；右键工具栏可关闭大图标/标题，`View -> Show Toolbar` 控制显示。
- **Navigation Pane**：连接、数据库、数据库对象的树形导航；`View -> Navigation Pane -> Show Navigation Pane` 控制显示；支持 `Flatten Connection` 去掉分组层级。
  - 底部 Filter bar 支持：搜索框即时过滤、按连接类型/数据库厂商/活动对象过滤、只看星标连接、定位当前活动标签、全部折叠。
  - `View -> Navigation Pane -> Show Filter On Top` 可把过滤栏放到顶部。
  - 双击数据库/模式建立连接；可展开表字段、索引、外键；可右键重命名字段、双击打开表设计器。
- **Tab Bar**：切换对象窗口标签；`Ctrl+Tab` 切换标签；可选择新标签在主窗口/最后标签窗口/新窗口打开。
- **Object Toolbar**：操作当前对象的附加控件。
- **Object Pane**：对象列表；List / Detail / ER Diagram 三种视图；`View -> Flatten Object List` 去掉分组；`View -> Choose Columns` 选择 Detail 视图显示列。
- **Information & AI Chat Pane**：共享面板，在 Information（常规信息、Preview SQL、DDL、Using、Objects、Member Of、Used By 等）和 AI Chat 之间切换；可最大化/恢复宽度。
- **Status Bar**：显示活动进程、连接状态和当前窗口信息。
- **连接颜色**：连接颜色显示在导航栏、对象窗口菜单栏/标签上，用于识别当前连接。
- 另有 Focus Mode、Dark Theme 等外观能力。

#### 事实：菜单与快捷键

手册中可核实的菜单路径包括：

- `File / Favorites -> Add to Favorites`
- `Edit -> Find`
- `View -> Navigation Pane -> Show Navigation Pane / Flatten Connection / Show Filter On Top`
- `View -> Information Pane -> Show Information Pane`
- `View -> Flatten Object List`
- `View -> Choose Columns`
- `View -> Word Wrap`
- `View -> Display -> Zoom`
- `View -> Result`（结果位置）
- `View -> Show Query Profiling and Status`
- `Tools -> Options`
- `Tools -> Find in Database/Schema`
- `Tools -> Console`
- `Tools -> Structure Synchronization`

快捷键（手册明确列出）包括：`Ctrl+Tab` 切换标签、`Ctrl+Q` 新建查询、`Ctrl+,` 打开选项、`F8` 主窗口/对象标签、`F11` Focus Mode、`F12` 只显示活动对象、`Ctrl+L` 历史日志、`Ctrl+Shift+F` 在数据库/模式中查找、`F6` 控制台、`Shift+Ctrl+#` 加入收藏、`Ctrl+#` 打开收藏。

#### 事实：查询编辑器与结果

- Query Editor 支持格式化（缩进、注释、大小写、Beautify SQL、Minify SQL）、实时错误检测、悬停信息、Ctrl+点击跳转声明、重构（重命名/别名）、剪贴板栈、代码折叠、语法/括号/用法高亮、查找替换、带引号复制、Word Wrap、缩放。
- Query Results 支持：运行全部/选中/当前语句、Continue on error、结果工具栏、单元格编辑器、数据剖析、导出、Pin 结果、最多 20 个并发结果集、查询参数 `[$name]`、自定义结果标签名、结果位置（整页/编辑器右侧/下方）、Query Profiling/Status。
- 结果标签被重新执行时会清空，Pin 后保留；Pin 标签会显示 pin 图标并自动靠左排列。

#### 事实：Options 组织

Options 覆盖 General、Appearance、Tabs、Code Insight、Editor、Records、Grid、Data Profiling、Format、AI、Auto Recovery、File Locations 等分组；可配置主题、标签打开位置与启动恢复、代码补全、编辑器字体/颜色、分页、事务、网格字体/斑马纹、日期格式、自动保存间隔、配置/导出目录等。

#### 借鉴什么（建议）

| 借鉴点 | 理由 | 在 mydb 的落点 |
|---|---|---|
| 面板职责清晰的七段式主窗口 | 用户能理解“连接在哪、对象在哪、编辑器在哪、信息在哪、状态在哪” | 差距分析基线；不要求新增面板 |
| Navigation Pane 的过滤、定位当前标签、全部折叠 | 直接解决多连接/多对象导航 | PM-03 |
| 菜单作为完整命令目录，View 子菜单控制面板显示 | 与 mydb 现有 6 菜单结构兼容 | PM-01 |
| 查询结果位置、Pin、多结果集 | 多结果分析真实需求 | 已有 B-07；本文件不重复，作为后续 |
| Options 分组与设置搜索 | 设置项增长后的组织方式 | B-18/B-28，本文件不展开 |
| 连接颜色贯穿标签/对象 | 多连接防误操作 | mydb 已有连接颜色和标签来源，可在 PM-05 状态栏增强显示 |

#### 不借鉴什么（建议）

| 不借鉴 | 原因 |
|---|---|
| Main Toolbar 上铺满对象创建、同步、备份、报表、计划任务、用户/服务器监视 | 会重新引入第二排重工具栏；mydb 已决定移除应用工具栏整行 |
| Virtual Grouping、Model、BI、Data Generation、Automation、报表打印、云协作、On-Prem Server | 企业级/重工作流，与轻量客户端定位冲突，且需要独立架构 |
| Favorites、Find in Database/Schema 等 Enterprise/Standard 功能作为核心依赖 | mydb 不按版本分层；如做，应作为轻量替代（本地搜索/最近使用），不直接照搬 |
| 每个对象打开独立模态设计器、多个窗口 | 交互成本高；mydb 已用标签 + 内联/对话框混合模式 |
| 大而全的 Options 第一版 | 设置中心已有 B-18/B-28；本文件只做菜单和状态反馈 |
| 复制 Navicat 的图标、插画、品牌资源 | 第三方资源不可复制；只参考交互组织 |

---

### 1.4 DBeaver

#### 事实：主窗口结构

DBeaver 官方文档明确：主窗口包含 menu bar、toolbar、main area（一个或多个 editors 和 views）以及 status bar。

- **Menu Bar**：默认包含 File、Edit、Navigate、Search、SQL Editor、Database、Window、Help。
  - File：创建文件/文件夹/项目/数据库连接/数据库项目/Diagram，以及 Import/Export。
  - Edit：Cut、Copy、Paste、Delete、Add Bookmark 等，作用于当前活动元素。
  - Navigate：在脚本和数据库对象间导航。
  - Search：搜索文件、数据库对象和数据。
  - SQL Editor：打开 SQL 编辑器并管理其外观。
  - Database：驱动、连接、事务、重连/断开。
  - Window：显示/隐藏、最小化/最大化视图和编辑器，显示栏，分屏，管理其他偏好。
  - Help：帮助资源、版本、更新。
  - 菜单可通过 `Window -> Customize Perspective -> Menu Visibility` 定制。
- **Toolbar**：高频基础命令；按钮启用状态随活动编辑器变化；可通过 `Window -> Customize Perspective -> Toolbar Visibility` 定制分区。
- **Shortcut Bar**：主区域左右两侧，视图/编辑器最小化后以快捷按钮出现。
- **Main area**：views（导航/信息）和 editors（内容）可拖拽停靠、标签化、分屏、最大化/最小化；标签溢出时有 Show List 图标。
- **Status bar**：文档确认存在，但未展开具体字段。

#### 事实：Connections / Database Navigator

- Connections 是数据库结构/数据的主工具，树形展示 connections、schemas、tables、views。
- 过滤栏在树上方，输入即时过滤；支持 View 菜单、右键 Filter、Hide/Show only/Configure/Toggle/Clear filter。
- 支持 folders、拖拽分组、Simple/Advanced/Custom 视图、Show all databases。
- 工具栏：Collapse All、Link with editor、Show all connections/Show connected only、Filter mode、View menu。
- View 菜单：New Database Connection、Driver Manager、Active Project、New Folder、Collapse All、Link with editor、Preferences、Refresh Projects。
- 右键菜单：Create/Open/Copy/Paste/Delete/Rename/Refresh/Connect/Invalidate/Disconnect、SQL Editor、Edit connection、Customize view、View/Edit/Create object、Filter、Copy Advanced Info、Read data in SQL console、Compare/Migrate、Generate SQL、Export/Import Data、Tools。
- Preferences 可配置连接主机显示、对象描述、统计、动作图标、文件夹优先、子项计数、排序、连接颜色、双击行为、默认编辑器页、展开深度、元素分页等。

#### 事实：Search 与 SQL Editor

- Search 视图统一承载 Database metadata search、Database full-text search、File search、Git search；结果在独立 Search 视图；支持重新运行、取消、Pin 视图、Previous Searches 历史、移除单条/清空历史、进度和 Background Tasks。
- 打开 Search 视图的入口是主工具栏上的 Search 按钮。
- SQL Editor 由 Script panel、Toolbar、Results panel 组成；结果面板是 Data Editor 实例，可多结果集同标签查看；支持 Outline、布局切换（隐藏/最大化结果面板、横向/纵向）、Active Database/Schema、错误标记、AI 解释错误（PRO）。
- SQL Editor 工具栏可定制；`Window -> Preferences -> User Interface -> Toolbar Customization` 或右键 Preferences 进入。
- 快捷键可在 `Window -> Preferences -> User Interface -> Keys` 修改，并提供 DBeaver Keyboard Only 方案。

#### 事实：命令面板

- 官方文档和快捷键页没有列出 VS Code 式命令面板。
- 官方 GitHub 讨论区存在用户请求：`dbeaver/discussions/35599`「Command Palette?」和 `dbeaver/discussions/19538`「Add a Ctrl+P palate...」。在 #35599 中，提问者后来补充发现了 **Find actions（Ctrl+3）**，可以在一个入口搜索命令；但讨论的主诉仍是“想在一个对话框里同时搜索命令、文件、对象”，说明 DBeaver 没有 VS Code 式的统一命令面板。
- 因此，DBeaver 的“快速入口”主要是菜单、工具栏、Find actions（Ctrl+3）、Connections/Search 视图和可配置快捷键；没有把连接/对象/文件/命令合并到一个 Quick Open。

#### 借鉴什么（建议）

| 借鉴点 | 理由 | 在 mydb 的落点 |
|---|---|---|
| 菜单按职责分层，命令目录完整 | 用户能在菜单里找到所有全局动作，而不是靠记忆工具栏 | PM-01 |
| 菜单/工具栏可定制、快捷键可改 | 长期方向正确 | B-38/B-28；本文件只做快捷键速查 PM-07 |
| Connections 的 Collapse All、Link with editor、过滤 | 与 mydb 侧栏/对象树直接对应 | PM-03 |
| Search 视图的搜索历史、Pin、取消、进度 | 未来全局搜索的产品基线 | B-25 后续；本文件不重复 |
| SQL Editor 的结果面板布局、Outline、错误位置 | 编辑器可信度 | 部分已在 mydb（结果折叠/错误条）；其余不抢跑 |
| Status bar 作为持续上下文 | 与 PM-05 一致 | PM-05 |
| 快捷键文档化与键盘优先 | 提升可发现性和效率 | PM-07 |

#### 不借鉴什么（建议）

| 不借鉴 | 原因 |
|---|---|
| Eclipse 式可停靠 views/editors、projects/workspaces、perspectives | 需要重写主界面布局模型，属于架构级改造；mydb 用固定工作区 + 可调宽度更稳 |
| 8 个顶层菜单 + 大量二级菜单 | 对轻量客户端过重；mydb 保持 6 个顶层菜单，通过 Edit/View 承载查找与导航 |
| 插件生态、JDBC 驱动管理、Driver Manager | 与 mydb 纯 Rust 驱动路线冲突 |
| Search 的跨文件/跨 Git/全文数据库搜索全部一次做完 | 需要索引、后台任务、权限和性能设计；超出当前批次 |
| AI、MCP、云连接、团队协作 | 与当前本地轻量定位冲突 |
| 复制 DBeaver 的图标/皮肤/品牌资源 | 第三方资源不可复制 |

---

### 1.5 横向对比与对 mydb 的启示

| 维度 | DBX | Navicat 18 | DBeaver | 对 mydb 的启示 |
|---|---|---|---|---|
| 主窗口 | 工具栏 + 侧栏 + 标签/编辑器组 + 右侧面板 | 7 段式工作台 + 状态栏 | 菜单 + 工具栏 + 可停靠区域 + 状态栏 | 保持现有四区骨架，不重写布局 |
| 菜单 | 无窗口文本菜单，命令面板/右键为主 | 菜单 + 工具栏并重 | 8 个顶层菜单，完整命令目录 | mydb 保留 6 菜单，但要让菜单“完整、同源、可用” |
| 工具栏 | 可定制、测量式溢出 | 重工具栏、可切换大图标/标题 | 可定制分区 | 不重新引入整行工具栏；用菜单/对象树/命令面板补入口 |
| 导航 | 侧栏搜索、定位、多选、折叠 | Filter bar、星标、定位、折叠 | Link with editor、connected only、过滤模式 | 先做定位与折叠，不伪造连接状态，不抢 B-26 收藏 |
| 标签 | 分组/拖动/固定/分离/重开 | Tab Bar、结果 Pin、20 结果集 | 标签组、溢出列表、分屏 | 先做“重新打开关闭的标签”，其余归 B-27/B-29 |
| 快速入口 | Quick Open 分类搜索 | 菜单/工具栏/收藏 | 菜单/Search 视图/Find actions | 命令面板已有 B-25，本文件不重复 |
| 设置 | 14 分类 + 搜索 + 可定制快捷键 | Options 多分组 | 菜单/工具栏/快捷键可定制 | 设置中心归 B-18/B-28；本文件只做快捷键速查 |
| 状态反馈 | 分散在工具栏/标签 | 独立 Status Bar | 独立 Status Bar | mydb 状态栏需要补上下文，而不是重做 |
| 结果区 | 多结果/网格/工作台 | 多结果、Pin、位置切换 | 多结果、Outline、布局切换 | 已有 B-07；本文件不抢跑 |

**一句话结论（建议）**：mydb 不需要更像 DBX 的“全家桶”，也不需要更像 DBeaver 的“Eclipse 工作台”；它需要把自己的 6 个菜单、4 个区域、1 条状态栏做成**同一套动作、同一套状态、同一套导航语言**。

---

## 2. 差距分析

### 2.1 现状核对（以当前代码为准）

已阅读并核对：`README.md`、`CHANGELOG.md` 的 `[Unreleased]`、`docs/BACKLOG.md`、`docs/ARCHITECTURE.md`、`src/App.vue` 的模板与 `appActions/appMenus`、`src/components/AppMenuBar.vue`、`WorkspaceStart.vue`、`ConnectionDialog.vue`、`ObjectBrowser.vue`、`ObjectCategoryGroups.vue`、`QueryWorkspace.vue`、`SqlEditor.vue`、`StatusBar.vue`、`HistoryPanel.vue`、`src/lib/app-actions.ts`、`src-tauri/src/lib.rs` 的 `build_menu`。

关键事实：

- 主窗口已有四区骨架：连接侧栏、对象树、编辑器/结果、右侧历史；侧栏/对象树/结果/右栏可拖拽、可折叠，布局写入 localStorage。
- 已有统一标签栏（查询/表/ER），`Ctrl+Tab` 循环，查询标签持久化，表/ER 标签仅会话。
- 已有窗口内文本菜单（文件/编辑/视图/工具/窗口/帮助）和 macOS 原生菜单（`#[cfg(target_os = "macos")]`）。
- 已有 `src/lib/app-actions.ts` 动作注册表（id/group/label/icon/keywords/shortcut/priority/placement/enabled/disabledReason/run），但目前主要被 `appMenus` 消费，没有应用工具栏行。
- 已有零标签「主工作区」：最近查询、当前连接、当前库对象入口；Redis 有对应形态。
- 已有 SQL 打开/保存/另存为、文件绑定未保存圆点、关闭提示；已在 `CHANGELOG` 的 `[Unreleased]` 记录。
- 状态栏目前只显示状态消息、驱动/地址、当前库/模式、结果行数与耗时。
- 没有设置中心、主题切换、命令面板、全局对象搜索、标签拖拽/固定/右键菜单、重新打开关闭标签、快捷键速查。

### 2.2 差距表

状态取值：**已有** = 已实现且可用；**部分有** = 有骨架/部分入口但缺少一致性或关键能力；**缺失** = 未实现；**有意不做** = 已由用户决策移除，不作为缺口。

| 领域 | 现状 | 状态 | 差距 / 说明 | 参考 | 本文件处理 |
|---|---|---|---|---|---|
| 主窗口分区 | 连接侧栏、对象树、编辑器/结果、右侧历史 | 已有 | 结构清晰；缺状态栏上下文和导航联动 | Navicat 7 段式、DBeaver main area | PM-03/PM-05 |
| 可调整布局 | 四条拖拽分隔线、折叠 rail、结果折叠、localStorage | 已有 | 无需重做；新增菜单项要遵守现有布局状态 | Navicat 面板显隐、DBeaver docking | 不重复 B-23 |
| 窗口内菜单栏 | 6 个顶层菜单，动作部分来自注册表 | 部分有 | 无键盘方向导航、无勾选、无子菜单、部分动作重复/语义错误 | DBeaver 菜单职责、Navicat View 子菜单 | PM-01/PM-02 |
| macOS 原生菜单 | 6 个顶层菜单 + app menu | 部分有 | 与窗口内菜单动作集不一致；菜单项始终启用；无 check 状态；无重新打开/查找/导入导出 | DBeaver 菜单目录、Navicat 菜单路径 | PM-01 |
| 应用工具栏整行 | 已移除，仅保留布局按钮/连接 chip | 有意不做 | 2026-11-24 决策是移除；BACKLOG B-32 仍描述工具栏方案，存在漂移 | DBX toolbar、Navicat Main Toolbar | 不重新引入；见 2.3 |
| 命令面板 / 快速打开 | 缺失 | 缺失 | 已有 B-25 完整规格，本文件不重复 | DBX Quick Open；DBeaver 仅 Find actions `Ctrl+3` | B-25 |
| 连接侧栏 | 搜索、分组、颜色、状态点、新建连接/分组、组级连接/断开 | 部分有 | 无“有打开标签/当前连接”过滤；无定位活动标签；无全部折叠；折叠状态不持久 | Navicat Filter bar、DBeaver Connections | PM-03 |
| 对象树 | 数据库/模式/分类/数量、搜索、键盘导航、活动表高亮 | 部分有 | 活动表只加 class，不滚动/展开；无全部折叠；无定位活动表按钮 | Navicat locate/collapse、DBeaver Link with editor | PM-03 |
| 标签栏 | 统一标签、横向滚动、关闭、来源、运行点、脏点 | 部分有 | 无右键菜单、无重新打开、无固定/拖动/溢出列表 | DBX tabMenu、Navicat Tab Bar、DBeaver tabs | PM-04；其余 B-27 |
| 零标签主工作区 | 最近查询、当前连接、当前库对象 | 部分有 | 无最近连接/最近对象/“打开 SQL 文件”；无持久 Home 标签 | DBX WelcomeScreen、B-26 | B-26，本文件不重复 |
| SQL 编辑器 | 方言、补全、选区执行、事务、文件打开/保存 | 部分有 | 查找/替换/注释/转到行等 CodeMirror 能力没有菜单入口；无格式化 | Navicat Query Editor、DBeaver SQL Editor | PM-06 |
| 结果区 | 排序、隐藏列、列宽、导出、错误条、折叠 | 部分有 | 无多结果集/Pin/结果位置 | Navicat/DBeaver 结果区 | B-07，本文件不重复 |
| 右侧面板 | 历史（搜索/回填/清空） | 部分有 | 无信息/对象详情/搜索/后台任务 | Navicat Information Pane、DBeaver views | B-24，本文件不重复 |
| 状态栏 | 状态消息、驱动/地址、库/模式、结果行数/耗时 | 部分有 | 无活动标签/连接状态/事务/只读/执行中/截断等上下文；窄窗口无优先级策略 | Navicat Status Bar、DBeaver status bar | PM-05 |
| 搜索/定位 | 连接搜索、对象搜索、历史搜索 | 部分有 | 无全局搜索、无搜索历史、无“定位当前标签” | DBeaver Search view、Navicat locate | PM-03；全局搜索 B-25 |
| 设置/主题 | 无设置界面，暗色固定 | 缺失 | 无主题切换、编辑器字体、标签行为、确认策略等 | Navicat Options、DBX Settings、DBeaver Preferences | B-18/B-28 |
| 快捷键 | 分散在 App.vue、Rust 菜单、CodeMirror、组件内 | 部分有 | 无速查入口、无集中展示、无冲突检查 | DBeaver Shortcuts、Navicat Hot Keys、DBX registry | PM-07 |
| 多窗口/分屏 | 单窗口 | 缺失 | 不属于本批 | DBeaver docking、Navicat 新窗口 | B-29 |

### 2.3 文档漂移与边界说明

这是本次调研发现的产品文档问题，建议在实施前先校正，否则会按旧方案重复开发：

1. **BACKLOG B-32 与当前代码/CHANGELOG 冲突**：B-32 写「AppToolbar 分组/溢出、连接 chip、Run 移入编辑器，代码已完成」，但当前 `src/components/` 没有 `AppToolbar.vue`，`CHANGELOG` 的 `[Unreleased]` 明确记录「移除菜单栏下方的应用工具栏整行」。本文件以**当前代码 + CHANGELOG** 为准：不重新引入整行应用工具栏，动作入口由菜单、对象树、右键菜单和后续命令面板承担。
2. **BACKLOG B-06 状态过期**：B-06 仍标「SQL 文件打开/保存待办」，但 `CHANGELOG` 的 `[Unreleased]` 已记录该能力进入开发版。建议在 BACKLOG 中改为「进行中/待验收」，避免重复实现。
3. **B-25/B-27/B-28/B-29/B-38 已有完整规格**：本文件只引用，不重写。尤其是命令面板、标签拖动/固定/溢出、设置中心、分屏、工具栏自定义，不应在本批重复提案。
4. **菜单双端边界**：macOS 原生菜单由 `src-tauri/src/lib.rs` 的 `build_menu` 构建；Windows/Linux 当前没有原生菜单，只有窗口内菜单。PM-01 的原生菜单同步要求仅适用于 macOS，其他平台以窗口内菜单状态为准。
5. **“删除最后一个查询标签”后的零标签状态**：这是现有有意设计（主工作区），不是缺陷。PM-05 状态栏需要支持这种状态，而不是自动新建标签。

---

## 3. 需求清单

### 3.0 编写约定

- 优先级：P0 = 本批立即做，直接提升主界面/菜单可信度；P1 = 紧随其后，成本中等；P2 = 后置或依赖前序能力。
- 所有需求只列「尚未实现」或「明确要改的现状」；不重复 B-01～B-38 已覆盖且未变更的内容。
- 范围默认优先前端 Vue/CSS；涉及 Rust 的条目会写明命令与平台；不新增数据库驱动，不引入新原生依赖。
- 每条验收标准 4–5 条，均可直接操作验证；安全底线：不泄露凭据、不绕过只读、不造成数据丢失。

| 需求 | 标题 | 优先级 | 是否首批 |
|---|---|---|---|
| PM-01 | 菜单信息架构与窗口内/原生菜单一致性 | P0 | 是（分 1a 前端 / 1b 原生） |
| PM-02 | 菜单栏键盘导航与可访问性 | P1 | 是 |
| PM-03 | 导航联动：连接侧栏与对象树定位 / 折叠 / 工作集过滤 | P1 | 否（紧随其后） |
| PM-04 | 重新打开关闭的标签 | P1 | 是 |
| PM-05 | 状态栏上下文与执行反馈升级 | P1 | 是（可与 1b 并行） |
| PM-06 | SQL 编辑器基础命令接入菜单 | P1 | 否（后续小步） |
| PM-07 | 快捷键速查与冲突检查 | P2 | 否 |

---

### PM-01 菜单信息架构与窗口内/原生菜单一致性

- **优先级**：P0
- **用户问题 / 场景**：
  - 同一个动作在「文件」和「工具」重复出现（如「导入数据」），用户不知道哪个是正式入口。
  - 窗口内菜单和 macOS 菜单内容不一致：原生菜单没有导入/导出、编辑/关闭连接、全屏等；窗口内菜单有「退出 mydb」，但实际执行的是关闭窗口。
  - 原生菜单项始终可点，没有连接时点「ER 图」「结构同步」会静默无反应；窗口内菜单却有禁用原因。
  - 面板开关没有勾选状态，用户无法从菜单判断当前哪些面板可见。
- **期望行为**：
  1. **单一菜单模型**：在 `src/lib/app-actions.ts` 或新增 `src/lib/app-menu.ts` 定义共享的菜单项（id、label、shortcut、action、enabled、disabledReason、checked、separator）；窗口内菜单和原生菜单从同一份 id/动作语义生成，禁止两套文案和两套判断。
  2. **固定职责**：
     - 文件：连接/文档生命周期（新建连接、新建查询、打开/保存/另存 SQL、导入数据、导出结果、关闭标签、重新打开关闭的标签、关闭窗口）。
     - 编辑：文本编辑（撤销/重做、剪切/复制/粘贴/全选，以及 PM-06 的查找/注释/转到行）。
     - 视图：刷新、执行历史、连接侧栏/对象树/结果面板/右侧面板开关（带勾选）、重置布局、切换全屏。
     - 工具：ER 图、结构同步、数据同步、编辑当前连接、关闭当前连接。
     - 窗口：最小化、平台需要的「前置全部窗口」；不再重复关闭窗口和退出。
     - 帮助：关于 mydb、键盘快捷键（PM-07 落地后）、GitHub 项目主页。
  3. **去重与语义修正**：
     - 「导入数据」只保留在文件菜单；从工具菜单移除重复项。
     - 「关闭窗口」只保留在文件菜单；窗口菜单只保留最小化和平台需要的「前置全部窗口」，不再重复关闭。
     - 「退出 mydb」不得再调用 `closeWindow`；macOS 只保留 app menu 的 Quit，窗口内菜单不再放「退出」；其他平台如需退出，调用 Tauri app quit 并二次确认（若当前有未保存 SQL/未提交事务）。
     - 全屏只在一个位置出现（建议视图），原生菜单的窗口菜单不再重复。
  4. **原生菜单同步**：
     - macOS 原生菜单补齐窗口内已实现的导入数据、导出结果、编辑/关闭连接、重新打开关闭的标签、切换全屏等动作；保留 app menu 的 about/services/hide/quit。
     - 面板开关使用 `CheckMenuItem`；普通项根据前端状态 `set_enabled`；状态由前端通过新增命令 `sync_menu_state` 同步（节流 ≥80ms，仅在相关状态变化时调用）。
     - 不可用项在窗口内菜单显示禁用原因；原生菜单至少置灰，不允许“可点但无反应”。
  5. **边界**：
     - 「新建表」继续只从对象树 `+` 和对象右键进入，不回菜单（与 2026-11-24 决策一致）。
     - 所有菜单动作只调用一次；菜单事件不得同时被窗口内菜单和全局 keydown 重复触发。
- **验收标准**：
  - [ ] 并排检查窗口内菜单和 macOS 原生菜单：除 macOS app menu（关于/服务/隐藏/退出）外，同一动作的文案、快捷键、启用状态一致，且没有重复的导入/导出/关闭/退出项。
  - [ ] 没有连接时，窗口内和原生菜单的 ER 图、结构同步、数据同步、刷新均置灰；窗口内菜单悬停/键盘可读到具体原因（如「请先选择连接」）；Redis 连接下 SQL 相关项置灰并显示对应原因；只读连接下导入数据等写操作菜单项置灰。
  - [ ] 点击视图菜单的连接侧栏/对象树/结果面板/右侧面板，菜单项勾选状态与面板实际显隐一致；用快捷键切换后 100ms 内菜单勾选同步。
  - [ ] macOS 窗口内菜单不再出现「退出 mydb」，app menu Quit 正常退出；Windows/Linux 的「退出 mydb」调用 app quit 而不是关闭窗口；关闭窗口只关闭窗口；不存在“点了没反应”的菜单项。
  - [ ] 切换标签、连接、只读状态、Redis/非 Redis、运行中状态时，菜单可用性更新且不触发重复查询、不改变当前标签/事务；连续快速切换不会造成菜单闪烁或状态错乱。
- **范围**：
  - 前端：`src/lib/app-actions.ts`、`src/App.vue`（appMenus、menu 事件、状态快照）、`src/components/AppMenuBar.vue`（checked 渲染、禁用原因）；无新增依赖；必要时新增 `src/lib/app-menu.ts`。
  - 后端：`src-tauri/src/lib.rs` 的 `build_menu`（CheckMenuItem、缺失动作、id 常量表）、新增 `sync_menu_state` 命令；`src-tauri/src/commands/` 下新增菜单状态命令（或放在 lib.rs 邻近模块）。
  - 驱动：无。
- **风险 / 备注**：
  - Tauri 2.12.5 本地 crate 源码已确认 `AppHandle::menu()`、`Menu::get(id)`、`MenuItem::set_enabled`、`CheckMenuItem::set_checked` 可用；仍需在 macOS 实机验证原生菜单置灰/勾选外观、快捷键与点击事件。
  - 原生菜单状态同步要防抖，避免每次输入/查询结果变化都重建菜单；只监听菜单相关状态（连接、只读、Redis、运行中、活动标签、面板显隐、事务状态）。
  - 非 macOS 没有原生菜单，验收只检查窗口内菜单；不要为 Windows/Linux 伪造原生菜单。
  - 菜单 id 容易漂移；建议在 Rust 与 TS 各保留一份稳定 id 列表，交付前人工对照一次，后续可用脚本校验。

---

### PM-02 菜单栏键盘导航与可访问性

- **优先级**：P1
- **用户问题 / 场景**：
  - 窗口内菜单只能用鼠标：Tab 无法进入，方向键无效，Esc 只关闭不恢复焦点；键盘用户和习惯全键盘操作的用户被排除。
  - 菜单按钮没有 `role="menubar"` / `role="menuitem"`，读屏器无法理解层级。
  - 打开菜单后焦点的位置不可预测，关闭后不知道回到哪里。
- **期望行为**：
  1. 顶层菜单是 roving tabindex：Tab 只把焦点放到菜单栏一次；←/→ 在顶层菜单之间移动；Home/End 到第一个/最后一个顶层菜单。
  2. Enter / Space / ↓ 打开当前菜单，焦点落到第一个可用项；↑/↓/Home/End 在菜单项间移动；Enter/Space 执行；Esc 关闭并把焦点还给触发按钮；Tab 关闭菜单并继续正常焦点顺序。
  3. 菜单打开时，→/← 切换到相邻顶层菜单并保持打开；鼠标 hover 仍可切换；点击外部关闭（现有行为保留）。
  4. 支持首字母 typeahead（500ms 内连续输入）；跳过分隔线；禁用项可聚焦以便读原因，但 Enter/点击不执行。
  5. 添加 `role="menubar"` / `role="menu"` / `role="menuitem"`、`aria-expanded`、`aria-haspopup`、`aria-disabled`；焦点可见环；打开菜单时不让按键穿透到 CodeMirror/表格。
- **验收标准**：
  - [ ] 在 SQL 编辑器聚焦时按 Tab 可进入菜单栏；←/→ 切换「文件/编辑/…/帮助」，Enter 打开，↑/↓ 选项，Esc 后焦点回到原按钮，再按 Tab 继续到页面下一个控件。
  - [ ] 打开「视图」菜单后按 ↑/↓ 可遍历所有项（含分隔线跳过）；Home/End 到首/尾；按 Enter 执行当前项并关闭菜单。
  - [ ] 在菜单打开时按 ←/→ 切换顶层菜单，菜单保持展开；点击外部或 Esc 关闭；关闭后不向编辑器输入字符。
  - [ ] 禁用项可被高亮/聚焦，读屏可读出 `aria-disabled` 和原因，但 Enter/点击不执行、不关闭菜单。
  - [ ] 连续快速按方向键/首字母不会报错、不会打开多个菜单、不会改变当前标签或执行数据库动作。
- **范围**：
  - 前端：`src/components/AppMenuBar.vue`（键盘状态、焦点管理、ARIA）、`src/App.vue`（打开时关闭其他浮层、全局 keydown 避让）、`src/styles.css`（`:focus-visible`）。
  - 后端/驱动：无。
- **风险 / 备注**：
  - CodeMirror 和表格单元格有自己的键盘处理；菜单打开时必须拦截方向键/Enter/Esc，关闭后完全恢复。
  - 中文输入法组合期间不要触发 typeahead；可用 `event.isComposing` 跳过。
  - 不要在全局捕获阶段处理普通字符，避免影响编辑器输入；只在菜单打开时处理。
  - 与布局快捷键、`Ctrl+Tab`、原生菜单加速键的冲突：菜单打开时优先菜单，关闭后恢复。

---

### PM-03 导航联动：连接侧栏与对象树定位 / 折叠 / 工作集过滤

- **优先级**：P1
- **用户问题 / 场景**：
  - 连接多、对象多时，打开一个表标签后，左侧侧栏和对象树仍停在别处；用户要手工展开分组、数据库、模式、分类，才能找到当前标签来源。
  - 连接分组和对象分类很多时，没有“全部折叠”，树越用越长。
  - 用户想只看“有打开标签的连接”，快速回到工作集，但当前没有过滤。
  - 注意：mydb 目前只有当前选中连接有实时状态，其他连接状态是 unknown；不要照搬 DBeaver 的 “Show connected only” 而伪造在线状态。
- **期望行为**：
  1. 连接侧栏标题行增加一个轻量过滤切换：**全部 / 有打开标签**。过滤只影响显示，不改变连接状态、不自动连接；搜索词与过滤同时生效；分组计数按过滤后结果更新；无匹配时显示空状态。
  2. 连接侧栏标题行增加「定位当前标签」按钮：根据活动标签的来源连接 id，展开其所在分组、滚动到可见并短暂高亮；如果活动标签没有连接（未绑定查询/空工作区），按钮禁用并显示原因；不自动连接、不切换连接。
  3. 对象树标题行增加「定位当前标签」按钮：当活动标签是表/视图/ER 且其来源与对象树当前范围一致时，展开数据库/模式/对象分类，滚动到活动对象并高亮；活动对象不存在时显示状态提示，不改变当前连接/库/模式。
  4. 对象树标题行增加「全部折叠」：收起所有对象分类、表字段展开和数据库/模式节点（保留当前库/模式的选中状态）；连接侧栏增加「全部折叠」收起所有分组；侧栏分组折叠状态写入 localStorage，重启恢复（对象分类折叠保持会话内即可）。
  5. 以上按钮只消费现有状态（`tabOrder`、`activeWorkspaceTabId`、`workspaceTabViews`、`selectedId`、`activeDatabase`、`activeSchema`、`tables`、`activeTable`），不新增后端请求、不调用 `switch_database`/`switch_schema`。
- **验收标准**：
  - [ ] 打开多个连接和多个标签后，点侧栏「有打开标签」只显示有查询/表/ER 标签的连接；切换回「全部」恢复；搜索与过滤组合正确，分组计数同步。
  - [ ] 在当前标签为 `orders`（表）时点对象树「定位当前标签」：自动展开对应数据库/模式/「表」分类，滚动到 `orders` 并高亮；不改变当前 SQL 文本、事务或查询结果。
  - [ ] 活动标签来源与对象树当前范围不一致时，点定位按钮只显示明确提示（如「当前对象树范围与活动标签不一致」），不静默切换数据库/模式。
  - [ ] 点侧栏「全部折叠」后所有分组收起；点对象树「全部折叠」后所有分类/字段收起；重新启动后侧栏分组折叠状态保持，对象分类状态至少在当前会话内保持。
  - [ ] 20 个连接、300 个对象的测试数据下，定位/折叠/过滤操作无明显卡顿，不新增网络/数据库请求；Redis 连接下对象树定位按钮隐藏或禁用并给出原因。
- **范围**：
  - 前端：`src/App.vue`（过滤状态、定位状态、折叠状态、localStorage）、`src/components/ObjectBrowser.vue`（暴露定位/折叠方法或 prop nonce、滚动）、`src/components/ObjectCategoryGroups.vue`（展开分类/字段、滚动到对象）、`src/components/Icon.vue`（如需要 `target`/`collapse` 图标）；`src/styles.css`。
  - 后端/驱动：无。
- **风险 / 备注**：
  - 不要为了“定位”自动切换数据库/模式；这会影响连接上下文和未提交事务。范围不一致时宁可只提示。
  - 对象树分类当前默认收起；定位需要先展开父级，再 `nextTick` 后滚动；要避免多次展开导致滚动抖动。
  - 侧栏「有打开标签」只根据标签来源计算，不代表连接在线；文案不要写「已连接」。
  - 收藏连接已有 B-26 规划，本项不实现星标，避免重复。
  - 对象树行数大时 `scrollIntoView({ block: "nearest" })` 即可，不要引入虚拟列表（属于更大改造）。

---

### PM-04 重新打开关闭的标签（Undo Close）

- **优先级**：P1
- **用户问题 / 场景**：
  - 误按 `⌘/Ctrl+W` 或点错关闭按钮后，查询 SQL、表数据页、ER 图就消失；查询内容虽然可能已持久化，但表/ER 标签和当前筛选/结构模式无法恢复。
  - DBX 有「重新打开关闭的标签」和最近关闭标签对话框；Navicat/DBeaver 也有丰富的标签恢复/管理能力。mydb 目前没有。
- **期望行为**：
  1. 维护**会话内**的关闭栈：最多 10 个最近关闭的标签快照，LIFO；关闭查询/表/ER 标签时记录；Redis 控制台标签可选记录（若无状态可忽略）。
  2. 文件菜单新增「重新打开关闭的标签」，快捷键 `Shift+Mod+T`（macOS `⇧⌘T`）；栈空时禁用并显示原因。
  3. 重新打开时：
     - 查询标签：恢复标题、SQL、连接 + 库/模式来源、文件绑定（若原标签绑定文件，恢复路径和未保存状态；路径只存在内存，不写 localStorage）、不恢复事务（如果原标签有未提交事务，按普通新标签打开，并在状态栏提示「事务未恢复」）。
     - 表/视图标签：恢复连接 + 库/模式 + 对象 + 数据/结构模式（结构模式可恢复索引高亮则恢复）；重新校验连接和对象，对象不存在时提示并不打开。
     - ER 标签：恢复连接 + 库/模式，重新挂载画布。
  4. 同一标签不要重复入栈；关闭后重新打开再关闭，应作为新的关闭记录；不要恢复已删除连接的标签（入栈记录可保留，但执行时跳过并提示）。
  5. 关闭栈不写入 localStorage；应用重启后清空；不记录完整结果集、不记录事务、不记录密码。
- **验收标准**：
  - [ ] 关闭一个查询标签后按 `⇧⌘T`，标签恢复，SQL、标题、连接 + 库/模式来源一致；连续按可依次恢复最近关闭的多个标签（最多 10 个）。
  - [ ] 关闭一个表标签（数据模式）再恢复，回到同一表的数据页；关闭结构模式标签再恢复，回到结构页并可恢复索引高亮；关闭 ER 标签再恢复，画布重新出现。
  - [ ] 文件菜单「重新打开关闭的标签」在栈空时置灰并有原因；有记录时点击与快捷键行为一致。
  - [ ] 删除来源连接后恢复该标签不会崩溃或误连其他连接：显示「来源连接已删除」，标签不打开。
  - [ ] 原标签处于手动事务时关闭后恢复，不自动开启事务、不提交/回滚原事务；状态栏有「事务未恢复」提示；应用重启后关闭栈为空。
- **范围**：
  - 前端：`src/App.vue`（关闭入口统一入栈、恢复函数、菜单动作）、新增 `src/lib/closed-tabs.ts`（快照类型、LIFO、去重、数量上限、纯函数）；复用现有 `openTableTabAt`、查询标签创建、ER 标签创建；`AppMenuBar` 无需特殊逻辑。
  - 后端/驱动：无。
- **风险 / 备注**：
  - 关闭文件绑定标签已有「保存/不保存/取消」流程；只有确认关闭后才入栈，取消不入栈。
  - 事务回滚是关闭流程的现有行为；恢复时不要复用原 `tab.id` 的事务条目，避免状态串线。
  - 表/ER 标签可能指向已删除对象；恢复前用现有 `tables`/`refreshTables` 校验，避免把错误弹窗当成恢复成功。
  - 不持久化关闭栈，避免 SQL 内容和文件路径长期留在 localStorage；这与现有“查询标签持久化、结果不持久化”的边界一致。
  - 与 B-27 的“关闭其他/全部”未来交互：关闭多个标签时可按关闭顺序批量入栈，但本项先只覆盖现有单标签关闭路径。

---

### PM-05 状态栏上下文与执行反馈升级

- **优先级**：P1
- **用户问题 / 场景**：
  - 当前状态栏只显示驱动/地址/库模式/结果行数/耗时；用户切到表标签后看不出“当前活动标签是哪个、来源是什么”，也不知道事务、只读、执行中、结果截断等状态。
  - 连接状态只在侧栏 footer 显示；查询执行中只有编辑器按钮变“执行中”，状态栏没有统一反馈。
  - 窄窗口时状态信息没有优先级，容易被挤掉或换行。
- **期望行为**：
  1. 状态栏分三段：左侧状态消息（现有 `status`，可省略号），右侧上下文段按优先级排列：只读标记、连接状态（点 + 文案）、活动标签/来源、事务状态、结果摘要、执行中。
  2. 活动标签：
     - 查询标签：显示 `查询 · 连接名 · 库/模式`；
     - 表/视图：显示 `表 · 连接名 · 库/模式`（结构模式加「结构」）；
     - ER：显示 `ER · 连接名 · 库/模式`；
     - 零标签：显示当前选中连接 + 当前库/模式，或「未连接」。
  3. 事务：活动查询标签 `inTransaction` 时显示「事务中 · 未提交」徽标（琥珀色）；切到其他标签时状态栏跟随活动标签，不误显示其他标签事务。
  4. 结果：仅当活动查询标签有结果时显示；查询结果显示「N 行 · X ms」，截断追加「已截断」；DML 显示「影响 N 行 · X ms」；无结果不显示 0。
  5. 执行中：活动标签 `running` 时显示 spinner + 「执行中…」；不伪造进度百分比。连接失败时显示「连接失败」；只读连接显示「只读」。
  6. 窄窗口优先级：≥1280px 显示全部；1100–1279px 隐藏结果摘要的耗时或合并来源；<1100px 优先保留活动标签/连接状态/只读，其余进 tooltip；960px 最小窗口不换行、不横向滚动。
- **验收标准**：
  - [ ] 在查询、表、ER、零标签四种状态间切换，状态栏的活动标签/来源立即更新，不残留上一个标签的结果行数/耗时。
  - [ ] 查询执行中显示 spinner +「执行中…」；完成后显示「N 行 · X ms」，截断结果追加「已截断」；DML 显示影响行数；失败时显示「查询失败」而非旧结果。
  - [ ] 手动事务标签显示「事务中 · 未提交」；切换到非事务标签后该徽标消失；提交/回滚后消失。
  - [ ] 只读连接、连接失败、Redis 连接（显示 dbNN）在状态栏有明确标识；零标签时显示当前连接与库/模式，不显示旧查询结果。
  - [ ] 960px 宽度下状态栏不换行、不遮挡内容；被隐藏的字段可通过 title/tooltip 查看；所有更新不触发额外后端请求。
- **范围**：
  - 前端：`src/components/StatusBar.vue`（新增 props、分段、CSS 优先级）、`src/App.vue`（传入 active tab kind/label/source、connectionStatus、inTransaction、running、readOnly）、`src/styles.css`（媒体查询/省略）。
  - 后端/驱动：无。
- **风险 / 备注**：
  - 当前 `result` 已按活动查询标签计算，切到表/ER 时为 null；不要为了“看起来丰富”去缓存并显示上一个标签结果，避免误导。
  - 连接状态只有当前选中连接有实时值；其他连接不要显示“已连接”。状态栏只显示当前连接状态。
  - 结果摘要与现有面板标题重复不是问题：状态栏负责全局上下文，面板标题负责结果局部信息；文案要保持一致。
  - 若未来支持结果多标签/多结果集（B-07），状态栏需要显示活动结果集序号；本项先按单结果设计，留出 `resultLabel` prop。

---

### PM-06 SQL 编辑器基础命令接入菜单

- **优先级**：P1
- **用户问题 / 场景**：
  - CodeMirror 的查找/替换（`Mod+F`）、注释（`Mod+/`）、转到行（`Mod+Alt+G`）其实已经由 `basicSetup` 提供，但用户在菜单里完全看不到，只能凭记忆或试快捷键。
  - Navicat/DBeaver 都把查找、注释、转到行放在 Edit/编辑器菜单中；mydb 的「编辑」菜单只有撤销/剪贴板，缺少编辑器语义。
- **期望行为**：
  1. 编辑菜单在现有撤销/重做/剪切/复制/粘贴/全选之后增加：
     - 「查找和替换…」`Mod+F`
     - 「查找下一个」`F3` / `Mod+G`
     - 「查找上一个」`Shift+F3` / `Shift+Mod+G`
     - 「转到行…」`Mod+Alt+G`
     - 「注释/取消注释」`Mod+/`
  2. 菜单项只在活动标签为查询标签时可用；其他状态禁用并说明「请先打开查询标签」。
  3. 执行菜单项时，先把焦点放到活动 SQL 编辑器，再调用 CodeMirror 命令；不改变 SQL 内容（注释命令除外）、不执行 SQL、不写执行历史。
  4. 保留现有 CodeMirror 快捷键行为；菜单与快捷键走同一命令，不出现两套查找面板或双重注释。
  5. 「格式化 SQL」「跳转声明」「悬停信息」不在本项范围；后续可单独立项，避免第一版引入格式化算法和方言差异。
- **验收标准**：
  - [ ] 打开查询标签，编辑菜单出现查找和替换、查找下一个/上一个、转到行、注释/取消注释；切到表/ER/空工作区时这些项置灰并显示原因。
  - [ ] 点「查找和替换…」或按 `Mod+F` 都打开同一个 CodeMirror 查找面板，Esc 关闭；不会叠出两个面板。
  - [ ] 选中多行或在某行按「注释/取消注释」后，按当前 SQL 方言生成正确的行注释（MySQL/PostgreSQL/SQLite 均为 `-- `），再按一次恢复；不执行查询、不改变结果。
  - [ ] 点「转到行…」输入合法行号后光标跳到对应行；输入 0、负数或超过总行数时显示提示且不移动；不影响编辑器外部输入框。
  - [ ] 在查找面板内按方向键/Enter 不触发菜单或全局快捷键；关闭面板后焦点回到编辑器，继续输入正常。
- **范围**：
  - 前端：`src/components/SqlEditor.vue`（`defineExpose` 暴露 `focus`/`openSearch`/`findNext`/`findPrevious`/`toggleComment`/`gotoLine`，或统一 `runEditorCommand`）、`src/components/QueryWorkspace.vue`（转发命令）、`src/App.vue`（Edit 菜单项、可用状态、调用 QueryWorkspace）、`package.json`（显式加入 `@codemirror/search`、`@codemirror/commands`，当前为传递依赖）。
  - 后端/驱动：无。
- **风险 / 备注**：
  - 不要重复实现查找 UI；CodeMirror 的 `searchPanel` 已存在，直接调用 `openSearchPanel`/`findNext`/`findPrevious`/`gotoLine`。
  - `toggleComment` 依赖语言的 commentTokens；需在 MySQL/PostgreSQL/SQLite 方言下各验证一次。若某方言缺失，退化为 `-- ` 行注释实现，不要静默失败。
  - WebView 可能拦截 `Mod+F`；当前 CodeMirror 已能处理，菜单调用要复用同一命令，避免与 WebView 查找冲突。
  - 菜单动作目标应是“活动查询标签”，而不是最后聚焦的任意输入框；这与现有「运行」按钮的语义一致。

---

### PM-07 快捷键速查与冲突检查（轻量版）

- **优先级**：P2
- **用户问题 / 场景**：
  - 快捷键分散在 `App.vue`、Rust 菜单、CodeMirror、结果网格和表数据页中；用户没有统一入口查看，新增快捷键可能冲突。
  - Navicat 有 Hot Keys 手册，DBeaver 有 Shortcuts 文档和 Keys 设置，DBX 有快捷键注册表和设置内搜索；mydb 目前只能看菜单里零散显示的快捷键。
- **期望行为**：
  1. 帮助菜单新增「键盘快捷键」，打开一个只读速查弹窗（`F1` 可选；不要抢占现有 F1 行为，若无冲突可绑定）。
  2. 弹窗按「全局 / 标签 / SQL 编辑器 / 结果网格 / 表数据 / Redis」分组列出：动作、快捷键（按平台显示 `⌘` / `Ctrl`）、作用条件；支持搜索。
  3. 数据来源优先使用共享菜单/动作注册表与 `src/lib/shortcuts.ts` 的静态定义；CodeMirror 的默认键位（查找、注释、转到行）纳入编辑器分组；不伪造未实现的快捷键。
  4. 开发模式下检查重复键位（同 scope 内同一组合绑定两个动作）并在控制台给出警告；运行时发现重复时在弹窗内标注，不阻止操作。
  5. 本项只做“查看/检查”，不做自定义快捷键；自定义留给 B-28 设置中心。
- **验收标准**：
  - [ ] 帮助 → 键盘快捷键打开弹窗，列出当前实际可用的全局、标签、编辑器、结果网格、表数据、Redis 快捷键；按平台显示正确修饰键。
  - [ ] 搜索「注释」「查找」「运行」「标签」能找到对应条目；清空搜索恢复分组。
  - [ ] 弹窗内不出现未实现的动作或旧快捷键；菜单显示的快捷键与速查表一致。
  - [ ] 人为制造同一 scope 的重复绑定，开发控制台出现警告，弹窗对应条目有标记；正常构建无报错。
  - [ ] Esc 关闭弹窗并恢复焦点；弹窗打开时不触发编辑器快捷键、不执行 SQL。
- **范围**：
  - 前端：新增 `src/components/ShortcutHelpDialog.vue`、`src/lib/shortcuts.ts`（静态定义 + 平台格式化 + 重复检测）；`src/App.vue`（Help 菜单入口、弹窗状态）；`src/components/Icon.vue`（如需 `keyboard` 图标）。
  - 后端/驱动：无。
- **风险 / 备注**：
  - 快捷键注册表容易与真实 handler 脱节；第一版宁可少列，也不要列不存在的快捷键。每条最好能用现有 handler 或 CodeMirror 命令名对应。
  - 原生菜单加速键在 macOS 由 Rust 处理；速查表要覆盖原生菜单可见项，否则用户会以为 F1 表里没有。
  - 不在第一版做录制/修改，避免把设置存储、冲突解决、平台差异一次性做完。

---

## 4. 首批建议

### 4.1 选择原则

- 优先解决“主界面和菜单栏一致性”，不做需要重写布局/驱动/插件系统的大工程。
- 选择日频高、失败代价低、可与现有架构共存、能独立验收的条目。
- 第一批完成后，用户应能明显感到：菜单可信、键盘可用、误关可救、状态可读。

### 4.2 建议顺序

| 顺序 | 需求 | 影响 | 成本 | 依赖 | 说明 |
|---|---|---|---|---|---|
| 1 | PM-01（阶段 1a：前端菜单模型 + 信息架构 + 语义修正） | 高 | 中 | 无 | 先建立单一菜单模型，修掉重复/退出语义/禁用状态不一致；暂不接原生菜单同步 |
| 2 | PM-02 菜单键盘导航 | 中高 | 低 | PM-01a 的菜单模型 | 纯前端、无数据风险，直接补齐菜单可用性 |
| 3 | PM-04 重新打开关闭的标签 | 中高 | 低 | PM-01a 的菜单槽位 | 独立于原生菜单；日常误关恢复价值高 |
| 4 | PM-01（阶段 1b：原生菜单同源 + 状态同步） | 高 | 中 | PM-01a、PM-04（重新打开项） | 在菜单项稳定后再接 Rust，避免反复改原生菜单 |
| 5 | PM-05 状态栏升级 | 中 | 低 | 无（可与 1b 并行） | 数据都已在 App 中，主要是展示与布局 |

**建议先做的 3 条（如果只能排 3 条）**：

1. **PM-01 阶段 1a**：菜单是用户理解产品的目录；当前重复项、假退出、原生/窗口不一致会直接损害信任。先做前端菜单模型和信息架构，成本可控。
2. **PM-02**：在 PM-01a 的模型上补键盘导航，纯前端、低风险，完成后菜单才真正“像桌面应用”。
3. **PM-04**：独立、日常高频、失败可逆；能立刻减少误关标签的损失，也是菜单完整性的自然补充。

PM-05 状态栏可在 PM-01b 期间并行，不阻塞；PM-03 导航联动成本略高，建议放在第一批之后单独排期；PM-06/PM-07 作为“菜单/快捷键完整性”的后续小步。

### 4.3 实施注意事项

- 先统一 `appActions` 的 id 和 `appMenus` 的菜单结构，再动 Rust 原生菜单；否则两边会同时改、同时漂移。
- 第一批不做新依赖、不改数据库驱动、不改凭据存储；所有改动应能用现有 `npm run check` 和 `cargo test` 验证（纯前端项至少 `npm run check`）。
- 每个需求的验收要在 SQLite / MySQL / PostgreSQL / Redis 四种连接状态下各走一遍；只读连接、无连接、Redis 连接、零标签是菜单禁用逻辑的高风险场景。
- 发布仍按现有规则：普通需求只改代码、跑检查、dev 调试；不自动提交/打包/覆盖 app；版本号只在用户明确要求时提升。

---

## 5. 明确不做（本批边界）

| 不纳入项 | 原因 | 已有归属 / 后续 |
|---|---|---|
| 重新引入整行应用工具栏（AppToolbar） | 与 2026-11-24 已完成的「移除应用工具栏整行」决策冲突；BACKLOG B-32 与当前代码不一致，需先由用户重新决策 | B-32 待重新校准，不在本文件 |
| 全局命令面板 / Quick Open | 已有 B-25 完整规格（入口、分组、最近使用、IME、验收），本文件重写会重复且容易分叉 | B-25 |
| 标签拖拽排序、固定、分组、完整溢出菜单、关闭其他/右侧/全部 | 已有 B-27；本文件只补「重新打开关闭的标签」这一项 | B-27 |
| 设置中心、主题切换、字体/密度/快捷键自定义 | 已有 B-18 / B-28；本文件只做菜单、状态栏和快捷键速查 | B-18/B-28 |
| 主工作区持久 Home 标签、最近连接/收藏连接/最近对象 | 已有 B-26；本文件不重复 | B-26 |
| 查询取消/超时、多语句逐条结果、多结果集/Pin/结果位置 | 已有 B-05 / B-07；本文件状态栏只显示现有单结果状态 | B-05/B-07 |
| 分屏、多窗口、标签分离/停靠、跨窗口状态同步 | 属于布局架构升级，成本高；DBX/DBeaver 的对应能力不适合直接移植 | B-29 + 独立架构评估 |
| 菜单/工具栏显示项自定义、快捷键录制 | 依赖设置中心和稳定注册表；第一版只做速查/冲突提示 | B-38 / B-28 |
| 全局跨连接对象搜索、数据库全文搜索、搜索历史持久化 | 需要元数据缓存/索引/后台任务，B-25 也只先做当前连接对象 | B-25 后续 |
| 新建视图/存储过程/函数/触发器、备份/还原、数据传输、报表/计划任务、用户/权限/服务器监视 | Navicat/DBeaver 的重型能力，需要独立架构和驱动能力；不属于主界面/菜单一致性第一批 | B-33～B-37 |
| AI 助手、MCP、插件中心、驱动商店、Docker/Web 登录 | 与 mydb 轻量本地客户端定位冲突；涉及外部服务、凭据和供应链风险 | 长期评估 |
| Eclipse 式可停靠工作区、Projects/Workspaces/Perspectives | 需要重写主窗口布局模型；mydb 的固定四区 + 可调宽度更稳 | 不做 |
| 新增数据库驱动（达梦/金仓/神通/JDBC 桥接） | 需要 JVM/sidecar/驱动分发，和纯 Rust 驱动路线冲突 | BACKLOG 长期探索 |
| 引入新原生依赖或第三方 UI/图标库 | 增加打包体积、平台差异和供应链风险；现有 Icon/SVG 足够 | 不做 |
| 复制 DBX/Navicat/DBeaver 的品牌图标、截图、插画、文案 | 第三方资源不可复制；只参考交互组织 | 不做 |

---

## 6. 来源清单

> 访问日期：2026-11-24。DBX 引用 `main` 分支提交 `0fbd067138d5e21523b0b35b74d471277246b520`。

### DBX（11）

1. DBX 仓库主页（Apache-2.0）：<https://github.com/t8y2/dbx>
2. `apps/desktop/src/App.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/App.vue>
3. `apps/desktop/src/components/layout/AppToolbar.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/layout/AppToolbar.vue>
4. `apps/desktop/src/components/layout/AppTabBar.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/layout/AppTabBar.vue>
5. `apps/desktop/src/components/layout/AppSidebar.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/layout/AppSidebar.vue>
6. `apps/desktop/src/components/quick-open/QuickOpenDialog.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/quick-open/QuickOpenDialog.vue>
7. `apps/desktop/src/components/layout/WelcomeScreen.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/layout/WelcomeScreen.vue>
8. `apps/desktop/src/lib/settings/toolbarCustomization.ts`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/lib/settings/toolbarCustomization.ts>
9. `apps/desktop/src/components/editor/EditorSettingsDialog.vue`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/components/editor/EditorSettingsDialog.vue>
10. `apps/desktop/src/lib/editor/shortcutRegistry.ts`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/lib/editor/shortcutRegistry.ts>
11. `apps/desktop/src/lib/tabs/tabMenu.ts`：<https://github.com/t8y2/dbx/blob/main/apps/desktop/src/lib/tabs/tabMenu.ts>

### Navicat 18 官方在线手册（9）

12. Main UI / Main Window：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/main_window.html>
13. Workspace Organization & Customization：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/organization_customization.html>
14. Query Editor：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/query_editor.html>
15. Query Results：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/query_results.html>
16. Hot Keys：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/hot_keys.html>
17. Configurations / Options：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/options_settings.html>
18. Favorites：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/favorites.html>
19. Search & Filter Tools：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/search_filter.html>
20. Console：<https://www.navicat.com/manual/online_manual/en/navicat_18/win_manual/console.html>

### DBeaver 官方文档与官方讨论（8）

21. Application Window Overview：<https://dbeaver.com/docs/dbeaver/Application-Window-Overview/>
22. Database Navigator / Connections：<https://dbeaver.com/docs/dbeaver/Database-Navigator/>
23. SQL Editor：<https://dbeaver.com/docs/dbeaver/SQL-Editor/>
24. Toolbar Customization：<https://dbeaver.com/docs/dbeaver/Toolbar-Customization/>
25. Shortcuts：<https://dbeaver.com/docs/dbeaver/Shortcuts/>
26. Search：<https://dbeaver.com/docs/dbeaver/Search-Tool/>
27. DBeaver 官方讨论「Command Palette?」：<https://github.com/orgs/dbeaver/discussions/35599>
28. DBeaver 官方讨论「Add a Ctrl+P palate to switch between tables, files, etc. like in VS Code」：<https://github.com/orgs/dbeaver/discussions/19538>

---

## 7. 附：目标菜单结构（建议稿）

> 该表是 PM-01 的目标信息架构，不代表当前实现；「本批」列标出由哪个需求落地，未标需求的是现有动作的对位整理。

| 顶层 | 菜单项 | 快捷键 | 本批 | 备注 |
|---|---|---|---|---|
| 文件 | 新建连接 | `⌘/Ctrl+N` | 现有 | |
| 文件 | 新建查询 | `⌘/Ctrl+T` | 现有 | |
| 文件 | 打开 SQL 文件… | `⌘/Ctrl+O` | 现有 | |
| 文件 | 保存 SQL | `⌘/Ctrl+S` | 现有 | |
| 文件 | 另存为… | `⇧⌘/Ctrl+Shift+S` | 现有 | |
| 文件 | 重新打开关闭的标签 | `⇧⌘/Ctrl+Shift+T` | PM-04 | 栈空时禁用 |
| 文件 | 导入数据… | — | 现有 | 从工具菜单去重 |
| 文件 | 导出结果… | — | 现有 | 仅查询结果可用 |
| 文件 | 关闭标签 | `⌘/Ctrl+W` | 现有 | |
| 文件 | 关闭窗口 | `⇧⌘/Ctrl+Shift+W` | 现有 | 只关窗，不退出 |
| 文件 | 退出 mydb（仅 Windows/Linux） | — | PM-01 | macOS 走 app menu Quit；调用 app quit |
| 编辑 | 撤销 / 重做 | `⌘Z` / `⇧⌘Z` | 现有 | |
| 编辑 | 剪切 / 复制 / 粘贴 / 全选 | 系统 | 现有 | |
| 编辑 | 查找和替换… | `⌘/Ctrl+F` | PM-06 | CodeMirror 搜索面板 |
| 编辑 | 查找下一个 / 上一个 | `F3` / `⇧F3`、`⌘G` / `⇧⌘G` | PM-06 | 与 CodeMirror 一致 |
| 编辑 | 转到行… | `⌥⌘G` / `Ctrl+Alt+G` | PM-06 | CodeMirror gotoLine |
| 编辑 | 注释/取消注释 | `⌘/Ctrl+/` | PM-06 | 按方言 |
| 视图 | 刷新对象 | `⌘/Ctrl+R` | 现有 | |
| 视图 | 执行历史 | `⌘/Ctrl+Y` | 现有 | |
| 视图 | 连接侧栏 [✓] | `⌘/Ctrl+B` | PM-01/02 | 勾选 |
| 视图 | 对象树 [✓] | `⇧⌘/Ctrl+Shift+B` | PM-01/02 | 勾选 |
| 视图 | 结果面板 [✓] | `⌘/Ctrl+J` | PM-01/02 | 勾选 |
| 视图 | 右侧面板 [✓] | `⇧⌘/Ctrl+Shift+H` | PM-01/02 | 勾选 |
| 视图 | 定位当前标签 | — | PM-03 | 视情况加快捷键 |
| 视图 | 重置布局 | — | 现有 | |
| 视图 | 切换全屏 | 系统 | PM-01 | 只保留一处 |
| 工具 | ER 图 | — | 现有 | |
| 工具 | 结构同步… | — | 现有 | |
| 工具 | 数据同步… | — | 现有 | |
| 工具 | 编辑当前连接… | — | 现有/PM-01 | 原生菜单补齐 |
| 工具 | 关闭当前连接 | — | 现有/PM-01 | 原生菜单补齐 |
| 窗口 | 最小化 | `⌘/Ctrl+M` | 现有 | |
| 窗口 | 前置全部窗口 | — | 现有（仅原生） | 原生菜单已有；窗口内菜单可选 |
| 帮助 | 关于 mydb | — | 现有 | |
| 帮助 | 键盘快捷键 | 可选 `F1` | PM-07 | 只读速查 |
| 帮助 | GitHub 项目主页 | — | 现有 | 原生菜单可补 |

**注**：macOS app menu 继续保留「关于 mydb / 服务 / 隐藏 / 隐藏其他 / 显示全部 / 退出 mydb」；窗口内菜单不再放「退出 mydb」。