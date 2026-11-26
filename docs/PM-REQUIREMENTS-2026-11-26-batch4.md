# mydb 产品需求（2026-11-26 批次：E-01～E-05 + F-01～F-05）

> 维护人：产品（PM）
> 文档日期：2026-11-26
> 基线版本：**v0.11.0（2026-11-24）**，并叠加当前工作区尚未提交的改动：编辑器多语句执行与多结果集、双查询分屏、独立查询窗口、数据库 / 模式级 SQL 导出、SQL 脚本执行与还原、Redis list/set/zset 编辑、对象树批量操作等
> 文档编号：本文件包含同日两轮产出。**E-01～E-05** 为同日首批（数据浏览 / 编辑基础）；**F-01～F-05** 为后续批次（宽表阅读、关联导航与快速录入），从文末「F 批」章节开始，不替代 E 批
> 状态更新（2026-11-26）：E-01～E-05、F-01～F-05 已全部完成，最新状态见 `docs/BACKLOG.md`；本文档保留为历史需求与验收记录。后续 G 批见 `docs/PM-REQUIREMENTS-2026-11-26-batch5.md`。
> 实施顺序：先完成 E 批，再进入 F 批；若只能做一项，E 批优先 E-01，F 批优先 F-01
> 前序文档：`docs/PM-REQUIREMENTS-2026-11-24.md`、`docs/PM-REQUIREMENTS-2026-11-24-batch2.md`、`docs/PM-REQUIREMENTS-2026-11-25.md`、`docs/PM-REQUIREMENTS-2026-11-25-batch2.md`、`docs/PM-REQUIREMENTS-2026-11-25-batch3.md`
> 文档状态：待评审 / 待排期
> 适用范围：SQLite / MySQL（含 MariaDB）/ PostgreSQL；Redis 仅涉及对象树与通用界面，不新增 Redis 服务端能力
> 关联文档：`README.md`、`CHANGELOG.md`、`docs/BACKLOG.md`、`docs/ARCHITECTURE.md`

---

## 0. 本批结论

### 0.1 一句话目标

在「能查询、能编辑、能同步、能导出」之后，补齐**每天看数据、改数据、导数据、找对象时最频繁的四类摩擦**：在大结果里找不到值、宽表只能横向滚动、改错一步只能全部放弃、业务 Excel 进不来。本批全部落在现有架构内，不新增数据库驱动、不引入外部服务、不改变连接与权限模型。

### 0.2 需求清单（按价值优先级排序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 依赖 | 关系 / 去重 |
|---|---|---|---|---|---|---|
| 1 | E-01 | 结果集与表数据快速查找（匹配计数 / 高亮 / 跳转） | P0 | 1 个开发会话 | `ResultGrid.vue`、`TableDataView.vue`、`virtual-rows.ts` | 新增；不替代数据库 `WHERE` 筛选，不改变导出与结果范围 |
| 2 | E-02 | 行详情 / 表单视图（宽表逐字段查看与编辑） | P1 | 1.5 个开发会话 | `TableDataView.vue`、`ResultGrid.vue`、`CellValueDialog.vue` | 新增；扩展 B-16 / D-03 的单单元格值查看器到整行 |
| 3 | E-03 | 表数据待提交编辑的撤销 / 重做 | P1 | 1 个开发会话 | `TableDataView.vue` 的 `updates` / `pendingDeletes` / `pendingInserts` | 新增；同时修复「删除标记后恢复会丢失已有编辑」的既有缺陷 |
| 4 | E-04 | Excel（.xlsx）数据导入 | P1 | 1 个开发会话 | `ImportWizardDialog.vue`、`commands/import_wizard.rs`、`models.rs` | 新增格式；第二批 C2-05 明确只做 XLSX **导出**，与本项不重复 |
| 5 | E-05 | 对象树置顶常用对象 | P2 | 0.5–1 个开发会话 | `ObjectBrowser.vue`、`ObjectCategoryGroups.vue`、localStorage | 新增；不替代对象搜索、命令面板或全局对象搜索 |

> 「1 个开发会话」指 Claude 在一次连续实现中完成编码、`npm run check`、`cargo test` 与本文件列出的可自动化验收项；真实数据库手工验收可单独安排。

### 0.3 推荐实施顺序与理由

**实施顺序：E-01 → E-03 → E-02 → E-04 → E-05。**

- **E-01 最先做**：每个查询结果和每张表都受益；纯前端、无数据库写入、无新依赖，风险最低；先建立共享的「格式化文本 + 匹配列表 + 虚拟滚动定位」能力，E-02 的行详情搜索和后续表格能力可以直接复用。
- **E-03 紧随其后**：改动集中在 `TableDataView.vue` 的待提交状态，范围小；同时修掉一个真实的待提交编辑丢失问题。放在 E-02 之前，E-02 的行内编辑可以直接接入同一套撤销快照，避免后补。
- **E-02 第三**：价值高但改动面最大（抽屉、编辑、与待提交/撤销/行导航的交互），在前两项稳定后做更安全。
- **E-04 第四**：与前三项完全独立，可并行；唯一需要新增 Rust 依赖，先做技术验证（读取 xlsx、日期/数字/布尔转换、工作表选择）再接入向导。
- **E-05 最后**：纯前端小项，不影响核心数据工作流；即使本批时间不足，延后到下一批也不会阻塞其他需求。

若只能做一项，做 **E-01**；若只能做两项，做 **E-01 + E-03**。

### 0.4 竞品研究与差距结论（2026-11-26）

调研对象与边界：DBX（Apache-2.0，参考交互与公开文档）、DBeaver（Apache-2.0，参考公开 wiki）、Navicat（专有，只参考公开产品页 / 手册中的功能描述）。以下只比较**用户可见能力与交互**，不复制任何竞品源码、品牌资源或专有文案。

#### 4.1 DBX（重点参考）

- **数据表格**：对「当前已加载结果」提供表格搜索；冻结左侧 / 右侧列；拖拽列头重排；隐藏全 NULL / 空字符串列；标准视图与**转置视图**；单元格详情（长文本、JSON、二进制、日期）；多次执行保留运行记录并可**固定结果**；复制 / 导出覆盖 CSV、JSON、Markdown、XLSX、SQL INSERT/UPDATE。  
  来源：<https://dbxio.com/en/docs/data-grid>
- **对象浏览器**：层级导航 + 搜索、**固定常用对象**、网格 / 列表视图、多选与批量操作。  
  来源：<https://dbxio.com/en/docs/object-browser>
- **查询编辑器**：语义诊断、`Ctrl/Cmd+Click` 表名导航、执行前预览 `INSERT/UPDATE/DELETE` 的影响数据、在编辑器内读取未执行的 DDL 做补全。  
  来源：<https://dbxio.com/en/docs/query-editor>
- **表数据导入**：支持 CSV、TSV、JSON、Excel（`.xlsx` / `.xlsm` / `.xls`，可选择工作表）、SQL、Parquet；导入向导包含预览、字段映射、空值策略、错误继续 / 回滚、批大小。  
  来源：<https://dbxio.com/en/docs/table-import>

#### 4.2 DBeaver（公开 wiki / 文档）

- **Data Editor 左侧视图**：Grid、Text、Spatial、Chart、**Record**（行列转置）、JSON/XML（PRO）；右侧面板包含 **Value Viewer**、**References**（外键引用）、Metadata、Calc、Grouping；顶部有过滤字段和**历史前进 / 后退**；底部有新增 / 复制 / 删除行、首行 / 上一行 / 下一行 / 末行、抓取全部、总行数计算。  
  来源：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>
- **结果集导航**：键盘 Home/End/PgUp/PgDn/Ctrl+Home/Ctrl+End；按外键打开被引用表、按链接值打开目标表，并用前进 / 后退在导航历史中往返。  
  来源：<https://github.com/dbeaver/dbeaver/wiki/Navigation>
- 社区 issue 中用户明确把 **Record View（Form View）+ Value Panel** 作为日常录入 / 校验依赖，说明「表单 / 记录视图」不是冷门能力。  
  来源：<https://github.com/dbeaver/dbeaver/issues/41593>

#### 4.3 Navicat（公开产品页 / 手册）

- 表数据查看器提供 **Grid View** 与 **Form View** 两种模式；Grid View 支持一次编辑多条记录、变更高亮、状态栏；导入向导支持 Excel。  
  来源：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>、<https://www.navicat.com.cn/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

#### 4.4 差距表与决策

| 能力 | 竞品参考 | mydb 现状（2026-11-26 代码核对） | 本批决策 |
|---|---|---|---|
| 当前结果 / 当前页查找 | DBX 表格搜索、DBeaver 结果导航 | 无；只有服务端 `WHERE`（表数据）和手动滚动 | **E-01** |
| 行详情 / 表单视图 | DBeaver Record / Value / References、Navicat Form View | 只有单单元格 `CellValueDialog`；宽表需横向滚动 | **E-02** |
| 待提交编辑撤销 / 重做 | DBeaver / Navicat 数据编辑器的通用预期 | 只有「放弃全部」；删除标记后恢复还会丢失已有编辑 | **E-03** |
| Excel 导入 | DBX、Navicat 导入向导 | 仅 CSV / JSON 导入；XLSX 只支持导出 | **E-04** |
| 对象置顶 | DBX 对象浏览器「固定常用对象」 | 只有搜索和分类展开；无置顶 / 收藏 | **E-05** |
| 结果集图表 | DBeaver Chart 视图 | 无 | 本批不做，见 0.6 |
| SQL 语义诊断 | DBX 语义诊断、DBeaver / Navicat 实时错误提示 | 有方言高亮、补全、格式化；无诊断 | 下一批优先候选 |
| 列冻结 / 转置 | DBX 冻结列、DBeaver Record 视图 | 无 | 本批不做，先解决查找与行详情 |
| Redis 值搜索 / 键命名空间 / 批量键操作 | DBX Redis 工作台 | 已有 SCAN、TTL、重命名、string/hash/list/set/zset 编辑；无值搜索、批量删除、命名空间树 | 下一批候选 |
| 事务面板 / 执行计划对比 | DBeaver Transaction Log、计划视图 | 已有按查询标签的手动事务与状态栏、执行计划可视化；无全局事务面板 / 计划对比 | 本批不做 |
| 连接自动重连 | DBX 断线自动重连 | 连接失败后需手动重新打开；无自动重试 | 单独评估，写操作重试语义有风险 |
| 数据库结构对比 | DBX Schema 对比、Navicat 结构同步 | 已有结构同步、数据同步、结构差异 SQL；无独立并排 diff 视图 | 本批不做 |

**结论**：mydb 在连接、同步、备份、导入向导、SQL 片段、数据库文档等「重能力」上已经不弱；当前最大的日常缺口集中在**数据表格的浏览效率**和**数据编辑的安全感**，而不是继续堆连接类型或重型工具。本批 5 项都直接作用于每天打开次数最多的结果网格、表数据页、导入向导和对象树。

### 0.5 与已实现 / 已提案需求的去重

以下能力已经存在或已被前序批次覆盖，本批**不重复**：

- 结果导出 CSV / JSON / Markdown / SQL INSERT / XLSX；行级复制 INSERT / UPDATE / JSON；值查看器；虚拟滚动；表数据批量编辑、筛选构建器、多列排序、分页（B-02 / B-03 / B-16 / C-03 / C2-05 / D-03）。
- 导入向导的 CSV / JSON 预览、列映射、空值策略、错误策略、进度与取消（B-09）；本批 E-04 只新增 Excel 格式，不改已有流程。
- SQL 片段、格式化、EXPLAIN 可视化、查询构建器、数据库文档 / DBML、数据生成器、备份计划、用户 / 权限只读、服务器活动（D-01～D-08）。
- 多语句执行与多结果集、双查询分屏、独立查询窗口、SQL 文件执行、数据库 / 模式 SQL 导出（当前工作区未提交改动；E 批不与其冲突）。
- 对象树搜索、方向键导航、批量操作、统一标签、主题设置、命令面板、全局对象搜索（B-23～B-33 已落地部分）。
- 连接导入 / 导出、危险 SQL 确认、查询取消 / 超时、TLS 证书校验、SSH 私钥口令（B-05 / B-12 / B-14 / B-15 / B-19）。
- 未实现但不在本批：右侧信息 Dock（B-04 / B-24）、完整首页（B-26）、报表 / 计划任务（B-36）、工具栏自定义（B-38）。

### 0.6 本批明确不做（附评估结论）

| 方向 | 为什么不放进本批 |
|---|---|
| 结果集图表 / 可视化 | DBeaver 有 Chart 视图，但对当前主要用户（开发 / 运维 / DBA）不是最高频路径；需要图表渲染、坐标 / 聚合语义、导出图片和颜色可访问性设计，体量接近一个独立专项。先用 E-01 / E-02 解决「看清数据」，图表下一批单独立项。 |
| SQL 语义诊断 / 实时错误提示 | 价值高，但需要 `sqlparser` 之类的解析器或服务端 prepare，且 MySQL / PostgreSQL / SQLite 方言扩展、CTE、动态 SQL 容易误报；误报会直接伤害编辑体验。建议作为下一批第一候选，先做「语法级、可关闭、不阻塞执行」的 MVP。 |
| Redis 值搜索 / 批量键操作 / 命名空间树 | 生产 Redis 上值搜索会读取大量键，批量删除风险高；需要限流、预览、二次确认和 Cluster 约束设计。mydb 现有 Redis 编辑已较完整，单独排期更稳。 |
| 全局事务面板 | 现有事务按查询标签隔离、状态栏可见、切换连接 / 关闭标签自动回滚；全局面板的使用频率不足以压过本批 5 项。 |
| 执行计划对比 | 已有单次计划可视化；对比需要保存多份计划、归一化节点和差异 UI，偏性能调优专项。 |
| 连接自动重连 | 读查询可安全重试，但写语句在断线时可能已经在服务端生效，自动重试会造成重复写入；需要按语句类型、事务状态和用户确认设计，不能作为小修快速上线。 |
| 列冻结 / 转置视图 | DBX / DBeaver 都有，但对「找不到值、看不全一行」的痛点，E-01 / E-02 更直接；列冻结可与后续表格增强一起做。 |
| 独立数据库并排 diff | 现有结构同步已能对比并生成迁移 SQL；独立 diff 视图是展示层增强，优先级低于数据编辑。 |
| 报表 / 系统级计划任务 / 新驱动 / AI / MCP | 与「轻量、本地、无后台服务」定位冲突或属于长期架构项，继续留在 Backlog 的长期探索。 |

### 0.7 通用约束（适用于本批全部需求）

1. **不新增数据库驱动，不引入外部服务**；E-04 允许新增一个纯 Rust 的 Excel 读取 crate，但必须登记许可证并说明打包体积影响。
2. 前端继续使用 Vue 3 + TypeScript + CodeMirror 6，不引入 UI 组件库、图标库或图表库；图标优先复用 `src/components/Icon.vue` 已有的 `search` / `pin` / `undo` / `redo` / `view` / `close` / `chevron-*`。
3. 组件不得直接访问数据库；所有数据库操作走 `src/lib/api.ts` 的 `invoke` 封装；本批除 E-04 外不需要新的 Rust 命令。
4. 只读连接、视图、无主键表的既有写保护不得绕过；新增编辑入口必须复用 `canEdit` / `ensure_writable` / `sql_guard` 语义。
5. 本批的查找、撤销、行详情状态都只在当前组件 / 当前标签内存中，不写数据库、不写执行历史、不写入 `connections.json`；需要持久化的置顶数据使用 `mydb.*.v1` 命名空间并做损坏回退。
6. localStorage / 内存状态都必须有上限：搜索匹配不缓存全量小写副本超过当前结果；撤销栈最多 100 步；置顶最多每连接 / 库 / 模式 50 条。
7. 不改变现有复制 / 导出语义：搜索只高亮和跳转，不改变结果行集、排序、隐藏列或导出快照；撤销只作用于待提交缓冲区，不执行 SQL。
8. 所有新增交互可用键盘完成，并有 `aria-label` / `role` / live region；不得劫持 SQL 编辑器、输入框、对话框内的快捷键。
9. 代码风格遵循工作区约定：TS / Vue 2 空格，Rust 4 空格；完成后运行 `npm run check`、`cargo test --manifest-path src-tauri/Cargo.toml`（如涉及 Rust），并做本文件列出的手工验收。
10. 所有用户可见改动先写入 `CHANGELOG.md` 的 `[Unreleased]`；普通提交不提升版本号；README 只在行为或能力描述发生变化时同步。项目没有 CI，验收以本地检查与手工步骤为准。

---

## E-01 结果集与表数据快速查找（匹配计数 / 高亮 / 跳转）

- **优先级**：P0
- **状态**：已完成（2026-11-26：`grid-search.ts`、`GridFindBar.vue`、结果网格 / 表数据查找条、`scrollToRow`、匹配计数 / 高亮 / 循环跳转；`npm run check`、`npm run build` 通过，10,000 × 50 纯函数搜索实测约 46 ms；README / CHANGELOG 已更新）
- **预估**：1 个开发会话
- **范围**：前端（`ResultGrid.vue`、`QueryWorkspace.vue`、`TableDataView.vue`、`virtual-rows.ts`、新增 `lib/grid-search.ts`、`styles.css`）；不涉及后端、驱动和数据库写入。

### 用户问题 / 场景

1. 执行 `SELECT * FROM orders WHERE created_at >= ...` 返回 2,000 行，想找某个订单号 / 邮箱 / 错误信息；当前只能肉眼滚动，或者回到编辑器改写 SQL 加 `WHERE` 再执行。改 SQL 会丢失当前结果和排序，代价很高。
2. 表数据页虽然有服务端 `WHERE`，但只想在**当前页**快速定位一个值（例如核对刚导入的 100 行），不想构造 SQL 条件，也不想触发新的数据库查询。
3. 结果被排序 / 隐藏列后，用户需要在当前视图内确认某个值是否存在；DBX 的「表格搜索」和 DBeaver 的结果集查找都只作用于已加载数据，不重新查询数据库，这正是本项要补齐的轻量路径。

### 现状核对（基于当前代码）

- `ResultGrid.vue` 有 `sortedRows`、`visibleColumns`、`hiddenColumns`、虚拟滚动和 `formatCell`，但**没有查找状态、匹配计算或跳转能力**。
- `TableDataView.vue` 有 `appliedFilter`（服务端 WHERE）、`displayRows`（含待提交 overlay / 新增 / 删除状态）和虚拟滚动，但**没有当前页查找**；`filterDraft` 不能用于客户端查找，回车会执行数据库筛选。
- `src/lib/virtual-rows.ts` 只暴露 `scrollToTop()`，没有按行号定位；查找跳转需要新增 `scrollToRow(index)`，并考虑 sticky 表头和居中显示。
- `formatCell` 在 `ResultGrid.vue` 与 `TableDataView.vue` 各有一份，行为基本一致（NULL → `"NULL"`、对象 → 紧凑 JSON、其余 `String()`）；本项应抽出共享纯函数，避免查找文本与显示文本不一致。
- 结果集上限默认 1,000 行（最多 10,000）；表数据每页 50 / 100 / 200 / 500，搜索范围必须明确标注，不能让用户误以为搜索了全表。

### 期望行为（可验收细节）

1. **入口**
   - 结果网格：`⌘F` / `Ctrl+F` 在焦点位于结果网格或其工具栏时打开查找条；`QueryWorkspace` 结果标题栏增加「查找」图标按钮（`Icon name="search"`），点击后调用 `ResultGrid` 暴露的 `focusSearch()` 并聚焦输入框。
   - 表数据页：`⌘F` / `Ctrl+F` 在焦点位于表数据页时打开查找条；`data-filterbar` 增加「查找」按钮（与 WHERE 筛选并排，但视觉上区分），点击聚焦输入框。
   - 查找条位于结果标题栏与表格之间 / 表数据筛选条下方，包含：输入框、`Aa` 大小写开关、`N / M` 计数、上一个 / 下一个按钮、清除按钮、关闭按钮；宽度约 300–360px，窄窗口可换行但不遮挡表格。
2. **查找语义**
   - 字面量子串匹配，默认不区分大小写；`Aa` 切换为区分大小写；不支持正则（避免非法正则和灾难性回溯）。
   - 只搜索**当前已加载的数据**：结果网格搜索 `sortedRows`（当前排序、当前结果集，不重新查询）；表数据页搜索当前页的 `displayRows`（包含待提交修改后的显示值、新增行；已标记删除的行仍按显示内容参与，便于撤销前核对）。
   - 结果网格只搜索**当前可见列**；隐藏列不参与匹配，避免出现「有匹配但看不到」的困惑。切换隐藏列后自动重算；若当前匹配失效则回到第一个匹配。
   - 匹配文本使用与网格显示一致的格式化结果（`NULL`、对象 / 数组 JSON、数字 / 布尔 `String()`）；不把对象内部字段名单独拆开搜索。
   - 空字符串或仅空白视为未启用；输入变化后约 100ms 防抖计算，不产生后端请求。
3. **匹配与高亮**
   - 计数显示「第 M / N 个匹配（K 行）」，N 为匹配单元格数、K 为涉及的显示行数；无匹配显示「无匹配」；数据为空显示「当前没有可查找的数据」。
   - 所有可见匹配单元格内的命中子串用 `<mark>` 包住（同一单元格多处命中都要高亮）；当前匹配单元格使用更强的 `grid-match-active` 样式（外框 / 背景），并在滚动到该行后可见。
   - 上一个 / 下一个按「行优先、列次之」的顺序循环；到首 / 尾后环绕。`Enter` = 下一个，`Shift+Enter` = 上一个；输入框内方向键不移动网格。
   - 跳转使用新增的 `scrollToRow(index)`：把目标行滚到视口中间附近（避免被 sticky 表头遮挡），保持横向滚动位置；虚拟滚动未渲染的行在滚入视口后自动高亮。
4. **重置与生命周期**
   - 结果网格收到新的 `result`（重新执行 / 切换结果集）时清空查找词、匹配和当前项；隐藏列变化时保留查找词并重算。
   - 表数据页在切换表 / 连接 / 数据库 / 模式时清空；刷新、翻页、筛选、排序后保留查找词并重算（数据已变，但用户通常希望继续找同一个值）；若当前匹配不存在则回到第一个匹配或显示无匹配。
   - 关闭查找条时清除高亮并恢复焦点到打开前的控件（网格 / 表数据页 / 查找按钮）；`Esc` 关闭并清空；再次 `⌘F` 聚焦并全选当前词。
   - 待提交编辑提交 / 放弃、数据重新加载后，若行数变化，确保当前匹配索引不越界。
5. **快捷键作用域**
   - 仅当事件目标位于对应组件的根节点内，且不是 `input` / `textarea` / `select` / `contenteditable` / CodeMirror 编辑器 / 打开的模态框时，才拦截 `⌘F` / `Ctrl+F`。
   - 在 SQL 编辑器内按 `⌘F` 仍然打开 CodeMirror 查找面板，不触发网格查找；在连接搜索、对象搜索、筛选输入框内不劫持。
   - 原生菜单 / WebView 的查找行为不得被全局注册的快捷键抢占；必要时只提供工具栏按钮作为兜底。
6. **不影响数据与导出**
   - 查找不改变 `sortedRows`、`hiddenColumns`、`visibleColumns`、表数据 `appliedFilter`、总数或分页。
   - 复制 / 导出仍使用完整的当前结果 / 当前页（可见列 + 当前排序），不导出「仅匹配行」；查找条旁用一行小字提示「仅查找已加载数据，不改变查询与导出范围」。
   - 不写执行历史，不触发数据库查询，不改变待提交状态。
7. **状态与性能**
   - 10,000 行 × 50 列的当前结果，输入查找词后不出现明显卡顿（目标 < 150ms，按本机 M 系列芯片估算）；防抖期间保留上一次匹配或显示「查找中…」。
   - 匹配计算放在纯函数中，便于后续复用；不要在模板渲染中逐单元格重复计算整表。
   - 结果截断时提示「仅搜索当前已加载的 N 行（结果已截断）」；表数据页提示「仅搜索当前页 N 行，全表请用 WHERE」。

### 涉及文件 / 命令（可行落点）

- 新增 `src/lib/grid-search.ts`：
  - `formatGridCell(value: unknown): string`：从两个组件抽取，保持现有显示行为（NULL / 对象 / 数字 / 布尔）。
  - `findGridMatches(rows: unknown[][], columns: { index: number; name: string }[], query: string, options: { caseSensitive: boolean }): { rowIndex: number; colIndex: number; start: number; end: number }[]`：返回按行、列排序的匹配列表；不做 DOM 操作。
  - `highlightSegments(text: string, query: string, caseSensitive: boolean): { text: string; match: boolean }[]`：供模板安全渲染 `<mark>`，不使用 `v-html`。
- 修改 `src/lib/virtual-rows.ts`：新增 `scrollToRow(index: number, options?: { center?: boolean })`，设置 `container.scrollTop` 并更新内部 `scrollTop`；考虑 sticky 表头高度与 `rowHeight`。
- 修改 `src/components/ResultGrid.vue`：查找状态、查找条、匹配高亮、`focusSearch()`、`scrollToRow`、键盘作用域；在 `watch(() => props.result)` 中重置查找；`defineExpose` 增加 `focusSearch`。
- 修改 `src/components/QueryWorkspace.vue`：结果标题栏增加「查找」按钮（图标 + tooltip），调用 `resultGrid?.focusSearch()`；不改变现有导出 / 折叠按钮布局顺序，必要时把按钮放在行数提示之前。
- 修改 `src/components/TableDataView.vue`：查找条、匹配状态、`displayRows` 搜索、高亮、`scrollToRow`、键盘作用域；与 `filterDraft` / `appliedFilter` 明确分开，避免回车误触数据库筛选。
- 修改 `src/styles.css`：`.grid-findbar`、`.grid-find-input`、`.grid-find-count`、`.grid-match`、`.grid-match-active`、`.data-findbar` 等；使用现有颜色 token，兼容浅色主题。
- 更新 `CHANGELOG.md` 的 `[Unreleased]` 与 README「结果表格 / 表数据」条目（实现后）。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] 执行 `SELECT * FROM users ORDER BY id`，结果网格按 `⌘F` 打开查找条；输入 `an` 显示匹配计数，`Enter` 依次跳到匹配单元格，行自动滚入视口并被高亮；`Shift+Enter` 反向循环。
- [ ] 在结果表头隐藏包含当前唯一匹配的列后，匹配计数减少 / 变为无匹配；重新显示该列后匹配恢复；查找不改变隐藏列状态。
- [ ] 查询结果按某列降序排序后，查找顺序按屏幕行顺序（排序后）而不是原始返回顺序。
- [ ] 表数据页打开 `orders`，当前页 100 行；在查找条输入某个 `status` 值，命中当前页的行并高亮；`WHERE` 输入框内容、总行数、分页不变；按 `Enter` 不会执行数据库筛选。
- [ ] 表数据页双击修改一个单元格为唯一值后，查找该新值能命中（使用待提交 overlay 的显示值）；放弃修改后查找不再命中该值。
- [ ] 10,000 行结果的查找输入不卡死；结果截断时显示「仅搜索当前已加载的 N 行」提示；表数据页显示「仅搜索当前页」提示。
- [ ] 在 SQL 编辑器内按 `⌘F` 打开 CodeMirror 查找面板，不出现网格查找条；在连接 / 对象搜索框内按 `⌘F` 不劫持输入。
- [ ] `Esc` 关闭查找条、清除高亮并把焦点还给打开前的控件；重新打开时输入框全选上次查找词。
- [ ] 查找 / 跳转不新增 SQL 历史记录、不触发数据库请求；复制全部和「导出…」仍导出完整当前结果（而不是仅匹配行）。
- [ ] `npm run check` 通过；无新增依赖；浅色主题下查找条和高亮可读。

### 风险 / 限制

- 只搜索已加载数据，不代表全表 / 全结果；必须明确文案，避免用户误以为查找了数据库全部行。大表跨页查找请使用 `WHERE`。
- 结果网格默认只搜索可见列；隐藏列的匹配不可见，本批不支持「跳到隐藏列并自动显示」。
- 匹配文本基于显示值，日期 / 数字的格式化可能与数据库原始文本不同；例如数字 `1.0` 显示为 `1`，查找 `1.0` 可能不命中。后续如需精确匹配原始值，应增加「原始值」开关。
- 大结果集每次输入都重新扫描；防抖和纯函数足够，但不要在查找时复制整份行数据。
- `⌘F` 与 WebView / CodeMirror / 原生菜单存在冲突风险；实现时必须验证焦点作用域，必要时只保留工具栏按钮与 `Enter` 导航，不注册全局快捷键。
- 虚拟滚动定位需要处理 sticky 表头、折叠结果面板、窗口 resize；验收中必须包含 500 / 10,000 行两种规模。

---

## E-02 行详情 / 表单视图（宽表逐字段查看与编辑）

- **优先级**：P1
- **状态**：已完成（2026-11-26：`RowDetailPanel.vue`、表数据编辑 / 查询结果只读两种模式、上下行导航、复制值 / 整行 JSON、隐藏列标记、删除行禁用编辑、与待提交历史和撤销联动；headless 验证字段展示、编辑、设 NULL、删除 / 撤销、行导航与隐藏列）
- **预估**：1.5 个开发会话
- **范围**：前端（新增 `RowDetailPanel.vue`，修改 `TableDataView.vue`、`ResultGrid.vue`、`CellValueDialog.vue`、`styles.css`）；复用 `ColumnInfo`、`setCellPending`、`parseInput`、`CellValueDialog`；不涉及后端和数据库写入。

### 用户问题 / 场景

1. 一张订单表有 40 个字段，想核对某一行的收货地址、备注、支付状态和扩展 JSON；在网格里要反复横向滚动，列宽再宽也看不全，容易看错行。
2. 运维 / 客服场景经常「按主键找到一行，逐字段确认后改其中一个字段」；当前只能双击单个单元格，长文本 / JSON 还要再开值查看器，缺少「一条记录」的整体视图。
3. 查询结果也可能很宽（JOIN 多表、几十个字段），但结果网格没有表元数据，不能编辑；用户仍需要一个只读的整行详情，用来复制字段或查看完整值。
4. Navicat 的 Form View、DBeaver 的 Record / Value / References 面板都说明：开发者和数据维护人员需要「以记录为中心」的查看方式，而不是只以单元格为中心。

### 现状核对（基于当前代码）

- `TableDataView.vue` 已有 `columns: ColumnInfo[]`（字段名、类型、可空、主键、自增、默认值、注释）、`displayRows`、`updates` / `pendingDeletes` / `pendingInserts`、`setCellPending()`、`parseInput()`、`canEdit`、`CellValueDialog`；**没有整行详情状态或面板**。
- `ResultGrid.vue` 有 `sortedRows`、`visibleColumns`、`hiddenColumns`、单元格右键菜单和 `CellValueDialog`；**没有行详情**，且结果列只有列名，没有类型 / 主键信息。
- `CellValueDialog.vue` 已支持 JSON 格式化、原始文本、自动换行、复制、另存为、NULL / BLOB 占位，可直接复用为行详情里的「查看值」。
- `TableDataView.vue` 的 `canEdit` 已覆盖只读连接、视图、无主键表；行详情的新增编辑必须复用同一判断，不能另写一套。
- 当前单元格编辑在 `commitEdit()` 中调用 `setCellPending()`；新增行编辑在 `setCellPending()` 内按 `row.origin === null` 找到 `pendingInserts` 草稿。行详情编辑应走同一入口。

### 期望行为（可验收细节）

1. **入口**
   - 表数据页工具栏增加「行详情」按钮（`Icon name="view"`），在当前有选中行 / 当前行时可用；无数据时禁用并提示「当前没有可查看的行」。
   - 单元格右键菜单增加「查看行详情…」；点击后打开右侧抽屉并定位到该行，同时关闭单元格菜单。
   - 结果网格单元格右键菜单增加「查看行详情…」（只读），打开同一视觉形态的抽屉。
   - 抽屉关闭后不影响网格选中、滚动、待提交状态；`Esc` 关闭（正在编辑字段时先取消编辑，再按一次关闭）。
2. **布局**
   - 抽屉固定在数据区右侧，宽度约 340–380px；窄窗口（< 900px）改为覆盖式抽屉，不挤压网格到不可用宽度；不要求可拖拽调宽（复用现有布局不是本项目标）。
   - 顶部：标题「行详情」/ 表名或「查询结果」、行位置（`第 X / N 行` 或 `当前页第 X 行 · 全表第 Y 行`）、上一个 / 下一个按钮、关闭按钮。
   - 字段列表纵向滚动；每行显示字段名、类型徽标、`PK` / `AI` / `NULL` 徽标（有则显示）、值的预览（最多 2–3 行，超出省略）、操作按钮。
   - 长文本 / JSON / 二进制不直接展开大段内容：预览截断，提供「查看值…」打开 `CellValueDialog`；二进制显示 `<BLOB n bytes>` 占位。
   - 字段注释有值时以 `title` 或字段名下方小字显示；没有注释不占位。
3. **只读信息（两种来源）**
   - 表数据页：显示完整 `ColumnInfo` 信息（类型、可空、主键、自增、默认值、注释）；字段值来自 `displayRows`（包含待提交 overlay / 新增草稿）；已标记删除的行仍可查看，但编辑操作禁用并显示「该行已标记删除，撤销删除后可编辑」。
   - 查询结果：显示所有结果列（包括当前隐藏列，并标记「已隐藏」），不显示类型 / 主键；字段值来自 `sortedRows` 当前行；只提供复制 / 查看值 / 上一个 / 下一个，不提供编辑。
   - 结果集行详情不因隐藏列而缺字段；隐藏列在字段名旁显示淡色「已隐藏」标记。
4. **编辑（仅表数据页）**
   - 可编辑条件与网格一致：`canEdit` 为真、字段非二进制、行不是 `delete` 状态、不是视图、连接可写。只读连接 / 视图 / 不可编辑行显示锁标识和原因，不显示编辑按钮。
   - 点击值的「编辑」或双击值进入行内输入；输入框使用与网格相同的 `parseInput()` 语义（`NULL` 文本 → `null`，空字符串按可空规则，数字类型转数字）。
   - `Enter` 或失焦提交，`Esc` 取消；提交调用 `setCellPending()`，网格对应单元格立即更新，待提交计数增加，并产生一条 E-03 的撤销记录。
   - 「设为 NULL」仅在 `column.nullable` 时可用；点击后提交 `null`，不可空字段禁用并显示原因。清空输入不等于 NULL：遵循 `parseInput()` 的现有规则，UI 用占位提示说明。
   - 新增草稿行（`origin === null`）同样可编辑，提交后写入 `pendingInserts` 草稿；未提交前关闭面板不丢失。
   - 不直接调用 `updateTableRow` / `applyTableChanges`；所有修改仍走待提交缓冲区与「提交」按钮。
5. **行导航与生命周期**
   - 上一个 / 下一个在当前页 / 当前结果内移动，不自动跨页；到达首 / 尾时按钮禁用，不做环绕（避免误以为翻页）。
   - 打开时定位到触发行的显示索引；若该行随后被删除 / 撤销 / 数据刷新导致索引变化，按「保持当前行 origin（表）或主键 / 行索引（结果）」尽量恢复；找不到时关闭面板并提示「当前行已不存在」。
   - 切换表 / 连接 / 数据库 / 模式、翻页、筛选、排序、刷新时关闭面板；待提交状态保留。
   - 不因打开面板额外请求数据库；表数据页不需要读取外键 / 结构元数据（本项不做外键跳转）。
6. **复制与查看值**
   - 每个字段提供「复制值」：复制原始值（NULL 复制 `NULL` 文本，对象复制紧凑 JSON，二进制复制占位文本）；复制成功用组件现有 message / toast，不新增全局弹窗。
   - 「查看值…」复用 `CellValueDialog`，传入当前值、字段名、类型、行标签；大字段 / JSON 行为与现有值查看器一致。
   - 结果网格行详情中的「复制整行 JSON」可直接复用 `buildRowJson()`（包含全部列或当前可见列？本项规定：行详情复制整行 JSON 使用全部结果列，并在提示中说明；这与网格右键「复制为 JSON」的可见列语义不同，需要文案区分）。
7. **可访问性与视觉**
   - 抽屉 `role="complementary"` / `aria-label="行详情"`；字段列表可用 Tab 访问；编辑输入有 `aria-label`；上一个 / 下一个按钮有 `title`（含快捷键如无可不写）。
   - 字段状态（已修改 / 新增 / 待删除）有文字或图标，不只靠颜色；浅色 / 深色主题都可读。
   - 面板不遮挡结果导出 / 分页 / 提交按钮；窄窗口覆盖时提供遮罩点击关闭（编辑中点击遮罩先确认 / 取消编辑，不丢输入）。

### 涉及文件 / 命令（可行落点）

- 新增 `src/components/RowDetailPanel.vue`：
  - Props：`open`、`title`、`rowLabel`、`fields`（字段名、类型、可空、主键、自增、注释、值、是否隐藏、是否可编辑、编辑禁用原因）、`editable`、`readOnlyReason` 等。
  - Emits：`close`、`previous`、`next`、`edit`（字段索引 + 新值）、`view-value`（字段索引）、`copy-value`。
  - 组件只负责展示与输入，不直接持有 `updates` / `pendingInserts`；编辑提交由父组件回调 `setCellPending()`。
- 修改 `src/components/TableDataView.vue`：
  - 新增 `detailRowIndex` / `detailOpen` 状态；从工具栏、单元格菜单打开；计算传给 `RowDetailPanel` 的字段列表（复用 `displayRows`、`columns`、`isBinaryColumn`、`canEdit`）。
  - 编辑回调复用 `setCellPending()` / `parseInput()`；打开 / 关闭 / 行导航时先 `commitEdit()`，避免网格输入与抽屉输入同时存在。
  - 数据加载 / 分页 / 筛选 / 排序 / 表切换时关闭或校正面板。
- 修改 `src/components/ResultGrid.vue`：
  - 新增只读行详情状态；单元格菜单增加入口；字段列表来自 `props.result.columns` + 全部列值（含隐藏列），行标签使用 `sortedRows` 索引；复用 `CellValueDialog` 与 `buildRowJson`。
- 修改 `src/components/CellValueDialog.vue`（仅在必要时）：允许传入行标签 / 字段标签，复用现有格式化与另存为；不改变其默认行为。
- 修改 `src/styles.css`：`.row-detail-panel`、`.row-detail-field`、`.row-detail-value`、`.row-detail-badge`、窄屏覆盖层；使用现有 token。
- 如 E-03 已实现，行详情编辑必须调用同一 `recordPendingHistory()` 入口，确保一次字段编辑可撤销。
- 更新 `CHANGELOG.md` 的 `[Unreleased]` 与 README「表数据视图 / 查询结果」条目（实现后）。

### 验收标准（可直接在 `examples/demo.db` 与临时表执行）

- [ ] `users` 表选中第 1 行，工具栏「行详情」打开抽屉，显示 `id` / `name` / `email` / `city` / `created_at` 的字段名、类型、主键 / 可空标记和值；行位置为 `第 1 / N 行`。
- [ ] 在抽屉中把 `name` 改为新值并回车：网格对应单元格立即变化，待提交计数 +1；点击「提交」后数据库更新，刷新后仍为新值。
- [ ] 关闭抽屉后再打开，待提交修改仍显示在抽屉和网格中；「放弃」后恢复原值。
- [ ] 把可空字段设为 NULL：抽屉显示 `NULL`，网格同步；对 NOT NULL 字段「设为 NULL」禁用并显示原因。
- [ ] 创建 30+ 字段的临时表并插入一行：抽屉可纵向查看全部字段，不需要横向滚动；长文本 / JSON 预览截断，点「查看值…」可看到完整内容。
- [ ] 查询 `SELECT id, name, city FROM users` 后隐藏 `city`，右键「查看行详情…」：抽屉仍显示 `city` 字段并标记「已隐藏」；没有编辑按钮；复制整行 JSON 包含隐藏列并提示。
- [ ] 结果网格按 `name` 降序后，行详情的上一个 / 下一个顺序与屏幕行顺序一致。
- [ ] 表数据页把一行标记删除后打开行详情：显示删除状态，编辑按钮禁用；点击「撤销删除」后编辑恢复可用，且此前对该行的待提交编辑仍在（与 E-03 联动）。
- [ ] 新增一行草稿后打开行详情：可编辑草稿字段，网格同步；提交后插入数据库。
- [ ] 只读连接 / 视图 / 无主键表按现有 `canEdit` 规则显示禁用状态；不出现可直接写库的入口。
- [ ] 打开 / 切换 / 关闭行详情不产生新的数据库查询、不写执行历史；窄窗口下抽屉覆盖网格且可关闭，编辑内容不丢失。
- [ ] `npm run check` 通过；键盘 Tab 可遍历字段操作，Esc 行为符合本文件定义。

### 风险 / 限制

- 行详情只解决「看一条记录」和「改简单字段」，**不做外键跳转、引用表列表、行历史导航**（DBeaver 的 References / 导航历史）。这些依赖结构元数据与跨表标签栈，建议后续单独立项。
- 结果网格没有类型 / 主键元数据，行详情只能只读；不要根据列名猜测类型或可编辑性。
- 抽屉编辑与网格编辑必须共用 `setCellPending`，否则会出现两套待提交状态；实现时禁止在抽屉内直接拼 SQL 或调用写接口。
- 新增行 / 删除行的行索引是 `displayRows` 索引而非 `origin`；数据刷新后索引可能变化，必须重新定位或关闭，避免改错行。
- 超宽表格的字段列表较长，抽屉内部滚动即可，不要一次性渲染大量 `CellValueDialog`。
- 窄窗口覆盖层与现有右侧面板 / 结果分屏可能叠加；需确认 z-index 与焦点，不与 B-23 布局、查询分屏冲突。

---

## E-03 表数据待提交编辑的撤销 / 重做

- **优先级**：P1
- **状态**：已完成（2026-11-26：待提交快照历史、工具栏撤销 / 重做、`⌘/Ctrl+Z` 与重做快捷键、删除保留编辑缺陷修复；快捷键速查同步。headless 验证撤销 / 重做顺序、删除 → 恢复保留编辑、删除时提交只生成 DELETE，历史上限 100）
- **预估**：1 个开发会话
- **范围**：前端（`TableDataView.vue`、新增 `lib/pending-history.ts`、`styles.css`、可选 `lib/shortcuts.ts`）；不涉及后端、驱动和数据库写入。

### 用户问题 / 场景

1. 在表数据页连续改了 8 个单元格、新增 2 行、标记删除 1 行；第 9 步改错时，当前只有「放弃」会把全部待提交变更清空，无法只撤销最后一步，只能重新改一遍。
2. 误点「删除行」后，再用右键「撤销删除标记」恢复，之前对那一行的单元格编辑却丢失了——这是当前实现中的一个真实缺陷（`deleteRow()` 会删除 `updates[origin]`，`restoreRow()` 不会恢复）。
3. DBeaver / Navicat 等数据编辑器都提供编辑级的撤销 / 重做；mydb 已有「待提交缓冲区」，但没有操作历史，用户对批量编辑缺乏安全感。

### 现状核对（基于当前代码）

- `TableDataView.vue` 维护三份待提交状态：
  - `updates: Record<number, Record<string, unknown>>`（按原始行号存放列覆盖值）；
  - `pendingDeletes: Set<number>`（原始行号）；
  - `pendingInserts: InsertDraft[]`（含稳定 `id` 与按列索引的值数组）。
- `pendingCount` 统计「有更新且未删除的行 + 删除行 + 新增草稿」；`buildPendingChanges()` / `buildPendingSql()` 生成提交内容。
- 用户动作入口：`commitEdit()` → `setCellPending()`；`submitInsert()`；`cloneSelectedRows()`；`deleteRow()` / `restoreRow()`；`confirmBatchDelete()`；`applyBatchSet()`（批量设置值 / NULL）；行详情编辑（E-02 新增）。
- `deleteRow()` 在标记删除时会删除该行的 `updates`；`restoreRow()` 只移除 `pendingDeletes`，不恢复 `updates`。因此「编辑 → 删除 → 撤销删除」会静默丢失编辑。
- 当前没有 `undoStack` / `redoStack`；`discardPendingChanges()` 会清空全部待提交状态。全局编辑菜单的撤销 / 重做只作用于 CodeMirror 编辑器，不作用于表数据。

### 期望行为（可验收细节）

1. **历史模型**
   - 每个表数据标签维护独立的内存历史：`undoStack`、`redoStack`，最多 100 步；保存的是**待提交状态快照**（`updates`、`pendingDeletes`、`pendingInserts` 的副本），不是 SQL 或数据库行。
   - 每次用户动作前记录快照；一次动作只产生一条历史。批量设置 10 行、批量删除 5 行、克隆 3 行都只算一步。
   - 新动作发生后清空 `redoStack`；撤销后可以重做；重做后再编辑则清空旧 redo。
   - 快照复制规则：`updates` 复制外层对象（现有编辑逻辑是替换 overlay 对象，不原地改）；`pendingDeletes` 新建 `Set`；`pendingInserts` 必须**逐条复制 `values` 数组**，因为当前 `setCellPending()` 对新增草稿是原地写 `values[colIndex]`，只复制外层数组会让撤销失效。值本身是字符串 / 数字 / 布尔 / null，不需要结构化克隆。
2. **记录动作**
   - 单元格编辑提交（`commitEdit` / 行详情编辑）；
   - 新增行提交（`submitInsert`）；
   - 克隆选中行为新增草稿（`cloneSelectedRows`）；
   - 标记删除 / 撤销删除（`deleteRow` / `restoreRow`，包括从右键菜单触发）；
   - 批量设置值 / NULL（`applyBatchSet`）；
   - 批量删除（`confirmBatchDelete`）。
   - 纯 UI 动作（选中行、展开抽屉、翻页、筛选、排序、刷新）不进入历史。
3. **撤销 / 重做行为**
   - 撤销：把当前待提交状态替换为上一个快照；网格、待提交计数、行详情（若打开）同步更新；不执行 SQL、不新增历史、不改变数据库。
   - 重做：恢复被撤销的快照；若撤销后又有新编辑，则重做栈已清空。
   - 撤销到空状态时 `pendingCount` 为 0，工具栏待提交区隐藏；行详情若指向已不存在的草稿 / 行，关闭或校正。
   - 撤销 / 重做不应破坏当前页码、筛选、排序、滚动位置（除非当前行不在视口，保持即可）。
4. **入口与快捷键**
   - 工具栏待提交区增加两个图标按钮：`undo` / `redo`，带 `title`（含快捷键）与 `aria-label`；无历史时禁用。
   - 快捷键：`⌘Z` / `Ctrl+Z` 撤销，`⌘⇧Z` / `Ctrl+Shift+Z` 重做；Windows / Linux 额外支持 `Ctrl+Y` 重做（与现有 `src/lib/shortcuts.ts` 的平台显示一致）。
   - 仅在焦点位于表数据页且不在输入框 / 单元格编辑器 / 批量设置对话框 / 筛选输入框 / 模态框内时拦截；否则让浏览器 / CodeMirror / input 自行处理文本撤销。
   - SQL 编辑器中的 `⌘Z` 仍然撤销编辑器文本，不能被表数据页全局监听抢占。
5. **删除恢复缺陷修复**
   - 标记删除时**保留**该行的 `updates` 覆盖值（`buildPendingChanges()` 已跳过删除行，不会把更新写入 SQL）；或者保存 `updatesBeforeDelete` 并在恢复时还原。推荐前者，逻辑更简单。
   - **单行 `deleteRow()` 与批量 `confirmBatchDelete()` 都必须遵守同一规则**：不得为了「看起来干净」而删除 `updates[origin]`；批量删除后若单行撤销删除（`restoreRow`），此前编辑必须恢复。
   - `restoreRow()` 后，之前对该行的单元格编辑重新出现在网格和行详情中，`pendingCount` 恢复为「更新 + 新增 + 其他删除」的正确数量。
   - 提交时删除行仍然只生成 DELETE，不生成 UPDATE；撤销删除后提交才包含 UPDATE。
   - 这条修复必须有独立验收用例，不能只靠撤销 / 重做间接覆盖。
6. **清空与边界**
   - 提交成功、放弃全部、切换表 / 连接 / 数据库 / 模式、翻页 / 筛选 / 排序 / 刷新（这些操作会先要求提交或放弃）之后清空历史；用户取消确认框时保留历史。
   - `discardPendingChanges()` 清空待提交状态与历史 / 重做栈；`pendingCount` 归零。
   - 只读连接、视图、无主键表没有可编辑动作，按钮禁用；不因历史为空而报错。
   - 历史不持久化，关闭标签 / 重启后不恢复；不写入 localStorage。
7. **与行详情联动（若 E-02 已实现）**
   - 行详情中的字段编辑与网格编辑走同一历史动作；撤销后抽屉显示旧值；当前行被撤销掉时关闭抽屉。
8. **视觉与文案**
   - 撤销 / 重做按钮放在「N 项待提交」旁，保持紧凑；按钮禁用状态有明显对比度；操作后可用现有 message bar 提示「已撤销上一步」/「已重做」，或不提示但按钮状态更新（二选一，验收以状态为准）。
   - 不新增全局 toast 系统。

### 涉及文件 / 命令（可行落点）

- 新增 `src/lib/pending-history.ts`（纯函数，不 import Vue）：
  - 定义快照类型（`updates`、`deletes`、`inserts`）。
  - `snapshotPendingState(...)`、`clonePendingState(...)`、`pushHistory(stack, snapshot, limit)`、`popHistory(...)` 等 helper；组件只负责在动作前调用。
  - 如不想新增文件，也可在 `TableDataView.vue` 内实现，但独立文件更便于后续复用与测试。
- 修改 `src/components/TableDataView.vue`：
  - 增加 `undoStack` / `redoStack`、`recordHistory()`、`undo()`、`redo()`、`resetHistory()`；在所有用户动作入口前调用 `recordHistory()`。
  - `deleteRow()` 不再删除 `updates[origin]`；`restoreRow()` 自然恢复显示；检查 `pendingCount` / `buildPendingChanges()` 对删除行的过滤。
  - 键盘监听与按钮；`onBeforeUnmount` 清理。
  - `submitPendingChanges()` 成功、`discardPendingChanges()`、`load()`、表 / 连接切换时 `resetHistory()`。
- 修改 `src/lib/shortcuts.ts` 与 `ShortcutHelpDialog.vue`（可选但推荐）：补充表数据撤销 / 重做的快捷键说明；确保不与全局菜单 `edit-undo` / `edit-redo` 冲突。
- 修改 `src/styles.css`：待提交区的 icon button 间距、禁用态。
- 更新 `CHANGELOG.md` 的 `[Unreleased]` 与 README「表数据视图」条目（实现后）。

### 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] 修改 `users` 第 1 行 `name`，再修改第 2 行 `city`；点撤销一次，第 2 行恢复、第 1 行修改保留；再撤销一次，第 1 行也恢复；重做两次按相反顺序恢复。
- [ ] 连续编辑同一单元格两次（先 A 后 B），撤销一次回到 A，再撤销回到原值；重做回到 A / B。
- [ ] 选中 3 行批量设置同一个值，撤销一次三行全部恢复（只算一步，不是三步）。
- [ ] 新增一行并填写字段，撤销后草稿行消失；重做后草稿行及字段值恢复。
- [ ] 克隆 2 行为新增草稿，撤销一次两行都消失；重做后都恢复。
- [ ] **缺陷修复**：编辑 `users` 第 1 行 `name` → 标记删除该行 → 点击「撤销删除标记」→ 此前的 `name` 编辑仍在网格 / 行详情中，待提交计数正确；提交后生成 UPDATE 而不是丢失编辑。
- [ ] 批量标记删除 3 行后撤销一次，3 行都恢复原状态；若其中一行此前有编辑，编辑也恢复。
- [ ] 提交成功后撤销 / 重做按钮禁用，历史清空；点击「放弃」后同样清空；切换表后不残留上一张表的历史。
- [ ] 在 SQL 编辑器按 `⌘Z` 只撤销编辑器文本，不撤销表数据；在单元格编辑输入框内按 `⌘Z` 撤销输入文本，不触发待提交历史。
- [ ] 只读连接 / 视图 / 无主键表的撤销 / 重做按钮禁用；操作不写入数据库、不新增执行历史。
- [ ] 连续 120 次单元格编辑后，历史最多保留 100 步，应用不卡顿、不崩溃；撤销最早可撤销的一步仍然正确。
- [ ] 行详情（若已实现）打开时撤销，字段值与待提交计数同步；当前行被撤销掉时面板关闭或校正，不显示错误行。
- [ ] `npm run check` 通过。

### 风险 / 限制

- 快照是待提交状态，不是数据库事务；撤销不会恢复已经提交的数据。UI 文案应避免让用户误以为可以撤销已提交变更。
- 大表 + 大量待提交行时，每步快照会复制 `updates` / `pendingInserts` 容器；100 步上限下内存可控，但不要在每次 keystroke 记录，只在动作提交时记录。
- 删除行保留 `updates` 后，`buildPendingChanges()` 必须继续跳过删除行的更新，否则会生成 UPDATE + DELETE 两条语句；验收必须覆盖。
- 与全局编辑菜单的 `edit-undo` / `edit-redo` 可能重复；本项只在表数据页内拦截，不注册全局 accelerator，避免抢走 CodeMirror。
- 与 E-02 行详情编辑共用历史入口；如果两项分开实现，E-02 必须先接入 `recordHistory()`，否则行详情编辑不可撤销。

---

## E-04 Excel（.xlsx）数据导入

- **优先级**：P1
- **状态**：已完成（2026-11-26：`calamine` 读取、工作表选择、数字 / 布尔 / 日期时间 / 公式缓存值转换、重复 / 空表头唯一化、50 MB / 200k 行 / 2k 列限制；导入向导扩展格式与工作表下拉；Rust 单测覆盖双工作表、类型、重复表头）
- **预估**：1 个开发会话（含 Rust 读取与测试）
- **范围**：后端（`src-tauri/Cargo.toml`、`commands/import_wizard.rs`、`models.rs`）；前端（`ImportWizardDialog.vue`、`lib/types.ts`、`lib/api.ts`）；复用现有导入向导、进度、取消、错误策略与只读拦截。

### 用户问题 / 场景

1. 业务同事给的数据通常是 `.xlsx`（订单、客户、库存、对账表），当前 mydb 导入只接受 CSV / JSON；用户必须先用 Excel「另存为 CSV」再导入，容易遇到编码、分隔符、日期格式、前导零丢失等问题。
2. 一个工作簿可能有多张表（如「订单」「明细」「字典」）；需要选择工作表、预览前 20 行、映射列后再写入，而不是盲目导入第一张表。
3. 数据库客户端竞品（DBX、Navicat）都把 Excel 导入作为导入向导的标准格式；mydb 已支持 XLSX 导出，导入侧是明显的不对称缺口。第二批 C2-05 明确只做 XLSX 导出，因此本项不是重复需求。

### 现状核对（基于当前代码）

- `ImportWizardDialog.vue` 是当前实际使用的导入向导（`App.vue` 只挂载 `ImportWizardDialog`；`ImportDataDialog.vue` 是旧实现，不在主流程）。它支持 `format: "csv" | "json"`，文件选择过滤器为 `CSV / JSON`，CSV 支持分隔符、表头、预览、列映射、空值策略、错误策略、进度与取消。
- 后端 `commands/import_wizard.rs`：
  - `read_tabular(path, format, options)` 只分派 `"json"` 和默认 `"csv"`；
  - `read_csv` 使用 `csv` crate，自动检测分隔符，返回 `TabularFile { columns, rows, delimiter }`；
  - `read_json` 支持对象数组 / `{rows:[...]}`；
  - `preview_import_file` / `execute_import_file` / `cancel_import` 已具备完整流程。
- `ImportPreview` / `ImportOptions`（Rust `models.rs` 与 TS `types.ts`）没有工作表字段；`detectedDelimiter` / `format` 是现有返回字段。
- `rust_xlsxwriter` 已用于 XLSX 导出；**没有 Excel 读取依赖**。
- 导入执行按行 INSERT，已有事务 / 遇错停止 / 跳过错误行三种策略；只读连接在命令层拒绝；这些都不需要重写。

### 期望行为（可验收细节）

1. **入口与格式**
   - `ImportWizardDialog` 文件选择过滤器扩展为 `CSV / JSON / Excel`，扩展名 `csv`、`json`、`xlsx`（可选 `xls`，但 MVP 验收只要求 `.xlsx`）。
   - `format` 类型扩展为 `"csv" | "json" | "xlsx"`；按扩展名自动识别，用户也可手动切换（切换后重新预览）。
   - 表数据页「导入数据…」、对象右键「导入数据」、表数据工具栏继续走同一向导；只读连接入口保持禁用 / 后端拒绝。
2. **工作表选择**
   - 预览阶段返回工作簿的工作表名列表；默认选择第一张**非隐藏**工作表；若只有隐藏工作表，选择第一张并提示。
   - 工作簿有多张表时，预览面板显示「工作表」下拉；切换后重新读取预览、重置列映射并按同名字段自动匹配（匹配不到则留空，用户手动映射）。
   - 单张工作表时不显示下拉，避免噪声。
   - 工作表名按原始名称显示（保留中文、空格、特殊字符）；不修改源文件。
3. **表头与数据范围**
   - 沿用 `hasHeader`：开启时第一行非空行作为列名；关闭时生成 `column_1`、`column_2`…。
   - 空行：整行为空的记录跳过；行尾空列按表头宽度补齐；数据行比表头宽时，额外列生成 `column_N` 并在预览中可见。
   - 表头为空 / 重复：生成稳定的显示名（如 `column_2`、`名称 (2)`），并保证列映射以**列索引**为键，不因重复名称覆盖；预览中保留原值供用户辨认。
   - 不要求支持多行表头、合并单元格语义、数据透视表；合并单元格按左上角值读取，其余为空，界面需在帮助文案中说明。
4. **单元格类型转换**
   - 文本：原样；数字：`42` 不写成 `42.0`，小数保留合理精度，不使用科学计数法（除非绝对值极大 / 极小，按 `f64` 常规显示）；布尔：`true` / `false`；空单元格：空字符串，再由「空字符串按 NULL」策略决定写 `NULL` 还是 `''`。
   - 日期 / 时间：转换为 `YYYY-MM-DD` 或 `YYYY-MM-DD HH:MM:SS` 文本（按单元格是否有时间部分决定）；不把 Excel 序列号原样写入。时区按 Excel 本地值处理，不做 UTC 转换。
   - 公式单元格：读取 Excel 缓存的计算结果（calamine 的能力），不计算公式；界面提示「公式按缓存值导入」。
   - 错误单元格（`#N/A`、`#DIV/0!` 等）：按空字符串导入，并在导入汇总中记录警告数量（不阻断）；如果实现成本高，至少按空字符串处理并提示。
   - 前导零：Excel 数字单元格本身不保留前导零；提示用户将列设为文本。以文本存储的 `00123` 原样保留。
5. **向导复用**
   - 预览前 20 行、列映射、忽略列、必填字段校验、空值策略、错误策略（全部回滚 / 遇错停止 / 跳过错误行）、进度 / 取消、失败行号与错误信息，全部复用现有 CSV/JSON 流程，不另做一套 Excel 专用导入逻辑。
   - 预览不写入数据库；执行阶段仍按行 INSERT，使用现有事务策略。
   - 文件读取失败（损坏、加密、非 xlsx、无工作表）返回可读错误，不创建空预览、不写数据库。
6. **限制与安全**
   - 文件大小上限 50 MB；单工作表最多 200,000 个数据行、2,000 列；超过时拒绝并提示拆分为 CSV 或分批导入。上限值写入 UI 提示和错误信息。
   - 不执行宏、不读取外部链接、不计算公式、不修改源文件；`.xlsm` 默认按普通工作簿读取值（如实现简单），但不保证宏行为。
   - 加密 / 受密码保护的工作簿给出明确「暂不支持加密工作簿」错误，不尝试破解。
   - 只读连接、系统表等既有保护不变。
7. **兼容性**
   - CSV / JSON 导入路径保持完全不变；旧的 `ImportPreview` / `ImportOptions` JSON（没有 `sheets` / `sheet` 字段）仍能反序列化（`serde(default)` / TS 可选字段）。
   - 非 Excel 格式不显示工作表控件；`detectedDelimiter` 对 Excel 返回空字符串。
   - `execute_import_file` 接收 `sheet` 时按所选工作表读取；未传时使用默认第一张表，保证向后兼容。

### 涉及文件 / 命令（可行落点）

- `src-tauri/Cargo.toml`：新增 `calamine`（MIT，纯 Rust；版本以实现时 crates.io 最新稳定为准）。如 crate 默认 feature 过大，只启用 xlsx 所需 feature；记录许可证与编译体积影响。
- `src-tauri/src/models.rs`：
  - `ImportPreview` 增加 `sheets: Vec<String>`、`sheet: Option<String>`（`#[serde(default)]`）。
  - `ImportOptions` 增加 `sheet: Option<String>`（`#[serde(default)]`）。
  - 如需警告数量，增加 `warnings: Vec<String>` 或 `warning_count: usize`（可选，不阻断）。
- `src-tauri/src/commands/import_wizard.rs`：
  - `read_tabular` 增加 `"xlsx" => read_xlsx(path, options)`。
  - `read_xlsx`：打开工作簿、列工作表、按 `options.sheet` 选择、读取首行 / 数据行、转换单元格为字符串；复用 `TabularFile`。
  - 将 `sheets` / 选中 `sheet` 放入 `ImportPreview`；`preview_import_file` 与 `execute_import_file` 都使用同一读取函数，避免预览和执行使用不同工作表。
  - 单元测试：用 `rust_xlsxwriter` 在临时目录生成含两张工作表、表头、数字、日期、布尔、空值、重复表头的工作簿，断言 `read_tabular` 的列 / 行 / 工作表选择 / 类型转换；测试不访问网络。
- `src/lib/types.ts`：`ImportPreview` 增加 `sheets?: string[]`、`sheet?: string | null`；`ImportOptions` 增加 `sheet?: string | null`；`format` 类型扩展。
- `src/lib/api.ts`：无需新命令，仅调整 `previewImportFile` / `executeImportFile` 的 `format` 类型或注释；`ImportOptions` 自动携带 `sheet`。
- `src/components/ImportWizardDialog.vue`：
  - `format` 联合类型、文件选择过滤、自动识别 `.xlsx`；
  - 工作表下拉与切换重预览；
  - 重置 / 自动匹配映射（按列索引或唯一显示名）；
  - 文件信息行显示「Excel · 工作表名 · N 列」；
  - 空值、表头、错误策略保持现有控件；Excel 时隐藏分隔符控件。
- 旧 `src/components/ImportDataDialog.vue` 不在主流程；本项不扩展它，避免两套向导（如后续清理可单独处理）。
- 更新 `CHANGELOG.md` 的 `[Unreleased]`、README「导入导出」条目与许可证 / 第三方依赖说明。

### 验收标准

- [ ] 新建 `import-demo.xlsx`：工作表 `users` 含表头 `id, name, email, active, joined_at` 和 3 行数据；工作表 `notes` 含 2 列。导入向导选择该文件后自动识别为 Excel，默认预览 `users`，显示 5 列 / 3 行。
- [ ] 切换到 `notes` 工作表：预览列 / 行更新，映射重置并按同名字段自动匹配；切回 `users` 后再次正确预览。
- [ ] 关闭表头选项后预览显示 `column_1`…；开启后第一行作为列名；空行被跳过。
- [ ] 数字 `42` 导入后为 `42`（不是 `42.0`）；日期列导入为 `YYYY-MM-DD` 或带时间格式；布尔列为 `true` / `false`；空单元格按「空字符串按 NULL」设置写入 NULL 或 `''`。
- [ ] 重复表头 `name, name` 或空表头时，映射界面能区分两列，导入后数据不串列；不会因对象键覆盖丢失一列。
- [ ] 预览映射时忽略一列、把另一列映射到不同字段，导入后只写入映射列；必填字段未映射时在执行前阻止并提示。
- [ ] 故意制造一行主键冲突，选择「全部回滚」时数据库无新增；选择「跳过错误行」时其余行写入，汇总显示成功 / 失败数量与行号。
- [ ] 只读连接选择 Excel 后执行被拒绝，数据库无变化；取消导入后不残留半截数据（按既有事务策略）。
- [ ] 超过 50 MB 的文件、超过 200,000 行的工作表被拒绝并提示；损坏 / 加密工作簿给出可读错误，不崩溃。
- [ ] CSV / JSON 导入原有验收用例仍通过（表头、分隔符、JSON 对象数组、空值、错误策略、进度取消）。
- [ ] `npm run check`、`cargo test --manifest-path src-tauri/Cargo.toml` 通过；新增 Rust 单元测试覆盖工作表选择与类型转换；无网络请求。

### 风险 / 限制

- `calamine` 会增加编译时间与二进制体积；只启用必要 feature，并在 README / 许可证说明中登记。若体积影响不可接受，退回 CSV-only 或改为前端 SheetJS（但会引入更大的 JS 包，不推荐）。
- Excel 的日期 / 时间、时区、1900/1904 日期系统、公式缓存值、合并单元格、隐藏行列都有边界；MVP 只保证常见值类型与单行表头，复杂模板需先整理。
- 大工作簿一次性读取到内存；50 MB / 200,000 行上限是保护，不是性能承诺。超大文件仍建议 CSV。
- 加密 / 宏 / 外部链接不处理；`.xls`（旧二进制格式）如 `calamine` 支持可顺带支持，但不在验收硬性范围。
- 前导零与数字格式由 Excel 存储方式决定，mydb 无法恢复已被 Excel 去掉的零；UI 必须提示。
- 预览与执行必须使用同一工作表与同一表头判断，否则会出现「预览 3 行、导入 0 行」的偏差；实现时把选择固化到 `ImportOptions` 并在执行前回显。

---

## E-05 对象树置顶常用对象

- **优先级**：P2
- **状态**：已完成（2026-11-26：`lib/object-pins.ts`、常用对象区、右键置顶 / 取消、按连接 + 库 / 模式持久化、搜索过滤、失效隐藏、50 上限与损坏回退；headless 验证置顶区、搜索与边界）
- **预估**：0.5–1 个开发会话
- **范围**：前端（`ObjectBrowser.vue`、`ObjectCategoryGroups.vue`、新增 `lib/object-pins.ts`、`styles.css`）；不涉及后端、驱动和数据库写入。

### 用户问题 / 场景

1. 一个 MySQL 库有 300 张表，日常只反复看 `orders`、`users`、`payments` 三张；对象树按分类展开、滚动、搜索，每次都要重新找，搜索词还要手动清空。
2. PostgreSQL 有多个模式，切到 `public` 后常用表在 `public`，切到 `audit` 后常用表又不同；用户希望「当前库 / 模式下的常用对象」固定在对象树顶部。
3. DBX 对象浏览器明确提供「固定常用对象」；mydb 已有对象搜索和命令面板，但缺少「固定的、一眼可见的快捷入口」。这不是替代搜索，而是把高频对象从层级树中提出来。

### 现状核对（基于当前代码）

- `ObjectBrowser.vue` 有连接侧栏筛选、对象搜索、数据库 / 模式切换、分类折叠、定位当前对象、全部折叠；`ObjectCategoryGroups.vue` 负责表 / 视图 / 例程 / 触发器等分类、行渲染、右键菜单、多选批量操作。
- 当前没有置顶 / 收藏状态，也没有独立的「常用对象」区域；对象行由 `ObjectCategoryGroups` 渲染，分类默认折叠。
- 对象点击最终通过 `select` 事件传到 `App.vue` 的 `openTableTabAt()`；置顶行应复用同一事件，不新增打开逻辑。
- `Icon.vue` 已有 `pin` 图标；右键菜单已有子菜单 / 危险色 / 快捷键样式可复用；localStorage 已有多个 `mydb.*.v1` 键的读写模式。

### 期望行为（可验收细节）

1. **置顶 / 取消置顶**
   - 表 / 视图行右键菜单增加「置顶到常用对象」；已置顶对象显示「取消置顶」。
   - 表 / 视图行在 hover 时可显示一个淡色 pin 图标按钮（可选，不作为硬性验收）；点击不触发行打开。
   - 置顶成功用现有 toast / message 提示「已置顶 orders」；取消提示「已取消置顶」。
   - 只读连接、Redis 连接是否允许置顶？置顶是本地 UI 操作，不写数据库，**允许**；Redis 没有该对象树分类，不显示入口。
2. **常用对象区域**
   - 在对象树搜索框下方、分类列表上方增加「常用对象」分组，带 pin 图标、对象数量和折叠箭头；默认展开（若用户折叠则记住状态）。
   - 每个置顶行显示对象图标 / 类型徽标（表 / 视图）、名称、当前活动高亮；点击与普通对象行一致（打开数据标签 / 切换当前表）；右键复用现有对象菜单，但至少包含「取消置顶」。
   - 置顶行是**快捷方式**，不改变分类中的原对象；分类里仍显示同一对象（可能需要视觉上标记 pin，但不是必须）。
   - 置顶区域最多显示 50 条；超过时提示「最多置顶 50 个对象」，阻止继续添加而不是静默丢弃。
   - 当前库 / 模式没有置顶对象时，显示一行淡色提示「右键对象 → 置顶到常用对象」；不占用过多高度，可折叠。
3. **作用域与持久化**
   - 置顶键必须包含：连接 id + 数据库（MySQL / SQLite）+ 模式（PostgreSQL）+ 对象类型 + 对象名；连接名 / 库名 / 模式名变化不应串数据。
   - 存储键 `mydb.objectPins.v1`，结构包含 `version: 1` 与按连接 / 范围分组的数组；读取时校验类型、截断到 50、忽略损坏项，整体损坏时回退为空，不弹错误。
   - 切换数据库 / 模式 / 连接后，只显示当前范围下的置顶对象；切回原范围后恢复。
   - 对象被删除或重命名后：刷新对象列表时不再显示失效置顶（不自动删除存储项，避免误删后重命名恢复；可在存储项上做「失效」隐藏）。在对象树刷新后若置顶项不在 `tables` 中，直接隐藏，不报错。
   - 连接被删除时，可清理该连接的置顶数据（best-effort）；不要求迁移。
4. **搜索与折叠**
   - 对象搜索框有内容时，「常用对象」区域只显示匹配的置顶项；没有匹配则隐藏该区域，避免占位。
   - 搜索不改变置顶集合；清空搜索后恢复完整置顶列表。
   - 「全部折叠」只折叠分类，不折叠「常用对象」（或单独保留其折叠状态）；「定位当前对象」如果目标在置顶区，滚动到置顶行并高亮，否则按现有分类定位逻辑。
5. **键盘与可访问性**
   - 置顶行可点击、可聚焦（`tabindex="0"`），Enter / Space 打开；右键菜单可用键盘方向键 / Enter / Esc（复用现有菜单）。
   - 置顶行不参与分类对象的 Shift / ⌘ 多选范围，避免与批量操作混淆；点击置顶行不改变批量选择。
   - `aria-label` 包含「常用对象」；pin 按钮有 `aria-pressed`（若实现 hover 按钮）。
6. **无后端与无数据影响**
   - 置顶只读写 localStorage，不触发 `listTables` / 任何数据库命令；不写 `connections.json`、不写钥匙串、不写执行历史。
   - 置顶不改变对象树数据、分类计数、对象排序或批量操作结果。
7. **视觉**
   - 「常用对象」标题栏与现有分类标题一致（字号、颜色、折叠箭头）；pin 图标使用 accent 色但不刺眼；置顶行高度与普通表行一致。
   - 窄窗口 / 对象树 rail 折叠时不显示置顶区；展开后恢复。
   - 浅色 / 深色主题均可读。

### 涉及文件 / 命令（可行落点）

- 新增 `src/lib/object-pins.ts`：
  - 类型 `PinnedObject { connectionId, database, schema, kind, name }`；
  - `loadPins()` / `savePins()` / `togglePin()` / `isPinned()` / `pinsForScope()`；localStorage 键 `mydb.objectPins.v1`，版本校验、上限、损坏回退。
  - 纯函数，不依赖 Vue；连接 / 库 / 模式用字符串键归一化，避免 `undefined` / `null` 混淆。
- 修改 `src/components/ObjectCategoryGroups.vue`：
  - Props：`pinned?: PinnedObject[]`（当前范围）、`activeTable` 已有、`search` 已有；Emits：`toggle-pin`、`select`（复用）。
  - 在分类列表前渲染「常用对象」区；行渲染复用现有表格行样式 / 图标 / 右键菜单；右键菜单增加置顶项。
  - 搜索时过滤置顶列表；无匹配隐藏区域。
  - 置顶区与分类区的选择 / 批量操作状态隔离。
- 修改 `src/components/ObjectBrowser.vue`：
  - 计算当前 scope key（connectionId + database + schema），调用 `pinsForScope()`；把 `pinned` 传给 `ObjectCategoryGroups`；监听 `toggle-pin` 调用 `togglePin()`。
  - 「定位当前对象」逻辑增加置顶区匹配；搜索清空 / 切换范围时更新。
  - 折叠状态与现有分类折叠分开存储（可复用 `collapsed` 的键前缀 `pinned:`，或新增 ref）。
- 修改 `src/styles.css`：`.object-pinned-section`、`.object-pinned-row`、`.pinned-empty`、`.pin-icon`；与 `.object-category` 视觉一致。
- 可选：`src/lib/shortcuts.ts` 不新增全局快捷键；置顶是鼠标 / 右键操作。
- 更新 `CHANGELOG.md` 的 `[Unreleased]` 与 README「对象浏览器」条目（实现后）。

### 验收标准（可直接在 `examples/demo.db` 与 MySQL / PostgreSQL 测试库执行）

- [ ] SQLite `demo.db`：右键 `orders` → 「置顶到常用对象」，对象树顶部出现「常用对象」区，包含 `orders`；点击置顶行打开 `orders` 数据标签，与分类中的行行为一致。
- [ ] 再次右键 `orders` → 「取消置顶」，置顶区移除；分类中的 `orders` 仍在，分类计数不变。
- [ ] 置顶 `users` 后折叠 / 展开对象树、切换查询标签、重启应用，置顶仍在；`localStorage` 中 `mydb.objectPins.v1` 有对应记录。
- [ ] MySQL 两个数据库分别置顶不同表；切换数据库只显示当前库的置顶；切回后恢复；PostgreSQL 两个模式同样。
- [ ] 删除一张已置顶的表并刷新对象树：置顶区不再显示该表，不报错、不影响其他置顶；存储中的失效项不导致 `JSON.parse` 失败。
- [ ] 对象搜索输入 `ord`：置顶区只显示匹配的 `orders`；清空搜索后恢复全部置顶；搜索不改变置顶集合。
- [ ] 置顶 51 个对象时第 51 个被拒绝并提示上限 50；删除 / 取消后可继续添加。
- [ ] 置顶行点击不改变分类多选状态；批量选择 / 批量删除 / 导出 DDL 的行为与未置顶时一致。
- [ ] 只读连接仍可置顶 / 取消（本地 UI 操作），不触发只读错误；Redis 连接不显示置顶入口。
- [ ] 手工写入损坏的 `mydb.objectPins.v1`（非法 JSON / 错误结构）后启动，对象树正常显示且置顶为空，不白屏、不弹错误。
- [ ] `npm run check` 通过；无新增后端请求、无新增依赖。

### 风险 / 限制

- 置顶行与分类行是同一对象的两个入口，活动高亮、定位、键盘导航可能出现重复；MVP 规定置顶行不参与分类多选与 Shift 范围选择，定位优先置顶，避免歧义。
- 对象重命名后旧置顶项失效；本项不做自动迁移（可用「刷新后隐藏」处理）。后续可增加「清理失效置顶」。
- 置顶键依赖连接 id；导入 / 导出连接后 id 可能变化，置顶不会跟随。这是本地 UI 状态，不写入连接配置，作为已知限制说明。
- 置顶区会增加对象树高度；默认展开但可折叠，且无置顶时只显示一行提示，避免挤占分类空间。
- 跨窗口（独立查询窗口）不共享置顶；独立查询窗口隐藏对象树，不受影响。

---

## 附录 A：与 Backlog / 前序批次的收口建议

| 本批编号 | 关系 | Backlog / 前序建议 |
|---|---|---|
| E-01 | 新增，无直接对应 | 可在 Backlog 新增 E-01；与 B-25 命令面板、全局对象搜索不重复（那是找对象，这是找数据） |
| E-02 | 扩展 B-16 / D-03 的单元格值查看器 | B-16 已由 D-03 覆盖；E-02 是整行视图，不能标记 B-16 为「已覆盖 E-02」 |
| E-03 | 新增，并修复 C-03 待提交缓冲区的缺陷 | 在 C-03 的「已完成」备注中补充「撤销 / 重做由 E-03 接手」；不要删除 C-03 历史 |
| E-04 | 扩展 B-09 / C2-05 的导入向导 | B-09 的 CSV / JSON 已完成；C2-05 明确只做 XLSX 导出，E-04 只新增 Excel 导入 |
| E-05 | 新增，对应 DBX 对象浏览器置顶 | 与 B-25 / D-07 全局搜索不重复；不改变 B-30 批量操作 |

**不建议在本批顺手改的旧项**：B-04 / B-24（右侧信息 Dock）、B-26（完整首页）、B-36（报表 / 计划任务）、B-38（工具栏自定义）。它们仍是有效需求，但与本批「数据浏览 / 编辑 / 导入 / 对象导航」的主题不同，混做会扩大回归面。

## 附录 B：实现与验收约定

1. **交付顺序**：按 E-01 → E-03 → E-02 → E-04 → E-05 实现；每项独立提交 / 至少在 `CHANGELOG.md` 的 `[Unreleased]` 中独立成条，便于回滚与逐项验收。
2. **先核对再编码**：每项开始前先核对当前 `App.vue` / 组件 / `api.ts` / `models.rs` 的实际结构；本文件的「现状核对」基于 2026-11-26 工作区，若实现侧已重构（例如多结果集 / 查询窗口进一步改造 `QueryWorkspace`），以实际代码为准，但不得改变需求的用户可见行为。
3. **禁止绕过保护**：E-02 / E-03 的所有编辑都必须进入待提交缓冲区并由「提交」统一写库；不得在抽屉 / 撤销逻辑中直接调用 `updateTableRow` / `applyTableChanges`。只读连接、视图、无主键表的限制保持现有 `canEdit` 语义。
4. **共享工具**：E-01 的 `formatGridCell` / `findGridMatches` 供 E-02 的行详情复用；E-03 的快照 helper 供行详情编辑复用。避免每个组件各写一套。
5. **性能上限**：查找按当前结果 / 当前页计算，10,000 行内无可感知卡顿；撤销栈 100 步；置顶 50 条 / 范围；Excel 50 MB / 200,000 行。超过上限必须提示，不得静默截断或卡死。
6. **手工验收数据库**：优先 `examples/demo.db`（SQLite）；E-02 / E-03 建议补充 MySQL / PostgreSQL 临时表；E-04 需要真实 `.xlsx` 文件；E-05 需要至少一个多库 / 多模式连接。若某驱动无法现场验证，在交付说明中列出未验证项，不要标记为「全部通过」。
7. **文档**：实现后更新 `README.md` 对应功能条目、`CHANGELOG.md` `[Unreleased]`、`docs/BACKLOG.md` 状态；版本号保持不变，除非用户明确要求「更新版本」。
8. **不新增依赖**：E-01 / E-02 / E-03 / E-05 不新增 npm / Rust 依赖；E-04 仅允许 `calamine`（MIT），并说明体积与许可证影响。禁止引入 UI 组件库、图表库、图标库、状态管理库。
9. **可访问性**：所有新增按钮 / 输入 / 抽屉有可读标签，键盘可操作，焦点可见；查找计数、导入进度、置顶提示使用 `aria-live` / `role="status"`。
10. **不修改用户数据**：任何搜索、详情、撤销、置顶操作都不得写入数据库、连接配置、钥匙串或执行历史；E-04 只在用户确认「执行导入」后写入目标表。

## 附录 C：参考资料

### DBX（Apache-2.0，仅参考功能与公开文档）

- 数据表格（表格搜索、冻结列、转置视图、单元格详情、多结果 / 固定结果）：<https://dbxio.com/en/docs/data-grid>
- 对象浏览器（搜索、固定常用对象、批量操作）：<https://dbxio.com/en/docs/object-browser>
- 查询编辑器（语义诊断、Ctrl/Cmd+Click 导航、执行前预览变更）：<https://dbxio.com/en/docs/query-editor>
- 表数据导入（CSV / JSON / Excel / 工作表选择 / 映射 / 错误策略）：<https://dbxio.com/en/docs/table-import>
- 仓库主页（Apache-2.0）：<https://github.com/t8y2/dbx>

### DBeaver（Apache-2.0，仅参考公开 wiki）

- Data Editor（Grid / Text / Record / Value / References / Metadata、历史导航、过滤字段）：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>
- 结果集导航（键盘导航、外键跳转、导航历史）：<https://github.com/dbeaver/dbeaver/wiki/Navigation>
- Record View / Value Panel 使用场景（社区 issue）：<https://github.com/dbeaver/dbeaver/issues/41593>

### Navicat（专有软件，仅参考公开产品页 / 手册）

- Data Editor for RDBMS（Grid View / Form View、变更高亮）：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>
- Navicat 17 手册（表查看器、导入向导）：<https://www.navicat.com.cn/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

### 本仓库

- `README.md`：当前功能基线、已知限制。
- `CHANGELOG.md`：`[Unreleased]` 与 0.11.0 变更。
- `docs/BACKLOG.md`：B / C / D 系列历史状态与长期探索。
- `docs/ARCHITECTURE.md`：Handlers / 命令 / 驱动 / 前端分层约束。

> 竞品功能仅用于确认「用户是否需要」与「交互应该怎样」，不得复制其源码、品牌图标、截图或专有文案。mydb 保持独立实现与 Apache-2.0 许可。

---

## 附：E 批变更记录

- 2026-11-26：创建本文件；确定 E-01～E-05，完成 DBX / DBeaver / Navicat 差距核对、与 C / D 批次去重、本批不做项与验收约定。

---
---

# F 批：宽表阅读、关联导航与快速录入（F-01～F-05）

> 批次定位：E 批解决「找值、看一行、撤销、Excel 导入、置顶对象」；F 批继续解决**宽表横向阅读、已返回结果的二次筛选、表间关联跳转、从 Excel / 剪贴板批量录入、SQL 执行范围**这五类每天都会遇到的摩擦。
> 实施前提：F 批是 E 批之后的后续批次，**不替代 E-01～E-05**。E 批仍未实现时，应先完成 E 批；若只能先做一项，E 批做 E-01，F 批做 F-01。
> 编号说明：本批使用 **F-01～F-05**，接续 E 批。状态全部为「待办」，不在本文件中宣称已实现。
> 竞品边界：DBX（Apache-2.0）与 DBeaver（Apache-2.0）只参考公开文档与交互；Navicat 为专有软件，只参考公开产品页 / 手册的功能描述，不复制源码、品牌资源、截图或专有文案。

## 0. 本批结论

### 0.1 一句话目标

在已有查询、编辑、同步、导出、备份能力之上，把**结果网格和表数据网格**从「能看」提升到「能高效比较、过滤、跳转和录入」，并把 SQL 编辑器的**执行范围**变得可预期，减少误执行与手工复制粘贴。

### 0.2 需求清单（按实施顺序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 依赖 | 关系 / 去重 |
|---|---|---|---|---|---|---|
| 1 | F-01 | 结果网格与表数据的列冻结（宽表阅读） | P0 | 1 个开发会话 | `ResultGrid.vue`、`TableDataView.vue`、`styles.css`、新增 `lib/frozen-columns.ts` | 新增；与 E-02 行详情互补，不替代行详情 |
| 2 | F-02 | 查询结果本地筛选（列条件 / 计数 / 清除） | P1 | 1 个开发会话 | `ResultGrid.vue`、`QueryWorkspace.vue`、结果导出快照 | 新增；与 E-01 查找不同：查找只高亮，筛选减少可见行；表数据已有服务端 WHERE，本项只做查询结果 |
| 3 | F-04 | 从剪贴板粘贴多行数据到表（待提交） | P1 | 1 个开发会话 | `TableDataView.vue`、新增 `PasteRowsDialog.vue`、`lib/clipboard.ts`、待提交缓冲区 | 新增；与 E-04 Excel 文件导入不同：剪贴板文本进入待提交，不直接写库 |
| 4 | F-03 | 外键关联跳转与反向引用（引用行 / 被引用记录） | P1 | 1.5–2 个开发会话 | 新增 `list_scope_foreign_keys` 命令、`TableDataView.vue`、`App.vue` 表标签筛选 | 新增；E-02 明确不做外键跳转，由本项接手；与 ER 图不同，本项直接打开数据 |
| 5 | F-05 | SQL 执行范围控制（当前语句 / 整个标签） | P1 | 0.5–1 个开发会话 | `SqlEditor.vue`、`QueryWorkspace.vue`、`App.vue`、`SettingsDialog.vue`、`lib/sql-statements.ts` | 增强现有执行链路；与 E-03 撤销、B-07 脚本模式不重复 |

> 「1 个开发会话」指 Claude 在一次连续实现中完成编码、`npm run check`、`cargo test` 与本文件列出的可自动化验收项；真实数据库手工验收可单独安排。
> 编号按主题分配，不等同于实施顺序；本文件章节顺序即推荐实施顺序，因此 F-04（剪贴板）章排在 F-03（外键）章之前。

### 0.3 推荐实施顺序与理由

**实施顺序：F-01 → F-02 → F-04 → F-03 → F-05。**

- **F-01 最先做**：纯前端、无数据库写入、无新依赖；宽表识别主键 / 名称是最高频的阅读摩擦，做完立刻可见；先落地 `frozen-columns.ts` 的偏移计算与 sticky 样式，F-02 的筛选条也在同一结果面板，后续改动更容易复用。
- **F-02 第二**：仍然集中在 `ResultGrid.vue`，与 F-01 共享列宽 / 隐藏列 / 排序 / 虚拟滚动状态；先冻结再筛选，可以一起验收横向与纵向两类「看不见」问题。若 E-01 已经落地，F-02 直接复用其匹配与格式化工具。
- **F-04 第三**：与网格状态解耦，改动集中在 `TableDataView.vue` + 新对话框；剪贴板读取权限（`clipboard-manager:allow-read-text`）已经存在，不需要新依赖，可独立并行。放在 F-03 之前，是因为它只走待提交缓冲区，风险低于新增后端元数据命令。
- **F-03 第四**：唯一需要新增后端命令和三个驱动实现的项；涉及外键元数据、复合键、同范围限制与表标签筛选，改动面最大，放在前面几项稳定后做。
- **F-05 最后**：会改变 `⌘/Ctrl+Enter` 的默认行为，需要设置项、快捷键与文案一起收口；放在最后便于单独做回归与发布说明。

若只能做一项，做 **F-01**；若只能做两项，做 **F-01 + F-02**。

### 0.4 竞品研究与差距结论（2026-11-26，第二批）

调研对象与边界：DBX（Apache-2.0，公开文档）、DBeaver（Apache-2.0，公开 wiki）、Navicat（专有，只参考公开产品页 / 手册中的功能描述）。只比较用户可见能力与交互，不复制实现。

#### 4.1 DBX（重点参考）

- **列冻结**：支持冻结左侧或右侧列，横向滚动时保持可见；列宽、列顺序、冻结状态和显示偏好可跨会话保留，并提供重置布局。  
  来源：<https://dbxio.com/en/docs/data-grid>
- **本地值筛选**：网格提供 Grid Search（只搜索已加载结果）、Local value filter（按值 / 列表 / 范围筛选，不发起数据库查询）、数据库侧 `WHERE`、`ORDER BY` 和当前页排序；文档明确提醒本地筛选 / 排序不会减少数据库返回的数据量。  
  来源：<https://dbxio.com/en/docs/data-grid>
- **执行范围**：`Mod+Enter` 默认执行**当前语句或选中 SQL**；没有选区时按光标位置决定执行目标，官方快捷键文档说明没有默认的「执行全部 SQL」快捷键。  
  来源：<https://dbxio.com/en/docs/keyboard-shortcuts>、<https://dbxio.com/en/docs/query-editor>
- **多结果 / 固定结果**：查询结果可以保留多次运行，或固定某个结果用于比较；刷新 / 排序 / 筛选在存在未保存编辑时可能被阻止。  
  来源：<https://dbxio.com/en/docs/data-grid>

#### 4.2 DBeaver（公开 wiki）

- **Data Editor**：提供 Grid / Text / Record / Value / References / Metadata 视图；References 面板展示当前记录通过外键关联的**被引用行**，以及引用当前主键的**引用行**，可以直接打开关联表。  
  来源：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>、<https://github.com/dbeaver/dbeaver/wiki/References-Panel>
- **外键导航**：结果集 / 数据编辑器支持按外键跳转，并在导航历史中前进 / 后退；别名列或缺失外键元数据会禁用导航。  
  来源：<https://github.com/dbeaver/dbeaver/wiki/Navigation>
- **剪贴板粘贴**：数据编辑器支持 TAB 分隔的剪贴板数据；Advanced Paste 提供「Insert multiple rows」「Ignore quotes」「Insert NULLs / NULL value mark」等选项，适合从 Excel 粘贴整块数据。  
  来源：<https://github.com/dbeaver/dbeaver/wiki/Data-Viewing-and-Editing>

#### 4.3 Navicat（公开产品页 / 手册）

- **列冻结**：右键列头 → Display → Freeze Selected Column，可冻结一列或多列；冻结列移到最左侧，取消冻结通过 Unfreeze Columns；列可拖拽重排、调整宽度、显示 / 隐藏。  
  来源：<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>
- **筛选与排序**：右键单元格 Filter → Field Value、Custom Filter，以及 Filter & Sort 面板；支持 =、!=、<、>、Contains、Begins with、Ends with、Is null、Is empty、Between 等条件，并可勾选启用 / 禁用条件。  
  来源：<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>
- **网格粘贴**：剪贴板数据按回车分行、制表符分列，可在 Grid View 选中目标单元格后 `Ctrl+V` 或右键 Paste；Navicat 也提供 Table Profile 保存筛选、排序、冻结 / 隐藏列、列顺序与宽度。  
  来源：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>、<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

#### 4.4 差距表与本批决策

| 能力 | 竞品参考 | mydb 现状（2026-11-26 代码核对） | 本批决策 |
|---|---|---|---|
| 列冻结 / 宽表横向阅读 | DBX 左 / 右冻结；Navicat 冻结选中列 | 结果网格支持列宽、隐藏列、排序；表数据支持列宽、排序、筛选；两者都**没有冻结列** | **F-01**，先做左侧冻结 |
| 查询结果本地筛选 | DBX Local value filter；Navicat Filter & Sort | 查询结果只有客户端排序与隐藏列，没有按列条件筛选；表数据有服务端 WHERE / 筛选构建器 | **F-02**，只做查询结果、只作用于已加载行 |
| 外键关联导航 | DBeaver References / 外键跳转 | ER 图与结构页展示外键，但数据页不能从 `user_id` 跳到 `users`，也不能反查引用行 | **F-03**，同库 / 模式 MVP |
| 多行剪贴板粘贴 | DBeaver Advanced Paste；Navicat 网格 Paste | 有 CSV / JSON /（E-04）Excel 文件导入，但没有剪贴板粘贴；当前只能逐行新增 | **F-04**，进入待提交缓冲区 |
| 执行当前语句 | DBX `Mod+Enter`；DBeaver 区分执行语句与脚本 | `⌘/Ctrl+Enter` 无选区时执行**整个标签**；虽有 `currentStatement()` / `statementAtCursor()`，但只用于解释和格式化 | **F-05**，默认当前语句 + 可切整个标签 |
| 结果固定 / 多次运行对比 | DBX 多结果 / Pin result | 每次执行替换上一次结果；多语句有「结果 N」，但不能固定或对比多次运行 | 下一批候选；本批先做执行范围，避免结果模型大改 |
| SQL 语义诊断 / 实时错误 | DBX 语义诊断；DBeaver / Navicat 错误提示 | 有方言高亮、补全、格式化、EXPLAIN；没有语法诊断 | 下一批候选；需要解析器与方言扩展，误报风险高，本批不做 |
| 转置视图 / Table Profile | DBX 转置；Navicat Table Profile | 无转置；表数据分页大小有记忆，筛选 / 排序 / 列宽不形成可保存配置 | 下一批；先完成 F-01 / F-02 的列状态，再做 Profile |
| Redis 值搜索 / 命名空间树 / 批量键操作 | DBX Redis 工作台 | Redis 已有 SCAN、TTL、重命名、string/hash/list/set/zset 编辑 | 下一批 / Redis 专项；需要限流与批量删除确认 |
| 连接自动重连 | DBX 断线自动重连 | 连接失败后手动重新打开 | 单独评估；写语句断线重试可能重复写入 |

**结论**：mydb 的「重能力」（结构 / 数据同步、备份计划、导入向导、SQL 片段、文档、服务器活动）已经比较完整；当前最影响日常使用的是**网格的阅读与二次加工能力**，以及**从关系数据直接跳到关联数据**。本批 5 项都落在现有网格、表数据、剪贴板和 SQL 编辑器上，不新增驱动、不引入外部服务。

### 0.5 与已实现 / 已提案需求的去重

以下能力已经存在或已被前序批次覆盖，F 批不重复：

- 查询结果导出 CSV / JSON / Markdown / SQL INSERT / XLSX；行级复制 INSERT / UPDATE / JSON；值查看器；虚拟滚动；多结果集；双查询分屏；独立查询窗口（B-02 / B-03 / B-16 / C2-05 / D-03 / 当前未提交改动）。
- 表数据分页、服务端 WHERE、筛选构建器、多列排序、待提交缓冲区、批量设置 / 克隆 / 删除、数据生成器（C-03 / D-04 / 当前未提交改动）。
- 对象树搜索、命令面板、全局对象搜索、对象置顶（E-05，未实现）；本批 F-03 是数据页外键跳转，不是对象搜索。
- ER 图布局、搜索定位、SVG 导出（C-08）；结构页外键列表与新增 / 删除外键（C-04）；F-03 只消费外键元数据打开数据，不修改 ER / 结构。
- CSV / JSON 导入向导、事务 / 错误策略、进度 / 取消（B-09）；E-04 Excel 文件导入（未实现）；F-04 是剪贴板文本进入待提交，不写入数据库。
- SQL 文件打开 / 保存 / 执行、多语句脚本执行、查询取消 / 超时、EXPLAIN、SQL 片段、格式化（B-05 / B-06 / B-07 / B-20 / D-01 / C2-01）；F-05 只调整执行范围，不改变脚本执行器。
- 连接导入 / 导出、危险 SQL 确认、TLS、SSH 口令、备份计划、服务器活动（B-12 / B-14 / B-15 / B-19 / D-06 / C-07）。

**E 批关系**：E-01 查找、E-02 行详情、E-03 撤销、E-04 Excel 导入、E-05 对象置顶均与本批不重复；F-02 的筛选可以与 E-01 查找叠加，F-04 与 E-04 共用列映射 / 类型转换的交互经验，但写入路径不同。

### 0.6 本批明确不做（附评估结论）

| 方向 | 为什么不放进本批 |
|---|---|
| 右侧列冻结 / 双侧冻结 | DBX 支持左右冻结，但左侧冻结（主键 / 名称 / 状态）覆盖绝大多数宽表阅读场景；右侧冻结需要另一套偏移与滚动方向处理，放到后续增强。 |
| OR 条件组 / 嵌套筛选 | F-02 先做 AND 多条件，足够覆盖「状态 + 金额 + 城市」；OR 组需要条件树 UI 与 SQL 语义展示，单独排期。 |
| 粘贴更新已有行 / 区域粘贴覆盖 | Navicat / DBeaver 支持选中单元格粘贴覆盖，但会绕过待提交的逐行定位与无主键限制；本批只做**追加新增行**，避免误覆盖。 |
| 跨库 / 跨模式外键跳转 | MySQL 外键可跨库、PostgreSQL 可跨模式；当前表标签与 `read_table_rows` 以连接 + 当前库 / 模式为上下文，跨范围需要切换连接上下文并可能触发事务保护，放到后续。 |
| 外键导航前进 / 后退历史 | DBeaver 有导航历史；本批源表标签保持打开，用户切回即可，先不做全局历史栈，避免与标签恢复 / 焦点逻辑纠缠。 |
| 结果固定 / 多次运行对比 | 涉及结果模型、内存上限、标签持久化与导出对象；价值高但改动大，下一批单独立项。 |
| SQL 语义诊断 / 自动补全增强 | 需要解析器或服务端 prepare，方言扩展与误报处理成本高；本批先让执行范围可预期，诊断另立专项。 |
| 转置视图 / 列拖拽重排 | 与 F-01 同属列布局，但转置会改变行列语义、右键菜单与导出；列重排还要处理持久化与冻结偏移，先不做。 |
| Redis 键命名空间 / 批量删除 | 需要限流、预览与 Cluster 约束；Redis 已有基础运维能力，单独排期。 |
| 连接自动重连 / 全局事务面板 / 计划任务 | 与「轻量、本地、无后台服务」定位或写操作安全冲突，继续留在 Backlog。 |

### 0.7 通用约束（适用于 F 批全部需求）

1. **不新增数据库驱动，不引入外部服务**；F-01 / F-02 / F-04 / F-05 不新增 npm / Rust 依赖；F-03 只新增一个 Tauri 命令，不新增 crate。
2. 前端继续使用 Vue 3 + TypeScript + CodeMirror 6；不引入 UI 组件库、图标库或图表库。图标优先复用 `src/components/Icon.vue` 已有的 `pin` / `filter` / `paste` / `play` / `chevron-*`；缺失时按现有 SVG 风格补 1–2 个。
3. 组件不得直接访问数据库；数据库操作走 `src/lib/api.ts` 的 `invoke` 封装。F-01 / F-02 / F-05 不需要新 Rust 命令；F-04 复用现有待提交与 `apply_table_changes`；F-03 新增 `list_scope_foreign_keys`。
4. 只读连接、视图、无主键表的既有写保护不得绕过；F-04 只在待提交缓冲区追加，提交仍走现有 `ensure_writable` / `sql_guard`；F-03 只读跳转不写库。
5. 任何新增状态都有上限：筛选条件最多 5 个；粘贴单次最多 5,000 行 / 5 MB；待提交新增行总数超过 10,000 时提示；冻结列状态最多保留 200 个表；外键元数据缓存 60 秒且最多 20 个范围。
6. 不改变现有复制 / 导出语义之外的行为：F-01 冻结列仍参与复制 / 导出；F-02 在筛选激活时复制 / 导出使用筛选后的行，并在导出对话框明确提示；F-04 不自动提交；F-05 只改执行范围，不改变单条语句的执行结果。
7. 所有新增交互可用键盘完成，并有 `aria-label` / `role` / live region；不得劫持 SQL 编辑器、输入框、对话框内的快捷键；F-02 的筛选快捷键不得与 E-01 查找、CodeMirror 查找冲突。
8. 代码风格遵循工作区约定：TS / Vue 2 空格，Rust 4 空格；完成后运行 `npm run check`、`cargo test --manifest-path src-tauri/Cargo.toml`（涉及 Rust 时必跑），并做本文件列出的手工验收。
9. 所有用户可见改动先写入 `CHANGELOG.md` 的 `[Unreleased]`；普通提交不提升版本号；README 只在行为或能力描述变化时同步。项目没有 CI，验收以本地检查与手工步骤为准。
10. 本批不修改 `src-tauri/src/db/driver.rs` 的既有 SQL 执行语义；F-03 的新增元数据查询必须只读，不得在用户表上执行 DDL 或写入。

---

## F-01 结果网格与表数据的列冻结（宽表阅读）

- **优先级**：P0
- **状态**：已完成（2026-11-26：`frozen-columns.ts`、结果 / 表数据列头冻结、sticky 偏移、边界阴影、pin 标记、表数据按表持久化、分屏不写入、至少保留一列与隐藏列剪枝；headless 验证结果冻结 3 列、表数据恢复 / 取消冻结）
- **范围**：前端（`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`、`src/styles.css`、新增 `src/lib/frozen-columns.ts`）；不涉及后端和数据库。
- **用户问题 / 场景**：30+ 字段的宽表或 `SELECT *` 结果横向滚动后，主键、名称、状态等关键列被滚出视口；想对比两行的同一字段，只能来回滚动或把结果导出到 Excel。E-02 的行详情能看「一行」，但解决不了「多行之间对照」。
- **现状核对（基于 2026-11-26 代码）**：
  - `ResultGrid.vue` 有 `columnWidths`、`defaultWidths`、`hiddenColumns`、`sortIndex`、`visibleColumns`、`sortedRows` 和虚拟滚动；表格滚动容器是 `.result-grid-wrap`，行号列固定 46px，表头可排序、可右键、可拖拽列宽。没有冻结列。
  - `TableDataView.vue` 有 `columnWidths`、`columns`、`sorts`、服务端筛选、虚拟滚动、行选择复选框、行内编辑和待提交状态；表头可排序、拖拽列宽，但没有隐藏列，也没有冻结列。
  - 两处表格都使用 `<colgroup>` + inline width；`styles.css` 负责 sticky 表头 / 行号 / 滚动条。新增冻结列必须与现有列宽、虚拟滚动、右键菜单和主题变量一起工作。
- **期望行为（可验收细节）**：
  1. **入口**：结果网格与表数据的列头右键菜单新增「冻结到此列」；已冻结时新增「取消冻结」。菜单项显示目标列名，例如「冻结到此列（包含 id、name）」。结果网格的列头右键菜单已有「复制列名 / 只看此列 / 隐藏此列 / 显示所有列 / 恢复自动列宽 / 导出结果…」，冻结项放在「复制列名」之后、隐藏类操作之前；表数据列头右键菜单新增同样的冻结项（如果没有列头右键菜单，则新增一个最小列头菜单，只含冻结 / 取消冻结）。
  2. **冻结语义**：冻结集合 = 从最左侧可见数据列到右键列之间的全部可见数据列。只做**左侧冻结**；行号列（结果网格固定 46px，表数据另含复选框列）始终固定，不受冻结操作影响。冻结列按原始列索引记录，避免隐藏 / 显示后错位。
  3. **约束**：至少保留 1 个可见数据列可横向滚动；只剩 1 个数据列或所有列都可见且无横向滚动时，「冻结到此列」禁用并提示「至少保留一列可滚动」/「当前无需横向滚动」。冻结数量上限为 `visibleColumns.length - 1`；若隐藏列导致冻结集合为空，自动取消冻结。
  4. **视觉**：冻结列在滚动时保持可见；最后一列冻结列右侧显示 1px 分隔线和轻微阴影（`inset` 或伪元素，避免 `border-collapse` 丢边框）；冻结列使用不透明的 `--panel` / `--surface` 背景，浅色 / 深色主题都可读；最后一列冻结列头显示 `pin` 图标和 `aria-label="已冻结 N 列"`；表头与数据行的冻结边界对齐。
  5. **交互保持**：冻结列仍可排序、右键、拖拽列宽、双击恢复列宽；调整冻结列宽度后，后续冻结列的 sticky 偏移立即重算；结果网格的隐藏 / 显示所有列仍可用；表数据的行内编辑、待提交高亮、行选择复选框、批量操作、右键菜单在冻结列上照常工作。冻结列不复制出第二份 DOM。
  6. **横向滚动**：滚动到最右时，冻结列仍在左侧；滚动条、表头、虚拟滚动 spacer 行不产生错位；行悬停 / 选中背景覆盖整行时，冻结列保持自己的背景层，不被滚动内容穿透。
  7. **持久化**：
     - 表数据：按「驱动 + 连接 ID + 数据库 / 模式 + 表名」写入 localStorage `mydb.frozenColumns.v1`，值为冻结列名数组（按从左到右顺序）；重开同一张表时按列名恢复，列已不存在则剪掉；状态最多保留 200 张表，超出按最近使用淘汰（可在实现中简单截断）。若分屏与主视图复用同一个 `TableDataView`，增加 `persist-frozen` prop（分屏传 `false`），避免分屏改动主视图状态；不要让 `readOnly` 兼作该判断，否则只读连接也会失去持久化。
     - 查询结果：不做跨重启持久化。`ResultGrid` 在内存中保留冻结列名数组；同一次查询重新执行后，如果新结果的前 N 个列名与之前冻结列名完全一致，则恢复冻结，否则重置为 0；切换结果集（多语句「结果 N」）时按该规则重新判断。重复列名时按「索引 + 列名」匹配，无法唯一匹配就重置。
  8. **不改变**：复制 / 导出仍使用完整可见列（冻结列包含在内）；不改变排序、隐藏列、总数、分页、筛选、执行历史；不触发任何数据库请求。
  9. **可访问性**：列头菜单项可通过键盘操作；pin 图标有文字替代；冻结不改变 DOM 顺序，屏幕阅读器仍按左到右读取。
- **涉及文件 / 命令**：
  - `src/components/ResultGrid.vue`：新增 `frozenIndices` 状态、列头菜单项、`frozenLeft(index)` 计算、sticky class / style；列宽变化时重算。
  - `src/components/TableDataView.vue`：同上；新增列头右键菜单；与行选择 / 编辑 / 虚拟滚动共用列宽。
  - 新增 `src/lib/frozen-columns.ts`：纯函数 `normalizeFrozenNames`、`pruneFrozenNames`、`frozenOffsets(widths, indices)`、`loadFrozenColumns(key)` / `saveFrozenColumns(key, names)`（localStorage 读写、上限与损坏回退）。
  - `src/styles.css`：`.frozen-cell` / `.frozen-head` / `.frozen-boundary` / `.frozen-pin` 等类；使用现有颜色 token，不新增硬编码颜色。
  - 命令：无。
- **验收标准**：
  - [ ] 创建 25–30 列的临时表（或 `SELECT` 出 25 个别名列），结果网格与表数据都能右键列头「冻结到此列」；横向滚动到最右，冻结的 `id` / `name` 始终可见，行号列也始终可见。
  - [ ] 冻结 3 列后，第 3 列右侧出现分隔线和阴影；表头 pin 图标与「已冻结 3 列」提示正确。
  - [ ] 拖动冻结列宽度、双击恢复宽度后，后续冻结列与滚动内容仍对齐，没有列头 / 单元格错位。
  - [ ] 在结果网格隐藏一个冻结列后，冻结边界与剩余列正确；显示所有列后不会把已隐藏列自动重新冻结；冻结集合为空时自动取消冻结。
  - [ ] 在表数据冻结列上双击编辑、提交前高亮、右键菜单、行复选框、批量设置值均正常；切换分页 / 排序 / 筛选后冻结仍保持。
  - [ ] 表数据冻结状态在关闭标签、重启应用后恢复；表被删除或列被重命名后不恢复不存在的列，不报错。
  - [ ] 同一查询重新执行且列结构一致时冻结恢复；列结构变化（增删列 / 别名变化）时冻结重置；多语句切换「结果 1 / 结果 2」时不会把冻结状态错套到另一结果。
  - [ ] 只剩 1 列时「冻结到此列」禁用并给出原因；没有横向滚动时菜单行为有明确提示，不产生空白列。
  - [ ] 拖拽列宽、排序、隐藏列、导出 CSV / JSON、复制整行 / 全部结果均不受冻结影响；导出内容包含冻结列。
  - [ ] 深浅主题下冻结列背景不透明，滚动内容不穿透；1000 行虚拟滚动时滚动流畅，无重复 DOM 或明显掉帧。
  - [ ] 冻结 / 取消冻结不产生任何数据库请求，不写执行历史，不改变结果行数。
- **风险 / 限制**：
  - `position: sticky` 与 `<table>` 的 `border-collapse` 容易丢边框，需用 `box-shadow: inset` 或伪元素画分隔线；不要为此把整张表改成 `div` 网格。
  - sticky 偏移依赖列宽；`colgroup` 的 auto width 在内容 / 字体 / 主题变化时可能变化，列宽计算必须与现有 `columnWidth()` 一致，并在窗口 resize / 字体设置变化时重算。若无法稳定测量，冻结列使用显式宽度，未冻结列保持 auto。
  - 行号 / 复选框列原本可能不是 sticky；本项需要把它们设为 sticky 才能与冻结列形成完整左侧固定区，注意 z-index 与表头 `top: 0` 的叠加。
  - 右侧冻结不做；RTL / 横向滚动条在左的系统未验证。
  - 查询结果不跨重启持久化是有意为之：结果本身不保存，冻结列也不应指向不存在的结果；表数据才做持久化。
  - 表数据没有隐藏列，列索引稳定；如果后续加入隐藏列，需要同步调整冻结索引 / 偏移逻辑。

---

## F-02 查询结果本地筛选（列条件 / 计数 / 清除）

- **优先级**：P1
- **状态**：已完成（2026-11-26：`result-filter.ts`、筛选条 / 条件徽标、10 种操作符、AND / 大小写 / 截断计数、筛选后查找 / 复制 / 导出快照摘要、新结果重置；headless 验证模糊 / 等于 / 数值 / NULL / 大小写与计数）
- **范围**：前端（`src/components/ResultGrid.vue`、`src/components/QueryWorkspace.vue`、`src/components/ResultExportDialog.vue`、`src/lib/result-export.ts`、新增 `src/lib/result-filter.ts`、`src/styles.css`）；不涉及后端和数据库。
- **用户问题 / 场景**：临时查询返回 2,000 行，只想看 `status = 'failed'` 或 `amount > 100` 的部分；改写 SQL 加 WHERE 会丢失原结果、需要重新查询，还可能因为截断看不到全量；表数据页有服务端 WHERE 和筛选构建器，但查询结果只有排序和隐藏列。E-01 的查找只能高亮匹配项，不能把无关行收起来。
- **现状核对（基于 2026-11-26 代码）**：
  - `ResultGrid.vue` 的 `sortedRows` 直接来自 `props.result.rows`（前端排序）；`visibleColumns` 过滤隐藏列；`buildExportSnapshot()` 使用 `visibleColumns` + `sortedRows`；复制全部 / 导出都基于该快照。
  - `QueryWorkspace.vue` 结果面板标题栏有「导出…」按钮；结果网格标题 / 状态栏显示行数、耗时、截断；没有筛选入口。
  - `TableDataView.vue` 的 `appliedFilter` 是**服务端** WHERE，传给 `readTableRows`；不要把它与 F-02 混用。
  - E-01（未实现）计划在结果网格和表数据页提供查找条；F-02 必须与其明确分层：查找只高亮 / 跳转，不减少行；筛选减少可见行。
- **期望行为（可验收细节）**：
  1. **入口**：`QueryWorkspace` 结果面板标题栏在「导出…」左侧新增「筛选」按钮（`filter` 图标），按钮上显示当前条件数徽标；结果网格单元格右键菜单新增「按此值筛选」；列头右键菜单新增「按此列筛选…」。筛选按钮打开 / 收起筛选条；筛选条位于结果面板标题栏与网格之间，不遮挡表头。
  2. **筛选条结构**：最多 5 个条件，条件之间为 **AND**；每行包含列选择、操作符选择、值输入、删除按钮；底部有「+ 添加条件」「清除筛选」「关闭」；右侧显示「显示 X / 共 Y 行（筛掉 Z 行）」。列选择包含结果的全部列，已隐藏列标注「（已隐藏）」；值输入对「为空 / 不为空」禁用。
  3. **操作符与语义**（MVP 固定集合）：
     - `包含` / `不包含`：把值转成显示文本做子串匹配；文本默认忽略大小写，可用筛选条上的「区分大小写」复选框切换；`null` / `undefined` 不匹配这两个操作符（避免把 NULL 当成空串）。
     - `等于` / `不等于`：数字按数值比较，布尔按布尔比较，字符串按文本比较；默认忽略大小写，勾选「区分大小写」后精确比较；`null` 只与「为空」匹配，不参与等于 / 不等于（与 SQL 的 NULL 语义保持一致，界面用提示说明）。
     - `大于` / `大于等于` / `小于` / `小于等于`：仅当左右两侧都能解析为有限数字时比较；左侧不是数字、右侧输入不是数字时该行不匹配，并在条件下方显示「数值比较仅适用于数字值」提示；不把日期字符串当日期解析。
     - `为空` / `不为空`：`null` / `undefined` 视为空；空字符串 `""` 不算空；`0` / `false` 不算空。
  4. **实时与计数**：值输入 150ms 防抖后重新计算；条件变化立即更新计数与网格；不点击「应用」也能看到结果，但保留「清除筛选」。计数使用筛选后行数 / 原始结果行数；结果被截断时在计数旁显示「结果已截断，仅筛选已加载的 N 行」。
  5. **与排序 / 隐藏 / 冻结的关系**：先筛选，再按现有 `sortIndex` / `sortDirection` 排序；隐藏列不影响筛选（可用隐藏列做条件）；冻结列、列宽、行号、右键菜单、虚拟滚动都基于筛选后的行集；筛选不改变原始 `result.rows`，只生成派生数组。
  6. **与查找（E-01）的关系**：如果 E-01 已实现，查找只在筛选后的行集内匹配，匹配计数显示「筛选结果中 N 处」；清除筛选后重新计算查找；两者互不重置对方，但新查询执行时都重置。
  7. **复制与导出语义**（本项必须明确，不能含糊）：
     - 筛选激活时，`ResultGrid` 的复制单元格 / 整行 / 全部结果 / JSON、`requestExport()` 的 CSV / JSON / Markdown / SQL INSERT / XLSX 都使用**筛选后 + 当前排序 + 当前可见列**的行集。
     - `ResultExportSnapshot` 增加可选字段 `filteredOut: number`、`filterSummary: string | null`（或等价结构）；导出对话框信息行显示「已筛选：status = failed（筛掉 820 行）」；CSV / JSON 文件只包含筛选后的行。
     - 没有筛选条件时行为与现在完全一致，快照字段为 `0` / `null`，不改变现有导出测试。
     - 用户想导出全量时，先「清除筛选」；对话框不自动提供「忽略筛选导出全部」，避免一次导出两种语义。
  8. **生命周期**：新查询执行、切换多结果集、结果列结构变化、关闭标签时清空筛选并关闭筛选条；排序、隐藏 / 显示列、列宽、冻结、打开 / 关闭导出对话框、复制不重置筛选；关闭筛选条但不清除条件时，条件继续生效，按钮徽标保留；「清除筛选」恢复全部行并清空条件。
  9. **禁用与错误**：执行中、无结果、非查询语句（`isQuery === false`）、0 行时筛选按钮禁用并给出原因（查询执行中 / 没有可筛选的结果 / 当前语句没有结果集 / 结果为空）；错误面板显示时不出现筛选条。
  10. **性能与上限**：10,000 行 × 5 条件在普通机器上计算 + 渲染更新不超过 100ms（不含浏览器绘制）；条件上限 5，超过时「添加条件」禁用并提示；不对每行创建持久的小写副本缓存，按条件即时转换；筛选条关闭后不保留临时输入。
  11. **可访问性**：筛选按钮 / 条件行 / 计数有可读标签；计数变化用 `aria-live="polite"`；Esc 关闭筛选条但保留条件；条件删除按钮可用键盘到达；不新增全局快捷键，避免与 `⌘/Ctrl+F`（E-01 / CodeMirror 查找）冲突。
- **涉及文件 / 命令**：
  - 新增 `src/lib/result-filter.ts`：`ResultFilterCondition`、`matchesResultFilter(row, columns, conditions, caseSensitive)`、`describeFilter(condition)`、`validateCondition` 等纯函数；不 import Vue。
  - `src/components/ResultGrid.vue`：`filterConditions`、`caseSensitive`、`filteredRows`、`displayRows`（筛选 + 排序）、筛选条 UI、右键菜单入口、计数、导出快照；`copyAll` / `copyRow` / `requestExport` 改用 `displayRows`。
  - `src/components/QueryWorkspace.vue`：结果面板标题栏「筛选」按钮、条件数徽标、筛选条开关；监听 `result` 变化关闭 / 重置；将筛选元数据传给导出对话框。条件状态可以放在 `ResultGrid` 内，通过类似现有 `hidden-change` 的 `filter-change` 事件向 `QueryWorkspace` 同步条件数；也可把条件状态提升到 `QueryWorkspace` 后传入，二选一，但同一状态只能有一份。
  - `src/components/ResultExportDialog.vue` + `src/lib/result-export.ts`：快照可选字段与信息行文案；保持旧快照兼容。
  - `src/styles.css`：`.result-filterbar`、`.filter-condition`、`.filter-count`、`.filter-badge` 等，复用现有表单 / 按钮 token。
  - 命令：无。
- **验收标准**：
  - [ ] `examples/demo.db` 执行 `SELECT * FROM orders`：筛选 `status = paid`，显示 2 / 4 行；再添加 `amount > 100`，显示 1 / 4 行；计数与网格一致。
  - [ ] 筛选 `status 包含 pai`（忽略大小写）命中 2 行；勾选「区分大小写」后同一条件命中 0 行；取消勾选恢复。
  - [ ] 筛选 `city 为空` 的行为正确：只有 NULL 命中，空字符串不命中；`不为空` 排除 NULL。
  - [ ] 数值比较：`amount > 100` 只保留数字大于 100 的行；对文本列使用「大于」时给出提示且不崩溃；输入非数字时该条件不匹配任何行。
  - [ ] 先按 `amount` 降序再筛选，结果仍按降序；隐藏 `status` 列后仍可用隐藏列筛选；冻结列、列宽、虚拟滚动保持正常。
  - [ ] 筛选激活时导出 CSV：文件只包含筛选后的行；导出对话框信息行显示筛掉的行数与条件摘要；JSON / Markdown / SQL INSERT / XLSX 同样只包含筛选后的行。
  - [ ] 筛选激活时「复制全部」TSV 与「复制整行 JSON」只包含筛选后的行 / 可见列；没有筛选时行为与现在一致。
  - [ ] 重新执行查询、切换「结果 1 / 结果 2」或修改结果列后，筛选重置并关闭；排序、隐藏列、调整列宽、冻结、打开导出对话框不重置筛选。
  - [ ] 关闭筛选条但条件仍在时，网格仍只显示筛选后的行，按钮徽标保留条件数；「清除筛选」恢复全部行。
  - [ ] 10,000 行结果上添加 3–5 个条件，输入时无可感知卡顿（目标 <100ms 计算），不发起数据库请求，不新增执行历史。
  - [ ] 空结果、非查询语句、执行中时筛选入口禁用并显示原因；错误面板显示时无筛选条。
  - [ ] 双查询分屏 / 独立查询窗口中的结果筛选互不影响；关闭窗口 / 标签后不残留筛选状态。
- **风险 / 限制**：
  - 本地筛选只作用于**已返回**的结果；如果数据库结果被 1,000 / 10,000 行上限截断，筛选无法找回未返回的行。必须显示截断提示，不能宣称「筛出了全部符合条件的数据」。
  - 本地筛选与 SQL `WHERE` 语义不同：NULL、字符串大小写、数字 / 字符串转换、数据库排序规则都会造成差异；界面用「仅筛选当前结果」和条件摘要说明，避免用户误以为数据被删除。
  - 筛选激活后导出范围变小是**有意行为**，与 B-02 原有「导出当前结果」语义一致；但必须在导出对话框明确提示，避免用户误以为导出了全量。
  - 结果列可能是表达式、重复别名或空列名；筛选用原始索引定位，不依赖列名唯一性；导出仍受 B-02 的重复列名限制。
  - 超大对象 / 数组值的「包含」匹配会把对象序列化为文本，性能与结果可能与数据库不同；文档中说明只做显示文本匹配。
  - 关闭筛选条但保留条件可能让用户误以为筛选已取消；按钮徽标、计数和「清除筛选」必须始终可见。
  - 若 E-01 尚未实现，F-02 的查找交互按「无查找条」降级，不得留下半成品按钮或引用不存在的事件。

---

## F-04 从剪贴板粘贴多行数据到表（待提交）

- **优先级**：P1
- **状态**：已完成（2026-11-26：`clipboard-table.ts` 解析 / 映射 / 类型转换、`PasteRowsDialog.vue` 预览与错误行、工具栏 / 右键 / `⌘/Ctrl+Shift+V`、只进入待提交并可撤销；纯函数验证 TSV / CSV / 引号换行 / 错误行）
- **范围**：前端（`src/components/TableDataView.vue`、新增 `src/components/PasteRowsDialog.vue`、新增 `src/lib/clipboard-table.ts`、`src/lib/clipboard.ts` 现有 `readClipboard`、`src/styles.css`）；不涉及后端和数据库写入。
- **用户问题 / 场景**：从 Excel / Numbers / 网页表格复制 20 行数据后，mydb 只能逐行点「新增行」或先另存 CSV 再用导入向导；导入向导会直接写库，缺少「先看预览、再进入待提交、确认后统一提交」的中间状态。Navicat 支持网格粘贴，DBeaver 的 Advanced Paste 支持多行插入，mydb 这块是空白。
- **现状核对（基于 2026-11-26 代码）**：
  - `src/lib/clipboard.ts` 已导出 `readClipboard()`，Tauri 端使用 `@tauri-apps/plugin-clipboard-manager` 的 `readText`；`src-tauri/capabilities/default.json` 已包含 `clipboard-manager:allow-read-text`，不需要新增权限。
  - `TableDataView.vue` 有 `pendingInserts: InsertDraft[]`（`values` 按列索引对齐）、`parseInput()`、`submitInsert()`、批量提交 / 回滚、只读 / 视图判断；新增行通过待提交缓冲区统一提交，不直接写库。
  - 现有导入向导 `ImportWizardDialog.vue` 走后端 `preview_import_file` / `execute_import_file`，按文件路径读取并逐行写库；F-04 不复用它写库，只复用它的列映射、空值策略与预览交互经验。
  - 工具栏已有「导入数据…」按钮和表右键「导入数据」；单元格右键菜单已有复制 / 查看值 / 复制为等项。
- **期望行为（可验收细节）**：
  1. **入口**：表数据工具栏在「导入数据…」旁新增「粘贴新增…」按钮（`paste` 图标）；单元格 / 行右键菜单新增「粘贴新增行…」；表格获得焦点且未处于单元格编辑时，`⌘/Ctrl+Shift+V` 打开粘贴对话框。**不劫持**普通 `⌘/Ctrl+V`：在单元格编辑输入框内仍按系统行为粘贴文本；在网格非编辑状态按普通 `⌘V` 不打开对话框，避免误触。
  2. **读取与限制**：调用 `readClipboard()` 读取纯文本；空文本提示「剪贴板没有可粘贴的文本」；超过 5 MB 或解析后超过 5,000 行时拒绝并提示上限；超过 20,000 行直接拒绝。只支持文本（TSV / CSV），不解析 HTML、图片、Excel 二进制；如果剪贴板只有 HTML，提示用户先在 Excel 中复制为文本。
  3. **解析**：自动识别分隔符（首行含 TAB → 制表符；否则逗号；否则分号；默认制表符），对话框中可手动切换；支持 RFC4180 双引号、字段内逗号 / 换行、`""` 转义；行分隔支持 CRLF / LF / CR；保留空字段。解析结果包含 `columns`（表头或「列 1..N」）、`rows`、`delimiter`、`headerDetected`、`rowCount`。
  4. **表头识别**：如果首行每个单元格（去除首尾空格、忽略大小写）都能匹配目标表列名，默认「首行为表头」开启；否则默认关闭，并把首行当作数据。用户可手动切换。表头匹配只用于生成源列名，不自动跳过未匹配列。
  5. **预览对话框**：
     - 显示目标表、识别到的分隔符、行列数、「首行为表头」开关、分隔符选择。
     - 预览前 20 行，表头显示源列名，单元格显示原始文本；超长值截断显示并可用 title 查看。
     - 列映射：每个源列一个目标字段下拉，按同名（忽略大小写）自动匹配；支持「忽略此列」；同一个目标字段不能被映射两次；自增列默认「忽略（由数据库生成）」；二进制列默认忽略并提示；未映射的 NOT NULL 且无默认值字段在开始前列出警告。
     - 选项「空单元格写入 NULL」默认开启；关闭时空字符串写入 `""`。显示「将新增 N 行到待提交，不会立即写入数据库」。
     - 底部按钮「取消」「加入待提交」；只读连接、视图、没有可编辑列时禁用并说明原因。
  6. **类型转换与错误行**：确认时按目标字段转换。现有 `parseInput(value, column)` 只处理 `NULL`、空值和常见数值类型，**不处理布尔 / 日期**；F-04 需要新增 `parseClipboardValue(value, column, emptyAsNull)`（或扩展 `parseInput` 并保证手动编辑不回归）：数值列接受整数 / 小数（MVP 不处理千分位，按原样转换，失败即错误），布尔列接受 `true/false/1/0`（按目标列类型识别 `BOOL/BOOLEAN/BIT/TINYINT(1)`），日期时间按文本写入（不做本地时区转换），空值按空值策略。明显无法转换的行（如数值列填 `abc`）在预览对话框中列出「第 N 行 · 列名 · 原因」，最多显示 20 条；用户可选择「跳过错误行」或「取消」。跳过时只把有效行加入待提交，并显示跳过数量；不存在部分写库，因为还没有提交。
  7. **写入路径**：确认后只向 `pendingInserts` 追加 `InsertDraft`，每条 `values` 按全部列索引生成，未映射 / 自增列保持 `undefined`；不调用 `insertTableRow`、`api.importCsv`、`execute_import_file` 或任何直接写库命令；不写执行历史。用户仍需点击现有「提交」按钮，由 `applyTableChanges` 在单个事务中写入；失败整体回滚并保留草稿的行为不变。
  8. **与现有待提交状态**：粘贴前已有的单元格编辑 / 新增 / 删除标记全部保留；粘贴只追加新增草稿；如果待提交新增总数超过 10,000 行，提示并拒绝本次粘贴；粘贴后选中状态清空，滚动到新增行区域（若现有实现支持），消息显示「已粘贴 N 行到待提交（跳过 M 行）」。
  9. **只读 / 视图 / 无主键**：只读连接、视图、没有可编辑列时入口禁用并显示原因；无主键表允许新增（与现有 `submitInsert` 一致），不因为粘贴而放宽编辑 / 删除限制。
  10. **可访问性**：对话框使用现有 `.modal` 样式，Esc 关闭并回到触发按钮；预览表格有表头；映射下拉有 label；错误列表可键盘读取；不新增全局快捷键。
- **涉及文件 / 命令**：
  - 新增 `src/lib/clipboard-table.ts`：纯函数 `detectDelimiter(text)`、`parseDelimitedText(text, delimiter, hasHeader)`、`autoMapColumns(source, tableColumns)`、`convertPasteRows(parsed, mapping, columns, emptyAsNull)`；不 import Vue。
  - 新增 `src/components/PasteRowsDialog.vue`：读取剪贴板、解析、预览、映射、错误行、确认；复用 `Icon.vue`、现有 modal / 表格 / 下拉样式。
  - `src/components/TableDataView.vue`：工具栏按钮、右键菜单、`⌘/Ctrl+Shift+V` 监听（仅表格焦点、非编辑态）、`pendingInserts` 追加、消息 / 只读判断；不新增写库命令。
  - `src/lib/clipboard.ts`：复用 `readClipboard()`；如需要，增加 `hasText` 或错误包装，不改变现有写入语义。
  - `src/styles.css`：粘贴对话框预览 / 映射 / 错误列表样式，复用现有表单 token。
  - 命令：无；权限已在 `src-tauri/capabilities/default.json`。
- **验收标准**：
  - [ ] 从 Excel / 文本编辑器复制 3 行 × 3 列 TSV，点击「粘贴新增…」：预览显示 3 行 3 列，分隔符识别为制表符，同名列自动映射；确认后待提交显示 3 行，数据库尚未变化；点击「提交」后 3 行写入，事务成功。
  - [ ] 带表头的 TSV：首行与表列名匹配时「首行为表头」默认开启，表头不写入数据；关闭后首行作为数据写入。
  - [ ] 逗号分隔且字段内含逗号 / 换行 / `""` 的文本解析正确，预览列数与实际一致；手动切换分隔符后重新解析正确。
  - [ ] 自增主键列默认忽略，提交后由数据库生成；手动把源列映射到自增列时给出提示，不静默覆盖。
  - [ ] 同一个目标字段不能被映射两次；未映射的 NOT NULL 无默认值字段在开始前提示；补映射后可继续。
  - [ ] 空单元格在「写入 NULL」开启时进入 NULL，关闭时进入空字符串；数值 / 布尔转换正确，数值列填 `abc` 时列出错误行，选择跳过时只加入有效行。
  - [ ] 已有待提交编辑 / 新增 / 删除标记在粘贴后保留；粘贴只追加，不覆盖；提交仍为单事务，失败整体回滚并保留草稿。
  - [ ] 只读连接、视图、无可编辑列时入口禁用并显示原因；绕过前端调用也不会在提交前写库（提交时由现有只读守卫拒绝）。
  - [ ] 在单元格编辑输入框内按 `⌘/Ctrl+V` 仍粘贴文本到输入框；网格非编辑态按 `⌘/Ctrl+Shift+V` 打开对话框；普通 `⌘V` 不打开。
  - [ ] 空剪贴板、超过 5 MB、超过 5,000 行、超过 20,000 行的输入都有明确提示，不崩溃、不部分写入。
  - [ ] 粘贴过程不产生数据库请求、不新增执行历史；关闭对话框不改变待提交状态。
- **风险 / 限制**：
  - Excel 剪贴板可能同时提供 HTML / 富文本，`readText` 在部分平台可能拿到空或非 TSV 文本；MVP 只处理纯文本，失败时提示用户复制为文本，不解析 HTML。
  - 日期 / 货币 / 千分位 / 本地化小数分隔符不做智能识别，按原文交给 `parseInput` / 数据库；失败行让用户修正或跳过，避免静默改值。
  - 表头识别是启发式：首行恰好与列名相同可能被误判为表头，用户可手动关闭；反过来，表头大小写 / 空格不一致时需要手动映射。
  - 粘贴进入待提交后占用内存；5,000 行 × 多列可能较大，提交前不要反复粘贴超大块；上限提示必须清楚。
  - 不做唯一键 / 外键预检查；冲突在提交时由数据库返回，现有事务回滚行为不变。
  - 二进制 / JSON / 数组字段的文本表示与数据库实际类型可能不同；默认忽略二进制列，其他类型按文本写入，失败在提交时报告。
  - 与 E-04 Excel 文件导入的边界：E-04 走文件 + 后端流式写入，适合大批量；F-04 走剪贴板 + 待提交，适合小批量人工校对。不要为了统一而把 F-04 改成直接写库。

---

## F-03 外键关联跳转与反向引用（引用行 / 被引用记录）

- **优先级**：P1
- **状态**：已完成（2026-11-26：`ScopeForeignKey` + `scope_foreign_keys` 默认实现与 SQLite 隐式主键补全、只读命令、60 秒范围缓存、表数据「关联」菜单、目标表 linkedFilter 标签与来源提示；SQLite 单测通过，headless 验证入向 / 出向菜单与跳转）
- **范围**：后端（`src-tauri/src/models.rs`、`src-tauri/src/db/driver.rs`、`src-tauri/src/db/mysql.rs`、`src-tauri/src/db/postgres.rs`、`src-tauri/src/db/sqlite.rs`、`src-tauri/src/commands/query.rs`、`src-tauri/src/lib.rs`）；前端（`src/lib/types.ts`、`src/lib/api.ts`、`src/components/TableDataView.vue`、`src/App.vue`、`src/styles.css`、可新增 `src/lib/foreign-key-cache.ts`）。
- **用户问题 / 场景**：在 `orders` 表看到 `user_id = 42`，想确认是哪个用户，只能复制 42、打开 `users`、输入 `WHERE id = 42`；反过来在 `users` 想查看该用户的所有订单，需要知道外键列名并手写查询。ER 图能看到关系，但不能带值跳到数据；结构页能看到外键，但不能一键查看引用行。
- **现状核对（基于 2026-11-26 代码）**：
  - `TableDataView.vue` 已有当前行原始值、列元数据（`ColumnInfo`）、`readTableRows(connectionId, table, limit, offset, filter, ...)` 的服务端 WHERE 能力、`sqlLiteral` / `quoteIdentifier` 工具；表标签由 `App.vue` 的 `TableTab` 管理，目前没有「初始筛选」字段。
  - 驱动 trait `DatabaseDriver` 已有 `list_foreign_keys(&self, table)` 和默认 `er_diagram()`；MySQL / PostgreSQL / SQLite 都有单表外键实现。`get_er_diagram` 会遍历当前范围所有表并读取列与外键，成本较高，不适合作为每次右键的元数据来源。
  - `ReadTableRows` 的 `filter` 是原始 WHERE 片段，TableDataView 内部已有 `appliedFilter` 和筛选条；表标签切换 / 分页 / 排序 / 待提交保护都围绕它工作。
  - `TableStructureView.vue` 使用 `get_table_structure` 展示外键；`er_diagram` 使用 `list_foreign_keys`；两者可复用外键读取逻辑，但都不能直接打开带筛选的数据。
- **期望行为（可验收细节）**：
  1. **后端元数据命令**：新增 Tauri 命令 `list_scope_foreign_keys(connection_id)`，返回当前连接 + 当前数据库 / 模式范围内的全部外键边：
     ```rust
     pub struct ScopeForeignKey {
         pub table: String,
         pub name: String,
         pub columns: Vec<String>,
         pub referenced_table: String,
         pub referenced_columns: Vec<String>,
     }
     ```
     字段按 `camelCase` 序列化。只包含当前范围（MySQL 当前库、PostgreSQL 当前模式、SQLite 当前文件）；排除系统库 / 系统模式；按 `table`、`name` 排序；同一约束的多列按外键列顺序返回。只读查询，不执行 DDL / 写入。
  2. **驱动实现**：
     - MySQL：在现有 `mysql_foreign_keys` 基础上增加 `table_name` 字段并去掉单表条件，一次查询当前 `table_schema` 的所有外键；`information_schema.key_column_usage` + `referential_constraints`，按 `table_name, constraint_name, ordinal_position` 排序。
     - PostgreSQL：在现有 `pg_foreign_keys` 基础上增加 `c.relname AS table_name` 并去掉 `c.relname = $2`，一次查询当前 `n.nspname` 的所有外键；保持 `con.contype = 'f'`、`unnest(conkey, confkey)` 的列顺序；只返回当前模式内的表与引用表（跨模式引用本批跳过）。
     - SQLite：在 trait 中提供默认 `scope_foreign_keys()`：遍历 `list_tables()` 中的表，调用现有 `list_foreign_keys(table)`，补上 `table` 字段；对 `PRAGMA foreign_key_list` 中 `to` 为 NULL（引用目标主键）的情况，读取目标表主键列填充 `referenced_columns`；无法解析目标主键时跳过该外键并记录（不要返回不完整列）。
     - 其他驱动（Redis 等）不实现 / 返回空；命令只在 SQL 连接上可用。
  3. **前端缓存**：新增 `src/lib/foreign-key-cache.ts`（或等价模块），按「连接 ID + 数据库 + 模式」缓存 `ScopeForeignKey[]`，TTL 60 秒，最多 20 个范围；`TableDataView` 打开时后台预取，失败不阻塞数据加载；右键菜单需要时若缓存为空 / 过期则重新请求。对象树刷新、执行 DDL 新建 / 删除表后，下一次请求自然过期；不要求实时失效。命令沿用驱动当前库 / 模式上下文，因此调用前必须确保连接已切换到表标签所属范围（沿用现有表数据加载的上下文约定）；缓存键包含范围，切换库 / 模式后不会误用上一个范围的缓存。
  4. **单元格右键菜单**：当点击列属于某个外键（出向）或当前表被其他表引用（入向）时，在「查看值…」之后增加分隔线和「关联」分组：
     - 出向：每个包含该列的外键一项，文案「打开引用行 → users（id）」；复合键显示「→ users（a, b）」。点击后打开 `users` 表标签，并应用 `referenced_columns = 当前行 FK 列值` 的 WHERE。
     - 入向：子菜单「查看引用记录 →」，列出所有以当前表为目标的子表 / 约束，例如「orders.user_id → users.id」；点击后打开 `orders` 表标签，并应用 `user_id = 当前行 id` 的 WHERE。若同一子表有多个外键，分别列出。入向与点击的是哪一列无关，按当前行被引用列的值判断（同一行可能有多条入向关系，来源列不同）；只要当前行的被引用列值齐全且非 NULL，就显示。
     - 如果点击列既不属于出向外键，当前行也没有可用的入向关系，则不显示「关联」分组，避免菜单噪声。
     - 元数据加载中：显示禁用的「正在加载关联信息…」；加载失败：显示可点击的「关联信息不可用，点击重试」；Redis / 视图不显示。
  5. **可用性判断**：
     - 出向：当前行所有外键列值都不为 NULL / undefined，且目标表在当前连接与当前库 / 模式下，才启用；任一为 NULL 时禁用并提示「外键列包含 NULL，无法定位引用行」。
     - 入向：当前行所有被引用列值都不为 NULL，且引用表在当前范围内，才启用；否则禁用并提示。
     - 跨库 / 跨模式的外键：菜单项显示但禁用，提示「跨库 / 模式关联暂不支持」；不自动切换连接上下文（避免触发未提交事务保护）。
     - 二进制 / 数组 / 对象值：无法安全生成匹配条件时禁用并提示；`sqlLiteral` 仍用于字符串 / 数字 / 布尔 / NULL（NULL 已在上面禁用）。
  6. **生成筛选条件**：按驱动使用 `quoteIdentifier` + `sqlLiteral`：
     - 出向：`"<referenced_col>" = <当前行 FK 值>`，多列用 `AND`。
     - 入向：`"<fk_col>" = <当前行被引用列值>`，多列用 `AND`。
     - 顺序按外键列顺序；不拼接用户输入，不使用字符串插值之外的未转义值；条件只用于 `readTableRows` 的 `filter` 参数。
  7. **打开目标表**：
     - `App.vue` 新增 `openLinkedTable`（或扩展 `openTableTabAt`）：在目标表标签上写入 `linkedFilter` / `linkedFilterLabel`（例如 `id = 1`、`来自 orders.user_id`），并让 `TableDataView` 以 `initialFilter` 初始化 `appliedFilter` / `filterDraft`、打开筛选条、加载第 1 页。
     - 目标表标签始终新建（`forceNew = true`），除非已存在**同连接 + 同库 / 模式 + 同表 + 同筛选条件**的标签，此时激活已有标签；不要复用用户正在编辑其它筛选项的标签。
     - 源表标签保持不变并继续打开；用户可通过标签栏切回，不需要额外的「返回」按钮。本批不做前进 / 后退历史。
     - 目标表筛选条显示生成的 WHERE；用户可以清除 / 修改筛选，修改后不自动回写源表；不改变目标表的排序 / 分页默认值。
     - 目标表打开时状态栏提示「已按 orders.user_id 跳转：users.id = 1」，只显示一次，不写执行历史。
  8. **只读与编辑**：只读连接允许跳转和浏览；目标表的待提交编辑与源表独立；跳转不写数据库、不清空源表待提交状态；如果目标表已有未提交编辑并被复用（同条件），沿用现有标签保护逻辑，不强行刷新。
  9. **错误处理**：元数据读取失败、目标表不存在、权限不足时，显示内联错误 / Toast，保留源表，不打开空标签；不能把原始数据库错误堆栈直接暴露为用户文案（保留原始错误供复制）。
  10. **不改变**：ER 图布局、结构页外键列表、表数据筛选 / 排序 / 分页、复制 / 导出、执行历史、只读保护。结果网格本批不提供外键跳转（缺少表名与主键元数据）。
- **涉及文件 / 命令**：
  - `src-tauri/src/models.rs`：新增 `ScopeForeignKey`（`#[serde(rename_all = "camelCase")]`）。
  - `src-tauri/src/db/driver.rs`：trait 新增默认 `scope_foreign_keys()`（遍历 `list_tables` + `list_foreign_keys`，补 `table`，处理 SQLite 目标主键）；不改变现有 `list_foreign_keys` / `er_diagram`。
  - `src-tauri/src/db/mysql.rs` / `postgres.rs`：覆写 `scope_foreign_keys()`，一次查询当前范围全部外键；复用现有 `quote_identifier` / `foreign_key_action` 等工具；SQLite 走默认实现或补目标主键解析。
  - `src-tauri/src/commands/query.rs`：新增 `list_scope_foreign_keys` 命令（`driver_for` + `driver.scope_foreign_keys()`）。
  - `src-tauri/src/lib.rs`：注册新命令。
  - `src/lib/types.ts`：`ScopeForeignKey` 接口；`src/lib/api.ts`：`listScopeForeignKeys(connectionId)`。
  - 新增 `src/lib/foreign-key-cache.ts`：TTL / 范围缓存、加载中 / 失败状态。
  - `src/components/TableDataView.vue`：预取外键、右键菜单出向 / 入向项、可用性判断、条件生成、打开目标表事件 / 调用。
  - `src/App.vue`：`TableTab` 增加 `linkedFilter` / `linkedFilterLabel`；`openTableTabAt` 支持初始筛选；`openLinkedTable` 去重 / 新建逻辑；传给 `TableDataView`。
  - `src/styles.css`：关联菜单分组 / 筛选来源提示样式（尽量复用现有 `context-menu` / `filter-meta`）。
  - 测试：`src-tauri/src/db/sqlite.rs` 增加 `scope_foreign_keys` 单测（demo schema：orders→users、order_items→orders、复合键、隐式主键）；MySQL / PostgreSQL 测试沿用现有环境变量门控，未配置时跳过。
- **验收标准**：
  - [ ] `examples/demo.db` 打开 `orders` 表，右键 `user_id = 1` 的单元格 →「打开引用行 → users（id）」；目标标签显示 `users` 且筛选 `"id" = 1`，1 行；状态栏提示来源；源 `orders` 标签仍在。
  - [ ] `orders` 表右键 `user_id` 单元格 → 入向列表包含 `order_items.order_id → orders.id`（如果当前表是 orders）；点击后打开 `order_items`，筛选 `"order_id" = 1`，行数与数据库一致。
  - [ ] `users` 表 `id = 1` 行 →「查看引用记录 → orders.user_id」；打开 `orders`，筛选 `"user_id" = 1`，显示 2 行；清除筛选后显示全部订单。
  - [ ] `order_items.order_id` 跳转到 `orders.id`；`orders.user_id` 跳转到 `users.id`；两级跳转都能独立完成。
  - [ ] 创建复合外键临时表（`child(a,b) REFERENCES parent(x,y)`），右键单元格生成的 WHERE 为两个条件 AND，目标行正确；任一外键列为 NULL 时菜单项禁用并提示。
  - [ ] 自引用外键（`employees.manager_id → employees.id`）可以跳转，目标标签筛选正确，不无限递归。
  - [ ] SQLite 外键 `REFERENCES parent` 未写目标列时，按目标表主键补全；目标主键无法解析时菜单不出现或提示不支持，不生成错误条件。
  - [ ] 目标表标签的筛选条可见、可分页 / 排序 / 清除；清除后显示全部行；重新从源表跳转会新建或复用同条件标签，不覆盖用户已有的不同筛选标签。
  - [ ] 跨库 / 跨模式外键菜单项禁用并提示；视图不出现关联分组；只读连接可跳转，数据库无任何写入。
  - [ ] 外键元数据读取失败时显示「关联信息不可用，点击重试」，不阻塞表数据浏览；重试成功后菜单出现。
  - [ ] 跳转不写执行历史、不新增 SQL 历史记录；源表 / 目标表的待提交编辑互不影响；跳转后关闭目标标签不影响源表。
  - [ ] 后端 SQLite 单测通过；MySQL / PostgreSQL 在设置 `MYDB_TEST_*` 时能列出示例库外键，未设置时测试跳过。
- **风险 / 限制**：
  - 外键元数据在不同版本 / 权限下差异较大：MySQL 需要 `information_schema` 权限，PostgreSQL 需要能读 `pg_catalog`；失败时必须降级为「不可用」，不能阻塞数据页。
  - 复合外键的列顺序必须与约束定义一致；`PRAGMA foreign_key_list` 的 `seq` / `to` 处理容易错，SQLite 必须补测试。
  - MySQL 外键可能跨库、PostgreSQL 可能跨模式；本批只支持当前范围，跨范围明确禁用，避免切换连接上下文触发事务保护。
  - 入向引用可能有多个子表 / 多个约束，菜单会变长；实现时限制子菜单高度并支持滚动，超过 30 项时提示「引用过多，请从结构页查看」，不无限渲染。
  - 筛选值来自当前页的原始值；如果值是浮点数 / 高精度小数 / 时间，`sqlLiteral` 的字符串化可能与数据库精确值有差异，导致目标行匹配不到；验收用整数 / 文本主键，风险在文档中说明。
  - 外键目标表可能已被删除或重命名；打开失败要给出可读错误，不创建空标签。
  - 目标表标签复用同条件时会激活已有标签；如果该标签有未提交编辑，沿用现有保护，不自动刷新或丢弃。
  - 本批不做导航历史、跨窗口同步、结果网格外键跳转；这些留在后续。

---

## F-05 SQL 执行范围控制（当前语句 / 整个标签）

- **优先级**：P1
- **状态**：已完成（2026-11-26：`query-run-mode.ts`、设置项、选区优先 / 当前语句 / 整个标签三种路径、Run 下拉与「第 N / M 条」徽标、复杂脚本拒绝、命令面板「运行整个标签」；headless 验证当前语句只执行 1 条、整个标签执行 3 条并产生多结果集）
- **范围**：前端（`src/components/SqlEditor.vue`、`src/components/QueryWorkspace.vue`、`src/App.vue`、`src/components/SettingsDialog.vue`、新增 `src/lib/query-run-mode.ts`、复用 `src/lib/sql-statements.ts`）；不涉及后端和数据库。
- **用户问题 / 场景**：编辑器里有 3 条语句，光标停在第二条，按 `⌘/Ctrl+Enter` 会把整个标签的 3 条全部执行；想重跑一条只能先选中，容易误执行后面的 UPDATE / DELETE。DBX 默认执行「当前语句或选中 SQL」，DBeaver 区分「执行语句」和「执行脚本」；mydb 目前没有可见的执行范围，也没有默认的当前语句行为。
- **现状核对（基于 2026-11-26 代码）**：
  - `SqlEditor.vue` 的 `Mod-Enter` keymap 只取选区；没有选区时 emit `run(undefined)`。`SqlEditor` 已暴露 `currentStatement()`，内部使用 `statementAtCursor()` / `statementRangeAtCursor()`；该能力目前主要给 EXPLAIN / 格式化使用。
  - `App.vue::runQueryOnTab()` 在 `selectedSql` 为空时使用 `tab.sql`（整个标签）；`executeSqlOnTab()` 用 `splitSqlStatements()` 拆分，多语句走多结果集路径；危险 SQL 确认基于传入的 `sqlText`；执行历史记录实际执行的 SQL。
  - `src/lib/sql-statements.ts` 已有 `splitSqlStatements()`、`hasSqlContent()`、`statementAtCursor()`、`statementRangeAtCursor()`；支持引号、行注释、块注释、PostgreSQL dollar quote，但不支持 MySQL `DELIMITER`。
  - `SettingsDialog.vue` 目前有主题、编辑器字号、危险 SQL 确认、格式化设置入口；设置通过 `src/lib/theme.ts` / `dangerous-sql.ts` 的 localStorage helper 持久化。没有执行范围设置。
  - 查询工作区 / 分屏 / 独立窗口都通过 `QueryWorkspace` 的 `run` 事件和 App 的 `runQuery` 进入同一条链路；`currentStatement()` 已能从活动或分屏编辑器读取。
- **期望行为（可验收细节）**：
  1. **执行范围设置**：新增 `src/lib/query-run-mode.ts`，localStorage 键 `mydb.queryRunMode.v1`，值为 `"statement"`（默认）或 `"tab"`；损坏 / 未知值回退 `"statement"`。SettingsDialog 增加「查询」分区，下拉「运行按钮默认范围」：当前语句 / 整个标签，附说明「有选区时始终执行选中 SQL」；切换后立即保存并生效。
  2. **默认执行行为**：`⌘/Ctrl+Enter` 和工具栏「运行」按钮：
     - 有选区：执行选区（不拆分，保持现有 `executeSqlOnTab` 行为；如果选区包含多条语句，仍进入多结果集路径）。
     - 无选区且模式为 `statement`：执行光标所在语句；使用 `statementAtCursor()`，光标在两条语句之间时取最近的前一条非空语句；光标在最前面 / 无前置语句时取第一条；光标在空文本 / 只有注释时禁用并提示「光标不在可执行语句内」。
     - 无选区且模式为 `tab`：执行整个标签（现有行为）。
  3. **执行范围入口**：在「运行」按钮右侧增加一个 chevron 下拉菜单（复用 `format-menu-host` / `context-menu` 样式），菜单项：
     - 「运行当前语句」——无论默认模式如何，执行当前语句一次（不改变设置）。
     - 「运行整个标签」——无论默认模式如何，执行整个标签一次（不改变设置）。
     - 显示当前默认模式的勾选 / 说明；菜单在运行中禁用。
     - 不新增全局快捷键；`⌘/Ctrl+Enter` 仍按默认模式执行，选区优先；如需「整个标签」可用菜单或把默认改成整个标签。
  4. **可见性**：当标签包含多条语句且没有选区时，运行按钮旁显示小徽标「第 N / M 条」（`aria-live="polite"`，随光标移动 100ms 防抖更新）；运行按钮 tooltip 根据模式显示「运行当前语句（⌘↩）」或「运行整个标签（⌘↩）」；单条语句时不显示「第 1 / 1 条」，避免噪声。
  5. **执行与结果**：只把选中的语句文本传给现有 `executeSqlOnTab`；危险 SQL 确认、只读守卫、事务、取消 / 超时、执行历史都作用于实际执行的 SQL；单条执行走现有单结果路径，不残留上一次多结果集；整个标签仍走多结果集路径。状态栏文案区分「执行选中 SQL…」「执行第 N 条…」「执行整个标签…」。
  6. **复杂脚本保护**：如果当前标签文本包含 `DELIMITER` 或 `CREATE TRIGGER ... BEGIN ... END` 等多语句结构，`splitSqlStatements()` 可能误判；在 `statement` 模式下检测到这类结构时，**不猜测当前语句**，禁用当前语句执行并提示「当前脚本包含 DELIMITER / 触发器体，无法确定当前语句；请选中后执行，或使用文件 → 执行 SQL 文件」。`tab` 模式继续沿用现有复杂脚本提示（多语句时要求使用执行 SQL 文件）。单条 `CREATE TRIGGER` 仍可按现有方式执行。
  7. **空 / 注释 / 边界**：空标签、只有空白 / 注释、光标在末尾分号后、光标在两条语句之间的注释中，都要有确定行为：不执行或回退到最近非空语句；不能把空字符串发给数据库，不能误执行后面的语句。多语句之间的 `;` 本身不计入语句文本。
  8. **设置与兼容**：默认值改为 `statement` 是行为变化；设置项允许用户改回 `tab`。现有查询标签 localStorage（`mydb.queryTabs.v1`）不新增字段；执行范围设置独立存储。分屏 / 独立查询窗口共享同一设置，但各自使用自己的编辑器光标；不要在后台窗口读取主窗口光标。
  9. **命令面板 / 菜单**：命令面板「运行当前查询」保持现有 id 与默认范围；新增「运行整个标签」命令（`query.run-all`），执行整个活动标签，受 `runDisabledReason` 约束；原生 / 窗口菜单如需增加入口，复用同一动作，不新增重复逻辑。
  10. **可访问性**：下拉菜单可用键盘（方向键 / Enter / Esc）；徽标有 `aria-label`；不改变 CodeMirror 的查找、补全、格式化快捷键。
- **涉及文件 / 命令**：
  - 新增 `src/lib/query-run-mode.ts`：`loadQueryRunMode()` / `saveQueryRunMode()`、默认值、localStorage 损坏回退。
  - `src/components/SqlEditor.vue`：`Mod-Enter` 保持「选区优先」；新增 `runScope` 事件或在 `run` 事件中携带 `mode: "selection" | "statement" | "tab"`；继续暴露 `currentStatement()`；不把整段 SQL 直接发往后端。若为最小改动，可保留 `run` 事件（选区 / undefined），新增 `run-all` 事件给「整个标签」快捷键 / 菜单。
  - `src/components/QueryWorkspace.vue`：Run 按钮旁的 chevron 菜单、徽标、`run-all` 透传；给按钮 tooltip 传入当前模式。
  - `src/App.vue`：`runQueryOnTab()` 根据「选区 → 模式 → currentStatement」决定 SQL；新增 `runAllQueryOnTab()`；活动 / 分屏编辑器 ref 选择与现有 `currentStatement()` 调用一致；状态栏文案；命令面板 `query.run-all`。
  - `src/components/SettingsDialog.vue`：新增「查询」分区与下拉；复用现有 settings 样式。
  - `src/lib/sql-statements.ts`：如需，补充「光标在语句间时取最近前置非空语句」的纯函数测试；不改变现有分句器语义。
  - 命令：无后端命令。
- **验收标准**：
  - [ ] 输入 3 条 SELECT（分别返回不同结果），光标停在第 2 条，默认模式为「当前语句」，按 `⌘/Ctrl+Enter`：只执行第 2 条，结果区只有第 2 条结果，执行历史只新增 1 条；按钮旁显示「第 2 / 3 条」。
  - [ ] 选中第 1 条的部分文本后按 `⌘/Ctrl+Enter`：只执行选区，不受默认模式影响。
  - [ ] 光标在第 1 条之前 / 最后一条之后 / 两条语句之间：分别执行第 1 条 / 最后一条 / 最近的前一条；空标签或只有注释时禁用并提示，不发送空 SQL。
  - [ ] 设置默认范围为「整个标签」后按 `⌘/Ctrl+Enter`：3 条全部执行，结果区出现「结果 1 / 2 / 3」，与现有行为一致；设置重启后保持。
  - [ ] 通过 Run 下拉选择「运行整个标签」时，即使默认是当前语句也执行全部 3 条，且默认设置不变；选择「运行当前语句」时只执行当前条。
  - [ ] 第 3 条是 `DELETE`：执行第 2 条不会触发危险 SQL 确认；执行第 3 条时确认框只显示第 3 条 SQL；整个标签执行时确认框显示整个标签（保持现有语义）。
  - [ ] 手动事务中执行当前语句：只执行该语句，提交 / 回滚行为不变；取消 / 超时只影响该语句。
  - [ ] 包含 `DELIMITER` 或 `CREATE TRIGGER ... BEGIN ... END` 的多语句脚本：当前语句模式给出明确提示，不误执行半条；选中后执行或使用 SQL 文件仍可用。
  - [ ] 双查询分屏 / 独立查询窗口中分别移动光标并运行：各自执行自己编辑器的当前语句，互不串用；设置共享但光标独立。
  - [ ] 命令面板「运行整个标签」执行全部语句；「运行当前查询」按默认范围执行；禁用原因与现有运行按钮一致。
  - [ ] 执行范围切换不改变 SQL 文本、不改变多结果集实现、不新增数据库请求；单条执行时上一次多结果集不残留。
- **风险 / 限制**：
  - 这是用户可见的默认行为变化：从「无选区执行整个标签」改为「执行当前语句」。必须在设置、Run 下拉、tooltip 和 `CHANGELOG.md` 中说明；用户可切回整个标签。
  - 前端分句器与后端 SQL 文件分句器能力不完全一致（后者支持 `DELIMITER` 等）；当前语句模式必须对无法安全分句的脚本显式拒绝，不能猜测。
  - 光标边界（分号后、注释中、dollar quote 内）容易出现 off-by-one；需要纯函数测试覆盖：空文本、只有注释、尾部分号、字符串内分号、`--` / `#` / `/* */`、`$$ ... $$`。
  - 多语句执行结果、事务、危险 SQL 确认、只读守卫、取消 / 超时都依赖现有 `executeSqlOnTab`；不要在 QueryWorkspace 里另起一条执行路径。
  - 分屏 / 独立窗口有多个编辑器实例；必须明确使用活动窗格 / 当前标签的 `currentStatement()`，不能读取隐藏编辑器。
  - 「最近前置语句」在光标位于两条语句之间时可能不是用户直觉；用徽标提前显示将执行第几条，并在验收中固定行为。
  - 未来若加入「执行整个脚本 / 出错继续 / 事务包裹」（B-07 剩余部分），本项的执行范围设置应能复用，不要设计成互斥的两套模式。

---

## 附录 F-A：与 Backlog / E 批的收口建议

| 本批编号 | 关系 | Backlog / 前序建议 |
|---|---|---|
| F-01 | 新增；E-02 行详情的补充 | 在 Backlog 新增 F-01；不把 E-02 标记为被替代，二者分别解决「一行」与「多行横向对照」 |
| F-02 | 新增；与 E-01 查找分层 | 在 Backlog 新增 F-02；E-01 仍负责查找 / 高亮，F-02 负责减少可见行；F-02 的导出语义需在实现时同步更新 B-02 的快照说明 |
| F-03 | 新增；接手 E-02 明确不做的外键跳转 | 在 Backlog 新增 F-03；不改变 C-08 ER 图与 C-04 结构页外键能力；跨库 / 模式与外键历史仍留在后续 |
| F-04 | 新增；与 E-04 / B-09 导入分层 | 在 Backlog 新增 F-04；E-04 负责 Excel 文件导入，B-09 负责 CSV / JSON 文件导入；F-04 只做剪贴板 + 待提交 |
| F-05 | 增强现有执行链路；与 B-07 部分重叠 | 在 Backlog 新增 F-05；B-07 的「脚本模式计数 / 出错继续 / 事务包裹 / 未执行列表 / 总耗时」仍未完成，F-05 只负责执行范围与当前语句徽标，不替代 B-07 |

**不建议在本批顺手改的旧项**：B-04 / B-24（右侧信息 Dock）、B-26（完整首页）、B-36（报表 / 计划任务）、B-38（工具栏自定义）、B-07 的脚本模式增强。它们仍是有效需求，但与本批「宽表阅读 / 关联导航 / 快速录入 / 执行范围」主题不同，混做会扩大回归面。

## 附录 F-B：实现与验收约定

1. **交付顺序**：按 F-01 → F-02 → F-04 → F-03 → F-05 实现；每项独立写入 `CHANGELOG.md` 的 `[Unreleased]`，便于回滚与逐项验收。
2. **先核对再编码**：每项开始前核对 `ResultGrid.vue` / `TableDataView.vue` / `QueryWorkspace.vue` / `App.vue` / 驱动 trait 的实际结构；本文件的「现状核对」基于 2026-11-26 工作区，若实现侧已重构（例如 E 批已落地），以实际代码为准，但不得改变用户可见行为。
3. **禁止绕过保护**：F-04 的所有新增行必须进入待提交缓冲区，由现有「提交」统一写库；不得在粘贴对话框里直接调用 `insertTableRow` / 导入命令。F-03 只读跳转不得写库；F-05 不得绕过危险 SQL 确认与只读守卫。
4. **共享工具**：F-01 的列宽 / 偏移 helper、F-02 的条件匹配 / 文本格式化、F-04 的 TSV / CSV 解析、F-05 的 `statementAtCursor()` 都应放在 `src/lib/` 纯函数模块中，组件只负责状态与 UI；不要在每个组件里复制一套。
5. **性能上限**：F-01 不复制 DOM；F-02 10,000 行 × 5 条件 <100ms；F-04 单次 5,000 行 / 5 MB；F-03 外键缓存 60 秒 / 20 个范围；F-05 光标徽标 100ms 防抖。超过上限必须提示，不得静默截断或卡死。
6. **手工验收数据库**：优先 `examples/demo.db`（SQLite，已有 orders→users、order_items→orders 外键）；F-01 / F-02 需一个 25–30 列临时表或宽 SELECT；F-03 需复合键 / 自引用临时表；F-04 需真实 Excel / 文本剪贴板；F-05 需 3 条语句和一条危险语句。MySQL / PostgreSQL 无法现场验证时，在交付说明中列出未验证项，不要标记为「全部通过」。
7. **文档**：实现后更新 `README.md` 对应功能条目、`CHANGELOG.md` `[Unreleased]`、`docs/BACKLOG.md` 状态；版本号保持不变，除非用户明确要求「更新版本」。
8. **不新增依赖**：F-01 / F-02 / F-04 / F-05 不新增 npm / Rust 依赖；F-03 不新增 crate。`@tauri-apps/plugin-clipboard-manager` 与 read 权限已经存在，不要重复安装或扩大权限。
9. **可访问性**：所有新增按钮 / 下拉 / 筛选条 / 对话框有可读标签，键盘可操作，焦点可见；计数、粘贴结果、关联加载状态使用 `aria-live` / `role="status"`。
10. **不修改用户数据**：F-01 / F-02 / F-03 / F-05 不写入数据库、连接配置、钥匙串或执行历史；F-04 只在用户确认「加入待提交」后修改内存中的待提交状态，只有用户点击现有「提交」才写库。

## 附录 F-C：参考资料

### DBX（Apache-2.0，仅参考功能与公开文档）

- 数据网格（列冻结、Grid Search、Local value filter、WHERE / ORDER BY、多结果 / 固定结果）：<https://dbxio.com/en/docs/data-grid>
- 查询编辑器（当前语句 / 选区执行、Ctrl/Cmd+Click 对象导航）：<https://dbxio.com/en/docs/query-editor>
- 快捷键（`Mod+Enter` 执行当前语句或选中 SQL，没有默认执行全部 SQL）：<https://dbxio.com/en/docs/keyboard-shortcuts>
- 仓库主页（Apache-2.0）：<https://github.com/t8y2/dbx>

### DBeaver（Apache-2.0，仅参考公开 wiki）

- Data Editor（Grid / Text / Record / Value / References / Metadata）：<https://github.com/dbeaver/dbeaver/wiki/Data-Editor>
- References 面板（被引用行 / 引用行、打开关联表）：<https://github.com/dbeaver/dbeaver/wiki/References-Panel>
- 结果集导航（外键跳转、导航历史）：<https://github.com/dbeaver/dbeaver/wiki/Navigation>
- 数据查看与编辑（剪贴板 TSV、Advanced Paste 多行插入 / NULL 标记）：<https://github.com/dbeaver/dbeaver/wiki/Data-Viewing-and-Editing>

### Navicat（专有软件，仅参考公开产品页 / 手册）

- Data Editor for RDBMS（Grid View / Form View、筛选与排序）：<https://www.navicat.com/en/products/data-editor-for-rdbms?product=navicat-premium>
- Navicat 17 手册（冻结列、Filter & Sort、网格粘贴、Table Profile）：<https://www.navicat.com/manual/pdf_manual/en/navicat_17/win_manual/navicat_en.pdf>

### 本仓库

- `README.md`：当前功能基线、已知限制。
- `CHANGELOG.md`：`[Unreleased]` 与 0.11.0 变更。
- `docs/BACKLOG.md`：B / C / D / E 系列历史状态与长期探索。
- `docs/ARCHITECTURE.md`：命令 / 驱动 / 前端分层约束。

> 竞品功能仅用于确认「用户是否需要」与「交互应该怎样」，不得复制其源码、品牌图标、截图或专有文案。mydb 保持独立实现与 Apache-2.0 许可。

---

## 附：F 批变更记录

- 2026-11-26：追加 F 批（F-01～F-05：列冻结、查询结果本地筛选、外键关联跳转、剪贴板多行粘贴、SQL 执行范围控制）；完成 DBX / DBeaver / Navicat 第二批差距核对，明确与 E 批及已实现能力的分层；推荐顺序 F-01 → F-02 → F-04 → F-03 → F-05。