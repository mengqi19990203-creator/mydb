# mydb 架构

## 目标

做一个轻量、启动快、跨平台的 Navicat 类数据库客户端。第一版不追求覆盖所有数据库，而是把「连接 → 对象浏览 → SQL 执行 → 结果展示」做成可复用的垂直链路，再按驱动逐个扩展。

## 技术栈

| 层 | 选型 | 说明 |
|---|---|---|
| 桌面外壳 | Tauri 2 | 体积小、原生窗口、Rust 后端 |
| 后端 | Rust + Tokio | 连接、查询、元数据、凭据与隧道 |
| 数据访问 | SQLx / redis | SQLite / MySQL / PostgreSQL；Redis 使用 redis crate |
| 前端 | Vue 3 + TypeScript + Vite | 组合式 API，按组件拆分 |
| SQL 编辑器 | CodeMirror 6 | SQL 高亮、补全、快捷键 |
| 配置存储 | JSON（后续系统钥匙串） | 连接元数据与密码分开 |

## 分层

```text
Vue 组件
  └─ src/lib/api.ts (invoke)
       └─ Tauri commands (src-tauri/src/commands/)
            └─ DatabaseDriver trait (src-tauri/src/db/driver.rs)
                 └─ SqliteDriver / PostgresDriver / ...
                      └─ SQLx / 原生驱动
```

约束：

- 前端不直接接触数据库驱动，只调用 Tauri 命令。
- 命令层负责参数校验、连接缓存和错误转换；SQL 方言差异放在驱动实现里。
- 驱动实现统一返回 `QueryResult` / `TableInfo`，前端不感知具体数据库。
- 连接配置与密码分开存储；密码可写入系统钥匙串，`connections.json` 只保存 `savePassword` 标志。

## 数据流

1. 前端调用 `invoke("execute_query", { connectionId, sql, maxRows })`。
2. `commands::execute_query` 从 `AppState` 取连接池；没有则读取 `connections.json` 并创建驱动。
3. 驱动执行 SQL，把结果行转换为 `serde_json::Value`。
4. 前端用 `QueryResult` 渲染结果表格。

连接池按 `connectionId` 缓存在 `AppState.drivers`（`Arc<dyn DatabaseDriver>`），删除连接时同步移除。

## 驱动扩展

新增一个数据库类型：

1. 在 `src-tauri/src/db/` 新建驱动模块，实现 `DatabaseDriver`：
   - `test_connection`
   - `list_tables`
   - `execute`
2. 在 `commands/connections.rs` 的 `driver_for` 里按 `config.driver` 构造对应驱动。
3. 在 `ConnectionConfig` 增加该驱动需要的字段（host、port、user、database 等），并保持向后兼容的默认值。
4. 前端连接对话框按驱动切换表单字段，`src/lib/types.ts` 同步类型。

当前已有 SQLite、MySQL 与 PostgreSQL 实现，接口刻意保持最小；等表编辑与分页落地后再抽象分页、取消查询、事务和元数据。

MySQL 驱动要点：

- 使用 `MySqlConnectOptions`，host / port / user / password / database / `ssl_mode` 全部来自连接配置。
- `DECIMAL` 映射为 `BigDecimal` 字符串，`DATE` / `DATETIME` / `TIME` 映射为 chrono 字符串，`JSON` 保留结构，`BLOB` 显示字节数。
- 表列表来自 `information_schema.tables WHERE table_schema = ?`，`?` 绑定驱动内当前数据库；`list_database_objects` 用一次元数据扫描（tables / routines / triggers / events）返回所有数据库的对象，前端在连接后即可显示每个库的数量，打开库不再触发请求。
- 当前数据库保存在驱动的 `RwLock<String>` 中；`switch_database` 只校验并更新它，不重建连接池。`execute` 会在同一条池连接上先 `USE` 当前数据库，保证编辑器中的非限定语句作用于切换后的库；表/字段/分页/写入 SQL 全部使用反引号全限定名。

PostgreSQL 驱动要点：

- 使用 `PgConnectOptions`，数据库不可在同一连接内切换，所以 `switch_database` 会重建驱动；`ssl_mode` 映射到 `PgSslMode::{Disable, Prefer, Require}`。
- 驱动内部用 `RwLock<String>` 保存当前模式，所有元数据与数据 SQL 都用 `"schema"."table"` 全限定；`execute` 会先在同一条池连接上执行 `SET search_path TO "schema"`，保证编辑器里的非限定语句跟随对象树选中的模式。
- 对象列表来自 `pg_class` + `pg_namespace`（表 / 视图 / 物化视图 / 序列）、`pg_proc`（存储过程 / 函数）、`pg_trigger`（非内部触发器）；模式列表隐藏 `pg_*` 与 `information_schema`。
- 字段、索引、外键分别来自 `pg_attribute` / `pg_attrdef`、`pg_index` + `unnest(indkey)`、`pg_constraint` + `unnest(conkey, confkey)`；表 DDL 由元数据合成，视图和物化视图使用 `pg_get_viewdef`。
- 写入语句的占位符带 `::<format_type>` 显式转换（PostgreSQL 不会像 MySQL 一样隐式把 text 参数转成目标列类型）。
- 结果映射覆盖 bool、整数、OID、浮点、NUMERIC、文本、日期时间、UUID、JSON/JSONB、BYTEA 与常见数组；没有专用解码器的类型回退到文本格式（`PgValueRef::as_str`）。

## SSH 隧道

`src-tauri/src/ssh.rs` 使用纯 Rust 的 `russh`，在 `127.0.0.1` 上随机监听一个本地端口：

1. `driver_for` 检测到 `ssh_enabled` 时调用 `SshTunnel::open`，先做一次 SSH 探针认证，失败直接返回明确错误。
2. 探测成功后返回本地端口；MySQL 驱动连接 `127.0.0.1:<local_port>`。
3. 每个本地 TCP 连接对应一个 `direct-tcpip` 通道；隧道任务按需创建 SSH 会话并双向转发。
4. `SshTunnel` 保存在 `MySqlDriver` 内，驱动被缓存移除或应用退出时 `Drop` 中止监听任务。
5. 主机密钥首次连接采用 TOFU，写入 `~/.ssh/known_hosts`；之后检测到变化会拒绝连接。

SSH 密码与数据库密码分开存储（`connection:<id>:ssh`），勾选保存时写系统钥匙串。当前私钥不支持 passphrase。

## 数据库层级与库切换

MySQL 连接是服务器级连接，`database` 只是可选的默认库；留空时后端自动使用 `mysql` 系统库。`DatabaseDriver` 提供默认的 `list_databases`（SQLite 返回空列表），MySQL 实现从 `information_schema.schemata` 读取全部数据库；`list_tables` 同时返回表、视图、存储过程、函数、触发器、事件和序列（MariaDB），SQLite 则返回表、视图、索引和触发器。

前端对象树按「数据库 → 表/视图/存储过程/函数/触发器/事件/序列 → 字段」渲染，并为每个分类固定显示数量（包括 0）：

- 点击数据库节点时，如果它不是当前库，调用 `switch_database`；MySQL 只更新驱动内的当前数据库（不重连、不重建池），PostgreSQL 则重建驱动。切换成功后只切换连接上下文，已打开的表标签按自己的来源保留；MySQL 直接使用首次连接时预取的对象缓存，PostgreSQL 重新拉取当前库/模式的对象。
- 当前库展开后，单击表/视图调用 `list_columns` 读取字段并显示类型与 PK；每行右侧的「查看数据」按钮或双击行打开（或复用）该对象的表标签，右键「在新标签打开」则强制新建。
- 存储过程/函数目前只列出名称，不展开字段。
- 前端用 `databaseByConnection` 记住每个连接本次会话选择的库，切换连接或重建连接时同步；连接编辑/删除会清理该缓存。
- PostgreSQL 在数据库节点下再渲染一层「模式」节点：`list_schemas` 读取模式，点击其他模式调用 `switch_schema`（只更新驱动内的当前模式，不重连）；`schemaByConnection` 记住每个连接本次会话选择的模式。每次刷新对象前前端都会重新应用一次记住的模式，防止驱动被重建后回落到 `public`。

## Redis

`src-tauri/src/redis_client.rs` 使用 `redis` crate 的 multiplexed async connection：

- 连接参数来自 `ConnectionConfig` 的 host / port / 可选用户名 / 密码 / 数据库编号；`AppState.redis_clients` 按连接缓存 `MultiplexedConnection`，保存或删除连接时清除缓存。
- `redis_databases` 先通过 `INFO cluster`（失败时回退 `CLUSTER INFO`）检测 Redis Cluster；集群模式固定只有 db0，不执行 `SELECT`。独立实例才用 `CONFIG GET databases` 读取逻辑库数量（不可用时回退到 16），并用 `INFO keyspace` 解析各库的键数量。`redis_select_database` 在缓存的 multiplexed 连接上执行 `SELECT`，并把选择记录到 `AppState.redis_databases`，连接被关闭/重建后自动恢复同一个库。前端 Redis 工作区的 `dbNN` 徽标在存在多个逻辑库或处于集群模式时是下拉按钮，否则是普通徽标；集群模式的下拉框只包含 db00。
- `redis_scan_keys` 使用 `SCAN MATCH COUNT` 循环扫描（默认最多 500 个键），再用 pipeline 批量读取 `TYPE` 和 `TTL`。
- `redis_key_detail` 按类型读取 string（GET/GETRANGE）、hash、list、set、zset、stream，单值和集合都有截断上限。
- `redis_command` 支持带引号的命令行解析，结果按 RESP 值格式化；命令只通过 Redis 协议执行，不经过 shell。
- Redis 暂不支持 SSH 隧道和 TLS；密码沿用系统钥匙串的 `connection:<id>:db` 账号。

## 表数据操作

`TableDataView` 通过 `read_table_rows` 分页读取，默认 100 行；后端 `read_rows` 执行 `COUNT(*)` + `SELECT * ... [WHERE ...] [ORDER BY ...] LIMIT ? OFFSET ?`，返回列元数据、行、总数和分页信息。排序字段必须先通过 `list_columns` 校验；WHERE 条件拒绝分号，避免一次请求执行多条语句。

- **列宽**：前端按列名保存临时宽度，表头右侧拖拽；`colgroup` + `table-layout: fixed` 控制渲染，切表时重置。
- **右键菜单**：复制单元格/整行、按值筛选（前端生成带引号的 WHERE 条件）、升/降序、删除行；菜单 fixed 定位并在点击其他区域或 Esc 时关闭。
- **行内编辑**：双击单元格，前端用主键列构造 `keys`，后端生成 `UPDATE ... SET ... WHERE pk = ?`。
- **新增 / 删除**：同样按主键定位；没有主键的表禁用编辑和删除，只允许浏览/新增。
- **标识符**：表名和列名来自数据库元数据，后端按方言加引号（SQLite / PostgreSQL `"..."`，MySQL `` `...` ``），值全部走 SQLx 绑定参数；PostgreSQL 的占位符为 `$n` 并附带列类型转换。
- **导入 CSV**：Rust 侧读取文件，按表头匹配列名，逐行调用驱动 `insert_row`；空字段写 NULL。
- **导出**：数据页把当前页序列化为 CSV/JSON；对象右键的 `export_object` 在 Rust 侧按「全部行 / 指定行数」读取，生成 CSV / JSON / SQL INSERT / 结构 DDL 后直接写文件。JSON 导出结构为 `{table, columns, rows}`，与数据页导出一致。

## 表结构与建表

`table_structure` 聚合字段、索引、外键和 DDL：

- **SQLite**：`PRAGMA table_info`、`PRAGMA index_list` + `index_info`、`PRAGMA foreign_key_list`，DDL 来自 `sqlite_master.sql`。
- **MySQL**：`information_schema.columns`、`information_schema.statistics`、`information_schema.key_column_usage`，DDL 来自 `SHOW CREATE TABLE`。
- **PostgreSQL**：`pg_attribute` / `pg_attrdef`、`pg_index`、`pg_constraint`；表 DDL 由字段、主键和外键合成，视图 / 物化视图使用 `pg_get_viewdef`。

前端「表名 · 结构」标签展示这些信息并支持复制 DDL。新建表设计器在前端根据方言生成 DDL（SQLite 双引号标识符、MySQL 反引号 + `ENGINE=InnoDB`、PostgreSQL 双引号 + `GENERATED BY DEFAULT AS IDENTITY`），复用 `execute_query` 执行，因此也会进入执行历史。

表结构页同时提供 ALTER 编辑：`src/lib/alter.ts` 根据驱动生成字段与索引的 SQL，`ColumnEditorDialog` / `IndexEditorDialog` 展示预览并通过 `execute_query` 逐条执行。MySQL 用 `CHANGE COLUMN` 重建字段定义（保留 `AUTO_INCREMENT`，字符串默认值按类型补引号），PostgreSQL 把改名、改类型（`USING 列::类型`）、可空性和默认值拆成多条语句，SQLite 仅生成 `RENAME COLUMN`；新增字段统一使用 `ADD COLUMN`，删除字段使用 `DROP COLUMN`，索引使用 `CREATE [UNIQUE] INDEX` / `DROP INDEX`（MySQL 需要 `ON 表`）。执行中途失败时对话框会提示部分语句可能已生效。

## 连接、分组与只读模式

- `ConnectionConfig.read_only`（`#[serde(default)]`）持久化在 `connections.json`。`commands::ensure_writable` 在命令入口拒绝写入；`db::sql_guard::ensure_read_only_sql` 是独立的保守分类器（先剥离注释/字符串，拒绝多语句，只允许 SELECT / SHOW / DESCRIBE / EXPLAIN / PRAGMA，禁止 `RETURNING` 写语句、`INTO OUTFILE`、`FOR UPDATE`、`nextval`、`EXPLAIN ANALYZE` 与 dollar-quoted 语句），不复用 `is_read_query` 的 `RETURNING` 启发式。`execute_query`、行级增删改、CSV 导入、重命名/删除/复制/快照、表维护均加守卫；Redis 删除键直接拒绝，命令控制台使用读命令白名单（未知命令默认拒绝）。
- 连接右键命令：`close_connection`（仅清驱动缓存）、`duplicate_connection`（新 UUID，默认不复制密码，可选从钥匙串/会话复制）、`connection_info`（hostPort / url / urlWithPassword / jdbc / dsn / cli，Rust 生成并做 URL 编码，含密码格式需前端二次确认）、`set_connection_read_only`（保存后清除驱动缓存）。
- 分组元数据保存在 app_config_dir 的 `groups.json`（`{version, groups:[{name,color,order,collapsed}]}`，原子 temp+rename 写入）。`connections.json` 的 `group` 字段表示成员关系；重命名分组会同步所有成员连接，删除分组只删除元数据并把成员移到未分组。分组命令：`list_connection_groups`、`save_connection_group`、`rename_connection_group`、`delete_connection_group`、`move_connection_to_group`。

## ER 图

顶部工具栏的「ER 图」会以标签形式打开 `ErDiagram.vue`（同一连接 + 库 / 模式复用同一个 ER 标签），后端命令 `get_er_diagram` 调用驱动的默认 `er_diagram` 实现：先 `list_tables` 过滤出表，再对每张表调用 `list_columns` 与 `list_foreign_keys`（SQLite `PRAGMA foreign_key_list`、MySQL `key_column_usage` + `referential_constraints`、PostgreSQL `pg_constraint`），只保留目标表也在当前范围内的外键。前端用轻量力导向布局初始化节点，支持拖拽、滚轮缩放、空白平移和适应窗口，点击节点通过 `openTable` 打开数据页；切换数据库或模式时组件按 scope key 重新挂载，多个 ER 标签用 `v-show` 保留各自布局。

## 结构同步

顶部工具栏的「同步」打开 `SchemaSyncDialog.vue`。后端 `compare_schemas(source, target, source_database, source_schema, target_database, target_schema, tables, include_indexes, include_foreign_keys)` 要求源与目标属于同一驱动（Redis 除外）。`commands/context.rs` 负责解析两侧范围：

- 源范围可以通过 `resolve_source` 指向任意 MySQL 数据库或 PostgreSQL 数据库 / 模式；选择与连接当前范围相同时复用缓存驱动，否则用 `isolated_driver_for` 新建一条不进入缓存的独立连接。
- 目标为另一条同类型连接时，未指定库 / 模式则使用该连接缓存的驱动及其当前数据库 / 模式，指定了则同样通过 `isolated_driver_for` 建立独立连接；目标为源连接本身时（仅 MySQL / PostgreSQL）也走独立连接。源和目标范围相同是允许的，此时依赖不同的源表 / 目标表进行对比；同一张表对自身会被前端拦截。
- SQLite 同一文件不能作为目标；需要另一条 SQLite 连接。
- 未指定 `source_table` 时对比全部表；指定时 `source_table` / `target_table` 组成一对（目标表不存在时按目标表名生成 `CREATE TABLE`，不同名时 SQL 全部指向目标表）。源端缺少指定表会直接报错。

随后对每张表调用 `table_structure`（字段、索引、外键、DDL）+ `read_rows(limit=1).total` 生成 `SchemaSnapshot`，再由纯函数 `diff_snapshots` 按 `DiffOptions` 生成差异：

- 目标缺少整张表 → 使用源 DDL 生成 `CREATE TABLE`（DDL 不可用时按字段拼接）；`include_indexes` / `include_foreign_keys` 开启时补齐非主键索引和可添加的外键。MySQL DDL 自带索引 / 外键，SQLite 的自动索引会被过滤。
- 目标缺少字段 → 生成 `ALTER TABLE ... ADD COLUMN`；NOT NULL 无默认值、主键 / 自增字段会给出警告。
- 类型 / 可空 / 默认值不同 → MySQL 生成 `MODIFY COLUMN`、PostgreSQL 生成 `TYPE ... USING` / `SET NOT NULL` / `SET DEFAULT` 建议，并整体以 `-- ` 注释写入脚本，避免误执行；SQLite 只提示。
- 开启索引选项时，源索引缺失会生成 `CREATE [UNIQUE] INDEX`；定义不同、目标多出索引只警告。开启外键选项时，MySQL / PostgreSQL 的缺失外键生成 `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY`；SQLite 无法通过 ALTER 添加，只提示。
- 目标多出的表 / 字段 / 索引 / 外键只提示，不生成删除语句。

前端展示按表分组的差异、行数与警告，并支持复制或保存生成的 SQL；不自动执行任何语句。

## 数据同步

顶部工具栏的「数据同步」打开 `DataSyncDialog.vue`。后端 `compare_table_data(source, target, source_table, target_table, source_database, source_schema, target_database, target_schema, max_rows)` 要求源与目标属于同一驱动（Redis 除外）；源端和目标端都通过 `resolve_source` / `resolve_target` 解析数据库 / 模式，支持另一条同类型连接、同一 MySQL / PostgreSQL 连接下的不同库 / 模式，以及同一库内的不同表。两侧 `list_columns` 必须都有主键且主键字段一致；默认每侧读取 5000 行（上限 50000，单列主键时按主键排序）。纯函数 `build_data_diff` 生成差异：

- 按主键规范化（JSON 值的 canonical 字符串，整数与等价浮点视为相同）建立行映射。
- 源独有行 → `INSERT`；双方都存在 → 比较公共非二进制 / 数组字段，不同则 `UPDATE`；目标独有行 → `DELETE`。
- 语句按 删除 → 更新 → 新增 排序；脚本带 `BEGIN` / `COMMIT`，目标表多出的字段不写入，源表独有或 BLOB / BYTEA / BINARY / 数组字段会跳过并给出警告。
- 任意一侧读取被截断时，跳过对应的 `DELETE` / `INSERT`，避免把未读取到的行误判为差异。
- `apply_table_data_sync(target, target_database, target_schema, statements)` 先在命令层做只读检查；目标为同连接跨库 / 跨模式时同样通过 `isolated_driver_for` 建立独立连接，再由 `DatabaseDriver::execute_transaction` 在单个事务中逐条执行，失败整体回滚。SQLite / MySQL / PostgreSQL 三个驱动均实现，MySQL / PostgreSQL 会先在固定连接上执行 `USE` / `SET search_path`。
- 源表 / 目标表列表在跨库 / 跨模式时通过 `list_context_tables` 读取，模式列表通过 `list_context_schemas` 读取；`get_connection_context` 返回连接当前数据库 / 模式，用于对话框默认值和来源说明。

前端展示连接、库 / 模式和表选择、每侧行数上限、汇总计数、行级差异和 SQL 预览；支持复制、保存以及二次确认后应用到目标。

## 统一标签栏

`App.vue` 用三类标签共用一条标签栏：`tabs` 保存查询窗口（`QueryTab` 记录 `connectionId`、`database`、`schema`、SQL、选区和事务状态），`tableTabs` 保存表 / 视图标签（`TableTab` 记录对象、来源和当前的数据 / 结构模式与索引高亮），`erTabs` 保存 ER 标签。`tabOrder` 按打开顺序保存所有标签 id，`activeWorkspaceTabId` 指向当前标签；`activeTableTab` / `activeErTab` / `viewMode` 都由它派生，所以同一时刻只有一个标签处于激活状态。

打开表 / 视图时按「连接 + 库 / 模式 + 对象名 + 对象类型」查找已有表标签：存在则复用并切换数据 / 结构模式，否则新建；「在新标签打开」跳过复用强制新建。表标签的每个 pane 常驻并用 `v-show` 切换，因此分页、筛选和列宽在标签切换后仍保留。切换任何标签都会先调用 `activateTabSource` 对齐连接上下文（MySQL `switch_database`、PostgreSQL `switch_database` + `switch_schema`）；目标连接有未提交事务且需要换库 / 模式时阻止切换。关闭标签后激活相邻标签；删除连接、编辑连接或对象被删除后会清理对应标签。

标签栏的「+」固定在右侧，标签放在独立的横向滚动容器中；溢出时显示左右滚动按钮，新增 / 切换标签会自动滚动到可见区域。查询标签的标题、SQL 和来源写入 WebView `localStorage` 的 `mydb.queryTabs.v1`，启动后恢复；查询结果、事务状态、选区和表 / ER 标签不持久化，删除连接会移除其标签。

## 对象导出、复制与快照

- `export_object(connection_id, kind, name, format, path, limit, with_header, operation_id)`：`limit` 为空表示全部行；驱动通过 `export_rows` 用 `fetch` 游标逐行回调，Rust 侧把每行直接写入 `BufWriter`，不会把整表载入内存。CSV 可关闭表头，JSON 按 `{table, columns, rows}` 流式拼接，`sql` 格式先写表 DDL 再逐行写 `INSERT`（MySQL 使用反引号并转义反斜杠），`ddl` 格式只写对象 DDL。`operation_id` 对应 `AppState.export_cancels` 中的原子标志，`cancel_export` 置位后驱动在下一行返回错误，命令会删除未完成的文件。
- `table_maintenance(connection_id, table, action)`：`empty` 生成 `DELETE FROM`；`truncate` 在 MySQL 生成 `TRUNCATE TABLE`，PostgreSQL 生成 `TRUNCATE TABLE ... RESTART IDENTITY`，SQLite 退化为 `DELETE FROM` 并额外清理 `sqlite_sequence`（不存在时忽略）。
- 对象右键的「生成 SQL」由 `src/lib/generate-sql.ts` 生成 SELECT（带 `LIMIT 100`）/ INSERT / UPDATE / DELETE（DDL 复用 `getObjectDdl`），通过 `generate-sql` 事件在 App 中新建查询标签；INSERT / UPDATE / DELETE 使用 `?`（MySQL/SQLite）或 `$n`（PostgreSQL）占位符，无主键时拒绝生成 UPDATE / DELETE。MySQL 使用「库.表」、PostgreSQL 使用「模式.表」限定，SQLite 使用 `"main"."表"`；「复制限定名」复制带方言引号的限定名。
- `duplicate_object(kind, source, target)`：表使用 MySQL `CREATE TABLE ... LIKE`、PostgreSQL `LIKE ... INCLUDING ALL`、SQLite 复制 `sqlite_master` DDL 并单独复制显式索引，随后执行 `INSERT INTO target SELECT * FROM source`。DDL 改名由 `db::driver::rename_ddl_object` 完成；复制视图沿用同一逻辑，MySQL 会先去掉 `DEFINER=`。
- `snapshot_view_to_table(source, target, with_data)`：执行 `CREATE TABLE ... AS SELECT * FROM 视图`；不包含数据时 PostgreSQL 追加 `WITH NO DATA`，MySQL / SQLite 追加 `WHERE 0`。
- 数据或索引复制失败时会删除刚创建的表；目标名称已存在时直接返回数据库错误，不做自动改名。

## SQL 编辑器手动事务

SQL 标签可以开启交互式事务。后端 `commands/transaction.rs` 在 `AppState.transactions` 中按查询标签 id 保存 `TransactionEntry`（连接 id + `Arc<Mutex<Box<dyn DatabaseSession>>>`）：

- `begin_transaction(connection_id, session_id)` 调用驱动的 `open_session` 固定一条池连接，再执行 `BEGIN` / `START TRANSACTION`。
- `execute_transaction_query` 在同一个 pinned 连接上执行语句，返回与普通查询一致的结果并写入执行历史。
- `commit_transaction` / `rollback_transaction` 提交或回滚并释放连接；提交失败会尝试回滚。
- `close_transaction_session` 在标签关闭时回滚；连接保存、删除、关闭、只读切换和切换数据库 / 模式前会回滚该连接上的事务。
- 只读连接仍可开启事务，但每条语句都会经过 `sql_guard` 检查。
- `DatabaseSession` 由三个驱动实现：SQLite / MySQL / PostgreSQL 各自复用原有的行映射和 `USE` / `SET search_path` 上下文。MySQL DDL 和部分数据库操作会隐式提交事务，这是数据库自身语义。

前端在 SQL 编辑器上方显示「事务中 · 未提交」和提交 / 回滚按钮；切换标签时事务保持，切换连接或关闭标签自动回滚。

## 执行历史

`commands/query.rs` 在每次 `execute_query` 后把 SQL、连接、时间、耗时和错误写入 `app_config_dir()/history.json`，最多保留 100 条。前端历史面板可以回填 SQL、切换对应连接或清空历史。

## 前端结构

- `App.vue`：应用外壳、连接列表、当前选择、查询状态。
- `ConnectionDialog.vue`：新建 / 编辑连接，文件选择，测试连接。
- `ObjectBrowser.vue`：数据库 / 模式层级与对象搜索，复用 `ObjectCategoryGroups.vue` 渲染分类；`ObjectCategoryGroups.vue` 负责分类计数、搜索过滤和按需加载列信息。对象树支持方向键 / Home / End / Enter 键盘导航；点击 SQLite 索引会通过 `get_index_table` 找到所属表并在结构页高亮该索引。
- `QueryWorkspace.vue`：编辑器与结果面板布局。
- `SqlEditor.vue`：CodeMirror 实例，`Mod-Enter` 触发运行；有选区时把选中 SQL 作为 `run` 事件载荷，无选区时执行整个标签。选区文本同步到 `QueryTab.selectedSql`，这样点击工具栏「运行」时即使编辑器失焦也能继续执行选中部分；切换标签或替换文档会清空选区。
- `ResultGrid.vue`：结果表格、NULL 样式、截断提示；列宽拖拽 / 双击恢复、表头三态排序、表头右键（只看此列 / 隐藏此列 / 显示所有列 / 恢复自动列宽）、单元格右键复制单元格 / 整行 / 列名 / 全部结果。
- `RedisWorkspace.vue`：Redis 键扫描、键详情、删除与命令控制台。
- `ExportDialog.vue`：对象导出对话框，选择格式、行范围（全部行 / 指定行数）与 CSV 表头；导出期间工具栏右下角显示进度提示，可取消。
- `ErDiagram.vue`：ER 图画布，力导向布局、节点拖拽、缩放与平移。
- `SchemaSyncDialog.vue`：选择源 / 目标连接与库 / 模式、全部表或指定表、是否包含索引 / 外键，展示结构与行数差异并生成迁移 SQL。
- `DataSyncDialog.vue`：源 / 目标均可选择连接、库 / 模式和表，按主键比较行数据并生成可执行的增删改 SQL，可确认后事务应用到目标。
- `lib/sync-scope.ts`：结构 / 数据同步共用的源与目标范围解析，负责两侧数据库 / 模式列表、同一连接跨库 / 跨模式的默认目标、源范围提示和范围校验。
- `ColumnEditorDialog.vue` / `IndexEditorDialog.vue`：表结构 ALTER 对话框，实时预览方言 SQL 并执行。
- `StatusBar.vue`：状态、驱动与地址、当前库（PostgreSQL 含模式、Redis 显示 `dbNN`）、行数与耗时。
- `ImportDataDialog.vue`：树级导入 CSV / JSON，选择目标表与文件；CSV 支持表头开关，JSON 支持对象数组或 `{rows: [...]}`。
- `CreateTableDialog.vue`：字段、类型、可空、复合主键、自增、默认值、注释与实时 DDL 预览。
- `lib/clipboard.ts`：统一剪贴板读写。桌面端优先走 `tauri-plugin-clipboard-manager`（原生 NSPasteboard），浏览器环境降级到 `navigator.clipboard`，最后用隐藏 textarea + `execCommand("copy")` 兜底。

结果表格按列名长度和前 80 行样本估算初始列宽，列宽只保留在当前结果内，执行新查询后重置；表头点击按「升序 → 降序 → 取消」循环，NULL 始终排在最后。表头右键提供只看此列、隐藏此列、显示所有列和恢复自动列宽，隐藏状态同样只保留在当前结果内，结果栏会显示「已隐藏 N 列 · 显示全部」。复制使用 TSV，带制表符、换行或引号的单元格会按 CSV 规则转义；复制整行 / 全部结果只包含当前可见列。

样式集中在 `src/styles.css`，用 CSS 变量定义深色设计令牌，后续再加浅色主题。

WKWebView 里 `navigator.clipboard.writeText` 在 `await` 之后常因用户手势失效而拒绝，直接调用会表现为「复制失败」。因此所有复制入口（对象菜单、结构页、建表设计器、结果表格、表数据、Redis、错误信息）统一走 `src/lib/clipboard.ts`；桌面端使用 Tauri 剪贴板插件，能力声明见 `src-tauri/capabilities/default.json` 的 `clipboard-manager:allow-read-text` / `clipboard-manager:allow-write-text`。

## 菜单与右键菜单

macOS 原生菜单栏在 `src-tauri/src/lib.rs` 的 `build_menu` 中构建，包含「文件 / 编辑 / 视图 / 窗口 / 帮助」；需要前端处理的自定义项通过 `AppHandle::emit` 发送 `menu:*` 事件，`App.vue` 的 `bindMenuEvents` 监听后复用现有动作。编辑菜单使用 Tauri 预定义的撤销 / 重做 / 剪切 / 复制 / 粘贴 / 全选，交给系统处理当前焦点控件。

WebView 默认右键菜单由 `App.vue` 的全局 `contextmenu` 监听接管：输入框、文本域和 `contenteditable`（含 CodeMirror）显示中文编辑菜单，其他区域显示中文应用动作菜单。表格单元格的右键菜单由 `TableDataView` 自行处理并调用 `preventDefault`，全局监听通过 `event.defaultPrevented` 避让，避免叠加两层菜单。

## 存储

- 连接配置：`app_config_dir()/connections.json`，原子写入（临时文件 + rename）。
- 数据库文件：用户选择，不复制、不修改。
- 密码：勾选保存时写入系统钥匙串（`keyring` 4.x，macOS Keychain / Windows Credential Manager / Linux Secret Service），账号为 `connection:<id>:db`；不勾选时只存在于 `AppState` 内存，进程退出即丢失。
- SSH 密码使用独立账号 `connection:<id>:ssh`，与数据库密码分开。

## 安全

- 不执行连接配置里的任意命令。
- SQL 由用户显式触发，不自动运行。
- 导出/导入配置时后续要剔除密码，并提供加密选项。
- SSH 隧道后续单独实现，密钥密码与数据库密码分别存储。

## 里程碑

| 版本 | 内容 |
|---|---|
| v0.1 | SQLite 连接、对象列表、SQL 执行、结果表格 |
| v0.2 | MySQL、连接表单、钥匙串、SSL、分组与颜色 |
| v0.3 | SSH 隧道 |
| v0.4 | 表数据分页、行内编辑、导入导出、执行历史 |
| v0.5 | 表结构视图、DDL 预览、新建表设计器；增量加入 Redis 与 MySQL 库级对象树 |
| v0.6 | PostgreSQL 驱动与「数据库 → 模式 → 对象 → 字段」对象树 |
| v0.7 | ER 图、结构同步、按主键的行级数据同步、ALTER 表结构 |
| v0.8 | SQL 手动事务、对象树键盘导航 / 索引聚焦、建表设计器增强、树级 CSV / JSON 导入 |
| 后续 | 国产库 JDBC 桥接、AI、MCP |

## 参考

- [DBX](https://github.com/t8y2/dbx)：Tauri 2 + Rust + Vue 3 的同类实现，Apache-2.0。
- 本项目是独立实现，不复制 DBX 源码；仅在架构与交互上参考。
