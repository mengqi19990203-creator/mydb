# mydb 产品需求（2026-11-25 第三批）

> 维护人：产品（PM）
> 基线版本：v0.11.0（2026-11-24）
> 第一批状态：C-01～C-08 已落地，见 `docs/PM-REQUIREMENTS-2026-11-25.md` 的 0.6
> 第二批状态：实现侧已完成基础格式化、EXPLAIN 可视化、对象创建、连接导入 / 导出；全局搜索进行中。第二批文档中的对象定义、表数据高级、用户 / 权限、备份计划仍未实现。
> 文档状态：待评审 / 待排期
> 适用范围：SQLite / MySQL（含 MariaDB）/ PostgreSQL；Redis 不在本批新增范围内
> 关联文档：`docs/PM-REQUIREMENTS-2026-11-25.md`、`docs/PM-REQUIREMENTS-2026-11-25-batch2.md`、`docs/BACKLOG.md`、`docs/ARCHITECTURE.md`、`CHANGELOG.md`

---

## 0. 本批结论

### 0.1 一句话目标

在「能搬数据、能改表、能看计划」之后，补上**写 SQL 与交付数据库结构的效率工具**：SQL 片段库、对象定义编辑、表数据高级查看、测试数据生成、只读用户 / 备份、受限查询构建器、数据库文档导出。本批不重复已完成的格式化、EXPLAIN、对象创建、连接导入导出与进行中的全局搜索。

### 0.2 需求清单（按实施优先级排序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 依赖 | 关系 / 去重 |
|---|---|---|---|---|---|---|
| 1 | D-01 | SQL 片段库（保存 / 搜索 / 插入） | P0 | 1 个开发会话 | 现有 `SqlEditor`、localStorage | 新增；无 Backlog 编号 |
| 2 | D-02 | 对象定义查看与编辑 | P1 | 1.5 个开发会话 | `get_object_ddl`、C-02 单语句执行 | 接手第二批未实现的 C2-03；对象创建已完成，不重复 |
| 3 | D-03 | 表数据高级能力：值查看器 / 虚拟滚动 / XLSX | P1 | 1.5–2 个开发会话 | 现有结果网格、C-03 表数据页 | 接手第二批未实现的 C2-05；不重复 C-03 批量编辑 |
| 4 | D-04 | 数据生成器（测试数据填充） | P1 | 1.5 个开发会话 | 表结构元数据、C-03 批量写入 | 新增；`BACKLOG.md` 的长期非目标，本项只做受控 MVP |
| 5 | D-05 | 用户 / 权限只读与服务器概览增强 | P2 | 1–1.5 个开发会话 | C-07 服务器活动面板 | 接手第二批未实现的 C2-07；不做授权 / 回收 |
| 6 | D-06 | 应用内备份计划 | P2 | 1.5–2 个开发会话 | C-01 导出、C-02 还原 | 接手第二批未实现的 C2-08；不做系统级调度 |
| 7 | D-07 | 查询构建器可行性（受限 SELECT 构建） | P2 | 2 个开发会话 | 现有元数据、SQL 引用工具 | 新增；重新评估 `BACKLOG.md` 的长期非目标，只做受限 MVP |
| 8 | D-08 | 数据库文档与 DBML 导出 | P2 | 1.5 个开发会话 | 现有元数据、ER 关系、本地笔记 | 新增；不是完整模型设计器 / PDF 报表引擎 |

> 「1 个开发会话」指 Claude 在一次连续实现中完成编码、`npm run check`、`cargo test` 与本文件列出的可自动化验收项；真实数据库手工验收可单独安排。

### 0.3 编号差异与接手说明

第二批文档（磁盘版）的编号与实现侧口径存在差异，本文件用 **D-01～D-08** 避免歧义：

- 实现侧已完成：**基础格式化、EXPLAIN 可视化、对象创建（视图 / 过程 / 函数 / 触发器）、连接导入 / 导出**；
- 进行中：**全局对象搜索**（第二批 C2-04）；
- 第二批文档中**尚未实现**的能力：对象定义查看 / 编辑、表数据值查看器 / 虚拟滚动 / XLSX、用户 / 权限只读、应用内备份计划。它们由本批 **D-02 / D-03 / D-05 / D-06** 接手；若实现侧在第三批启动前已经完成，则对应 D 项关闭，不重复开发。
- 第二批磁盘版中的「数据库 / 模式创建删除」「查询取消 / 超时」如仍未实现，保留在第二批待办，本文件不重复。
- 本批**不重复**：C-01～C-08 的任何能力；基础格式化；EXPLAIN；对象创建；连接导入导出；全局搜索（进行中）。

### 0.4 本批明确不做

| 能力 | 结论 | 理由 |
|---|---|---|
| 完整字段级血缘 | 不做 | 需要解析视图 / 函数 / 触发器 / 动态 SQL，误报代价高；数据库文档只能展示外键关系与字段定义，不推断数据流。 |
| 完整查询构建器（聚合、子查询、CTE、UNION、画布） | D-07 只做受限 SELECT | 完整构建器是 IDE 级能力；先用受限 MVP 验证交互与 SQL 生成质量，再决定是否扩展。 |
| 数据库模型设计 / 正向工程 / 反向工程 | D-08 只做文档与 DBML 导出 | 模型设计需要画布、布局、版本与 DDL 生成，工作量远超本批；DBML 单向导出先满足交接与评审。 |
| PDF 报表引擎 / 定时报表 / 打印服务 | 不做 | 需要报表模板、分页、字体与 PDF 依赖；D-08 只导出可打印 HTML 与 Markdown。 |
| 真实 PII / 生产数据脱敏 | 不做 | D-04 只生成合成测试数据，不读取生产数据做脱敏；避免隐私与合规风险。 |
| 外键感知的数据生成 / 跨表引用完整性 | D-04 第一版不做 | 需要依赖图、父表采样与冲突处理；第一版只生成单表数据，外键列可手工填固定值或留空。 |
| 系统级备份调度 / 守护进程 | D-06 只做应用内计划 | 与「轻量、无后台服务」定位冲突；应用关闭时不执行，界面明确说明。 |
| 用户授权 / 回收 / 改密 | D-05 只读 | MySQL 8 与 PostgreSQL 权限模型差异大，误操作风险高。 |
| 片段 / 笔记云同步 | 不做 | 需要账号与云服务；本批只做本地存储与 JSON 导入导出。 |
| AI 生成 SQL / AI 生成数据 | 不做 | 外部模型服务、隐私与凭据管理不在本批范围。 |

### 0.5 通用约束

- **只写需求与验收，不写实现代码**；实现按单个需求独立完成、独立验收。
- **不提升版本号**：改动先写 `CHANGELOG.md` 的 `[Unreleased]`；只有用户明确说「更新版本」才提升。
- **只读连接**：D-02 保存定义、D-04 写入数据、D-06 备份输出目录之外的写操作必须在后端命令层拒绝；查看、搜索、导出、生成预览允许。
- **不复制品牌资源**：Navicat / DBeaver 只参考功能与公开行为，不复制图标、截图、专有代码；DBX 为 Apache-2.0，复制代码片段需在文件头注明来源与许可证。
- **新增依赖需说明**：D-03 允许引入 `rust_xlsxwriter`（MIT OR Apache-2.0）；D-04 优先用现有依赖实现确定性随机数，不引入重型 faker 库；任何新依赖都要登记到 README / 许可证说明。
- **存储**：片段、笔记、生成器预设、备份计划均使用带版本号的本地 JSON / localStorage；损坏时回退默认并保留备份，不阻塞应用启动。
- **隐私**：片段可能含用户粘贴的敏感 SQL，界面提示不要保存密码 / 令牌；数据生成只产生合成值；日志不记录完整 SQL / 凭据。
- **性能上限**：片段 500 条 / 单条 50 KB；数据生成 10,000 行；文档导出 1,000 张表；DBML / HTML 单文件 20 MB；超过必须提示。
- **验收环境**：SQLite 用 `examples/demo.db`；MySQL / PostgreSQL 用真实实例（README 的 `MYDB_TEST_*`）。无实例时至少完成 SQLite + 单元测试，并标注未覆盖项。
- **完成定义（DoD）**：`npm run check` + `cargo test` 通过；验收清单逐项勾选；`CHANGELOG.md` 写入 `[Unreleased]`；README / 已知限制更新；Backlog 对应项标记「已被 D-xx 替代 / 接手」。

---

## D-01 SQL 片段库（保存 / 搜索 / 插入）

- **优先级**：P0
- **预估**：1 个开发会话
- **依赖**：现有 `SqlEditor`、`QueryWorkspace`、localStorage
- **关系**：新增；不依赖后端，不访问数据库。第一项可在本会话完成。

### 用户问题 / 场景

日常查询里反复出现同一批模板：分页查询、按时间范围统计、建表骨架、事务包裹、EXPLAIN、常用 JOIN。每次都要从历史记录里翻或从别的文件复制，历史记录还会被新查询挤出 100 条。用户需要一个轻量的「我的 SQL 片段」：保存当前选区 / 当前语句，命名、打标签、搜索，一键插入光标位置；不执行、不联网。

### 期望行为

1. **入口**：
   - SQL 编辑器标题栏增加「片段」按钮；
   - 编辑菜单增加「SQL 片段库…」与「保存选区为片段…」；
   - 快捷键 `Shift+Mod+K` 打开片段库；命令面板增加同名命令；
   - 只在查询标签可用；表 / ER / Redis 面板不显示。
2. **数据模型**（本地）：
   - `id`、`name`（必填）、`trigger`（短标识，必填且唯一）、`body`（SQL 文本）、`tags`（字符串数组）、`driver`（`any` / `mysql` / `postgres` / `sqlite`）、`builtin`（只读内置）、`createdAt` / `updatedAt`；
   - 存储键 `mydb.sqlSnippets.v1`，带版本号；最多 500 条，单条正文最多 50 KB；
   - 首次启动内置 6 条只读片段：`sel`（SELECT *）、`cnt`（COUNT）、`join`（INNER JOIN 模板）、`ins`（INSERT 模板）、`upd`（UPDATE 模板）、`tx`（事务包裹）；内置可「复制为我的片段」，不能直接改。
3. **片段库对话框**：
   - 左侧列表：搜索框、过滤（全部 / 我的 / 内置 / 当前驱动）、按名称 / trigger / 标签排序；
   - 搜索匹配名称、trigger、标签与正文（正文只做子串匹配，不渲染 SQL）；
   - 右侧预览：完整正文、驱动标签、标签、更新时间；
   - 操作：插入、编辑、复制、删除（确认）、新建、导入 JSON、导出 JSON；
   - 空状态引导「选中 SQL → 保存为片段」。
4. **保存**：
   - 「保存选区为片段」：有选区保存选区，无选区保存光标所在语句；两者都为空时提示；
   - 打开编辑对话框预填正文，用户填写名称 / trigger / 标签 / 驱动；
   - trigger 不能为空、不能重复、只能包含字母 / 数字 / 下划线 / 短横线；重复时提示并阻止保存；
   - 保存后不自动插入。
5. **插入**：
   - 有选区时替换选区，无选区时插入光标位置；
   - 正文中的 `${cursor}` 作为插入后的光标位置标记（只处理第一个；没有标记则光标放在插入文本末尾）；
   - 插入是一个 CodeMirror 事务，`Mod+Z` 一次撤销；
   - 插入后聚焦编辑器，不执行 SQL、不新增历史、不改变连接。
6. **驱动过滤**：
   - 当前连接为 MySQL 时显示 `any` 与 `mysql` 片段；PostgreSQL / SQLite 同理；
   - 不匹配的片段在「全部」中可见但标注「不适用于当前驱动」，插入前提示（允许继续，用户自己负责）。
7. **导入 / 导出**：
   - 导出为 JSON：`{version: 1, snippets: [...]}`，不含连接信息 / 密码；
   - 导入时校验版本与字段；trigger 冲突自动加 `-2` 后缀，名称冲突允许并存；
   - 文件上限 2 MB；损坏时提示，现有片段不变。
8. **安全与隐私**：
   - 提示「片段保存在本机，不要保存密码、令牌或完整连接串」；
   - 不把片段写入执行历史 / 日志；不做云同步。
9. **边界**：
   - 第一版不做编辑器自动补全触发（trigger 仅用于搜索 / 标识）；不做多光标占位符跳转；不做跨设备同步。这些留待后续。

### 验收标准

- [ ] 选中一段 SQL 点「保存选区为片段」，填写名称 `分页查询`、trigger `page`，保存后片段库出现该条；重启应用后仍存在。
- [ ] 无选区、光标在第二条语句内时保存，正文为第二条语句，不包含第一条 / 第三条。
- [ ] 片段库搜索 `page`、`分页`、标签均能命中；搜索结果点击后预览正确。
- [ ] 当前连接 MySQL 时只显示 `any` / `mysql` 片段；PostgreSQL 片段在「全部」中标注不适用，插入前提示。
- [ ] 在空编辑器插入 `sel`，文本出现在光标处，光标位于 `${cursor}` 标记处；一次 `Mod+Z` 撤销整个插入。
- [ ] 有选区时插入片段，只替换选区，选区外文本不变。
- [ ] trigger 为空 / 重复 / 含空格时保存被阻止并提示；修改为唯一值后保存成功。
- [ ] 内置片段不能编辑 / 删除，但可「复制为我的片段」后修改。
- [ ] 导出 JSON 后清空本地片段，再导入，片段数量、名称、trigger、正文、标签、驱动恢复一致；无密码 / 连接信息。
- [ ] 导入损坏 JSON 或超过 2 MB 时提示，现有片段不变。
- [ ] 删除片段有确认；删除后列表与存储同步，重启后不恢复。
- [ ] 插入 / 保存片段不执行 SQL、不新增执行历史、不发送数据库请求。

### 涉及文件与命令

**前端**

- 新增 `src/components/SqlSnippetDialog.vue`：列表、搜索、过滤、预览、新建 / 编辑 / 删除 / 插入 / 导入导出。
- 新增 `src/components/SqlSnippetEditorDialog.vue`（或内嵌）：名称、trigger、正文、标签、驱动。
- 新增 `src/lib/sql-snippets.ts`：类型、内置片段、localStorage 读写、校验、冲突处理、JSON 导入导出。
- 修改 `src/components/SqlEditor.vue`：暴露 `insertSnippet(text)` / `replaceSelection(text)`，处理 `${cursor}`、单事务、聚焦。
- 修改 `src/components/QueryWorkspace.vue`：标题栏「片段」按钮；保存选区入口。
- 修改 `src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`、`src/App.vue`：编辑菜单 / 命令面板 / 快捷键。
- 修改 `src/lib/shortcuts.ts`、`src/components/ShortcutHelpDialog.vue`：登记 `Shift+Mod+K`。
- 修改 `src/styles.css`：片段库与编辑对话框。

**后端**

- 无新增命令；不访问数据库。

### 风险与限制

- **本地存储**：localStorage 可能被清理或受配额限制；导出 JSON 是唯一备份手段，界面需提示。超过 500 条 / 50 KB 时阻止新增。
- **敏感内容**：片段正文可能含用户误存的密码；不写日志、不上传，导入导出前提醒。
- **无自动补全**：第一版 trigger 不参与 CodeMirror 补全，避免与现有表名 / 字段补全冲突；后续可加。
- **驱动差异**：片段可能含方言专属语法；只做标注与提示，不做语法校验。
- **多标签**：片段库全局共享，不按标签隔离；插入只作用于当前活动编辑器。
- **撤销**：必须单事务，否则一次撤销只删一部分文本。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/editor/SnippetQuickAddDialog.vue`；`apps/desktop/src/lib/sql/sqlSnippetTemplates.ts`、`snippetRangeSelection.ts`、`vscodeSnippetsImport.ts`；文档 `docs/content/docs/sql-snippets.cn.mdx`（标签、触发键、状态、占位符 `${}` / `${name}` / `${1:default}`）。参考其「片段管理 + 编辑器展开」；本批不实现占位符跳转与云同步。
- **Navicat**：查询编辑器的代码片段 / 片段库；参考官方手册的 Query Editor 章节。
- **DBeaver**：SQL 模板 / Templates 视图；参考 DBeaver 的 SQL Templates 文档。只参考功能，不复制实现。

---

## D-02 对象定义查看与编辑

- **优先级**：P1
- **预估**：1.5 个开发会话
- **依赖**：`get_object_ddl`、现有 `execute_query`、C-02 单语句执行
- **关系**：接手第二批未实现的 C2-03（对象定义部分）。对象创建已由 `CreateObjectDialog` 完成，本项不重复创建。

### 用户问题 / 场景

对象已经能创建，但建完之后只能「复制 DDL」到外部编辑器看；想在应用内核对视图、函数、触发器定义，或改一处再保存，仍做不到。用户需要「右键查看定义 → 搜索 / 复制 / 另存为 → 受控编辑 → 预览 → 保存」的闭环。

### 期望行为

1. **查看定义**：对有 DDL 的对象右键「查看定义…」；打开 `ObjectSourceDialog.vue`，显示完整 DDL，带行号、语法高亮、自动换行、搜索、复制、另存为 `.sql`；只读、不执行、不修改对象。
2. **范围**：
   - MySQL：表、视图、存储过程、函数、触发器、事件；
   - PostgreSQL：表、视图、物化视图、函数、存储过程、触发器、索引、序列；
   - SQLite：表、视图、索引、触发器。
3. **可编辑范围**：
   - 可编辑：MySQL / PostgreSQL 视图；PostgreSQL 函数 / 存储过程；SQLite 视图 / 触发器；
   - 只读：MySQL 存储过程 / 函数 / 触发器 / 事件（避免 `DELIMITER` 与依赖问题）；表结构 / 索引 / 序列继续走结构页。
4. **编辑流程**：编辑 → 预览（目标、完整 SQL、是否包含 DROP）→ 保存；视图 / PostgreSQL 例程用 `CREATE OR REPLACE`；SQLite 视图 / 触发器用 `DROP + CREATE` 并在一个事务中执行；成功后重新读取定义、刷新对象树；失败保留编辑内容，原定义不变。
5. **PostgreSQL 重载函数**：先列出候选签名（名称 + 参数类型 + 返回类型），选择后读取；未选择或无法唯一确定时禁止编辑；后端新增 `list_object_variants`，`get_object_ddl` 接受可选 identity。
6. **搜索**：对话框内本地文本搜索 / 高亮 / 下一个；不发送数据库请求。
7. **安全**：只读连接禁用保存并在后端拒绝；定义文本上限 2 MB；不写历史 / 日志；不自动 `DROP ... CASCADE`；不修改依赖对象。
8. **错误**：读取失败显示原始错误并保留对话框；保存失败显示失败语句与错误；对象已删除 / 重建时提示重新打开，不覆盖同名新对象。

### 验收标准

- [ ] MySQL 存储过程右键「查看定义」显示完整 `CREATE PROCEDURE ... BEGIN ... END`，可搜索、复制、另存为；编辑禁用并说明原因。
- [ ] PostgreSQL 视图、物化视图、函数、触发器、索引均可查看可读定义，与 `pg_get_*` 一致。
- [ ] SQLite 视图 / 触发器可查看 `sqlite_master` 原始 DDL。
- [ ] PostgreSQL 重载函数查看时列出至少两个签名，选择后显示对应定义；未选择时不能编辑。
- [ ] MySQL / PostgreSQL 视图编辑保存后查询结果按新定义变化，对象树刷新；失败时原定义不变、编辑内容保留。
- [ ] PostgreSQL 函数 / 存储过程编辑使用 `CREATE OR REPLACE`，保存后依赖对象未被删除。
- [ ] SQLite 触发器编辑预览同时显示 `DROP TRIGGER` 与 `CREATE TRIGGER`；失败回滚，原触发器仍在。
- [ ] SQLite 视图编辑预览显示 `DROP VIEW` / `CREATE VIEW`；失败回滚。
- [ ] 只读连接查看可用、保存禁用，绕过前端后端返回只读错误。
- [ ] 超过 2 MB 的定义只读展示并提示；保存失败不丢失编辑内容。

### 涉及文件与命令

**前端**

- 新增 `src/components/ObjectSourceDialog.vue`：查看 / 编辑 / 预览 / 搜索 / 另存为。
- 新增 `src/components/ObjectSourcePreview.vue`：完整 SQL、目标、DROP 提示。
- 修改 `src/components/ObjectCategoryGroups.vue`：右键「查看定义…」。
- 修改 `src/App.vue`：打开对话框、保存后刷新对象树 / 重新读取。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：`getObjectDdl` identity、`listObjectVariants`。
- 修改 `src/styles.css`：定义对话框与搜索高亮。

**后端**

- 修改 `src-tauri/src/commands/data.rs`：`get_object_ddl` 增加 identity；新增 `list_object_variants`；可选 `save_object_definition`。
- 修改 `src-tauri/src/db/driver.rs`：新增 `object_variants` / `object_ddl_with_identity` 默认方法。
- 修改 `src-tauri/src/db/mysql.rs`、`postgres.rs`、`sqlite.rs`：定义读取、变体、保存（SQLite 事务 DROP + CREATE）。
- 修改 `src-tauri/src/lib.rs`：注册命令。

### 风险与限制

- 重载函数必须按 OID / 签名定位，不能按名称取第一个。
- SQLite 无 `CREATE OR REPLACE`，DROP + CREATE 必须在事务中；失败回滚。
- 对象依赖不分析，只提示风险；不自动级联删除。
- MySQL 例程只读是刻意限制，后续单独评估。
- 定义可能缺注释 / 权限 / 存储属性；以数据库返回的完整 DDL 为准。
- 对象被删除 / 重建后保存可能覆盖新对象；后端需校验标识并提示。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/objects/ObjectSourceDialog.vue`、`ObjectSourceSqlPreview.vue`；`apps/desktop/src/lib/table/objectSource*.ts`、`routineDefinitionSave.ts`；文档 `docs/content/docs/schema-browser.cn.mdx`、`object-browser.cn.mdx`。
- **Navicat**：对象设计器 / 源码编辑；参考官方手册的 View / Procedure / Function / Trigger 章节。
- **mydb 现状**：`ObjectCategoryGroups.vue::menuCopyDdl`、`get_object_ddl`、`CreateObjectDialog.vue`。

---

## D-03 表数据高级能力：值查看器 / 虚拟滚动 / XLSX

- **优先级**：P1
- **预估**：1.5–2 个开发会话
- **依赖**：现有 `ResultGrid.vue`、`TableDataView.vue`、`result-export.ts`
- **关系**：接手第二批未实现的 C2-05。C-03 的待提交 / 批量编辑 / 筛选排序分页不重复。

### 用户问题 / 场景

JSON / JSONB、长文本、超宽表在单元格里被压成一行；结果 10,000 行时一次性渲染所有行导致滚动卡顿；导出只有 CSV / JSON / Markdown / SQL，交给业务同事还要再转 Excel。用户需要「点开看值、滚动不卡、直接给 .xlsx」。

### 期望行为

1. **值查看器**：
   - 结果网格右键「查看值…」，双击非数字 / 非布尔单元格也可打开；表数据页右键「查看值…」，双击仍编辑；
   - 显示列名、行号、字符数 / 字节数；「格式化 / 原始」切换；合法 JSON 默认 2 空格格式化，非法 JSON 只显示原文并提示；
   - 自动换行、复制、另存为 `.txt` / `.json`；NULL 显示 `NULL`；BLOB / BYTEA 显示占位与字节数，不做二进制解码；
   - 2 MB 以上只读展示；Esc / 遮罩关闭，焦点回原单元格。
2. **虚拟滚动**：
   - 结果网格超过 100 行启用；只渲染视口 + 10 行 overscan；固定行高、表头 sticky；
   - 列宽、隐藏列、排序、行号、右键菜单、复制、导出快照保持现有行为；10,000 行滚动到底无白屏，`tbody tr` 不超过 200；
   - 表数据页 500 行时同样启用；待提交高亮、行选择、批量设置、单元格编辑保持可用；编辑中的行不得被回收导致输入丢失；
   - 共享 `useVirtualRows` / `VirtualTableBody`，虚拟化只影响渲染，不影响导出 / 复制全部。
3. **XLSX 导出**：
   - 查询结果「导出…」增加 XLSX；表数据页导出增加 XLSX（当前页）；对象右键导出增加 XLSX（全部 / 指定行数）；
   - 表头加粗、冻结首行、自动筛选；数字写数值、布尔写布尔、NULL 空单元格、JSON 写文本、二进制写占位；
   - 字符串以 `=`、`+`、`-`、`@` 开头必须按文本写入，禁止公式解释；
   - 工作表名清理 `[]:*?/\` 并截断 31 字符；列宽估算上限 60；
   - 结果 / 当前页导出上限 10,000 / 500 行；对象导出上限 200,000 行，超过拒绝并提示；
   - 使用 `rust_xlsxwriter`（MIT OR Apache-2.0），临时文件 + rename，失败 / 取消清理。

### 验收标准

- [ ] JSON 单元格打开后默认格式化，可切原始；复制内容与当前视图一致；非法 JSON 不空白、不抛错。
- [ ] 100 KB 文本查看器滚动流畅；自动换行、另存为 `.txt` 正常。
- [ ] NULL / BLOB 占位正确，无乱码。
- [ ] 结果 10,000 行滚动到底，DOM 行数少于 200；排序、隐藏列、右键、导出仍正确。
- [ ] 表数据页 500 行虚拟滚动生效；行选择、批量设置、编辑、待提交高亮正常。
- [ ] 编辑第 400 行后滚动离开再回来，待提交值仍保留。
- [ ] 查询结果导出 XLSX，Excel / Numbers 打开：表头、冻结、筛选、中文正确。
- [ ] 表数据当前页 XLSX 行数一致；NULL 为空，数字为数值。
- [ ] 对象导出 10,000 行 XLSX 行数正确；超过 200,000 行拒绝并提示。
- [ ] `=1+1`、`+SUM(A1)` 在 XLSX 中作为文本显示，不执行公式。
- [ ] 失败 / 取消后无残留临时文件；数据库无写入，历史不新增。

### 涉及文件与命令

**前端**

- 新增 `src/components/CellValueDialog.vue`。
- 新增 `src/components/VirtualTableBody.vue` / `src/lib/virtual-rows.ts`。
- 修改 `src/components/ResultGrid.vue`、`src/components/TableDataView.vue`：查看值、虚拟滚动、编辑态保护。
- 修改 `src/components/ResultExportDialog.vue`、`src/lib/result-export.ts`、`src/components/ExportDialog.vue`：XLSX 选项。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`；`src/styles.css`。

**后端**

- 新增 `src-tauri/src/commands/xlsx.rs`：`export_query_result_xlsx`。
- 修改 `src-tauri/src/commands/data.rs`：`export_object` 增加 `xlsx`；复用 `export_rows`，限制 200k。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/Cargo.toml`：`rust_xlsxwriter`。
- 修改 `src-tauri/src/models.rs`：XLSX 请求 / 汇总。

### 风险与限制

- 虚拟滚动 + 可编辑表格：编辑中的行必须固定，或先结束编辑再回收。
- 列宽 / 横向滚动对齐需实测；`<colgroup>` 与表头必须一致。
- XLSX 内存构建；20 万行是上限，超过拒绝。
- 日期类型第一版统一文本，避免格式错误但失去 Excel 日期排序。
- 公式注入必须按文本写入，不能用公式 API。
- 二进制不写原始字节；需要时用 SQL dump。

### 参考

- **DBX（Apache-2.0）**：`DataGridCellDetailDialog.vue`、`DataGridCellDetailTextPreview.vue`；`common/VirtualScrollArea.vue`、`JsonTree.vue`；`lib/export/xlsxExport.ts`；`src-tauri/src/commands/xlsx_export.rs`；文档 `data-grid.cn.mdx`。
- **Navicat**：数据查看器备注 / JSON 查看、虚拟滚动、导出 Excel；参考官方手册的 Table Data Viewer / Export Wizard。
- **第三方库**：`rust_xlsxwriter`（MIT OR Apache-2.0）。

---

## D-04 数据生成器（测试数据填充）

- **优先级**：P1
- **预估**：1.5 个开发会话
- **依赖**：表结构元数据、C-03 的批量写入 / 事务能力
- **关系**：新增。`BACKLOG.md` 曾把「数据生成」列为长期非目标；本项只做受控的测试数据生成，不读取生产数据、不做脱敏。

### 用户问题 / 场景

建好表后要造几十到几千行测试数据：手写 `INSERT` 或被 `NOT NULL`、唯一键、日期 / 邮箱 / 姓名格式反复卡住；从生产库复制又涉及隐私。用户需要按列选择生成器、预览前 20 行、在一个事务里批量插入，失败能回滚并指出行号。

### 期望行为

1. **入口**：表数据页工具栏「生成数据…」；对象树表右键「生成数据…」；只对表开放，视图 / 只读连接禁用。
2. **配置**：
   - 行数 1–10,000，默认 100；批大小 100–1,000，默认 500；
   - 可选随机种子（相同种子生成相同数据，便于复现）；
   - 每列一行配置：生成器、参数、NULL 比例（0–100%）、是否启用；
   - 自动跳过自增列、生成列、二进制列；主键非自增列默认使用「序列」生成器；
   - 可达列：名称、类型、可空、默认值、主键 / 自增信息来自现有 `getTableStructure`。
3. **生成器（第一版）**：
   - `sequence`：起始值 + 步长；
   - `integer`：最小 / 最大；
   - `decimal`：最小 / 最大 / 小数位；
   - `text`：长度范围、字符集（字母 / 数字 / 混合）；
   - `regex`：有限正则（字面量、字符类、`\d`、`\w`、量词 `{n}` / `{n,m}`；不支持回溯引用 / 环视）；
   - `email` / `name` / `phone` / `company`：内置合成词库，不联网；
   - `date` / `datetime`：起止范围、格式；
   - `boolean`：true 比例；
   - `uuid`：v4 格式（用本地随机源）；
   - `fixed`：固定值；
   - `null`：始终 NULL（仅可空列）。
   - 每列有「按类型智能默认」：整数 → integer，文本 → text，日期 → date，布尔 → boolean，UUID → uuid。
4. **预览与 SQL**：
   - 预览前 20 行（表格），并显示将执行的批量 INSERT 预览（参数以字面量展示）；
   - 用户可改行数 / 生成器后重新生成预览；预览不写库。
5. **执行**：
   - 在一个事务中按批插入；全部成功才提交；任一失败整体回滚，错误包含失败行号 / 列名 / 数据库原始错误；
   - 唯一键冲突、`NOT NULL` 违反、外键失败都按上述方式报告；
   - 显示进度（已生成 / 已插入行数）并可取消；取消后回滚，目标表无新增行；
   - 只读连接在命令层拒绝。
6. **限制**：
   - 第一版不做外键感知生成、跨表引用、唯一值自动去重、数据脱敏；
   - 不生成 BLOB / 二进制 / 几何类型；这些列必须留空或固定值；
   - 不读取表中现有数据（除结构元数据）；不联网。
7. **复用**：插入走 C-03 的批量变更 / 事务通道，不新造一套参数绑定逻辑；如果 C-03 未实现，则后端新增 `generate_table_data` 命令统一处理。

### 验收标准

- [ ] 对含自增主键、`VARCHAR`、`INT`、`DATE`、`BOOLEAN` 的表生成 100 行：自增列跳过，其他列类型正确，提交后行数 +100。
- [ ] 预览显示前 20 行与 INSERT 预览；修改行数 / 生成器后预览更新，数据库无变化。
- [ ] 相同随机种子生成两次，数据完全一致；不同种子不同。
- [ ] 文本长度、整数范围、日期范围、布尔比例符合配置。
- [ ] NULL 比例 50% 时约一半行该列为 NULL（允许统计误差），仅可空列可设置。
- [ ] 唯一键冲突时整体回滚，表中无新增行，错误指出冲突行 / 列。
- [ ] `NOT NULL` 列未配置生成器且无默认值时，开始前阻止并指出列。
- [ ] 取消 5,000 行生成：当前批次结束后停止并回滚，表中无新增行；进度显示正确。
- [ ] 只读连接入口禁用，绕过前端后端拒绝。
- [ ] 10,000 行生成在可接受时间内完成（分批提交 / 事务），界面不冻结。
- [ ] 二进制 / 生成列不出现在生成器列表中；固定值写入按类型转义。
- [ ] 生成过程不联网、不读取表数据、不写执行历史（或只记一条 DDL/生成摘要）。

### 涉及文件与命令

**前端**

- 新增 `src/components/GenerateDataDialog.vue`：行数 / 种子 / 列配置 / 预览 / 进度。
- 新增 `src/components/GeneratorColumnEditor.vue`：生成器选择与参数。
- 新增 `src/lib/data-generator.ts`：类型映射、默认生成器、本地预览（前 20 行）、参数校验。
- 修改 `src/components/TableDataView.vue`、`src/components/ObjectCategoryGroups.vue`：入口与禁用。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：生成请求 / 进度 / 结果。
- 修改 `src/styles.css`。

**后端**

- 新增 `src-tauri/src/commands/generate.rs`（或扩展 `data.rs`）：`generate_table_data(connection_id, table, plan, count, seed, batch_size, operation_id)`、`cancel_generate(operation_id)`。
- 修改 `src-tauri/src/models.rs`：`GeneratePlan`、`ColumnGenerator`、`GenerateProgress`、`GenerateSummary`。
- 修改 `src-tauri/src/db/driver.rs`：优先复用 `insert_rows_batch`（若 C-05 已提供）；否则新增批量插入默认方法。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 随机数：优先用本地确定性 PRNG（xorshift / PCG，自实现，无新依赖），种子为 u64；不使用系统随机源保证可复现。

### 风险与限制

- **唯一键**：随机数据可能冲突；第一版不做自动去重，失败整体回滚并提示，用户可增大范围 / 降低行数。
- **外键**：不生成引用值；外键列需用户固定值或留空（若可空），否则数据库报错。
- **类型解析**：`ColumnInfo.data_type` 是数据库原始类型字符串；解析失败时回退 text 生成器并提示。
- **大字段**：不生成二进制 / 超大文本；文本上限 10,000 字符。
- **性能**：10,000 行 × 宽表可能很慢；分批 + 事务 + 进度；取消只保证当前批次后停止。
- **隐私**：内置姓名 / 邮箱 / 电话都是合成值，不使用真实名单；不联网。
- **只读 / 权限**：命令层拒绝只读；数据库权限不足时原始错误。
- **C-03 依赖**：若 C-03 的批量写入接口未实现，需先补最小批量插入，不能逐行 N 次往返。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/generate/DataGenerateDialog.vue`、`GeneratorSelect.vue`、`params/*`（Number / Text / Regex / Date / Boolean / UUID / Sequence / ForeignKey 等）；`ModelGenerateDialog.vue`。参考其「按列选生成器 + 参数 + 预览」；本批只做单表、无外键感知的精简集合。
- **Navicat**：数据生成（Data Generation）；参考官方产品页 `https://www.navicat.com/en/products/data-generation?product=navicat-premium` 与官方手册的 Data Generation 章节。
- **DBeaver**：数据生成 / Mock Data 功能（版本 / edition 不同）；只参考交互，不复制实现。

---

## D-05 用户 / 权限只读与服务器概览增强

- **优先级**：P2
- **预估**：1–1.5 个开发会话
- **依赖**：C-07 的 `ServerActivityDialog.vue`、`server_overview` / `server_processes`
- **关系**：接手第二批未实现的 C2-07；不做授权 / 回收 / 改密。

### 用户问题 / 场景

C-07 已能看连接数、运行时长和进程，但排查「谁有权限」「为什么这个账号连不上」「当前库多大、哪张表最占空间」仍要手写系统表 SQL。用户需要只读查看用户 / 角色、角色属性、当前库权限，以及连接使用率、库大小和 Top 表；绝不能暴露密码散列。

### 期望行为

1. 沿用「工具 → 服务器活动…」对话框，新增「用户」页签；概览页扩展指标；只读连接可看。
2. **概览增强**：
   - 公共：版本、运行时长、当前用户、当前库 / 模式、连接数 / 最大连接数、使用率、当前库大小、表数；
   - MySQL：`Threads_running`、`Queries`、`Slow_queries`、QPS（两次采样差值）、InnoDB 缓冲池命中率、Top 5 表（数据 + 索引大小）；
   - PostgreSQL：活动 / 空闲会话、QPS、缓存命中率、Top 5 表（`pg_total_relation_size`）；
   - 指标不可得显示「不支持 / 无权限」，不影响其他指标；QPS 首次显示「采样中」。
3. **用户 / 角色页（只读）**：
   - MySQL：User、Host、plugin、account_locked、password_expired、password_last_changed（列不存在留空）；选中后按需 `SHOW GRANTS FOR`；**禁止查询 / 展示 `authentication_string`**；`mysql.user` 不可读时回退 `SELECT user, host`，仍失败显示无权限。
   - PostgreSQL：rolname、rolsuper、rolcanlogin、rolcreatedb、rolcreaterole、rolconnlimit、rolvaliduntil；`pg_auth_members` 成员关系（最多 200）；当前数据库 CONNECT / CREATE / TEMPORARY 权限；不查 `rolpassword`。
   - 搜索 / 刷新；结果上限 500；没有新建 / 授权 / 回收 / 改密 / 删除按钮。
4. **安全**：不查询 / 展示密码散列；不写执行历史；不复制散列；只读连接与写连接行为一致。
5. **范围**：MySQL / PostgreSQL；SQLite / Redis 不显示用户页。

### 验收标准

- [ ] MySQL 用户页列出用户与 Host、插件、锁定 / 过期状态；无密码散列字段。
- [ ] 选中用户按需显示 `GRANT ...`；无权限时显示可读错误。
- [ ] PostgreSQL 列出角色属性与成员关系；显示当前库 CONNECT / CREATE / TEMPORARY 权限。
- [ ] 只读连接可查看；页面无授权 / 回收 / 改密按钮。
- [ ] MySQL 概览显示最大连接、运行线程、QPS、缓冲池命中率、库大小、Top 5 表，数值与手工 SQL 一致。
- [ ] PostgreSQL 概览显示活动 / 空闲会话、最大连接、QPS、缓存命中率、库大小、Top 5 表。
- [ ] 首次打开 QPS 显示采样中，第二次刷新显示数值；关闭自动刷新后停止采样。
- [ ] 无权限看 `mysql.user` 时显示说明，概览与进程页仍可用。
- [ ] 用户页搜索 / 刷新 / 500 条上限提示可用；不新增执行历史。
- [ ] SQLite / Redis 不显示用户页。

### 涉及文件与命令

**前端**

- 修改 `src/components/ServerActivityDialog.vue`：用户页签、概览卡片、Top 表、搜索、只读提示。
- 新增 `src/components/ServerUsersPanel.vue`：列表 + 详情 / 授权。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`、`src/styles.css`。

**后端**

- 修改 `src-tauri/src/commands/server.rs`：`server_overview` 扩展；新增 `server_users`、`server_user_grants`、`server_top_tables`。
- 修改 `src-tauri/src/models.rs`、`src-tauri/src/lib.rs`。
- MySQL / PostgreSQL 系统表查询；只读；不调用 `ensure_writable`（查看不需要写）。
- 不新增写命令。

### 风险与限制

- MySQL 5.7 / 8.0 / MariaDB 的 `mysql.user` 列不同；必须能力探测 / 回退。
- 普通账号可能看不到用户列表；部分结果要能解释。
- Top 表扫描可能慢；限制当前库 / 模式，提供刷新，不高频自动执行。
- QPS 基准需保存上次采样；刷新间隔变化时重置，避免负数。
- 绝不查询 / 日志记录密码散列。
- 不是权限编辑器，不要添加禁用按钮伪装功能。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/admin/DatabaseUserAdmin.vue`、`MySqlDashboard.vue`、`PostgresDashboard.vue`；`apps/desktop/src/lib/database/databaseUserAdmin.ts`、`mysqlServerStatus.ts`、`postgresServerStatus.ts`、`serverMetrics`。
- **Navicat**：用户 / 权限管理器与服务器监视；参考官方手册的 User & Privilege Manager / Server Monitor 章节。
- **mydb 现状**：`commands/server.rs`、`ServerActivityDialog.vue`（C-07）。

---

## D-06 应用内备份计划

- **优先级**：P2
- **预估**：1.5–2 个开发会话
- **依赖**：C-01 导出引擎、C-02 还原入口
- **关系**：接手第二批未实现的 C2-08；不做报表、任意查询任务、系统级调度。

### 用户问题 / 场景

C-01 可以手动导出，但「每天下班前备份一次」仍要人记得点。用户需要保存备份计划：固定连接、库 / 模式、表范围、输出目录、频率、保留份数；应用运行时到点执行，历史可查，失败可见。用户接受「应用关闭时不会执行」，但不能被界面误导成系统级任务。

### 期望行为

1. 入口「工具 → 备份计划…」；打开 `BackupSettingsDialog.vue`。
2. **计划列表**：名称、连接、范围、频率、下次运行、上次结果、启用开关；新建 / 编辑 / 删除 / 立即运行 / 历史 / 打开目录；空状态说明仅应用运行时执行。
3. **计划编辑**：
   - 名称、连接（SQLite / MySQL / PostgreSQL）、启用；
   - 范围：MySQL 数据库（多选 / 全部非系统库）、PostgreSQL 当前库 + 模式、SQLite 当前文件；表范围全部 / 包含 / 排除（`*`、`?`，支持 `schema.table`）；
   - 内容：结构、数据（至少一项）；可选 `DROP ... IF EXISTS`；第一版纯 `.sql`，不做 gzip / `.nb3`；
   - 输出：绝对目录；文件名模板 `mydb-backup-{connection}-{database}-{timestamp}.sql`，支持 `{connection}`、`{database}`、`{schema}`、`{date}`、`{time}`、`{timestamp}`；预览完整路径；
   - 频率：每小时（1–24）/ 每天指定时间 / 每周指定星期与时间；本地时区；
   - 保留最近 N 次成功（1–30），只删本计划生成、匹配模板的文件；
   - 保存前校验连接、目录、内容、模板、频率。
4. **调度与执行**：
   - 启动加载 `backup-jobs.json`；每 30 秒检查到期；同一时刻只运行一个任务；同计划不重叠；
   - 复用 C-01 导出逻辑（抽成共享函数，不调用 Tauri 命令）；临时文件 + rename；
   - 进度 / 取消；取消 / 失败删除临时文件；
   - 成功 / 失败写 `backup-history.json`（最多 200 条）：计划、连接、范围、起止、状态、路径、大小、错误；
   - 保留策略只删本计划历史中记录且位于输出目录内的文件；删除失败记录警告；
   - 错过计划：启动后提示「有 N 个计划已错过，是否立即补跑？」，不静默补跑；
   - 连接删除 / 失效：计划保留但标记不可用，暂停调度。
5. **限制**：固定横幅「仅在 mydb 打开时执行；关闭应用、休眠或退出不会运行；不会安装系统服务」；不做增量 / 物理备份 / PITR。
6. **恢复**：历史记录「恢复…」打开 C-02 并预填路径，不自动执行。
7. **安全**：计划文件不含密码 / SSH 口令 / 私钥口令；只存 connectionId；输出目录禁止 `..` 穿越；只读连接可备份。

### 验收标准

- [ ] 新建每日 02:00 计划，保存后显示下次运行；重启应用后仍在。
- [ ] 「立即运行」生成 `.sql`，历史显示成功、路径、大小、表 / 行数；文件可用 C-02 还原。
- [ ] 计划改到 1 分钟后，应用运行时自动执行；列表显示运行中，完成后更新下次运行。
- [ ] 计划时间应用关闭，启动后提示「已错过 N 次」，不自动执行；选择补跑 / 跳过行为正确。
- [ ] 保留 3 份：连续 4 次后最旧被删，仅剩 3 份；非本计划文件不删。
- [ ] 取消 / 失败后无残留临时文件；历史状态正确。
- [ ] 连接失效 / 密码未保存时失败并记录原始错误；计划保留、标记不可用。
- [ ] 只读连接可创建 / 运行计划；备份不写源库。
- [ ] 计划文件无 password / sshPassword / keyPassphrase；模板含 `../` 被拒绝或清理。
- [ ] 两个计划同时到期时串行执行，不并发写同一目录。
- [ ] 历史「恢复…」打开 C-02 并预填路径，不自动执行。
- [ ] 关闭应用后无后台进程；界面横幅明确限制。

### 涉及文件与命令

**前端**

- 新增 `src/components/BackupSettingsDialog.vue`、`BackupJobEditor.vue`。
- 修改 `src/App.vue`、`src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`：入口 / 进度监听。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`、`src/styles.css`。

**后端**

- 新增 `src-tauri/src/commands/backup.rs`：计划 CRUD、立即运行、取消、历史、错过查询。
- 新增 `src-tauri/src/backup_store.rs`、`src-tauri/src/backup_scheduler.rs`：原子读写、30 秒 tick、串行队列、事件。
- 修改 `src-tauri/src/lib.rs`：setup 启动 scheduler；注册命令。
- 修改 `src-tauri/src/commands/export.rs`：抽出 `export_database_to_path` 共享函数。
- 修改 `src-tauri/src/models.rs`：`BackupJob`、`BackupSchedule`、`BackupRun`、`BackupProgress`、`BackupSummary`。
- 复用 `chrono`；不需要新依赖。

### 风险与限制

- 不是系统调度器：退出 / 休眠 / 崩溃不执行；界面重复说明。
- 错过计划只补跑一次，避免启动瞬间打满磁盘。
- 保留只删自己记录且位于输出目录的文件；防符号链接 / 路径穿越。
- 密码未保存时运行失败；提示先保存密码。
- 逻辑备份慢 / 占磁盘；第一版不压缩；失败清理。
- 多计划串行；文件名含计划名 / 时间戳，避免覆盖。
- 时区 / 夏令时可能重复或跳过；记录实际执行时间。

### 参考

- **DBX（Apache-2.0）**：`crates/dbx-core/src/scheduled_backup/{models,engine,service,store}.rs`；`apps/desktop/src/components/backup/*`；`apps/desktop/src/lib/backup/*`；文档 `database-backup.cn.mdx`（频率、保留、表范围、应用必须运行）。
- **Navicat**：自动运行 / 计划任务（备份）；参考官方手册 Auto Run / Schedule。本项明确不承诺系统级调度。
- **mydb 现状**：C-01 导出、C-02 还原、`storage.rs`。

---

## D-07 查询构建器可行性（受限 SELECT 构建）

- **优先级**：P2
- **预估**：2 个开发会话
- **依赖**：现有 `listTables` / `listScopeColumns`、`sql-literal.ts`
- **关系**：新增。`BACKLOG.md` 把查询构建器列为长期非目标；本项只做受限 MVP，验证交互与 SQL 生成质量，不承诺完整 IDE 能力。

### 用户问题 / 场景

不熟悉 SQL 的用户知道要查哪几张表，但 JOIN 条件、别名、GROUP BY 容易写错；熟悉 SQL 的用户也不想为一次性查询手敲所有列。Navicat 与 DBeaver PRO 都提供可视化查询构建器。mydb 需要一个「选表 → 选列 → 连关系 → 加条件 → 排序 / 限制 → 生成 SQL」的轻量入口，生成后打开到查询标签，由用户检查再执行。

### 期望行为

1. **入口**：「工具 → 查询构建器…」；使用当前连接与当前库 / 模式；只读连接可用（不执行写操作）；Redis 不显示。
2. **表选择**：
   - 左侧列出当前库 / 模式的表 / 视图，支持搜索；
   - 添加 1–5 张表；同一表可加一次（第一版不做自连接）；
   - 每张表显示别名（默认表名首字母，可改，需唯一）。
3. **列选择**：
   - 树形勾选列；默认不选；支持全选 / 清空；
   - 输出 `alias.column AS column`；别名冲突时提示；
   - 表达式 / 聚合第一版不支持（无 COUNT / SUM / GROUP BY）。
4. **关系 / JOIN**：
   - 选中第二张表时，若与已有表存在外键，自动建议 JOIN（本地 / 引用列）；
   - 用户可手动添加 JOIN：左表列、右表列、类型 INNER / LEFT；同一对表可多条件（AND）；
   - 未连接的表在预览中提示「笛卡尔积风险」，允许但显示警告；
   - 第一版不支持 RIGHT / FULL / 子查询 / CTE / UNION。
5. **筛选**：
   - 多条件 AND；字段、操作符（`=`、`<>`、`>`、`>=`、`<`、`<=`、`LIKE`、`NOT LIKE`、`IS NULL`、`IS NOT NULL`）、值；
   - 标识符按方言引用，值按 `sqlLiteral` 转义；生成 SQL 可查看；
   - 不提供 OR / 括号 / 子查询（走 SQL 编辑器）。
6. **排序与限制**：
   - 多列排序（ASC / DESC，最多 3 列）；LIMIT 1–10,000，默认 100；
   - 没有排序列时提示「顺序未定义」。
7. **预览与输出**：
   - 实时 SQL 预览（只读、高亮）；复制；
   - 「在查询标签打开」：新建查询标签并填入 SQL，**不自动执行**；
   - 可选「另存为片段」（接入 D-01）——若 D-01 未完成则不显示；
   - 构建器状态不持久化，关闭即清空（第一版）。
8. **错误与空状态**：未选表 / 未选列时 SQL 预览为空并提示；元数据加载失败显示错误；引用的表 / 列被删除时提示重建。
9. **安全**：只读元数据；不执行生成 SQL；不写历史；引用 / 转义统一走现有工具。

### 验收标准

- [ ] 选择 `users` 表并勾选 `id`、`name`，预览生成 `SELECT "users"."id" AS "id", ... FROM "users" LIMIT 100`（按方言引用）。
- [ ] 选择 `users` 与 `orders`，存在外键时自动建议 `users.id = orders.user_id`；接受后生成 INNER JOIN。
- [ ] 手动改为 LEFT JOIN 后 SQL 使用 `LEFT JOIN`；多条件 JOIN 用 AND 连接。
- [ ] 未连接的两张表生成 CROSS JOIN 并显示「笛卡尔积风险」警告。
- [ ] 筛选 `status = 'active' AND created_at >= '2026-01-01'` 生成正确 SQL；值含单引号时转义正确。
- [ ] `IS NULL` / `IS NOT NULL` 不生成值占位；`LIKE` 保留用户输入的 `%`。
- [ ] 多列排序 `status DESC, created_at ASC` 顺序正确；LIMIT 生效。
- [ ] 点击「在查询标签打开」创建新查询标签并填入 SQL，未自动执行；编辑器可继续修改。
- [ ] 只读连接可使用构建器；不产生数据库写入 / 执行历史。
- [ ] 未选表 / 未选列时预览为空并提示；元数据失败显示可读错误。
- [ ] 表 / 列名含引号、空格、中文时 SQL 可执行；别名冲突被阻止。
- [ ] 关闭后重新打开构建器状态为空（第一版不持久化）。

### 涉及文件与命令

**前端**

- 新增 `src/components/QueryBuilderDialog.vue`：表选择、列选择、JOIN、筛选、排序、预览。
- 新增 `src/components/QueryBuilderJoinEditor.vue`（或内嵌）：JOIN 条件。
- 新增 `src/lib/query-builder.ts`：状态模型、SQL 生成、引用 / 转义、JOIN 建议、校验。
- 修改 `src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`、`src/App.vue`：工具菜单入口。
- 修改 `src/lib/api.ts` / 现有元数据调用：使用 `listTables` / `listScopeColumns`；不新增后端。
- 修改 `src/styles.css`：构建器布局。

**后端**

- 无新增命令；只读取现有元数据。

### 风险与限制

- **范围蔓延**：聚合 / 子查询 / CTE / UNION / 画布会迅速变成 IDE；第一版明确禁止，评审通过后再扩展。
- **JOIN 语义**：自动建议只基于外键；外键缺失时用户需手动连接，不能猜测。
- **SQL 注入**：所有标识符 / 值必须引用 / 转义；预览与打开标签使用同一生成函数。
- **方言差异**：引用符、LIMIT 语法不同；复用 `sql-literal.ts`。
- **元数据权限**：看不到的表 / 列不显示；刷新后失效的表要提示。
- **不执行**：生成 SQL 后必须由用户运行；不能在构建器内自动执行，避免误查大表。
- **未保存状态**：第一版关闭即丢弃；用户需先打开到查询标签。

### 参考

- **Navicat**：查询构建器（Query Builder）的选表、关系、条件、排序、生成 SQL；参考官方手册的 Query Builder 章节。
- **DBeaver**：Visual Query Builder（PRO / CloudBeaver）；官方文档 `https://dbeaver.com/docs/dbeaver/Visual-Query-Builder/`；只参考交互，不复制实现。
- **DBX**：当前未提供同等可视化构建器；本项不复用其代码，仅参考其 SQL 片段 / 编辑器交互。

---

## D-08 数据库文档与 DBML 导出

- **优先级**：P2
- **预估**：1.5 个开发会话
- **依赖**：现有元数据、ER 外键关系、`sql-literal` / 引用工具
- **关系**：新增；不是完整模型设计器，也不做 PDF 报表引擎。ER 图（C-08）已完成，本项补「可交付文档」。

### 用户问题 / 场景

交接或评审时，ER 图只能看，不能写说明；表 / 字段注释散落在数据库里，业务方需要一个可搜索、可打印的文档；团队还需要 DBML 导入到建模工具。用户希望从当前库 / 模式生成一份带表清单、字段、主外键、索引与本地笔记的文档，并导出 DBML / Markdown / HTML。

### 期望行为

1. **入口**：「工具 → 数据库文档…」；使用当前连接与当前库 / 模式；只读连接可用；Redis 不显示。
2. **文档视图**：
   - 左侧目录：概览、表分组、表列表（搜索）；表按名称 / 分组排序；
   - 中间表详情：字段名、类型、可空、默认值、主键、外键、索引、数据库注释；
   - 右侧 / 底部：本地笔记编辑（表级、字段级），自动保存（防抖），显示「已保存 / 保存失败」；
   - 表分组：新建 / 重命名 / 删除分组，表可移动到分组；分组颜色可选；
   - 数据库注释只读展示；本地笔记独立存储，不写回数据库。
3. **笔记存储**：
   - `database-docs.json`（应用配置目录）或 localStorage，带版本号；
   - 键：连接 ID + 数据库 + 模式 + 表名 +（可选）字段名；
   - 表 / 字段重命名或删除后，孤立笔记保留但标记「对象不存在」；不自动删除；
   - 不记录密码 / 连接串；不云同步。
4. **导出**：
   - **DBML**：表、字段、类型、主键、索引、外键关系；不包含触发器 / 过程 / 函数 / 序列 / 权限；不支持的构造在导出报告中警告；单向导出，不导入 DBML；
   - **Markdown**：标题、生成时间、连接 / 库 / 模式、目录、每张表的字段表、主外键 / 索引、数据库注释 + 本地笔记；
   - **HTML**：自包含单文件（内联 CSS，无外部字体 / JS），可打印；同样内容；
   - 导出使用原生保存对话框；临时文件 + rename；取消 / 失败清理；文件上限 20 MB。
5. **部分失败**：
   - 单张表元数据读取失败时，其他表继续；文档中标记「读取失败」并在导出报告列出；
   - 无外键元数据时提示「无法生成关系」；不伪造关系；
   - 无可导出对象时提示，不生成空文件。
6. **预览与复制**：导出前可预览前 N 行 / DBML 片段；支持复制全文。
7. **安全**：只读元数据；不执行 DDL / DML；不写数据库；笔记不写日志。

### 验收标准

- [ ] 打开 SQLite / MySQL / PostgreSQL 的数据库文档，表清单、字段、主键、外键、索引与数据库注释正确。
- [ ] 为一张表与一个字段填写本地笔记，切换表再回来笔记仍在；重启应用后仍在。
- [ ] 表被删除后，其笔记在「孤立笔记」中可见，不被静默删除。
- [ ] 导出 DBML，包含所有表、字段、主键、索引与 Ref；导入 DBML 查看器（或文本检查）结构正确；不含例程 / 权限。
- [ ] 导出 Markdown，包含目录、表字段表、关系、数据库注释与本地笔记；中文正常。
- [ ] 导出 HTML，浏览器打开自包含、可打印；无外部依赖；中文 / 特殊字符正确。
- [ ] 某张表元数据读取失败时，其他表仍导出，报告列出失败表；不生成半截文件。
- [ ] 无外键时文档显示「无关系元数据」，不画假关系。
- [ ] 只读连接可使用文档与导出；数据库无写入，历史不新增。
- [ ] 超过 1,000 张表时提示范围过大 / 分批；导出超过 20 MB 时拒绝并提示。
- [ ] 取消 / 失败后无残留临时文件；笔记保存失败时提示，不丢当前编辑内容。

### 涉及文件与命令

**前端**

- 新增 `src/components/DatabaseDocsDialog.vue`：目录、表详情、笔记、分组、导出。
- 新增 `src/components/DocsNoteEditor.vue`（可选）：表 / 字段笔记。
- 新增 `src/lib/database-docs.ts`：文档模型、分组、笔记存取、孤立笔记。
- 新增 `src/lib/dbml.ts`：DBML 生成与转义。
- 新增 `src/lib/database-docs-export.ts`：Markdown / HTML 生成、文件名、预览。
- 修改 `src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`、`src/App.vue`：工具菜单入口。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：文档所需元数据（尽量复用现有 `getTableStructure` / `listTables` / `getErDiagram`）。
- 修改 `src/styles.css`：文档布局、打印样式。

**后端**

- 优先复用现有元数据命令；若逐表 `getTableStructure` 太慢，新增 `database_docs_snapshot(connection_id)` 一次性返回表 / 字段 / 索引 / 外键 / 注释（只读）。
- 笔记存储：新增 `commands/docs.rs` 读写 `database-docs.json`（原子写、版本、损坏回退），或前端 localStorage（二选一，推荐应用配置目录 + 后端，便于备份）。
- 修改 `src-tauri/src/lib.rs`：注册命令。
- 不新增数据库写入命令。

### 风险与限制

- **笔记键**：表 / 字段重命名会产生孤立笔记；第一版保留并标记，不自动迁移。
- **DBML 覆盖**：只支持表 / 字段 / 主键 / 索引 / 外键；枚举、例程、触发器、权限不导出，必须警告。
- **HTML 体积**：大库可能超过 20 MB；分批或只导出选中表。
- **注释来源**：数据库注释与本地笔记分开展示，避免覆盖；导出时明确标注。
- **性能**：逐表读取结构在数百表时慢；需要快照命令或并发限制 + 进度。
- **不是模型设计器**：不提供画布、布局、正向 / 反向工程；ER 图仍由 C-08 负责。
- **PDF**：不做 PDF；HTML 可通过浏览器打印，界面说明。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/docs/DatabaseDocsDialog.vue`、`DataDictionaryDialog.vue`；`apps/desktop/src/lib/docs/*`；`crates/dbx-core/src/data/docs/{collector,dbml,export,snapshot,relations}.rs`；文档 `docs/content/docs/database-docs.cn.mdx`（笔记、分组、DBML 单向导出、边界）。
- **Navicat**：数据字典（Data Dictionary）与模型；官方产品页 `https://www.navicat.com/en/products/data-dictionary?product=navicat-premium`。本项不做 PDF 报表，只做 DBML / Markdown / HTML。
- **DBeaver**：ER 图与数据字典导出；只参考功能。
- **DBML**：`https://dbml.dbdiagram.io/docs/`。

---

## 附录 A：需求与 Backlog / 第二批的收口建议

| 既有项 | 建议状态 | 说明 |
|---|---|---|
| 第二批 C2-03（对象定义） | 由 D-02 接手 | 若实现侧已开始，以 C2-03 为准，D-02 关闭 |
| 第二批 C2-05（表数据高级） | 由 D-03 接手 | 值查看器 / 虚拟滚动 / XLSX；若已开始则 D-03 关闭 |
| 第二批 C2-07（用户 / 权限） | 由 D-05 接手 | 只读用户 + 概览；不做授权 |
| 第二批 C2-08（备份） | 由 D-06 接手 | 应用内备份计划；不做系统调度 |
| B-10 对象定义 | D-02 覆盖 | 查看 / 编辑 / 重载函数候选 |
| B-16 大字段 / JSON | D-03 覆盖 | 值查看器 + 虚拟滚动 + XLSX |
| B-37 用户 / 权限 | D-05 部分覆盖 | 只读用户 + 概览；授权待后续 |
| B-34 / B-36 备份 / 计划 | D-06 部分覆盖 | 只做备份计划，不做报表 / 任意任务 |
| B-09 导入向导 | 保留待办 | 本批不做 XLSX 导入 |
| B-05 查询取消 / B-13 库模式创建 | 保留第二批待办 | 不在本批重复 |
| 查询构建器 / 数据生成 | D-07 / D-04 受限重开 | 只做 MVP，不承诺完整 IDE 能力 |
| 数据库文档 / DBML | D-08 新增 | 不是完整模型设计器 |

## 附录 B：实现与验收约定

1. **一次只做一个需求**：D-01 完成并验收后再开始 D-02；不要把片段库与对象定义混在一个未验收的提交里。
2. **独立可回滚**：新增组件 / 命令尽量内聚；不要提前抽象未验证接口。
3. **测试**：纯函数（片段校验、生成器、SQL 构建、DBML / Markdown）必须有单元测试或可重复手工用例；真实数据库回归使用 README 环境变量；前端 `npm run check`，Rust `cargo test`。
4. **错误文案**：包含对象 / 表 / 列 / 行号与数据库原始错误；不要只显示 `error`。
5. **隐私**：不记录密码、片段正文、笔记全文、生成数据到日志；用户列表不查密码散列；导出文件提示敏感信息。
6. **版本与发布**：本批不提升版本号；发布等待用户明确说「更新app」或「合并代码」。
7. **参考与许可**：复制 DBX 代码需在文件头注明 `Source: https://github.com/t8y2/dbx (Apache-2.0)` 与原路径；不复制 Navicat / DBeaver 任何资源。

## 附录 C：本批参考资料

- DBX 仓库（Apache-2.0）：`https://github.com/t8y2/dbx`
  - 片段：`apps/desktop/src/components/editor/SnippetQuickAddDialog.vue`；`apps/desktop/src/lib/sql/sqlSnippetTemplates.ts`、`snippetRangeSelection.ts`；文档 `docs/content/docs/sql-snippets.cn.mdx`
  - 对象源码：`apps/desktop/src/components/objects/ObjectSourceDialog.vue`；`apps/desktop/src/lib/table/objectSource*.ts`
  - 表数据：`apps/desktop/src/components/grid/DataGridCellDetailDialog.vue`；`common/VirtualScrollArea.vue`；`lib/export/xlsxExport.ts`；`src-tauri/src/commands/xlsx_export.rs`
  - 数据生成：`apps/desktop/src/components/generate/*`；`apps/desktop/src/components/generate/params/*`
  - 用户 / 服务器：`apps/desktop/src/components/admin/*`；`apps/desktop/src/lib/database/{databaseUserAdmin,mysqlServerStatus,postgresServerStatus}.ts`
  - 备份：`crates/dbx-core/src/scheduled_backup/*`；`apps/desktop/src/components/backup/*`；`apps/desktop/src/lib/backup/*`；文档 `database-backup.cn.mdx`
  - 数据库文档：`apps/desktop/src/components/docs/*`；`apps/desktop/src/lib/docs/*`；`crates/dbx-core/src/data/docs/*`；文档 `database-docs.cn.mdx`
- Navicat 17 官方手册（功能参考，不使用其代码 / 资源）：`https://www.navicat.com/manual/pdf_manual/en/navicat_17/mac_manual/navicat_mac_en.pdf`
  - 数据生成：`https://www.navicat.com/en/products/data-generation?product=navicat-premium`
  - 数据字典：`https://www.navicat.com/en/products/data-dictionary?product=navicat-premium`
- DBeaver Visual Query Builder：`https://dbeaver.com/docs/dbeaver/Visual-Query-Builder/`（PRO / CloudBeaver；只参考交互）
- DBML 文档：`https://dbml.dbdiagram.io/docs/`
- 第三方库：`rust_xlsxwriter`（MIT OR Apache-2.0，`https://github.com/jmcnamara/rust_xlsxwriter`）
- mydb 现状：`src/components/SqlEditor.vue`、`src/components/QueryWorkspace.vue`、`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`、`src/components/ServerActivityDialog.vue`、`src-tauri/src/commands/{query,server,export}.rs`

---

> 本文件只定义需求与验收，不包含实现代码。实现完成后先更新 `CHANGELOG.md` 的 `[Unreleased]`，再等待用户指令决定是否提交 / 打包 / 更新版本。