# mydb 产品需求（2026-11-25 批次）

> 维护人：产品（PM）
> 基线版本：v0.11.0（2026-11-24）
> 文档状态：已实现（2026-11-25）；C-05 逐列映射 / 跨驱动、C-06 目标结构快照、C-07 用户权限仍待后续
> 适用范围：桌面客户端（Tauri 2 + Rust + Vue 3），驱动范围 SQLite / MySQL（含 MariaDB）/ PostgreSQL；Redis 仅在本批明确说明处涉及
> 关联文档：`docs/BACKLOG.md`、`docs/PM-REQUIREMENTS-2026-11-24.md`、`docs/PM-REQUIREMENTS-2026-11-24-batch2.md`、`docs/ARCHITECTURE.md`、`CHANGELOG.md`

---

## 0. 本批结论

### 0.1 一句话目标

让 mydb 从「能看能改单张表」升级为「能把一个库完整带走、能在目标端安全落地变更、能批量编辑表数据」的数据库工作台，同时补齐 Navicat / DBX 中最常用、最容易形成完整工作流的三类能力：**逻辑导出与还原、迁移与同步落地、表数据与表结构编辑**。

### 0.2 需求清单（按实施优先级排序）

| 顺序 | 编号 | 标题 | 优先级 | 预估 | 依赖 | 与既有 Backlog 的关系 |
|---|---|---|---|---|---|---|
| 1 | C-01 | 数据库 / 模式级 SQL 导出（结构 + 数据） | P0 | 1 个开发会话 | 无 | 合并并替代 B-21 与 B-34 的导出部分 |
| 2 | C-02 | SQL 脚本执行与还原 | P0 | 1 个开发会话 | C-01 的产物用于回归 | 吸收 B-07，承接 B-34 的还原部分 |
| 3 | C-03 | 表数据批量编辑与浏览增强 | P1 | 1–1.5 个开发会话 | 无 | 细化并替代 B-08 |
| 4 | C-04 | 表设计器补全：索引 / 外键 / 注释 / 预览 | P1 | 1–1.5 个开发会话 | 无 | 扩展既有建表设计器，无对应 B 项 |
| 5 | C-05 | 数据传输向导：多表迁移与建表 | P1 | 1.5–2 个开发会话 | C-01 的结构导出、现有结构/数据同步 | 细化并替代 B-35 |
| 6 | C-06 | 结构同步应用到目标（受控自动执行） | P1 | 0.5–1 个开发会话 | C-01（可选快照）、现有结构同步 | 新增；基线明确「结构同步不自动执行」 |
| 7 | C-07 | 服务器监视与进程列表 | P2 | 1–1.5 个开发会话 | 无 | 承接 B-37 的进程 / 服务器部分，不含用户权限 |
| 8 | C-08 | ER 图导出、布局持久化与定位 | P2 | 1 个开发会话 | 无 | 细化并替代 B-17 |

> 「1 个开发会话」指 Claude 在一次连续实现中完成编码、`npm run check`、`cargo test` 与文档中列出的可自动化验收项；真实数据库手工验收可单独安排。

### 0.3 推荐实施顺序与理由

1. **C-01 先做**：它是本批唯一能独立产生「可交付物」（一个可执行 `.sql` 文件）的需求，也是 C-02、C-05、C-06 的共同基础；实现边界清楚，最容易在一个会话内验收。
2. **C-02 第二个**：与 C-01 组成「导出 → 还原」闭环，能立刻用 C-01 的产物做真实回归，避免导出功能只写文件却无法证明可用。
3. **C-03 第三个**：日常使用频率最高，且完全在前端可控范围内；做完后表数据页才具备「先预览、再提交」的安全感。
4. **C-04 → C-05 → C-06**：表设计器先补齐对象创建的基础元数据，数据传输与结构同步应用都依赖这些 DDL 生成规则。
5. **C-07 / C-08 最后**：独立性强，可在主线完成后按用户反馈调整范围。

### 0.4 本批明确不做（附评估结论）

| 能力 | 结论 | 理由 |
|---|---|---|
| 用户 / 角色 / 权限查看与授权（B-37 后半） | 本批不做，下一批评估 | 只读用户列表对轻量客户端价值有限；授权 / 回收在 MySQL 8 与 PostgreSQL 的权限模型差异大、误操作风险高。先让 C-07 的服务器面板落地，下一批在同一面板增加「用户」页，且第一版只读。 |
| EXPLAIN / 执行计划可视化（B-20） | 本批不做，下一批首个 P1 | 现有 `sql_guard` 已允许只读连接执行 `EXPLAIN`；入口与结果展示可以独立成项。本批先保证「数据搬得走、改得对」。 |
| SQL 格式化 / 压缩（PM-06 已明确排除） | 本批不做 | 需要方言差异与格式化算法；PM-06 已声明不在编辑器菜单范围内，避免把菜单项做成空壳。 |
| 查询构建器 / 模型设计 / 数据生成 | 不做，长期非目标 | `BACKLOG.md` 已将其列为与「轻量客户端」定位冲突的 IDE 级能力，需要单独立项评估。 |
| 定时备份 / 系统级计划任务（B-36） | 本批不做，保留研究 | 见 0.4.1。 |
| gzip 备份、Navicat `.nb3` 兼容、调用 `mysqldump` / `pg_dump` | 不做 | 第一版只用纯 Rust 逻辑导出；外部工具涉及安装路径、版本差异、进程管理与跨平台打包，不适合本阶段。 |
| 跨驱动数据传输 | 不做 | 类型映射、字符集、默认值、索引语义差异大；C-05 只做同驱动。 |
| 查询结果网格直接编辑 | 不做 | 结果集缺少可靠表名 / 主键 / 可更新性判定；表数据页已覆盖编辑场景。 |
| Redis 备份 / 键导入导出 | 不做 | Redis 的数据模型与 SQL dump 不同，需要单独设计 RDB / JSON 方案。 |
| 表设计器「编辑已有表」的全量重建 | 不做 | C-04 只补外键与注释；字段类型 / 可空 / 默认值已有结构页 ALTER，完整表重建风险高，另立需求。 |

#### 0.4.1 关于备份 / 还原 / 定时任务的评估

- **手动逻辑备份与还原适合本阶段**：由 C-01（导出）+ C-02（还原）覆盖，不引入常驻进程，符合「轻量、启动快、无后台服务」定位。
- **定时备份不适合本阶段**：Tauri 应用关闭后没有任何调度能力；若为保证「到点执行」而引入系统服务 / LaunchAgent，会显著增加安装、签名、权限与跨平台成本，也与当前定位冲突。DBX 的定时备份同样依赖桌面端运行。
- 若用户后续明确要求，建议的 MVP 是「应用内计划」：保存连接、范围、频率与保留份数；应用运行时到点执行；应用未运行时在下次启动提示「已错过 N 次」并允许补跑，界面明确写「不会在应用关闭时执行」。在需求评审通过前不排期。

### 0.5 通用约束（适用于本批全部需求）

- **只写需求与验收，不写实现代码**；实现由 Claude 按单个需求独立提交，完成一个再开始下一个。
- **不提升版本号**：按项目约定，普通改动只写入 `CHANGELOG.md` 的 `[Unreleased]`；只有用户明确说「更新版本」才提升版本号。
- **不新增第三方 UI / 图标库**；优先复用现有 `Icon.vue`、`styles.css` 的设计令牌与现有对话框样式。
- **不复制 Navicat / DBX 的代码、图标、截图、品牌资源**。DBX 为 Apache-2.0；如实现时复制了 DBX 代码片段，必须在文件头注明来源仓库、文件路径与 Apache-2.0，并同步更新 `README.md` 的「参考与许可」；本文件中的 DBX 路径仅用于行为与数据流参考。
- **只读连接**：所有写操作必须在后端命令层拒绝；前端隐藏 / 禁用入口只能作为提示，不能作为唯一防线。只读判断统一使用现有 `ensure_writable` / `connection_read_only`。
- **原子写文件**：所有导出 / 备份文件必须先写同目录临时文件，成功后 `rename`；失败或取消删除临时文件，不留下半截文件。复用 `commands/data.rs::write_text_file` 的原子写思路。
- **不阻塞主线程**：后端命令使用 `async` 与流式读写；前端在执行期间保持可交互，进度通过 Tauri 事件或轮询更新。
- **中文界面**：新增文案全部中文；错误信息优先保留数据库原始错误，并补充可读上下文（对象名、语句序号、行号）。
- **验收环境**：SQLite 使用 `examples/demo.db`；MySQL / PostgreSQL 使用真实实例，环境变量沿用 README 中的 `MYDB_TEST_MYSQL_*` / `MYDB_TEST_PG_*`。没有真实实例时，至少完成 SQLite 全链路 + Rust 单元测试，并在交付说明中标注未覆盖项。
- **完成定义（DoD）**：代码 + `npm run check` 通过 + `cargo test` 通过 + 本需求验收清单逐项勾选 + `CHANGELOG.md` 写入 `[Unreleased]` + 相关 README / 已知限制更新 + 对应 BACKLOG 项状态更新（被替代项标记「已被 C-xx 替代」）。

### 0.6 实现状态（2026-11-25）

- C-01 数据库 / 模式级 SQL 导出：已完成（二进制十六进制保真、进度 / 取消、失败汇总）。
- C-02 SQL 脚本执行与还原：已完成（分句器、预览、只读预检、事务 / 失败策略、进度 / 取消、单条历史）。
- C-03 表数据待提交 / 批量编辑 / 筛选排序分页：已完成。
- C-04 表设计器索引 / 外键 / 注释 / 预览与结构页外键：已完成；SQLite 外键修改仍需重建表，按钮禁用。
- C-05 数据传输向导：已完成同驱动多表追加 / upsert / 清空写入、目标缺表建表、进度 / 取消；逐列映射与跨驱动未实现。
- C-06 结构同步应用到目标：已完成重新对比、危险确认、事务 / 逐条策略、进度 / 取消；目标结构快照未实现，已在界面说明。
- C-07 服务器监视与进程列表：已完成 MySQL / PostgreSQL 概览、进程、终止会话；用户 / 权限未实现。
- C-08 ER 图导出 / 布局持久化 / 定位：已完成（SVG 导出，localStorage 布局）。

---

## C-01 数据库 / 模式级 SQL 导出（结构 + 数据）

- **优先级**：P0
- **预估**：1 个开发会话
- **依赖**：无
- **替代关系**：合并 `B-21`（数据库 / 模式级结构导出）与 `B-34`（备份与还原）的「导出」部分；交付后 `B-21` 关闭，`B-34` 仅保留「还原」并由 C-02 承接。

### 用户问题 / 场景

现在只能对单张表 / 视图导出 CSV / JSON / SQL INSERT / DDL。要把一个库交给同事、迁移到测试环境、变更前留底，只能逐张表导出再手工拼接：顺序容易错、外键和视图依赖会失败、二进制字段会变成 `<BLOB n bytes>` 占位文本、没有进度也无法取消。用户需要的是「选一个库 / 模式，得到一个能直接执行的结构 + 数据脚本」。

### 期望行为

1. **入口**：
   - MySQL 数据库节点右键「导出 SQL…」；
   - PostgreSQL 模式节点右键「导出 SQL…」；
   - SQLite 连接根节点右键「导出 SQL…」；
   - 「文件」菜单增加「导出数据库 SQL…」，作用于当前连接与当前库 / 模式；
   - 未打开连接时入口禁用，并显示禁用原因。
2. **范围选择**：对话框固定为「当前库 / 模式」，不提供「导出所有数据库」。左侧按类型分组列出对象并显示数量：表、视图、物化视图、序列、存储过程、函数、触发器、事件（按驱动能力显示）。默认全选；支持搜索、全选 / 全不选、按类型勾选；至少选择 1 个对象才能导出。
3. **内容选项**：
   - 结构（默认开）：表、视图、序列、例程、触发器、事件的 DDL；
   - 数据（默认开）：仅对「表」生成 `INSERT`；视图 / 物化视图不生成数据；勾选数据时，未勾选的表只导出结构；
   - 包含 `DROP ... IF EXISTS`（默认关）：在对应 `CREATE` 前生成删除语句，不生成 `CASCADE`；
   - MySQL「生成 `CREATE DATABASE IF NOT EXISTS` 与 `USE`」（默认关）：默认脚本在「执行时当前选中的库」中运行；
   - 移除 MySQL `DEFINER`（默认开）：避免因账号不存在导致还原失败；关闭时保留原始定义并在文件头提示；
   - 每条 `INSERT` 行数（默认 100，范围 1–500）：控制多行 `INSERT` 的批量大小，同时受单条语句字节上限约束；
   - 数据写入使用事务（默认开）：在数据段前后生成 `START TRANSACTION` / `COMMIT`（SQLite 用 `BEGIN` / `COMMIT`）。
4. **文件结构与顺序**：
   1. 文件头注释：mydb 版本、导出时间、源连接名、驱动、库 / 模式、选项摘要、注意事项（逻辑备份，不替代原生 dump）；
   2. 会话设置：MySQL `SET NAMES utf8mb4; SET FOREIGN_KEY_CHECKS=0;`，PostgreSQL `SET client_encoding='UTF8'; SET standard_conforming_strings=on;`，SQLite `PRAGMA foreign_keys=OFF;`；
   3. 序列（如支持）→ 表结构 → 表数据 → 非主键索引 / 外键约束 → 视图 / 物化视图 → 存储过程 / 函数 → 触发器 / 事件；
   4. 会话清理：恢复外键检查、`COMMIT`（若仍有未提交事务）。
   - 表按名称排序。现有 PostgreSQL `build_table_ddl` 会把外键内联进建表语句；导出路径必须把它拆成「表 DDL（不含外键）→ 所有表创建完成 → `ALTER TABLE ... ADD CONSTRAINT`」，避免循环 / 前向引用失败。MySQL 的 `SHOW CREATE TABLE` 会把索引与外键内联，无法拆分，需依赖 `SET FOREIGN_KEY_CHECKS=0` 与建表顺序保证可执行。PostgreSQL / SQLite 的非主键索引在数据写入后创建（顺序与性能兼顾）。
   - SQLite 需显式导出 `CREATE INDEX` 与 `CREATE TRIGGER`（`sqlite_master.sql`），不能只导出 `CREATE TABLE`。
5. **数据保真**：
   - 字符串按方言转义单引号与反斜杠；标识符按方言引用（MySQL 反引号，PostgreSQL / SQLite 双引号）；
   - `NULL` 写成 `NULL`，空字符串写成 `''`，布尔、数字、日期时间按目标驱动可执行的格式输出；
   - **二进制必须保真**：MySQL `BLOB/BINARY/VARBINARY/GEOMETRY`、PostgreSQL `BYTEA`、SQLite `BLOB` 必须输出为目标方言可执行的十六进制字面量（如 MySQL `0x...`、PostgreSQL `'\x...'::bytea` 或 `decode('...','hex')`、SQLite `X'...'`），不能用 `<BLOB n bytes>` 占位。若驱动暂时无法取得原始字节，必须把该表标记为失败并列入汇总，不能静默写入占位文本。
   - MySQL `BIT`、JSON、ENUM、SET 按可执行字面量处理；PostgreSQL JSON / JSONB 输出合法 JSON 字符串并显式转换；数组类型输出 `ARRAY[...]` 或可执行文本（第一版允许以文本形式写入并标注）。
6. **进度与取消**：
   - 导出期间显示：当前对象 / 对象总数、已写行数、已写字节数、耗时；
   - 至少每 500ms 或每 2000 行更新一次进度，不阻塞 UI；
   - 「取消」在 2 秒内生效，停止读取并删除临时文件；
   - 完成后显示汇总：文件路径、对象数、表数、行数、字节数、警告 / 失败对象列表；提供「复制路径」。
7. **错误处理**：
   - 单个对象 DDL 读取失败（无权限、对象已删除）时，记录到失败清单并继续导出其他对象；
   - 数据读取失败时，该表停止写入，已写入的部分保留在文件中但汇总标记「不完整」；用户可选择重新导出；
   - 所有对象都失败时返回错误，不留下文件；
   - 磁盘写入失败（空间不足、无权限）立即停止并清理临时文件。
8. **只读与历史**：只读连接允许导出；导出不写入 SQL 执行历史，不修改源库。
9. **文件编码**：UTF-8，无 BOM；中文表名 / 字段名必须正确引用；换行统一 `\n`。

### 验收标准

- [ ] SQLite：对 `examples/demo.db` 导出全部对象与数据，生成一个 `.sql`；用 `sqlite3 new.db < dump.sql` 在空库执行成功，表 / 视图 / 索引 / 触发器数量与源库一致，逐表行数一致。
- [ ] MySQL：对一个含主键、复合唯一索引、外键、视图、触发器、存储过程的测试库导出；在空库执行成功，表 / 视图 / 索引 / 外键 / 触发器 / 存储过程存在，逐表行数一致。
- [ ] PostgreSQL：对一个含 identity、复合主键、普通索引、外键、视图、函数的测试模式导出；在新库同名模式中执行成功，非主键索引与外键存在，函数可调用，逐表行数一致。
- [ ] 二进制保真：三端各建一张含 1 KB 二进制字段（覆盖 0x00–0xFF）的表，导出后还原，`length` 与逐字节比较一致；文件中不出现 `<BLOB` 占位文本。
- [ ] 仅结构：关闭「数据」后导出，文件中没有 `INSERT INTO`，脚本可在空库执行且对象齐全。
- [ ] 仅数据：关闭「结构」后导出，文件中只有 `INSERT`，在已存在同结构的库中执行成功。
- [ ] `DROP` 选项：默认关闭时文件不含 `DROP`；开启后包含 `DROP ... IF EXISTS`，且不含 `CASCADE`，在空库与已有对象的库中均可执行。
- [ ] 取消：导出 10 万行表时点击取消，2 秒内停止，目标文件不存在，同目录无残留临时文件。
- [ ] 大表：10 万行表的导出过程中内存占用不随行数线性增长（不出现明显卡顿 / 崩溃），进度持续更新，窗口其他标签可操作。
- [ ] 权限失败：用无视图权限的账号导出，视图被列入失败清单，其余对象正常写入，汇总可见失败原因。
- [ ] 只读连接：可完成导出；导出前后源库行数与校验值不变；`history.json` 无新增记录。
- [ ] 中文与特殊字符：表名 / 字段名含中文、空格、引号、反引号时，导出脚本可执行；字符串值含单引号、反斜杠、换行时还原后完全一致。
- [ ] 空范围：未选择任何对象时提示并禁止导出，不生成空文件。

### 涉及文件与命令

**前端**

- 新增 `src/components/DatabaseExportDialog.vue`：范围树、选项、进度、取消、汇总。
- 修改 `src/components/ObjectBrowser.vue`：数据库 / 模式节点右键菜单增加「导出 SQL…」，通过 emit 交给 `App.vue`。
- 修改 `src/components/ObjectCategoryGroups.vue`：SQLite 连接根节点的对象分组入口（如现有结构允许）增加同一动作。
- 修改 `src/App.vue`：打开对话框、传递当前连接 / 库 / 模式、监听进度事件、导出完成后的提示。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：新增命令与类型。
- 修改 `src/lib/app-actions.ts`：命令面板中增加「导出数据库 SQL…」（可选，但需与菜单动作一致）。

**后端**

- 新增 `src-tauri/src/commands/export.rs`（或在 `commands/data.rs` 中扩展）：`export_database`、进度事件、取消标志复用 `AppState.export_cancels`。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：模块导出与命令注册。
- 修改 `src-tauri/src/models.rs`：`DatabaseExportOptions`、`DatabaseExportSummary`、`ExportProgress`、`ExportFailure`。
- 修改 `src-tauri/src/db/driver.rs`：新增「可导出 DDL 部件」与「带原始二进制的流式行」能力，例如：
  - `table_ddl_parts(&self, table) -> Result<TableDdlParts, String>`（表 DDL、索引语句、外键语句、触发器语句）；
  - `export_rows_raw(&self, table, limit, sink) -> Result<u64, String>`（`ExportCell = Null | Text(String) | Number(...) | Binary(Vec<u8>)`）。
  - 默认实现可回退到现有 `export_rows` + 占位符，但必须返回「二进制不可用」标记，供上层决定是否失败。
- 修改 `src-tauri/src/db/mysql.rs`、`src-tauri/src/db/postgres.rs`、`src-tauri/src/db/sqlite.rs`：实现上述能力；复用现有 `object_ddl`、`table_structure`、`create_index_sql`（`commands/sync.rs`，需提升可见性）、`sql_literal`（`commands/data.rs` / `commands/data_sync.rs`，建议抽到共享模块）。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn export_database(
      app: AppHandle,
      state: State<'_, AppState>,
      connection_id: String,
      path: String,
      options: DatabaseExportOptions,
      operation_id: Option<String>,
  ) -> Result<DatabaseExportSummary, String>;

  #[tauri::command]
  pub async fn cancel_export(state: State<'_, AppState>, operation_id: String) -> Result<(), String>;
  ```
- 进度事件：`database-export:progress`，载荷包含 `operationId`、`phase`、`objectName`、`objectIndex`、`objectTotal`、`rows`、`bytes`、`message`。

**驱动 / 数据**

- 表结构 DDL 来源：MySQL `SHOW CREATE TABLE`；PostgreSQL 现有 `build_table_ddl` + 额外 `CREATE INDEX` / `ALTER TABLE ADD CONSTRAINT`；SQLite `sqlite_master` + 显式索引 / 触发器。
- 行数据来源：新增 raw 流式读取，避免 `Value::String("<BLOB ...>")` 丢失字节。

### 风险与限制

- **DDL 依赖顺序**：视图依赖视图、函数依赖类型、循环外键都可能无法一次排序成功。第一版按「表 → 索引 / 外键 → 视图 → 例程 → 触发器」排序，失败对象汇总提示；不承诺 100% 自动排序。
- **MySQL DDL 隐式提交**：`CREATE TABLE` 等会隐式提交，不能把整个文件包在一个事务里。第一版只对数据段使用事务，文件头明确提示。
- **PostgreSQL 模式限定**：现有 PG DDL 生成带模式名的 `CREATE TABLE "schema"."table"`。导出到另一个模式时需用户替换或使用同名模式；第一版不提供「去掉模式名」选项，必须在文件头写明。
- **序列当前值**：PostgreSQL 独立序列的 `last_value` 读取需要额外权限，第一版可只导出 `CREATE SEQUENCE` 结构，不保证当前值；若实现读取，需在验收中单列。
- **生成列 / identity / 默认表达式**：DDL 由数据库返回时保真；自行拼装的 PG DDL 可能丢失部分属性，需以 `pg_get_*` 或结构元数据为准。
- **二进制**：需要在三个驱动中打通原始字节读取；这是本项最大技术风险。若某个驱动无法在本次完成，必须显式失败而不是写占位符，并在交付说明中列出。
- **超大对象**：单表数十 GB 时文件可能耗尽磁盘；第一版不提供预估大小，只在进度中显示已写字节。
- **安全**：导出文件可能含敏感数据；界面不自动打开文件，只显示路径；不写入日志。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/export/DatabaseExportDialog.vue`、`ExportExecutionMode.vue`、`ExportProgressDialog.vue`、`SqlInsertModeDialog.vue`；`apps/desktop/src/lib/export/databaseExport.ts`、`databaseExportOptions.ts`、`databaseExportSelection.ts`；`crates/dbx-core/src/data/database_export.rs`、`export_runtime.rs`、`table_export.rs`、`script_generator.rs`；文档 `docs/content/docs/database-export.cn.mdx`。参考其「结构 / 数据 / 对象范围 / 批量 INSERT / 进度与取消 / 导入辅助语句」的行为划分，不复制代码。
- **Navicat**：转储 SQL 文件（Dump SQL File）、备份 / 还原中的「结构 + 数据」导出；参考官方手册 `https://www.navicat.com/manual/pdf_manual/en/navicat_17/mac_manual/navicat_mac_en.pdf` 的 Dump SQL File 章节。

---

## C-02 SQL 脚本执行与还原

- **优先级**：P0
- **预估**：1 个开发会话
- **依赖**：C-01（用于导出 → 还原闭环回归）
- **替代关系**：吸收 `B-07`（多语句脚本执行与逐条结果），承接 `B-34` 的「还原」部分。

### 用户问题 / 场景

拿到一个 `.sql` 文件（同事导出、C-01 产物、DBA 给的迁移脚本）后，mydb 只能打开到编辑器再手工全选执行；没有分句、没有进度、失败后不知道第几条出错、哪些已生效；大文件还会卡住编辑器。用户需要一个「执行 SQL 文件」的受控入口：先看摘要，再决定事务与失败策略，执行中能看到进度并取消，结束后拿到逐条结果。

### 期望行为

1. **入口**：
   - 「文件」菜单「执行 SQL 文件…」（`⌘/Ctrl+Shift+E` 或与现有菜单不冲突的键位）；
   - 查询编辑器检测到多条语句时，运行按钮旁显示「脚本模式（N 条）」；点击后进入同一执行对话框（预填当前编辑器内容）；
   - 对象树 / 数据库节点右键「执行 SQL 文件…」（可选，和文件菜单共用对话框）。
2. **文件读取与预览**：
   - 从原生对话框选择 `.sql`，后端直接按路径读取，不把整文件传到前端；
   - 上限 100 MB；超过时拒绝并提示大小；
   - 仅接受 UTF-8（允许 UTF-8 BOM，读取时去除）；检测到 NUL 或非 UTF-8 时拒绝，不尝试猜测编码；
   - 预览显示：文件大小、语句数、前 20 条语句摘要（带行号）、检测到的内容类型（DDL / DML / 事务控制 / `DELIMITER` / 不支持的命令）、目标连接与当前库 / 模式；
   - 空文件或只有注释时拒绝执行。
3. **分句规则**（后端纯 Rust 模块，带单元测试）：
   - 按 `;` 分句，正确跳过单引号、双引号、反引号、方括号标识符；
   - 跳过 `--`、`#`（MySQL）、`/* ... */` 注释；
   - PostgreSQL dollar-quoted 块 `$tag$ ... $tag$` 不切分；
   - MySQL `DELIMITER $$` / `DELIMITER ;` 指令行生效，指令本身不发送给数据库；未闭合时报错并给出行号；
   - SQLite `CREATE TRIGGER ... BEGIN ... END;` 与 MySQL 触发器体按「一个完整语句」处理；
   - PostgreSQL `DO $$ ... $$;`、函数体按 dollar quote 规则处理；
   - 遇到 `\c`、`\i`、`\copy` 等 psql 元命令、或无法确定的结构时拒绝执行并给出行号，不猜测。
4. **执行选项**：
   - 失败策略：遇错停止（默认）/ 出错继续；
   - 事务：整个脚本一个事务（默认开）/ 不使用事务；
   - 事务开启时的提示：MySQL 的 DDL 会隐式提交，无法真正回滚；PostgreSQL / SQLite 的 DDL 在事务内可回滚；界面需按驱动显示不同提示，不能笼统承诺「全部回滚」；
   - 每条语句执行后不自动提交（由事务或驱动默认行为决定）。
5. **只读连接**：
   - 执行前对全部语句做只读预检（复用 `db::sql_guard::ensure_read_only_sql`，逐条）；
   - 只要存在一条写语句，就在预览页列出前若干条违规语句并禁止开始，不执行任何语句；
   - 纯 SELECT 脚本可执行，并逐条返回结果摘要。
6. **执行与进度**：
   - 使用同一条专用连接顺序执行，避免连接池切换导致 `USE` / `SET` / 临时表失效；执行结束后释放；
   - 每执行一条发送一次进度事件（可节流），显示：当前序号 / 总数、语句摘要、成功 / 失败、影响行数、耗时、累计成功 / 失败 / 跳过；
   - 至少每 500ms 更新一次界面；执行期间对话框不可关闭（只能取消），避免「后台继续跑但用户不知道」；
   - 取消：当前语句执行完后停止；剩余标记「未执行」；事务开启时回滚；事务关闭时明确报告「已执行 N 条，已生效，无法自动回滚」。
7. **结果与错误**：
   - 汇总：成功 / 失败 / 跳过条数、总耗时、总影响行数；
   - 失败详情：语句序号、文件行号、SQL 摘要（前 200 字符）、数据库原始错误；可复制；
   - 失败后根据策略：停止时后续标记「未执行」；继续时记录失败并继续执行；
   - 「遇错停止 + 事务」时明确显示「已全部回滚」；「遇错停止 + 无事务」时明确显示「前 N 条已生效」；
   - 事务提交失败时显示明确错误，不报告成功。
8. **历史记录**：一次脚本执行只写入一条执行历史（标题包含文件名与语句数，成功 / 失败与总耗时），不按语句逐条写入，避免刷爆 100 条历史上限。
9. **目标范围**：使用打开对话框时的当前连接与当前库 / 模式；第一版不在对话框内切换目标连接。脚本中的 MySQL `USE` 只在本次专用连接内生效，不改变应用当前库；PostgreSQL `SET search_path` 同理。
10. **兼容 C-01**：C-01 生成的 MySQL 例程 / 触发器使用 `DELIMITER` 包裹时，本项必须能原样执行；SQLite 触发器、PostgreSQL 函数体必须正确分句。

### 验收标准

- [ ] 3 条 `SELECT` 的脚本：执行后逐条显示成功与结果摘要，汇总 3 成功 / 0 失败 / 0 跳过。
- [ ] 第 2 条语法错误、关闭事务、遇错停止：第 1 条成功并已生效，第 2 条失败，第 3 条「未执行」；报告包含语句序号、文件行号与原始错误。
- [ ] 同上但开启事务（PostgreSQL / SQLite）：第 1 条数据变更被回滚，目标库无变化，明确显示「已全部回滚」。
- [ ] MySQL 开启事务且脚本含 `CREATE TABLE`：界面提示「DDL 会隐式提交，无法全部回滚」，执行失败后不谎报回滚。
- [ ] MySQL 脚本包含 `DELIMITER $$` 的存储过程与触发器：各自作为一个语句执行成功，对象出现在对象树；`DELIMITER` 行不发送给数据库。
- [ ] PostgreSQL 脚本包含 `$body$ ... ; ... $body$` 函数与 `DO $$ ... $$;`：分句正确，不误切。
- [ ] SQLite 脚本包含 `CREATE TRIGGER ... BEGIN ...; ...; END;`：作为一个语句执行成功。
- [ ] 只读连接：脚本含 `DELETE` 时，预览页列出违规语句并禁止开始，数据库无任何变化；纯 SELECT 脚本正常执行。
- [ ] 取消：执行 1 万条 INSERT 的脚本，点击取消后当前语句结束即停止；事务开启时目标表无新增行；事务关闭时报告实际已执行条数。
- [ ] 大文件：50 MB / 10 万条语句的脚本可执行，进度持续更新，界面不冻结；超过 100 MB 时拒绝并提示。
- [ ] 编码：UTF-8 BOM 文件正常执行；含非法字节或 NUL 的文件被拒绝且不执行任何语句。
- [ ] 闭环：用 C-01 导出的 SQLite / MySQL dump 在空库执行成功，PostgreSQL dump 在空库 + 已创建同名模式中执行成功；对象与行数与源一致（二进制表同样一致）。
- [ ] 历史：执行一个 100 条语句的脚本后，历史只新增 1 条记录，标题包含文件名与语句数。

### 涉及文件与命令

**前端**

- 新增 `src/components/SqlFileExecuteDialog.vue`：文件选择、预览、选项、进度、逐条结果、取消。
- 修改 `src/components/AppMenuBar.vue` / `src/lib/app-actions.ts` / `src/App.vue`：新增「执行 SQL 文件…」菜单动作。
- 修改 `src/components/QueryWorkspace.vue`、`src/components/SqlEditor.vue`：多条语句时显示「脚本模式（N 条）」入口；不改变单条执行路径。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：命令与进度事件类型。

**后端**

- 新增 `src-tauri/src/db/sql_script.rs`：分句器 + 单元测试（引号、注释、dollar quote、DELIMITER、触发器体、行号映射）。
- 新增 `src-tauri/src/commands/script.rs`：`execute_sql_file`、`cancel_sql_script`、进度事件、只读预检、历史记录。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：模块与命令注册。
- 修改 `src-tauri/src/models.rs`：`ScriptOptions`、`ScriptStatementResult`、`ScriptSummary`、`ScriptProgress`。
- 复用 `src-tauri/src/commands/query.rs::ensure_query_allowed` 的只读判定逻辑；复用 `DatabaseSession` 或在驱动上新增 `execute_script` 专用方法。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn execute_sql_file(
      app: AppHandle,
      state: State<'_, AppState>,
      connection_id: String,
      path: String,
      options: ScriptOptions,
      operation_id: Option<String>,
  ) -> Result<ScriptSummary, String>;

  #[tauri::command]
  pub async fn cancel_sql_script(state: State<'_, AppState>, operation_id: String) -> Result<(), String>;
  ```
- 进度事件：`sql-script:progress`，载荷包含 `operationId`、`index`、`total`、`line`、`preview`、`status`（running / success / failed / skipped）、`rowsAffected`、`elapsedMs`、`error`。

**驱动**

- 无需新增驱动能力即可执行 DDL / DML；若使用事务，需确保三个驱动的 `DatabaseSession` 支持任意语句（现有 `open_session` 已覆盖 SQLite / MySQL / PostgreSQL，需回归验证）。
- MySQL `USE` 行为需在专用连接上验证，不能污染缓存驱动。

### 风险与限制

- **分句器永远无法覆盖所有方言扩展**：遇到不确定结构必须拒绝并给出行号，不能猜测执行。第一版明确不支持 psql 元命令、MySQL `SOURCE`、`\G`。
- **MySQL DDL 隐式提交**：事务包裹无法回滚已执行的 DDL；界面必须按驱动显示真实语义。
- **触发器体**：SQLite 无 `DELIMITER`，只能识别 `CREATE TRIGGER ... BEGIN ... END`；嵌套 `BEGIN` 的复杂体可能误判，需在测试中覆盖常见形态。
- **超大文件**：100 MB 上限是保护；读取与分句需要流式或分块，避免一次性占用双倍内存。
- **取消延迟**：单条语句执行期间无法中断（数据库客户端层面），取消只保证「当前语句结束后停止」；界面需说明。
- **部分生效**：无事务模式下失败必然留下已执行语句，必须如实报告，不能显示「执行失败，未生效」。
- **历史污染**：一个脚本只记一条历史；若用户需要逐条记录，后续另立需求。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/sql-file/SqlFileExecutionDialog.vue`、`SqlFileProgressIndicator.vue`、`FailureDetailsList.vue`、`sqlFilePathInput.ts`；`apps/desktop/src/composables/useLargeSqlFileFallback.ts`、`useSqlFileContentSearch.ts`；`crates/dbx-core/src/data/sql_file_import.rs` 及 `sql_file_import/table_restore.rs`、`failure_log.rs`、`includes.rs`；文档 `docs/content/docs/sql-file.cn.mdx`。参考其「大文件、进度、失败明细、重试」的行为。
- **Navicat**：运行 SQL 文件（Run SQL File）与还原备份时的脚本执行；参考官方手册的 Run SQL File / Restore 章节。

---

## C-03 表数据批量编辑与浏览增强

- **优先级**：P1
- **预估**：1–1.5 个开发会话
- **依赖**：无
- **替代关系**：细化并替代 `B-08`（表数据页批量编辑与事务提交）；新增过滤 / 排序 / 分页增强。

### 用户问题 / 场景

表数据页目前双击单元格就立即 `UPDATE`，新增 / 删除行也立即执行。一次改多行时没有确认环节，改错一个值就直接落库，也无法先看 SQL。筛选只有一个自由输入的 `WHERE`；排序只能单列；分页只能上一页 / 下一页。用户需要的是：先改、再看 SQL、最后一次事务提交；多行批量设置；更安全的筛选、多列排序和可跳转的分页。

### 期望行为

#### A. 待提交变更集

1. 编辑单元格、新增行、删除行都先进入「待提交」缓冲区，不立即写库：
   - 修改过的单元格高亮；
   - 新增行带「新增」标记；
   - 待删除行显示删除线 / 红色标记，可撤销；
   - 工具栏显示「N 项待提交」，并提供「查看 SQL」「提交」「放弃」。
2. 「查看 SQL」按「删除 → 更新 → 新增」顺序生成将执行的语句，值以方言字面量展示（仅预览，不执行）；同时显示每条语句预计影响 1 行。
3. 「提交」在一个数据库事务中执行全部变更：
   - 全部成功才提交；
   - 任一失败整体回滚，保留待提交状态，错误定位到具体项（行号 + 列名 + 原始错误）；
   - 更新 / 删除必须恰好影响 1 行；影响 0 行（行已被他人删除或主键变化）或大于 1 行时视为失败并回滚。
4. 提交成功后刷新当前页与总数，清空待提交；「放弃」清空缓冲区并恢复原值。
5. 有未提交变更时：
   - 切换页码、筛选、排序、刷新、切换数据 / 结构分段、关闭标签，先弹出「提交 / 放弃 / 取消」；
   - 选择「取消」时保留当前页面与全部待提交变更。
6. 无主键表：编辑 / 删除仍禁用，只允许新增；只读连接：全部禁用，后端提交时拒绝。

#### B. 多行选择与批量操作

7. 行首复选框支持单选 / 多选；`Shift` 选择连续区间，`⌘/Ctrl` 切换单行；「全选当前页」「清空选择」。
8. 选中行后提供：
   - 「批量设置值…」：选择一列，输入值或选择 `NULL`，应用到全部选中行（进入待提交，不立即执行）；
   - 「复制所选」：TSV（含表头开关）与 JSON；
   - 「克隆所选为新行」：复制值生成新增草稿，自增列 / 主键列留空，用户可修改；
   - 「删除所选」：标记为待删除，显示数量并二次确认。
9. 批量设置值必须按列类型解析（复用现有 `parseInput` / `sqlLiteral` 规则）；无法解析的值在提交前提示，不写入草稿。

#### C. 筛选、排序、分页

10. 筛选：
    - 提供「筛选构建器」：多条件（AND 组合），每行选择字段、操作符（`=`、`<>`、`>`、`>=`、`<`、`<=`、`LIKE`、`NOT LIKE`、`IS NULL`、`IS NOT NULL`）与值；
    - 标识符按方言引用，值按字面量转义；生成 SQL 在界面上可查看；
    - 保留「高级 WHERE」自由输入作为切换项，满足复杂条件；自由输入仍禁止分号，并提示风险；
    - 应用筛选后回到第 1 页，清空筛选恢复全表。
11. 排序：
    - 表头点击保持现有三态（升 / 降 / 清除）；
    - `Shift + 点击` 追加多列排序，界面显示排序 chips（列名 + 方向 + 移除）；
    - 后端接收排序列表，逐列校验存在性并引用；有主键时自动追加主键列作为稳定排序的次级键，保证跨页顺序稳定；
    - 清除排序恢复默认顺序。
12. 分页：
    - 每页行数选择 50 / 100 / 200 / 500，默认 100；按表记忆（localStorage），重启后恢复；
    - 页码输入框支持跳转，非法页码自动纠正到 1..总页数；
    - 显示「第 X / Y 页，共 N 行」；筛选 / 排序变化后总数刷新。
13. 列宽拖拽、列显隐、单元格右键复制等现有行为保持不变。

### 验收标准

- [ ] 修改两行各一个字段、新增一行、删除一行：工具栏显示 4 项待提交，修改单元格高亮，新增 / 删除标记正确。
- [ ] 「查看 SQL」按 DELETE → UPDATE → INSERT 顺序列出 4 条语句，数据库无变化。
- [ ] 提交成功后 4 项变更全部生效，表格刷新，待提交计数归零。
- [ ] 构造第 3 条必然失败（如唯一键冲突）：提交后前两条也回滚，数据库无变化，错误指出失败项，待提交仍保留。
- [ ] 点击「放弃」后表格恢复原值，数据库无变化。
- [ ] 多选 3 行（复选框 + Shift）后批量设置某列为 `NULL`：3 行进入待提交，未写库；提交后 3 行为 NULL。
- [ ] 克隆 2 行：生成 2 条新增草稿，自增主键列为空；提交后新增 2 行。
- [ ] 删除 3 行：标记待删除并显示数量；确认后仅标记，提交后数据库删除 3 行。
- [ ] 并发冲突：在另一连接删除某行后在 mydb 提交该行更新，提交失败并回滚全部，提示「目标行已不存在或已变化」。
- [ ] 筛选构建器：设置「status = 'active' AND name LIKE 'A%'」后结果正确；值含单引号（如 `O'Brien`）时生成的 SQL 可执行；清除筛选恢复全表。
- [ ] 高级 WHERE 仍可用；输入含分号时被拒绝并提示。
- [ ] 多列排序：`status DESC, created_at ASC` 跨页顺序稳定；有主键表自动附加主键排序；排序 chips 可单独移除。
- [ ] 每页 200 行设置后关闭并重开表标签仍为 200；跳转到第 3 页显示正确区间；页码越界自动纠正。
- [ ] 有待提交变更时切换筛选 / 排序 / 页码 / 关闭标签会弹出确认；选择取消后页面与变更均保留。
- [ ] 无主键表的编辑 / 删除禁用，新增可用；只读连接的全部写入口禁用且后端拒绝。

### 涉及文件与命令

**前端**

- 修改 `src/components/TableDataView.vue`（主要改动）：变更集状态、选择模型、批量操作、提交 / 放弃、离开守卫。
- 新增 `src/components/BatchSetDialog.vue`：批量设置列与值 / NULL。
- 新增 `src/components/ChangePreviewDialog.vue`：变更 SQL 预览与提交确认。
- 新增 `src/components/FilterBuilderDialog.vue`：多条件筛选构建器。
- 新增 `src/lib/table-changes.ts`：`PendingChange` 模型、合并 / 撤销 / 生成语句 / 计数。
- 修改 `src/lib/sql-literal.ts`、`src/lib/generate-sql.ts`：复用字面量与引用规则；不要在前端拼接参数化写入。
- 修改 `src/App.vue`：标签关闭前查询 `TableDataView` 的 dirty 状态；同一表多个标签时只影响当前标签。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`。

**后端**

- 修改 `src-tauri/src/models.rs`：`TableSort`、`PendingTableChange`、`TableChangeResult`。
- 修改 `src-tauri/src/commands/data.rs`：
  - `read_table_rows` 增加 `sorts: Option<Vec<TableSort>>`（保留旧的 `order_by` / `order_dir` 兼容参数或直接迁移调用点）；
  - 新增 `apply_table_changes(connection_id, table, changes) -> Result<TableChangeResult, String>`；
  - 新增 `preview_table_changes`（可选；若前端生成预览则不需要）。
- 修改 `src-tauri/src/db/driver.rs`：新增 `apply_table_changes`，默认实现返回「不支持」；三个驱动实现：
  - 获取连接并 `begin`；
  - 按 DELETE → UPDATE → INSERT 执行；
  - UPDATE / DELETE 校验影响行数必须为 1；
  - 任一失败 `rollback` 并返回失败项序号与数据库错误；
  - 成功后 `commit`。
- 修改 `src-tauri/src/db/sqlite.rs`、`mysql.rs`、`postgres.rs`：实现批量变更；复用现有 `update_row` / `insert_row` / `delete_row` 的构造逻辑，但必须在同一事务连接内执行。PostgreSQL 继续显式类型转换。
- 修改 `read_rows` 的排序参数：三个驱动逐列校验列存在、按方言引用、拼接 `ORDER BY`；空列表时不排序。
- 命令注册到 `src-tauri/src/lib.rs`。

### 风险与限制

- **没有乐观锁**：只按主键更新；如果其他会话修改了非主键列，本次提交会覆盖对方。第一版用「影响行数必须为 1」防止丢行，但不做版本比对；风险需在界面提示。
- **无主键表**：仍无法安全定位行，禁止编辑 / 删除；批量设置与克隆只对可定位行生效。
- **批量变更规模**：一次提交几千条会生成大量语句并长时间占用事务；第一版建议在待提交超过 500 项时提示分批提交，但不强制。
- **类型解析**：空字符串与 `NULL`、布尔、日期、二进制字段的输入规则必须与现有行内编辑一致；二进制列不参与批量设置（禁用）。
- **筛选构建器**：只支持 AND；OR / 括号 / 子查询走高级 WHERE。自由 WHERE 仍是「用户对自己数据库负责」的入口，不做 SQL 注入防护承诺，只做分号与明显多语句拦截。
- **稳定分页**：非唯一排序列在数据变化时仍可能跨页重复；自动追加主键可缓解，但无主键表无法保证。
- **多标签同一表**：不同表标签的待提交变更彼此独立；切换标签不提交，也不合并。
- **生成列**：`ColumnInfo` 尚无 generated 标记，批量设置可能包含生成列导致数据库报错；失败时回滚并提示，后续可补元数据。

### 参考

- **DBX（Apache-2.0）**：文档 `docs/content/docs/data-grid.cn.mdx`；`apps/desktop/src/components/grid/DataGridBulkEditDialog.vue`、`DataGridInsertRowsDialog.vue`、`DataGridFilterBuilder.vue`、`DataGridConditionEditor.vue`、`DataGridPagination.vue`、`DataGridQueryControls.vue`、`DataGridCopyFormatControl.vue`；`crates/dbx-core/src/data/grid_clipboard_guard.rs`。参考其「批量编辑、待提交、筛选构建、分页、复制格式」的行为。
- **Navicat**：表数据查看器的多行选择、批量更新、筛选与排序；参考官方手册的 Table Data Viewer / Filter 章节。

---

## C-04 表设计器补全：索引 / 外键 / 注释 / 预览

- **优先级**：P1
- **预估**：1–1.5 个开发会话
- **依赖**：无
- **替代关系**：扩展既有「新建表设计器」；`B-33` 只做视图 / 例程创建，不覆盖本项。

### 用户问题 / 场景

现在的建表设计器只能编辑字段，索引要建完表再去结构页逐个添加，外键完全不能在创建时声明，表注释缺失，结构页的外键只读。用户想一次把表设计完整：字段、复合索引、外键、注释，并在执行前看到完整 DDL；已有表也要能补外键和注释。

### 期望行为

#### A. 新建表设计器

1. 对话框改为分段 / 页签：**字段**、**索引**、**外键**、**表选项**、**预览**。
2. 字段页保留现有能力：名称、类型、可空、主键（复合）、自增、默认值、注释；增加只读的「生成列 / 标识列」提示（若元数据可得）。
3. 索引页：
   - 每行：索引名（可自动生成，如 `idx_<table>_<cols>`）、列（多选并按顺序排列，支持上移 / 下移）、唯一开关、方法（PostgreSQL 可选 btree / hash；MySQL 第一版固定 BTREE）、PostgreSQL 部分索引 `WHERE`（可选）；
   - 支持复合索引；同一列可重复出现在不同索引；
   - 索引名在当前表内不可重复；列必须来自字段页。
4. 外键页：
   - 每行：约束名（可自动生成）、本地列（有序）、引用表（下拉当前库 / 模式内的表）、引用列（有序）、`ON DELETE` / `ON UPDATE`（NO ACTION / RESTRICT / CASCADE / SET NULL）；
   - 校验：本地列与引用列数量一致、引用表存在、引用列存在；SQLite 额外提示「引用列必须有唯一约束」，只警告不阻断；
   - MySQL 若本地列无索引，提示将自动创建索引（InnoDB 行为），不重复生成。
5. 表选项页：表注释（MySQL 写入 `COMMENT='...'`；PostgreSQL 生成 `COMMENT ON TABLE`；SQLite 明确提示忽略）、MySQL 引擎 / 字符集 / 排序规则（可选，第一版可只做注释）。
6. 预览页：
   - 按执行顺序列出全部语句（建表、注释、索引、外键），并显示可复制的完整脚本；
   - 执行前校验：表名为空、字段为空、重复字段名、索引 / 外键引用不存在、SQLite 复合主键与自增冲突等，给出具体错误；
   - 点击「创建」后按方言执行：MySQL 单条建表（内联索引 / 外键 / 注释）；PostgreSQL 建表 + 注释 + 索引 + 外键（单事务）；SQLite 建表（内联外键）+ 索引，注释忽略并提示；
   - 失败时整体回滚（PostgreSQL / SQLite），MySQL 若建表成功但后续注释失败，需明确报告并允许重试；保留对话框内容。

#### B. 结构页的外键与注释

7. 表结构页「外键」区域增加「添加外键」「删除」：
   - 添加：复用 C-04 的外键表单，先预览 `ALTER TABLE ... ADD CONSTRAINT`；
   - 删除：预览 `ALTER TABLE ... DROP FOREIGN KEY`（MySQL）/ `DROP CONSTRAINT`（PostgreSQL），确认后执行；
   - SQLite：两个按钮禁用，提示「SQLite 需重建表，暂不支持」；
   - 只读连接：禁用并在后端拒绝。
8. 字段编辑对话框增加「注释」：
   - MySQL：`MODIFY COLUMN` 带 `COMMENT`，保留现有类型 / 可空 / 默认值；
   - PostgreSQL：执行 `COMMENT ON COLUMN`；
   - SQLite：禁用并提示不支持；
   - 修改后结构页与 DDL 预览同步更新。
9. 索引编辑继续使用现有 `IndexEditorDialog`；若索引列顺序能力不足，按本项要求补齐（多列有序）。

### 验收标准

- [ ] MySQL：创建含复合主键、两列唯一索引、一个外键、表注释与字段注释的表；预览脚本与执行后结构页一致；可正常插入数据。
- [ ] PostgreSQL：创建含 identity、普通索引、外键、表注释与字段注释的表；预览包含 `COMMENT ON` 与 `CREATE INDEX`；执行成功；故意制造外键错误时整体回滚，不留下半成品表。
- [ ] SQLite：创建含内联外键与独立索引的表；索引存在；表 / 字段注释被忽略且有明确提示。
- [ ] 索引列顺序：`(a, b)` 与 `(b, a)` 生成不同 DDL，结构页显示顺序一致；重复索引名被阻止；引用不存在字段被阻止。
- [ ] 外键：引用表下拉只包含当前库 / 模式表；本地列与引用列数量不一致时阻止执行；`ON DELETE CASCADE` 在执行后的结构页可见。
- [ ] 已有表：MySQL / PostgreSQL 添加外键、删除外键均先预览再执行，刷新后结构页正确；SQLite 按钮禁用并解释原因。
- [ ] 字段注释：MySQL / PostgreSQL 修改字段注释后重新打开结构页可见；SQLite 禁用。
- [ ] 只读连接：创建、添加外键、删除外键、改注释全部禁用，绕过前端调用后端仍返回只读错误。
- [ ] 预览复制：预览页复制的脚本与执行语句一致（去除首尾空白后逐条相等）。
- [ ] 创建成功后对象树自动刷新并出现新表，无需重启。

### 涉及文件与命令

**前端**

- 修改 `src/components/CreateTableDialog.vue`：页签、索引 / 外键 / 表选项 / 预览、校验。
- 新增 `src/components/ForeignKeyEditorDialog.vue`：外键表单，供建表与结构页复用。
- 修改 `src/components/TableStructureView.vue`：外键添加 / 删除入口。
- 修改 `src/components/ColumnEditorDialog.vue`：注释字段。
- 修改 `src/components/IndexEditorDialog.vue`：多列顺序。
- 新增 `src/lib/table-ddl.ts`（或扩展 `src/lib/alter.ts`）：按方言生成建表 / 索引 / 外键 / 注释语句，纯函数便于测试。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`。

**后端**

- 修改 `src-tauri/src/models.rs`：`IndexSpec`、`ForeignKeySpec`、`TableOptions`、`AlterForeignKeyRequest`。
- 修改 `src-tauri/src/commands/data.rs`：
  - `alter_table_foreign_key(connection_id, table, action, spec)`；
  - 可选的 `set_table_comment` / `set_column_comment`（若前端通过 `execute_query` 执行，则不需要新命令，但需保证多语句顺序与事务）；
  - 建表继续使用现有 `execute_query` 执行前端生成的 DDL 列表，或在后端新增 `create_table` 命令统一事务。
- 修改 `src-tauri/src/db/mysql.rs`、`postgres.rs`、`sqlite.rs`：
  - 外键添加 / 删除的方言 SQL 与校验；
  - 注释修改；
  - 索引 DDL 生成可复用 `commands/sync.rs::create_index_sql` / `drop_index_sql`（提升为共享函数）。
- 修改 `src-tauri/src/lib.rs`：注册新命令。

### 风险与限制

- **SQLite 限制**：不能在已有表上添加 / 删除外键；第一版明确禁用，不做重建表。
- **MySQL 外键要求**：引用列必须有索引，本地列无索引时 InnoDB 会自动创建；预览中不重复生成，但需提示。
- **PostgreSQL 约束名**：默认约束名可能冲突；生成时带表名前缀，失败时显示原始错误。
- **注释转义**：表 / 字段注释中的单引号必须转义；PostgreSQL 使用 `COMMENT ON` 的字符串字面量。
- **生成列 / 表达式默认值**：设计器第一版不专门支持生成列；用户可在预览页手工修改 DDL 后复制执行（不提供编辑器内改 DDL 后执行，避免绕过校验）。
- **已有表全量编辑**：本项不做字段重排、引擎 / 字符集变更、表重建；这些仍走结构页单字段 ALTER 或后续需求。
- **MySQL DDL 隐式提交**：建表后的注释 / 索引若失败，表可能已存在；必须如实报告并允许清理。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/structure/TableStructureEditor.vue`、`StructureIndexColumnPicker.vue`、`StructureColumnMultiSelect.vue`、`TableColumnPasteDialog.vue`；文档 `docs/content/docs/table-structure.cn.mdx`。参考其字段 / 索引 / 外键 / DDL 预览的页签划分与校验思路。
- **Navicat**：表设计器（Table Designer）的字段、索引、外键、注释与 DDL 预览；参考官方手册的 Table Designer 章节。

---

## C-05 数据传输向导：多表迁移与建表

- **优先级**：P1
- **预估**：1.5–2 个开发会话
- **依赖**：C-01 的结构导出能力（DDL 生成）、现有结构同步 / 数据同步
- **替代关系**：细化并替代 `B-35`；不重复现有的「结构同步」与「按主键数据同步」比较能力。

### 用户问题 / 场景

现有数据同步一次只能比较一张表，要求两边主键一致，只生成差异 SQL，不负责建表；跨库迁移多张表时，用户仍要逐表建结构、再逐表同步。Navicat / DBX 的「数据传输」是选源、选目标、选多张表、一次搬完，并可选目标表不存在时自动建表。mydb 需要同样的多表搬运能力。

### 期望行为

1. **入口**：「工具 → 数据传输…」，要求当前有活动连接；也允许从对象树数据库节点的「传输到…」进入并预填源。
2. **向导步骤**：源 → 目标 → 对象 → 选项 → 预检 / 执行 → 结果。
3. **源与目标**：
   - 源：连接 + 数据库（MySQL）/ 数据库 + 模式（PostgreSQL）/ SQLite 连接；
   - 目标：同驱动连接 + 数据库 / 模式；支持同一连接下跨库 / 跨模式（复用 `commands/context.rs::isolated_driver_for`），源连接当前库 / 模式不被改变；
   - SQLite 目标必须是另一条连接（同一文件不允许自己传自己）；
   - 跨驱动禁用并提示。
4. **对象选择**：
   - 列出源范围的全部表（视图 / 例程第一版不提供），支持搜索、全选 / 全不选、按表勾选；
   - 每张表的目标名默认同名，可编辑；目标范围已有同名表时标记「已存在」；
   - 两个源表映射到同一目标名时阻止继续；
   - 目标表不存在时默认勾选「创建目标表」，可取消；取消时该表在执行前报「目标表不存在」。
5. **列映射**：
   - 按列名自动匹配；可逐表调整源列 → 目标列（一对一），或忽略源列；
   - 目标表已有列中，`NOT NULL` 且无默认值、非自增且未映射的列在预检阶段阻止该表；
   - 源列在目标不存在时默认忽略并提示；
   - 第一版不做类型转换，只做同驱动映射；类型不兼容（如目标列更短）在预检给出警告，执行失败按错误处理。
6. **传输模式**（每张表或全局选择）：
   - **追加**：保留目标现有数据，插入源行；
   - **覆盖**：先 `DELETE FROM 目标表`，再插入；需二次确认；不使用 `TRUNCATE`（MySQL 隐式提交，破坏事务）；
   - **按主键 Upsert**：要求源 / 目标都有相同主键且主键列已映射；MySQL `INSERT ... ON DUPLICATE KEY UPDATE`，PostgreSQL `INSERT ... ON CONFLICT (...) DO UPDATE`，SQLite 同 PostgreSQL 语法；
   - 第一版不提供「重建结构」；需要重建时先手动删表或使用结构同步。
7. **建表规则**：
   - 目标缺失且勾选建表时，按源表结构生成目标表：字段、主键、非空、默认值、自增 / identity、注释、普通索引；外键默认不建（避免顺序问题），可勾选「包含外键」并在所有表数据写入后统一添加；
   - 表名替换为目标名；索引 / 约束名加目标表前缀，避免冲突；
   - 建表失败时该表标记失败，其他表继续（默认）或停止（可选）。
8. **读取与写入**：
   - 有主键：按主键排序分页读取（每批 500 行，可调 100–5000），保证稳定；
   - 无主键：一次性读取至多 50000 行；超过时拒绝该表并提示「无主键表无法稳定分批」；
   - 写入按批 `INSERT`（多行），Upsert 按批生成；二进制 / 数组字段按驱动能力处理，无法处理时该表失败并说明；
   - 每张表默认一个事务：数据段（清空 + 写入）成功才提交；PostgreSQL / SQLite 可把建表 / 索引 / 外键一并纳入事务，失败全部回滚；MySQL 的建表 / 索引 DDL 会隐式提交，只能保证数据段回滚，已建的表保留并在汇总中说明；失败回滚该表的数据，已完成的表保留并如实报告；
   - 可选「整个任务一个事务」仅对 PostgreSQL / SQLite 且不含 MySQL DDL 时开放；MySQL DDL 隐式提交需提示。
9. **预检**：
   - 目标只读 → 阻止开始；
   - 源只读 → 允许；
   - 目标表缺失且未勾选建表 → 列出；
   - 列映射缺必填列 → 列出；
   - Upsert 缺主键 / 主键未映射 → 列出；
   - 覆盖模式未确认 → 阻止；
   - 预检通过后显示「将复制 N 张表，预计 M 行」；行数可选「快速估算」或跳过（`COUNT(*)` 大表较慢）。
10. **执行与进度**：
    - 逐表执行，显示表序号 / 总数、表名、阶段（建表 / 清空 / 读取 / 写入）、已复制行数、失败行数；
    - 支持取消：当前批次完成后停止；当前表回滚；已完成表保留；
    - 完成后汇总：成功 / 失败 / 跳过表数、总行数、耗时；失败表列出原因；可复制汇总。
11. **安全**：覆盖模式、包含外键、删除目标数据等危险操作必须二次确认；目标只读在后端再次拦截；不修改源库。

### 验收标准

- [ ] 同驱动两个连接之间复制 2 张表（各 1 万行），目标表自动创建，字段 / 主键 / 索引存在，逐表行数与抽样值一致。
- [ ] 同一 MySQL 连接跨数据库、同一 PostgreSQL 连接跨模式传输成功，源连接当前库 / 模式不变；传输后可在源连接继续查询。
- [ ] 列映射：源 `name` 映射到目标 `full_name`、忽略 `legacy` 列，数据正确写入；目标必填列未映射时预检阻止并指出表 / 列。
- [ ] 追加模式保留目标已有行；覆盖模式在确认后清空目标再写入；Upsert 对已存在主键更新、对不存在主键插入。
- [ ] Upsert 在任一侧缺主键或主键未映射时被预检阻止，不生成部分数据。
- [ ] 建表时包含外键：所有表数据写入完成后外键才添加；循环引用不导致建表失败。
- [ ] 中途失败（如目标列长度不足）：该表回滚，目标表无部分行；其他表按策略继续或停止；汇总准确。
- [ ] 取消：复制 10 万行途中取消，当前表回滚，已完成表保留，汇总显示「已取消」与已完成数量。
- [ ] 只读目标：预检阻止，目标库无任何写入；源只读可正常读取。
- [ ] 无主键表：超过 5 万行时拒绝并提示；小于 5 万行可复制。
- [ ] 两个源表映射到同一目标名时阻止继续；目标已有表且未勾选建表时执行前报错。
- [ ] 进度持续更新，窗口其他标签可操作；任务结束后无残留事务 / 连接。

### 涉及文件与命令

**前端**

- 新增 `src/components/DataTransferDialog.vue`：向导、源 / 目标选择、对象树、列映射、模式、预检。
- 新增 `src/components/TransferObjectTree.vue`：多选表与搜索。
- 新增 `src/components/TransferProgressDialog.vue`：进度、取消、汇总。
- 修改 `src/components/AppMenuBar.vue` / `src/lib/app-actions.ts` / `src/App.vue`：工具菜单入口。
- 修改 `src/lib/sync-scope.ts`：复用源 / 目标范围解析；新增传输专用类型。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`。

**后端**

- 新增 `src-tauri/src/commands/transfer.rs`：`start_data_transfer`、`cancel_data_transfer`、进度事件、预检。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/src/models.rs`：`TransferRequest`、`TransferTablePlan`、`TransferColumnMapping`、`TransferProgress`、`TransferSummary`。
- 复用：
  - `commands/context.rs::isolated_driver_for`（源 / 目标独立驱动）；
  - `commands/sync.rs` 的 `create_index_sql`、`column_definition`、`add_foreign_key_sql`（提升可见性）；
  - `commands/data_sync.rs` 的 `sql_literal`、`quote_identifier`（抽到共享模块，避免重复）；
  - `commands/data.rs` 的流式读取思路。
- 修改 `src-tauri/src/db/driver.rs`：新增 `insert_rows_batch` 与 `upsert_rows_batch`（或 `transfer_rows`），默认返回不支持；三个驱动实现参数化 / 字面量批量写入。
- 修改 `src-tauri/src/db/mysql.rs`、`postgres.rs`、`sqlite.rs`：批量插入、Upsert 方言、按主键分页读取。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn start_data_transfer(
      app: AppHandle,
      state: State<'_, AppState>,
      request: TransferRequest,
      operation_id: String,
  ) -> Result<TransferSummary, String>;

  #[tauri::command]
  pub async fn cancel_data_transfer(state: State<'_, AppState>, operation_id: String) -> Result<(), String>;
  ```
- 进度事件：`data-transfer:progress`。

### 风险与限制

- **无主键表**：无法稳定分批，只能限制行数一次读取；大表会占用内存，必须在预检阶段拒绝。
- **外键与顺序**：默认不建外键；勾选外键时在所有数据写完后再添加，避免前向引用。已有目标表的外键可能影响覆盖模式（`DELETE` 失败），需在预检提示。
- **覆盖模式性能**：`DELETE` 大表可能很慢并产生大量日志；第一版坚持事务安全，不提供 `TRUNCATE`。
- **Upsert 方言差异**：MySQL 使用 `ON DUPLICATE KEY UPDATE`，PostgreSQL / SQLite 使用 `ON CONFLICT`；需要确认主键列集合与目标表一致，否则预检拒绝。
- **类型与字符集**：同驱动不同版本仍可能有类型差异（如 MySQL 字符集、PostgreSQL 自定义类型）；第一版只警告，不做转换。
- **连接占用**：传输可能长时间占用源 / 目标连接；需要确保不阻塞同一连接上的其他标签，并支持取消释放。
- **进度事件频率**：大表每批发送会过于频繁；按 500ms 节流。
- **MySQL DDL 隐式提交**：建表与数据写入不能整体回滚；失败时已建的表保留，汇总必须写清楚。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/transfer/DataTransferDialog.vue`、`DataTransferProgressDialog.vue`、`ObjectSelectionTree.vue`、`TransferTaskTree.vue`、`transferStrategy.ts`、`transferSelections.ts`、`structurePlanSummary.ts`；`apps/desktop/src/lib/database/dataTransferSelection.ts`；`crates/dbx-core/src/data/transfer.rs`、`transfer_runner.rs`、`transfer/structure_plan.rs`、`transfer/overwrite_clear.rs`；文档 `docs/content/docs/data-transfer.cn.mdx`。参考其「创建目标表、追加 / 覆盖 / Upsert、批大小、进度、失败处理」。
- **Navicat**：数据传输（Data Transfer）的源 / 目标、表选择、创建目标表、追加 / 更新 / 覆盖模式；参考官方手册的 Data Transfer 章节。

---

## C-06 结构同步应用到目标（受控自动执行）

- **优先级**：P1
- **预估**：0.5–1 个开发会话
- **依赖**：现有结构同步；可选依赖 C-01（应用前结构快照）
- **替代关系**：新增。`BACKLOG.md` 已实现基线明确「结构同步生成可执行 SQL，不自动执行」，本项是该行为的增量。

### 用户问题 / 场景

结构同步已经能生成迁移 SQL，但用户只能复制 / 保存，再手动找地方执行；执行时不知道是否用了正确的目标库、有没有漏掉语句、失败后停在哪。用户需要「对比 → 审阅 → 直接应用」的闭环，同时不能把危险 DROP 默默执行掉。

### 期望行为

1. 在结构同步对话框的 SQL 预览下方增加「应用到目标…」按钮（只读目标禁用）。
2. 点击后打开确认页：
   - 目标连接、数据库 / 模式、语句总数、按类型统计（CREATE / ALTER / DROP / 其他）；
   - 危险语句数量（`DROP TABLE` / `DROP COLUMN` / `DROP INDEX` / `DROP CONSTRAINT`）单独高亮；
   - 「应用前导出目标结构快照」复选框（默认开，依赖 C-01 的结构导出；生成到临时目录并在结果中显示路径）；
   - 危险操作必须勾选「我确认执行删除语句」并输入目标库 / 模式名确认（仅当包含 DROP 时）。
3. **应用前重新对比**：使用与预览时相同的源 / 目标 / 选项重新生成一次 diff；若语句集合与预览不一致（目标被他人改动、源已变化），中止并提示「差异已变化，请重新对比」，不应用旧语句。
4. **执行策略**：
   - PostgreSQL / SQLite：所有语句在一个事务中执行，失败全部回滚；
   - MySQL：DDL 隐式提交，按顺序逐条执行，失败即停止；明确显示「已执行 N 条，剩余未执行，无法自动回滚」；
   - 只读目标：后端拒绝；
   - 连接断开：显示「执行状态未知，请重新对比确认」，不自动重试。
5. **进度**：显示当前语句序号 / 总数、语句摘要、成功 / 失败、耗时；可取消（当前语句结束后停止；PostgreSQL / SQLite 回滚）。
6. **结果**：
   - 成功：显示已应用语句数、耗时、快照路径（如有）；自动刷新对象树；自动重新对比并显示「无差异」；
   - 失败：显示失败语句序号、SQL 摘要、数据库原始错误、已执行 / 未执行数量；事务模式下明确「已全部回滚」；
   - 提供「复制失败详情」。
7. **安全**：
   - 默认不包含 DROP（沿用现有 `include_drops` 默认 false）；
   - 危险语句必须单独统计并在确认页高亮，不能只靠一个不说明数量的总开关；
   - 不在应用后自动删除快照文件。
8. **范围**：沿用现有结构同步的源 / 目标选择：同驱动、跨连接或同实例跨库 / 跨模式；SQLite 的限制保持现状（只提示，不生成不安全 ALTER / DROP）。

### 验收标准

- [ ] 源表新增一个字段与一个索引，对比后应用：目标表结构与源一致，重新对比显示无差异，对象树已刷新。
- [ ] PostgreSQL / SQLite：构造第 2 条语句失败（如重复索引名），应用后第 1 条也回滚，目标结构不变，错误指出语句序号。
- [ ] MySQL：构造第 2 条失败，界面显示「第 1 条已执行、第 2 条失败、其余未执行」，不显示「已回滚」。
- [ ] 含 DROP 的差异：未勾选确认时按钮禁用；勾选并输入正确目标名后才执行；执行后目标对象被删除。
- [ ] 应用前目标被他人改动（例如先手工加了一个字段）：重新对比发现语句变化，应用被拒绝并提示重新对比，未执行任何语句。
- [ ] 只读目标：按钮禁用，绕过前端调用后端返回只读错误，目标无变化。
- [ ] 快照：勾选后生成目标结构 `.sql`，路径在结果中可见且文件可读；不勾选时不生成。
- [ ] 同连接跨库 / 跨模式：目标为同一 MySQL 连接的另一数据库、同一 PostgreSQL 连接的另一模式时应用成功，源连接当前库 / 模式不变。
- [ ] 取消：应用 50 条语句途中取消，PostgreSQL / SQLite 全部回滚；MySQL 报告已执行条数。
- [ ] 连接中断：显示「状态未知」，不自动重试，不误报成功。

### 涉及文件与命令

**前端**

- 修改 `src/components/SchemaSyncDialog.vue`：应用按钮、确认页、危险确认、进度、结果。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：应用命令与结果类型。
- 修改 `src/App.vue`：应用成功后的对象树刷新与提示。

**后端**

- 修改 `src-tauri/src/commands/sync.rs`：
  - 新增 `apply_schema_sync(...)`：接收与 `compare_schemas` 相同的源 / 目标 / 选项参数 + `expected_statements: Vec<String>`（或语句哈希）+ 执行选项；
  - 内部重新生成 diff 并比较语句集合；不一致返回可识别错误码 / 错误前缀；
  - 按驱动选择事务或逐条执行；
  - 返回 `SchemaApplyResult { applied, total, failedAt, error, rolledBack }`。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册命令。
- 复用 `db::driver::execute_transaction`（PostgreSQL / SQLite）与 `DatabaseSession`；MySQL 逐条 `execute`。
- 快照调用 C-01 的结构导出函数（若 C-01 尚未实现，本项可先不做快照，但验收中该项标记「依赖 C-01」）。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn apply_schema_sync(
      app: AppHandle,
      state: State<'_, AppState>,
      request: SchemaApplyRequest,
      operation_id: Option<String>,
  ) -> Result<SchemaApplyResult, String>;
  ```
- 进度事件：`schema-sync:progress`。

### 风险与限制

- **MySQL DDL 非事务**：不能承诺回滚；必须如实报告已执行条数。
- **差异过期**：重新对比是必要保护，但两次对比之间仍可能有竞态；执行前提示用户避免并发变更。
- **对象依赖**：视图 / 例程依赖顺序可能失败；失败即停止，不继续猜测。
- **权限**：目标账号可能能对比但不能 ALTER；错误按语句显示。
- **快照本身**：结构快照依赖 C-01；若 C-01 未完成，本项先不提供快照，不得用一个简化实现伪装完整备份。
- **危险操作**：不做「一键全部删除」；DROP 需要显式确认目标名，避免误连生产库。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/diff/SchemaDiffDialog.vue`、`SchemaDiffDeployStep.vue`、`SchemaDiffOptionsPanel.vue`、`SchemaDiffDdlPanel.vue`、`SchemaDiffObjectTree.vue`；`apps/desktop/src/lib/schema/schemaDiff.ts`、`useSchemaDiffSession.ts`；文档 `docs/content/docs/schema-diff.cn.mdx`。参考其「对比 → 部署审核 → 执行」的分步与失败处理。
- **Navicat**：结构同步（Structure Synchronization）的对比、脚本审阅与执行；参考官方手册的 Structure Synchronization 章节。

---

## C-07 服务器监视与进程列表

- **优先级**：P2
- **预估**：1–1.5 个开发会话
- **依赖**：无
- **替代关系**：承接 `B-37` 的「进程 / 会话列表 + 服务器状态」部分；**不包含用户 / 权限**（本批明确不做）。

### 用户问题 / 场景

连接变慢或出现锁等待时，用户只能另开一个 SQL 标签手写 `SHOW PROCESSLIST` / `SELECT * FROM pg_stat_activity`，不同数据库语法不同，也无法一键终止异常会话。用户需要一个随当前连接打开的「服务器活动」面板：看状态、看会话、终止会话，只读连接只能看不能杀。

### 期望行为

1. **入口**：「工具 → 服务器活动…」，或连接右键「服务器活动…」；要求当前连接已打开。SQLite / Redis 不显示入口。
2. **面板形态**：对话框（第一版不新增标签类型），包含「概览」「进程」两个页签；标题显示连接名、驱动、当前库 / 模式；右上角「刷新」与自动刷新选择（关闭 / 5 秒 / 10 秒，默认关闭）。
3. **概览页**（按驱动）：
   - 公共：服务器版本、运行时长、当前连接数 / 最大连接数、当前库 / 模式、当前用户；
   - MySQL：`Threads_running`、`Queries`、`Slow_queries`、当前库大小、当前库表数；
   - PostgreSQL：活动会话数、空闲会话数、当前库大小、当前模式表数、缓存命中率（可选）；
   - 每个数值标注来源与「刷新时间」；数值格式化（字节、时长、千分位）。
4. **进程页**：
   - 列：会话 ID / PID、用户、数据库 / 模式、客户端地址、命令 / 状态、耗时（秒）、当前 SQL（截断显示，可展开 / 复制）；
   - 当前应用自身会话高亮并标记「当前连接」，**禁止终止自己**；
   - 筛选：只看活动会话、按用户 / 数据库过滤、文本搜索；
   - 刷新与自动刷新与概览共享；刷新期间显示加载状态，不叠加请求。
5. **终止会话**：
   - 每行提供「终止会话…」与「取消当前查询…」（MySQL `KILL <id>` / `KILL QUERY <id>`；PostgreSQL `pg_terminate_backend(pid)` / `pg_cancel_backend(pid)`）；
   - 必须先确认，确认框显示会话 ID、用户、数据库、耗时与 SQL 摘要；
   - 只读连接：按钮禁用，后端拒绝；
   - 终止自己：禁用并提示；
   - 成功 / 失败以行内状态或 Toast 反馈；失败保留原始数据库错误；成功后刷新列表。
6. **权限与错误**：
   - 账号无 `PROCESS`（MySQL）或非超级用户（PostgreSQL）时，进程列表可能只显示自己的会话；界面显示「当前账号只能看到部分会话」而不是报错；
   - 概览某个指标查询失败时，该指标显示「无权限 / 不支持」，其他指标正常；
   - 连接失效时提示重新连接，不崩溃。
7. **安全与历史**：面板只读（除终止会话外）；终止操作不写入 SQL 执行历史；当前 SQL 文本只用于展示，不写入日志。
8. **性能**：进程列表限制返回最多 500 行，超过时提示；SQL 文本截断到 2000 字符；自动刷新在对话框关闭时停止。

### 验收标准

- [ ] MySQL：打开面板能看到当前会话与其他会话的 ID、用户、库、命令、耗时、SQL；当前会话被标记且「终止」禁用。
- [ ] MySQL：另开一个 `SELECT SLEEP(300)` 会话，在面板中终止它，确认后会话消失；「取消当前查询」只结束查询、会话仍在。
- [ ] PostgreSQL：`pg_stat_activity` 数据正确显示；`pg_terminate_backend` 与 `pg_cancel_backend` 分别生效；当前 PID 禁止终止。
- [ ] 只读连接：可查看概览与进程；终止 / 取消按钮禁用，绕过前端调用后端返回只读错误，目标会话不受影响。
- [ ] 概览数值与手工 SQL 查询一致（版本、运行时长、连接数、慢查询数、库大小）；字节 / 时长格式化正确。
- [ ] 自动刷新 5 秒：连续运行 30 秒无请求叠加、无内存增长；关闭对话框后停止请求。
- [ ] 权限受限账号：进程列表只显示自己的会话时给出说明；概览无权限的指标显示可读错误，其他指标仍显示。
- [ ] 进程超过 500 行时提示截断；SQL 文本超过 2000 字符时可展开查看完整内容并复制。
- [ ] SQLite / Redis 连接不显示「服务器活动」入口。
- [ ] 打开面板、刷新、终止会话均不新增 SQL 执行历史记录。

### 涉及文件与命令

**前端**

- 新增 `src/components/ServerActivityDialog.vue`：概览 / 进程两个页签、刷新、自动刷新、终止确认。
- 修改 `src/components/AppMenuBar.vue` / `src/lib/app-actions.ts` / `src/App.vue`：工具菜单与连接右键入口。
- 修改 `src/lib/api.ts`、`src/lib/types.ts`：`ServerStatus`、`ServerProcess`、`KillSessionRequest`。

**后端**

- 新增 `src-tauri/src/commands/server.rs`：`server_status`、`server_processes`、`server_kill_session`（`cancel_only` 区分终止会话 / 取消查询）。
- 修改 `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`：注册。
- 修改 `src-tauri/src/models.rs`：`ServerStatus`、`ServerProcess`、`ServerKillRequest`。
- 修改 `src-tauri/src/db/driver.rs`：新增默认返回「不支持」的 `server_status`、`server_processes`、`kill_session`、`cancel_query`；MySQL / PostgreSQL 实现，SQLite 返回不支持。
- 修改 `src-tauri/src/db/mysql.rs`：`SHOW GLOBAL STATUS` / `SHOW GLOBAL VARIABLES`、`information_schema.PROCESSLIST`（或 `SHOW FULL PROCESSLIST`）、`KILL`。
- 修改 `src-tauri/src/db/postgres.rs`：`pg_stat_activity`、`pg_postmaster_start_time()`、`pg_settings`、`pg_database_size(current_database())`、`pg_terminate_backend`、`pg_cancel_backend`。
- 终止命令自行检查 `connection_read_only`，**不要**把 `KILL` / `pg_terminate_backend` 暴露为通用 `execute_query` 调用；`sql_guard` 对只读连接的禁用保持现状。
- 命令签名建议：
  ```rust
  #[tauri::command]
  pub async fn server_status(app: AppHandle, state: State<'_, AppState>, connection_id: String) -> Result<ServerStatus, String>;

  #[tauri::command]
  pub async fn server_processes(app: AppHandle, state: State<'_, AppState>, connection_id: String) -> Result<Vec<ServerProcess>, String>;

  #[tauri::command]
  pub async fn server_kill_session(
      app: AppHandle,
      state: State<'_, AppState>,
      connection_id: String,
      session_id: String,
      cancel_only: bool,
  ) -> Result<(), String>;
  ```

### 风险与限制

- **权限差异**：MySQL 8 / MariaDB 的 `PROCESSLIST` 字段与权限不同；PostgreSQL 非超级用户只能看到部分 `pg_stat_activity` 字段。必须按「可解释的部分结果」处理。
- **终止会话高危**：必须确认；禁止终止自己的连接；只读连接禁止；后端再次校验。
- **MySQL `KILL` 语义**：`KILL` 断开连接，`KILL QUERY` 只取消语句；界面上必须区分，不能只写「终止」。
- **SQL 文本敏感**：可能包含明文参数；不写日志、不自动复制。
- **自动刷新**：多个面板 / 多个连接同时刷新会增加服务器负担；第一版只允许当前活动连接一个面板。
- **服务器版本差异**：MariaDB 与 MySQL 8 的 `SHOW STATUS` 键名不同；缺失指标显示「不支持」，不报整体失败。
- **不是监控系统**：不保存历史指标、不画趋势图；历史监控需要独立需求。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/components/admin/ProcessListPanel.vue`、`MySqlDashboard.vue`、`PostgresDashboard.vue`、`SessionDiagnostics.vue`；`apps/desktop/src/lib/database/mysqlProcessList.ts`、`postgresProcessList.ts`、`mysqlServerStatus.ts`、`postgresServerStatus.ts`、`processListSelection.ts`。参考其「状态卡片 + 进程表 + 终止确认」的划分。
- **Navicat**：服务器监视（Server Monitor）与进程列表（Process List）；参考官方手册的 Server Monitor / Process List 章节。

---

## C-08 ER 图导出、布局持久化与定位

- **优先级**：P2
- **预估**：1 个开发会话
- **依赖**：无
- **替代关系**：细化并替代 `B-17`（ER 图增强）。

### 用户问题 / 场景

ER 图目前只能在当前会话里看：拖好的布局切换标签或重启就丢；不能导出给同事评审；表多时找不到某张表；从对象树也无法定位到 ER 图。用户需要「可保存、可导出、可搜索定位」的 ER 图，作为结构评审与交接材料。

### 期望行为

1. **工具栏**（ER 标签内）：适应窗口、放大 / 缩小、重置布局、导出 SVG、导出 PNG、搜索框、显示 / 隐藏视图节点（可选）。
2. **导出**：
   - SVG：包含当前可见表 / 视图节点、字段名、主键标记、外键连线、标题；内联样式，脱离应用 CSS 也能正确显示；中文不乱码；
   - PNG：使用浏览器 Canvas 从当前 SVG / 视图绘制，按 2x 像素比导出，背景使用当前主题色；文件名默认 `<连接>-<库/模式>-er.svg/png`；
   - 导出使用原生保存对话框；失败时提示，不生成空文件。
3. **布局持久化**：
   - 节点坐标按「连接 ID + 数据库 + 模式」写入 localStorage（键带版本号，例如 `mydb.erLayout.v1`）；
   - 拖动结束、重置布局、导出后写入；读取时校验坐标是有限数字，损坏则回退自动布局；
   - 同一来源重新打开恢复位置；不同库 / 模式互不影响；表被删除后忽略其坐标，新增表自动放置且不与已有节点重叠。
4. **搜索定位**：
   - 搜索框匹配表名、字段名；匹配节点高亮（描边 / 阴影），非匹配节点降低透明度（可关闭）；
   - 显示匹配数量与「上一个 / 下一个」；回车循环居中；
   - 清空搜索恢复全部节点。
5. **对象树定位到 ER**：
   - 对象树表 / 视图右键增加「在 ER 图中定位」；
   - 若当前连接 + 库 / 模式已有 ER 标签，切换过去并高亮目标节点；没有则打开新 ER 标签，加载后高亮；
   - 目标节点不存在（对象已删除）时提示，不打开空白 ER。
6. **规模与性能**：
   - 表数量 > 100 时显示提示「表较多，建议使用搜索定位」，界面保持可交互；
   - 搜索 / 高亮不全量重建 DOM；拖拽节流；
   - 加载失败时显示错误与重试，不冻结标签。
7. **只读**：ER 图只读，不提供关系编辑、建表、改结构入口；只读连接可正常使用。

### 验收标准

- [ ] 导出 SVG 后在浏览器打开：表名、字段名、主键标记、外键连线可见；中文与特殊字符正常。
- [ ] 导出 PNG：文件非空，尺寸与当前视图匹配，背景与主题一致。
- [ ] 拖动 3 个节点后关闭应用再打开：同连接 + 库 / 模式的节点位置恢复；切换另一库 / 模式布局独立。
- [ ] 重置布局：节点重新自动布局，重新打开后仍是重置后的位置。
- [ ] 搜索表名与字段名均能命中并高亮；回车在结果间循环并居中；清空后恢复。
- [ ] 对象树「在 ER 图中定位」：已有 ER 标签时切换并高亮；没有时新建 ER 标签并高亮；对象不存在时提示。
- [ ] 120 张表的 ER 图打开后窗口保持可交互，出现规模提示；搜索定位可用。
- [ ] 删除一张表后重新加载 ER：该节点消失，其他节点位置不变，新表自动放置且不重叠。
- [ ] localStorage 数据损坏（手工写入非法 JSON）时回退自动布局，不白屏。
- [ ] 只读连接可使用 ER 与导出；不产生任何数据库写入。

### 涉及文件与命令

**前端**

- 修改 `src/components/ErDiagram.vue`：工具栏、导出、搜索、高亮、布局读写、规模提示。
- 新增 `src/lib/er-layout.ts`：布局存取、版本键、坐标校验、增量放置。
- 新增 `src/lib/er-export.ts`：SVG 序列化、PNG Canvas 导出（不引入图形库）。
- 修改 `src/App.vue`：ER 标签激活与高亮参数；对象树定位事件。
- 修改 `src/components/ObjectBrowser.vue` / `src/components/ObjectCategoryGroups.vue`：右键「在 ER 图中定位」。
- 修改 `src/styles.css`：高亮、搜索、导出按钮样式。
- 无后端命令；不修改数据库。

### 风险与限制

- **localStorage 上限**：大量节点坐标可能接近 5MB；只保存坐标与版本，不保存字段 / 连线。超过上限时提示并停止保存，不阻塞使用。
- **表重命名**：坐标按表名保存，重命名后视为新节点；第一版不做重命名映射。
- **PNG 字体**：Canvas 绘制依赖系统字体；中文可能因字体缺失而回退，需在验收中检查。
- **导出体积**：超大 ER 的 SVG / PNG 可能很大；第一版不压缩。
- **不是模型设计器**：只读展示，不编辑关系、不生成迁移；结构变更仍走表设计器 / 结构同步。
- **性能**：力导向布局在数百节点时仍可能慢；第一版用规模提示与搜索定位缓解，不做 WebGL。

### 参考

- **DBX（Apache-2.0）**：`apps/desktop/src/lib/diagram/erDiagram.ts` 及 `packages/app-tests/erDiagram.test.ts`；仓库根目录 `dbx-er-diagram-architecture.html`（架构说明）。参考其 ER 布局与导出数据结构，不复制代码。
- **Navicat**：ER 图（ER Diagram）的布局、导出图片与在图中定位表；参考官方手册的 ER Diagram 章节。

---

## 附录 A：需求与既有 Backlog 的收口建议

交付本批需求后，建议按以下方式更新 `docs/BACKLOG.md`，避免重复提案：

| 既有项 | 建议状态 | 说明 |
|---|---|---|
| B-07 多语句脚本执行与逐条结果 | 已被 C-02 替代 | 分句、逐条结果、事务、只读预检均在 C-02 内交付 |
| B-08 表数据页批量编辑与事务提交 | 已被 C-03 替代 | C-03 额外包含多选、批量设置、筛选 / 排序 / 分页增强 |
| B-17 ER 图增强 | 已被 C-08 替代 | 导出、布局、搜索、定位均覆盖 |
| B-21 数据库 / 模式级结构导出 | 已被 C-01 替代 | C-01 的「仅结构」选项即该能力，且支持数据与进度 |
| B-34 备份与还原 | 导出由 C-01、还原由 C-02 覆盖 | 定时备份仍按 0.4.1 结论不排期 |
| B-35 数据传输向导 | 已被 C-05 替代 | 多表、建表、列映射、模式、进度均覆盖 |
| B-37 用户 / 权限管理与服务器监视 | 部分被 C-07 替代 | C-07 只交付进程 + 服务器状态；用户 / 权限保留待后续 |
| B-20 EXPLAIN 快捷入口 | 保留待办 | 下一批首个 P1，本批不做 |
| B-36 报表与计划任务 | 保留待办 / 研究 | 按 0.4.1 结论 |
| B-33 对象创建补全 | 保留待办 | 与 C-04 的表设计器不同：B-33 是视图 / 例程 / 触发器创建 |

## 附录 B：实现与验收约定

1. **一次只做一个需求**：完成 C-01 并验收后再开始 C-02；不要把 C-01 与 C-02 混在一个未验收的提交里。
2. **每个需求独立可回滚**：新增命令、对话框与菜单项应尽量内聚；不要为了共用而提前抽象未经验证的接口。
3. **测试**：
   - 纯函数（分句、DDL 生成、字面量、排序）必须有 Rust / TypeScript 单元测试；
   - 真实数据库回归使用 README 中的环境变量；没有实例时在交付说明写明未覆盖项；
   - 前端至少通过 `npm run check`；如项目后续引入前端测试框架，再补组件测试。
4. **错误文案**：必须包含「对象名 + 操作 + 数据库原始错误」；不要只显示 `error`。
5. **日志与隐私**：不把连接密码、完整 SQL（可能含敏感值）写入日志；导出文件路径可以显示，但不自动上传 / 打开。
6. **版本与发布**：本批全部需求不提升版本号；发布仍等用户明确说「更新app」或「合并代码」。
7. **参考与许可**：复制 DBX 代码时，文件头注明 `Source: https://github.com/t8y2/dbx (Apache-2.0)` 与原始路径；不要复制 Navicat 任何资源。

## 附录 C：本批参考资料

- DBX 仓库（Apache-2.0）：`https://github.com/t8y2/dbx`
  - 导出：`apps/desktop/src/components/export/`、`crates/dbx-core/src/data/database_export.rs`
  - SQL 文件执行：`apps/desktop/src/components/sql-file/`、`crates/dbx-core/src/data/sql_file_import.rs`
  - 数据传输：`apps/desktop/src/components/transfer/`、`crates/dbx-core/src/data/transfer.rs`
  - 结构对比：`apps/desktop/src/components/diff/`、`apps/desktop/src/lib/schema/schemaDiff.ts`
  - 管理面板：`apps/desktop/src/components/admin/`、`apps/desktop/src/lib/database/*ProcessList.ts`、`*ServerStatus.ts`
  - 表结构：`apps/desktop/src/components/structure/`
  - ER：`apps/desktop/src/lib/diagram/erDiagram.ts`
  - 文档：`docs/content/docs/database-export.cn.mdx`、`data-transfer.cn.mdx`、`schema-diff.cn.mdx`、`database-backup.cn.mdx`、`table-structure.cn.mdx`、`table-import.cn.mdx`、`data-grid.cn.mdx`、`query-editor.cn.mdx`、`sql-file.cn.mdx`
- Navicat 17 官方手册（功能与交互参考，不使用其代码 / 资源）：
  - `https://www.navicat.com/manual/pdf_manual/en/navicat_17/mac_manual/navicat_mac_en.pdf`
  - 相关章节：Dump SQL File / Run SQL File、Data Transfer、Structure Synchronization、Data Synchronization、Table Designer、Server Monitor、Process List、ER Diagram、Backup / Restore
- mydb 自身：
  - 架构与扩展：`docs/ARCHITECTURE.md`
  - 现有需求：`docs/BACKLOG.md`、`docs/PM-REQUIREMENTS-2026-11-24*.md`
  - 驱动抽象：`src-tauri/src/db/driver.rs`
  - 命令注册：`src-tauri/src/lib.rs`、`src-tauri/src/commands/mod.rs`
  - 前端 API：`src/lib/api.ts`、`src/lib/types.ts`

---

> 本文件只定义需求与验收，不包含实现代码。所有实现完成后，先更新 `CHANGELOG.md` 的 `[Unreleased]`，再按项目约定等待用户指令决定是否提交 / 打包 / 更新版本。