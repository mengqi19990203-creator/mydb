# mydb

轻量、跨平台的数据库客户端。当前是 **v0.10.0**：支持 SQLite / MySQL / PostgreSQL / Redis、SSH 隧道、MySQL 库级与 PostgreSQL 模式级对象树、统一的查询 / 表 / ER 多标签、表数据与结构视图、结构同步与行级数据同步、手动事务、新建表设计器、数据导入和执行历史。

技术栈参考 [DBX](https://github.com/t8y2/dbx)：**Tauri 2 + Rust + Vue 3 + TypeScript**，SQL 编辑器使用 CodeMirror 6，数据库访问使用 SQLx。

界面按 DBX 的暗色工作台风格对齐：自定义 Overlay 标题栏、命令工具栏、连接/对象搜索、表标签、数据/结构分段控件、紧凑数据网格和深色模态框；仅参考视觉语言，不复制其品牌资源。

版本变更记录见 [CHANGELOG.md](CHANGELOG.md)；版本号只在明确要求时提升。

## 当前功能

- **SQLite**、**MySQL / MariaDB**、**PostgreSQL** 与 **Redis** 连接
- 连接的新建、编辑、删除，配置持久化到应用配置目录
- MySQL 支持 host / port / user / password / 可选默认数据库 / SSL 模式；不填数据库时自动连接 `mysql` 系统库
- MySQL 对象浏览器按「数据库 → 表 / 视图 / 存储过程 / 函数 / 触发器 / 事件 / 序列 → 字段」展开；每类对象都会显示数量（包括 0）。连接建立后一次性读取全部数据库的对象与数量，切换数据库不会重连。
- PostgreSQL 对象浏览器按「数据库 → 模式 → 表 / 视图 / 物化视图 / 存储过程 / 函数 / 触发器 / 序列 → 字段」展开；可在模式间切换，SQL 编辑器中的非限定语句会跟随当前模式（`search_path`）
- **连接与分组**：左侧连接 / 分组右键菜单支持打开 / 刷新对象、新建查询、关闭连接、编辑、复制连接（可选复制已保存密码）、复制连接信息（URL / 主机:端口 / JDBC / DSN / 命令行 / 含密码 URL，含密码需二次确认）、只读模式、移动到分组、删除；分组支持新建 / 重命名 / 删除、连接全部 / 断开全部；分组元数据保存在 `groups.json`
- **只读连接**：连接可标记为只读，命令层拒绝写入（SQL 只允许 SELECT/SHOW/DESCRIBE/EXPLAIN/PRAGMA，禁止多语句与 `RETURNING` 绕过），Redis 写命令同样被拒绝
- **对象右键菜单**：表 / 视图 / 物化视图可打开数据、查看结构、在新标签打开、生成 SQL（SELECT / INSERT / UPDATE / DELETE / DDL，直接在查询标签打开）、复制 DDL / 名称 / 带引号限定名、导出、重命名、删除、刷新；表支持「复制为新表」（结构 + 全部数据）、清空表 / 截断表；视图支持「复制为新视图」和「快照为新表」（可选是否包含数据）；存储过程 / 函数 / 触发器 / 事件 / 序列 / 索引按当前驱动能力提供复制 DDL、重命名与删除；数据库和模式节点提供切换、新建表、刷新、复制名称；表右键支持「导入数据」（CSV / JSON）
- SQLite 对象树同样固定显示表 / 视图 / 索引 / 触发器及各自数量
- Redis 支持 host / port / 可选用户名 / 密码 / 数据库编号；左侧扫描键，右侧查看 string / hash / list / set / zset / stream 内容、删除键，并执行任意 Redis 命令；工作区和状态栏会显示当前数据库编号（如 `db01`）；主页面 `dbNN` 徽标可点击展开逻辑库下拉框，列表显示各库键数量，选中后立即切换并在重连后保持所选库；检测到 Redis Cluster 时只显示 `db00` 且不提供切库下拉框
- 数据库密码可选保存到系统钥匙串（macOS Keychain / Windows Credential Manager / Linux Secret Service）；不勾选时仅当前运行有效
- MySQL / PostgreSQL 支持 **SSH 隧道**：私钥或密码认证，首次连接自动写入 `~/.ssh/known_hosts`（TOFU），之后检测主机密钥变化
- 连接分组与颜色标记
- 左侧连接列表 + 分层对象浏览器（数据库 / 表 / 视图 / 存储过程 / 函数 / 字段）；支持搜索、方向键 / Home / End / Enter 键盘导航、右键菜单；点击 SQLite 索引节点会打开所属表的结构并高亮该索引
- CodeMirror SQL 编辑器，`⌘ / Ctrl + Enter` 运行；有选区时只执行选中的 SQL，没有选区时执行整个标签
- **统一标签栏**：查询、表 / 视图和 ER 图共用同一条标签栏并按打开顺序排列，可以同时打开多个表（每个表标签记录所属连接与数据库 / 模式，并可分别切换数据 / 结构）；切换标签会自动切换连接上下文（MySQL `switch_database`、PostgreSQL `switch_database` + `switch_schema`），目标连接有未提交事务且需要换库 / 模式时阻止切换；同一对象再次打开会复用已有标签，右键「在新标签打开」则总是新开一个；`Ctrl + Tab` 在全部标签间循环
- **查询标签**：每个标签记录所属连接和数据库 / 模式（例如 `mysql · app`），切换数据源不会改写已有标签；标签栏支持横向滚动、自动定位当前标签，SQL 和来源信息写入 localStorage，重启后恢复（查询结果和表 / ER 标签不保存）
- **手动事务**：SQL 标签可以「开始事务 / 提交 / 回滚」，事务期间固定一条连接并在编辑器上方显示「事务中 · 未提交」；关闭标签、切换连接、关闭连接或编辑连接会自动回滚；切换数据库 / 模式前要求先结束事务
- 结果表格：列名、行号、NULL 样式、执行耗时、截断提示；列宽拖拽与双击恢复、表头三态排序、表头右键（只看此列 / 隐藏此列 / 显示所有列 / 恢复自动列宽）、单元格右键复制单元格 / 整行 / 列名 / 全部结果（TSV）
- DDL / DML 执行，显示影响行数
- **表数据视图**：分页浏览、点击表头排序、WHERE 条件筛选、列宽拖拽、单元格右键菜单（复制 / 按值筛选 / 排序 / 删除）、双击单元格行内编辑、新增行、删除行（依赖主键）
- **表结构视图**：字段、索引、外键、DDL 预览与复制；可直接添加 / 编辑 / 删除字段与索引，方言 SQL 预览后执行（MySQL / PostgreSQL 支持修改类型、可空与默认值，SQLite 仅支持重命名字段）
- **ER 图**：按外键自动布局当前数据库 / 模式的表关系，支持拖拽节点、滚轮缩放与适应窗口，点击表直接打开数据；入口在顶部工具栏，以标签形式打开，同一来源复用同一 ER 标签
- **结构同步**：源端和目标端都可以选择同类型连接与数据库 / 模式；可对比全部表，或分别指定源表 / 目标表（同一数据库内也可以表对表；目标表可选择已有表，目标缺失时可沿用源表名生成建表 SQL），并可选是否包含索引与外键。生成迁移 SQL（新增表 / 字段 / 索引 / 外键），字段变更以注释建议给出，支持复制与保存，不自动执行
- **数据同步**：源端和目标端都可以手动选择数据库 / 模式和表（同一 MySQL / PostgreSQL 连接下可跨库 / 跨模式，也可以在同一数据库内同步不同表），按主键逐行生成 INSERT / UPDATE / DELETE（删除 → 更新 → 新增，单事务），展示行级差异并支持复制、保存，或确认后在目标库直接应用；两张表都必须有相同主键，二进制 / 数组字段暂不参与，每侧默认最多比较 5000 行（可调至 50000）
- **新建表设计器**：字段 / 类型 / 可空 / 主键（可复合）/ 自增 / 默认值 / 注释编辑，实时 DDL 预览，一键执行创建；MySQL 注释写入建表语句，PostgreSQL 建表后执行 `COMMENT ON`，SQLite 忽略注释
- **导入导出**：树级「导入数据」支持 CSV（可选表头）与 JSON（对象数组或 `{rows: [...]}`）；数据页导出当前页为 CSV / JSON；对象右键可导出整表或指定行数的 CSV / JSON / SQL INSERT / 结构 DDL，行数据流式写入文件，CSV 可选择是否包含表头，导出过程可取消
- **执行历史**：右侧「SQL 历史」面板，支持搜索、点击回填、清空，最多保留 100 条
- **中文菜单**：macOS 原生菜单栏（文件 / 编辑 / 视图 / 窗口 / 帮助）全部中文化，应用菜单、编辑菜单动作可直接使用；全局右键菜单同样为中文，输入区提供撤销 / 重做 / 剪切 / 复制 / 粘贴 / 全选，对象区提供新建连接 / 新建查询 / 新建表 / 刷新对象 / 执行历史
- 深色界面，原生窗口

## 开发

环境要求：

- macOS 13+ / Windows 10+ / Linux（Tauri 2 支持范围）
- Node.js 24+
- Rust 1.88+
- macOS 需要 Xcode Command Line Tools

安装依赖：

```bash
cd ~/Desktop/conding/mydb
npm install
```

启动开发版：

```bash
npm run tauri:dev
```

构建：

```bash
npm run tauri:build
```

类型检查 + Rust 编译检查：

```bash
npm run check
```

重新生成应用图标：

```bash
# Dock / 任务栏图标：只保留蓝色数据库圆柱
swift tools/make_app_icon.swift --cylinder-only /tmp/mydb-icon.png
npx tauri icon /tmp/mydb-icon.png
rm -rf src-tauri/icons/android src-tauri/icons/ios

# 标题栏 mark（圆柱 + 绿色光标）在 src/components/AppMark.vue
```

`tools/make_app_icon.swift` 用 CoreGraphics 绘制深色数据库圆柱；`npx tauri icon` 会生成 macOS / Windows / Linux 所需尺寸，桌面项目可删除自动生成的移动端目录。应用图标只保留圆柱，是为了在 Dock / 任务栏的 16–32px 下更清楚；标题栏的圆柱 + 光标由独立 SVG 渲染。

Rust 测试：

```bash
cd src-tauri && cargo test
```

PostgreSQL 驱动的真实回归测试默认跳过，设置 `MYDB_TEST_PG_*` 后会在目标库上创建并清理 `mydb_test` 模式（建议指向测试库）：

```bash
MYDB_TEST_PG_HOST=127.0.0.1 MYDB_TEST_PG_PORT=5432 \
MYDB_TEST_PG_USER=postgres MYDB_TEST_PG_PASSWORD=secret \
MYDB_TEST_PG_DATABASE=postgres \
cargo test --manifest-path src-tauri/Cargo.toml postgres
```

MySQL 真实回归默认跳过，只读取指定表的字段/索引/外键与 DDL，不写入数据：

```bash
MYDB_TEST_MYSQL_HOST=127.0.0.1 MYDB_TEST_MYSQL_PORT=3306 \
MYDB_TEST_MYSQL_USER=root MYDB_TEST_MYSQL_PASSWORD=secret \
MYDB_TEST_MYSQL_CONFIG_DB=mysql MYDB_TEST_MYSQL_DB=app MYDB_TEST_MYSQL_TABLE=users \
cargo test --manifest-path src-tauri/Cargo.toml mysql
```

> Rust 通过 rustup 安装在 `~/.cargo`，`npm` 脚本会自动加载 `~/.cargo/env`。如果你在终端直接运行 `cargo`，先执行 `. "$HOME/.cargo/env"`。

## 快速试用

仓库自带一个示例数据库，生成后直接在应用里打开：

```bash
sqlite3 examples/demo.db < examples/seed.sql
```

然后在连接对话框中选择 `examples/demo.db`。

## 使用

1. 点击左侧 `+` 新建连接。
2. 选择 MySQL、PostgreSQL、SQLite 或 Redis：
   - MySQL：填写 host、port、用户名、密码和 SSL 模式；数据库可留空，默认连接 `mysql` 系统库，之后在对象树里切换数据库。
   - PostgreSQL：填写 host、port、用户名、密码、SSL 模式和数据库（留空默认 `postgres`）；连接后在对象树里切换数据库与模式。
   - MySQL / PostgreSQL 需要经堡垒机访问时，勾选「通过 SSH 隧道连接」，填写 SSH 主机 / 端口 / 用户名，并选择私钥或密码认证。
   - SQLite：选择本地数据库文件。
   - Redis：填写 host、port、可选用户名和密码、数据库编号（默认 0）。
3. 点击「测试连接」，保存。
4. MySQL 对象树中点击数据库展开对象，单击表展开字段，点击表行右侧的「查看数据」按钮进入「表数据」标签：可分页浏览、点击表头排序、用 WHERE 输入条件筛选、拖拽表头边界调整列宽；双击单元格编辑，右键单元格可复制、按值筛选、排序或删除行。PostgreSQL 先在数据库下选择模式，再按同样方式浏览表、视图与物化视图。
5. Redis 连接会打开专用工作区：输入匹配模式扫描键，点击键查看内容，底部命令框可直接执行 `GET`、`HGETALL` 等命令。
6. 点击「表名 · 结构」标签查看字段、索引、外键和 DDL。
7. 对象面板顶部的 `+` 打开新建表设计器，填写字段并预览 DDL 后执行创建。
8. 顶部「SQL」标签用于自由查询，按 `⌘ / Ctrl + Enter` 或点击「运行」；编辑器上方可以「开始事务」，事务中用「提交」或「回滚」结束。
9. 顶部「历史」打开右侧「SQL 历史」面板，可搜索、点击回填到编辑器或清空。
10. 顶部「数据同步」按主键对比两张表；源端和目标端都可以选择数据库 / 模式与表，目标也可以选同一 MySQL / PostgreSQL 连接下的另一个库 / 模式。预览行级差异与 SQL，可复制 / 保存，或确认后在一个事务中应用到目标。
11. 对象右键「导入数据」支持 CSV / JSON；树中选择 SQLite 索引会打开所属表结构并高亮索引。

连接配置保存在 Tauri 应用配置目录：

- macOS：`~/Library/Application Support/com.mengqi.mydb/connections.json`
- Windows：`%APPDATA%\com.mengqi.mydb\connections.json`
- Linux：`~/.config/com.mengqi.mydb/connections.json`

密码不会写入 `connections.json`；勾选「保存密码到系统钥匙串」时写入系统凭据存储，否则只保留在当前进程内存中。

## 目录

```text
mydb/
├── src/                      Vue 前端
│   ├── components/           连接、对象树、SQL、结果、表数据、表结构、同步、建表、历史
│   ├── lib/                  Tauri invoke 封装与类型
│   ├── App.vue               应用外壳与状态编排
│   └── styles.css            深色设计令牌与布局
├── src-tauri/                Rust / Tauri 后端
│   └── src/
│       ├── commands/         Tauri 命令：连接、查询、库/表数据、导入导出、历史、Redis
│       ├── db/               驱动抽象、SQLite / MySQL / PostgreSQL 实现
│       ├── redis_client.rs   Redis 连接、SCAN、键详情与命令执行
│       ├── keychain.rs       系统钥匙串读写
│       ├── ssh.rs            SSH 隧道（russh，本地端口转发）
│       ├── history.rs        执行历史持久化
│       ├── models.rs         前后端共享数据结构
│       ├── storage.rs        连接配置持久化
│       └── lib.rs            Tauri 入口与命令注册
├── docs/ARCHITECTURE.md      架构与扩展说明
└── README.md
```

## 路线图

- **v0.1**：SQLite 垂直切片
- **v0.2**：MySQL 驱动、连接表单、系统钥匙串、SSL、连接分组与颜色
- **v0.3**：SSH 隧道
- **v0.4**：表数据分页、行内编辑、导入导出 CSV / JSON、执行历史
- **v0.5**：表结构视图、DDL 预览、新建表设计器；增量加入 Redis 与 MySQL 库级对象树
- **v0.6**：PostgreSQL 驱动，按「数据库 → 模式 → 表 / 视图 / 物化视图 / 存储过程 / 函数 / 触发器 / 序列 → 字段」浏览
- **v0.7**：ER 图、表结构 ALTER、结构差异对比与迁移 SQL、按主键的行级数据同步
- **v0.8**：SQL 编辑器手动事务、对象树键盘导航与索引聚焦、建表设计器默认值 / 复合主键 / 注释、树级 CSV / JSON 导入（当前）
- **后续**：达梦 / 金仓 / 神通等国产库的 JDBC 桥接、AI 助手、MCP

## 已知限制

- 当前支持 SQLite、MySQL / MariaDB、PostgreSQL 与 Redis。
- Redis 暂不支持 SSH 隧道与 TLS；MySQL 的存储过程 / 函数目前只列出名称，查看定义尚未实现。
- 查询标签按「连接 + 数据库 / 模式」记录来源，标签栏显示连接名和库 / 模式；SQL、标题和来源保存在 WebView localStorage（`mydb.queryTabs.v1`），重启后恢复，结果、事务状态和选中文本不保存。删除连接会同时移除其查询标签。切换标签时如果目标库 / 模式与当前连接不同，会先切换连接上下文；该连接有未提交事务时会阻止切换。
- MySQL 切换数据库不会重建连接：驱动持有当前数据库并在执行用户 SQL 前 `USE`，元数据与表操作使用全限定名；切换数据库会关闭当前表标签。SQL 编辑器中未限定库名的语句会作用于切换后的库。
- PostgreSQL 切换数据库会重连（PostgreSQL 不支持在同一连接内切换数据库）；切换模式不影响连接，编辑器中的非限定语句会跟随当前模式。
- 对象树的分类（表 / 视图 / 存储过程等）默认折叠，按需展开。
- 对象重命名 / 删除能力按驱动区分：MySQL 支持重命名表与视图；PostgreSQL 支持表、视图、物化视图、序列、索引、函数、存储过程（重载函数需唯一名称）；SQLite 仅支持重命名表。PostgreSQL 同名触发器存在于多张表时会拒绝直接删除。
- PostgreSQL 的数组、JSON/JSONB、NUMERIC、UUID、时间类型会映射为结构化或字符串结果；没有专用解码器的类型（如 INET、INTERVAL、自定义枚举）按文本显示。
- 行内编辑和删除依赖主键；没有主键的表只允许浏览和新增。PostgreSQL 写入时会按目标列类型显式转换参数。
- 新建表设计器支持复合主键、默认值（按 SQL 表达式填写）与字段注释；索引仍需创建后在结构页添加。
- 树级导入 CSV / JSON 逐行执行，不会自动开启事务；中途失败会保留已导入的行。JSON 只导入表中已有的字段，未知字段忽略，嵌套对象 / 数组按 JSON 文本写入。
- 表结构页的 ALTER 能力按驱动区分：MySQL 使用 `CHANGE COLUMN`（会重写该字段定义，列级字符集 / 排序规则不会保留）；PostgreSQL 修改类型时自动追加 `USING 列::新类型`；SQLite 只能重命名字段，新增 / 删除字段与索引可用，修改类型、可空性或默认值需要重建表，暂不支持。SQLite 添加字段时默认值必须是常量，NOT NULL 字段必须提供默认值。
- ER 图只展示当前库 / 模式下的表和外键关系（不含视图、物化视图）；字段与外键按表逐个读取，表数量很多时首次加载会较慢；布局只在当前会话内有效，不持久化。
- 结构同步可以对比全部表或指定表，并可选是否包含索引与外键；字段、索引、外键的“修改定义”只以警告列出，不自动生成修改语句，删除类变更（目标多出的字段 / 索引 / 外键 / 表）也只提示。源与目标必须属于同一驱动；目标可以是另一条同类型连接，也可以是同一 MySQL / PostgreSQL 连接下的另一个数据库 / 模式。SQLite 无法选择同一文件作为目标，需要另一条 SQLite 连接。MySQL 的建表 DDL 本身包含索引和外键，关闭对应选项不会从缺失表的 DDL 中剔除。
- 数据同步按主键匹配，源端和目标端都可以选择数据库 / 模式，两张表必须都有相同主键；只比较双方共有的非二进制 / 数组字段，目标多出的字段不写入。每侧默认读取 5000 行、最多 50000 行；读取被截断时会跳过新增 / 删除语句，避免把未读取到的行误判为差异。脚本使用未限定的表名，需在目标库 / 模式执行；同连接跨库 / 跨模式的两侧都通过独立连接读取，目标写入也在独立连接中执行，不会改动源连接当前选中的库 / 模式。应用到目标时整批语句在一个事务中执行，失败会回滚。
- CSV 导入按表头匹配列名，逐行 INSERT；数据页的 CSV / JSON 导出只导出当前页；对象右键导出可选全部行或指定行数，数据按游标流式写入文件，导出中可取消（取消后会删除未完成的文件）。
- 复制为新表会复制结构与数据：MySQL 使用 `CREATE TABLE ... LIKE`，PostgreSQL 使用 `LIKE ... INCLUDING ALL`，SQLite 额外复制显式索引；三者都不会复制外键、触发器、权限与注释。视图快照使用 `CREATE TABLE AS SELECT`，取消「包含数据」时只创建空表结构。
- 复制时目标名称已存在会直接返回数据库错误，不做自动改名。
- 只读连接由后端命令层强制拦截（`ensure_writable` + `db::sql_guard`），前端目前只禁用了「新建表」并显示锁标识；行内编辑、导入等入口仍可点击，但会收到「当前连接为只读模式，已阻止写入」。复制连接默认不复制密码；「复制连接信息 → 含密码 URL」需要二次确认。分组元数据在首次重命名/新建后写入 `groups.json`，由连接配置中的 group 字段引用。
- 表右键「生成 SQL」：SELECT 带 `LIMIT 100`；INSERT / UPDATE / DELETE 使用 `?`（MySQL/SQLite）或 `$n`（PostgreSQL）占位符，必填字段会给出注释；没有主键的表禁用 UPDATE / DELETE。「在新标签打开」会为表单独创建一个表标签；树级「导入数据」支持 CSV / JSON（逐行执行，无事务）。清空表使用 `DELETE FROM`（保留自增计数）；截断表在 MySQL 使用 `TRUNCATE TABLE`、PostgreSQL 使用 `TRUNCATE TABLE ... RESTART IDENTITY`、SQLite 使用 `DELETE FROM` 并清理 `sqlite_sequence`。
- SQL 导出使用结果页的类型映射：PostgreSQL 的数组会写成 JSON 文本、BYTEA 会写成 `<BLOB n bytes>` 占位符；需要迁移二进制数据时请使用数据库原生工具。
- SSH 私钥暂不支持 passphrase，也不使用 ssh-agent；请使用无口令私钥或密码认证。
- SSH 主机密钥首次连接采用 TOFU（自动写入 `~/.ssh/known_hosts`），之后检测到变化会拒绝连接。

## 参考与许可

架构思路参考 [DBX](https://github.com/t8y2/dbx)（Apache-2.0），本项目为独立实现，不包含 DBX 源码。项目使用 Apache-2.0 许可证，见 [LICENSE](LICENSE)。
