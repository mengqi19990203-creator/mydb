# mydb 产品需求（2026-11-26 第三批：G-01～G-05）

> 文档编号：PM-REQUIREMENTS-2026-11-26-batch5
> 批次定位：E-01～E-05、F-01～F-05 已于 2026-11-26 全部完成（状态见 `docs/BACKLOG.md`），本批**不重复**已完成的查找、行详情、撤销 / 重做、Excel 导入、对象置顶、列冻结、结果筛选、外键跳转、剪贴板粘贴和执行范围控制。本批继续研究 DBX、DBeaver、Navicat 仍然高频、但 mydb 尚未覆盖的能力。
> 状态：G-01～G-05 已于 2026-11-26 全部完成并验收。
> 竞品边界：DBX（Apache-2.0）与 DBeaver（Apache-2.0）只参考公开文档与交互；Navicat 为专有软件，只参考公开产品页 / 帮助文档中的功能描述。不复制源码、品牌图标、截图或专有文案。mydb 保持独立实现与 Apache-2.0 许可。

## 0. 本批结论

### 0.1 一句话目标

让结果网格从「能看」提升到「能选、能算、能画」；让表数据按用户看到的筛选 / 排序导出；让常用表视图可以命名保存和复用；让 SQL 在运行前得到一次高置信度、不阻塞执行的静态自检。

### 0.2 需求清单（按推荐实施顺序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 主要依赖 | 关系 / 去重 |
|---|---|---|---|---|---|---|
| 1 | G-01 | 结果集矩形选区与统计摘要 | P0 | 1.5–2 个开发会话 | `ResultGrid.vue`、F-02 筛选、E-01 查找、虚拟滚动 | 新增；不改表数据行选择、不改导出范围 |
| 2 | G-02 | 表数据按筛选 / 排序导出全部匹配行（含选中行） | P0 | 1.5 个开发会话 | `TableDataView.vue`、`export_object` / 驱动、现有 CSV / JSON / XLSX / SQL 写入能力 | 新增导出范围；不改变对象右键导出默认语义 |
| 3 | G-03 | 结果集图表（柱 / 线 / 饼，聚合与 Top N） | P1 | 1.5–2 个开发会话 | `QueryWorkspace.vue`、`ResultGrid.vue`、F-02 筛选 | 新增；不引入图表库，不做 BI 仪表盘 |
| 4 | G-04 | 表数据列显示与表视图配置（命名保存 / 应用） | P1 | 1.5 个开发会话 | `TableDataView.vue`、F-01 冻结列、localStorage | 新增；保留现有分页记忆与冻结列存储 |
| 5 | G-05 | SQL 静态检查（按需、高置信度、不阻塞执行） | P2 | 2 个开发会话 | `SqlEditor.vue`、`QueryWorkspace.vue`、现有元数据与危险 SQL 检测 | 新增；完整语义诊断 / 实时波浪线不在本批 |

> 「1 个开发会话」指一次连续实现：编码、`npm run check`、涉及 Rust 时 `cargo test --manifest-path src-tauri/Cargo.toml`，以及本文列出的可自动化验收项；真实 MySQL / PostgreSQL 手工回归可另排。

### 0.3 推荐实施顺序与理由

**实施顺序：G-01 → G-02 → G-03 → G-04 → G-05。**

- **G-01 最先做**：每个查询结果都受益；纯前端、无数据库写入、无新依赖；先建立「矩形选区 + 单元格取值范围 + 统计格式化」三块共享能力，G-03 的聚合、G-02 的选中行导出都可以复用。
- **G-02 第二**：这是当前最容易产生错误预期的缺口——表数据明明筛选出 2,000 行，工具栏「导出当前页」只导出 100 行，对象右键导出又会忽略筛选。它需要后端流式导出与排序一致性，但和 G-01 完全解耦，可并行。
- **G-03 第三**：图表依赖稳定的筛选行集与聚合语义；放在 G-01 / G-02 之后，可以直接复用统计 helper，避免另写一套数值判断。先做单图，不做多图仪表盘。
- **G-04 第四**：表数据隐藏列 / 列宽 / 筛选 / 排序 / 冻结的命名配置，是「重复分析同一张表」的高频摩擦；放在 G-02 之后，导出对话框可以直接沿用「当前可见列」语义。
- **G-05 最后**：静态检查有误报风险，必须限定为按需、高置信度、只警告不阻塞；放在最后即使缩减范围也不影响前四项交付。

若只能做一项，做 **G-01**；若只能做两项，做 **G-01 + G-02**。

### 0.4 竞品研究与差距结论（2026-11-26，第三批）

调研对象与边界：DBX（Apache-2.0，公开文档）、DBeaver（Apache-2.0，公开 wiki / 官方文档）、Navicat（专有，只参考公开产品页 / 帮助文档 / 手册中的功能描述）。只比较用户可见能力与交互，不复制实现。

#### 4.1 DBX（重点参考）

- **数据表格选择与摘要**：支持单元格、矩形区域、整行 / 多行选择；选中后可查看数量、求和等选择摘要；复制支持 TSV / CSV / JSON / Markdown / SQL INSERT / UPDATE。
- **转置视图与固定结果**：转置视图交换行列，适合单行多字段记录；查询结果可以保留多次运行，或固定需要对比的结果。
- **列布局**：列宽、自动适配、拖拽重排、左 / 右冻结、显示 / 隐藏 / 搜索列、隐藏全 NULL 列，布局可跨会话持久化。
- **查询编辑器**：对未解析的表、列、别名给出语义诊断；`Ctrl/Cmd+Click` 打开表或定位对象；执行前可预览 UPDATE / INSERT / DELETE 的影响行。
- **导出与筛选**：数据库侧 WHERE / ORDER BY、客户端当前页排序、本地值筛选；导出 CSV / JSON / Markdown / XLSX / SQL INSERT / UPDATE。
- 来源：<https://dbxio.com/en/docs/data-grid>、<https://dbxio.com/cn/docs/data-grid>、<https://dbxio.com/en/docs/query-editor>

#### 4.2 DBeaver（公开 wiki / 官方文档）

- **Data Editor**：Grid / Text / Record 视图，Team Edition 文档另有 Chart 视图；右侧有 Value、References、Metadata、Calc、Grouping 面板；顶部有导航前进 / 后退历史。
- **Calc 面板**：对选中单元格计算 Count、Count distinct、Count nulls、Sum、Average、Minimum、Maximum、Median、Mode；支持按列分组。
- **Grouping 面板**：拖拽列生成 GROUP BY 查询；Chart 视图支持 Bar / Line / Pie，并可配置 X / Y、聚合函数、样本行数。
- **数据导出**：从查询结果或 Data Editor 导出时可选择 Use fetched rows、Selected rows only、Selected columns only；表级导出需要自定义 SQL 才能带 WHERE。
- 来源：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>、<https://dbeaver.com/docs/dbeaver/Calc-Panel/>、<https://dbeaver.com/docs/dbeaver/26.2/Data-export/>、<https://dbeaver.com/docs/team-edition/25.3/web/Managing-Charts/>

#### 4.3 Navicat（公开产品页 / 帮助文档 / 手册）

- **数据编辑器**：Grid View / Form View；Filter & Sort 面板；可把筛选、排序、列显示设置保存为命名 Profile，并按用途切换。
- **Data Profiling**：列质量（distinct / unique / NULL / 非 NULL 比例）、列分布直方图、列统计（Count、Distinct、Unique、Empty String、Min、Max、Average、Standard Deviation、NULL 数）。
- **图表**：20+ 图表类型，可从表 / 视图 / 查询结果直接可视化。
- **筛选导出**：应用筛选后可选择「Current x Records」/「Data in This Page」导出，而不是整表。
- 来源：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>、<https://help.navicat.com/hc/en-us/articles/217806188-How-do-I-export-filtered-records-after-applying-a-filter-to-a-table-or-collection>、<https://help.navicat.com/hc/en-us/articles/46711325532697-How-to-use-the-Data-Profiling-feature-to-analyze-data>、<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

#### 4.4 差距表与本批决策

| 能力 | 竞品参考 | mydb 现状（2026-11-26 代码核对） | 本批决策 |
|---|---|---|---|
| 结果集矩形选区 / 统计摘要 | DBX 选区摘要；DBeaver Calc；Navicat Data Profiling | 只有单元格右键复制、整行 / 列名 / 全部复制；没有拖选、键盘扩展、选中计数 / 求和 / 平均 | **G-01**，结果网格先做，表数据行选择保持现状 |
| 表数据按筛选 / 排序导出 | Navicat「Current x Records」；DBeaver「Use fetched rows / Selected rows only」 | 表数据只导出当前页；对象右键导出全部 / 指定行数，但不接受筛选与排序；选中行不能导出 | **G-02**，新增导出范围与流式后端 |
| 结果集图表 | DBeaver（Team Edition）Chart 视图；Navicat 20+ 图表 | 无图表；只能导出到 Excel 或手写 GROUP BY | **G-03**，单图 MVP，不引入图表库 |
| 表视图配置 | Navicat Profile；DBX 持久化列布局 | 只记住每页行数与冻结列；列宽、筛选、排序、隐藏列不持久；表数据没有隐藏列 | **G-04**，命名视图 + 表数据隐藏列 |
| SQL 静态检查 | DBX 语义诊断；DBeaver / 通用编辑器错误提示 | 只有数据库返回错误后的提示；危险 SQL 确认是执行前唯一静态检查 | **G-05**，按需高置信度检查，不做完整语义层 |
| 结果固定 / 多次运行对比 | DBX Pin result | 多语句有「结果 N」，但每次执行替换上一次；不能固定对比 | 下一批；涉及结果模型与内存上限，本批不做 |
| 转置视图 / 列拖拽重排 | DBX 转置 / 列重排；DBeaver Record 视图 | 行详情已覆盖单行纵向查看；结果网格没有转置、列重排 | 下一批；列重排与表视图配置一起评估，转置单独评估 |
| SQL 对象点击导航 | DBX Ctrl/Cmd+Click；DBeaver 链接 | 有补全和全局对象搜索，但不能从 SQL 文本打开表 / 字段 | 下一批；可复用 `completionTables`，但需别名解析与导航历史设计 |
| 粘贴覆盖已有行 / 区域粘贴 | DBX / DBeaver / Navicat | F-04 只追加待提交新增行 | 下一批；覆盖写入涉及行定位、主键和无主键限制，风险高于追加 |
| 跨驱动数据传输 / 逐列映射 | DBX 跨连接传输；DBeaver 数据传输 | 同驱动、同名字段、整表复制；无逐列映射与类型转换 | 后续专项；需要类型映射、表达式转换与失败策略 |
| 连接自动重连 | DBX 自动重连 | 连接断开后手动重开 | 后续专项；写语句重试可能重复写入，不能作为小修上线 |
| 对象树 / 标签管理 | DBX 对象置顶；Navicat 对象筛选 | 已有对象搜索、置顶、批量操作、标签拖动 / 固定 / 溢出 / 分屏 / 独立窗口 | 本批不做；当前没有明显高频缺口 |
| 表结构 / DDL 体验 | Navicat 表设计器；DBX 可编辑 DDL | 已有结构页 ALTER、索引 / 外键编辑、新建表设计器、结构同步、DDL 预览 | 本批不做；剩余「现有表可视化整体设计器」价值中等、改动大，另立专项 |

**结论**：mydb 的重能力（结构 / 数据同步、备份、导入、文档、服务器活动、SQL 文件）已经比较完整；当前最影响日常使用的是**网格的分析能力、筛选后的导出、可复用的表视图，以及执行前的 SQL 自检**。本批五项都落在现有网格、表数据、结果面板和 SQL 编辑器上，不新增驱动、不引入外部服务。

### 0.5 与已实现 / 已提案需求的去重

- **E-01 查找**：查找只高亮匹配，不改变可见行；G-01 选区不查找、不跳转，只做范围选择、统计与复制。两者可叠加。
- **E-02 行详情**：行详情一次看一行；G-01 看多行 / 多列块并统计；G-03 看整体分布与趋势。三者互补。
- **E-03 待提交撤销**：G-04 应用视图配置时如遇未提交修改，复用现有 `ensureClean` 守卫；G-01 / G-03 不触碰待提交状态。
- **E-04 Excel 导入**：G-02 是导出范围，方向相反；不改变导入向导。
- **E-05 对象置顶**：与 G 批无关。
- **F-01 冻结列**：G-01 选区只覆盖可见列，冻结列照常显示选择背景；G-04 保存冻结列名，不改写 `mydb.frozenColumns.v1` 的既有语义。
- **F-02 结果筛选**：G-01 统计、G-03 图表都只使用筛选后的行；G-02 表数据导出使用表数据自己的服务端筛选，不是结果筛选。
- **F-03 外键跳转**：与 G 批无关；G-05 不执行 SQL、不跳数据。
- **F-04 剪贴板粘贴**：G-02 导出选中行，不导入；G-04 不改变粘贴新增。
- **F-05 执行范围**：G-05 检查范围复用「当前语句 / 整个标签」语义，但不执行 SQL、不写历史。
- **B-02 查询结果导出**：仍导出当前结果 / 当前筛选行；G-02 是表数据导出，不替代结果导出。
- **B-09 导入向导 / E-04**：G-02 只做导出。
- **B-35 数据传输**：会把数据写入目标库；G-02 只写本地文件，不连接目标。
- **C-03 / D-04 表数据待提交**：G-02 / G-04 不绕过提交缓冲区；G-02 的当前页 / 选中行导出包含待提交 overlay 时必须明确标注。
- **C2-05 XLSX 导出**：G-02 复用现有 XLSX 写入与 200,000 行上限，不重复实现格式。
- **B-16 / D-03 值查看器**：G-01 不替代值查看器；双击仍打开值查看器。
- **B-04 / B-24 信息 Dock、B-26 首页、B-36 报表、B-38 工具栏自定义**：本批不触碰。

### 0.6 本批明确不做（附评估结论）

| 方向 | 为什么不放进本批 |
|---|---|
| 完整 SQL 语义诊断 / 实时波浪线 | 需要方言感知的语义层、别名 / CTE / 子查询解析和元数据失效处理；误报会直接影响编辑体验。G-05 只做按需、高置信度、只警告不阻塞的静态检查；完整诊断另立专项。 |
| 结果固定 / 多次运行对比 | 涉及结果模型从「当前结果」变成「多份快照」、内存上限、标签持久化与差异算法；价值高但改动大，下一批单独立项。 |
| 转置视图 / 列拖拽重排 | 行详情已覆盖单行纵向查看；转置会改变行列语义、右键菜单与导出；列重排还要处理冻结偏移与持久化。下一批与表视图配置一起评估。 |
| 粘贴覆盖已有行 / 区域粘贴 | F-04 只追加新增行；覆盖写入需要逐行定位、主键 / 无主键限制、冲突提示与撤销，风险明显高于追加。 |
| 跨驱动数据传输 / 逐列映射 | 需要类型映射、表达式转换、目标 DDL 生成与失败策略，是独立架构专项；现有同驱动传输不扩展。 |
| 连接自动重连 | 读查询可安全重试，但写语句在断线时可能已在服务端生效；需要按语句类型、事务状态与用户确认设计。 |
| BI 仪表盘 / 多图表联动 | G-03 只做单结果集的单图；多图布局、联动筛选、保存仪表盘属于 BI 级能力，与轻量客户端定位冲突。 |
| 现有表可视化整体设计器 | 现有结构页、索引 / 外键编辑、新建表设计器、结构同步已覆盖大部分；整体设计器的交互与事务安全需要独立评估。 |
| 对象树 / 标签管理增强 | 已有搜索、置顶、批量、拖动 / 固定 / 溢出、分屏、独立窗口；没有足够高频的剩余缺口。 |
| Redis 命名空间树 / 批量键操作 | 需要限流、预览、Cluster 约束；属于 Redis 专项，不混入本批。 |

### 0.7 通用约束（适用于 G 批全部需求）

1. **不新增数据库驱动，不引入外部服务**；G-01 / G-03 / G-04 / G-05 不新增 npm / Rust 依赖；G-02 不新增 crate，复用现有 `csv`、`rust_xlsxwriter`、`sqlx` 与导出写入代码。
2. **不引入 UI / 图表 / 图标库**。G-03 用 SVG 原生绘制；图标优先复用 `src/components/Icon.vue` 现有 `table` / `view` / `structure` / `download` / `check` / `filter` / `pin`，缺失时按现有线性风格补 1–3 个。
3. **组件不得直接访问数据库**；所有数据库操作走 `src/lib/api.ts` 的 `invoke` 封装。G-02 新增一个只读导出命令；G-05 不新增后端命令。
4. **只读连接、视图、无主键表的既有写保护不得绕过**。G-01 / G-03 / G-04 / G-05 全部只读；G-02 只读连接也允许导出，但不写数据库。
5. **所有新增状态都有上限**：选区最多 50,000 个单元格 / 复制 10 MB；统计 / 图表最多分析 10,000 行；图表最多 50 个分类 / 2,000 个点；表视图每个表最多 20 个配置、全局最多 200 张表；SQL 检查最多 100 KB / 200 条问题。超过上限必须提示，不得静默截断或卡死。
6. **不改变现有复制 / 导出语义之外的行为**：G-01 新增选区复制；G-02 显式选择导出范围；G-03 不改 SQL / 结果 / 导出；G-04 只影响表数据列显示与视图配置；G-05 不执行 SQL。
7. **可访问性**：所有新增按钮 / 输入 / 下拉 / 对话框有 `aria-label` 或可读文本，键盘可操作，焦点可见；选区摘要、图表摘要、检查结果计数、导出进度使用 `aria-live` / `role="status"`。
8. **不劫持既有快捷键**：SQL 编辑器的查找 / 复制 / 撤销、输入框内 `⌘/Ctrl+C/V/Z`、表数据编辑、对话框内按键保持原行为。G-01 的复制快捷键只在结果网格获得焦点且存在选区时生效。
9. **代码风格**：Vue / TS 2 空格，Rust 4 空格；完成后运行 `npm run check`；G-02 涉及 Rust 时运行 `cargo test --manifest-path src-tauri/Cargo.toml`；手工验收优先 `examples/demo.db`，MySQL / PostgreSQL 用临时表并清理。
10. **不修改用户数据**：G-01 / G-03 / G-04 / G-05 不写数据库、连接配置、钥匙串或执行历史；G-02 只读取数据库并写本地文件；表视图配置只写 localStorage。

---

## G-01 结果集矩形选区与统计摘要

- **优先级**：P0
- **状态**：已完成（2026-11-26：`grid-selection.ts` / `grid-stats.ts`、鼠标矩形选区、键盘导航 / 全选 / Esc、摘要条、复制选区、统计整列、冻结 / 隐藏 / 筛选联动；headless 验证 6 格拖动、全选 16 格与数值统计）
- **范围**：前端（`src/components/ResultGrid.vue`、新增 `src/lib/grid-selection.ts`、新增 `src/lib/grid-stats.ts`、`src/lib/virtual-rows.ts`、`src/styles.css`）；不涉及后端和数据库。
- **用户问题 / 场景**：查询结果里想算「这几行金额合计多少」「这一列有多少个不同状态、多少 NULL」「选中的 20 个单元格平均值是多少」，现在只能复制到 Excel 或另写一条 SQL；右键只能复制单个单元格、整行或全部结果，不能拖选一块区域；宽表跨行对照时也很难判断自己选中了哪些值。

### 现状核对（基于 2026-11-26 代码）

- `ResultGrid.vue` 有 `visibleColumns`、`filteredRows`、`sortedRows`、虚拟滚动、隐藏列、冻结列、查找、F-02 本地筛选；单元格只有右键菜单和双击值查看器，没有选区模型。
- 单元格模板有 `data-cell="row:col"`，但 `tabindex="-1"`，没有方向键导航；`copyCell` / `copyRow` / `copyColumnName` / `copyAll` 都是即时复制，不维护选择范围。
- 行号列固定 46px；数据行使用 `<tr>` + `<td>`，虚拟滚动只渲染视口附近行；选区必须用行 / 列索引表示，不能依赖 DOM。
- `grid-search.ts` / E-01 查找只做高亮与跳转，不改变行集；F-02 筛选会改变 `filteredRows` / `sortedRows`。

### 期望行为（可验收细节）

1. **选区模型**：结果网格支持一个矩形选区，由「锚点单元格 + 焦点单元格」定义，覆盖连续的行与**可见数据列**。行号列不参与选区；隐藏列不参与；冻结列参与且保持显示。选区用 `sortedRows` 的索引和 `visibleColumns` 的列索引记录，排序 / 筛选后清空，避免错位。
2. **鼠标**：
   - 单击单元格设置单格选区并聚焦网格。
   - 按住拖动扩展到矩形范围；拖动期间不触发浏览器文本选择，不打开右键菜单。
   - `Shift + 单击` 从当前锚点扩展到点击位置；再次单击（不带 Shift）重新开始。
   - 在选区外右键：先把选区重置到该单元格，再打开原有单元格菜单；在选区内右键：保留选区，菜单新增「复制选区」。
   - 双击仍然打开值查看器（D-03 行为不变）；双击不应破坏选区。
3. **键盘**：
   - 结果网格容器可获得焦点（`tabindex="0"`）；获得焦点后显示当前活动单元格的焦点环。
   - 方向键移动活动单元格；`Shift + 方向键` 扩展选区；`Home` / `End` 到当前行首 / 行尾；`PageUp` / `PageDown` 按视口高度移动；`Cmd/Ctrl + A` 选择全部可见数据单元格；`Esc` 清空选区。
   - 活动单元格移出视口时自动滚动到可见位置；虚拟滚动不得导致选区错位。
4. **统计摘要**：
   - 有选区时，在结果网格底部（或结果面板状态条）显示摘要条，至少包含：选中行数 × 列数、非空单元格数、NULL 数、去重值数；数值单元格额外显示计数、求和、平均、最小、最大。
   - 数值判定：JS `number` 或可解析为有限数字的字符串计入数值统计；`NULL`、空串、布尔、日期、对象 / 数组不计入数值，但仍计入总单元格数、非空数和去重数。去重按显示值比较，大小写敏感（与数据库值一致）。
   - 数字格式化：整数不加 `.0`；小数最多保留 6 位有效小数或用千分位（与现有结果格式化一致）；极大 / 极小数用紧凑格式并保留完整值在 `title`。
   - 摘要条使用 `aria-live="polite"`；没有选区时隐藏；选区变化时更新，不触发数据库请求。
5. **复制选区**：
   - 结果网格聚焦且有选区时，`Cmd/Ctrl + C` 复制选区为 TSV；无选区时不劫持浏览器复制。
   - 右键菜单新增「复制选区」和「复制选区（含列名）」；按可见列顺序输出；NULL 输出空串；对象 / 数组输出 JSON 文本；单元格内换行、制表符按现有复制规则处理（至少保证 TSV 可被 Excel 正确解析）。
   - 最多复制 50,000 个单元格或 10 MB 文本，超过时提示「选区过大，请缩小范围」并拒绝复制；不生成半截内容。
   - 复制成功用现有 toast 提示「已复制 R × C 个单元格」；不写执行历史。
6. **整列统计**：
   - 列头右键菜单新增「统计此列」，把该列全部可见行设为选区并显示摘要；不改变排序、隐藏、冻结和导出范围。
   - 可选增加「复制整列」，输出当前可见行的该列值（每行一行）；如果实现成本高，可只做「统计此列」。
7. **与其他功能的关系**：
   - F-02 筛选激活时，统计与复制只针对筛选后的行；筛选条件变化、排序变化、隐藏列变化、结果列结构变化、重新执行查询、切换「结果 N」时清空选区。
   - E-01 查找高亮与选区独立；查找跳转不改变选区。
   - 冻结列横向滚动时，选区背景与冻结单元格保持对齐；隐藏当前选区涉及的列时，选区收缩到剩余可见列，若没有可见列则清空。
   - 值查看器、行详情、导出、复制整行 / 全部结果保持现有行为；G-01 不改变导出范围。
   - 查询执行中、没有结果、结果不是查询结果、结果为空时禁用选区与摘要。

### 涉及文件 / 可行落点

- `src/components/ResultGrid.vue`：新增 `selection` 状态（anchor / focus / active）、鼠标与键盘事件、选区渲染 class、底部摘要条、右键菜单「复制选区 / 统计此列」；把 `sortedRows` / `visibleColumns` 传给 helper。
- 新增 `src/lib/grid-selection.ts`：`normalizeRange`、`selectionBounds`、`selectionCellCount`、`clampSelection`、`rangeRows` / `rangeColumns`、`isCellSelected`；纯函数，便于单测。
- 新增 `src/lib/grid-stats.ts`：`isNumericCell`、`summarizeCells`、`summarizeColumn`、`formatStatValue`；返回 `{ count, nonNull, nulls, distinct, numericCount, sum, avg, min, max }`。
- `src/lib/virtual-rows.ts`：如现有 helper 不足以计算「活动单元格滚动到视口」，补充 `rowIndexAtOffset` / `scrollIndexIntoView`，不要复制一套虚拟滚动逻辑。
- `src/styles.css`：选区背景、活动单元格焦点环、摘要条、冻结列选区背景；使用主题变量，浅色 / 深色都可读。
- 不新增 Rust 命令，不修改 `ResultGrid` 的 `export-request` 快照。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] 执行 `SELECT id, name, email, city, created_at FROM users ORDER BY id`，拖选 3 行 × 4 列：摘要显示 12 个单元格、12 个非空、0 个 NULL，复制 TSV 与屏幕值一致。
- [ ] 执行包含数值列的查询（如 `SELECT id, amount, status FROM orders ORDER BY id`），选中 amount 列 3 个值：求和 / 平均 / 最小 / 最大与手工计算一致；混合文本列只显示计数 / 去重 / NULL。
- [ ] `Shift + 单击` 从锚点扩展；方向键 / `Shift + 方向键` / `Home` / `End` / `PageUp` / `PageDown` / `Cmd/Ctrl + A` / `Esc` 行为符合描述；活动单元格滚出视口时自动滚入。
- [ ] 右键选区外单元格会重置选区并打开菜单；右键选区内单元格保留选区；双击仍打开值查看器。
- [ ] 隐藏一列后选区与复制只包含可见列；冻结前两列后横向滚动，选区背景与冻结列对齐；隐藏选区涉及的列后选区正确收缩。
- [ ] 开启 F-02 筛选（例如 `status = paid`）后统计只包含筛选后的行；修改筛选 / 排序 / 隐藏列 / 重新执行查询后选区清空。
- [ ] 10,000 行结果（可用 SQLite `WITH RECURSIVE` 生成）拖选与键盘移动无明显卡顿，摘要计算 < 100ms，虚拟滚动无错位；50,000 单元格以上复制被拒绝并提示。
- [ ] SQL 编辑器和表数据页的 `Cmd/Ctrl+C` / `Cmd/Ctrl+A` 不受影响；结果网格无选区时 `Cmd/Ctrl+C` 不被劫持。
- [ ] 选区、统计、复制均不触发数据库请求、不写执行历史、不改变导出范围。

### 风险 / 限制

- 虚拟滚动 + 鼠标拖动需要处理「拖动到视口边缘自动滚动」；第一版可只做视口内拖动，键盘可跨视口，但验收必须覆盖滚动后的选区背景。
- 浏览器原生文本选择与单元格拖选冲突；在网格内禁用原生选择、改用显式复制是最稳妥方案，但会影响用户直接选中文本复制的习惯，需在菜单中保留「复制单元格」。
- 数值字符串的判定可能产生歧义（前导零、货币符号、千分位）；第一版只接受 `Number()` 可解析的有限数字，不做本地化解析。
- 去重统计对大选区有内存成本；50,000 单元格上限内可接受，超过上限直接拒绝。
- 冻结列 + 隐藏列 + 选区三个索引体系容易错位，必须统一用「原始列索引」存储，渲染时再映射到可见列。

---

## G-02 表数据按筛选 / 排序导出全部匹配行（含选中行）

- **优先级**：P0
- **状态**：已完成（2026-11-26：`TableDataExportDialog.vue`、当前页 / 选中行 / 全部匹配、CSV / JSON / XLSX / SQL INSERT、`export_table_rows` 流式读取与取消、筛选排序一致性；SQLite 单测覆盖 WHERE + 排序导出）
- **范围**：前端（`src/components/TableDataView.vue`、新增 `src/components/TableDataExportDialog.vue`、新增 `src/lib/table-export.ts`、`src/lib/api.ts`、`src/lib/types.ts`）；后端（`src-tauri/src/models.rs`、`src-tauri/src/commands/data.rs`、`src-tauri/src/db/driver.rs`、`sqlite.rs` / `mysql.rs` / `postgres.rs`、`src-tauri/src/lib.rs`）；复用 `csv`、`rust_xlsxwriter`、`commands/xlsx.rs` 与现有导出格式化代码。
- **用户问题 / 场景**：表数据页筛选出 2,000 行、按金额排序后，工具栏只有「导出当前页 CSV / JSON / XLSX」，实际只导出当前 100 行；改用对象右键「导出」又会导出整张表、忽略筛选和排序。用户想把当前筛选结果或选中的几十行交给同事 / 做报表，只能先复制到 Excel，容易漏行或顺序不一致。

### 现状核对（基于 2026-11-26 代码）

- `TableDataView.vue` 有 `appliedFilter`（WHERE 字符串）、`sorts`（多列排序）、`limit`（每页 50 / 100 / 200 / 500）、`rows`（当前页）、`total`（筛选后总数）、`selectedRows`（当前页行选择）、`displayRows`（含待提交 overlay）。
- `exportCsv` / `exportJson` / `exportXlsx` 都只使用 `rows`（当前页），不包含待提交修改，也不接受筛选 / 排序之外的参数；没有「全部匹配」「选中行」入口。
- 后端 `export_object` 的 `limit` 只能是「全部 / 指定行数」，`export_stream` 调用 `driver.export_rows(table, limit, sink)`；`export_rows` 只生成 `SELECT * FROM table [LIMIT n]`，没有 WHERE / ORDER BY。`read_rows_sorted` 已有筛选、多列排序、主键稳定排序与字段校验逻辑，可复用其规则。
- `export_object` 目前不做进度事件（只有取消 flag）；数据库级导出 `database-export:progress` 有进度事件，但对象 / 表数据导出没有。

### 期望行为（可验收细节）

1. **入口**：
   - 表数据工具栏新增「导出…」按钮（`download` 图标），打开 `TableDataExportDialog`；现有「导出当前页 CSV / JSON / XLSX」可以保留，避免破坏老用户习惯，但文案明确「当前页」。
   - 对话框标题显示表名、当前连接 / 库 / 模式、筛选条件摘要与排序摘要。
2. **导出范围**：
   - **当前页（按网格显示）**：导出当前页 `displayRows` 中未被标记删除的行；包含待提交的更新值与新增行；不包含待删除行；对话框显示「含未提交修改，数据库尚未写入」警告。
   - **当前页选中行**：仅在当前页存在行选择时可用；只导出选中的显示行，包含待提交 overlay；选择为空时禁用并说明。
   - **全部匹配（数据库当前值）**：按当前 `appliedFilter` + `sorts` 从数据库流式读取；排序必须与网格一致（多列 + 主键稳定排序）；如果存在待提交修改，先要求「提交 / 放弃 / 仅导出当前页」，不得把未提交值与数据库值混在一个文件里。
   - 未设置筛选时，「全部匹配」等价于整表导出。
3. **格式**：
   - CSV：可选表头；字段按 RFC 4180 转义；NULL 输出空串（与现有导出一致）；行序与网格一致。
   - JSON：输出 `{ table, columns, rows }` 或与现有对象 JSON 导出一致的结构；保留数字 / 布尔 / null；对象按 JSON 文本。
   - XLSX：复用现有工作簿写入；表头加粗 / 冻结 / 自动筛选；数字与布尔保留类型；单文件最多 200,000 行，超过时提示改用 CSV。
   - SQL INSERT：目标表默认当前表名，可编辑；按当前驱动引用标识符、转义字面量；包含全部列；不包含 DDL（需要结构请用对象右键导出 DDL）；二进制按现有 SQL 字面量规则处理，无法保真时明确提示。
4. **数量与提示**：
   - 对话框显示「将导出 N 行 × M 列」：当前页 / 选中行用 `displayRows` / `selectedRows` 计算；全部匹配用当前 `total`（筛选后计数）或重新执行一次 `read_table_rows(limit=1)` 获取，避免使用过期计数。
   - 0 行时禁用导出并提示「当前范围没有可导出的行」；全部匹配超过 1,000,000 行时二次确认；XLSX 超过 200,000 行时禁用并建议 CSV。
   - 筛选条件包含分号时沿用现有 `read_rows_sorted` 的拒绝规则；排序字段不存在时后端返回明确错误，不生成半截文件。
5. **后端命令**：
   - 新增只读命令 `export_table_rows`（避免与 `commands/export.rs` 内部 `export_table_data` 函数重名），参数包含 `connection_id`、`table`、`format`、`path`、`filter`、`sorts`、`limit`、`with_header`、`operation_id`。
   - 驱动层新增 `export_rows_filtered(table, limit, filter, sorts, sink)`：SQLite / MySQL / PostgreSQL 各自实现游标流式读取；默认实现可用 `read_rows_sorted` 分批读取，但大表必须走游标。
   - 生成 SQL 时复用 `read_rows_sorted` 的 WHERE 校验、排序字段校验、去重与主键稳定排序规则；MySQL 使用当前库限定名，PostgreSQL 使用当前模式限定名，SQLite 使用现有引用规则。
   - 动态 SQL 必须使用 `sqlx::AssertSqlSafe` 包装（SQLx 0.9 约束）；不新增 crate。
   - `operation_id` 注册到现有 `state.export_cancels`，取消时停止读取并删除未完成文件；失败时删除文件并返回原始错误。
6. **进度与取消**：
   - 全部匹配导出时显示进度对话框：已读取 / 已写入行数、字节数（可分阶段）、当前表名；提供「取消」；完成后显示路径、行数、耗时，支持复制路径。后端每 N 行（例如 2,000 行）发出一次 `table-export:progress` 事件（`operation_id`、行数、字节数、阶段），前端按 `operation_id` 过滤；如果首版不实现事件，至少显示不确定进度 + 取消，不能只有完成态。
   - 当前页 / 选中行导出为内存操作，可不显示进度，但大选区仍需避免阻塞 UI。
7. **语义 / 安全**：
   - 只读连接允许导出；导出不写数据库、不写执行历史。
   - 当前页 / 选中行导出包含待提交修改时，必须在对话框和完成提示中明确「含未提交修改」；全部匹配导出只能读取数据库当前值。
   - 不改变对象右键「导出」的默认行为；对象导出仍为全部 / 指定行数。
8. **列范围**：默认导出全部表列；G-04 落地后，如果表数据有隐藏列，导出使用当前可见列，并在对话框显示「按当前可见列导出」。G-02 本身不实现隐藏列。

### 涉及文件 / 可行落点

- `src/components/TableDataView.vue`：新增「导出…」入口；把 `appliedFilter`、`sorts`、`displayRows`、`selectedRows`、`limit`、`total` 传给对话框；处理 `ensureClean` 与待提交提示。
- 新增 `src/components/TableDataExportDialog.vue`：范围、格式、表头、目标表、行数提示、待提交警告、进度 / 取消 / 完成状态。
- 新增 `src/lib/table-export.ts`：范围与格式枚举、当前页 / 选中行快照、文件名生成、CSV / JSON / SQL 内容构建（与结果导出区分）、XLSX 调用参数组装。
- `src/lib/api.ts` / `src/lib/types.ts`：`exportTableRows` 封装、请求 / 进度类型。
- `src-tauri/src/models.rs`：`TableExportRequest` / `TableExportProgress`（如需要）。
- `src-tauri/src/commands/data.rs`：新增 `export_table_rows` 命令；复用 `export_stream` 的写入分支或抽取 `export_rows_to_path` helper；注册取消。
- `src-tauri/src/db/driver.rs`：新增 `export_rows_filtered` trait 方法，默认实现可回退到 `read_rows_sorted`。
- `src-tauri/src/db/sqlite.rs` / `mysql.rs` / `postgres.rs`：游标流式实现；复用各自的 `qualified_identifier` / `quote_identifier`、`list_columns`、主键读取与排序构建逻辑。
- `src-tauri/src/lib.rs`：注册 `commands::export_table_rows`。
- 复用 `src-tauri/src/commands/xlsx.rs` 的 `write_cell` / `safe_sheet_name`，不要新写 XLSX 逻辑。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] `orders` 表筛选 `status = 'paid'`，按 `amount DESC` 排序，网格显示 N 行；「全部匹配」CSV 导出 N 行，行序、值、表头与网格一致，对话框显示 N。
- [ ] 无筛选时「全部匹配」导出整表；与对象右键「导出全部行」内容一致（CSV 表头选项相同）。
- [ ] 当前页选择 3 行，「当前页选中行」CSV 只包含 3 行；取消选择后该范围禁用并说明。
- [ ] 修改一个单元格但未提交，导出当前页 CSV：文件包含修改后的值并显示「含未提交修改」；标记删除的行不出现在文件中。
- [ ] 有待提交修改时选择「全部匹配」：对话框要求先提交 / 放弃 / 仅导出当前页；不会把未提交值混入数据库快照。
- [ ] CSV / JSON / XLSX / SQL INSERT 四种格式均可用；CSV 表头开关、JSON 类型、XLSX 表头冻结与自动筛选、SQL 目标表引用与转义正确。
- [ ] 筛选包含分号（如 `id = 1; DROP TABLE users`）被拒绝并提示；排序字段不存在时返回明确错误；不会生成半截文件。
- [ ] 10 万行表执行「全部匹配」CSV：显示进度、可取消；取消后未完成文件被删除；应用不冻结，完成后行数与数据库一致。
- [ ] XLSX 超过 200,000 行时禁用并建议 CSV；1,000,000 行以上二次确认。
- [ ] 只读连接可以导出；导出不写数据库、不写执行历史；对象右键导出行为不变。
- [ ] SQLite 单元测试覆盖 WHERE / 排序 / 主键稳定排序 / 取消；MySQL / PostgreSQL 门控测试覆盖同名列、复合主键与 NULL 排序。

### 风险 / 限制

- **排序一致性**：如果导出排序与网格排序实现不一致，用户会看到文件顺序与屏幕不同。必须复用同一套排序规则（含主键 tiebreaker），并在验收中逐行对比。
- **筛选注入**：现有表数据筛选本来就接受用户输入的原生 WHERE；导出沿用同一信任边界，只拒绝分号，不新增 SQL 解析器。输入框已有提示，但这不是 SQL 注入防护，需在文档中说明。
- **待提交语义**：当前页 / 选中行包含未提交修改是「所见即所得」，但导出文件可能包含数据库中不存在的值；必须显著标注，且全部匹配范围禁止混合。
- **大表内存**：CSV / JSON / SQL 必须流式；XLSX 受 `rust_xlsxwriter` 内存与 200,000 行上限限制；MySQL / PostgreSQL 游标读取需要避免长事务。
- **二进制 / 数组 / JSON**：沿用现有导出字面量规则；无法保真的类型要在对话框或完成提示中说明，不能静默写占位。
- **取消**：取消只保证停止后续读取，已写入部分必须删除；数据库驱动可能在取消后仍返回少量行，需在 sink 中再次检查。
- **命名冲突**：`commands/export.rs` 已有内部 `export_table_data` 函数；新命令务必用不同名称（建议 `export_table_rows`），避免 Rust 名称冲突。

---

## G-03 结果集图表（柱 / 线 / 饼，聚合与 Top N）

- **优先级**：P1
- **状态**：已完成（2026-11-26：三视图切换、原生 SVG、聚合 / Top N、筛选联动、负值坐标轴、SVG 导出、数据表复制；10,000 行聚合实测约 1.5 ms）
- **范围**：前端（`src/components/QueryWorkspace.vue`、新增 `src/components/ResultChart.vue`、新增 `src/lib/result-chart.ts`、`src/components/ResultGrid.vue` 暴露图表快照、`src/styles.css`）；不涉及后端和数据库。
- **用户问题 / 场景**：查询返回一堆数字，想快速看「各状态订单金额占比」「每天新增用户趋势」「金额分布」，现在只能导出 Excel 或再写一条 `GROUP BY` SQL；DBeaver / Navicat 都能直接从结果集画图，mydb 没有。

### 现状核对（基于 2026-11-26 代码）

- `QueryWorkspace.vue` 结果面板标题栏有「结果 / 执行计划」切换、筛选按钮、查找按钮、导出按钮；没有图表入口。
- `ResultGrid.vue` 有 `result.columns`、`filteredRows`、`sortedRows`、`visibleColumns`、隐藏列、F-02 筛选；没有图表数据接口。
- `ResultExportDialog.vue` / `result-export.ts` 只导出表格；没有 SVG / 图片导出。
- 项目没有图表库；`package.json` 没有 chart / d3 依赖。

### 期望行为（可验收细节）

1. **入口与视图切换**：
   - 结果面板的「结果 / 执行计划」分段控件扩展为「结果 / 图表 / 执行计划」；「图表」仅在当前结果 `isQuery === true`、有行、未在运行时可用。
   - 切到图表时保留当前 F-02 筛选条件；图表标题显示「筛选 N 个条件」徽标与「调整筛选」按钮；点击「调整筛选」切回结果视图并打开筛选条。
   - 切换结果集（「结果 1 / 结果 2」）、重新执行查询、结果列结构变化时重置图表配置；排序、隐藏列、列宽、冻结、筛选变化不重置配置，只重新渲染。
2. **数据来源**：
   - 使用当前结果集 + F-02 筛选后的 `sortedRows`；列使用当前可见列（隐藏列不可选）；不重新查询数据库。
   - 由 `ResultGrid` 通过 `defineExpose` 提供 `getChartData()`（列名、可见列索引、按当前排序的行）或等价的只读快照；图表组件不得自行再实现一套筛选。
3. **配置项**：
   - 图表类型：柱状图（垂直）、折线图、饼图 / 环形图。
   - X 轴列：任意可见列；默认选择第一个文本 / 日期列，否则使用「行号」。
   - Y 轴列：数值列；默认第一个数值列；选择「计数」时不需要 Y。
   - 聚合：不聚合（原始行）/ 求和 / 平均 / 计数 / 最小 / 最大；默认「求和」，无 Y 时默认「计数」。「不聚合」主要用于折线 / 逐行柱；柱 / 饼遇到重复 X 时按行拆分为 `X (行 n)`，不静默合并。
   - Top N：5 / 10 / 20 / 50 / 全部；默认 20；超过 N 的分类聚合为「其他」（柱 / 饼），折线按当前顺序截断并提示。
   - 排序：按分类升序 / 降序 / 按值升序 / 按值降序；柱 / 饼默认按值降序，折线默认按 X 当前顺序。
   - 显示数值标签开关；饼图显示百分比；图例默认显示（单系列柱 / 折线可隐藏）。
4. **聚合语义**：
   - 按 X 值分组；每个分类计算 Y 的聚合值；「计数」= 分组内行数，包含 Y 为 NULL 的行；求和 / 平均 / 最小 / 最大忽略 NULL。
   - X 为 NULL 归入「(NULL)」分类；Y 为 NULL 在求和 / 平均 / 最小 / 最大中忽略，并在图表下方显示「N 行因 Y 为空被忽略」。
   - 数值字符串按 G-01 的 `isNumericCell` 规则计入；非数值 Y 列禁止选择并给出提示；布尔、日期、对象不作为数值。
   - 折线图 X 为日期 / 数字时按当前排序或 X 值排序；不能解析为日期 / 数字的文本按分类处理。
5. **渲染与交互**：
   - 使用原生 SVG + Vue 响应式；不引入图表库；根据容器宽度自适应（`ResizeObserver`），高度固定或可拖拽。
   - 柱状图：X 轴标签过长时截断并 `title` 显示完整值；Y 轴刻度自动取整；柱宽自适应；hover 显示分类、聚合值、行数；可选数值标签。
   - 折线图：点 + 线；点数超过 500 时允许抽样或按 Top N 截断并提示；hover 显示最近点；缺失值断开，不插 0。
   - 饼图：Top N + 其他；百分比保留 1 位小数；图例可点击隐藏 / 显示分类（可选）；颜色不足时循环使用调色板。
   - 颜色：使用 8 色主题调色板，浅色 / 深色主题都满足对比度；不使用品牌资源；同一分类在同一图表中颜色稳定。
   - 空状态：没有数值列、结果为空、Y 全为 NULL 时显示可操作提示，不画空图。
6. **性能与上限**：
   - 最多分析 10,000 行；超过时只取前 10,000 行并提示「仅分析前 10,000 行」；分类最多 50 个；SVG 点 / 柱最多 2,000 个。
   - 配置变化 150ms 防抖；重渲染不阻塞输入；10,000 行 × 单列聚合 < 100ms。
   - 不修改结果数据、不改变排序 / 筛选 / 隐藏列 / 导出范围。
7. **导出**：
   - 图表工具栏提供「导出 SVG…」：用现有保存对话框 + `write_text_file` 写入自包含 SVG；文件名含表 / 查询标签与时间戳；SVG 包含标题、图例、坐标轴和当前数据。
   - 第一版不做 PNG；如后续需要，可用 Canvas 渲染 SVG 后导出，但不在本批。
   - 导出不触发数据库请求；不写执行历史。
8. **可访问性**：
   - 图表容器有 `role="img"` 与 `aria-label`，概括「柱状图：按 status 统计 amount 总和，共 5 个分类」。
   - 提供「查看图表数据」折叠表，列出分类、聚合值、行数 / 占比；键盘可聚焦并复制。
   - 配置控件均有可读标签；颜色不是唯一信息载体（标签 / 图例 / 数据表至少有一个）。
9. **生命周期**：
   - 切到执行计划或折叠结果面板时保留图表配置；重新执行查询 / 结果集变化时重置；关闭标签时丢弃。
   - 图表模式不影响 F-02 筛选状态；筛选条只在结果视图显示，但条件继续生效。
   - 分屏 / 独立窗口各自维护图表配置，不写入主窗口存储。

### 涉及文件 / 可行落点

- `src/components/QueryWorkspace.vue`：结果面板视图状态从 `showExplain` 扩展为 `resultView: "grid" | "chart" | "explain"`；图表切换按钮；把 `result`、`resultSets`、`resultSetIndex`、筛选元数据传给图表；处理「调整筛选」回到结果视图。切换图表时 `ResultGrid` 必须保持挂载（例如 `v-show`），否则 E-01 / F-02 的筛选状态与滚动状态会被卸载；如需卸载，必须先把筛选状态提升到 `QueryWorkspace`，不能在两处各存一份。
- `src/components/ResultGrid.vue`：`defineExpose({ getChartData })` 或新增 `chart-data` 事件；返回 `{ columns, visibleColumnIndices, rows }`（`sortedRows` 快照），保证与网格筛选 / 排序一致；不复制筛选逻辑。
- 新增 `src/components/ResultChart.vue`：配置栏、SVG 绘制、tooltip、图例、数据表 fallback、导出按钮、空状态。
- 新增 `src/lib/result-chart.ts`：`buildChartSeries`、`aggregateByCategory`、`topNWithOther`、`niceTicks`、`formatChartValue`、`chartAriaLabel`；纯函数，便于单测。
- `src/styles.css`：图表容器、配置栏、SVG 文本 / 网格线 / 图例、浅色 / 深色主题、响应式。
- `src/lib/api.ts`：复用 `writeTextFile`；如需保存对话框，复用现有 `saveDialog`。
- 不新增 Rust 命令，不新增 npm 依赖。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [x] 执行 `SELECT status, COUNT(*) AS n, SUM(amount) AS total FROM orders GROUP BY status`：柱状图 X=status、Y=total，数值与结果网格一致；切换饼图显示占比；切换折线图可显示分类顺序。
- [x] 执行 `SELECT * FROM orders`：按 status 分组求和 amount；Top 3 + 其他；按值升序 / 降序切换正确。
- [x] 执行 `SELECT created_at, amount FROM orders ORDER BY created_at`：折线图按日期顺序；X 为 NULL 归入 `(NULL)`；Y 为 NULL 被忽略并显示忽略行数。
- [x] 开启 F-02 筛选（如 `status = paid`）后图表只包含筛选行；点击「调整筛选」回到结果视图并打开筛选条；筛选条件变化后图表更新。
- [x] 隐藏列不出现在 X / Y 下拉中；切换结果集 / 重新执行查询后图表配置重置；排序 / 隐藏列 / 冻结 / 列宽变化不重置配置。
- [x] 非查询结果（INSERT / UPDATE）、空结果、无数字列时显示空状态与原因；运行中禁用图表入口。
- [x] 10,000 行结果聚合 < 100ms；分类 > 50、点 > 2,000 时有截断提示；切换图表类型不卡顿。
- [x] 导出 SVG 能在浏览器 / 预览中打开，包含标题、图例、坐标轴、当前数据；浅色 / 深色主题下文字与线条可读。
- [x] 图表不触发数据库请求、不写执行历史、不改变结果 / 排序 / 筛选 / 导出范围；SQL 编辑器与结果网格操作不受影响。
- [x] 图表有 `aria-label` 与「查看图表数据」表；键盘可操作配置控件。

### 风险 / 限制

- SVG 自绘需要处理坐标轴刻度、标签重叠、负值、空分类、极小 / 极大值；第一版只保证常见数值范围，异常值显示提示或截断。
- 饼图分类过多会不可读；Top N + 其他是强制策略，不能全部铺开。
- 折线图 X 为文本时没有天然顺序，只能按当前排序或分类排序；用户可能误以为时间顺序，需在图上标注排序方式。
- 聚合语义（计数是否包含 NULL、平均是否忽略 NULL）必须在 UI 中写清楚，否则同一份数据不同工具结果不一致。
- 颜色可访问性：8 色循环在分类很多时可能重复；图例 + 数据表兜底，不单独依赖颜色。
- 图表模式与 F-02 筛选条分离，用户可能找不到筛选入口；必须提供「调整筛选」按钮和条件徽标。
- 不使用图表库意味着交互（缩放、框选、动画）有限；第一版只做静态图 + hover，不做缩放 / 拖拽。

---

## G-04 表数据列显示与表视图配置（命名保存 / 应用）

- **优先级**：P1
- **状态**：已完成（2026-11-26：隐藏列、命名视图、默认应用、待提交守卫、失败回退、可见列导出快照、SQL 列投影；headless 验证保存 / 切换 / 重开 / 失败回退 / 损坏存储）
- **范围**：前端（`src/components/TableDataView.vue`、新增 `src/components/TableViewMenu.vue`、新增 `src/lib/table-views.ts`、`src/lib/frozen-columns.ts` 复用、`src/styles.css`）；localStorage `mydb.tableViews.v1`；不涉及后端和数据库。
- **用户问题 / 场景**：同一张表每天要按同样的条件分析：筛选 `status = 'paid'`、按金额降序、隐藏几个不关心的列、冻结前两列、调好列宽。现在每次重新打开表都要重做一遍；表数据页甚至没有隐藏列功能；Navicat 的 Table Profile 可以保存并复用这些设置。

### 现状核对（基于 2026-11-26 代码）

- `TableDataView.vue` 有 `columns`、`rows`、`appliedFilter`、`sorts`、`limit`、`columnWidths`、`frozenIndices` / `frozenNames`；没有 `hiddenColumns`，模板直接遍历 `columns`。
- 每页行数按表记忆（`mydb.tablePageSize.v1`）；冻结列按驱动 + 连接 + 库 / 模式 + 表持久化（`mydb.frozenColumns.v1`）；列宽、筛选、排序只在当前标签内有效。
- 表数据导出当前页使用 `columns` + `rows`；如果增加隐藏列，需要同步导出 / 复制语义。
- F-01 已实现冻结列；G-04 不替换它，只把冻结列名作为视图配置的一部分保存 / 应用。

### 期望行为（可验收细节）

1. **表数据隐藏列**：
   - 列头右键菜单新增「隐藏此列」「显示所有列」；工具栏显示「已隐藏 N 列」徽标或菜单项。
   - 至少保留 1 个可见列；隐藏最后一列时禁用并提示「至少保留一列」。
   - 隐藏列不参与网格渲染、查找、复制、当前页导出、G-02 导出；但仍在 `columns` 中，待提交更新 / 删除匹配、主键判断、行详情、结构字段信息不受影响。
   - 行详情抽屉仍显示隐藏列，并标记「已隐藏」；值查看器 / 单元格菜单只对可见列生效。
2. **列宽**：
   - 保留现有拖拽与双击恢复；新增「按内容自动列宽」可选（如果成本高，只保留现有）。
   - 列宽按列名保存到视图配置；应用配置时恢复；列不存在则忽略。
3. **命名视图**：
   - 表数据工具栏新增「视图」下拉（`view` / `list` 图标）：列出当前表的视图配置、当前活动项、默认项；底部「保存当前视图…」「管理视图…」。
   - 保存：输入名称（1–40 字符，同表内唯一），保存当前 `appliedFilter`、`sorts`、`limit`、列宽、隐藏列名、冻结列名；可选「设为默认」。
   - 应用：点击列表项立即应用；若当前有未提交修改，先走现有 `ensureClean` 提交 / 放弃 / 取消；取消则不改变任何状态。
   - 管理：重命名、删除、设为默认 / 取消默认；删除当前活动配置后回到无配置状态；默认配置在打开表时自动应用。
   - 当前活动配置在工具栏显示名称；用户手动修改筛选 / 排序 / 列宽 / 隐藏 / 冻结后，显示「已修改」标记，不自动写回配置（需要「保存」或「更新当前视图」）。
4. **配置内容与语义**：
   - `filter`：保存应用后的 WHERE 字符串（不是筛选构建器的中间态）；应用时填入 `appliedFilter` 并触发一次正常读取。
   - `sorts`：保存 `TableSort[]`；应用时校验列仍存在，缺失则丢弃该排序并提示。
   - `limit`：保存每页行数；应用时更新现有分页记忆。
   - `widths`：`Record<profileColumnKey, number>`；`profileColumnKey` 默认等于列名，列名重复时使用 `名称#序号`。应用时只恢复仍存在的列，展示仍用列名。
   - `hidden`：`profileColumnKey[]`；应用时只隐藏仍存在的列，至少保留一个可见列。
   - `frozen`：`profileColumnKey[]`；应用时通过现有冻结列逻辑恢复，并同步写入 `mydb.frozenColumns.v1`，保证关闭重开仍生效。
   - 第一版**不保存列顺序 / 不做列拖拽重排**；列顺序仍按表结构。
5. **存储与隔离**：
   - 新 key `mydb.tableViews.v1`，结构包含 `version`、`entries`；每个 entry 按「驱动 + 连接 ID + 数据库 / 模式 + 表名 + 对象类型（table/view）」隔离。
   - 每张表最多 20 个配置，全局最多 200 张表；超出按最近使用淘汰；损坏 / 旧版本 JSON 回退为空并清理，不阻塞表加载。
   - 配置只存列名、过滤字符串、排序、尺寸等元数据，不存行数据、不存密码。
6. **应用失败与恢复**：
   - 应用配置后读取失败（筛选引用已删除列、排序字段不存在、权限错误）：显示可读错误，回退到应用前的筛选 / 排序 / 列状态，并保留表可用；不要留下半应用状态。
   - 隐藏列 / 列宽 / 冻结列引用的列不存在时自动剪掉，不报错。
   - 切换表、数据库 / 模式、连接时关闭视图菜单并重置活动配置；不同表 / 视图配置互不影响。
7. **其他行为**：
   - 只读连接可以保存 / 应用视图（纯本地），不写数据库、不写执行历史。
   - 视图菜单的打开 / 切换不触发数据库请求；应用配置只触发现有的一次 `read_table_rows`。
   - 表数据复制 / 导出使用当前可见列；无隐藏列时与现状一致。现有「导出当前页 CSV/JSON/XLSX」按钮如果保留，必须改为使用 `displayRows` + 可见列（含待提交 overlay），或统一收口到 G-02 的导出对话框；不能继续导出隐藏列或忽略待提交修改。
   - G-02 导出对话框显示「按当前可见列」；如果用户在对话框打开期间隐藏列，导出使用打开时的快照，避免歧义。

### 涉及文件 / 可行落点

- `src/components/TableDataView.vue`：新增 `hiddenColumns` / `visibleColumns`、列头隐藏 / 显示菜单、列宽持久化、视图菜单入口、活动配置状态、应用 / 保存 / 恢复逻辑；模板 `v-for` 从 `columns` 改为 `visibleColumns`；导出 / 复制 / 查找改用可见列。
- 新增 `src/components/TableViewMenu.vue`：视图列表、保存对话框、重命名 / 删除 / 默认、已修改标记、空状态。
- 新增 `src/lib/table-views.ts`：类型 `TableViewProfile`、`TableViewStore`；`loadTableViewStore` / `saveTableViewStore`、`profilesForScope`、`upsertProfile` / `deleteProfile` / `setDefaultProfile`、`pruneProfiles`、`profileScopeKey`；损坏回退与上限。
- `src/lib/frozen-columns.ts`：复用 `loadFrozenNames` / `saveFrozenNames` / `frozenScopeKey`；不改变现有 key 语义。
- `src/styles.css`：视图菜单、隐藏列徽标、活动配置 / 已修改标记、保存对话框；复用现有上下文菜单与模态框样式。
- 不新增 Rust 命令，不修改数据库 schema。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [x] `users` 表隐藏 `email`、`created_at` 两列：网格、查找、复制、当前页 CSV 导出都不包含这两列；行详情仍显示并标记「已隐藏」；「显示所有列」恢复。
- [x] 隐藏到只剩 1 列时「隐藏此列」禁用并提示；列头菜单状态正确。
- [x] 调整 `name`、`city` 列宽后保存视图；关闭表标签再打开，列宽恢复；双击列宽恢复自动宽度。
- [x] 设置筛选 `status = 'paid'`、按 `amount DESC` 排序、每页 200、隐藏 `user_id`、冻结 `id` / `status`，保存为「已支付订单」；关闭重开并应用，筛选 / 排序 / 分页 / 隐藏 / 冻结 / 列宽全部恢复，数据行与保存时一致（除数据库变化）。
- [x] 同一张表保存第二个视图「全部订单」，在两个视图间切换；默认视图在打开表时自动应用；重命名 / 删除 / 取消默认行为正确；重名被拒绝。
- [x] 有待提交修改时应用另一个视图：弹出提交 / 放弃 / 取消；取消后筛选 / 排序 / 列状态不变；提交 / 放弃后按选择继续。
- [x] 手动修改筛选或排序后显示「已修改」；不自动写回配置；点击「更新当前视图」后保存。
- [x] 视图配置引用的列被删除 / 重命名后：自动剪掉隐藏 / 列宽 / 冻结项；排序项缺失时提示并丢弃；筛选失效时回退到应用前状态，表仍可用。
- [x] 不同连接 / 数据库 / 模式 / 表 / 视图的配置互不串扰；只读连接可保存 / 应用；重启应用后配置仍在。
- [x] 损坏 `mydb.tableViews.v1`（非法 JSON、错误版本）时表正常打开并回退为空；超过每表 20 个 / 全局 200 张表时按策略淘汰。
- [x] 视图菜单打开 / 切换不触发数据库请求；应用只触发一次正常读取；不写执行历史。

### 风险 / 限制

- 筛选字符串是用户输入的原生 WHERE；保存 / 应用沿用表数据现有信任边界，不新增 SQL 解析或安全承诺。
- 列名可能重复（视图 / 连接结果），表数据列名来自 `ColumnInfo`，正常情况下唯一；如果重复，视图配置已用 `profileColumnKey`（默认列名，重复时 `名称#序号`）区分，但 UI 展示与用户输入仍按列名，重名时需要给出提示。
- 隐藏列与待提交更新 / 删除匹配：待提交状态以列名为键，不受隐藏影响；但用户可能忘记隐藏列里还有未提交值，工具栏的「N 项待提交」必须始终可见。
- 列宽 / 冻结列与现有 localStorage 存储并存：应用配置写入冻结列 key，可能覆盖用户刚手动调整的冻结状态；这是预期行为，但需要有「已修改」提示，避免用户误以为自动同步。
- 隐藏列后导出 SQL INSERT 只包含可见列，可能违反 NOT NULL 或遗漏业务字段；G-02 对话框在 SQL 格式下必须提示，或提供「包含隐藏列」开关，不能静默生成不完整 INSERT。
- 视图配置不会随列重命名自动迁移；第一版按列名剪枝，用户需重新保存。
- 不做列拖拽重排 / 转置；Table Profile 的「列顺序」能力留到下一批。
- localStorage 容量有限；只存元数据，20 × 200 的上限已足够，仍要处理 `QuotaExceededError`，失败时提示且不丢当前会话状态。

---

## G-05 SQL 静态检查（按需、高置信度、不阻塞执行）

- **优先级**：P2
- **状态**：已完成（2026-11-26：tokenizer、表 / 列 / 危险语句检查、检查对话框、工具栏 / 编辑菜单 / 原生菜单 / 命令面板、选区和范围、元数据缺失降级；headless 验证多语句、跳转与切换标签清空）
- **范围**：前端（`src/components/SqlEditor.vue`、`src/components/QueryWorkspace.vue`、新增 `src/components/SqlCheckDialog.vue`、新增 `src/lib/sql-check.ts`、`src/lib/dangerous-sql.ts` 复用、`src/lib/sql-statements.ts` 复用、`src/lib/types.ts`、菜单 / 命令面板注册）；不涉及后端和数据库。
- **用户问题 / 场景**：SQL 写错表名、列名、引号或括号时，只有执行后数据库报错才知道；`UPDATE` / `DELETE` 忘写 WHERE 虽然有危险确认，但检查发生在执行前一刻；DBX 能在编辑时给出未解析表 / 列提示。用户需要一个轻量、不会误拦截执行的「检查 SQL」。

### 现状核对（基于 2026-11-26 代码）

- `SqlEditor.vue` 有 CodeMirror 方言、`completionTables: ScopeTableColumns[]`（当前连接 / 库 / 模式的表与字段）、查找、格式化、片段、执行范围、手动事务；没有诊断。
- `sql-statements.ts` 能安全拆分语句并定位光标所在语句，但只做分句，不产出标识符位置。
- `dangerous-sql.ts` 能识别缺少 WHERE 的 UPDATE / DELETE、DROP、TRUNCATE、ALTER DROP，并用于执行前确认；可以复用为检查的一部分。
- 数据库错误只在执行后由 `run` 返回；执行历史只记录实际执行。
- 项目没有 SQL 解析器依赖；`@codemirror/lang-sql` 提供语法高亮，但不应把它当作完整语义层。

### 期望行为（可验收细节）

1. **入口与范围**：
   - SQL 编辑器工具栏新增「检查」按钮（`check` 图标）；编辑菜单新增「检查 SQL」；命令面板新增同名命令。
   - 默认检查**当前语句**（复用 F-05 的 `statementAtCursor()` 语义）；有选区时检查选区；提供下拉「检查当前语句 / 检查整个标签」。
   - 检查不执行 SQL、不打开事务、不写执行历史、不修改编辑器内容；检查结果可关闭，不阻塞运行。
2. **词法检查（所有语句）**：
   - 未闭合的单引号 / 双引号 / 反引号 / 方括号 / PostgreSQL dollar quote、未闭合的块注释、括号不配对：报 `error`。
   - 字符串、注释、quoted identifier 内的关键字不参与结构检查；`--`、`#`、`/* */` 正确处理。
   - 超过 100 KB 或 5,000 个 token 时停止并提示「SQL 过大，请缩小选区」；最多报告 200 条问题。
3. **对象检查（需要当前 scope 元数据）**：
   - 从 `FROM`、`JOIN`、`UPDATE`、`INSERT INTO`、`DELETE FROM` 后提取表名（支持 `db.table` / `schema.table` / 引号标识符）；与 `completionTables` 的当前 scope 比较。
   - 表名不存在：报 `warning`「未在当前库 / 模式找到表 xxx」；如果表名带其它库 / 模式前缀且无法验证，跳过并提示「跨范围对象未检查」。
   - 列检查只对**简单单表语句**启用：无 `JOIN`、无 `UNION`、无 CTE（`WITH`）、无子查询、无 `SELECT *`；检查 SELECT 列表、WHERE、GROUP BY、ORDER BY、UPDATE SET、INSERT 列清单中的裸列名与 `table.column`。
   - 列名不存在：报 `warning`；别名、函数名、表达式、`*`、常量、关键字、`NULL`、`TRUE/FALSE`、参数占位符（`?` / `$n` / `:name`）跳过；无法确定时宁可不报。
   - 大小写：未加引号的标识符按驱动规则比较（MySQL / SQLite 大小写不敏感，PostgreSQL 未加引号折叠为小写）；加引号的标识符按原样比较。第一版可统一大小写不敏感并标注「可能误报」，但必须避免把 `"User"` 和 `user` 当成同一个对象。
   - `completionTables` 为空或元数据未加载时，跳过对象检查，只做词法 / 危险语句检查，并在结果中说明。
4. **危险语句检查（所有语句，复用现有检测）**：
   - 缺少 WHERE 的 UPDATE / DELETE、DROP TABLE / DATABASE / SCHEMA、TRUNCATE、ALTER TABLE ... DROP COLUMN / CONSTRAINT：报 `warning` / `info`，与现有危险 SQL 确认文案一致，不重复弹确认。
   - 不把字符串 / 注释中的关键字误报；多语句逐条检查。
5. **结果显示**：
   - 打开 `SqlCheckDialog`：顶部汇总「N 个错误 / N 个警告 / N 条提示」；列表按行号排序，显示严重级别、行:列、消息、代码（如 `unknown-table` / `unknown-column` / `unclosed-string` / `dangerous-dml`）。
   - 点击问题跳转到编辑器对应位置并选中（不自动执行）；对话框可保持打开；提供「复制问题」「重新检查」「关闭」。
   - 没有问题时显示「未发现高置信度问题」；明确提示「静态检查基于当前元数据，可能漏报 / 误报，执行结果以数据库为准」。
   - 严重级别只用于展示，不阻止运行；用户仍可直接执行。
6. **跳过与限制**：
   - 包含 `DELIMITER`、`CREATE PROCEDURE / FUNCTION / TRIGGER / EVENT` 过程体、动态 SQL（`PREPARE` / `EXECUTE`）、CTE、JOIN、子查询、`UNION` 的语句：列检查跳过；表检查尽量做，无法确定时跳过。
   - 不检查权限、数据类型、函数签名、GROUP BY 完整性、窗口函数语义、跨库 / 跨模式对象；这些留给数据库。
   - 不做实时波浪线 / 保存时自动检查；第一版只做按需检查，避免编辑时噪音与性能问题。
7. **集成**：
   - 分屏 / 独立查询窗口各自拥有编辑器与检查入口；检查只读取当前连接 / 库 / 模式的元数据缓存，不发起新查询。
   - 检查期间不锁编辑器；关闭标签 / 切换连接后丢弃结果。
   - 不影响 CodeMirror 查找、格式化、补全、撤销 / 重做和 F-05 执行范围。

### 涉及文件 / 可行落点

- `src/components/SqlEditor.vue`：工具栏「检查」按钮、`defineExpose({ runCheck })`、获取选区 / 当前语句 / 整个标签、问题跳转（`EditorView.dispatch({ selection, scrollIntoView })`）。
- `src/components/QueryWorkspace.vue`：承载 `SqlCheckDialog`、检查范围下拉、结果状态；不新增后端调用。
- 新增 `src/components/SqlCheckDialog.vue`：汇总、问题列表、严重级别筛选、跳转、复制、重新检查、空状态、免责声明。
- 新增 `src/lib/sql-check.ts`：
  - `tokenizeSql(text)`：带位置的轻量 tokenizer（字符串、quoted identifier、注释、dollar quote、数字、标识符、关键字、标点）。
  - `checkSql({ sql, driver, tables, scope })`：词法检查、表 / 列检查、危险语句检查；返回 `SqlCheckIssue[]`。
  - `statementKind` / `extractTableRefs` / `extractSimpleColumns` 等纯函数。
- `src/lib/dangerous-sql.ts`：复用现有危险 SQL 检测结果，转换成 `warning` / `info`，不要复制规则。
- `src/lib/sql-statements.ts`：复用 `splitSqlStatements` / `statementAtCursor` / `statementRangeAtCursor`。
- `src/lib/types.ts`：新增 `SqlCheckIssue` / `SqlCheckResult` 类型；`ScopeTableColumns` 已有，不重复定义。
- `src/lib/app-actions.ts` / `AppMenuBar.vue` / `App.vue`：编辑菜单与命令面板注册；不新增全局快捷键，避免与编辑器冲突。
- 不新增 Rust 命令，不新增 npm 依赖。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [x] `SELECT * FROM userz`：报「未找到表 userz」warning，行 / 列定位准确；点击跳转到 `userz`。
- [x] `SELECT emial FROM users`：报「未找到列 emial」warning；`SELECT email FROM users WHERE city = '上海'` 无问题。
- [x] `SELECT 'from userz', "email" FROM users`：字符串内的 `from userz` 不误报；quoted identifier 正确处理。
- [x] `UPDATE orders SET status = 'paid'`：报缺少 WHERE 警告；`UPDATE orders SET status = 'paid' WHERE id = 1` 不报。
- [x] `SELECT u.id, o.amount FROM users u JOIN orders o ON o.user_id = u.id`：列检查跳过（JOIN），不产生列误报；`WITH x AS (...) SELECT ...`、子查询同样跳过列检查。
- [x] `SELECT (1 + 2 FROM users`：报括号不配对；`SELECT 'abc FROM users`：报未闭合字符串；未闭合块注释 / dollar quote 同理。
- [x] 多语句：逐条检查，行号基于整个标签；只检查选区 / 当前语句时，行号仍映射到编辑器真实位置。
- [x] 当前语句 / 整个标签切换正确；光标在第 2 条语句时默认只检查第 2 条；选区优先。
- [x] 元数据未加载 / 空 scope：只做词法 / 危险语句检查，并显示「对象检查不可用」；不发起数据库请求。
- [x] 100 KB / 200 条问题上限生效；检查期间编辑器可继续输入；关闭对话框 / 切换标签后结果丢弃。
- [x] 检查不执行 SQL、不写执行历史、不改变编辑器内容、不阻止运行；CodeMirror 查找 / 格式化 / 补全 / 撤销不受影响。
- [x] 分屏 / 独立查询窗口各自检查各自的 SQL；互不影响。

### 风险 / 限制

- **误报是最大风险**：别名、CTE、子查询、窗口函数、`USING`、`NATURAL JOIN`、数据库函数、扩展语法都会让简单解析出错。第一版只对「简单单表、无复杂结构」做列检查，宁可漏报不误报；表检查也只做高置信度引用。
- **大小写 / 引号**：不同驱动对未加引号标识符的折叠规则不同；第一版可以按驱动做基本处理，但无法覆盖所有排序规则 / 扩展类型。
- **元数据过期**：`completionTables` 可能滞后于 DDL；检查结果必须注明「基于当前元数据」，并提供刷新对象后的重新检查。
- **性能**：100 KB / 5,000 token 上限内应同步完成；超过时停止，不做后台 worker，避免复杂化。
- **不替代数据库**：不检查权限、类型、函数签名、GROUP BY 完整性、锁 / 事务影响；用户仍可能执行失败。
- **不做实时波浪线**：CodeMirror 装饰器与增量更新会增加复杂度和误报噪音；本批只做按需检查，后续再评估实时诊断。
- **菜单 / 快捷键**：不新增全局快捷键，避免与编辑器、系统快捷键冲突；入口放在工具栏、编辑菜单和命令面板即可。

---

## 附录 G-A：测试数据与验证建议

- **SQLite**：`sqlite3 examples/demo.db < examples/seed.sql`；使用 `users`（5 列）、`orders`（5 列）、`order_items`（5 列）验证选区、统计、导出、图表、视图配置。
- **10,000 行**：可用 SQLite 递归 CTE：
  ```sql
  WITH RECURSIVE cnt(x) AS (
    SELECT 1
    UNION ALL
    SELECT x + 1 FROM cnt WHERE x < 10000
  )
  SELECT x, x * 2 AS double, 'row-' || x AS label FROM cnt;
  ```
- **MySQL / PostgreSQL**：临时表覆盖复合主键、NULL、数值字符串、日期、二进制；G-02 的真实回归需设置 `MYDB_TEST_MYSQL_*` / `MYDB_TEST_PG_*`（见 README）。
- **图表**：至少验证「分组聚合结果」与「原始明细 + 客户端聚合」两种输入；验证 NULL、负值、0、极大值、文本分类。
- **SQL 检查**：准备一组正例 / 反例：未知表、未知列、合法 JOIN、CTE、子查询、字符串内关键字、注释内关键字、引号标识符、多语句、未闭合符号、危险 DML。

## 附录 G-B：参考资料

### DBX（Apache-2.0，仅参考功能与公开文档）

- 数据表格（选区与摘要、转置、固定结果、列布局、隐藏空列）：<https://dbxio.com/en/docs/data-grid>
- 数据表格中文文档：<https://dbxio.com/cn/docs/data-grid>
- 查询编辑器（语义诊断、Ctrl/Cmd+Click、执行预览、执行范围）：<https://dbxio.com/en/docs/query-editor>
- 快捷键（当前语句 / 选区执行）：<https://dbxio.com/en/docs/keyboard-shortcuts>
- 仓库主页（Apache-2.0）：<https://github.com/t8y2/dbx>

### DBeaver（Apache-2.0，仅参考公开 wiki / 官方文档）

- Data Editor（Grid / Record / Chart / References / Metadata、导航历史）：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>
- Data Editor 官方文档（Chart、References、Calc、Grouping）：<https://dbeaver.com/docs/team-edition/25.3/desktop/Data-Editor/>
- Calc 面板（选中单元格统计）：<https://dbeaver.com/docs/dbeaver/Calc-Panel/>
- Managing Charts（柱 / 线 / 饼、X / Y、聚合）：<https://dbeaver.com/docs/team-edition/25.3/web/Managing-Charts/>
- 数据导出（Use fetched rows / Selected rows only / Selected columns only）：<https://dbeaver.com/docs/dbeaver/26.2/Data-export/>
- References 面板：<https://github.com/dbeaver/dbeaver/wiki/References-Panel>

### Navicat（专有软件，仅参考公开产品页 / 帮助文档 / 手册）

- Data Editor for RDBMS（Grid View / Form View、Filter & Sort、Profile、图表）：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>
- 筛选后导出：<https://help.navicat.com/hc/en-us/articles/217806188-How-do-I-export-filtered-records-after-applying-a-filter-to-a-table-or-collection>
- Data Profiling：<https://help.navicat.com/hc/en-us/articles/46711325532697-How-to-use-the-Data-Profiling-feature-to-analyze-data>
- Navicat 17 手册（Filter & Sort、Profile、图表、导出）：<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

### 本仓库

- `README.md`：当前功能基线、已知限制。
- `CHANGELOG.md`：`[Unreleased]` 与 0.11.0 变更。
- `docs/BACKLOG.md`：B / C / D / E / F 系列历史状态与长期探索。
- `docs/ARCHITECTURE.md`：命令 / 驱动 / 前端分层约束。
- `docs/PM-REQUIREMENTS-2026-11-26-batch4.md`：E / F 批需求与验收，已全部完成。

> 竞品功能仅用于确认「用户是否需要」与「交互应该怎样」，不得复制其源码、品牌图标、截图或专有文案。mydb 保持独立实现与 Apache-2.0 许可。

---

## 附：G 批变更记录

- 2026-11-26：创建本文件；确认 E-01～E-05、F-01～F-05 已全部完成，不重复提案；完成 DBX / DBeaver / Navicat 第三批差距核对；确定 G-01～G-05 与推荐实施顺序 G-01 → G-02 → G-03 → G-04 → G-05；若只能做一项做 G-01。
