# mydb 产品需求（2026-11-25 第二批）

> 维护人：产品（PM）
> 基线版本：v0.11.0（2026-11-24）
> 第一批状态：C-01～C-08 已落地，详见 `docs/PM-REQUIREMENTS-2026-11-25.md` 的「0.6 实现状态」
> 工作区现状：基础格式化（SqlEditor.formatSql）、对象创建（CreateObjectDialog）、连接导入导出（ConnectionTransferDialog）已有实现；并行实现仍在收尾，以 `npm run check` 的实际结果为准。本文件只定义剩余缺口，不重复已有代码。
> 文档状态：待评审 / 待排期
> 适用范围：SQLite / MySQL（含 MariaDB）/ PostgreSQL；Redis 不在本批新增范围内
> 关联文档：`docs/PM-REQUIREMENTS-2026-11-25.md`、`docs/BACKLOG.md`、`docs/ARCHITECTURE.md`、`CHANGELOG.md`

---

## 0. 本批结论

### 0.1 一句话目标

在「能搬数据、能改表」之后，补齐日常查询与对象维护的效率工具：**格式化收口、库 / 模式创建、对象定义、跨库找对象、看清大字段、取消慢查询、用户只读查看、备份计划**。基础格式化、EXPLAIN、对象创建、连接导入导出已在工作区落地，本批只验收或补缺口（见 0.7）；本批也不重复第一批的导出 / 还原 / 批量编辑 / 表设计器 / 传输 / 结构同步 / 进程面板 / ER。

### 0.2 需求清单（按实施优先级排序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 依赖 | 与既有需求的关系 |
|---|---|---|---|---|---|---|
| 1 | C2-01 | SQL 格式化收口（压缩 / 范围 / 设置 / 安全） | P0 | 1 个开发会话 | 现有 `SqlEditor.formatSql` | 已有基础格式化；只补压缩、当前语句、设置与安全边界 |
| 2 | C2-02 | 数据库 / 模式创建与删除 | P1 | 1 个开发会话 | 现有对象树数据库 / 模式节点 | 替代 B-13；EXPLAIN 已由并行实现落地，见 0.7 |
| 3 | C2-03 | 对象定义查看与编辑 | P1 | 1.5 个开发会话 | `get_object_ddl`、C-02 单语句执行 | 替代 B-10；对象创建已由 CreateObjectDialog 实现，不重复 |
| 4 | C2-04 | 全局数据库搜索（对象 / 字段） | P1 | 1–1.5 个开发会话 | 现有元数据、C2-03（可选） | 新增；补 B-25 只搜当前库的缺口 |
| 5 | C2-05 | 表数据高级能力：值查看器 / 虚拟滚动 / XLSX | P1 | 1.5–2 个开发会话 | 现有结果网格与 C-03 表数据页 | 细化并替代 B-16；新增虚拟滚动与 XLSX |
| 6 | C2-06 | 查询取消与超时 | P1 | 1.5 个开发会话 | 现有 `execute_query` / 事务 / `DatabaseSession` | 替代 B-05；连接导入导出已在工作区实现，不重复 |
| 7 | C2-07 | 用户 / 权限查看与服务器概览增强 | P2 | 1–1.5 个开发会话 | C-07 服务器活动面板 | 承接 B-37 的用户部分，并扩展 C-07 指标 |
| 8 | C2-08 | 应用内备份计划 | P2 | 1.5–2 个开发会话 | C-01 导出引擎、C-02 还原 | 承接 B-34 / B-36 的「定时备份」子集 |

> 「1 个开发会话」指 Claude 在一次连续实现中完成编码、`npm run check`、`cargo test` 与本文件列出的可自动化验收项；真实数据库手工验收可单独安排。

### 0.3 为什么 C2-01 排第一

- 它**只改前端**：不需要新命令、不碰数据库连接、不做数据迁移，失败影响面最小。
- 它**每天都会用到**：从日志、ORM、慢查询里复制出来的一行 SQL 需要立刻可读；压缩后又方便回贴。
- 它**可在本会话完成**：工作区已有 `SqlEditor.formatSql`（`sql-formatter`、选区 / 整篇、固定 upper/2 空格）；本项只补「当前语句优先、压缩、设置、DELIMITER / 过程体防护、大小上限」，不是从零重写。
- 它**不阻塞其他需求**：后续对象定义编辑、全局搜索都可以复用同一个编辑器命令入口；EXPLAIN 已由并行实现落地，不再排期。

### 0.4 与第一批已完成项的去重

| 第一批已完成能力 | 本批不重复的内容 | 本批新增的差异 |
|---|---|---|
| C-01 数据库级 SQL 导出 | 不再提「导出结构 / 数据 / 进度 / 取消」 | C2-08 只把导出引擎接到定时任务上 |
| C-02 SQL 脚本执行与还原 | 不再提分句、事务、进度、只读预检 | C2-03 复用单语句执行，不重写脚本执行器 |
| C-03 表数据待提交 / 批量编辑 / 筛选排序分页 | 不再提变更集、批量设置、筛选构建器、分页 | C2-05 只加值查看器、虚拟滚动、XLSX 导出 |
| C-04 表设计器索引 / 外键 / 注释 | 不再提建表字段 / 索引 / 外键 | C2-03 只做对象定义；对象创建已由 `CreateObjectDialog` 实现 |
| C-05 数据传输 | 不再提多表传输 / 建表 / 追加 / Upsert | **逐列映射与跨驱动仍为缺口，本批不排期** |
| C-06 结构同步应用到目标 | 不再提重新对比 / 危险确认 / 事务策略 | **目标结构快照仍为缺口，本批不排期** |
| C-07 服务器概览 / 进程 / 终止会话 | 不再提进程列表与终止会话 | C2-07 加用户 / 角色只读页与容量指标 |
| C-08 ER 图导出 / 布局 / 定位 | 不再提 ER 导出与布局 | 无 |
| 已有基础格式化（`SqlEditor.formatSql`） | 不重复选区 / 整篇格式化 | C2-01 补压缩、当前语句、设置、DELIMITER / 过程体防护 |
| 对象创建（`CreateObjectDialog`） | 不重复视图 / 存储过程 / 函数 / 触发器创建 | C2-03 只补定义查看与编辑 |
| 连接导入导出（`ConnectionTransferDialog`） | 不重复 mydb JSON / DBeaver JSON / Navicat `.ncx` 导入与 mydb JSON 导出 | 旧版 DBeaver XML、覆盖策略、SSH 细节作为后续，不在本批 |

> EXPLAIN 的执行计划视图已由并行实现落地（`commands/explain.rs`、`explain-plan.ts`、`ExplainPlanViewer.vue`），本批不再立项；其验收按 B-20 进行。

### 0.5 本批明确不做

| 能力 | 结论 | 理由 |
|---|---|---|
| 字段级血缘（完整 lineage） | 本批不做，保留研究 | 需要解析视图 / 函数 / 触发器 / 动态 SQL，跨方言差异大、误报代价高；轻量客户端没有查询日志与数据目录支撑。C2-04 的「按字段名搜索并定位所属表」只解决「这个字段在哪」，不推断「这个字段从哪来」。后续如要做，先做视图依赖图，不直接承诺列级血缘。 |
| 跨连接全局搜索 / 数据内容全文搜索 | 不做 | 跨连接需要同时打开多个连接并聚合结果，性能和权限边界复杂；本批只在**当前连接**内跨库 / 模式搜索对象名与字段名。 |
| `EXPLAIN ANALYZE` / 计划图（画布）/ 计划导出 | 不做 | 工作区已有 EXPLAIN 计划树；`ANALYZE` 会真实执行语句，风险高；画布与导出需要独立布局引擎，另立需求。 |
| 用户授权 / 回收 / 改密 | 不做 | MySQL 8 与 PostgreSQL 权限模型差异大，误操作风险高；本批只读查看。 |
| 系统级定时任务 / 守护进程 | 不做 | 与「轻量、无后台服务」定位冲突；C2-08 只做应用内计划，并明确写「应用关闭时不执行」。 |
| 连接配置反向导出（DBeaver / Navicat 格式） | 不做 | 工作区已支持导入 DBeaver / Navicat 与导出 mydb JSON；`.ncx` 没有稳定公开 schema，驱动 ID 与密码加密方式随版本变化，产出无法保证对方可导入，后续验证通过再单独立项。 |
| 解密 DBeaver / Navicat 保存的密码 | 不做 | 私有加密，解密涉及凭据处理；现有导入已按「重新输入密码」处理，不读取第三方凭据。 |
| XLSX 导入 | 不做 | 本批只做导出；导入继续走 CSV / JSON 与 B-09 导入向导。 |
| MySQL 存储过程 / 函数 / 触发器的原地编辑 | 不做 | `DELIMITER` 与依赖关系容易出错；本批只提供查看、复制、保存为 `.sql`。 |
| 查询构建器 / 数据生成 / AI | 不做 | 仍属长期非目标，见 `BACKLOG.md`。 |

### 0.6 通用约束

- **只写需求与验收，不写实现代码**；实现按单个需求独立完成、独立验收。
- **不提升版本号**：所有改动先写 `CHANGELOG.md` 的 `[Unreleased]`；只有用户明确说「更新版本」才提升。
- **只读连接**：新增的写操作（保存对象定义、备份文件写入目标库之外）必须在后端命令层拒绝；只读查询、查看定义、搜索、导出、执行计划、取消查询允许。
- **不复制品牌资源**：Navicat / DBeaver 只参考功能与公开格式说明，不复制图标、截图、专有代码。DBX 为 Apache-2.0；复制代码片段时必须在文件头注明来源与许可证，并更新 `README.md` 的「参考与许可」。
- **新增依赖需说明**：C2-01 复用已引入的 `sql-formatter`（MIT）；C2-05 允许引入 `rust_xlsxwriter`（MIT OR Apache-2.0）。两者都必须在 `README.md` / 许可证说明中登记；不得引入 UI 组件库或图标库。
- **性能上限**：格式化 100 KB 文本；搜索结果 200 条；计划树 2000 节点；XLSX 单表 20 万行；值查看器 2 MB 文本。超过上限必须提示，不得静默截断或卡死。
- **私密数据**：用户列表不得查询 / 展示 `authentication_string`、`rolpassword` 等密码散列；连接导入不得写入密码、SSH 口令、私钥口令；日志不得记录完整 SQL 或凭据。
- **验收环境**：SQLite 用 `examples/demo.db`；MySQL / PostgreSQL 用真实实例，环境变量沿用 README 的 `MYDB_TEST_MYSQL_*` / `MYDB_TEST_PG_*`。无真实实例时至少完成 SQLite + 单元测试，并在交付说明标注未覆盖项。
- **完成定义（DoD）**：`npm run check` + `cargo test` 通过；验收清单逐项勾选；`CHANGELOG.md` 写入 `[Unreleased]`；相关 README / 已知限制更新；BACKLOG 对应项标记「已被 C2-xx 替代」。

---

### 0.7 并行实现状态（已落地，不再立项）

> 截至本文写作时，工作区已有以下实现（并行会话仍在收尾，验收前先跑 `npm run check`）。它们不计入本批 8 个新需求；若行为有缺口，按对应 Backlog 项验收或另立小修，不在本文件中重复描述。

| 已落地项 | 代码落点 | 状态与处理 |
|---|---|---|
| 基础 SQL 格式化 | `src/components/SqlEditor.vue::formatSql`、`sql-formatter` | 选区 / 整篇、固定 upper/2 空格可用；缺压缩、当前语句、设置、DELIMITER 防护 → C2-01 只补缺口 |
| EXPLAIN 执行计划 | `src-tauri/src/commands/explain.rs`、`src/lib/explain-plan.ts`、`src/components/ExplainPlanViewer.vue` | 已有后端命令、MySQL / PostgreSQL / SQLite 解析与计划视图；B-20 由 PM 验收，不再单列需求 |
| 对象创建 | `src/components/CreateObjectDialog.vue`、`ObjectBrowser.vue` | 视图 / 存储过程 / 函数 / 触发器创建入口已接入；B-33 验收，不再单列需求 |
| 连接导入 / 导出 | `src/components/ConnectionTransferDialog.vue` | mydb JSON、DBeaver JSON、Navicat `.ncx` 导入 + mydb JSON 导出已实现；B-14 验收，不再单列需求 |
| 第一批 C-01～C-08 | 见 `docs/PM-REQUIREMENTS-2026-11-25.md` 0.6 | 导出 / 还原 / 批量编辑 / 表设计器 / 传输 / 结构同步 / 服务器进程 / ER |

## C2-01 SQL 格式化收口：压缩 / 范围 / 设置 / 安全

- **优先级**：P0
- **预估**：1 个开发会话
- **依赖**：现有 `SqlEditor.formatSql`、已引入的 `sql-formatter` 15.x（MIT）
- **替代关系**：不是从零新增。工作区已有「选区或整篇格式化」的基础实现；本项只补齐剩余缺口，并保持现有入口与行为不回退。

### 现状（不要重复实现）

- `SqlEditor.formatSql()` 已使用 `sql-formatter`，按驱动选择 `mysql` / `postgresql` / `sqlite`，固定 `keywordCase: upper`、`tabWidth: 2`、`linesBetweenQueries: 1`；
- 已有选区时替换选区，无选区时替换整个文档；
- 编辑器「格式化」按钮、编辑菜单 `editor-format-sql`、命令面板均已接入；
- 失败时静默保留原文；没有压缩、设置、当前语句识别、大小上限、`DELIMITER` / 过程体防护。

### 用户问题 / 场景

基础格式化解决了一行 SQL 的可读性，但还有四个日常痛点：1）没有选区时总是格式化整篇，多语句脚本会被整体重排，用户只想整理光标所在的一条；2）从格式化稿回贴工单时需要压缩成一行，现在只能手工删空格；3）关键字大小写、缩进只能接受固定值；4）对 `DELIMITER`、`CREATE PROCEDURE ... BEGIN ... END` 直接格式化可能破坏脚本。用户需要在这些边界上安全地收口，而不是再做一个新的格式化按钮。

### 期望行为

1. **入口保持并补齐**：
   - 保留编辑器「格式化」按钮、编辑菜单、命令面板与 `Shift+Mod+F`；
   - 编辑器「格式」按钮改为下拉：格式化、压缩、格式化设置…；
   - 编辑菜单增加「压缩 SQL」；命令面板增加「压缩当前 SQL」；
   - 压缩不设默认快捷键，避免误触。
2. **作用范围（修正现有行为）**：
   - 有选区：只替换选区；
   - 无选区且光标**确实位于**某条语句内：只替换该语句（使用 `splitSqlStatements` 的区间判断，不能用 `statementAtCursor` 的兜底结果）；
   - 无选区且光标不在任何语句内、或文档为空：格式化整个文档；
   - 只作用于当前活动查询标签。
3. **格式化（回归 + 设置）**：
   - 方言跟随驱动；保留字符串、引号标识符、注释原文；
   - 关键字大小写 `upper` / `lower` / `preserve`，默认 `upper`；
   - 缩进 2 / 4 空格，默认 2；语句间空行 0 / 1 / 2，默认 1；
   - 是否保留注释（格式化时），默认保留；压缩时普通注释始终移除；
   - 设置写入 localStorage `mydb.editor.format.v1`，重启恢复；提供「恢复默认」。
4. **压缩规则**：
   - 词法扫描，不能用正则全局替换；跳过单引号、双引号、反引号、方括号标识符、`$$ ... $$` / `$tag$ ... $tag$`；
   - 移除 `--`、`#`、`/* ... */` 注释，但保留 MySQL 可执行注释 `/*! ... */` 与优化器提示 `/*+ ... */`；
   - 折叠字面量与标识符之外的连续空白为单个空格；每条语句一行，保留结尾分号；
   - 不改变字符串、标识符内部的内容与大小写。
5. **安全边界**：
   - 文本含 `DELIMITER` 指令：拒绝格式化 / 压缩，提示「包含 DELIMITER，请先拆分或只处理过程体」；
   - 目标语句以 `CREATE PROCEDURE` / `CREATE FUNCTION` / `CREATE TRIGGER` 开头：拒绝，提示「过程 / 函数 / 触发器定义暂不自动格式化」；
   - 文本或选区超过 100 KB：拒绝并提示大小；
   - 格式化库抛错：保留原文，状态栏 / 提示显示「格式化失败，已保留原文」；不能部分替换；
   - 只改文本，不执行 SQL、不新增执行历史、不连接数据库。
6. **撤销与光标**：
   - 一次格式化 / 压缩必须是**一个** CodeMirror 事务，`Mod+Z` 一次恢复原文；
   - 选区格式化后选中新文本；无选区时把光标放在替换范围起点附近；
   - 修改触发标签未保存标记，符合现有 SQL 编辑逻辑。
7. **兼容**：
   - 现有「无选区格式化整篇」的用户仍可通过「光标不在任何语句内」或设置项回到整篇行为；
   - 不改变 SQL 文件打开 / 保存、执行、事务、只读连接的现有逻辑。

### 验收标准

- [ ] 回归：选中一段 SQL 点「格式化」，只有选区变化；无选区且光标在第二条语句内时，只有第二条变化，第一条 / 第三条不动。
- [ ] 无选区且光标在语句外（例如文档末尾空行）时，格式化整个文档；多语句之间按设置保留 0 / 1 / 2 个空行。
- [ ] 关键字大小写设为 `lower` 后 `SELECT` 变为 `select`，字符串与注释中的 `SELECT` 不变；重启后设置保持。
- [ ] 缩进改为 4 后重新格式化，缩进变为 4 空格；「恢复默认」回到 upper / 2 空格 / 1 空行。
- [ ] 压缩多行 SQL：每条语句一行、普通注释消失、字符串内空格与换行保持不变；MySQL `/*! STRAIGHT_JOIN */` 与 `/*+ INDEX(...) */` 保留。
- [ ] 含 `DELIMITER $$` 的脚本点格式化 / 压缩被拒绝，文本不变，提示明确。
- [ ] `CREATE PROCEDURE ... BEGIN ... END` 定义点格式化被拒绝，不产生半格式化文本。
- [ ] 一次 `Mod+Z` 精确恢复原文，再 `Mod+Shift+Z` 回到格式化 / 压缩结果；不会出现多次撤销。
- [ ] 100 KB 文本可完成格式化 / 压缩，操作期间显示忙碌状态；超过 100 KB 时拒绝且文本不变。
- [ ] 格式化库异常（模拟导入失败 / 非法方言）时保留原文并提示，不白屏、不静默。
- [ ] 编辑按钮下拉、编辑菜单、命令面板、快捷键四个入口行为一致；不新增 SQL 执行历史，数据库无请求。
- [ ] 只读连接下格式化 / 压缩可用。

### 涉及文件与命令

**前端**

- 修改 `src/components/SqlEditor.vue`：保留 `formatSql`，增加 `compressSql`；修正无选区时的「当前语句」判断；单事务替换；大小 / 关键字 / 空行设置读取；错误回调给上层。
- 修改 `src/components/QueryWorkspace.vue`：编辑器「格式」下拉（格式化 / 压缩 / 设置）；忙碌状态与错误提示。
- 新增 `src/components/SqlFormatSettingsDialog.vue`：大小写、缩进、语句间空行、保留注释、恢复默认。
- 新增 `src/lib/sql-format.ts`：选项归一化、localStorage 读写、压缩词法扫描器、`DELIMITER` / 过程体检测、大小限制。
- 修改 `src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`、`src/App.vue`：编辑菜单 / 命令面板增加压缩与设置。
- 修改 `src/lib/shortcuts.ts`、`src/components/ShortcutHelpDialog.vue`：登记 `Shift+Mod+F` 与压缩入口说明。
- `package.json` 已含 `sql-formatter`，无需新增依赖。
- 修改 `src/styles.css`：格式下拉与设置对话框。

**后端**

- 无新增命令；不访问数据库。

### 风险与限制

- **过程体**：MySQL / PostgreSQL 的 `BEGIN ... END`、`$$` 体不适合直接格式化；本项拒绝，不尝试猜测。
- **方言差异**：`sql-formatter` 对 MariaDB 特殊语法、MySQL `REPLACE` 函数、PostgreSQL `DO` 块支持有限；失败时保留原文。
- **压缩破坏注释**：普通注释会被删除，用户需理解这是文本清理；依赖注释的脚本应先格式化。
- **大文本**：100 KB 上限；实测卡顿可下调到 50 KB，但必须提示。
- **撤销边界**：必须单事务；多次 dispatch 会导致一次撤销只恢复一部分。
- **未保存标记**：格式化会触发 dirty，这是预期行为；不要静默写回 SQL 文件。
- **设置迁移**：localStorage 值损坏时回退默认，不要让整个编辑器不可用。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/lib/sql/sqlFormatter.ts`、`sqlFormatterConfig.ts`、`layout.ts`；`apps/desktop/src/lib/editor/queryEditorFormatSelection.ts`；`apps/desktop/src/components/editor/SqlFormatterSettingsPanel.vue`；文档 `docs/content/docs/sql-formatter.cn.mdx`。参考其「只改文本、不执行 SQL、方言与选项分离」的边界；不复制其自定义排版引擎。
- **Navicat**：SQL 美化 / 格式化与压缩；参考官方手册的 Query Editor 章节。
- **第三方库**：`sql-formatter` 15.x（MIT），`https://github.com/sql-formatter-org/sql-formatter`。
- **mydb 现状**：`src/components/SqlEditor.vue::formatSql`、`src/components/QueryWorkspace.vue::formatEditorSql`、`src/App.vue` 的 `editor-format-sql`。

---

## C2-02 数据库 / 模式创建与删除

- **优先级**：P1
- **预估**：1 个开发会话
- **依赖**：现有对象树数据库 / 模式节点、`driver_for`、只读守卫
- **替代关系**：细化并替代 B-13。EXPLAIN 已由并行实现落地（见 0.7），不占用本批需求位。

### 用户问题 / 场景

本地开发经常要新建一个 MySQL 数据库或 PostgreSQL 模式；现在只能新开查询手写 `CREATE DATABASE` / `CREATE SCHEMA`，创建后还要手动刷新对象树，名称里的特殊字符也要自己引用。删除旧库 / 模式同样要手写 `DROP`，容易误删系统库或当前正在用的库。用户需要对象树上的「新建 / 删除」入口，带校验与二次确认。

### 期望行为

1. **入口**：
   - MySQL：连接根节点右键「新建数据库…」；数据库节点右键「删除数据库…」；
   - PostgreSQL：连接根节点右键「新建数据库…」；模式节点右键「新建模式…」「删除模式…」；
   - SQLite / Redis 不显示这些菜单；
   - 只读连接禁用并显示原因。
2. **新建数据库（MySQL）**：
   - 字段：名称（必填）、字符集（默认 `utf8mb4`，可选常用值）、排序规则（可选，默认跟随字符集）；
   - 生成 `CREATE DATABASE <quoted name> CHARACTER SET ... COLLATE ...`；
   - 名称不能为空、不能是系统库（`information_schema`、`mysql`、`performance_schema`、`sys`）、不能包含分号；
   - 成功后刷新对象树；提供「创建后切换到此库」勾选（默认开），切换失败不影响创建结果。
3. **新建数据库 / 模式（PostgreSQL）**：
   - 新建数据库：名称必填；第一版只支持名称与默认编码 / 模板，不提供 owner / locale 高级项；
   - 新建模式：名称必填；生成 `CREATE SCHEMA <quoted name>`；
   - PostgreSQL 的 `CREATE DATABASE` 不能在事务中执行；后端必须在普通连接上直接执行，不经过事务；
   - 新建数据库成功后对象树刷新，提示「需要重新连接才能浏览该数据库」；新建模式成功后刷新模式列表并可切换。
4. **删除**：
   - 二次确认框显示对象名、类型与「不可恢复」提示；当前连接的库 / 模式额外警告「正在使用」；
   - MySQL 系统库禁止删除；PostgreSQL `pg_catalog` / `information_schema` / `pg_toast` 等系统模式禁止删除；
   - 不生成 `CASCADE`；非空库 / 模式由数据库返回错误并原样展示；
   - 删除成功后刷新对象树；若删除的是当前库 / 模式，先切回安全默认库 / 模式（MySQL `mysql`、PostgreSQL `postgres` / `public`）或关闭连接，不能停留在已删除对象上。
5. **校验与错误**：
   - 名称前后空白去除；空名称、系统名称、含分号、超长（>63 / 64 字符按驱动）在提交前提示；
   - 已存在、权限不足、字符集 / 排序规则不支持时显示数据库原始错误，对话框不关闭、输入保留；
   - 只读连接在命令层拒绝；前端禁用只是提示。
6. **安全**：
   - 标识符必须按驱动引用，不能字符串拼接未转义名称；
   - 不做级联删除；不自动删除连接、表或数据文件；
   - 删除数据库 / 模式不写入 SQL 执行历史（或按现有 DDL 策略记录一条，不能记录两次）。

### 验收标准

- [ ] MySQL 新建 `mydb_demo`（utf8mb4）后对象树出现该库；勾选「创建后切换」时当前库变为 `mydb_demo`，可继续建表。
- [ ] MySQL 新建时输入已存在名称 / 系统库名 / 空名称，均被阻止或返回数据库错误，对话框保留输入。
- [ ] PostgreSQL 新建 `mydb_demo` 数据库后对象树刷新；提示需要重新连接，重新连接后可浏览。
- [ ] PostgreSQL 在当前数据库下新建 `app_test` 模式，对象树出现并可切换；该模式可建表。
- [ ] 删除刚创建的空 MySQL 数据库成功；删除非空数据库返回数据库错误，对象仍在。
- [ ] 删除 PostgreSQL 模式成功；删除系统模式 / 当前模式时菜单禁用或额外警告，确认后行为符合预期。
- [ ] 删除确认框包含对象名与「不可恢复」提示；不存在一键 `CASCADE` 选项。
- [ ] 只读连接入口禁用，绕过前端调用后端返回只读错误；SQLite / Redis 不显示入口。
- [ ] 删除当前 MySQL 库后，应用自动切到 `mysql` 或关闭连接，不继续对已删除库执行操作。
- [ ] 名称含反引号 / 双引号 / 空格时按驱动正确引用，创建与删除均成功。
- [ ] 创建 / 删除失败时对象树不出现幽灵节点；刷新后与数据库实际状态一致。

### 涉及文件与命令

**前端**

- 新增 `src/components/CreateDatabaseDialog.vue`：名称、MySQL 字符集 / 排序规则、创建后切换。
- 修改 `src/components/ObjectBrowser.vue`：连接根节点「新建数据库」；数据库 / 模式节点「删除」；按驱动显示。
- 修改 `src/components/ObjectCategoryGroups.vue`（如数据库节点菜单在此）：同一动作与禁用逻辑。
- 修改 `src/App.vue`：打开对话框、创建后刷新 / 切换、删除当前库后的回退。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：创建 / 删除命令与结果。
- 修改 `src/styles.css`：对话框与危险确认样式。

**后端**

- 新增 `src-tauri/src/commands/database.rs`（或扩展 `connections.rs`）：
  - `create_database(connection_id, name, charset, collation)`;
  - `create_schema(connection_id, name)`;
  - `drop_database(connection_id, name)`;
  - `drop_schema(connection_id, name)`。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/src/db/driver.rs`：新增 `create_database` / `create_schema` / `drop_database` / `drop_schema` 默认返回不支持；MySQL / PostgreSQL 实现。
- 修改 `src-tauri/src/db/mysql.rs`：`CREATE DATABASE` / `DROP DATABASE`，字符集 / 排序规则白名单校验。
- 修改 `src-tauri/src/db/postgres.rs`：`CREATE DATABASE` / `DROP DATABASE` / `CREATE SCHEMA` / `DROP SCHEMA`；`CREATE DATABASE` 不走事务。
- 所有命令先 `ensure_writable`；名称经驱动引用；系统库 / 模式名单在后端再次校验。

### 风险与限制

- **高危删除**：必须二次确认并显示名称；不做 `CASCADE`；非空库由数据库拒绝，不能偷偷清理。
- **PostgreSQL `CREATE DATABASE`**：不能在事务块中执行；不能与当前连接的其他事务共用；创建后通常需要新连接才能看到。
- **当前库删除**：删除后当前上下文失效；必须先切换或关闭连接，不能继续执行。
- **字符集 / 排序规则**：MySQL 版本差异大（8.0 默认 `utf8mb4_0900_ai_ci`，MariaDB 不同）；只提供常用白名单，避免拼接任意值。
- **权限**：普通账号可能只能建库不能删库；错误原样显示。
- **系统对象**：不要把 `sys` / `information_schema` 等当普通库；后端再次拦截。
- **历史**：创建 / 删除是 DDL，是否记录历史需与现有 DDL 策略一致，避免两条。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/lib/database/createDatabaseSql.ts`、`createDatabaseCharsetOptions.ts`、`createDatabaseErrorHints.ts`、`databaseNamespaceCreation.ts`；对象树数据库 / 模式菜单。参考其「名称校验 + 字符集白名单 + 错误提示」；不复制其多驱动 namespace 适配层。
- **Navicat**：新建数据库 / 模式与删除数据库；参考官方手册的 Database / Schema 章节。
- **mydb 现状**：`src/components/ObjectBrowser.vue`、`src/components/ObjectCategoryGroups.vue`、`B-13`。

---

## C2-03 对象定义查看与编辑

- **优先级**：P1
- **预估**：1.5 个开发会话
- **依赖**：`get_object_ddl`、现有 `execute_query`、C-02 的单语句执行与错误处理
- **替代关系**：替代 B-10。对象创建（视图 / 存储过程 / 函数 / 触发器）已由工作区的 `CreateObjectDialog.vue` 实现，本项**不重复创建**，只补定义查看与编辑。

### 现状（不要重复实现）

- 对象树顶部已有「新建表 / 新建视图 / 新建存储过程 / 新建函数 / 新建触发器」入口，`CreateObjectDialog.vue` 已能生成并执行 DDL；
- 对象右键只有「复制 DDL」；没有完整定义查看器，也没有编辑入口；
- 后端 `get_object_ddl` 已覆盖 MySQL 表 / 视图 / 例程 / 触发器 / 事件、PostgreSQL 表 / 视图 / 物化视图 / 函数 / 存储过程 / 触发器 / 索引、SQLite 表 / 视图 / 索引 / 触发器；
- PostgreSQL 同名重载函数目前只按名称读取，未提供变体选择。

### 用户问题 / 场景

对象已经能创建，但建完之后只能「复制 DDL」到外部编辑器看；想在应用内核对一个视图、函数或触发器到底定义成什么，或者改一处定义再保存，仍然做不到。用户需要「右键查看定义 → 搜索 / 复制 / 另存为 → 受控编辑 → 预览 → 保存」的闭环，并且不能改错重载函数或误删依赖对象。

### 期望行为

1. **查看定义**：
   - 对有 DDL 的对象，右键菜单增加「查看定义…」；
   - 打开 `ObjectSourceDialog.vue`，显示完整 DDL；带行号、SQL 语法高亮、自动换行、文本搜索（高亮 / 下一个）、复制、另存为 `.sql`；
   - 只读展示，不执行、不修改对象；Esc / 关闭按钮关闭，焦点回到对象树。
2. **支持范围**：
   - MySQL：表、视图、存储过程、函数、触发器、事件；
   - PostgreSQL：表、视图、物化视图、函数、存储过程、触发器、索引、序列；
   - SQLite：表、视图、索引、触发器。
3. **可编辑范围**：
   - 可编辑：MySQL / PostgreSQL 视图；PostgreSQL 函数 / 存储过程；SQLite 视图 / 触发器；
   - 只读：MySQL 存储过程 / 函数 / 触发器 / 事件（避免 `DELIMITER` 与依赖问题），界面明确说明；
   - 表结构、索引、序列等不提供定义编辑，继续走结构页 / 索引入口。
4. **编辑流程**：
   - 对话框分「编辑」与「预览」；预览显示目标对象、完整 SQL、是否包含 `DROP`；
   - 视图 / PostgreSQL 例程使用 `CREATE OR REPLACE`；SQLite 视图 / 触发器使用「DROP + CREATE」并在**一个事务**中执行；
   - 保存前必须经过预览；执行成功后重新读取定义、刷新对象树；失败保留编辑内容与预览，数据库原定义不变；
   - 不自动执行 `DROP ... CASCADE`，不修改依赖对象；预览中对依赖风险给出文字提示。
5. **PostgreSQL 重载函数**：
   - 查看定义前先列出候选签名（名称 + 参数类型 + 返回类型）；用户选择后再读取；
   - 未选择或无法唯一确定时禁止编辑，不静默展示第一个；
   - 后端新增 `list_object_variants`；`get_object_ddl` 接受可选 `identity` 定位具体变体。
6. **搜索**：
   - 对话框内搜索只在本地文本中定位，不发送数据库请求；支持区分大小写开关（可选）。
7. **安全与权限**：
   - 只读连接禁用保存，后端命令层再次拒绝；
   - 定义文本上限 2 MB；超过时只读展示并提示，不加载到编辑器；
   - 不把定义文本写入执行历史 / 日志；
   - MySQL `DEFINER` 原样展示；保存时不自动移除，除非用户在预览中手工修改。
8. **错误**：
   - 读取失败显示数据库原始错误，保留对话框；
   - 保存失败显示失败语句、数据库错误；不回滚用户已经完成的其他操作；
   - 对象在打开对话框后被删除时，保存给出明确错误，不覆盖同名新对象。

### 验收标准

- [ ] MySQL 存储过程右键「查看定义…」显示完整 `CREATE PROCEDURE ... BEGIN ... END`，可搜索、复制、另存为 `.sql`；编辑按钮不存在或禁用并说明原因。
- [ ] PostgreSQL 视图、物化视图、函数、触发器、索引均可查看到可读定义；DDL 与 `pg_get_*` 结果一致。
- [ ] SQLite 视图 / 触发器可查看到 `sqlite_master` 中的原始 DDL。
- [ ] PostgreSQL 同名重载函数点「查看定义」先列出至少两个签名；选择后显示对应定义；未选择时不能编辑。
- [ ] MySQL / PostgreSQL 视图编辑并保存后，查询结果按新定义变化；对象树与定义对话框刷新；失败时原定义不变、编辑内容保留。
- [ ] PostgreSQL 函数 / 存储过程编辑使用 `CREATE OR REPLACE`；保存后对象仍存在，依赖对象不因保存被删除。
- [ ] SQLite 触发器编辑：预览同时显示 `DROP TRIGGER` 与 `CREATE TRIGGER`；执行失败时事务回滚，原触发器仍存在。
- [ ] SQLite 视图编辑：预览显示 `DROP VIEW` 与 `CREATE VIEW`；成功后新定义生效，失败回滚。
- [ ] 只读连接下「查看定义」可用，「编辑 / 保存」禁用；绕过前端调用后端返回只读错误。
- [ ] 非法定义保存失败时显示数据库原始错误，编辑器内容不丢失，数据库原定义不变。
- [ ] 定义超过 2 MB 时只读展示并提示，不进入编辑器、不卡死界面。
- [ ] 搜索文本可定位并高亮，不触发数据库请求；关闭对话框后对象树焦点恢复。
- [ ] 保存后执行历史 / 日志中没有完整定义文本。

### 涉及文件与命令

**前端**

- 新增 `src/components/ObjectSourceDialog.vue`：查看 / 编辑 / 预览 / 搜索 / 另存为 / 只读提示。
- 新增 `src/components/ObjectSourcePreview.vue`（或内嵌预览）：完整 SQL、目标说明、DROP 提示、复制。
- 修改 `src/components/ObjectCategoryGroups.vue`：右键「查看定义…」；按 `canFetchDdl` 与可编辑类型显示入口。
- 修改 `src/App.vue`：打开对话框、保存成功后刷新对象树 / 重新读取定义。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：`getObjectDdl` 增加可选 identity；`listObjectVariants` 类型。
- 修改 `src/styles.css`：定义对话框、编辑器、搜索高亮。
- 可复用 `SqlEditor.vue` 的只读展示能力；若其运行 / 事务按钮不适合对话框，增加 `readonly` / `hide-actions` 参数，而不是复制第二套编辑器。

**后端**

- 修改 `src-tauri/src/commands/data.rs`：
  - `get_object_ddl(connection_id, kind, name, identity: Option<String>)`；
  - `list_object_variants(connection_id, kind, name)`；
  - 可选 `save_object_definition(...)`（统一只读校验、单语句 / 事务执行、错误上下文）。
- 修改 `src-tauri/src/db/driver.rs`：新增 `object_variants` / `object_ddl_with_identity` 默认方法。
- 修改 `src-tauri/src/db/mysql.rs`：视图 / 例程 / 触发器 / 事件定义；MySQL 定义编辑不开放。
- 修改 `src-tauri/src/db/postgres.rs`：`pg_proc` 按 OID 列出重载、`pg_get_functiondef` 读取具体变体；视图 / 触发器定义。
- 修改 `src-tauri/src/db/sqlite.rs`：`sqlite_master` 读取；SQLite 视图 / 触发器 DROP + CREATE 事务。
- 修改 `src-tauri/src/lib.rs`：注册新命令。

### 风险与限制

- **重载函数**：PostgreSQL 同名函数可能多个；对象树按名称聚合，必须补变体列表，否则会改错对象。
- **SQLite DROP + CREATE**：事务失败必须回滚；不能自动级联删除依赖对象。
- **对象依赖**：视图 / 函数可能被其他对象引用；本项不做依赖分析，保存前只提示风险。
- **DDL 保真**：数据库返回的定义可能缺注释 / 权限 / 存储属性；编辑以完整 DDL 为准，不自行补全。
- **MySQL 例程**：只读是刻意限制；不要为了“功能完整”而开放，`DELIMITER` 与依赖问题需要单独设计。
- **权限**：读取定义与保存定义需要不同权限；失败时显示原始错误。
- **大小**：2 MB 上限；超过只读展示，避免 CodeMirror 卡死。
- **对象被替换**：对话框打开后对象被删除 / 重建，保存可能覆盖同名新对象；后端应校验 OID / 标识并提示重新打开。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/objects/ObjectSourceDialog.vue`、`ObjectSourceSqlPreview.vue`、`RoutineMetadataPanel.vue`；`apps/desktop/src/lib/table/objectSource*.ts`、`routineDefinitionSave.ts`、`tableObjectSourceKind.ts`；文档 `docs/content/docs/schema-browser.cn.mdx`、`object-browser.cn.mdx` 的「打开源码 / DDL 与源码」。参考其「源码只读 / 可编辑 / 预览 / 保存」分层；不复制其 Oracle 专用逻辑。
- **Navicat**：对象设计器与「Design View / 源码」编辑；参考官方手册的 View / Procedure / Function / Trigger 章节。
- **mydb 现状**：`src/components/ObjectCategoryGroups.vue::menuCopyDdl`、`CreateObjectDialog.vue`、`get_object_ddl`、`B-10`。

---

## C2-04 全局数据库搜索（对象 / 字段）

- **优先级**：P1
- **预估**：1–1.5 个开发会话
- **依赖**：现有元数据命令；C2-03（点击例程结果打开定义，可选）
- **替代关系**：新增；补 B-25 命令面板只搜「当前库 / 模式已加载对象」的缺口。

### 用户问题 / 场景

对象树只能看当前库 / 模式，命令面板也只索引当前范围。用户记得「有个 `order_status` 字段」但不知道在哪张表、哪个库；或者只记得视图名的一部分，却要挨个数据库展开。需要一次搜索当前连接下的所有库 / 模式，按对象名、字段名、注释命中，并直接打开目标。

### 期望行为

1. **入口**：
   - 对象树标题栏增加「搜索所有库 / 模式…」按钮；
   - 命令面板增加「搜索数据库对象…」；
   - 快捷键 `Shift+Mod+O`；
   - Redis 连接不显示入口；SQLite 搜索当前文件。
2. **搜索范围**：
   - 固定为**当前连接**，不跨连接；
   - MySQL：当前连接下所有非系统数据库（默认排除 `information_schema`、`mysql`、`performance_schema`、`sys`）；
   - PostgreSQL：当前数据库下所有非系统模式（默认排除 `pg_catalog`、`information_schema`、`pg_toast`）；不跨数据库重连；
   - SQLite：`sqlite_master` 中的表 / 视图 / 索引 / 触发器；
   - 搜索对象：表、视图、物化视图、序列、存储过程、函数、触发器、索引；字段名与表 / 字段注释；
   - 不搜索数据行、不搜索视图 / 函数源码正文。
3. **匹配规则**：
   - 最小 2 个字符；忽略大小写；子串匹配；
   - 表名优先于字段名，完全匹配优先于前缀，前缀优先于子串；
   - 结果按「库 / 模式 → 类型」分组，最多 200 条；超过显示「结果过多，请补充关键字」，不静默截断；
   - 系统对象默认不出现；提供「包含系统对象」开关（默认关）。
4. **交互**：
   - 输入 250ms 防抖；新请求发出时取消 / 忽略旧请求结果（operation id 或递增序号），避免旧结果覆盖新结果；
   - 结果项显示：名称、类型徽标、限定路径（连接 · 库 · 模式）、字段所属表、类型 / 注释摘要；
   - ↑/↓ 选择，Enter 打开，Esc 关闭；Tab 在「全部 / 表 / 字段 / 例程」之间切换过滤；
   - 打开行为：
     - 表 / 视图：打开数据标签；
     - 字段：打开所属表的结构标签，并滚动 / 高亮该字段（结构页需支持 `focusColumn` 参数）；
     - 存储过程 / 函数 / 触发器：打开 C2-03 的定义对话框；若 C2-03 未实现，则复制 DDL 并提示；
     - 序列 / 索引：打开所属对象的结构或复制名称；
   - 搜索框保留最近一次查询（会话内），不写入历史。
5. **性能与错误**：
   - MySQL 用 `information_schema.tables` / `columns` / `routines` 的单次查询或合并查询；PostgreSQL 用 `pg_class` / `pg_namespace` / `pg_attribute` / `pg_proc`；SQLite 用 `sqlite_master` + `PRAGMA table_info`（按需）；
   - 不对每个对象逐个发请求；查询超时 / 权限不足时显示可读错误，不清空已有结果；
   - 连接关闭 / 切换连接时关闭搜索并清空结果；
   - 只读连接可用；不产生业务写入。
6. **边界说明**：
   - 第一版不做跨连接、不做 PostgreSQL 跨数据库、不做数据内容全文搜索、不做字段血缘；
   - 界面明确写「搜索当前连接的对象与字段名」。

### 验收标准

- [ ] MySQL 连接含 3 个数据库，搜索 `user` 能同时命中不同库中的表 / 视图，结果显示库名与类型；系统库默认不出现。
- [ ] 打开「包含系统对象」后，`information_schema` 中的匹配对象出现；关闭后消失。
- [ ] PostgreSQL 在多个模式下搜索表名与字段名均命中；结果路径显示模式名；不触发跨数据库重连。
- [ ] SQLite 搜索表 / 视图 / 索引 / 触发器与字段名均正确。
- [ ] 搜索 `order_status` 字段时结果显示所属表；Enter 打开该表结构并高亮字段。
- [ ] 输入 1 个字符时不请求；输入 2 个字符后 250ms 内发起一次请求；快速连续输入不会出现旧结果覆盖新结果。
- [ ] 结果超过 200 条时显示截断提示与「请补充关键字」，界面不卡死。
- [ ] ↑/↓/Enter/Esc 与 Tab 类型过滤可用；鼠标点击同样能打开。
- [ ] 存储过程结果点击后打开定义对话框（C2-03 已完成时）；未完成时给出明确降级行为。
- [ ] 只读连接搜索可用；不产生 SQL 执行历史；不修改数据库。
- [ ] 连接切换 / 关闭时搜索关闭，旧结果不残留在新连接上。
- [ ] 权限不足（如看不到 `pg_proc`）时显示可读错误，已有结果保留，不崩溃。

### 涉及文件与命令

**前端**

- 新增 `src/components/DatabaseSearchDialog.vue`：输入、类型过滤、分组结果、键盘导航、错误 / 截断提示。
- 新增 `src/lib/database-search.ts`：结果排序、分组、类型徽标、路径格式化。
- 修改 `src/components/ObjectBrowser.vue`：搜索按钮。
- 修改 `src/components/TableStructureView.vue`：接受 `focusColumn` 并滚动 / 高亮。
- 修改 `src/App.vue`：打开对话框、执行结果动作、切换连接时清理。
- 修改 `src/lib/app-actions.ts`、`src/lib/shortcuts.ts`、`src/components/ShortcutHelpDialog.vue`：命令面板与快捷键。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：搜索命令与结果类型。

**后端**

- 新增 `src-tauri/src/commands/search.rs`：`search_objects(connection_id, query, options)`。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/src/models.rs`：`ObjectSearchOptions`、`ObjectSearchResult`、`ObjectSearchKind`。
- 修改 `src-tauri/src/db/driver.rs`：新增 `search_objects` 默认实现（返回不支持）或复用现有 `list_database_objects` + `list_columns`（性能不足，不推荐）。
- 修改 `src-tauri/src/db/mysql.rs`、`postgres.rs`、`sqlite.rs`：各驱动一次查询返回对象 / 字段结果；限制 200 条；转义 `%` / `_`。
- 不新增数据库写入命令。

### 风险与限制

- **元数据规模**：MySQL 实例可能有数百个库；必须用服务端过滤与 `LIMIT`，不能先拉全量再前端过滤。
- **权限**：`information_schema` / `pg_catalog` 只显示有权限的对象；结果不完整时提示「仅显示当前账号可见对象」，不伪造。
- **PostgreSQL 跨库**：不实现；用户需要切换数据库后重新搜索。
- **注释搜索**：MySQL 表 / 字段注释在 `information_schema` 中可取；PostgreSQL 需要 `obj_description` / `col_description`，可能较慢；第一版可只搜名称，注释作为可选增强。
- **字段消歧**：同名字段很多，必须显示所属表与库 / 模式；不能只给字段名。
- **大小写**：MySQL 在部分平台表名大小写敏感；搜索按 `LOWER()` 可能无法命中大小写敏感对象，需在实现时按驱动策略处理并说明。
- **不执行 SQL**：搜索只读元数据；不要为了匹配源码而读取所有例程定义。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/search/DatabaseSearchDialog.vue`、`DatabaseSearchPanel.vue`；`apps/desktop/src/lib/database/databaseSearch.ts`、`databaseSearchColumns.ts`、`databaseSearchCapabilities.ts`；快速打开 `apps/desktop/src/components/quick-open/GlobalSearch*.vue`；文档 `docs/content/docs/schema-browser.cn.mdx` 的「搜索与过滤」。参考其「当前连接范围、类型分组、服务端过滤」的设计；不复制跨 100+ 驱动的适配层。
- **Navicat**：对象搜索 / 在数据库中查找（Find in Database）；参考官方手册的 Search 章节。
- **mydb 现状**：`src/App.vue::paletteObjects`（只遍历当前 `databases` / `schemas` / `tables`）、`B-25`。

---

## C2-05 表数据高级能力：值查看器 / 虚拟滚动 / XLSX

- **优先级**：P1
- **预估**：1.5–2 个开发会话
- **依赖**：现有 `ResultGrid.vue`、`TableDataView.vue`、`result-export.ts`、C-03 的批量编辑
- **替代关系**：细化并替代 B-16；新增虚拟滚动与 XLSX 导出（B-02 / C-03 均未覆盖）。

### 用户问题 / 场景

JSON / JSONB、长文本、超宽表在单元格里被压成一行，只能复制出去看；结果 10,000 行时一次性渲染所有 `<tr>`，滚动卡顿；导出只有 CSV / JSON / Markdown / SQL，交给业务同事还要再转 Excel。用户需要「点开看值、滚动不卡、直接给 .xlsx」。

### 期望行为

#### A. 大字段 / JSON 值查看器

1. 入口：
   - 结果网格：右键「查看值…」，双击非数字 / 非布尔单元格也可打开；
   - 表数据页：右键「查看值…」；双击仍进入编辑，避免破坏现有编辑习惯；
   - 仅对当前单元格有效，不打开整行。
2. 查看器内容：
   - 标题显示列名、行号、类型（可得时）、字符数 / 字节数；
   - 「格式化 / 原始」切换；合法 JSON 对象 / 数组默认格式化 2 空格缩进；非法 JSON 只显示原文并提示「不是合法 JSON」；
   - 自动换行开关；等宽字体；深色主题；
   - 复制当前视图内容；另存为 `.txt` / `.json`（按当前视图）；
   - NULL 显示 `NULL` 并禁用保存；BLOB / BYTEA 显示 `<BLOB n bytes>` 占位与提示，不做二进制解码；
   - 2 MB 以上文本只读展示并提示，不加载到可编辑控件。
3. 交互：Esc / 遮罩关闭；关闭后焦点回到原单元格；不修改数据库、不修改单元格。

#### B. 虚拟滚动

4. 结果网格：
   - 行数超过 100 时启用虚拟滚动；只渲染视口 + 10 行 overscan；
   - 固定行高（约 30px）；表头 sticky；列宽、隐藏列、排序、行号、右键菜单、复制、导出快照保持现有行为；
   - 滚动到 10,000 行底部不出现明显白屏；DOM 中 `tbody tr` 数量不超过 200；
   - 排序后滚动位置回到顶部；筛选 / 重新执行后重置。
5. 表数据页：
   - 每页 500 行时同样启用虚拟滚动；待提交高亮、行选择、批量设置、单元格编辑、右键菜单保持可用；
   - 编辑中的行不得被回收导致输入丢失；滚动到编辑行外时先提交 / 取消该单元格编辑（按现有逻辑）。
6. 共享实现：新增 `useVirtualRows` / `VirtualTableBody` 组合式或组件，避免两套逻辑；行高与容器高度由 CSS 变量统一。
7. 降级：容器高度未知、打印模式、导出时仍使用完整数据；虚拟化只影响渲染，不影响导出与复制全部。

#### C. XLSX 导出

8. 入口：
   - 查询结果「导出…」格式增加 XLSX；
   - 表数据页工具栏「导出」增加 XLSX（当前页）；
   - 对象右键导出格式增加 XLSX（全部行或指定行数）；
   - 导出对话框显示行数、列数、工作表名、是否含表头。
9. 内容规则：
   - 第一行表头，加粗、冻结首行、启用自动筛选；
   - 数字写为数值单元格，布尔写布尔，日期 / 时间按文本或日期格式（第一版可统一文本并保留原样）；
   - NULL 写空单元格；JSON / 对象写紧凑 JSON 文本；二进制写 `<BLOB n bytes>` 占位并在导出汇总提示；
   - 字符串以 `=`、`+`、`-`、`@` 开头时必须按文本写入，禁止被 Excel 解释为公式；
   - 工作表名去除 `[]:*?/\` 并截断到 31 字符；空名回退 `Sheet1`；
   - 列宽按内容估算，上限 60 字符；超长文本不自动换行。
10. 实现：
    - 新增 Rust 命令 `export_query_result_xlsx`，接收列名、行数据、工作表名、是否表头；使用 `rust_xlsxwriter`（MIT OR Apache-2.0）；
    - 对象导出扩展 `export_object` 的 `format = "xlsx"`，复用流式读取，单文件最多 200,000 行；超过时拒绝并提示先用 CSV；
    - 写文件使用同目录临时文件 + rename，失败 / 取消清理；
    - 结果集导出上限为当前结果行数（≤10,000）；表数据页为当前页（≤500）。
11. 进度与取消：对象 XLSX 导出显示进度并可取消；结果 / 当前页导出通常很快，仍显示忙碌状态。

### 验收标准

- [ ] 结果网格中一个 JSONB 单元格右键「查看值…」，默认格式化显示；切到「原始」显示单行；复制内容与当前视图一致。
- [ ] 非法 JSON 文本只显示原文并提示，不空白、不抛错。
- [ ] 100 KB 文本查看器滚动流畅，自动换行开关生效；另存为 `.txt` 内容与原文一致。
- [ ] NULL 显示 `NULL`，BLOB 显示占位与字节数，不出现乱码。
- [ ] 结果 10,000 行时滚动到底部，DOM 中 `tbody tr` 少于 200；排序、隐藏列、右键复制、导出仍正确。
- [ ] 表数据页每页 500 行时虚拟滚动生效；行选择、批量设置、单元格编辑、待提交高亮均正常。
- [ ] 在虚拟列表中编辑第 400 行后滚动离开再回来，待提交值仍保留（不因行回收丢失）。
- [ ] 查询结果导出 XLSX，用 Excel / Numbers 打开：表头加粗、首行冻结、自动筛选可用、中文正常。
- [ ] 表数据当前页导出 XLSX，行数与页面一致；NULL 为空单元格，数字为数值类型。
- [ ] 对象右键导出 10,000 行 XLSX，行数正确；超过 200,000 行时拒绝并提示。
- [ ] 字符串 `=1+1`、`+SUM(A1)`、`-2`、`@user` 在 XLSX 中作为文本显示，不执行公式。
- [ ] 导出失败 / 取消后无残留临时文件；数据库无写入，执行历史不新增。
- [ ] 虚拟滚动启用前后，复制全部结果与导出快照的行数 / 顺序一致。

### 涉及文件与命令

**前端**

- 新增 `src/components/CellValueDialog.vue`：值查看器。
- 新增 `src/components/VirtualTableBody.vue` 或 `src/lib/virtual-rows.ts`：虚拟行计算、滚动、overscan。
- 修改 `src/components/ResultGrid.vue`：双击 / 右键查看值、虚拟滚动、行高与容器测量。
- 修改 `src/components/TableDataView.vue`：右键查看值、500 行虚拟滚动、编辑态保护。
- 修改 `src/components/ResultExportDialog.vue`、`src/lib/result-export.ts`：XLSX 选项与快照传递。
- 修改 `src/components/ExportDialog.vue`、`src/components/ObjectCategoryGroups.vue`：对象 XLSX 导出入口。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：XLSX 命令与格式类型。
- 修改 `src/styles.css`：虚拟表格、值查看器样式。

**后端**

- 新增 `src-tauri/src/commands/xlsx.rs`（或 `commands/export.rs` 内新增）：`export_query_result_xlsx`。
- 修改 `src-tauri/src/commands/data.rs`：`export_object` 增加 `xlsx` 分支；复用 `export_rows`，限制 200k 行；临时文件 + rename。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/Cargo.toml`：引入 `rust_xlsxwriter`（MIT OR Apache-2.0）。
- 修改 `src-tauri/src/models.rs`：`QueryResultXlsxRequest`、`XlsxExportSummary`。
- 说明：XLSX 写入在 `spawn_blocking` 中执行，避免阻塞 async runtime。

### 风险与限制

- **虚拟滚动 + 可编辑表格**：表数据页有行内编辑、批量选择、待提交高亮；行回收可能丢焦点。必须在编辑期间固定该行，或先结束编辑再回收。
- **列宽与横向滚动**：现有列宽拖拽依赖 `<table>` 布局；虚拟化后要保持 `<colgroup>` 与表头 / 行对齐，需实测。
- **XLSX 内存**：`rust_xlsxwriter` 在内存构建工作表；200k 行是上限，超过拒绝。不要为了“全部导出”无限放大。
- **日期类型**：不同驱动返回字符串 / 数字；第一版统一按文本写入可避免格式错误，但会失去 Excel 日期排序；需在界面说明。
- **公式注入**：所有字符串必须用文本写入，不能调用公式写入 API；这是安全要求，不是可选。
- **二进制**：XLSX 不写原始二进制，只写占位；需要二进制时用 C-01 SQL dump 或原生工具。
- **值查看器**：2 MB 上限；超过只读预览；不做 BLOB 十六进制编辑。
- **导出范围**：结果导出只覆盖当前可见列与排序；对象导出覆盖全部行；表数据只导出当前页。与现有语义保持一致。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/grid/DataGridCellDetailDialog.vue`、`DataGridCellDetailTextPreview.vue`、`DataGridCellDetailHexViewer.vue`；`apps/desktop/src/components/common/VirtualScrollArea.vue`、`JsonTree.vue`；`apps/desktop/src/lib/export/xlsxExport.ts`、`xlsxHeader.ts`；`src-tauri/src/commands/xlsx_export.rs`；文档 `docs/content/docs/data-grid.cn.mdx` 的「单元格详情」「批量粘贴」。参考其「值详情分级、虚拟滚动、XLSX 表头」；不复制其图片 / 十六进制解码器。
- **Navicat**：数据查看器的「备注 / JSON 查看」、虚拟滚动与导出 Excel；参考官方手册的 Table Data Viewer / Export Wizard 章节。
- **第三方库**：`rust_xlsxwriter` 0.104+（MIT OR Apache-2.0）。

---

## C2-06 查询取消与超时

- **优先级**：P1
- **预估**：1.5 个开发会话
- **依赖**：现有 `execute_query` / `executeTransactionQuery`、`AppState`、三个驱动的 `DatabaseSession`
- **替代关系**：细化并替代 B-05；C-07 的「终止服务器会话」是进程级操作，本项是单条查询级取消，两者不重复。

### 用户问题 / 场景

误执行大表 JOIN、无索引查询或 `SELECT SLEEP(300)` 后，mydb 只能一直转圈：运行按钮禁用、没有停止入口，用户只能等数据库返回或强行退出应用；退出后也不知道查询是否还在服务器上跑。用户需要「运行中可停止、超时自动结束、结束后连接还能继续用」，并且不能因为取消把别的标签或未提交事务弄坏。

### 期望行为

1. **界面**：
   - 查询执行中，运行按钮切换为「停止」；点击后按钮显示「停止中…」，再次点击不重复发送；
   - 快捷键 `Mod+.` 触发停止；快捷键速查表登记；
   - 停止 / 超时后，当前标签恢复可编辑，spinner 消失，结果区显示「已取消」或「查询超时（60 秒）」；其他标签不受影响；
   - 表数据页、导入、导出等已有独立取消入口，本项不改变它们的行为。
2. **执行标识**：
   - 每次查询执行生成 `operation_id`（前端生成或后端返回），传给 `execute_query`；`executeTransactionQuery` 同样支持；
   - `AppState` 新增运行中查询注册表：`operation_id -> { connectionId, driverKind, backendId, cancelCapability, startedAt, cancelRequested }`；
   - 查询结束（成功 / 失败 / 取消 / 超时）后移除注册项；重复取消返回「查询已结束」而不是报错。
3. **MySQL**：
   - 查询在一条固定连接上执行；执行前读取 `CONNECTION_ID()`；
   - 取消使用**另一条连接**执行 `KILL QUERY <id>`，只结束当前语句，不断开连接；
   - `KILL QUERY` 失败（无权限 / 语句已结束）时返回可读错误，不自动升级为 `KILL <id>`；
   - 取消后连接仍可执行下一条查询；手动事务中的查询取消后事务仍可提交 / 回滚。
4. **PostgreSQL**：
   - 查询在固定连接上执行；执行前读取 `pg_backend_pid()`；
   - 取消使用另一条连接执行 `SELECT pg_cancel_backend(<pid>)`；
   - 返回 `false` 表示数据库未接受取消，界面显示「数据库未接受取消，请稍后重试」；
   - 不调用 `pg_terminate_backend`（那是 C-07 的会话终止）；取消后事务状态不变。
5. **SQLite**：
   - SQLx 未暴露 `sqlite3_interrupt`；本项不假装支持服务端取消；
   - 点击停止只设置本地标志，界面显示「SQLite 不支持中断，将在超时后结束」；60 秒超时后返回错误并释放等待；
   - 不因为取消而丢弃整个连接池；后续查询仍可用。
6. **超时**：
   - 普通查询与事务查询默认 60 秒超时（第一版固定，不接设置面板）；
   - 超时后先尝试服务端取消（MySQL / PostgreSQL），再返回「查询超时（60 秒）」；
   - 超时不是成功；执行历史记录失败状态与耗时；
   - 取消 / 超时后不自动重试。
7. **事务**：
   - 手动事务的 `TransactionEntry` 记录后端的连接 ID / PID；取消只作用于该事务连接上的当前查询；
   - 取消后事务仍可 `COMMIT` / `ROLLBACK`；不能因为取消自动回滚或关闭事务；
   - 关闭标签 / 切换连接时的现有回滚逻辑不变；关闭连接前若有运行中查询，先请求取消并提示。
8. **历史与状态**：
   - 取消 / 超时本次查询写入执行历史，状态为失败，错误为「已取消」/「查询超时」；
   - 不写入成功记录，不重复写两条；
   - 状态栏显示当前标签「执行中 / 停止中 / 已取消 / 已超时」，不残留全局 spinner。
9. **只读**：只读连接可取消自己的 SELECT；取消不是写操作，不经过 `ensure_writable`。
10. **边界**：Redis 命令、导出、导入、同步已有各自取消机制，本项不接管；`EXPLAIN` 查询通常很快，也走同一超时/取消注册但不单独优化。

### 验收标准

- [ ] MySQL 执行 `SELECT SLEEP(30)`，点击「停止」后 2 秒内返回「已取消」，随后在同一连接执行 `SELECT 1` 成功。
- [ ] MySQL 取消只结束语句不断开连接：`SHOW PROCESSLIST` 中应用连接仍存在（或下一条查询直接成功）。
- [ ] PostgreSQL 执行 `SELECT pg_sleep(30)`，点击「停止」后 2 秒内返回「已取消」，连接与事务状态可用。
- [ ] PostgreSQL 取消返回 `false` 时显示「数据库未接受取消」，不误报成功。
- [ ] SQLite 执行长递归 CTE，点击「停止」后显示「SQLite 不支持中断，将在超时后结束」；60 秒后返回超时，后续查询可用。
- [ ] 两个查询标签同时运行，取消其中一个，另一个继续并正常返回结果。
- [ ] 手动事务中执行慢查询并取消：事务未提交、未回滚，随后可继续执行语句并提交 / 回滚。
- [ ] 查询自然结束后再点停止，提示「查询已结束」，不产生错误弹窗；停止按钮已消失。
- [ ] 60 秒超时后标签恢复可编辑，结果区显示超时，执行历史为失败且耗时合理。
- [ ] 取消 / 超时后不自动重试；重新运行同一 SQL 可正常执行。
- [ ] 只读连接可取消 SELECT；取消操作不触发只读写入错误。
- [ ] 关闭标签 / 关闭连接时若有运行中查询，先请求取消并提示；不留下后台查询注册项。
- [ ] 快捷键 `Mod+.` 与运行按钮行为一致；连续按两次不会发送两个取消请求。

### 涉及文件与命令

**前端**

- 修改 `src/components/QueryWorkspace.vue`：运行按钮在 `running` 时切换为「停止」；`stopping` 状态；错误区显示取消 / 超时。
- 修改 `src/App.vue`：生成 `operationId`，调用带 operationId 的执行命令；`stopQuery()`；关闭标签 / 连接前取消；状态栏文案。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：`executeQuery` / `executeTransactionQuery` 增加 operationId；新增 `cancelQuery(operationId)`。
- 修改 `src/lib/shortcuts.ts`、`src/components/ShortcutHelpDialog.vue`：`Mod+.`。

**后端**

- 修改 `src-tauri/src/commands/query.rs`：`execute_query` 接收 `operation_id`，注册运行中查询，使用固定连接执行，超时 / 取消处理，结束注销，写历史。
- 修改 `src-tauri/src/commands/transaction.rs`：`begin_transaction` 记录后端连接 ID / PID；`execute_transaction_query` 支持 operationId；取消不改变事务状态。
- 新增 `src-tauri/src/commands/query_cancel.rs`：`cancel_query(operation_id)`、注册表查询、取消能力判断。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：`AppState` 增加 `running_queries`；注册取消命令。
- 修改 `src-tauri/src/db/driver.rs` / `DatabaseSession`：增加 `backend_id()`（MySQL `CONNECTION_ID()`、PostgreSQL `pg_backend_pid()`、SQLite `None`）与 `cancel_query(backend_id)`（默认不支持）；或由命令层按驱动实现。
- 修改 `src-tauri/src/db/mysql.rs`、`postgres.rs`、`sqlite.rs`：获取 backend id、另一连接发送 `KILL QUERY` / `pg_cancel_backend`、SQLite 超时。
- 修改 `src-tauri/src/models.rs`：`QueryRunState`、`CancelQueryResult`。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn execute_query(
      app: AppHandle,
      state: State<'_, AppState>,
      connection_id: String,
      sql: String,
      max_rows: Option<usize>,
      operation_id: Option<String>,
  ) -> Result<QueryResult, String>;

  #[tauri::command]
  pub async fn cancel_query(
      app: AppHandle,
      state: State<'_, AppState>,
      operation_id: String,
  ) -> Result<CancelQueryResult, String>;
  ```
- 超时使用 `tokio::time::timeout`；取消请求设置标志后由执行侧在超时 / 语句返回时清理。

### 风险与限制

- **服务端权限**：MySQL `KILL QUERY` 需要 `PROCESS` 或同用户权限；PostgreSQL `pg_cancel_backend` 需要同角色 / 超级用户。权限不足必须解释，不能假装取消成功。
- **SQLite 无中断**：只能超时；界面必须明确，不要显示「已取消」而实际还在跑。
- **连接池**：固定连接执行 + 另一连接取消会占用两条连接；取消后必须归还，避免池耗尽。
- **事务边界**：取消不能自动回滚；用户仍需自行提交 / 回滚。MySQL DDL 可能已隐式提交，取消后需提示确认。
- **竞态**：查询可能在取消到达前完成；此时返回成功结果，不能报取消。
- **超时后状态未知**：数据库可能仍在执行；PostgreSQL / MySQL 取消失败时明确「数据库可能仍在运行」，不自动重试。
- **历史重复**：只写一条失败记录；不要同时写取消失败与查询失败两条。
- **与 C-07 区分**：不要用 `KILL <id>` / `pg_terminate_backend` 替代查询取消；那会断开连接并影响事务。

### 参考

- **DBX（Apache-2.0）**：`crates/dbx-core/src/query/query_cancel.rs`（运行中查询注册、取消令牌、完成等待、驱动中断）；`src-tauri/src/commands/query_cancel.rs`；文档 `docs/content/docs/query-editor.cn.mdx` 的「停止执行」与「查询取消取决于驱动能力」。参考其「注册 / 取消 / 等待 / 清理」生命周期；本项不复制其多驱动令牌系统。
- **Navicat**：停止查询 / 取消正在执行的语句；参考官方手册的 Query Editor 章节。
- **mydb 现状**：`src-tauri/src/commands/query.rs`、`src-tauri/src/commands/transaction.rs`、`src/components/QueryWorkspace.vue`、`B-05`。

---

## C2-07 用户 / 权限查看与服务器概览增强

- **优先级**：P2
- **预估**：1–1.5 个开发会话
- **依赖**：C-07 的 `ServerActivityDialog.vue`、`server_overview` / `server_processes`
- **替代关系**：承接 B-37 的「用户 / 权限只读」部分；扩展 C-07 的概览指标，不改进程页。

### 用户问题 / 场景

C-07 已经能看连接数、运行时长和进程，但排查「谁有权限」「为什么这个账号连不上」「当前库多大、哪张表最占空间」仍要手写系统表 SQL。用户需要在一个面板里只读查看用户 / 角色、角色属性、当前库权限，以及连接使用率、库大小和 Top 表；绝不能暴露密码散列，也不能在这里误授权。

### 期望行为

1. **入口与形态**：沿用「工具 → 服务器活动…」对话框，新增第三个页签「用户」；概览页扩展指标。标题显示连接、驱动、当前库 / 模式；只读连接可看，不能改（本项本来就没有改）。
2. **概览增强**：
   - 公共：版本、运行时长、当前用户、当前库 / 模式、连接数 / 最大连接数、连接使用率（%）、当前库大小、当前库表数；
   - MySQL：`Threads_running`、`Queries`、`Slow_queries`、QPS（基于两次采样的差值）、InnoDB 缓冲池命中率（可得时）、Top 5 表（数据 + 索引大小）；
   - PostgreSQL：活动 / 空闲会话数、QPS（`xact_commit + xact_rollback` 差值或 `pg_stat_database`）、缓存命中率（`blks_hit / (blks_hit + blks_read)`）、Top 5 表（`pg_total_relation_size`）；
   - 每个指标显示采样时间；不可得时显示「不支持 / 无权限」，不影响其他指标；
   - 自动刷新沿用 C-07 的关闭 / 5 秒 / 10 秒；QPS 需要至少两次采样，首次显示「采样中」。
3. **用户 / 角色页（只读）**：
   - MySQL：
     - 列表字段：User、Host、plugin、account_locked、password_expired、password_last_changed（列不存在时留空）；
     - 选中用户后按需执行 `SHOW GRANTS FOR 'user'@'host'`，显示授权语句列表；
     - **禁止查询 / 展示 `authentication_string`**；
     - `mysql.user` 不可读时回退 `SELECT user, host FROM mysql.user`；仍失败则显示「当前账号无权查看用户列表」。
   - PostgreSQL：
     - 列表字段：rolname、rolsuper、rolcanlogin、rolcreatedb、rolcreaterole、rolconnlimit、rolvaliduntil；
     - 显示角色成员关系（`pg_auth_members`，最多 200 条）；
     - 显示当前数据库的 CONNECT / CREATE / TEMPORARY 权限（`has_database_privilege`）；不扫描所有数据库；
     - 不查询 `rolpassword`。
   - 通用：
     - 搜索用户 / 角色名；刷新；结果上限 500；
     - 详情区只读展示，不提供「新建用户 / 授权 / 回收 / 改密 / 删除」按钮；
     - 页面顶部固定提示「只读查看；授权与回收请使用 SQL 或专用工具」；
     - 权限不足、视图不存在、版本字段缺失时按分区显示错误。
4. **安全**：
   - 任何查询不得选择密码散列、私钥、令牌；
   - 不把用户列表 / 授权语句写入执行历史；
   - 只读连接与写连接行为一致（都只读）。
5. **驱动范围**：MySQL / PostgreSQL；SQLite / Redis 不显示「用户」页签。

### 验收标准

- [ ] MySQL 打开「用户」页，列出用户与 Host、插件、锁定 / 过期状态；不存在 `authentication_string` 字段或任何散列内容。
- [ ] 选中一个用户，按需显示 `GRANT ...` 语句；无权限时显示可读错误，不崩溃。
- [ ] PostgreSQL 列出角色、登录 / 超级用户 / 建库 / 建角色 / 连接限制 / 有效期；显示角色成员关系。
- [ ] PostgreSQL 显示当前数据库的 CONNECT / CREATE / TEMPORARY 权限；切换数据库后刷新，权限跟随当前库。
- [ ] 只读连接可查看用户页；页面没有任何授权 / 回收 / 改密按钮。
- [ ] MySQL 概览显示最大连接数、Threads_running、QPS、缓冲池命中率、当前库大小、Top 5 表；数值与手工 SQL 一致（QPS 允许采样误差）。
- [ ] PostgreSQL 概览显示活动 / 空闲会话、最大连接数、QPS、缓存命中率、当前库大小、Top 5 表。
- [ ] 首次打开 QPS 显示「采样中」，第二次刷新后显示数值；关闭自动刷新后停止采样。
- [ ] 无权限查看 `mysql.user` 时显示「当前账号无权查看用户列表」，概览与进程页仍可用。
- [ ] 用户页搜索、刷新、500 条上限提示可用；不新增 SQL 执行历史。
- [ ] SQLite / Redis 不显示「用户」页签。

### 涉及文件与命令

**前端**

- 修改 `src/components/ServerActivityDialog.vue`：第三个「用户」页签、概览指标卡片、Top 表、搜索、只读提示。
- 新增 `src/components/ServerUsersPanel.vue`：用户列表 + 详情 / 授权。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：`ServerUser`、`ServerUserDetail`、`ServerTopTable`、扩展 `ServerOverview`。
- 修改 `src/styles.css`：用户表、详情抽屉、指标卡。

**后端**

- 修改 `src-tauri/src/commands/server.rs`：
  - `server_overview` 增加 max_connections、running、QPS 采样原始值、缓存命中率、库大小、表数、Top 5 表；
  - 新增 `server_users(connection_id)`、`server_user_grants(connection_id, user, host)`；
  - 新增 `server_top_tables(connection_id, limit)`（或并入 overview）。
- 修改 `src-tauri/src/models.rs`：`ServerUser`、`ServerUserDetail`、`ServerTopTable`、`ServerOverview` 扩展字段。
- 修改 `src-tauri/src/lib.rs`：注册命令。
- MySQL SQL：
  - 概览：`SHOW GLOBAL VARIABLES LIKE 'max_connections'`、`SHOW GLOBAL STATUS` 子集；
  - 库大小 / Top 表：`information_schema.tables` 按 `data_length + index_length` 排序；
  - 用户：`mysql.user` 安全列；授权：`SHOW GRANTS FOR`。
- PostgreSQL SQL：
  - 概览：`pg_settings` / `pg_stat_activity` / `pg_database_size(current_database())` / `pg_stat_database`；
  - Top 表：`pg_total_relation_size`；
  - 用户：`pg_roles`、`pg_auth_members`、`has_database_privilege`。
- 不新增写入命令；不调用 `ensure_writable`（查看不需要写权限）。

### 风险与限制

- **版本差异**：MySQL 5.7 / 8.0 / MariaDB 的 `mysql.user` 列不同；PostgreSQL 9.x 与 16 的 `pg_roles` 字段也有差异。必须使用能力探测 / 回退查询，不能假设固定列。
- **权限**：普通账号可能看不到 `mysql.user`；`pg_roles` 可见但 `rolpassword` 永远不查。部分结果要能解释。
- **性能**：Top 5 表需要扫描 `information_schema.tables` / `pg_class`，大实例可能慢；限制当前库 / 模式并提供刷新，不自动高频执行。
- **QPS 采样**：必须保存上一次采样值与时间；自动刷新间隔变化时重置基准，避免算出负数或夸张值。
- **安全**：绝不把散列、授权语句写入日志 / 历史；界面不提供复制散列的入口。
- **不是权限编辑器**：本项明确只读；不要添加禁用状态的授权按钮，避免误导。
- **与 C-07 的边界**：进程页、终止会话行为不变；本项只加用户页与概览指标。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/admin/DatabaseUserAdmin.vue`、`MySqlDashboard.vue`、`PostgresDashboard.vue`；`apps/desktop/src/lib/database/databaseUserAdmin.ts`、`mysqlServerStatus.ts`、`postgresServerStatus.ts`、`serverMetrics`（QPS / 缓存命中 / 字节格式化）。参考其「用户列表 + 授权详情」「状态采样 + 速率」；本项只做只读子集，不复制创建 / 授权 / 回收逻辑。
- **Navicat**：用户 / 权限管理器与服务器监视；参考官方手册的 User & Privilege Manager / Server Monitor 章节。
- **mydb 现状**：`src-tauri/src/commands/server.rs`、`src/components/ServerActivityDialog.vue`（C-07）。

---

## C2-08 应用内备份计划

- **优先级**：P2
- **预估**：1.5–2 个开发会话
- **依赖**：C-01 的导出引擎、C-02 的还原入口
- **替代关系**：承接 B-34 的「备份」与 B-36 的「自动运行 / 计划任务」中的**仅备份**子集；不做报表、不做任意查询任务。

### 用户问题 / 场景

C-01 可以手动导出，但“每天下班前备份一次”仍要人记得点。用户需要保存备份计划：固定连接、库 / 模式、表范围、输出目录、频率与保留份数；应用运行时到点自动执行，历史可查，失败可见。用户也接受“应用关闭时不会执行”，但不能被界面误导成系统级任务。

### 期望行为

1. **入口**：「工具 → 备份计划…」或「文件 → 备份计划…」；打开 `BackupSettingsDialog.vue`。
2. **计划列表**：
   - 显示名称、连接、范围、频率、下次运行、上次结果、启用开关；
   - 操作：新建、编辑、删除、立即运行、查看历史、打开输出目录；
   - 空状态说明「仅在 mydb 运行时执行」。
3. **计划编辑**：
   - 基本信息：名称（必填）、连接（仅 SQLite / MySQL / PostgreSQL）、启用；
   - 范围：MySQL 数据库（多选或全部非系统库）/ PostgreSQL 当前数据库 + 模式（多选）/ SQLite 当前文件；表范围：全部 / 包含匹配 / 排除匹配（`*`、`?`，支持 `schema.table`）；
   - 内容：结构、数据（至少选一项）；可选 `DROP ... IF EXISTS`；第一版输出纯 `.sql`，不做 gzip / `.nb3`；
   - 输出：目录必须为绝对路径；文件名模板默认 `mydb-backup-{connection}-{database}-{timestamp}.sql`，支持 `{connection}`、`{database}`、`{schema}`、`{date}`、`{time}`、`{timestamp}`；预览完整路径；
   - 频率：每小时（间隔 1–24 小时）/ 每天指定时间 / 每周指定星期与时间；使用本地时区；
   - 保留：最近 N 次成功备份（1–30），只删除本计划生成的、匹配命名模板的文件；
   - 保存前校验：连接存在、目录可写、至少选结构或数据、模板不为空、频率参数合法。
4. **调度与执行**：
   - 应用启动时加载 `backup-jobs.json`；每 30 秒检查一次到期计划；
   - 同一时刻只运行一个备份任务；同一计划不会重叠；其他计划按顺序等待或跳过并记录；
   - 任务使用 C-01 的导出逻辑（抽成共享函数，不调用 Tauri 命令），输出到临时文件后 rename；
   - 进度事件显示当前库 / 表、已写行数、字节数；支持取消；取消 / 失败删除临时文件；
   - 成功 / 失败写入 `backup-history.json`（最多 200 条）：计划名、连接、范围、起止时间、状态、文件路径、大小、错误；
   - 保留策略在成功后执行；只删除本计划历史中记录且路径位于输出目录内的文件；删除失败记录警告，不删其他文件；
   - 应用未运行时错过的计划：启动后不静默补跑；显示「有 N 个计划已错过，是否立即补跑？」；用户选择后执行；
   - 连接被删除 / 编辑后失效：计划保留但标记「连接不可用」，暂停调度。
5. **限制说明**：
   - 界面固定横幅：「备份计划仅在 mydb 打开时执行；关闭应用、休眠或退出不会运行。不会安装系统服务。」
   - 不提供系统级守护、LaunchAgent、Windows 服务；
   - 不做增量 / 物理备份 / PITR；只是逻辑 `.sql`；
   - 不替代数据库原生 dump 与云备份。
6. **恢复**：历史记录提供「恢复…」，打开 C-02 的「执行 SQL 文件」对话框并预填文件路径；不自动执行。
7. **安全**：
   - `backup-jobs.json` 不含密码、SSH 口令、私钥口令；只存 connectionId；
   - 输出目录用户选择，禁止 `..` 穿越；文件名模板清理路径分隔符；
   - 只读连接可备份（导出是读操作）。

### 验收标准

- [ ] 新建每日 02:00 的计划，保存后列表显示下次运行时间；重启应用后计划仍在。
- [ ] 点「立即运行」，生成 `.sql`，历史记录状态成功、文件路径、大小、行数 / 表数可见；文件可用 C-02 还原。
- [ ] 应用保持运行并把计划时间改到 1 分钟后，到点自动执行；执行期间列表显示运行中，完成后更新下次运行。
- [ ] 应用在计划时间处于关闭状态，启动后提示「已错过 N 次」，不自动执行；选择「立即补跑」后执行，选择「跳过」后只更新下次运行。
- [ ] 保留 3 份：连续运行 4 次后，最旧的一份被删除，只剩 3 份；输出目录中非本计划生成的文件不被删除。
- [ ] 取消运行后临时文件被删除，历史记录状态为「已取消」，目标目录无半截文件。
- [ ] 连接失效 / 密码未保存时运行失败，历史记录原始错误；计划仍保留并标记连接不可用，不崩溃。
- [ ] 只读连接可创建并运行计划；备份过程不写源库。
- [ ] `backup-jobs.json` 中无 password / sshPassword / keyPassphrase 字段；文件名模板含 `../` 时被拒绝或清理。
- [ ] 两个计划同时到期时串行执行，不并发写同一目录；历史记录按计划区分。
- [ ] 历史「恢复…」打开 C-02 对话框并预填路径，不自动执行。
- [ ] 关闭应用后不会出现后台进程；界面横幅明确说明限制。

### 涉及文件与命令

**前端**

- 新增 `src/components/BackupSettingsDialog.vue`：计划列表、历史、立即运行、恢复入口、限制横幅。
- 新增 `src/components/BackupJobEditor.vue`：连接 / 范围 / 内容 / 目录 / 频率 / 保留 / 模板。
- 修改 `src/App.vue`、`src/components/AppMenuBar.vue`、`src/lib/app-actions.ts`：入口与进度监听。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：计划 / 历史 / 运行状态类型。
- 修改 `src/styles.css`：备份设置样式。

**后端**

- 新增 `src-tauri/src/commands/backup.rs`：计划 CRUD、立即运行、取消、历史、错过后补跑查询。
- 新增 `src-tauri/src/backup_store.rs`（或在 `storage.rs` 中扩展）：`backup-jobs.json`、`backup-history.json` 原子读写、版本号、损坏回退。
- 新增 `src-tauri/src/backup_scheduler.rs`：启动加载、30 秒 tick、到期判断、串行队列、事件发送。
- 修改 `src-tauri/src/lib.rs`：`setup` 中启动 scheduler；注册命令。
- 修改 `src-tauri/src/commands/export.rs`：抽出 `export_database_to_path(...)` 共享函数，供命令与调度器复用；保留原有进度 / 取消。
- 修改 `src-tauri/src/models.rs`：`BackupJob`、`BackupSchedule`、`BackupRun`、`BackupProgress`、`BackupSummary`。
- 复用 `chrono` 计算下次运行；不需要新依赖。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn list_backup_jobs(app: AppHandle) -> Result<Vec<BackupJob>, String>;

  #[tauri::command]
  pub async fn save_backup_job(app: AppHandle, job: BackupJob) -> Result<BackupJob, String>;

  #[tauri::command]
  pub async fn delete_backup_job(app: AppHandle, id: String) -> Result<(), String>;

  #[tauri::command]
  pub async fn run_backup_job(app: AppHandle, state: State<'_, AppState>, id: String, operation_id: String) -> Result<BackupSummary, String>;

  #[tauri::command]
  pub async fn cancel_backup(state: State<'_, AppState>, operation_id: String) -> Result<(), String>;

  #[tauri::command]
  pub async fn list_backup_runs(app: AppHandle) -> Result<Vec<BackupRun>, String>;
  ```
- 进度事件：`backup:progress`。

### 风险与限制

- **不是系统调度器**：应用退出 / 休眠 / 崩溃都不会执行；界面必须反复说明，不能只写在设置页角落。
- **错过的计划**：不能自动补跑所有历史次数，避免启动瞬间打满磁盘；只提示一次，用户选择补跑一次。
- **文件保留**：只删除自己历史中记录且路径在输出目录内的文件；符号链接 / 路径穿越必须防护。
- **连接凭据**：计划只存 connectionId；密码未保存时运行失败，提示用户先保存密码或手动运行。
- **大数据**：逻辑备份可能很慢、占磁盘；第一版不提供 gzip，用户需自行压缩；超过磁盘时失败并清理。
- **并发**：两个计划写同一目录时用不同文件名（含 timestamp / 计划名）；仍串行执行，避免 IO 竞争。
- **时间变更**：系统时区 / 夏令时变化可能造成重复或跳过；按本地时间计算，记录实际执行时间，不保证严格间隔。
- **历史**：最多 200 条；清理历史不删除备份文件（除非用户手动删除计划并选择清理）。

### 参考

- **DBX（Apache-2.0）**：`crates/dbx-core/src/scheduled_backup/models.rs`、`engine.rs`、`service.rs`、`store.rs`；`apps/desktop/src/components/backup/ScheduledDatabaseBackupSettings.vue`、`BackupTableSelector.vue`、`DatabaseBackupConfigFields.vue`；`apps/desktop/src/lib/backup/scheduledDatabaseBackup.ts`、`backgroundDatabaseBackup.ts`；文档 `docs/content/docs/database-backup.cn.mdx`（频率、保留、表范围、立即运行、应用必须运行）。参考其计划字段与保留策略；本项不实现 gzip / `.nb3`、不做多连接批量计划。
- **Navicat**：自动运行 / 计划任务（备份）；参考官方手册的 Auto Run / Schedule 章节。本项明确不承诺系统级调度。
- **mydb 现状**：C-01 导出、C-02 还原、`storage.rs` 原子写。

---

## 附录 A：需求与 Backlog 的收口建议

交付本批后，建议按以下方式更新 `docs/BACKLOG.md`：

| 既有项 | 建议状态 | 说明 |
|---|---|---|
| B-05 查询取消与超时 | 已被 C2-06 替代 | 运行中停止、60 秒超时、事务内取消、历史记录 |
| B-10 对象定义查看与编辑 | 已被 C2-03 替代 | 查看 / 编辑 / 重载函数候选均覆盖；MySQL 例程仍只读 |
| B-13 新建 / 删除数据库与模式 | 已被 C2-02 替代 | MySQL 数据库、PostgreSQL 数据库 / 模式；含危险确认与当前上下文回退 |
| B-14 连接配置导入 / 导出 | 已实现，不再立项 | 工作区 `ConnectionTransferDialog` 已支持 mydb JSON、DBeaver JSON、Navicat `.ncx` 导入与 mydb JSON 导出；旧版 DBeaver XML、覆盖策略后续评估 |
| B-16 大字段 / JSON 查看器 | 已被 C2-05 替代 | 值查看器 + 虚拟滚动 + XLSX |
| B-20 EXPLAIN 快捷入口 | 已实现，不再立项 | 工作区已有 `commands/explain.rs`、`explain-plan.ts`、`ExplainPlanViewer.vue`；PM 按 B-20 验收即可 |
| B-33 对象创建补全 | 已实现，不再立项 | 工作区 `CreateObjectDialog` 已支持视图 / 存储过程 / 函数 / 触发器创建；本批只补定义编辑 |
| B-37 用户 / 权限管理与服务器监视 | 部分被 C2-07 替代 | 用户只读 + 概览增强；授权 / 回收不做 |
| B-34 备份与还原 / B-36 报表与计划任务 | 备份计划由 C2-08 覆盖；报表 / 任意任务保留 | C2-08 只做备份，不做报表与系统级调度 |
| B-25 全局命令面板 | 部分被 C2-04 补充 | C2-04 只做当前连接对象 / 字段搜索 |
| B-04 信息面板 | 保留待办 | C2-07 只做服务器概览与用户页，不做连接 / 对象信息面板 |

## 附录 B：实现与验收约定

1. **一次只做一个需求**：C2-01 完成并验收后再开始 C2-02；不要把格式化与数据库 / 模式创建混在一个未验收的提交里。
2. **每个需求独立可回滚**：新增组件 / 命令尽量内聚；不要为了共用提前抽象未验证的接口。
3. **测试**：
   - 纯函数（格式化 / 压缩、计划解析、导入解析、虚拟行计算）必须有单元测试或可重复的手工用例；
   - 真实数据库回归使用 README 环境变量；没有实例时至少完成 SQLite；
   - 前端通过 `npm run check`；Rust 通过 `cargo test`。
4. **错误文案**：包含对象名 / 文件 / 语句序号与数据库原始错误；不要只显示 `error`。
5. **隐私**：不记录密码、凭据、完整 SQL 到日志；用户列表不查散列；导入不解密密码。
6. **版本与发布**：本批不提升版本号；发布等待用户明确说「更新app」或「合并代码」。
7. **参考与许可**：复制 DBX 代码需在文件头注明 `Source: https://github.com/t8y2/dbx (Apache-2.0)` 与原路径；`sql-formatter`（MIT）与 `rust_xlsxwriter`（MIT OR Apache-2.0）需在 README / 许可证说明中登记；不复制 Navicat / DBeaver 任何资源。

## 附录 C：本批参考资料

- DBX 仓库（Apache-2.0）：`https://github.com/t8y2/dbx`
  - 格式化：`apps/desktop/src/lib/sql/sqlFormatter.ts`、`sqlFormatterConfig.ts`、`layout.ts`；`apps/desktop/src/components/editor/SqlFormatterSettingsPanel.vue`
  - 执行计划：`apps/desktop/src/lib/diagram/explainPlan.ts`；`apps/desktop/src/components/explain/`
  - 对象源码：`apps/desktop/src/components/objects/ObjectSourceDialog.vue`；`apps/desktop/src/lib/table/objectSource*.ts`
  - 数据库搜索：`apps/desktop/src/components/search/DatabaseSearchDialog.vue`；`apps/desktop/src/lib/database/databaseSearch.ts`
  - 表数据：`apps/desktop/src/components/grid/DataGridCellDetailDialog.vue`；`apps/desktop/src/components/common/VirtualScrollArea.vue`；`apps/desktop/src/lib/export/xlsxExport.ts`；`src-tauri/src/commands/xlsx_export.rs`
  - 用户 / 服务器：`apps/desktop/src/components/admin/DatabaseUserAdmin.vue`、`MySqlDashboard.vue`、`PostgresDashboard.vue`；`apps/desktop/src/lib/database/databaseUserAdmin.ts`、`mysqlServerStatus.ts`
  - 备份：`crates/dbx-core/src/scheduled_backup/`；`apps/desktop/src/components/backup/`；`apps/desktop/src/lib/backup/`
  - 查询取消：`crates/dbx-core/src/query/query_cancel.rs`；`src-tauri/src/commands/query_cancel.rs`
  - 已实现参考（本批不重复）：连接导入 `apps/desktop/src/lib/imports/dbeaverImport.ts`、`navicatImport.ts`；对象创建 `CreateObjectDialog.vue` 的交互由工作区实现
  - 文档：`docs/content/docs/sql-formatter.cn.mdx`、`query-editor.cn.mdx`、`schema-browser.cn.mdx`、`object-browser.cn.mdx`、`data-grid.cn.mdx`、`database-backup.cn.mdx`、`connection-import.cn.mdx`
- Navicat 17 官方手册（功能参考，不使用其代码 / 资源）：`https://www.navicat.com/manual/pdf_manual/en/navicat_17/mac_manual/navicat_mac_en.pdf`
- Navicat 连接迁移 KB（已实现的导入参考）：`https://help.navicat.com/hc/en-us/articles/218281627-How-to-get-the-connection-information-when-migrating-Navicat-to-new-computer`
- DBeaver 配置文档（已实现的导入参考）：`https://dbeaver.com/docs/dbeaver/Configuration-files-in-DBeaver/`、`https://dbeaver.com/docs/dbeaver/Admin-Manage-Connections/`
- 第三方库：`sql-formatter`（MIT，`https://github.com/sql-formatter-org/sql-formatter`）、`rust_xlsxwriter`（MIT OR Apache-2.0，`https://github.com/jmcnamara/rust_xlsxwriter`）
- mydb 现状：`src/App.vue::explainQuery`、`src/lib/sql-statements.ts`、`src/components/SqlEditor.vue::formatSql`、`src/components/CreateObjectDialog.vue`、`src/components/ConnectionTransferDialog.vue`、`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`、`src-tauri/src/commands/query.rs`、`src-tauri/src/commands/server.rs`

---

> 本文件只定义需求与验收，不包含实现代码。实现完成后先更新 `CHANGELOG.md` 的 `[Unreleased]`，再等待用户指令决定是否提交 / 打包 / 更新版本。