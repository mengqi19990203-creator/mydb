# mydb 产品需求（2026-11-27 第五批：I-01～I-06）

> 文档编号：PM-REQUIREMENTS-2026-11-27-batch7
> 批次定位：E-01～E-05、F-01～F-05、G-01～G-05、H-01～H-05 与 MongoDB 首版均已完成，v0.12.0 已于 2026-11-26 发布。本批**不重复**已完成的查找、行详情、撤销 / 重做、Excel 导入、对象置顶、列冻结、结果筛选、外键跳转、剪贴板粘贴、执行范围、结果选区统计、筛选导出、结果图表、表视图配置、SQL 静态检查、SQL 对象导航、结果固定 / 对比、列重排 / 转置、列剖析、连接健康检查，以及 MongoDB 首版的连接、库 / 集合浏览、文档 CRUD、JSON 筛选 / 投影 / 排序、聚合、导入导出与索引管理。本批聚焦「MongoDB 从能浏览到能命令式操作」和「SQL 结果阅读 / 参数化执行」两条主线。
> 状态：I-01 已完成（2026-11-27），I-02～I-06 仍为**待办**。实现完成后再更新 `docs/BACKLOG.md` 与 `CHANGELOG.md` 的 `[Unreleased]`；本次不改动它们。
> 代码基线：v0.12.0（2026-11-26），git 工作区干净。实现前需按每项「现状核对」再核对一次实际代码；若前端已重构，以实际结构为准，但不得改变本文定义的用户可见行为。
> 竞品边界：DBX（Apache-2.0）与 DBeaver（Apache-2.0）只参考公开文档、README 与交互；Navicat 为专有软件，只参考公开产品页 / 帮助文档 / 手册中的功能描述。**不复制任何竞品的源代码、品牌图标、截图或专有文案**；mydb 保持独立实现与 Apache-2.0 许可。文中出现的命令语法与字段名是 mydb 自己定义的产品契约，不是竞品源码的搬运。
> 范围声明：本文件只写产品需求，不包含实现代码；不修改版本号、构建产物、连接配置或任何源代码。

## 0. 本批结论

### 0.1 一句话目标

让 MongoDB 能用熟悉的命令完成查询、聚合和写入，能直接连接 Atlas / 副本集，并能安全地做数据库与集合管理；同时让 SQL 结果可以按列格式化、快速定位列，并让重复查询可以带参数执行。

### 0.2 需求清单

编号按主题分组：I-01～I-03 是 MongoDB 工作流，I-04～I-06 是 SQL 结果与执行体验；**编号不等同于推荐实施顺序**，顺序见 0.3。

| 编号 | 标题 | 优先级 | 预估 | 主要依赖 | 去重关系 |
|---|---|---|---|---|---|
| I-01 | MongoDB 查询控制台（mongosh 风格命令 + Extended JSON） | P0 | 2–3 个开发会话 | `MongoWorkspace.vue`、`commands/mongo.rs`、`client_for`、`ensure_writable`、现有文档网格 | 新增；MongoDB 首版只有表单式浏览 / 聚合，没有命令控制台 |
| I-02 | MongoDB URI / SRV 连接 | P1 | 1–1.5 个开发会话 | `ConnectionConfig`、`ConnectionDialog.vue`、`connections.rs`、`mongo.rs`、钥匙串 | 新增；现有 Mongo 只支持单 host + port |
| I-03 | MongoDB 数据库与集合管理（创建 / 删除 / 重命名 / 统计） | P1 | 1–1.5 个开发会话 | `commands/mongo.rs`、`MongoWorkspace.vue`、`models.rs` | 新增；现有只支持新建 / 删除集合 |
| I-04 | 结果与表数据列显示格式器（日期 / 数字 / JSON / 脱敏 / 模板） | P1 | 1.5–2 个开发会话 | `ResultGrid.vue`、`TableDataView.vue`、`lib/grid-search.ts`、E-01 / F-02 / G-01 / H-05 | 新增；只改变显示，不改变数据与导出的原始值 |
| I-05 | SQL 参数化查询（执行前参数面板） | P1 | 1.5–2 个开发会话 | `SqlEditor.vue`、`QueryWorkspace.vue`、`App.vue`、复用 G-05 `tokenizeSql`、`sql-literal.ts`、`sql-statements.ts` | 新增；与 F-05 执行范围、事务、危险 SQL、历史分层 |
| I-06 | 列快速定位与空列隐藏（列搜索 / 隐藏全 NULL） | P2 | 0.5–1 个开发会话 | `ResultGrid.vue`、`TableDataView.vue`、G-04 表视图、E-01 查找 | 新增；G-04 是手动隐藏与命名视图，E-01 是值查找，本项是列导航与自动隐藏 |

> 「1 个开发会话」指一次连续实现：编码、`npm run check`、涉及 Rust 时 `cargo test --manifest-path src-tauri/Cargo.toml`，以及本文列出的可自动化验收项；真实 MongoDB / MySQL / PostgreSQL 手工回归可另排。MongoDB 当前没有真实服务器回归，本批必须把「门控真实回归」作为交付的一部分，不能只用模拟数据宣称完成。

### 0.3 推荐实施顺序与理由

**实施顺序：I-01 → I-02 → I-04 → I-05 → I-03 → I-06。**

| 编号 | 用户价值 | 实现风险 | 价值 × 风险判断 | 排序理由 |
|---|---|---|---|---|
| I-01 | 很高（Mongo 用户） | 中高 | 本批最大缺口 | MongoDB 首版只能靠表单筛选与固定聚合框，复杂查询 / count / distinct / 写入都要绕路；控制台是 Mongo 工作流的核心入口 |
| I-02 | 高（云 / 副本集用户） | 中 | 高性价比 | Atlas、云厂商与副本集普遍使用 URI / SRV；先解决「连不上」，控制台才有真实用户 |
| I-04 | 高（所有 SQL 用户） | 中低 | 覆盖面最广 | 日期、JSON、长数字、手机号 / 邮箱的显示是每天阅读结果的第一道摩擦；纯前端、无写入 |
| I-05 | 高（重复查询用户） | 中 | 价值高但需克制 | 参数化能明显减少手工拼 SQL；涉及词法边界、类型、危险 SQL、历史与只读，必须完整验收 |
| I-03 | 中高（Mongo 管理员） | 中 | 价值明确但非每日 | 创建 / 删除数据库、重命名集合、查看集合统计是运维动作，频率低于查询；可在 Mongo 主线稳定后做 |
| I-06 | 中（宽表用户） | 低 | 低风险补全 | 列搜索与隐藏全空列是很小的体验改进，和 I-04 同属列阅读，放在最后可复用 I-04 的列面板思路 |

- **I-01 最先做**：它是本批唯一会改变 MongoDB 使用方式的能力。先有控制台，后续 I-02 的 URI / SRV、I-03 的对象管理才有统一的验证入口；控制台复用现有 Mongo 驱动与底层命令，不新增数据库驱动。
- **I-02 第二**：连接串是云 MongoDB 的默认入口。放在 I-01 后面，是因为控制台可以在本地 host/port 连接上先验收；URI / SRV 只改变连接建立方式，不改变控制台契约。
- **I-04 第三**：在 Mongo 主线之后切到 SQL 显示层。它覆盖所有 SQL 用户、风险低、可独立交付；先做格式器，I-06 的列面板可以复用同一套列标识和 header 菜单。
- **I-05 第四**：参数化是执行链路改动，放在显示层之后，避免同时改「怎么显示」和「怎么执行」；完成后 SQL 侧的日常体验闭环。
- **I-03 第五**：Mongo 管理动作不是每日高频；在控制台与连接稳定后再做，能复用 `client_for` 与错误处理，风险更可控。
- **I-06 最后**：最小、最独立；即使只做列搜索或只做空列隐藏，也能单独交付，不影响前五项。

**若只能做一项**：做 **I-01**。MongoDB 首版的最大问题不是「少一个统计面板」，而是「不能像数据库客户端那样直接执行命令」；控制台一落地，临时查询、聚合调试、count / distinct、单条写入都能在一个界面完成。

**若只能做两项**：做 **I-01 + I-02**。控制台 + URI / SRV 构成「MongoDB 能连上、能查」的最小闭环；I-02 不依赖 I-01，但两者合并交付的感知最强。

**若只能做三项**：做 **I-01 + I-02 + I-04**。第三项优先给覆盖所有 SQL 用户的列格式器，而不是 Mongo 管理动作；I-03 可以顺延到下一批，不影响 I-01 / I-02 的完整性。

> 如果团队更希望「Mongo 工作一次做完」，也可以把 I-03 紧接在 I-02 之后执行；I-03 与 I-04 / I-05 没有代码依赖，调换不会互相阻塞。本文推荐顺序按「用户覆盖面 × 风险」排序，而不是按主题强行凑在一起。

### 0.4 竞品研究与差距结论（2026-11-27，第五批）

调研对象与边界：DBX（Apache-2.0，重点参考其公开 README 与 dbxio.com 文档）、DBeaver（Apache-2.0，公开官方文档）、Navicat（专有，只参考公开产品页 / 手册中的功能描述）。只比较用户可见能力与交互，不复制实现、品牌与文案。

#### 4.1 DBX（重点参考）

- **MongoDB 工作台**：对象树包含数据库、集合、索引与 GridFS bucket，显示文档数 / 大小等集合统计；支持创建、重命名、删除集合，删除数据库与索引；连接支持 `mongodb://` 与 `mongodb+srv://`，副本集、authSource 与 TLS 参数可写在连接串里。
- **MongoDB 查询标签**：支持 shell 风格的命令（`use <database>`、`db.collection.find(...)` 等），补全数据库、集合、字段、查询 / 更新操作符、聚合阶段和游标方法；`use` 切换查询上下文；结果网格支持动态字段与完整 JSON 视图，保留 Extended JSON 类型（ObjectId、日期、Long）。
- **MongoDB 查询结果编辑**：只有单条 `find` / `findOne`、无投影且结果包含 `_id` 时，网格可编辑；支持修改字段、删除行，并在保存前预览待执行的 `updateOne` / `deleteOne`；聚合、带投影的查询和多命令脚本只读；查询结果不能新增行。
- **MongoDB GridFS**：浏览 / 创建 / 删除 bucket，上传 / 下载 / 预览 / 删除文件，查看文件名、大小、上传时间、chunks 与 metadata。
- **数据表格**：列宽 / 列顺序 / 左右冻结 / 显示隐藏 / 按列名或注释搜索；「隐藏全 NULL 列」与「隐藏全 NULL / 空串列」两个独立开关（不把 `0`、`false`、空白串当作空；空结果不触发隐藏）；列格式器支持全局或按列的日期时间格式、JSON 路径提取、值脱敏、自定义显示模板和数字 / 布尔呈现，**只改变显示，不修改数据**。
- **查询编辑器**：执行前检测参数占位符并弹出参数输入；支持 `?`、`:name`、`${name}`、`#{name}`、`@name` 与 `@set` 变量；参数类型可为字符串、数字、布尔、NULL 或原始 SQL；查询历史按连接保存，保留数量可配置；SQL Library 支持文件夹与快速打开。
- **连接与配置**：配置导出 / 导入支持 AES-256-GCM + PBKDF2 加密，可选择包含密码；连接支持 SSH、代理与自动重连。
- 来源：<https://github.com/t8y2/dbx>、<https://dbxio.com/en/docs/mongodb>、<https://dbxio.com/en/docs/data-grid>、<https://dbxio.com/en/docs/query-editor>、<https://dbxio.com/en/docs/config-export>。

#### 4.2 DBeaver（公开官方文档）

- **MongoDB**：Enterprise Edition 支持 MongoDB 的 schema 浏览、数据查看器与 SQL / JavaScript 查询；JavaScript 查询支持 MongoDB 2 / 3 的常用写法与一部分 mongo shell 命令（例如 `db.test_col.find().toArray()`），并对当前选中的数据库执行。
- **SQL 参数**：SQL 编辑器支持命名参数 `:name`，执行时弹出 Bind parameter(s) 对话框；匿名 `?` 参数需要在 SQL 编辑器设置中启用；支持 `@set` 变量与 Variables 面板。
- **数据编辑器**：Grid / Record 视图、列显示与顺序控制、数据格式设置；Calc 面板对选中单元格做 Count / Distinct / Sum / Average / Min / Max 等统计。
- 来源：<https://dbeaver.com/docs/dbeaver/23.1/MongoDB/>、<https://dbeaver.com/docs/dbeaver/SQL-Execution/>、<https://dbeaver.com/docs/team-edition/26.0/desktop/Data-Editor/>、<https://dbeaver.com/docs/dbeaver/Calc-Panel/>。

#### 4.3 Navicat（公开产品页 / 手册）

- **Navicat for MongoDB**：集合 / 文档浏览、可视化 Find Builder 与 Aggregate Builder、代码补全 / 片段、执行计划可视化、MongoDB 4 事务支持、MongoDB 用户与角色管理。
- **数据表格**：表 / 集合的 Data Profiling（列质量、分布、统计）、筛选后导出、表视图配置；查询结果支持固定 / 对比快照。
- 来源：<https://www.navicat.com.cn/products/navicat-for-mongodb.html>、<https://www.navicat.com/manual/pdf_manual/en/navicat_18/linux_manual/navicat_en.pdf>、<https://www.navicat.com/en/products/query-editing>。

#### 4.4 全量差距表与本批决策

| 能力 | 竞品参考 | mydb 现状（2026-11-26 / 27 代码核对） | 现状标注 | 本批决策 |
|---|---|---|---|---|
| MongoDB 命令控制台 | DBX 查询标签 + shell 命令 + 补全；DBeaver JS / 部分 shell | `MongoWorkspace.vue` 只有 `documents / aggregation / indexes` 三种模式；`mongo.rs` 全部是结构化命令，没有 shell 解析器、`use` 上下文、命令补全或控制台历史 | **完全缺失** | **I-01** |
| MongoDB URI / SRV | DBX `mongodb://` / `mongodb+srv://`；DBeaver MongoDB URL | `ConnectionConfig` 只有 host / port / authSource / TLS；`mongo_uri()` 固定拼 `mongodb://`；不支持多 host、副本集、SRV 与任意 URI 参数 | **完全缺失** | **I-02** |
| MongoDB 数据库 / 集合管理 | DBX 删除数据库、创建 / 重命名 / 删除集合、集合统计、索引管理（部分 GridFS） | 已有新建 / 删除集合、索引创建 / 删除；没有删除数据库、重命名集合、集合统计；数据库选择器只能切换 | **已有雏形（仅集合增删）** | **I-03** |
| MongoDB GridFS | DBX 完整 GridFS 工作区 | 无 bucket / 文件浏览、上传、下载、预览 | **完全缺失** | **不做，见 0.6；留到后续 Mongo 专项** |
| MongoDB 查询结果网格编辑 | DBX 单 find / findOne 无投影可编辑，待执行 `updateOne` / `deleteOne` 预览 | 文档编辑走 JSON 对话框，保存即替换整条文档；没有网格编辑、没有待提交预览 | **部分覆盖（JSON 编辑）** | **不做，见 0.6；I-01 结果先只读** |
| MongoDB 用户 / 角色 | DBX 公开文档未记录；Navicat 有完整用户 / 角色管理 | 无 Mongo 用户 / 角色面板；关系库已有只读的用户 / 角色页 | **完全缺失（Mongo）** | **不做，见 0.6；安全专项再做** |
| MongoDB 事务 | DBX 公开文档未记录；Navicat 支持 MongoDB 4 事务 | 无事务会话、无 commit / rollback；所有 Mongo 写入立即生效 | **完全缺失** | **不做，见 0.6；需要副本集 / 分片集群与独立会话生命周期** |
| MongoDB explain / profiler | DBX 发布说明提到 `find().explain()`；Navicat 有执行计划可视化 | 聚合 / 查询只有 30 秒超时与结果；没有 explain、没有慢查询面板 | **完全缺失** | **不做，见 0.6；I-01 第一版不支持 `.explain()`** |
| SQL 参数化查询 | DBX `?` / `:name` / `${}` / `#{}` / `@` / `@set`；DBeaver `:name` / `?` | 执行链路直接发送最终 SQL；G-05 tokenizer 已能识别 `:name`、`?` 等 parameter token，但没有参数面板、替换、预览或类型 | **完全缺失** | **I-05** |
| 列显示格式器 | DBX 日期时间 / JSON 路径 / 脱敏 / 模板 / 数字布尔呈现 | `formatGridCell()` 只做 NULL / 对象 JSON / String；没有按列格式化；复制 / 导出使用原始值 | **完全缺失** | **I-04** |
| 列搜索 / 隐藏全空列 | DBX 按列名或注释搜索、两个空列开关 | `ResultGrid` 与 `TableDataView` 只能逐列手动隐藏 / 显示全部；没有列搜索，没有自动隐藏空列 | **完全缺失（自动隐藏与列搜索）** | **I-06** |
| SQL Library 文件夹 / 快速打开 | DBX 文件夹、文件树、Quick Open | 已有 SQL 片段库（标签、驱动过滤、JSON 导入导出）与 SQL 文件打开 / 保存；没有嵌套文件夹与 Quick Open | **已有替代方案** | **不做，见 0.6** |
| 连接配置加密导出 | DBX AES-256-GCM + PBKDF2，可选包含密码 | 已有 mydb / DBeaver / Navicat 连接导入导出，但**永不包含密码与 SSH 凭据** | **已有更保守方案** | **不做，见 0.6** |
| 跨驱动数据传输 / 逐列映射 | DBX 跨引擎传输 + 类型转换；DBeaver Data migration | 已有同驱动、同名字段的多表传输与行级数据同步；跨驱动明确拒绝 | **已有雏形（同驱动）** | **不做，见 0.6；架构专项** |
| AI SQL 助手 / MCP / Docker / CLI | DBX 内置 AI、MCP Server、Docker / Web、CLI | 无 AI、无 MCP、无 Web / CLI；桌面本地客户端定位 | **不在定位内** | **不做，见 0.6** |
| 连接代理 / 多级 SSH | DBX 支持 SSH 与代理；mydb 只有单级 SSH 隧道 | 已有 SSH 隧道、TLS、主机密钥 TOFU；没有 HTTP / SOCKS 代理、多级跳板 | **已有单级方案** | **不做，见 0.6** |
| 多工作表 XLSX / SQL UPDATE 导出 | DBX 支持多 sheet、INSERT / UPDATE | 已有 CSV / JSON / Markdown / SQL INSERT / XLSX 单表导出；对象导出支持全部 / 指定行 | **已有主力方案** | **不做，见 0.6** |
| 查询历史保留策略 | DBX 按连接、200–无限可配置 | 已有 SQL 历史（搜索 / 回填 / 清空，最多 100 条，全局） | **已有基础方案** | **不做，见 0.6** |
| 数据对比 | DBX Data compare；DBeaver Data compare | 已有结果固定 / 对比、行级数据同步与结构同步 | **已有替代方案** | **不做** |

**结论**：mydb 的关系库能力与数据工作流已经非常完整，当前最明显的短板集中在 MongoDB（命令式查询、云连接、对象管理），以及 SQL 结果层的「按列理解 / 定位」和「参数化执行」。本批六项都落在现有 Mongo 工作区、结果网格、表数据、SQL 编辑器与连接配置上；I-01 / I-02 / I-03 以新增 Mongo 命令与连接字段为主，I-04 / I-05 / I-06 为前端为主，均不新增数据库驱动、不引入外部服务。

### 0.5 与已实现 / 已提案需求的去重

| 已实现 / 已提案 | 与本批的关系 |
|---|---|
| MongoDB 首版（v0.12.0） | I-01 增加命令控制台；I-02 增加 URI / SRV；I-03 增加数据库与集合管理。三者都不重写现有的文档浏览、JSON 筛选 / 投影 / 排序、聚合、导入导出与索引管理。 |
| E-01 结果 / 表数据查找 | E-01 查「值」；I-06 查「列名 / 列注释」并自动隐藏空列；I-04 的格式器会改变显示文本，E-01 的查找在 I-04 后按显示文本匹配（无格式器时与现状一致）。 |
| E-02 行详情 / 表单视图 | E-02 一次看一行；I-04 只改变网格显示，行详情仍显示原始值；I-06 只影响列可见性。 |
| E-03 待提交撤销 / 重做 | I-04 / I-06 是显示配置，不进入待提交、不触发撤销历史；I-05 与表数据编辑无关。 |
| E-04 Excel 导入 | 本批不改变导入向导；I-01 的 Mongo 控制台不处理 xlsx。 |
| E-05 对象置顶 | I-03 的 Mongo 数据库 / 集合管理不涉及关系库对象树置顶。 |
| F-01 列冻结 | I-04 格式器不改变冻结规则；I-06 的列面板必须与冻结列、横向滚动、隐藏列对齐。 |
| F-02 结果本地筛选 | I-04 的显示格式不改变筛选语义：结果筛选仍按原始值比较；I-06 的自动隐藏列仍可作为筛选条件（与 G-04 隐藏列规则一致）。 |
| F-03 外键跳转 | 与本批无关；I-05 的参数替换不改变外键 WHERE 生成。 |
| F-04 剪贴板粘贴 | 与本批无关；I-06 不新增粘贴语义。 |
| F-05 执行范围 | I-05 的参数面板只作用于实际执行的 SQL（选区 / 当前语句 / 整个标签）；不改变 F-05 的默认范围与「第 N / M 条」逻辑。 |
| G-01 选区统计 | I-04 不改变选区统计，统计仍用原始值；I-06 的自动隐藏列不影响选区统计的列集合（只影响可见列）。 |
| G-02 表数据筛选导出 | I-04 的导出默认仍用原始值；I-06 自动隐藏列与手动隐藏列一样不进入导出，导出对话框需明确提示隐藏数量；不改变 G-02 的范围与流式读取。 |
| G-03 结果图表 | I-04 的显示格式不改变图表读取的原始值；I-06 只影响可见列选择。 |
| G-04 表视图配置 | I-06 的自动隐藏是**会话级叠加**，不写入 `TableViewProfile.hidden`；手动隐藏仍由 G-04 持久化；应用命名视图时自动隐藏重置为关闭。 |
| G-05 SQL 静态检查 | I-05 复用 G-05 tokenizer 的 `parameter` token，检查时把占位符当作不透明表达式，不新增误报；I-05 不改变检查范围与错误码。 |
| H-01 SQL 对象导航 | I-05 的参数占位符不应被 H-01 当成表 / 列；点击占位符不导航。I-04 不改变导航。 |
| H-02 结果固定 / 对比 | I-04 的格式器可按列名作用于固定结果视图，但固定快照内容不变；对比使用原始值。I-06 的列面板对固定结果视图同样可用，自动隐藏只作用于当前视图。 |
| H-03 列重排 / 转置 | I-04 的格式器跟随列标识而不是位置；列重排后仍正确；转置视图可按同一规则显示，但不改变原始值。I-06 的列面板与列顺序、冻结块对齐。 |
| H-05 列剖析 | 剖析继续使用原始值；I-04 / I-06 不改变统计结果。 |
| B-02 结果导出 | I-04 默认导出原始值；新增「复制显示值」是可选入口，不改变导出格式与原子写。 |
| B-16 值查看器 | 值查看器继续显示原始值 / 原始 JSON；I-04 的「脱敏」不是安全边界，查看器仍可看到原文。 |
| B-25 命令面板 | I-01 可在命令面板增加「MongoDB 查询控制台」入口（可选）；I-05 可增加「编辑参数」但不新增全局快捷键；I-06 不强制加入。 |
| B-27 标签管理 | I-01 的 Mongo 控制台是 Mongo 工作区内的模式，不新增 App 级查询标签；不改变标签拖动 / 固定 / 溢出。 |
| B-29 分屏 / 独立窗口 | I-01 第一版不做 Mongo 分屏 / 独立窗口；I-05 的参数值按查询标签独立，分屏与独立窗口互不共享。 |
| B-35 数据传输 | 本批不做跨驱动 / 逐列映射，现有同驱动传输不变。 |

### 0.6 本批明确不做（附评估结论）

| 方向 | 为什么不放进本批 |
|---|---|
| MongoDB GridFS | DBX 有完整 bucket / 文件管理，但它是独立的文件存储工作流：需要 bucket 列表、上传 / 下载、chunk 处理、预览、权限与存储占用提示。它和「数据库查询」不是同一频次，放在后续 Mongo 专项更合适。 |
| MongoDB 查询结果网格编辑（pending `updateOne` / `deleteOne`） | DBX 只在单条 `find` / `findOne`、无投影、结果含 `_id` 时可编辑；需要行身份、字段路径、类型保持、投影 / 聚合只读判断、待执行语句预览与撤销语义。I-01 第一版结果只读；已有 JSON 文档编辑对话框继续保留。 |
| MongoDB 用户 / 角色管理 | DBX 公开 MongoDB 文档没有把它列为工作台能力；Navicat 有完整用户与角色管理。mydb 关系库已有只读用户 / 角色页，但 Mongo 的 RBAC 涉及 `usersInfo`、`rolesInfo`、自定义角色、权限继承与安全审计。先不做「半套」授权 / 回收；如要做，另开安全专项，第一版至少只读。 |
| MongoDB 事务 | Navicat 支持 MongoDB 4 事务，但 DBX 公开文档没有把它列为工作台能力。事务需要副本集 / 分片集群、session 生命周期、跨集合写、commit / abort、与连接切换 / 关闭标签的交互；当前 Mongo 写入全部即时生效。单独做会话与事务专项，不在本批混入。 |
| MongoDB explain / profiler / 可视化聚合构建器 | DBX 发布说明有 `find().explain()`，Navicat 有执行计划与 Aggregate Builder。第一版控制台先保证命令解析、结果与写安全；explain、慢查询面板和拖拽式聚合构建器留到下一批。 |
| 完整 JS / mongosh 语法 | I-01 只接受 **Extended JSON** 参数（键名与字符串双引号），不支持未加引号的键、单引号、`ObjectId("...")`、`ISODate(...)`、正则、函数、`getSiblingDB`、`bulkWrite`、`runCommand`、`$out` / `$merge`。完整 JS 解析器与沙箱会显著增加体积和误执行风险；先用严格 JSON 保证可预测。 |
| 跨驱动数据传输 / 逐列映射 | 需要类型映射矩阵、目标 DDL、表达式 / 常量转换、dry-run 与跨驱动事务语义；前几批已明确列为架构专项，本批不重复。 |
| 连接配置加密导出 / 导入（含密码） | DBX 使用 AES-256-GCM + PBKDF2；mydb 当前的安全策略是导出永不包含密码、SSH 密码与私钥口令。加入加密导出需要新的密码学依赖、口令管理、迁移与泄露风险评估，单独做安全设计。 |
| SQL Library 嵌套文件夹 / Quick Open | mydb 已有 SQL 片段库（标签、驱动过滤、JSON 导入导出）和 SQL 文件打开 / 保存；DBX 的文件夹与 Quick Open 是组织方式差异，不是当前高频缺口。 |
| 多工作表 XLSX / SQL UPDATE 导出 | mydb 已有 CSV / JSON / Markdown / SQL INSERT / 单表 XLSX，以及对象级全部 / 指定行数导出。多 sheet 拆分与 UPDATE 导出是格式增强，优先级低于 Mongo 与参数化。 |
| AI SQL 助手 / MCP / Docker Web / CLI | 与当前「本地轻量桌面客户端」定位冲突；涉及外部模型服务、凭据、隐私与打包体积，不属于本批。 |
| HTTP / SOCKS 代理、多级 SSH 跳板 | 已有单级 SSH 隧道、TLS、主机密钥 TOFU；代理与多级跳板需要新的网络层与凭据链，单独评估。 |
| 全表数据剖析 / 服务端聚合 / 表数据转置 | H-05 已明确只分析已加载结果与表数据当前页；I-04 / I-06 不改变这一边界。 |
| 列格式器的全局默认 / 按类型默认 | 第一版只做「按列」规则；全局默认需要类型推断、优先级与更多冲突规则，留到后续增强。 |
| 固定结果持久化 / 历史结果持久化 | 沿用 H-02 的会话内存策略，不把结果行写入 localStorage。 |
| 自动重放 Mongo 写命令 / 自动重试 | 断线与超时后写命令可能已在服务端生效；I-01 不自动重放、不自动重试，失败由用户决定是否重跑。 |

### 0.7 全批次通用约束

1. **不新增数据库驱动，不引入外部服务**。I-01～I-06 只使用现有 SQLite / MySQL / PostgreSQL / MongoDB / Redis 能力；I-01 允许新增官方 CodeMirror 语言包（`@codemirror/lang-javascript`，MIT）与直接声明已作为传递依赖安装的 `@codemirror/autocomplete`（MIT），不引入 UI 框架、图表库或数据库 SDK。若实现时能复用现有编辑器而不新增依赖，也可以不新增。
2. **独立实现**。不得复制 DBX / DBeaver / Navicat 的源代码、品牌图标、截图、专有文案或命令模板；只能参考公开文档描述的行为，用 mydb 自己的命名、样式与代码实现。命令语法、错误文案、示例数据全部自行编写。
3. **分层约束**：Vue 组件不得直接访问数据库；所有数据库操作走 `src/lib/api.ts` 的 `invoke` 封装。MongoDB 写入必须在 Rust 命令层经过 `ensure_writable`；SQL 只读连接必须在替换参数后的 SQL 上经过现有 `ensure_query_allowed` / `sql_guard`。
4. **只读连接不得绕过**：I-01 的写命令、I-03 的建库 / 删除 / 重命名、I-05 的原始 SQL 参数都必须在后端被只读拦截；前端只做提示，不作为安全边界。
5. **所有新增状态都有上限**：Mongo 控制台编辑器 100 KB、单次最多 20 条命令、单条结果最多 500 个文档、`insertMany` 单次最多 1,000 个文档、命令超时 30 秒、控制台历史 50 条；URI 长度最多 2,048 字符；列格式器每张表最多 50 条规则、全局最多 200 个 scope；SQL 参数每次最多 50 个、单个值最多 10 KB、SQL 最多 100 KB；列搜索 / 空列分析最多 10,000 行 × 500 列。超过上限必须提示，不得静默截断或卡死。
6. **写操作必须显式确认**：I-01 的空筛选 `updateMany` / `deleteMany` / `findOneAndDelete` 等、I-03 的删除数据库 / 重命名集合必须先预览并二次确认；不得因为「用户已经点了运行」就直接执行不可恢复操作。I-05 的原始 SQL 参数必须显式标记并确认。
7. **不泄露凭据**：I-02 的连接串密码、查询参数中的敏感值不得写入 `connections.json`、localStorage、日志、错误消息、导出文件或复制信息（除非用户明确选择「含密码」并二次确认）；I-05 的参数值不得写入历史 / localStorage；I-04 的脱敏只是显示，不是安全边界，界面必须明确说明。
8. **不改变既有复制 / 导出 / 排序 / 筛选语义**：I-04 默认复制 / 导出原始值，只新增可选的「复制显示值」；I-06 的自动隐藏列与手动隐藏列一样不进入复制 / 导出，但必须显示隐藏数量；I-05 不改变结果列、排序、筛选与导出。
9. **可访问性**：新增编辑器、输入框、下拉、对话框、列面板、结果表格必须有可读标签或 `aria-label`，键盘可操作，焦点可见；状态变化（参数已应用、写入成功、列被自动隐藏）使用状态栏或 `aria-live`，颜色不是唯一信息载体。
10. **不劫持既有快捷键**：SQL 编辑器的查找 / 复制 / 撤销 / 补全、结果网格的 `⌘/Ctrl+F`、表数据的编辑快捷键保持原行为；I-01 的 `⌘/Ctrl+Enter` 只在 Mongo 控制台聚焦时生效；I-05 不新增全局快捷键；I-06 的列搜索只在面板内响应。
11. **代码风格与检查**：Vue / TS 2 空格，Rust 4 空格；完成后运行 `npm run check`；涉及 Rust 时运行 `cargo test --manifest-path src-tauri/Cargo.toml`。手工验收优先 `examples/demo.db`；MongoDB 测试使用临时数据库并在结束时删除，MySQL / PostgreSQL 使用临时表 / 临时会话并清理。
12. **不修改用户数据之外的状态**：I-04 / I-06 只写 localStorage 的显示配置；I-05 只保留会话内参数值；I-01 只写用户明确执行的 Mongo 命令；I-02 / I-03 只写连接配置与用户明确执行的库 / 集合操作。测试结束必须清理 `mydb_i01` / `mydb_i03` 等临时数据库。

### 0.8 约束适用性逐项说明

| 约束 | 是否适用 | 逐项说明 |
|---|---|---|
| 不新增驱动 | 全部适用 | I-01～I-06 只在现有 MongoDB / SQL 驱动上做；不接入新数据库。 |
| 不引入外部服务 / 依赖 | 全部适用 | I-01 只可能新增官方 CodeMirror 语言 / 补全包；I-04 / I-05 / I-06 纯前端；I-02 / I-03 复用现有 `mongodb` crate、Tauri 命令与钥匙串。 |
| 不写业务数据（显示 / 分析类） | I-04 / I-06 适用；I-01 / I-03 有明确写命令 | I-04 / I-06 只改变显示 / 可见性；I-01 / I-03 的写操作必须用户显式执行并确认；I-05 只替换 SQL 参数，不额外写库。 |
| 写操作必须走现有待提交 / 事务 | 本批的 Mongo 写入不进入关系库待提交；SQL 参数不改变事务 | I-01 的 Mongo 写入即时生效，不提供待提交（DBX 的控制台同样即时）；关系库表数据写入仍走现有待提交；I-05 在手动事务中复用同一 session，不自动提交。 |
| 不改连接配置 / 凭据 | I-02 例外但受控 | I-02 新增可选 URI 字段与独立凭据槽，不修改旧连接字段；其余项不写 `connections.json` / 钥匙串。 |
| 不改现有复制 / 导出语义 | 适用，但 I-04 新增可选「复制显示值」 | 默认复制 / 导出仍用原始值；I-06 自动隐藏列与手动隐藏列一样不进入导出；I-05 不改变结果导出。 |
| localStorage 只存非敏感元数据 | 全部适用 | I-01 只存控制台草稿（按连接，不含结果）；I-04 只存格式规则（不含数据值）；I-05 不存参数值；I-06 不存自动隐藏状态到 localStorage。 |
| 不新增后台常驻 / 系统服务 | 全部适用 | 无新后台进程；Mongo 控制台只在用户运行时执行命令。 |
| 不改变事务语义 | 全部适用 | I-05 复用现有事务 session；I-01 / I-03 不开启 Mongo 事务；I-04 / I-06 不碰事务。 |

---

## I-01 MongoDB 查询控制台（mongosh 风格命令 + Extended JSON）

- **优先级**：P0
- **状态**：已完成（2026-11-27：控制台、解析器、写安全、补全 / 历史 / 草稿、结果与导出）
- **预估**：2–3 个开发会话
- **范围**：
  - 前端：`src/components/MongoWorkspace.vue`（新增「查询」模式）、新增 `src/components/MongoConsole.vue`、抽出或复用 `MongoDocumentGrid`（若现有文档网格内联，可先抽成组件，不要求重写文档浏览）、新增 `src/lib/mongo-console.ts`、新增 `src/lib/mongo-console-history.ts`、`src/lib/api.ts`、`src/lib/types.ts`、`src/styles.css`、`package.json`（仅 CodeMirror 官方语言 / 补全包）。
  - 后端：新增 `src-tauri/src/commands/mongo_console.rs`（或拆出 `src-tauri/src/mongo_shell.rs`），复用 `commands/mongo.rs` 的 `client_for` / `parse_object` / `document_to_json`（需要把共享 helper 提升为 `pub(crate)`）、`models.rs`、`lib.rs` 命令注册、Rust 单元测试。
  - 不涉及新的数据库驱动、外部服务或 Mongo 事务。

### 用户问题 / 场景

MongoDB 首版只能通过左侧集合列表、JSON 筛选框、字段投影框、排序框和固定聚合框操作。想执行 `countDocuments`、`distinct`、`findOneAndUpdate`、`insertMany`，或临时写一条带 `$group` 的聚合，只能在「文档」「聚合」「索引」三个模式之间切来切去，或者退出应用用 mongosh。DBX / DBeaver / Navicat 都把「查询标签 / 命令控制台」作为 MongoDB 的核心入口；mydb 缺少这一层，MongoDB 就只能算「能看」，不能算「能用」。

### 现状核对（基于 2026-11-26 / 27 代码）

- `src/components/MongoWorkspace.vue` 的 `mode` 只有 `"documents" | "aggregation" | "indexes"`；顶部工具栏只有刷新、导入、导出、新建文档；没有命令编辑器、结果标签、执行按钮或命令历史。
- `src-tauri/src/commands/mongo.rs` 已有 `mongo_find_documents`、`mongo_aggregate_documents`、`mongo_insert_document`、`mongo_insert_documents`、`mongo_replace_document`、`mongo_delete_document`、`mongo_list_indexes`、`mongo_create_index`、`mongo_drop_index`、`mongo_create_collection`、`mongo_drop_collection`；全部接收结构化参数，没有 shell 文本解析。
- `client_for()` 按 `connection_id` 缓存 `mongodb::Client`；`ensure_writable()` 在写命令入口拦截只读连接；Mongo 查询 / 聚合有 30 秒超时，`MAX_PAGE_SIZE = 1000`；`parse_object()` 只接受合法 JSON 对象，`document_to_json()` 输出 Extended JSON。
- `MongoDocumentPage` 只包含 `documents / total / skip / limit / elapsedMs / truncated`；文档网格在前端按动态字段并集渲染，没有可复用的「通用查询结果」模型。
- SQL 编辑器 `SqlEditor.vue` 是 SQL 方言与 CodeMirror SQL 语言包；`package.json` 没有 `@codemirror/lang-javascript`，也没有 Mongo 命令补全。
- **现状标注：完全缺失。**

### 期望行为（可验收细节）

#### 1. 入口与布局

1. MongoDB 工作区顶部的分段控件增加第四项「查询」（现有：文档 / 聚合 / 索引 / 查询）。进入「查询」后：
   - 左侧数据库 / 集合列表保持可见，仍可切换数据库和集合；控制台使用**同一个当前数据库**。
   - 主区域为「控制台工具栏 + 命令编辑器 + 结果面板」；结果面板不挤占左侧列表，窄窗口下编辑器与结果上下排列。
   - 工具栏包含：运行（主按钮，带「运行当前命令 / 运行全部」下拉）、示例（只读模板下拉）、历史（会话内最近 50 条）、清空结果、只读 / 写入模式标识。
   - 只读连接显示「只读连接 · 写命令会被拒绝」；非只读连接显示「写命令立即生效，不支持撤销」的常驻轻提示（可折叠，但首次进入必须可见）。
2. 控制台的当前数据库与左侧数据库选择器**共用同一状态**：`use <database>` 成功后，左侧选择器切到该库并刷新集合列表；如果当前选中的集合不属于新库，清空集合选择，文档模式显示空状态；控制台结果与编辑器草稿保留。左侧切换数据库时，控制台上下文同步。
3. 编辑器草稿按连接保存在 `mydb.mongoConsoleDraft.v1`：按 `connectionId` 存最近一次文本（最多 100 KB、最多 50 个连接，超出按最近使用淘汰），不保存结果、不保存历史、不保存参数值。删除连接时清理对应草稿。
4. 切换标签 / 模式 / 连接后草稿保留；关闭连接或应用重启后草稿恢复；结果不恢复。

#### 2. 编辑器与执行范围

1. 使用 CodeMirror 6 的 JavaScript 语法高亮、行号、括号匹配、撤销 / 重做与查找；不要求实现完整的 mongosh 语法高亮。新增 `@codemirror/lang-javascript`（MIT）与直接声明 `@codemirror/autocomplete`（MIT）；若不新增依赖，则用现有 CodeMirror 基础能力 + 纯文本高亮，但必须保留行号与 `⌘/Ctrl+Enter`。
2. `⌘/Ctrl+Enter`：有选区时运行选区；无选区时按工具栏下拉的「运行当前命令 / 运行全部」执行。默认「运行当前命令」；「当前命令」由光标所在的最外层语句决定（语句边界见下一节）。
3. 工具栏「运行」下拉可临时选择「运行当前命令 / 运行全部」，不改变默认设置；编辑器中显示「第 N / M 条」徽标（M > 1 时）。
4. 运行中禁用再次运行，按钮显示「运行中…」；不使用 SQL 的「停止」按钮（Mongo 命令无法可靠取消；30 秒超时后返回错误）。运行中切换数据库 / 集合 / 模式时提示「命令执行中，请等待完成」。
5. 编辑器内 `⌘/Ctrl+F` 打开 CodeMirror 查找；不与结果网格 / SQL 编辑器查找冲突。

#### 3. 命令语法与解析

1. **语句边界**：按顶层分号 `;` 分句；字符串（双引号 / 单引号）、注释（`//`、`/* ... */`）、括号 / 方括号 / 花括号内的分号不切分。`use <db>` 与 `show ...` 可以省略分号，按行结束。空语句、纯注释语句忽略。
2. **单次上限**：最多 20 条语句；整个编辑器最多 100 KB；超过时拒绝执行并给出行号。
3. **支持的顶层命令**：
   - `use <database>`：切换当前数据库；数据库名按原样（可含 `-`、`_`、数字），不需要引号；不存在的数据库允许切换（MongoDB 允许连接到一个尚未创建的库，首次写入时创建），集合列表显示为空。
   - `show dbs` / `show databases`：列出数据库。
   - `show collections`：列出当前数据库集合。
4. **集合访问形式**：
   - `db.<collection>.<method>(...)`：`<collection>` 只允许 `[A-Za-z0-9_-]+`；包含点、空格、`$` 等特殊字符时使用 `db.getCollection("<name>").<method>(...)`。
   - `db.getCollection("<name>").<method>(...)`：`<name>` 必须是双引号字符串；支持集合名含点 / 连字符 / 中文。
   - 不支持 `db["name"]`、`db.getSiblingDB(...)`、`db.runCommand(...)`、链式跨库访问。
5. **参数必须是 Extended JSON**：
   - 对象键必须双引号；字符串必须双引号；数字、布尔、`null`、数组、嵌套对象按 JSON 规则。
   - ObjectId 使用 `{"$oid":"..."}`；日期使用 `{"$date":"2026-11-27T10:00:00Z"}` 或 `{"$date":{"$numberLong":"..."}}`；Long 使用 `{"$numberLong":"..."}`；二进制使用 `{"$binary":{...}}`。
   - **不支持**未加引号的键（`{name:"x"}`）、单引号、`ObjectId("...")`、`ISODate(...)`、正则字面量、函数、模板字符串、`undefined`、`new Date()`。解析失败时给出「第 X 行第 Y 列：参数必须是 Extended JSON；键名和字符串需要双引号」之类的可读提示，并建议使用「示例」中的模板。
6. **支持的方法**：

| 方法 | 参数 | 结果 | 备注 |
|---|---|---|---|
| `find(<filter>[, <projection>])` | 两个可选对象 | 文档列表 | 默认 `{}`；默认按 `_id` 升序；可链式 `.sort(<doc>)`、`.skip(<n>)`、`.limit(<n>)`、`.count()`、`.pretty()`；`.pretty()` 为无操作，便于粘贴 |
| `findOne(<filter>[, <projection>])` | 两个可选对象 | 单个文档或 `null` | 默认取第一条 |
| `countDocuments(<filter>)` | 可选对象 | 单行 `count` | 精确计数；大集合可能慢 |
| `estimatedDocumentCount()` | 无 | 单行 `count` | 使用集合元数据，标注「估算」 |
| `distinct(<field>[, <filter>])` | 字符串 + 可选对象 | 单列 `values` | 值按服务器返回顺序；最多显示 500 个，超出标注截断 |
| `aggregate(<pipeline>)` | JSON 数组 | 文档列表 | 沿用现有 `parse_pipeline()` 规则：禁止 `$out` / `$merge`；最多 500 个结果 |
| `insertOne(<doc>)` | 对象 | 写入摘要 | 只读连接拒绝；返回 `acknowledged` / `insertedId` |
| `insertMany(<array>[, <options>])` | 数组 + 可选对象 | 写入摘要 | 单次最多 1,000 个文档；超过拒绝；`ordered` 可选，默认 true |
| `updateOne(<filter>, <update>[, <options>])` | 两个对象 + 可选对象 | 写入摘要 | 返回 `matchedCount` / `modifiedCount` / `upsertedId` |
| `updateMany(<filter>, <update>[, <options>])` | 两个对象 + 可选对象 | 写入摘要 | 空筛选需确认；返回计数 |
| `replaceOne(<filter>, <replacement>[, <options>])` | 两个对象 + 可选对象 | 写入摘要 | 替换文档不得包含 `_id` 变更；空筛选需确认 |
| `deleteOne(<filter>)` | 对象 | 写入摘要 | 空筛选需确认；返回 `deletedCount` |
| `deleteMany(<filter>)` | 对象 | 写入摘要 | 空筛选需确认 |
| `findOneAndUpdate(<filter>, <update>[, <options>])` | 两个对象 + 可选对象 | 文档或 `null` | `returnDocument` 支持 `"before"` / `"after"`，默认 `"before"`；空筛选需确认 |
| `findOneAndReplace(<filter>, <replacement>[, <options>])` | 两个对象 + 可选对象 | 文档或 `null` | 空筛选需确认 |
| `findOneAndDelete(<filter>)` | 对象 | 文档或 `null` | 空筛选需确认 |

> `find` 未指定 `.limit()` 时默认取前 50 个文档；显式 `.limit(n)` 最大 500；`aggregate` 最多返回 500 个结果。默认 50 是为了避免一次拉取过多文档，用户可用 `.limit()` 主动扩大。

7. **不支持的方法**：`createIndex` / `dropIndex`（用索引页）、`drop` / `dropDatabase` / `renameCollection`（用 I-03 或对象菜单）、`createCollection`（用左侧 `+`）、`bulkWrite`、`mapReduce`、`explain`、`stats`、`validate`、`watch`、`getSiblingDB`、`runCommand`、事务 / `startSession`、GridFS。遇到这些方法时返回「该命令暂不支持；请使用左侧对象操作 / 索引页 / 后续专项」，**不执行任何部分**。
8. **链式方法限制**：只允许 `find()` 后接 `.sort()`、`.skip()`、`.limit()`、`.count()`、`.pretty()`，顺序不限，但 `.count()` 之后不能再链式调用；`findOne()`、`aggregate()`、写方法不支持链式。重复的 `.limit()` 取最后一次；`.skip()` / `.limit()` 参数必须是 0–500 的整数（`.limit(0)` 视为不限制但最多 500）；超过 500 截断并标注。
9. **多命令预检**：一次运行多条命令时，先解析全部语句；任一条语法错误、不支持的方法、只读连接下的写命令或需要确认的危险写命令，都**不执行任何一条**，返回对应语句序号与原因。预检通过后按顺序执行；运行期某条失败时停止后续命令，保留已完成结果，并提示「脚本非事务，已完成 N 条命令，其中写入 M 条」。

#### 4. 结果与展示

1. **文档结果**：复用 MongoDB 文档网格的「动态字段并集」渲染；每行一个文档，单元格显示标量或 JSON 文本；支持复制单元格 / 整行 / 全部为 JSON 或 TSV；支持导出当前结果为 JSON / CSV（最多 500 个文档，文件名含数据库与集合）。文档结果**只读**，本批不提供网格编辑。
2. **单文档结果**（`findOne` / `findOneAnd*`）：如果文档存在，显示一行网格；如果为 `null`，显示「未找到文档」空状态，不伪造空行。
3. **标量结果**（`countDocuments` / `estimatedDocumentCount` / `distinct`）：使用两列表格（`count` / `values`），`distinct` 值最多 500 个；支持复制。
4. **写入摘要**：表格列包含 `acknowledged`、`insertedId`、`insertedCount`、`matchedCount`、`modifiedCount`、`deletedCount`、`upsertedId` 中实际存在的字段；零值也要显示；支持复制 JSON。
5. **多命令结果**：结果区顶部显示「结果 1 / 结果 2…」标签，每个标签显示语句摘要（截断到 80 字符）与类型（文档 / 标量 / 写入 / 空）；切换标签不重新执行。失败时保留已完成的结果，当前标签停在失败语句并显示错误。
6. **执行信息**：显示耗时、数据库、集合、结果数 / 截断；错误显示服务器错误码 / 名称 / 消息（如有）与语句序号；不显示堆栈、不显示完整连接串或密码。
7. **空结果**：`find` 无匹配显示「查询成功，但没有匹配文档」；`deleteOne` 匹配 0 条显示 `deletedCount: 0`；不把空结果当成错误。
8. **结果生命周期**：结果只保存在当前 Mongo 工作区内存；切换连接、关闭连接、重新运行或点「清空结果」时清空；不写入 localStorage、不写入 SQL 历史。

#### 5. 写入安全与确认

1. **只读连接**：所有写方法（`insert*`、`update*`、`replaceOne`、`delete*`、`findOneAndUpdate/Replace/Delete`）在预检阶段被拒绝，错误信息包含语句序号与「当前连接为只读模式」；读方法仍可用。后端 `ensure_writable` 是最终边界。
2. **危险写确认**：
   - `updateMany` / `deleteMany` / `replaceOne` / `findOneAndUpdate` / `findOneAndReplace` / `findOneAndDelete` 的筛选为空对象 `{}`；
   - `deleteOne` 的筛选为空对象 `{}`；
   - `insertMany` 超过 100 个文档；
   - 其他被判定为「无筛选、可能影响整个集合」的写命令。
   满足任一条件时，后端返回 `requiresConfirmation`，包含数据库、集合、方法、原因、预计影响数量（空筛选用 `estimatedDocumentCount`，失败则显示「未知」）与脱敏后的命令预览；**不执行写入**。前端弹出确认框，要求勾选「我确认将影响整个集合 / 大量文档」后才允许再次运行；第二次请求带 `confirmed: true`。
3. **确认粒度**：一次运行多条命令时，确认框列出所有需要确认的语句；确认后整批执行；取消则一条都不执行。
4. **写入立即生效**：Mongo 控制台不提供关系库式的待提交缓冲区、撤销 / 重做或回滚；写入成功后刷新集合列表（`insert` 可能隐式创建集合）并在状态栏显示影响计数。UI 必须明确「写入立即生效，不支持撤销」。
5. **不自动重试 / 不自动重放**：超时、网络错误、服务器错误后不自动重试写命令；已完成部分保留，由用户决定是否重跑。没有事务，不能保证多命令原子性。
6. **`$where` / `$function` / 服务器端 JS**：不额外拦截；由数据库用户权限与服务器配置控制。UI 在首次使用包含 `$where` / `$function` 的命令时给出轻提示「该查询会在服务器执行 JavaScript，请确认可信」。只读连接仍不能写。
7. **聚合写阶段**：`aggregate` 中出现 `$out` / `$merge` 时直接拒绝（沿用现有规则），不执行；提示使用对象导出 / 数据同步等显式工具。

#### 6. 补全、示例与历史

1. **补全范围**：
   - `use ` 后补全当前连接可见的数据库名；
   - `show ` 后补全 `dbs` / `databases` / `collections`；
   - `db.` 后补全当前数据库的集合名（最多 200 个，超出提示继续输入）；
   - `db.<collection>.` 后补全支持的方法名；
   - 方法参数内补全常用查询操作符（`$eq`、`$gt`、`$gte`、`$lt`、`$lte`、`$in`、`$nin`、`$ne`、`$exists`、`$regex` 等）、更新操作符（`$set`、`$unset`、`$inc`、`$push`、`$pull`、`$addToSet`、`$rename` 等）与聚合阶段（`$match`、`$group`、`$project`、`$sort`、`$limit`、`$skip`、`$unwind`、`$lookup`、`$count` 等）；
   - 字段名补全：仅使用**当前集合已在文档模式加载过的字段并集**（`MongoWorkspace` 已有 `fields` computed）；没有加载过则不提示字段，不额外发起采样请求。
2. **补全交互**：`Ctrl+Space` 手动唤起；输入 `.` / `$` / 引号时可按现有 CodeMirror 习惯自动提示；补全项显示类型图标 / 简短说明；键盘上下选择、Enter 插入、Esc 关闭；中文输入法组合期间不弹出。
3. **示例**：只读下拉，至少包含 `find`、带排序 / 分页的 `find`、`countDocuments`、`distinct`、`aggregate`（`$match` + `$group`）、`insertOne`、`updateOne`、`deleteOne`。点击示例插入到光标处（替换选区），不自动执行；示例中的集合名使用 `users` / `orders`，并在注释中说明需要替换。
4. **历史**：会话内最近 50 条命令，记录命令文本、时间、数据库、成功 / 失败；默认不持久化，关闭应用即清空；点击回填到编辑器，可清空。历史不写入 SQL 历史文件，不与 SQL 历史混合。
5. **不执行 SQL 静态检查 / 格式化**：Mongo 控制台没有 SQL 检查、格式化、执行计划入口；不要为了复用而把 SQL 检查套到 JS 文本上。

#### 7. 失败与边界

- **连接丢失**：显示「连接已断开，请重试或重连」；不自动重放命令；已完成的写入保留并明确提示。复用现有健康检查 / 重连，但重连不恢复控制台运行中的命令。
- **认证 / 权限错误**：原样显示服务器错误（去除连接串与密码）；提示「当前用户可能没有该数据库 / 集合的权限」。
- **集合不存在**：`find` / `aggregate` 返回服务器错误；`insertOne` 会隐式创建集合，成功后刷新集合列表；`getCollection` 名称非法时在解析阶段报错。
- **数据库不存在**：`use` 允许切换（Mongo 语义），`show collections` 为空；写入时由服务器创建数据库；不伪造数据库。
- **结果过大**：单条命令最多返回 500 个文档 / 值；超出显示「已截断，仅显示前 500 个」；`distinct` 值最多 500 个；不自动分页（用户可加 `.skip()` / `.limit()` 或聚合 `$limit`）。
- **单文档过大**：BSON 文档上限由服务器决定（通常 16 MB）；前端只显示 JSON 文本，不截断值本身，但复制 / 导出受浏览器内存限制；超过 2 MB 的单元格建议用 JSON 查看器 / 复制，不在网格内展开。
- **超时**：30 秒超时；写命令超时后不自动重试，提示「可能已在服务器生效，请先查询确认」。
- **多命令部分失败**：保留已完成结果；提示已完成数量与写入数量；不自动回滚。
- **非法 JSON**：解析错误包含行列与示例；不执行任何语句。
- **对象名含特殊字符**：`db.getCollection("...")` 支持；`db.a-b` 解析为不支持的表达式，提示改用 `getCollection`。
- **只读 / 无权限**：只读连接读命令可用；写命令预检拒绝；无权限时由服务器返回错误，不在前端猜测。
- **`$out` / `$merge`**：聚合预检拒绝；不执行部分阶段。
- **`use` 指向系统库**：允许 `use admin` 读取（用户可能有权限），但写命令仍受权限与只读限制；控制台不主动阻止系统库，I-03 才做系统库删除保护。

#### 8. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| 编辑器文本 | 100 KB；超过拒绝执行，保留编辑 |
| 单次命令数 | 20 条；超过拒绝 |
| 单条结果文档 / 值 | 500；超出截断并提示 |
| `insertMany` | 1,000 个文档 / 条；超过拒绝 |
| 命令参数总大小 | 1 MB / 条；超过拒绝 |
| 执行超时 | 30 秒 |
| 补全项 | 200 个 / 类；超出提示继续输入 |
| 历史 | 50 条 / 会话 |
| 草稿 | 每个连接 100 KB，最多 50 个连接 |
| 解析 | 100 KB / 20 条命令目标 < 50 ms；不在每次按键时解析全文，补全按需扫描 |

#### 9. 明确不做

- 不做完整的 mongosh / JavaScript 解析器、不做 `ObjectId()` / `ISODate()` / 正则 / 函数 / `eval`。
- 不做 Mongo 事务、`startSession`、`commitTransaction` / `abortTransaction`。
- 不做查询结果网格编辑、待提交 `updateOne` / `deleteOne`、插入行。
- 不做 `explain()`、profiler、慢查询分析、执行计划可视化。
- 不做 GridFS、用户 / 角色、副本集管理、分片管理。
- 不做 `createIndex` / `dropIndex` / `drop` / `dropDatabase` / `renameCollection` / `createCollection`（使用现有索引页、对象菜单与 I-03）。
- 不做 `getSiblingDB`、`bulkWrite`、`runCommand`、`mapReduce`、`watch`。
- 不做控制台分屏 / 独立窗口 / App 级查询标签；第一版是 Mongo 工作区内的一个模式。
- 不做语法高亮主题切换、格式化、自动补全字段采样请求。
- 不做命令自动重试、自动重放、自动事务包裹。

#### 10. 涉及文件 / 可行落点

- `src/components/MongoWorkspace.vue`：`mode` 增加 `"console"`；分段控件；工具栏与结果区布局；`use` 成功后同步 `database` / `loadCollections`；刷新集合列表。
- 新增 `src/components/MongoConsole.vue`：编辑器、运行 / 示例 / 历史 / 清空、结果标签、确认框、错误与空状态；通过 props / emit 与 `MongoWorkspace` 交互，不直接访问数据库。
- 新增 `src/lib/mongo-console.ts`：
  - `splitMongoStatements(text)`：返回 `{ text, start, end }[]`，处理字符串、注释、括号；
  - `parseMongoCommand(text, database)`：返回 `{ kind: "use" | "show" | "call"; database?; collection?; method?; args? }` 或 `{ error, line, column }`；
  - `mongoCompletions(context)`：数据库 / 集合 / 方法 / 操作符 / 阶段 / 已加载字段；
  - `formatMongoResultCell(value)`、`mongoResultToJson()` 等纯函数。
- 新增 `src/lib/mongo-console-history.ts`：会话内 50 条，内存数组；不写 localStorage。
- 抽离 `MongoDocumentGrid.vue`（可选）：从 `MongoWorkspace.vue` 抽出动态字段网格与单元格 JSON 渲染，供文档模式与控制台结果复用；如果抽离风险大，可先在 `MongoConsole.vue` 内复用同一套 `fields` / `cellText` 逻辑，但不得复制两套不一致的渲染规则。
- `src-tauri/src/commands/mongo_console.rs`（推荐）：命令解析（Rust 权威解析器）、方法白名单、参数 Extended JSON 校验、预检、只读 / 危险确认、执行与结果映射；`mongo.rs` 的 `client_for` / `parse_object` / `document_to_json` / `parse_pipeline` 提升为 `pub(crate)` 或移到共享模块。
- `src-tauri/src/models.rs`：新增 `MongoConsoleResult` / `MongoConsoleOutcome` / `MongoConsoleConfirmation` / `MongoConsoleWriteSummary` 等 `Serialize` 结构；字段使用 `camelCase`。
- `src-tauri/src/lib.rs`：注册 `mongo_console_execute`；如拆分解析器，注册不需要额外命令。
- `src/lib/api.ts` / `src/lib/types.ts`：`mongoConsoleExecute(connectionId, database, script, confirmed)` 与对应类型。
- `src/styles.css`：控制台编辑器、结果标签、确认框、只读 / 写入提示、历史下拉、错误块；复用现有 token。
- `package.json`：`@codemirror/lang-javascript`（MIT）、`@codemirror/autocomplete`（MIT）；若决定不新增，保留行号 / 快捷键 / 纯文本方案，但需在文档中说明降级。
- 测试：Rust 单元测试覆盖语句切分、命令解析、参数类型、危险判定、只读拦截、白名单；真实 Mongo 回归使用新增 `MYDB_TEST_MONGO_URI`（见附录）。

#### 11. 验收标准（可直接在临时 MongoDB 库执行）

- [ ] 进入 MongoDB 连接 → 切到「查询」→ 执行 `use mydb_i01`，左侧数据库选择器同步，集合列表刷新。
- [ ] `db.users.find({"city":"上海"}).sort({"id":1}).limit(10)` 返回最多 10 个文档；字段并集正确；`_id` 以 `{"$oid":"..."}` 显示。
- [ ] `db.users.find({"createdAt":{"$gte":{"$date":"2026-01-01T00:00:00Z"}}})` 按日期筛选；`{"$numberLong":"..."}` 保持为 Long 文本 / 不丢精度。
- [ ] `db.orders.aggregate([{"$match":{"status":"paid"}},{"$group":{"_id":"$user_id","total":{"$sum":"$amount"}}},{"$sort":{"total":-1}}])` 返回聚合结果；`$out` / `$merge` 被拒绝。
- [ ] `db.users.countDocuments({"status":"active"})`、`db.users.estimatedDocumentCount()`、`db.users.distinct("city")` 分别返回单行 / 值列表，并标注估算 / 截断。
- [ ] `db.users.insertOne({"name":"测试","age":30})` 返回 `insertedId`；再 `find` 能查到；集合列表刷新（若隐式建集合）。
- [ ] `db.users.updateOne({"_id":{"$oid":"..."}},{"$set":{"age":31}})` 返回 `matchedCount` / `modifiedCount`；`updateMany` / `deleteMany` 空筛选弹出确认，取消时零写入，确认后执行。
- [ ] 只读连接执行 `insertOne` / `updateOne` / `deleteMany` 被拒绝，且不执行任何语句；读命令仍可用。
- [ ] 一次运行 `use mydb_i01; db.users.find({}); db.orders.countDocuments({})`：三个结果标签；中间某条失败时保留前面的结果，后续不执行，并显示语句序号。
- [ ] `{name:"x"}`、单引号、`ObjectId("...")` 被解析器拒绝，错误包含行列与「Extended JSON」提示；`db.getCollection("a.b").find({})` 可用。
- [ ] `db.users.drop()`、`db.users.createIndex(...)`、`db.getSiblingDB("x")`、`db.users.find(...).explain()` 返回「暂不支持」及替代入口，不执行。
- [ ] 结果最多 500 个文档；`distinct` 最多 500 个值；`insertMany` 1,001 个文档被拒绝；100 KB / 20 条命令上限生效。
- [ ] 命令超时 30 秒后返回错误，不自动重试；写命令超时提示「可能已生效」。
- [ ] 控制台草稿按连接重启恢复；结果不恢复；历史只在当前会话；清空结果 / 切换连接后结果清空。
- [ ] `⌘/Ctrl+Enter` 在有选区时只运行选区；无选区时按「当前命令 / 运行全部」；多命令时「第 N / M 条」正确。
- [ ] 只读 / 非只读提示正确；写入成功后状态栏显示影响计数；错误信息不含连接串 / 密码。
- [ ] 真实 MongoDB 门控回归（认证 / TLS / 集合 CRUD / 聚合 / 写入 / 只读）通过；无真实实例时明确标注「未完成真实回归」，不能只凭模拟数据验收。

#### 12. 风险 / 限制

- **解析器是最大风险**：Extended JSON 能保证可预测，但和 mongosh 的 JavaScript 语法有差距；用户从网上复制 `ObjectId("...")`、单引号或未加引号的键会失败。必须给出清晰错误与示例，不能静默猜测。
- **写命令立即生效**：没有待提交、撤销、事务；`updateMany` / `deleteMany` 误操作无法回滚。确认框与空筛选检测必须严格，但仍不能替代备份。
- **命令白名单的边界**：MongoDB 方法很多，第一版只支持表内方法；遇到未知方法一律拒绝，不能把未知文本透传给 `runCommand`。
- **`$where` / `$function`**：会在服务器执行 JavaScript，性能与安全由数据库权限控制；UI 只提示，不做沙箱。
- **副本集 / 分片集群**：部分命令（如 `renameCollection`、`collStats`）在分片集群上行为不同；I-01 只做 CRUD / 聚合，I-03 的错误需原样呈现。
- **结果网格复用**：现有 Mongo 文档网格是内联的，抽组件可能牵连文档浏览；如果抽离风险大，控制台可以先使用独立的只读网格，但必须复用同一套字段并集 / JSON 单元格规则，避免两套显示不一致。
- **CodeMirror JavaScript 包**：新增约几十 KB 的前端体积；如果用户对体积敏感，可降级为纯文本编辑器，但会失去语法高亮。补全仍可用纯文本扫描实现。
- **无真实 Mongo 实例**：当前开发环境没有可用的 MongoDB 实例；Rust 单测与前端模拟只能覆盖解析与状态，不能替代真实认证 / TLS / 副本集 / SRV 回归。本项交付必须附带真实环境回归清单。

---

## I-02 MongoDB URI / SRV 连接

- **优先级**：P1
- **状态**：已完成（2026-11-27，URI / 字段两种方式、独立钥匙串槽、脱敏与真实 URI 回归）
- **预估**：1–1.5 个开发会话
- **范围**：
  - 前端：`src/components/ConnectionDialog.vue`（MongoDB 连接方式切换、URI 输入、测试 / 保存 / 编辑、密码占位）、`src/lib/types.ts`、`src/lib/api.ts`、`src/App.vue`（连接信息复制、创建 / 编辑连接后的状态、连接列表目标摘要）、`src/components/ConnectionTransferDialog.vue`（如涉及导入导出展示）。
  - 后端：`src-tauri/src/models.rs`（`ConnectionConfig` 新字段）、`src-tauri/src/commands/connections.rs`（校验、保存、复制、连接信息、测试连接）、`src-tauri/src/commands/mongo.rs`（URI 构建 / 连接 / 测试 / ping / 重连）、`src-tauri/src/keychain.rs`（如果新增凭据类型）、`src-tauri/src/lib.rs`（命令签名变化时更新注册）。
  - 不新增数据库驱动，不引入 `mongodb-legacy` 或外部代理。

### 用户问题 / 场景

MongoDB Atlas、云厂商托管 MongoDB、副本集和分片集群通常给出一条 `mongodb+srv://...` 或 `mongodb://host1,host2/db?replicaSet=...` 连接串。mydb 当前只能填单个 host、port、用户名、authSource 和 TLS 开关，无法表达多 host、SRV、副本集、`readPreference`、`retryWrites`、`tlsCAFile` 等参数；用户只能把云连接拆成单机地址，或者在应用外维护 mongosh。DBX 明确支持 `mongodb://` 与 `mongodb+srv://`，这是 Mongo 用户的默认连接方式。

### 现状核对（基于 2026-11-26 / 27 代码）

- `ConnectionConfig` 有 `host / port / username / database / mongo_auth_source / ssl_mode / ssl_ca_path / ssl_client_cert_path / ssl_client_key_path`，没有 URI、多 host 或 SRV 字段；`mongo_uri()` 固定拼 `mongodb://`，只有 SSH 隧道时追加 `directConnection=true`。
- `ConnectionDialog.vue` 的 MongoDB 表单是 host / port / 用户名 / 密码 / authSource / TLS 下拉；`test_connection` 与 `client_for` 都通过 `connection_target()` 得到单个 host / port，再建 URI。
- `connection_info` 对 MongoDB 输出 `mongodb://...` URL、`mongosh "mongodb://..."` 命令；没有 URI 模式、没有 SRV、没有多 host。
- 密码保存在系统钥匙串（开发版为本机凭据文件），`connections.json` 不存密码；`has_password` 是派生字段。
- `mongodb = "3.10"` 使用默认 features（含 `dns-resolver`、`rustls-tls`），`ClientOptions::parse` 具备解析 `mongodb+srv` 的能力；但项目没有使用 URI 模式的入口。
- **现状标注：完全缺失。**

### 期望行为（可验收细节）

#### 1. 连接方式切换

1. MongoDB 连接对话框在「主机 / 端口」之外新增「连接串（URI / SRV）」方式，二选一：
   - **主机 / 端口**：保持现有字段与行为不变（含 SSH、TLS、authSource）。
   - **连接串**：显示单个 URI 输入框（单行，可横向滚动；支持粘贴长串），隐藏 host / port / 用户名 / 密码 / authSource / TLS 字段，保留「保存到系统钥匙串」「只读」「连接名称 / 分组 / 颜色」。
2. 切换方式时，不自动把旧字段拼成 URI，也不自动把 URI 拆成字段；如果当前方式已有未保存内容，切换前提示「切换连接方式会清空当前方式的未保存输入」。
3. 编辑已有连接时，按保存的 `mongoUriMode` 恢复方式；URI 模式显示**脱敏后的 URI**（密码替换为 `******`），密码输入框显示「已保存连接串，留空保持不变」。用户不修改 URI 时保留已保存密文；修改 URI 时按新 URI 重新提取 / 保存密文。
4. 新建 URI 连接时默认不预填示例密码；输入框 placeholder 给出两种示例：
   - `mongodb://user:password@127.0.0.1:27017/mydb?authSource=admin`
   - `mongodb+srv://user:password@cluster0.example.mongodb.net/mydb?retryWrites=true&w=majority`

#### 2. 支持的 URI 范围

1. 支持 `mongodb://` 与 `mongodb+srv://`：
   - `mongodb://`：单 host、多 host（`host1:27017,host2:27017`）、IPv6 方括号、`user:password@`、`/database`、查询参数。
   - `mongodb+srv://`：单个 SRV 主机名，不带端口；驱动通过 DNS SRV / TXT 发现节点；默认 TLS 由驱动按 SRV 规则处理。
2. 允许的查询参数交给 `ClientOptions::parse` 解析，至少覆盖：`authSource`、`replicaSet`、`tls` / `ssl`、`tlsCAFile`、`tlsCertificateKeyFile`、`readPreference`、`retryWrites`、`w`、`appName`、`directConnection`、`authMechanism`。未知参数、非法值、冲突参数（如 `mongodb+srv` + `directConnection=true`）由驱动或后端返回可读错误，**不得静默忽略**。
3. 应用自身设置 `appName = "mydb"`；如果 URI 已包含 `appName`，保留用户值并在测试结果中显示最终 appName，不覆盖。
4. `mongodb+srv` 不允许与 SSH 隧道同时启用；`mongodb://` 多 host 也不允许与 SSH 隧道同时启用。UI 在 URI 模式下禁用 SSH 开关并说明「URI / SRV 连接请直接访问数据库；需要堡垒机时请使用主机 / 端口模式」。单 host `mongodb://` + SSH 也不在本批支持，避免「URI 里写的是远端地址、隧道只转发一个本地端口」的歧义。
5. URI 模式不使用 `ssl_mode` / `ssl_ca_path` / 客户端证书字段；TLS 完全由 URI 参数控制。如果 URI 没有 TLS 参数，按驱动默认（`mongodb://` 默认不加密，`mongodb+srv://` 默认加密）；测试连接成功后可在连接信息中显示「TLS：由连接串控制」。
6. 数据库路径 `/db` 作为连接后的初始数据库；没有路径时按驱动默认（通常是 `test`），但左侧数据库列表仍列出全部可见数据库。`authSource` 缺省时沿用驱动默认（通常是 `admin`，以 URI / 驱动为准），不强行写 `authSource=admin`。

#### 3. 凭据与存储

1. `ConnectionConfig` 新增：
   - `mongo_uri_mode: bool`（默认 `false`，旧配置不受影响）；
   - `mongo_uri: String`（默认空；URI 模式下保存**脱敏后的显示串**，不含密码与敏感查询参数）；
   - 派生字段 `has_mongo_uri_secret: bool`（不持久化，仅用于对话框占位）。
2. 完整 URI（含密码 / 敏感参数）必须保存在**独立凭据槽**（建议钥匙串 kind `mongo-uri`；开发版进入本机凭据文件），与数据库密码 `db` 槽分开；不得写入 `connections.json`、localStorage、日志、错误信息或导出文件。
3. 保存规则：
   - 用户勾选「保存到系统钥匙串」：完整 URI 写入 `mongo-uri`；配置只保存脱敏 URI。
   - 不勾选：完整 URI 只保留在当前进程内存（与现有密码行为一致）；关闭应用后丢失，下次需要重新输入。
   - 编辑时留空：沿用已保存密文；显式清空 / 点「清除已保存连接串」：删除 `mongo-uri` 槽，配置中的脱敏 URI 保留但不含密码。
   - 如果用户修改了 URI 且新 URI 不含密码，保存前提示「新连接串没有密码，继续将清除已保存的连接串密码」；用户确认后才清除，避免只改 host 时静默丢密码。
4. 脱敏规则：
   - 去掉 userinfo 中的 password（保留 username，显示 `user:******@`）；
   - 查询参数名包含 `password`、`secret`、`token`、`key`（大小写不敏感）的值替换为 `******`；
   - 其余 host、port、database、`authSource`、`replicaSet`、`tls` 等保留，便于识别；
   - 如果 URI 含密码但无法安全解析，整段 userinfo 替换为 `******@`，不把可疑原文写入配置。
5. 连接信息复制：
   - 默认输出脱敏 URI / `mongosh "脱敏 URI"`；
   - 「含密码 URL」需要二次确认，确认后从钥匙串 / 会话内存取完整 URI 输出；取不到时提示「未保存连接串密码，请先编辑连接并输入」；
   - 复制到剪贴板的完整 URI 不写入历史 / 日志。
6. 连接导入 / 导出：
   - 导出始终输出脱敏 URI，**不含** `mongo-uri` 密文；
   - 导入 `mongoUriMode` 与脱敏 URI，标记「需要重新输入连接串密码」；不因导入失败而覆盖现有连接；
   - 旧版 mydb / DBeaver / Navicat 导入的 host / port 连接保持原方式，不自动转 URI。

#### 4. 测试连接与运行

1. 「测试连接」：
   - URI 模式使用完整 URI（会话内输入或已保存密文）建立客户端，执行 `ping`，10 秒超时；成功后显示「连接成功 · 数据库 N 个 · TLS / SRV 由连接串控制」之类的摘要（数据库数量可延迟到打开连接后再显示，测试阶段至少显示 ping 成功）。
   - 失败时返回可读原因：URI 语法、DNS SRV 解析、TLS 证书、认证失败、超时、主机不可达；错误中不得包含密码。
2. 保存后打开连接：
   - `client_for` 在 URI 模式使用完整 URI 建立 `mongodb::Client`；缓存、健康检查、重连、只读标记与现有 Mongo 行为一致。
   - 连接断开后的 `ping` / `reconnect` 使用同一 URI 与凭据；重连不重放命令、不恢复事务（Mongo 没有事务界面）。
   - 左侧数据库列表、集合浏览、文档 CRUD、聚合、索引、I-01 控制台、I-03 管理全部复用现有逻辑。
3. 连接列表 / 状态栏的地址摘要：URI 模式显示脱敏 URI 的 host 部分（如 `cluster0.example.mongodb.net` 或 `host1,host2`），不显示密码；`connection_info` 不再强行拼 `mongodb://{host}:{port}`。
4. 只读连接：URI 模式与字段模式一样，`ensure_writable` 拦截所有写命令；不因 URI 权限更高而绕过。
5. 复制连接：复制 `mongo_uri_mode` 与脱敏 URI，但**不复制** `mongo-uri` 密文；新连接需要重新输入密码（与现有「复制连接默认不复制密码」一致）。
6. 删除连接 / 清除凭据：同时清理 `mongo-uri` 槽、客户端缓存、隧道；不留孤儿凭据。

#### 5. 校验与失败边界

- **URI 为空**：保存 / 测试时提示「连接串不能为空」。
- **scheme 不支持**：只接受 `mongodb://` / `mongodb+srv://`；`http://`、`mongodb+srv` 拼写错误等直接拒绝，并给出正确示例。
- **密码含特殊字符**：URI 中的 userinfo 必须 percent-encode；如果用户直接粘贴含 `@`、`:`、`/` 的未编码密码，`ClientOptions::parse` 可能解析错误；错误信息提示「密码中的特殊字符需要 percent-encode，或改用主机 / 端口模式」。
- **多 host 格式错误**：缺少端口、空 host、重复 host、IPv6 未加方括号等由驱动返回错误；前端不猜测修正。
- **SRV 解析失败**：提示「DNS SRV / TXT 记录不可用，请检查域名、网络或 DNS」；不自动回退到 A 记录或单 host。
- **SRV + SSH**：保存 / 测试时拒绝，提示使用主机 / 端口模式。
- **SRV + directConnection**：拒绝，提示两者不兼容。
- **TLS 失败**：显示证书 / 主机名错误；不回退明文；不自动忽略证书。
- **认证失败**：显示「认证失败，请检查用户名 / 密码 / authSource」；不回显密码。
- **数据库不存在**：URI 指向的数据库允许尚不存在（Mongo 语义），连接成功后列表为空或显示其他库；首次写入时由服务器创建。
- **连接串过长**：超过 2,048 字符拒绝保存，提示拆分或使用主机 / 端口模式。
- **旧配置兼容**：没有 `mongoUriMode` 的旧 JSON 按 `false` 读取；`mongoUri` 为空；旧连接行为不变。
- **模式切换**：URI → 主机 / 端口时，保留 URI 密文但不用于字段模式；字段模式 → URI 时，旧密码槽不当作 URI；两种密文互不覆盖，避免把密码当 URI 使用。
- **导入导出**：导出不含密文；导入后 `hasMongoUriSecret` 为 false；用户必须重新输入。
- **只读 / 权限**：连接成功但用户无 `listDatabases` 权限时，左侧列表可能为空；显示服务器错误，不伪造数据库列表。

#### 6. 数据与存储

- `connections.json` 新增 `mongoUriMode`、`mongoUri` 两个字段；旧字段全部保留，旧配置可直接读取。
- 完整 URI 存在系统钥匙串（kind `mongo-uri`）或开发版本机凭据文件；不进入 `connections.json`、localStorage、SQL 历史、结果导出。
- 连接配置目录、钥匙串释放、删除连接、复制连接、导入导出规则与现有连接一致。
- 不新增 localStorage key；不在 WebView 中持久化 URI 密码。

#### 7. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| URI 长度 | 2,048 字符 |
| 测试连接超时 | 10 秒 |
| ping / 健康检查 | 5 秒（沿用现有） |
| SRV DNS 解析 | 由驱动处理；失败给出可读错误 |
| 多 host 数量 | 由驱动接受；UI 不额外限制，但超过 10 个 host 提示确认 |
| 连接信息复制 | 脱敏默认；含密码需二次确认 |

#### 8. 明确不做

- 不做 `mongodb-legacy` / 旧版驱动回退；Rust 驱动失败时如实报错。
- 不做 URI 与字段的双向自动转换（不在保存时拆分 / 拼接）。
- 不做 HTTP / SOCKS 代理、多级 SSH 跳板；URI 模式不支持 SSH。
- 不做副本集拓扑可视化、分片管理、节点角色面板。
- 不做 SQL 驱动的 URI 输入（MySQL / PostgreSQL 仍用字段表单）。
- 不做连接串加密导出；导出仍脱敏。
- 不做 SRV 的 DNS 手动覆盖 / 自定义 resolver。
- 不做连接串中的 `authMechanismProperties` 等敏感参数的 UI 编辑器；按原样交给驱动，脱敏保存。

#### 9. 涉及文件 / 可行落点

- `src-tauri/src/models.rs`：`ConnectionConfig` 增加 `mongo_uri_mode`、`mongo_uri`、派生 `has_mongo_uri_secret`；`Default` 中初始化；serde `default` 保证旧配置兼容。
- `src-tauri/src/commands/connections.rs`：
  - `validate_config()` 对 `mongodb` 分支：URI 模式下校验非空、scheme、长度、SSH / SRV 冲突；不再要求 host 非空；
  - `save_connection` / `test_connection`：URI 模式取 `mongo-uri` 密文或会话 URI；保存时写入独立凭据槽；
  - `clear_connection_secret`：支持 `mongo-uri` kind；
  - `connection_info`：URI 模式输出脱敏 URI / `mongosh`；含密码分支使用完整 URI；
  - `duplicate_connection`：复制 URI 模式配置，不复制密文。
- `src-tauri/src/commands/mongo.rs`：
  - `mongo_uri()` 改为根据 `config.mongo_uri_mode` 返回完整 URI（从密文 / 会话取）或按字段构建；
  - `connect_with_config()` 不再强制 host / port 参数；URI 模式直接 `ClientOptions::parse`；
  - `test_mongo_connection` / `client_for` / `ping` / `reconnect` 适配；
  - 错误信息统一脱敏。
- `src-tauri/src/keychain.rs`：如新增 `mongo-uri` kind，扩展读写；
- `src/components/ConnectionDialog.vue`：方式切换、URI 输入、密码占位、SSH 禁用、校验、测试、保存 payload；
- `src/App.vue`：`SavePayload` 可新增 `mongoUriSecret?: string`；连接信息复制分支；连接摘要；
- `src/lib/types.ts` / `src/lib/api.ts`：`ConnectionConfig` 新字段、`saveConnection` / `testConnection` 可选 URI 密文、`clearConnectionSecret` kind、`connectionInfo`；
- `src/components/ConnectionTransferDialog.vue`：导入 / 导出时展示「连接串已脱敏，需要重新输入密码」；
- `src/styles.css`：URI 输入、方式切换、脱敏提示、错误块。

#### 10. 验收标准

- [ ] 新建 MongoDB 连接 → 选择「连接串」→ 输入 `mongodb://user:pass@127.0.0.1:27017/mydb?authSource=admin` → 测试成功 → 保存；`connections.json` 中只有脱敏 URI（`user:******@`），没有明文密码。
- [ ] 关闭应用重开 → 连接仍可打开（密码从钥匙串读取）；编辑连接时 URI 显示脱敏，密码占位「已保存连接串，留空保持不变」。
- [ ] 输入 `mongodb+srv://cluster0.example.mongodb.net/mydb`（在可解析的测试域名 / Atlas 上）→ 测试成功；无法解析时显示 DNS 错误，不崩溃。
- [ ] 多 host `mongodb://host1:27017,host2:27017/db?replicaSet=rs0` 可保存；连接成功后能列出数据库。
- [ ] URI 模式启用 SSH 被拒绝并说明；`mongodb+srv` + `directConnection=true` 被拒绝；`http://` 被拒绝。
- [ ] 复制连接信息默认脱敏；含密码 URL 需二次确认；未保存密码时提示先输入。
- [ ] 导出连接配置不含 `mongo-uri` 密文；导入后提示重新输入连接串密码。
- [ ] 复制连接不复制 URI 密文；删除连接后钥匙串槽被清理。
- [ ] 只读 URI 连接执行 Mongo 写命令被拒绝；读命令正常。
- [ ] 旧版 host / port 连接配置读取、编辑、连接、复制信息行为不变。
- [ ] URI 模式下的健康检查 / 重连使用同一 URI；断线后重连不重放命令。
- [ ] 错误信息不包含密码；连接列表摘要不显示密码。
- [ ] 真实 MongoDB（本地 / Atlas / 副本集）门控回归通过；无实例时标注未完成。

#### 11. 风险 / 限制

- **SRV 依赖 DNS**：公司网络、DNS 污染、Atlas IP allowlist 都会导致失败；错误提示必须区分「DNS 失败」「TLS 失败」「认证失败」「超时」，不能统一报「连接失败」。
- **凭据拆分**：完整 URI 存钥匙串、脱敏 URI 存配置，编辑 / 复制 / 导入导出 / 模式切换都要小心，避免把 `******` 当密码或把密码写进配置。实现时优先写单元测试覆盖脱敏与注入。
- **URI 参数差异**：不同 MongoDB 版本 / 驱动对参数支持不同；未知参数必须报错而不是忽略，否则用户以为 `replicaSet` / `readPreference` 生效了。
- **SSH 与 SRV 互斥**：这是产品取舍，不是技术缺陷；必须在 UI 与错误信息中明确，避免用户反复尝试。
- **旧驱动 / 旧服务器**：Rust 驱动 3.10 对旧 MongoDB 版本的支持有限；如果用户连接老版本失败，建议升级服务器或使用主机 / 端口模式，不引入 legacy 驱动。
- **密码特殊字符**：URI userinfo 的 percent-encode 是常见坑；错误信息要给出可操作建议，不能只抛驱动原文。
- **脱敏不是万无一失**：如果 URI 的其他查询参数携带秘密（例如 `authMechanismProperties`），用参数名关键字脱敏可能漏掉；配置只保存脱敏串，完整串在钥匙串，已降低风险；仍建议用户不要把凭据写进 URI 的其他参数。

---

## I-03 MongoDB 数据库与集合管理（创建 / 删除 / 重命名 / 统计）

- **优先级**：P1（同为 P1 时按用户覆盖面排序，实施顺序排在 I-04 / I-05 之后）
- **状态**：已完成（2026-11-27，数据库新建 / 删除、集合重命名 / 统计、确认与只读保护）
- **预估**：1–1.5 个开发会话
- **范围**：
  - 后端：`src-tauri/src/commands/mongo.rs`（新增 `mongo_create_database` / `mongo_drop_database` / `mongo_rename_collection` / `mongo_collection_stats`）、`src-tauri/src/models.rs`（统计结构）、`src-tauri/src/lib.rs`（注册命令）。
  - 前端：`src/components/MongoWorkspace.vue`（数据库行菜单、集合右键菜单、统计对话框 / 抽屉）、新增 `src/components/MongoCollectionStatsDialog.vue`、可能复用 `ConfirmDialog` / `PromptDialog`、`src/lib/api.ts`、`src/lib/types.ts`、`src/styles.css`。
  - 不涉及 GridFS、用户 / 角色、事务、索引批量删除。

### 用户问题 / 场景

MongoDB 首版可以新建 / 删除集合，但不能删除数据库、重命名集合，也看不到集合的文档数、大小、存储大小、索引大小。想清理测试库、重命名一个拼错的集合、判断某个集合为什么占空间，只能去 mongosh 或 Atlas 控制台。DBX 的对象树明确提供删除数据库、创建 / 重命名 / 删除集合与集合统计；mydb 需要把这一层补齐，同时保证不可恢复操作有足够确认。

### 现状核对（基于 2026-11-26 / 27 代码）

- `mongo.rs` 已有 `mongo_create_collection` / `mongo_drop_collection`；`mongo_list_databases` / `mongo_list_collections` 只返回名字 / kind。
- `MongoWorkspace.vue` 的数据库选择器只能切换；集合右键只绑定 `deleteCollection`；没有数据库级菜单、没有重命名、没有统计。
- `MongoCollectionInfo` 只有 `name` / `kind`；没有 `count` / `size` 等统计字段。
- 只读连接通过 `ensure_writable` 拦截写命令；删除集合已有确认框（`deleteCollection` / `confirmDropCollection`）。
- 数据库创建是 MongoDB 的隐式行为：没有「空数据库」；创建集合即创建数据库。当前没有「新建数据库」入口。
- **现状标注：部分覆盖（仅集合新建 / 删除）。**

### 期望行为（可验收细节）

#### 1. 数据库级操作

1. 数据库选择器旁新增「数据库」菜单（或数据库行右侧 `…`），包含：
   - **新建数据库…**；
   - **删除当前数据库…**（只读连接禁用；系统库禁用）；
   - **刷新数据库列表**（复用现有刷新）。
2. **新建数据库**：
   - 由于 MongoDB 不能保留空数据库，对话框要求填写「数据库名称」与「首个集合名称」，并明确说明「MongoDB 会在创建首个集合时创建数据库；空数据库不会保留」。
   - 数据库名 / 集合名做基本校验（非空、长度、不能包含空字符；数据库名不能包含 `/ \ . " $ * < > : | ?`；集合名不能包含 `$`、不能以 `system.` 开头；最终以服务器错误为准）。
   - 确认后调用 `create_collection(database, firstCollection)`；成功后刷新数据库列表、切到新库、选中新集合；失败显示服务器错误，不创建半个状态（Mongo 创建集合是单步操作）。
   - 如果用户在对话框取消，不创建任何东西。
3. **删除数据库**：
   - 菜单项显示当前数据库名；只读连接禁用。
   - 点击后弹出二次确认：显示数据库名、集合数量（来自 `list_collections`，失败则显示「未知」）、`将删除该数据库下的全部集合与数据，且不可恢复`；要求用户输入数据库名（完全匹配）后才能确认。
   - 系统库 `admin`、`local`、`config` 禁止删除；前端禁用并说明，后端再次拒绝。
   - 成功后调用 `dropDatabase`；刷新数据库列表；如果删除的是当前库，自动切到列表中的第一个库（没有则显示空状态）；清空当前选中的集合；控制台草稿保留但下次执行会提示数据库不存在。
   - 失败（权限、副本集 / 分片限制）显示服务器错误；不自动重试。
4. 数据库列表刷新后保持当前选择（如果仍存在）；当前库被删除时按上述规则回退。

#### 2. 集合级操作

1. 集合右键菜单在现有「删除集合」之外新增：
   - **重命名集合…**（只读连接、视图 / timeseries 禁用）；
   - **集合统计…**（只读连接可用）；
   - 保留「删除集合」（已有确认）。
2. **重命名集合**：
   - 对话框显示「从 `<database>.<from>` 到 `<database>.<to>`」；只允许同一数据库内重命名；目标名必须非空、不能与现有集合重名；不允许 `dropTarget`（不覆盖已有集合）。
   - 视图 / timeseries 在本批禁用重命名，提示「视图 / 时间序列集合暂不支持从此处重命名」；最终以服务器能力为准。
   - 确认后调用 `renameCollection`（admin 命令，源 / 目标使用完整 namespace）；成功后刷新集合列表并选中新名称；失败显示服务器错误（权限、目标已存在、分片限制）。
   - 不复制数据、不修改文档；重命名是元数据操作，但可能锁集合，确认框提示「大量写入时可能短暂阻塞」。
3. **删除集合**：沿用现有确认；补充显示集合 kind 与「不可恢复」；只读连接禁用；成功刷新列表并清空选中。
4. **集合统计**：
   - 打开只读统计对话框 / 抽屉，显示：
     - namespace、集合类型（collection / view / timeseries）、capped；
     - `count`（服务器 `collStats` 返回的文档数；标注「服务器统计」）；
     - `size`、`storageSize`、`avgObjSize`、`totalIndexSize`、`nindexes`；
     - 每个索引的名称与大小（`indexSizes`，最多 50 个，超出折叠）；
     - timeseries 的 `timeField` / `metaField` / `granularity`（如果服务器返回）；
     - 可选 `validator` / `readOnly` 等字段（如果返回）。
   - 字节数同时显示原始值与人类可读值（KB / MB / GB，1 位小数）；计数使用千分位。
   - 支持「刷新」与「复制 JSON」；不自动轮询。
   - 视图 / timeseries 的 `collStats` 可能不可用：显示服务器错误与说明，不伪造 0。
5. 统计对话框不执行任何写操作；只读连接可用；不需要 `ensure_writable`。
6. 集合删除 / 重命名 / 数据库删除后，如果 I-01 控制台正在使用该集合 / 数据库，不强制关闭控制台；下一次运行时由服务器返回「命名空间不存在」，UI 提示刷新对象列表。

#### 3. 安全与确认

1. 只读连接：新建数据库、删除数据库、重命名集合、删除集合全部禁用（前端）并由 `ensure_writable` 拒绝（后端）；统计与刷新可用。
2. 系统库保护：`admin`、`local`、`config` 禁止删除；数据库列表仍显示，但删除菜单禁用并说明。是否允许在系统库内建集合？本批不做专门限制，按用户权限与服务器规则执行；但「新建数据库」不接受系统库名。
3. 不可恢复操作必须二次确认：
   - 删除数据库：输入数据库名；
   - 删除集合：现有确认 + 集合名；
   - 重命名：显示源 / 目标 + 目标不存在检查；
   - 所有确认框默认焦点在「取消」，`Esc` 关闭，不因回车误执行。
4. 不执行 `dropTarget`、不执行 `CASCADE`、不自动删除依赖索引（删除数据库 / 集合会由服务器删除其索引，这是 Mongo 语义，UI 说明即可）。
5. 失败后不重试、不自动刷新成「成功」；保留错误与对象列表，用户可手动刷新。

#### 4. 失败与边界

- **权限不足**：`dropDatabase` / `renameCollection` 需要相应权限；显示服务器错误码 / 名称 / 消息，提示「当前用户可能没有该操作权限」。
- **目标集合已存在**：重命名拒绝，不覆盖；提示换名或先删除目标。
- **源集合不存在**：刷新后提示「集合已不存在」；不创建空集合。
- **数据库不存在**：删除 / 统计返回错误；刷新列表。
- **系统库**：删除被阻止；新建数据库使用系统库名被拒绝。
- **视图 / timeseries**：重命名禁用；统计可能不可用；删除集合允许（按现有能力），但确认框显示 kind。
- **分片集群**：`renameCollection` / `collStats` 可能有额外限制；原样显示错误，不猜测。
- **超大数据库**：删除数据库可能耗时；本批不做进度条（Mongo `dropDatabase` 没有细粒度进度），按钮进入「删除中…」并禁用重复点击；超时由现有命令超时 / 用户等待处理。
- **并发修改**：确认后对象可能已被其他人删除 / 重命名；服务器错误如实显示，不自动重建。
- **只读连接**：所有写操作在预检阶段拒绝；不执行任何部分。
- **控制台 / 文档页状态**：删除当前集合后，文档模式显示空状态并提示刷新；不保留已删除集合的旧数据页。

#### 5. 数据与存储

- 不新增 localStorage key；数据库 / 集合选择仍使用现有内存状态（`mongoDatabaseByConnection` / `selectedCollection`）。
- 统计结果只保存在对话框内存，不持久化。
- 不写连接配置、不写钥匙串。
- 删除数据库 / 集合 / 重命名只改数据库，不改本地配置；刷新后以服务器为准。

#### 6. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| 数据库列表 | 沿用现有，无额外上限 |
| 集合列表 | 沿用现有；统计对话框只对单个集合 |
| 统计超时 | 10 秒 |
| 重命名超时 | 30 秒 |
| 删除数据库 | 使用现有命令超时 / 连接超时；按钮防重复 |
| 索引大小展示 | 最多 50 个，超出折叠 |
| 集合数量显示 | 删除确认时最多读取 1 次列表；失败显示未知 |
| 复制统计 JSON | 单次最多 1 MB；超过提示 |

#### 7. 明确不做

- 不做 GridFS bucket / 文件管理。
- 不做 MongoDB 用户 / 角色管理（只读或授权 / 回收都不做）。
- 不做 MongoDB 事务、会话、change stream。
- 不做集合复制、集合清空（`deleteMany({})`）、批量删除集合；这些属于数据操作，后续单独评估。
- 不做集合选项编辑（capped、validator、collation、timeseries 选项）与新建集合高级选项；新建仍只填名称。
- 不做索引批量删除 / 重建；沿用索引页的单条创建 / 删除。
- 不做数据库重命名（MongoDB 不支持；只能复制后删除，本批不做）。
- 不做删除进度条 / 后台任务；操作在当前命令内完成。
- 不做跨数据库移动集合（MongoDB `renameCollection` 不支持跨库；UI 明确同库）。
- 不做系统库的写保护扩展到所有集合操作；本批只保护删除数据库与新建数据库名。

#### 8. 涉及文件 / 可行落点

- `src-tauri/src/commands/mongo.rs`：
  - `mongo_create_database(app, state, connection_id, database, first_collection)`：`ensure_writable` + `create_collection`；
  - `mongo_drop_database(..., database, confirm_name)`：校验系统库 / 确认名 / `ensure_writable` + `database.drop()`；
  - `mongo_rename_collection(..., database, from, to)`：校验同库 / 非空 / 目标存在；`client.database("admin").run_command(doc!{"renameCollection": ..., "to": ..., "dropTarget": false})`；
  - `mongo_collection_stats(..., database, collection)`：`run_command(doc!{"collStats": collection})`，解析为 `MongoCollectionStats`；
  - 复用 `client_for` / `ensure_writable`；错误信息不带凭据。
- `src-tauri/src/models.rs`：`MongoCollectionStats`（`namespace`, `kind`, `capped`, `count`, `size`, `storageSize`, `avgObjSize`, `nindexes`, `totalIndexSize`, `indexSizes: Vec<MongoIndexSize>`, `timeseries: Option<...>`, `validator: Option<Value>`）；`MongoIndexSize { name, size }`。
- `src-tauri/src/lib.rs`：注册四个命令。
- `src/lib/api.ts` / `src/lib/types.ts`：四个封装与类型。
- `src/components/MongoWorkspace.vue`：数据库菜单、集合右键项、统计对话框状态、刷新 / 选中逻辑。
- 新增 `src/components/MongoCollectionStatsDialog.vue`：统计展示、刷新、复制 JSON、错误 / 空状态；复用 `ConfirmDialog` / `PromptDialog` 做删除 / 重命名确认（如果现有组件支持输入确认；否则新增轻量对话框）。
- `src/styles.css`：统计对话框、数据库菜单、危险项、确认输入。
- Rust 单元测试：系统库拒绝、确认名不匹配、目标存在、统计字段解析、只读拦截（可通过纯函数 / 参数校验覆盖；真实数据库门控回归）。

#### 9. 验收标准

- [ ] 在临时 MongoDB 上「新建数据库」`mydb_i03`，首个集合 `seed`：数据库列表出现 `mydb_i03`，集合列表出现 `seed`，当前选中 `seed`。
- [ ] 取消新建对话框不创建任何数据库 / 集合。
- [ ] 删除 `mydb_i03`：确认框显示集合数量，输入错误名称不能确认，输入正确名称后删除；列表回到其他数据库；控制台使用该库时下次执行提示不存在。
- [ ] 尝试删除 `admin` / `local` / `config`：菜单禁用，后端拒绝，不执行。
- [ ] 只读连接：新建 / 删除数据库、重命名 / 删除集合全部被拒绝；统计可用。
- [ ] 重命名 `seed` → `seed_renamed`：列表更新并选中新名称；目标已存在时拒绝且不覆盖；视图 / timeseries 禁用并说明。
- [ ] 集合统计显示文档数、size、storageSize、avgObjSize、nindexes、totalIndexSize 与索引大小；字节数有人类可读格式；复制 JSON 可用；刷新生效。
- [ ] 统计视图 / timeseries 不可用时显示服务器错误，不显示伪造 0。
- [ ] 删除集合后文档模式清空选中并提示刷新；控制台下次执行提示不存在。
- [ ] 所有写操作失败后不自动重试；错误不含连接串 / 密码。
- [ ] 真实 MongoDB 门控回归（创建 / 删除 / 重命名 / 统计 / 权限失败）通过；无实例时标注未完成。

#### 10. 风险 / 限制

- **不可恢复操作**：删除数据库 / 集合没有回收站；确认输入与系统库保护是最后一道防线，但仍建议用户备份。UI 必须明确「不可恢复」。
- **权限差异**：`dropDatabase` 与 `renameCollection` 需要较高权限；普通应用账号可能失败。错误必须可读，不能把权限失败说成「数据库不存在」。
- **renameCollection 的锁与分片限制**：重命名可能短暂阻塞写入，分片集群可能不支持；确认框提示，失败如实显示。
- **统计口径**：`collStats` 的 `count` / `size` 是服务器统计，可能与精确 `countDocuments` 不同；UI 标注「服务器统计」，不承诺实时精确。
- **空数据库语义**：用户可能期望「新建空数据库」，但 MongoDB 不支持；必须用「首个集合」解释，避免创建后列表里看不到。
- **系统库保护范围**：本批只保护删除与新建；用户仍可在系统库内按权限操作集合。是否扩大保护需要单独评估，避免阻止合法的管理员操作。
- **与 I-01 控制台的状态同步**：删除 / 重命名后控制台草稿不会自动改；下次执行由服务器报错。UI 可提示刷新，但不在本批做自动重写 SQL。

---

## I-04 结果与表数据列显示格式器（日期 / 数字 / JSON / 脱敏 / 模板）

- **优先级**：P1
- **状态**：已完成（2026-11-27，日期 / 数字 / JSON 路径 / 脱敏 / 模板、结果会话与表数据持久化）
- **预估**：1.5–2 个开发会话
- **范围**：
  - 前端：`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`、新增 `src/components/ColumnFormatDialog.vue`、新增 `src/lib/column-format.ts`、`src/lib/grid-search.ts`（仅扩展可选的显示文本回调）、`src/lib/types.ts`（如需共享类型）、`src/styles.css`。
  - 不涉及后端、数据库、连接配置或导出格式；不修改任何数据。
- **与已有能力的关系**：E-01 查找按显示文本匹配（无格式器时行为不变）；F-02 结果筛选、G-01 选区统计、G-02 导出、G-03 图表、H-05 剖析、E-02 行详情、B-16 值查看器继续使用原始值；H-02 固定结果视图可按列名沿用同一规则；H-03 列重排 / 转置按列标识应用。

### 用户问题 / 场景

查询结果里的时间字段是 `2026-11-27T10:00:00Z`，`created_at` 是 10 位 epoch，金额是 1234567.891，JSON 字段是一整段文本，手机号 / 邮箱在截图时需要打码。当前只能看到原始值，或者导出到 Excel 再格式化；每次都要在脑子里换算。DBX 的数据表格提供全局 / 按列的日期时间格式、JSON 路径提取、值脱敏、自定义显示模板和数字 / 布尔呈现，并且明确「只改变显示，不修改数据」。mydb 需要补上这一层，让结果「看得懂」，同时保证复制、导出、筛选、统计仍然基于原始值。

### 现状核对（基于 2026-11-26 / 27 代码）

- `src/lib/grid-search.ts` 的 `formatGridCell(value)` 只处理 NULL / 对象 JSON / String；`ResultGrid.vue` 与 `TableDataView.vue` 都把它别名为 `formatCell` 用于单元格显示、标题、复制（部分路径）。
- `ResultGrid.vue` 的列头右键已有排序、筛选、冻结、隐藏、统计、列宽等；没有「显示格式」入口；`TableDataView.vue` 的列头右键同样没有格式入口。
- G-04 表视图保存筛选、排序、分页、列宽、隐藏、冻结、列顺序；没有显示格式字段。
- F-02 结果筛选按原始值比较；G-01 选区统计、H-05 剖析、G-02 导出都按原始值；E-01 查找通过 `findGridMatches` 调用 `formatGridCell`，目前等价于原始显示。
- 值查看器 `CellValueDialog` 对 JSON 做格式化 / 原始切换；行详情显示原始值；导出默认原始值。
- **现状标注：完全缺失。**

### 期望行为（可验收细节）

#### 1. 入口与标识

1. 结果网格与表数据的列头右键菜单新增「显示格式…」；工具栏可选新增「格式」按钮（打开当前列或第一列的格式对话框，列多时不强制）。
2. 已应用格式的列头显示一个小 `fx` / `format` 线性图标（`aria-label="已应用显示格式"`），hover 显示规则摘要（如「日期：YYYY-MM-DD」「脱敏：保留前 3 后 4」）；图标不遮挡排序 / 冻结 / 列宽拖拽区域。
3. 格式对话框：
   - 顶部显示列名、列类型（表数据可用 `ColumnInfo.dataType`；结果没有类型则显示「推断」或不显示）、当前 scope（表 / 结果）；
   - 规则类型选择：日期时间 / 数字 / JSON 路径 / 脱敏 / 模板；
   - 类型相关参数、预览区（当前数据前 10 个非 NULL 值）、应用 / 清除 / 取消；
   - 非法规则在对话框内提示，不允许应用；已有非法规则加载时回退原始值并在列头显示警告。
4. 结果网格的格式只在当前查询标签 / 结果集会话内有效；表数据的格式按「驱动 + 连接 + 库 / 模式 + 表 / 视图」持久化。表数据格式独立于 G-04 命名视图（第一版不写入 `TableViewProfile`），切换视图不改变格式。

#### 2. 规则类型与参数

| 类型 | 参数 | 适用值 | 输出 / 行为 |
|---|---|---|---|
| 日期时间 | 来源：自动 / ISO 字符串 / epoch 秒 / epoch 毫秒 / Extended JSON `$date`；格式：`YYYY-MM-DD`、`YYYY-MM-DD HH:mm:ss`、`YYYY-MM-DD HH:mm:ss.SSS`、`HH:mm:ss`、自定义 token；时区：本地 / UTC | 字符串、数字、`{"$date":...}` 对象 | 解析成功则按格式输出；失败显示原始值 + 警告图标；纯数字只有显式选择 epoch 才当日期，避免把 ID 当年份 |
| 数字 | 小数位 0–6；千分位；百分比；前缀 / 后缀（各最多 20 字符） | number（bigint 可选转字符串） | `1234567.891` → `1,234,567.89`、`¥1,234.57`、`12.3%`；非数字原样；百分比按值 ×100 显示，不改变底层 |
| JSON 路径 | 路径（点 / 方括号，如 `user.name`、`items[0].id`、`$.user.name`）；输出：标量文本 / 紧凑 JSON | 合法 JSON 字符串或对象 | 提取到的值格式化输出；缺失 / 非法路径 / 非 JSON 回退原始值 + 警告；禁止 `eval` / 函数 / 正则 |
| 脱敏 | 模式：全部 / 保留前 N 后 M（0–20）/ 邮箱局部；掩码字符固定 `•` 或 `*`（可选） | 字符串 | 全部 → `••••••`；保留前后 → `138••••1234`；邮箱 → `a***@example.com`；长度不足时全部掩码；NULL 仍显示 NULL |
| 模板 | 模板文本，最多 200 字符，必须包含 `{value}`；第一版只支持 `{value}`，不支持 `{column}` / `{row}` | 标量（字符串 / 数字 / 布尔） | `{value} 元`、`ID-{value}`；`{value}` 替换为原始值的字符串形式；不执行表达式；未知占位符按字面显示并提示 |

1. 日期自定义 token 至少支持 `YYYY`、`MM`、`DD`、`HH`、`mm`、`ss`、`SSS`；不支持本地化月份名 / 时区名，避免引入日期库。`YYYY-MM-DD HH:mm:ss` 使用本地时区；UTC 选项把同一时刻转成 UTC 输出。
2. 日期来源「自动」只识别：ISO 8601 字符串（含 `T`、可选时区）、`{"$date":"..."}`、`{"$date":{"$numberLong":"..."}}`；不把 `20261127`、`11/27/2026`、任意 10 位数字自动当日期，避免误判。需要时用户显式选择 epoch 秒 / 毫秒。
3. 数字格式只对 JS `number` 生效；`bigint` 可先转字符串再按数字规则（如果精度可控），无法精确格式化时原样。`NaN` / `Infinity` 原样并提示。
4. JSON 路径语法：
   - 支持 `a.b`、`a[0].b`、`$.a.b`；不支持通配符、过滤器、递归下降、函数；
   - 路径最多 200 字符、深度最多 10；
   - 对象 / 数组值在「标量文本」模式下用紧凑 JSON；在「紧凑 JSON」模式下同样输出 JSON；区别是标量字符串是否去引号；
   - 缺失字段回退原始值 + 警告，不显示 `undefined`。
5. 脱敏：
   - 只对字符串生效；数字 / 布尔 / NULL 原样（如果用户需要对数字脱敏，可先用模板转字符串，但第一版不承诺）；
   - 「保留前 N 后 M」若 `N + M >= 长度`，全部掩码并提示；
   - 邮箱模式只处理含 `@` 的字符串，保留 `@` 与域名，用户名首字符保留、其余掩码；不合格式则全部掩码。
   - **脱敏是显示层的视觉处理，不是数据安全能力**：复制原始值、导出、值查看器、行详情仍可看到原文；UI 必须在格式对话框与列头 tooltip 中说明。
6. 模板只做字符串替换，不解析表达式、不访问其他列、不做条件判断；`{value}` 出现多次全部替换；没有 `{value}` 时拒绝应用。
7. NULL / undefined 永远显示 `NULL`（沿用现有样式），不应用日期 / 数字 / JSON / 模板 / 脱敏规则；空字符串可以应用脱敏 / 模板，按字符串处理。

#### 3. 作用域与持久化

1. **表数据**：按 `driver + connectionId + database + schema + table + tableKind` 隔离，存入 `mydb.columnFormatters.v1`（version 1）：
   - 每张表最多 50 条规则；全局最多 200 个 scope；超出按最近使用淘汰；
   - 规则键使用列名；重复列名使用 `name#index`（与 G-04 `profileColumnKey` 一致）；列被删除 / 重命名后，失效规则在打开表时自动剪枝并提示；
   - 写入失败保留当前会话状态，不阻塞表格；损坏 / 旧版本 JSON 回退为空并清理；
   - 不保存单元格值、样本、SQL 或结果行。
2. **查询结果**：只在当前查询标签 / 结果集会话内有效，按列名（重复列名按显示索引）保存于内存；新结果 / 切换结果集 / 切换标签时重置；H-02 固定结果视图可按当前标签的规则按列名显示，固定快照内容不变。
3. 表数据格式与 G-04 命名视图独立：应用 / 更新 / 删除视图不改变格式；格式也不会写入视图。若后续需要「视图包含格式」，另做增强。
4. 格式规则不影响：
   - 列宽、列顺序、冻结、隐藏、排序、筛选、分页；
   - 结果行数、总行数、截断标记；
   - 表数据待提交缓冲区、撤销 / 重做、提交 SQL；
   - 数据库中的任何值。
5. 清除规则：对话框「清除」删除当前列规则；列头菜单可选「清除此列格式」；表数据格式菜单可「清除本表全部格式」（二次确认，仅删本地配置）。

#### 4. 显示与数据操作的分层

1. **显示**：网格单元格文本、单元格 `title`、虚拟滚动渲染使用格式化后的文本；列头 `fx` 标识；查找高亮基于格式化后的文本（见下）。
2. **编辑**：表数据双击编辑时，输入框显示**原始值**（不是格式化值）；提交时按原始值解析类型；保存后网格显示格式化值。行详情、批量设置、粘贴新增都使用原始值。
3. **复制**：
   - 默认「复制单元格 / 整行 / 全部 / 选区」继续复制**原始值**（与现在一致）；
   - 单元格右键新增「复制显示值」；选区摘要 / 批量复制可选「复制显示值（TSV）」；
   - 整行 JSON 始终使用原始值，避免把显示格式当成数据。
4. **导出**：CSV / JSON / XLSX / Markdown / SQL INSERT 全部使用原始值；导出对话框增加提示「显示格式不会写入导出文件」；不新增「按显示值导出」开关（避免脱敏被误当成安全导出）。
5. **查找（E-01）**：`findGridMatches` 增加可选的显示文本回调；有格式器时按格式化后的文本匹配，高亮位置对应显示文本；无格式器时与现状完全一致。查找计数、循环跳转、隐藏列规则不变。
6. **筛选（F-02）**：本地结果筛选按原始值比较；界面提示「筛选基于原始值，不受显示格式影响」。例如日期显示为 `2026-11-27`，筛选 `包含 2026-11` 仍能匹配原始 ISO 文本；如果自定义格式完全改变了文本，筛选以原始值为准。
7. **统计 / 剖析 / 图表 / 对比**：G-01 选区统计、H-05 剖析、G-03 图表、H-02 结果对比全部使用原始值；不因格式器改变数值 / 日期 / NULL 统计。
8. **值查看器 / 行详情**：显示原始值；值查看器的 JSON 格式化 / 原始切换不变；行详情可显示「显示格式：…」提示但编辑与复制值仍为原始值。
9. **转置（H-03）**：转置视图可按同一规则显示单元格文本；复制 TSV 默认原始值，可选显示值；不改变转置的 500 × 100 限制。
10. **固定结果（H-02）**：固定条 / 固定视图的网格按当前标签规则显示；对比对话框使用原始值；固定快照本身不写入格式。

#### 5. 预览与失败

1. 格式对话框预览使用当前已加载数据的前 10 个非 NULL 值；没有数据时显示「暂无样本，仍可保存规则」；预览不发起数据库请求。
2. 规则应用前做本地校验：
   - 日期自定义 token 非法、空格式 → 拒绝；
   - 数字小数位越界 → 拒绝；
   - JSON 路径为空 / 超过 200 字符 / 深度超过 10 → 拒绝；
   - 脱敏保留位数越界 → 拒绝；
   - 模板缺少 `{value}` 或超长 → 拒绝。
3. 运行期格式化失败（值不符合规则、JSON 解析失败、日期无效）：
   - 显示原始值，列头 / 单元格显示 warning 小图标，tooltip 说明「显示格式无法应用：…」；
   - 不抛异常、不阻塞虚拟滚动、不影响复制 / 导出；
   - 同一列连续失败不反复弹 toast，只在列头标记。
4. 存储损坏 / 版本不匹配：回退为空规则，表格正常显示原始值；可在设置 / 格式菜单中清理。
5. 列被删除：规则剪枝；列被重命名：旧规则失效，不自动迁移（提示可重新设置）。
6. 重复列名：规则按键 `name#index` 隔离；格式对话框显示 `name (2)`；结果会话按显示索引隔离，重新执行后如果列顺序变化，规则重置。
7. 隐藏列：格式规则保留；列重新显示时生效；自动隐藏（I-06）不影响规则。
8. 大值：超过 2 MB 的文本 / JSON 只在值查看器中处理；网格格式化仍输出字符串，但不做 JSON 深度遍历超过 1 MB 的对象（回退原始值 + 提示），避免卡顿。

#### 6. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| 表格式规则 | 50 条 / 表；200 个 scope；最近使用淘汰 |
| 规则长度 | 路径 200 字符、模板 200 字符、前后缀各 20 字符 |
| 日期 token | 固定集合；自定义格式最长 80 字符 |
| 预览样本 | 10 个值 / 次 |
| 格式化性能 | 1,000 行 × 100 列目标 < 100 ms；只格式化可见单元格；虚拟滚动不因格式化卡顿 |
| JSON 解析 | 单值超过 1 MB 不做路径提取，回退原始值并提示 |
| 持久化大小 | 单 scope 规则 JSON 目标 < 100 KB；全局 < 2 MB |

#### 7. 明确不做

- 不做全局默认格式 / 按数据类型自动应用；第一版只有按列规则。
- 不做条件格式（如「金额 > 1000 标红」）、颜色规则、图标集。
- 不做公式 / 表达式 / 脚本模板；模板只支持 `{value}`。
- 不做日期库 / 时区数据库；只支持固定 token 与本地 / UTC。
- 不做按显示值导出、按显示值筛选、按显示值统计。
- 不做把脱敏后的值写回数据库、复制到剪贴板时自动脱敏。
- 不做列格式随 G-04 命名视图保存；不写入 `TableViewProfile`。
- 不做结果格式跨重启持久化。
- 不做图片 / 二进制 / BLOB 的格式化；二进制继续按现有占位显示。
- 不做多列组合模板、跨列引用、条件分支。

#### 8. 涉及文件 / 可行落点

- 新增 `src/lib/column-format.ts`：
  - 类型：`ColumnFormatRule`（`date` / `number` / `jsonPath` / `mask` / `template`）、`ColumnFormatScope`；
  - `formatDisplayValue(value, rule): { text, warning? }`；
  - `parseDateValue(value, source, timezone)`、`applyDatePattern(date, pattern, timezone)`；
  - `formatNumber(value, options)`、`extractJsonPath(value, path)`、`maskValue(value, options)`、`applyTemplate(value, template)`；
  - `loadTableFormats(scopeKey)` / `saveTableFormats(scopeKey, rules)` / `pruneFormats(columns, rules)`；`mydb.columnFormatters.v1` version 1、上限与损坏回退；
  - 纯函数，便于单元测试（项目当前没有前端测试 runner，可先用 `npm run check` + 手工验收；如新增测试需遵守项目约定）。
- 新增 `src/components/ColumnFormatDialog.vue`：规则类型、参数、预览、校验、应用 / 清除；不直接访问数据库。
- `src/components/ResultGrid.vue`：
  - `formats` 状态（按列名 / 索引，会话内存）；
  - `formatCellForColumn(value, columnIndex)` 替换显示 / title 路径；保留 `formatGridCell` 作为原始格式化；
  - 列头右键「显示格式…」「清除此列格式」；`fx` 图标；
  - 查找调用 `findGridMatches` 时传入显示文本回调；
  - 复制默认原始值；新增「复制显示值」；
  - 固定结果视图按当前标签规则显示（如果 H-02 的固定网格是独立组件，传入同一规则）。
- `src/components/TableDataView.vue`：
  - 读取 / 写入 `mydb.columnFormatters.v1`；scope key；
  - 显示 / title 使用格式；编辑输入 / 行详情 / 批量设置 / 粘贴 / 待提交 SQL 使用原始值；
  - 列头右键入口；`fx` 图标；复制默认原始值 + 「复制显示值」；
  - 导出对话框提示原始值；G-04 视图应用不改变格式。
- `src/lib/grid-search.ts`：`findGridMatches(rows, columns, query, options)` 增加可选 `display?: (value, columnIndex) => string`；默认 `formatGridCell`；不改变现有调用行为。
- `src/lib/types.ts`：如需要共享 `ColumnFormatRule` 类型；否则放在 `column-format.ts` 并导出。
- `src/styles.css`：`fx` 图标、格式对话框、警告小图标、预览表、深浅主题。
- 不涉及 `src-tauri`。

#### 9. 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] 查询 `SELECT id, name, email, city, created_at, '{"profile":{"level":3}}' AS payload, NULL AS blank, 0 AS zero FROM users`，给 `created_at` 设日期格式 `YYYY-MM-DD`：单元格显示日期，原始值不变；`payload` 设 JSON 路径 `profile.level` 显示 `3`；`blank` 仍显示 NULL；`zero` 不被 JSON / 日期规则误处理。
- [ ] 日期来源：ISO 字符串、`{"$date":"..."}`、epoch 秒 / 毫秒分别设置；非法日期 / 非日期字符串回退原文并显示警告；10 位 ID 不自动当年份。
- [ ] 数字规则：小数位 2 + 千分位 + 前缀 `¥`；`1234567.891` 显示 `¥1,234,567.89`；非数字原样；`NaN` / `Infinity` 原样提示。
- [ ] JSON 路径：`user.name`、`items[0].id`、`$.a.b` 正常；缺失 / 非法 / 非 JSON 回退原文 + 警告；不执行函数 / 正则。
- [ ] 脱敏：全部 / 保留前 3 后 4 / 邮箱；短字符串全部掩码；NULL 仍 NULL；复制默认原始值，右键「复制显示值」得到掩码文本。
- [ ] 模板：`{value} 元`、`ID-{value}`；缺少 `{value}` 拒绝应用；超长拒绝；不解析表达式。
- [ ] 表数据 `users` 设置格式后重启应用仍生效；查询结果格式切换标签后重置；同一列在结果与表数据中的规则互不影响。
- [ ] 列重排、冻结、隐藏、转置、固定结果视图下格式跟随列标识，不串列；重复列名按 `name#index` 隔离。
- [ ] 查找按显示文本匹配（如日期显示 `2026-11-27`，查找可命中）；无格式器时查找结果与 E-01 完全一致；筛选 / 选区统计 / 剖析 / 图表 / 对比仍使用原始值。
- [ ] 复制整行 JSON、导出 CSV / JSON / XLSX / SQL INSERT 使用原始值；导出对话框提示「显示格式不会写入导出文件」。
- [ ] 格式规则非法 / 存储损坏 / 列被删除时回退原文并提示，不阻塞表格；至少保留正常显示与复制。
- [ ] 1,000 行 × 100 列结果滚动 / 格式化不卡顿；1 MB 以上 JSON 不做路径提取并提示。
- [ ] 深浅主题下 `fx` 图标、警告、对话框可读；键盘可操作；脱敏提示明确「不是安全边界」。

#### 10. 风险 / 限制

- **脱敏不是安全能力**：这是本项最重要的产品风险。用户可能以为脱敏后复制 / 导出不会泄露；UI 必须在格式对话框、列头 tooltip、复制菜单中说明「只影响显示，原始值仍可复制 / 导出 / 查看」。
- **日期误判**：自动识别只能覆盖 ISO / Extended JSON；epoch 必须显式选择。不要为了「聪明」把 10 位 ID、订单号当年份。
- **格式与数据混淆**：编辑、行详情、复制、导出必须使用原始值；只有网格显示和查找使用格式化文本。实现时要把 `formatGridCell`（原始）与 `formatDisplayValue`（显示）严格分开，避免把格式化值写回数据库。
- **查找语义变化**：有格式器时查找按显示文本，可能与 F-02 筛选（原始值）不一致；UI 要分别说明。无格式器时保持现状。
- **重复列名 / 列顺序**：结果按显示索引、表数据按 `name#index`；重新执行 / 重排列后规则可能不匹配，必须安全回退，不能串列。
- **JSON 路径性能**：大 JSON 深拷贝 / 解析可能卡顿；限制 1 MB 与深度 10，失败回退。
- **时区**：本地 / UTC 两种；不做完整 IANA 时区选择，避免引入日期库。夏令时边界由浏览器 `Date` 处理，文档说明。
- **持久化增长**：每表 50 条、全局 200 scope，规则 JSON 很小，但仍要版本化与损坏回退；不要把样本值写进去。

---

## I-05 SQL 参数化查询（执行前参数面板）

- **优先级**：P1
- **状态**：已完成（2026-11-27，`:name` / SQLite/MySQL `?`、类型转义、原始 SQL 确认、敏感掩码、事务 / 多语句 / EXPLAIN / 历史原模板）
- **预估**：1.5–2 个开发会话
- **范围**：
  - 前端：`src/components/SqlEditor.vue`（参数徽标 / 检测入口）、`src/components/QueryWorkspace.vue`（参数按钮 / 面板状态 / 执行回调）、`src/App.vue`（执行流程、危险 SQL、历史、事务、只读、分屏 / 独立窗口）、新增 `src/components/SqlParametersDialog.vue`、新增 `src/lib/sql-parameters.ts`、`src/lib/types.ts`、`src/styles.css`。
  - 复用：G-05 `tokenizeSql()`（已识别 `parameter` token）、`src/lib/sql-statements.ts` 的 `splitSqlStatements` / `hasSqlContent`、`src/lib/sql-literal.ts` 的驱动感知字符串转义、`src/lib/dangerous-sql.ts`。
  - 不涉及后端命令变化；参数在客户端替换后走现有 `execute_query` / `execute_transaction_query` / `explain_query`。
- **与已有能力的关系**：F-05 执行范围决定「执行哪段 SQL」；I-05 只决定「这段 SQL 里的占位符替换成什么」。G-05 静态检查对占位符保持不透明；危险 SQL 在替换后的 SQL 上检查；历史保存原始 SQL。

### 用户问题 / 场景

排查问题时经常复用同一条 SQL：`SELECT * FROM users WHERE city = '上海' AND id > 100`。每次改城市或 ID 都要在编辑器里改字面量，容易忘改、容易引号转义错，也不方便保留一条「模板」。DBX 和 DBeaver 都支持执行前参数输入：SQL 里写 `:city`、`:min_id` 或 `?`，运行时弹框填值；DBX 还支持字符串、数字、布尔、NULL 和原始 SQL 类型。mydb 目前只能手工改 SQL，缺少参数化这一层。

### 现状核对（基于 2026-11-26 / 27 代码）

- `src/lib/sql-check.ts` 的 `tokenizeSql()` 已支持 `parameter` token：`?`、`?NNN`、`:name`、`@name`、`$N` 等；这些 token 目前只用于静态检查（检查时大多跳过），没有参数提取、替换或 UI。
- `src/lib/sql-literal.ts` 已按驱动转义字符串（MySQL 反斜杠 / 单引号，PostgreSQL / SQLite 单引号）；`sqlLiteral()` 可格式化 JSON 值，但当前用于导出 / 行复制，不是参数面板。
- 执行链路：`App.vue::runQueryOnTab()` 先取选区 / 当前语句 / 整个标签，再做危险 SQL 检查，最后 `executeSqlOnTab()` 调 `api.executeQuery` 或 `api.executeTransactionQuery`；多语句在 `executeSqlOnTab()` 里用 `splitSqlStatements()` 拆分后逐条执行。
- SQL 历史由后端 `record_query_history_timed()` 记录传入的 SQL；当前没有「原始 SQL / 展开 SQL」区分。
- 手动事务 `begin_transaction` / `execute_transaction_query` / `commit` / `rollback` 已存在；参数替换必须复用同一事务 session。
- SQL 片段的 `${cursor}` 是现有占位符（`sql-snippets.ts`），与本项命名冲突；因此第一版不支持 `${name}` 参数语法。
- **现状标注：完全缺失。**

### 期望行为（可验收细节）

#### 1. 支持的占位符语法（第一版保守子集）

1. **命名参数 `:name`**：所有 SQL 驱动支持。名称规则 `[A-Za-z_][A-Za-z0-9_]*`，最长 64 字符；区分大小写（`:City` 与 `:city` 是两个参数）；同名出现多次共用一个值。
2. **位置参数 `?`**：仅 MySQL / SQLite，且仅单条语句。按出现顺序编号 `?1`、`?2`…；参数个数必须与占位符个数一致。PostgreSQL 禁用 `?`，因为 `?`、`?|`、`?&` 是 JSONB 运算符，无法可靠区分。
3. **不支持**：
   - `${name}`：与现有 SQL 片段 `${cursor}` 冲突，第一版不识别；
   - `#{name}`：不是三种驱动的原生占位符，且容易与 MyBatis 语法混淆；
   - `@name`：MySQL 用户变量 / SQL Server 变量语义，不能安全替换；
   - `$1` / `$2`：PostgreSQL 原生位置参数，当前执行层不是 prepared statement，替换语义不明确；
   - `?NNN` / `:1` 等编号形式；第一版只支持 `?` 顺序与 `:name`。
   遇到不支持的写法不误报、不替换；在运行前提示「该占位符语法暂不支持」。
   同一条语句只允许一种参数语法：`:name` 与 `?` 混用时拒绝运行并提示拆分，避免命名与位置顺序冲突。
4. 占位符只在 SQL 代码区域识别：字符串、双引号 / 反引号 / 方括号标识符、注释内的 `:name` / `?` 不识别；PostgreSQL 的 `::type` 强制转换不识别为参数（检查前一个字符是否为 `:`）；MySQL `:=` 不识别；`?` 在 PostgreSQL 中一律不识别（JSONB 运算符）。
5. 参数数量上限：每次执行最多 50 个唯一参数；SQL 文本最多 100 KB（沿用 G-05 上限）；单个参数值最多 10 KB；超过拒绝并提示。

#### 2. 检测与入口

1. 用户点「运行」或 `⌘/Ctrl+Enter` 时，先按 F-05 确定实际执行的 SQL 片段（选区 / 当前语句 / 整个标签），再检测参数。
2. 如果没有参数：行为与现在完全一致，不弹参数面板。
3. 如果有参数且当前标签没有「已确认参数快照」，打开 `SqlParametersDialog`：
   - 显示参数列表（命名参数按首次出现顺序；位置参数按 `?1`… 编号）；
   - 每个参数显示：名称 / 序号、出现的语句序号（多语句时）、类型选择、值输入、NULL 复选框、原始 SQL 复选框（带警告）、敏感标记（可选）；
   - 底部「预览最终 SQL」可展开；「运行」「取消」。
4. 如果当前标签已有参数快照且 SQL 文本与参数列表未变化，直接复用快照值执行；工具栏「参数」徽标显示参数个数，点击可重新编辑；「清除参数」删除快照。
5. 编辑器工具栏可显示「参数：N」徽标（仅在检测到参数时），点击打开面板；不要求实时波浪线。
6. 多语句：
   - 命名参数可跨语句共用；面板按首次出现顺序列出，并标注「用于第 1、3 条语句」；
   - 位置参数只允许单条语句；多语句含 `?` 时拒绝运行并说明「位置参数 ? 仅支持单条语句；请改用 :name 或分别执行」；
   - 每条语句单独替换、单独检查危险 SQL、单独执行；一条失败默认停止后续（沿用现有多语句逻辑）。
7. 分屏 / 独立查询窗口：各自维护参数快照，不共享；同一标签的左右分屏如果复制了同一 SQL，参数值不自动同步。

#### 3. 参数面板与类型

1. 参数类型：
   - **字符串**（默认）：按驱动用单引号转义；MySQL 处理反斜杠 / `\0` / 单引号；PostgreSQL / SQLite 处理单引号与 NUL。
   - **数字**：只接受有限十进制 / 科学计数法；输出不带引号；`NaN` / `Infinity` 拒绝。
   - **布尔**：输出 `TRUE` / `FALSE`（三种驱动都接受；SQLite 也接受）。
   - **NULL**：输出 `NULL`，不需要值输入。
   - **原始 SQL**：原样插入（不引号、不转义）；必须显式勾选「我确认这是可信 SQL 片段」；用于 `ORDER BY` 列名、`LIMIT` 片段、函数等无法用字符串参数表达的场合。
2. 值校验：
   - 字符串：允许空字符串；空字符串与「未填」必须区分（未填阻止运行；显式空串合法）；
   - 数字：`^-?\d+(\.\d+)?([eE][+-]?\d+)?$`；超出 JS 安全整数时提示可能丢精度，建议按字符串 + 数据库转换；
   - 布尔：下拉选择，不允许自由文本；
   - 原始 SQL：非空；不得包含顶层分号（替换后必须仍是单条语句）；不得以注释开头绕过检查；
   - 名称参数重复出现共用值；位置参数必须全部填写。
3. 敏感标记：
   - 可选复选框「敏感」；勾选后预览中该值显示为 `'••••'`，执行仍使用真实值；
   - 敏感值不写入历史、不写入 localStorage、不在错误提示中回显；
   - 默认不勾选；密码 / token 类参数建议用户勾选。
4. 预览：
   - 显示替换后的 SQL（按语句分组）；敏感值掩码；原始 SQL 参数用不同颜色 / 标记；
   - 提供「复制预览」但复制的是掩码版本；不提供复制真实值。
5. 面板记忆：
   - 参数值只保存在当前标签的会话内存；切换标签保留（同一查询标签内），关闭标签 / 应用退出清空；
   - SQL 文本或参数列表变化时，旧快照失效并重新弹面板；
   - 不写入 localStorage、不写入执行历史、不写入 SQL 文件。
6. 取消面板：不执行 SQL、不写历史、不改变结果；编辑器内容不变。

#### 4. 替换与执行

1. **先分句，后替换**：按 F-05 确定 SQL 片段后，先用 `splitSqlStatements()` 按原始 SQL 分句；每条语句单独提取 / 替换参数。参数值中的分号、注释、引号不得影响分句。
2. **替换规则**：
   - 使用 `tokenizeSql()` 的 token 位置定位占位符，跳过字符串 / 注释 / 引号标识符；
   - `:name`：前一个字符不能是 `:`（避免 PostgreSQL `::`）；按名称查值；未提供则阻止运行；
   - `?`：仅 MySQL / SQLite 单语句；按顺序取值；`?` 出现在字符串 / 注释内不计数；
   - 替换后再次用 `splitSqlStatements()` 检查该语句仍是单条（防止原始 SQL 参数注入分号）；如果变成多条，拒绝并提示。
3. **危险 SQL 检查**：对**替换后的每条语句**调用 `findDangerousSql()`，而不是原始模板；这样原始 SQL 参数引入的 `DROP` / 无 WHERE `UPDATE` 也会被拦截。确认对话框显示替换后的 SQL（敏感值掩码）与参数摘要。
4. **只读连接**：对替换后的 SQL 调用 `ensure_query_allowed()`（现有后端在 `execute_query` 内执行 `sql_guard`）；原始 SQL 参数不能绕过只读。前端在运行前也可用现有只读标记提示，但后端是最终边界。
5. **事务**：如果标签处于手动事务，替换后的 SQL 通过 `executeTransactionQuery(connectionId, tabId, sql, ...)` 执行，复用同一 session；参数面板不自动提交 / 回滚；切换连接、关闭标签仍按现有规则回滚。
6. **多语句**：逐条替换、逐条危险检查、逐条执行；失败默认停止并保留已完成结果；参数面板只弹一次，所有语句共用值。执行中「第 N / M 条」逻辑不变。
7. **EXPLAIN**：参数面板同样适用于「解释」；对替换后的 SQL 调 `explain_query`，不写历史；EXPLAIN 不自动开启 / 提交事务，如果标签处于事务，沿用现有 explain 路径并保持事务状态不变。
8. **SQL 文件执行**：`SqlFileExecuteDialog` 第一版不识别参数；文件中的 `:name` / `?` 按普通 SQL 文本执行，不弹参数面板；预览中提示「文件执行不支持参数替换」。不改变文件执行流程。
9. **静态检查**：G-05 检查原始 SQL；`parameter` token 视为不透明表达式，不报未知表 / 列；不因参数面板改变检查范围、错误码或危险 SQL 提示。
10. **历史**：
    - 执行历史保存**原始 SQL**（含 `:name` / `?`），不保存展开后的值，避免密码 / token 入库；
    - 历史条目可回填编辑器；回填后再次运行会重新检测参数并复用 / 重新输入；
    - 可在历史详情中显示「参数：N 个（未保存值）」；不提供「重放上次参数」。
11. **结果**：结果面板显示「已应用 N 个参数」徽标（不显示值）；结果列、排序、筛选、导出与普通查询一致；不改变结果集结构。
12. **错误**：
    - 参数缺失 / 类型错误在客户端阻止执行，显示字段级错误；
    - 数据库错误照常显示；如果错误文本包含 SQL，前端显示原始模板或掩码后的 SQL，不回显敏感参数值；
    - 替换后语句变成多条、原始 SQL 参数包含分号时，拒绝执行并指出参数名。

#### 5. 与其他能力的关系

1. **F-05 执行范围**：参数面板只作用于实际执行的 SQL；选区 / 当前语句 / 整个标签的选择在参数检测之前完成；不改变默认范围设置。
2. **G-05 静态检查**：检查原始 SQL；参数 token 不误报；检查对话框中的行号与编辑器一致。
3. **H-01 对象导航**：点击 `:name` / `?` 不导航；点击参数前后的表 / 列仍按原规则。
4. **SQL 片段**：`${cursor}` 仍是片段光标占位符；本项不支持 `${name}`，避免冲突；片段插入后再写 `:name` 可正常使用。
5. **危险 SQL**：在替换后的 SQL 上检查；确认框显示替换后的 SQL；原始 SQL 参数的警告优先展示。
6. **手动事务**：复用现有 session；参数面板打开时不开启事务；取消面板不改变事务状态。
7. **独立窗口 / 分屏**：各自参数快照；主窗口参数不共享；关闭窗口 / 分屏清理快照。
8. **SQL 文件 / 片段 / 查询构建器**：第一版只在查询标签的编辑器执行流程中支持参数；查询构建器生成的 SQL 不含参数；SQL 文件执行不弹参数面板。
9. **表数据筛选 / 导入 / 导出**：不使用 SQL 参数语法；避免把 `:name` 当参数。

#### 6. 失败与边界

- **未填参数**：阻止执行，面板标红；不执行任何语句、不写历史。
- **参数名拼写错误**：面板只列出 SQL 中实际出现的参数；如果用户想用某个参数但 SQL 中没写，不会出现；提供「在 SQL 中未找到参数」说明。
- **同名参数多次出现**：共用一个值；面板显示出现次数 / 语句序号。
- **位置参数数量不匹配**：阻止执行，提示「SQL 有 3 个 ?，但填写了 2 个值」。
- **PostgreSQL `::` 转换**：不识别为参数；`SELECT id::text FROM users` 不弹参数面板。
- **PostgreSQL JSONB `?`**：不识别为参数；`SELECT payload ? 'key' FROM events` 不弹参数面板；如果用户想参数化 JSONB 键，请用 `:key` + `payload ? :key`（但 PostgreSQL 的 `?` 运算符与占位符同名，第一版建议用 `jsonb_exists(payload, :key)`，文档给出示例）。
- **MySQL 用户变量 `@name`**：不识别；`SET @x = 1; SELECT @x` 原样执行。
- **字符串 / 注释中的占位符**：不识别；`SELECT ':name'`、`-- :name`、`/* ? */` 不弹面板。
- **原始 SQL 参数**：可注入任意 SQL 片段；必须显式确认；替换后必须是单条语句；危险 SQL 检查仍执行；只读连接仍拦截。
- **参数值包含分号 / 注释 / 引号**：字符串类型正确转义，不影响分句；原始 SQL 类型拒绝顶层分号。
- **SQL 过大 / 参数过多**：超过 100 KB / 50 个参数 / 10 KB 单值拒绝，给出上限。
- **事务中取消**：参数面板取消不影响事务；事务中执行失败按现有规则保留事务，用户手动回滚。
- **历史泄露**：历史只存原始 SQL；如果原始 SQL 本身包含字面量密码，仍会入库——这是现有行为，UI 提示「不要把密码写进 SQL」。
- **数据库错误回显**：某些驱动错误可能包含参数值；前端无法完全控制，敏感参数建议使用原始 SQL 以外的类型并避免在错误中回显；文档说明。

#### 7. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| SQL 文本 | 100 KB（沿用 G-05） |
| 参数数量 | 50 个唯一参数 / 次 |
| 单个值 | 10 KB |
| 参数名长度 | 64 字符 |
| 参数面板打开 | 100 KB / 50 参数目标 < 50 ms |
| 替换 | 线性扫描 token；不重复解析整篇多次 |
| 预览 | 最多显示 100 KB；超出截断预览，但执行仍用完整 SQL |
| 多语句 | 沿用现有上限；每条单独替换 / 检查 |
| 历史 | 1 条 / 次执行；不保存参数值 |

#### 8. 明确不做

- 不支持 `${name}` / `#{name}` / `@name` / `$1` / `?NNN`；只支持 `:name` 与 MySQL/SQLite 单语句 `?`。
- 不做服务端 prepared statement / 绑定协议；第一版是客户端字面量替换。
- 不做参数类型从数据库列类型自动推断；用户手选类型。
- 不做参数默认值、参数集、命名参数文件、`@set` 变量。
- 不做 SQL 文件执行、SQL 片段、查询构建器、表数据筛选的参数化。
- 不做参数值的持久化、跨标签共享、团队共享。
- 不做参数值加密存储；不保存值。
- 不做「上次参数一键重放」；历史只存原始 SQL。
- 不做自动把 `?` 转成 `:name` 或反向转换。
- 不做参数面板中的 SQL 格式化 / 语法高亮增强（沿用编辑器）。
- 不做数据库端类型转换错误的前端预判（如字符串传给整数列），由数据库返回错误。

#### 9. 涉及文件 / 可行落点

- 新增 `src/lib/sql-parameters.ts`：
  - `extractSqlParameters(sql, driver): { kind: "named" | "positional"; name; index; statementIndex; from; to }[]`；
  - `substituteSqlParameters(sql, values, driver): { sql; missing; errors; applied }`；
  - `validateParameterValue(value, type)`；
  - 复用 `tokenizeSql()` 的 token 位置；对 `::`、`:=`、字符串 / 注释做排除；纯函数，便于测试。
- 新增 `src/components/SqlParametersDialog.vue`：参数列表、类型、值、NULL、原始 SQL 确认、敏感标记、预览、校验、运行 / 取消；不直接访问数据库。
- `src/components/QueryWorkspace.vue`：
  - 工具栏「参数」徽标 / 按钮；
  - 参数面板状态、已确认快照、清除；
  - 运行回调先取 scope SQL，再交给 App 的参数流程；
  - 分屏 / 独立窗口各自状态。
- `src/components/SqlEditor.vue`：
  - 可选显示「参数：N」徽标（由 QueryWorkspace 传入）；
  - 不改变 CodeMirror 的查找 / 补全 / 撤销。
- `src/App.vue`：
  - `runQueryOnTab()` 在确定 `sqlText` 后检测参数；如有参数且无快照，打开面板并暂存 `pendingRun`；
  - 面板确认后，对每条语句替换、危险检查、只读检查、执行；
  - 多语句循环中逐条替换；事务 / explain / 分屏 / 独立窗口分支适配；
  - 历史仍传原始 SQL；结果徽标显示参数个数；
  - 关闭标签 / 切换连接时清理参数快照。
- `src/lib/types.ts`：`SqlParameterDefinition`、`SqlParameterValue`、`SqlParameterType` 等。
- `src/styles.css`：参数面板、徽标、预览、错误、敏感标记、原始 SQL 警告。
- 不涉及 `src-tauri` 命令签名；后端只接收替换后的 SQL。

#### 10. 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] `SELECT * FROM users WHERE city = :city AND id > :min_id ORDER BY id`：运行弹面板；填 `上海`、`2`；结果与直接写字面量一致；编辑器 SQL 不变。
- [ ] `SELECT * FROM users WHERE name LIKE :pattern`：填 `%张%`；字符串正确转义；单引号 / 反斜杠 / 中文正常。
- [ ] `SELECT * FROM users WHERE id = ?`（SQLite / MySQL）：单语句弹 `?1`；填数字；PostgreSQL 下同样的 SQL 不弹参数（`?` 是 JSONB 运算符），按原样执行或由数据库报错。
- [ ] PostgreSQL：`SELECT id::text FROM users WHERE id = :id` 只识别 `:id`，不把 `::text` 当参数；`SELECT payload ? 'key' FROM events` 不弹参数。
- [ ] 字符串 / 注释中的 `:name`、`?`、`-- :x`、`/* ? */` 不识别。
- [ ] 多语句 `SELECT * FROM users WHERE city = :city; SELECT * FROM orders WHERE user_id = :uid;`：一次面板填两个值，逐条执行；`?` 多语句被拒绝并提示改用 `:name`。
- [ ] 手动事务中执行带参数 UPDATE：替换后走 `executeTransactionQuery`；提交 / 回滚正常；参数面板不自动提交。
- [ ] 只读连接用 `:x` 执行 `DELETE FROM users WHERE id = :x`：被后端只读拦截；原始 SQL 参数也无法绕过。
- [ ] 危险 SQL：`:sql` 原始参数填 `1; DROP TABLE users` 被拒绝（顶层分号 / 多条语句）；填 `1 OR 1=1` 时替换后的 UPDATE / DELETE 缺少 WHERE 仍触发危险确认。
- [ ] 参数未填 / 数字非法 / 位置参数数量不匹配：阻止执行，面板标红，不写历史、不产生结果。
- [ ] 敏感参数勾选后预览显示 `'••••'`；执行使用真实值；历史 / localStorage 中没有该值。
- [ ] 执行历史保存 `:city` 原始模板，不保存 `上海`；回填后可重新填参数。
- [ ] 参数面板取消：不执行、不写历史、结果不变、编辑器不变。
- [ ] 切换标签 / 分屏 / 独立窗口：参数快照互不共享；关闭标签清理。
- [ ] SQL 静态检查：带 `:name` 的 SQL 不误报未知表 / 列；危险 SQL 检查在替换后执行。
- [ ] 100 KB / 50 参数 / 10 KB 单值上限生效；超过给出可读错误。

#### 11. 风险 / 限制

- **客户端替换不是 SQL 注入防护**：字符串参数按驱动转义，但这是开发者便利功能，不是面向不可信输入的安全边界；原始 SQL 参数可执行任意片段，必须显式确认。文档与 UI 要说明。
- **解析歧义**：`::`、`:=`、JSONB `?`、MySQL 用户变量、字符串 / 注释都是坑。第一版用保守子集 + 明确不支持，宁可少支持，不要误替换。
- **历史泄露**：历史保存原始 SQL；如果用户在 SQL 里直接写了密码，仍会入库。这是现有行为，参数化本身不能解决；UI 提示不要把敏感值写进 SQL。
- **错误回显**：数据库错误可能包含参数值；无法完全控制；敏感参数与原始 SQL 要谨慎。
- **多语句复杂度**：参数面板、危险检查、只读检查、事务、停止 / 取消、结果标签都要逐条处理；实现时先把单语句路径做稳，再扩多语句。
- **与 SQL 片段 `${cursor}` 的冲突**：第一版不支持 `${name}`，用户从 DBX / MyBatis 复制过来的写法会失败；错误提示要给出 `:name` 的替代。
- **性能**：100 KB / 50 参数的扫描应在毫秒级；不要在每次按键时重新提取（只在运行 / 打开面板 / 徽标更新时）。
- **PostgreSQL `?` 运算符**：完全禁用 `?` 参数是最安全的选择；如果用户坚持，请用 `:name` + `jsonb_exists()`，文档给示例。
- **原始 SQL 参数的类型 / 语义**：不同于字符串，替换后可能改变 SQL 结构；必须做单语句与危险检查，不能只做字符串拼接。

---

## I-06 列快速定位与空列隐藏（列搜索 / 隐藏全 NULL）

- **优先级**：P2
- **状态**：已完成（2026-11-27，列搜索、手动显隐、全 NULL / 空串自动隐藏、例外与至少保留一列）
- **预估**：0.5–1 个开发会话
- **范围**：
  - 前端：`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`、新增 `src/components/ColumnVisibilityPanel.vue`、新增 `src/lib/column-visibility.ts`、`src/styles.css`。
  - 复用：G-04 表视图的手动隐藏 / 命名视图、E-01 的网格查找、F-01 冻结列、H-03 列顺序。
  - 不涉及后端、数据库或持久化新 key（手动隐藏仍由 G-04 保存；自动隐藏是会话状态）。

### 用户问题 / 场景

一张 30–50 列的表或查询结果里，想找 `updated_at` / `remark` 列，只能横向滚动或用鼠标在列头里找；想隐藏一堆全为 NULL 的列，只能一列一列右键「隐藏此列」；表数据有列注释，但列头只显示名称，没法按注释找。DBX 的数据表格支持按列名或注释搜索列，并提供「隐藏全 NULL 列」「隐藏全 NULL / 空串列」两个开关；mydb 已有手动隐藏与命名视图，但缺少列定位与自动隐藏。

### 现状核对（基于 2026-11-26 / 27 代码）

- `ResultGrid.vue` 用 `hiddenColumns: Set<number>` 保存隐藏列；列头右键有「隐藏此列」「显示所有列」；没有列搜索、没有空列检测。
- `TableDataView.vue` 用 `hiddenNames: string[]` / `hiddenSet` 保存手动隐藏，G-04 的 `TableViewProfile.hidden` 持久化；列头右键有隐藏 / 显示全部；没有列搜索、没有空列检测；`ColumnInfo` 有 `comment` 字段。
- E-01 的 `GridFindBar` 查找的是**单元格值**，不是列名；`findGridMatches` 遍历可见列。
- G-04 的命名视图保存筛选、排序、分页、列宽、隐藏、冻结、列顺序；没有自动隐藏空列。
- **现状标注：完全缺失（列搜索与自动空列隐藏）；手动隐藏已有。**

### 期望行为（可验收细节）

#### 1. 入口与面板

1. 结果网格与表数据工具栏新增「列」按钮（图标 + 文本或 icon-only + tooltip），显示当前隐藏列数量；列头右键新增「列…」入口。
2. 点击打开 `ColumnVisibilityPanel`（popover / 侧边小面板，不遮挡整张表）：
   - 顶部搜索框：按列名 / 注释过滤列列表，忽略大小写，实时匹配；显示「匹配 X / 共 Y 列」；
   - 列列表：每行复选框 + 列名 + 类型 / 注释（表数据有注释时显示，结果没有则省略）；勾选 = 显示，取消 = 隐藏；
   - 底部操作：`全部显示`、`隐藏全 NULL 列`（单选 / 开关）、`隐藏全 NULL / 空串列`（单选 / 开关）、关闭；
   - 隐藏计数：手动隐藏与自动隐藏分开显示，例如「手动隐藏 3 列 · 自动隐藏 2 列」。
3. 面板打开时 Esc 关闭；焦点进入搜索框；Tab 在复选框 / 按钮间移动；关闭后焦点回到触发按钮。
4. 结果网格与表数据的列面板视觉一致；窄窗口下面板可滚动，最大高度不超过视口。

#### 2. 列搜索

1. 搜索只过滤**面板里的列列表**，不改变网格的显示 / 隐藏、排序、筛选、滚动位置。
2. 匹配范围：
   - 结果：列名；没有注释。
   - 表数据：列名 + `ColumnInfo.comment`（有注释时）；类型可作为辅助提示但不参与匹配（避免 `int` 匹配所有数字列）。
3. 大小写不敏感；支持中文；不把空格 / 特殊字符当正则（字面子串）。
4. 搜索无结果时显示「没有匹配的列」；清空搜索恢复全部列。
5. 列搜索不触发数据库请求；列注释来自当前已加载的 `ColumnInfo`，不额外查询。

#### 3. 手动显示 / 隐藏

1. 复选框与现有列头「隐藏此列」行为一致：
   - 至少保留一列可见；尝试隐藏最后一列时禁用并提示「至少保留一列」；
   - 手动隐藏写入现有状态：结果网格 `hiddenColumns`（会话）；表数据 `hiddenNames` + G-04 持久化；
   - 隐藏列不参与网格渲染、查找、复制、导出；行详情仍显示并标记「已隐藏」（沿用现有规则）。
2. `全部显示`：
   - 清除手动隐藏（结果 / 表数据）；
   - 把自动隐藏开关重置为「关闭」；
   - 保留列宽、列顺序、冻结、排序、筛选。
3. 列面板中的复选框与列头右键「隐藏此列 / 显示所有列」双向同步；在面板打开时从列头隐藏列，面板列表实时更新。

#### 4. 自动隐藏空列

1. 两个开关互斥（单选）：
   - **关闭**（默认）：不自动隐藏；
   - **隐藏全 NULL 列**：只把「所有已检查行都是 NULL / undefined」的列视为空列；
   - **隐藏全 NULL / 空串列**：把「所有已检查行都是 NULL / undefined 或空字符串 `""`」的列视为空列。
   DBX 提供两个独立开关；mydb 第一版做成互斥单选，避免同时开启时「NULL + 空串」覆盖「仅 NULL」的优先级歧义，交互也更接近「一个筛选档位」。
2. 「空」的定义：
   - `NULL` / `undefined` 算空；
   - `""` 只在第二档算空；
   - 空白串（`"   "`）、`0`、`false`、`"0"`、`[]`、`{}` 都**不算空**（与 DBX 文档一致，避免把合法的 0 / false 隐藏）；
   - 列中混合 NULL 与 `""` 时，第二档算空，第一档不算。
3. 检查范围：
   - **结果网格**：当前已加载的全部结果行（受结果集上限 1,000 行），**不受 F-02 本地筛选影响**；面板提示「基于已加载 N 行，不含本地筛选」。
   - **表数据**：当前页的显示行（含待提交插入 / 更新 overlay，**不含**标记删除的行）；面板提示「基于当前页 N 行，含待提交修改」；翻页 / 筛选 / 排序后重新计算。
   - 结果为空 / 表数据当前页为 0 行：禁用两个自动隐藏开关，提示「当前没有数据，无法判断空列」；如果开关已开而数据变空，暂不隐藏任何列，数据恢复后重新计算。
4. 与手动隐藏的关系：
   - 自动隐藏是**额外的临时隐藏集合**，不修改 `hiddenNames` / `TableViewProfile.hidden`；
   - 关闭自动隐藏后，手动隐藏的列仍然隐藏；
   - 手动隐藏的列即使不是空列也保持隐藏；自动隐藏的列如果被用户手动「显示」，该列从自动隐藏集合中排除（本次会话），直到数据 / 开关重新计算？——第一版简化为：自动隐藏集合只由数据和开关决定，手动复选框显示自动隐藏列时，把该列加入「自动隐藏例外」集合（会话内存），重新计算时保留例外；「全部显示」清空例外。
5. 至少保留一列：
   - 如果自动隐藏会隐藏所有列，保留最左侧第一列，并在面板 / 状态栏提示「所有列都为空，已保留第一列」；
   - 不因为自动隐藏而让网格无列可显示。
6. 自动隐藏列与手动隐藏列一样：
   - 不参与网格渲染、查找、复制、导出；
   - 仍可作为 F-02 筛选条件（筛选基于原始值，不看可见性）；
   - 行详情仍显示并标记「已隐藏（自动）」；
   - 列宽 / 冻结 / 列顺序配置保留，重新显示时恢复。
7. 自动隐藏不改变：
   - 行数、总数、分页、排序、筛选条件；
   - 表数据待提交缓冲区、撤销 / 重做、提交 SQL；
   - 数据库中的任何值；
   - G-04 命名视图的 `hidden` 字段。
8. 切换结果集 / 重新执行 / 切换表 / 切换分页 / 应用命名视图时：
   - 自动隐藏开关重置为「关闭」，自动隐藏集合清空，例外清空；
   - 手动隐藏按现有规则保留（结果会话内 / 表数据视图 / G-04）。
9. 性能：
   - 结果最多 1,000 行 × 500 列；表数据最多 500 行 × 500 列；
   - 逐列扫描，遇到非空值立即停止该列；目标 < 30 ms；
   - 超过 10,000 行 / 500 列只检查前 10,000 行 / 前 500 列并提示；
   - 不触发数据库请求，不阻塞虚拟滚动；计算在数据变化后一次完成，不在滚动时重复。

#### 5. 显示与状态

1. 工具栏「列」按钮显示隐藏总数（手动 + 自动）；tooltip 分别列出「手动隐藏 X · 自动隐藏 Y」。
2. 状态栏 / 面板提示自动隐藏依据（行数、是否含待提交、是否不含筛选）；不弹出打断式 toast。
3. 自动隐藏导致当前活动单元格 / 选区所在列被隐藏时：
   - 选区按现有规则清空或夹到可见列；不访问隐藏列；
   - 不自动滚动到隐藏列。
4. 列头 `fx`（I-04）与自动隐藏无冲突；隐藏列不显示 `fx`，重新显示后恢复。
5. 列搜索词在面板关闭后清空；重新打开面板为空搜索。

#### 6. 失败与边界

- **空结果 / 空页**：自动隐藏开关禁用；不隐藏任何列。
- **所有列都为空**：只保留第一列，提示；不显示空白网格。
- **只有一列**：手动隐藏禁用；自动隐藏即使该列为空也保留它。
- **手动隐藏全部**：现有规则阻止最后一列；面板复选框同步禁用。
- **列在数据变化后新增 / 删除**：重新计算；已不存在的列从隐藏 / 例外集合中剪枝。
- **重复列名**：结果用列索引、表数据用 `name#index`；面板显示 `name (2)`；隐藏按列标识，不按名称误伤同名另一列。
- **列注释**：只有表数据有；结果搜索只匹配列名，不报错。
- **待提交数据**：表数据自动隐藏包含待提交 overlay；如果用户新增了一行且某列有值，该列不应被自动隐藏；标记删除的行不参与。
- **筛选后**：结果自动隐藏不看 F-02 筛选（按已加载全部行），避免筛选变化导致列忽隐忽现；面板明确提示。
- **大数据**：超过上限只检查前 N 行 / 列并提示；不得卡死。
- **应用命名视图**：自动隐藏重置；手动隐藏来自视图；不把自动隐藏写回视图。

#### 7. 数据与存储

- 结果自动隐藏 / 例外 / 搜索：组件会话内存，不持久化。
- 表数据自动隐藏 / 例外 / 搜索：组件会话内存，不持久化；切换表 / 页 / 视图后重置。
- 手动隐藏：沿用现有存储（结果会话内存；表数据 G-04 `mydb.tableViews.v1` / 隐藏列配置）。
- 不新增 localStorage key；不写连接配置 / 数据库。

#### 8. 性能 / 上限

| 项目 | 上限 / 目标 |
|---|---|
| 分析行数 | 结果 1,000 行；表数据 500 行；超过 10,000 行只检查前 10,000 |
| 分析列数 | 500 列；超过只检查前 500 并提示 |
| 计算耗时 | 1,000 × 500 目标 < 30 ms；逐列短路 |
| 列搜索 | 500 列目标 < 10 ms |
| 面板渲染 | 500 列以内；超过分页 / 滚动，不一次性渲染全部 |
| 例外集合 | 每视图最多 500 列 |

#### 9. 明确不做

- 不做按列名 / 注释的全局对象搜索（已有 B-25 / 对象搜索）。
- 不做按值查找（已有 E-01）。
- 不做自动隐藏「全 0 / 全 false / 全空白串」列；只做 NULL / 空串两档。
- 不做按列类型 / 列宽 / 使用频率排序。
- 不做列分组 / 列收藏 / 列别名。
- 不做自动隐藏结果写入 G-04 命名视图；不改变手动隐藏的持久化。
- 不做服务端列裁剪或 `SELECT` 列重写。
- 不做表数据转置（H-03 只做只读查询结果转置）。
- 不做列面板拖拽排序（列顺序已有 H-03）。
- 不做自动隐藏动画 / 过渡，避免虚拟滚动错位。

#### 10. 涉及文件 / 可行落点

- 新增 `src/components/ColumnVisibilityPanel.vue`：搜索、列列表、复选框、自动隐藏开关、隐藏计数、例外、关闭 / 焦点管理；通过 props / emit 与两个网格交互。
- 新增 `src/lib/column-visibility.ts`：
  - `hiddenByNulls(rows, columns, mode: "null" | "null-empty", options)`：返回隐藏列索引 / 键；
  - `columnMatches(column, query)`：名称 / 注释匹配；
  - `pruneHidden(set, columns)`、`mergeHidden(manual, auto, exceptions)`；
  - 纯函数，便于测试。
- `src/components/ResultGrid.vue`：
  - 工具栏「列」按钮 + 面板；
  - `autoHidden: Set<number>`、`autoMode: "off" | "null" | "null-empty"`、`autoExceptions`；
  - `visibleColumns` 排除手动 + 自动隐藏；
  - 至少保留一列；隐藏计数；列头右键「列…」；
  - 结果集 / 重新执行 / 筛选变化时按规则重置自动状态（筛选变化不重置，按已加载行）；
  - 与 E-01 查找、F-02 筛选、G-01 统计、H-03 列顺序 / 冻结对齐。
- `src/components/TableDataView.vue`：
  - 同样的面板与状态；自动隐藏基于当前页 `displayRows`（含待提交、排除删除）；
  - `hiddenNames` 手动状态不变；自动隐藏集合独立；
  - 切换分页 / 表 / 应用视图时重置自动状态；
  - 与 G-04 视图、行详情「已隐藏」标记、导出提示对齐。
- `src/styles.css`：列面板、搜索框、复选框、自动隐藏开关、隐藏计数、警告提示、深浅主题。
- 不涉及 `src-tauri`。

#### 11. 验收标准（可直接在 `examples/demo.db` 执行）

- [ ] 查询 `SELECT * FROM users`（5 列）：打开「列」面板，搜索 `mail` 命中 `email`；搜索结果只过滤列表，不改变网格列。
- [ ] 表数据 `users` 面板搜索列注释（给临时表加注释）能命中；结果网格没有注释时不报错。
- [ ] 手动取消勾选 `city`：列隐藏；列头右键「显示所有列」与面板「全部显示」同步；至少保留一列的限制生效。
- [ ] 查询包含全 NULL 列、全空串列、`0`、`false`、空白串：选「隐藏全 NULL」只隐藏全 NULL 列；选「隐藏全 NULL / 空串」再隐藏全空串列；`0` / `false` / 空白串列不被隐藏。
- [ ] 空结果 / 表数据空页：自动隐藏开关禁用并提示；数据恢复后可重新开启。
- [ ] 表数据当前页有待提交新增行，其中某列有值：自动隐藏不隐藏该列；标记删除的行不参与判断。
- [ ] 结果有 F-02 本地筛选：自动隐藏仍按已加载全部行计算（不含筛选），面板提示正确；筛选变化不导致列忽隐忽现。
- [ ] 所有列都为空：自动隐藏只保留第一列并提示；不出现无列网格。
- [ ] 手动隐藏 3 列 + 自动隐藏 2 列：工具栏徽标显示 5，tooltip 分开显示；「全部显示」清空手动并关闭自动。
- [ ] 隐藏列不参与 E-01 查找 / 复制 / 导出；仍可作为 F-02 筛选条件；行详情显示并标记「已隐藏」。
- [ ] 重新执行查询 / 切换表 / 翻页 / 应用 G-04 命名视图：自动隐藏重置为关闭，手动隐藏按现有规则保留；自动隐藏不写入 `TableViewProfile`。
- [ ] 重复列名按 `name#index` 隔离，隐藏一个不会误伤另一个。
- [ ] 1,000 行 × 100 列计算 < 30 ms；10,000 行上限提示；滚动不卡顿。
- [ ] 深浅主题、键盘 Tab / Esc、aria-label 可用。

#### 12. 风险 / 限制

- **“空列”口径**：用户可能认为 `0` / `false` / 空白串是空；按 DBX 文档与数据安全考虑，只把 NULL / 空串算空，避免隐藏合法值。面板要解释。
- **导出范围变化**：自动隐藏列与手动隐藏列一样不进入复制 / 导出；用户可能不知道导出少列。工具栏徽标、面板计数、导出对话框提示必须明确。
- **筛选联动**：结果自动隐藏按已加载全部行而不是筛选后行，避免列在筛选时忽隐忽现；如果用户期望按筛选结果判断，面板要说明。第一版不提供切换，避免状态复杂。
- **表数据待提交**：必须包含 overlay、排除删除行；否则用户可能看不到刚输入的值。实现时复用现有 `displayRows`，不要直接扫 `rows`。
- **性能**：逐列短路；不要每帧计算；数据 / 分页 / 模式变化时计算一次。
- **状态叠加**：手动隐藏、自动隐藏、自动例外、G-04 视图、列顺序、冻结、格式器（I-04）叠加，容易出现隐藏集合与列索引错位；统一使用列标识（结果索引 / 表数据 `name#index`）并写单元测试 / 手工验收。
- **自动隐藏与查找 / 选区**：隐藏当前活动单元格所在列时要安全处理选区，不能访问越界索引。
- **第一版不做持久化**：用户重启后自动隐藏重置；这是有意的，避免「上次隐藏了哪些空列」在不同数据下产生误导。

---

## 附录 I-A：测试数据与验证建议

### A.1 SQLite 示例库

- 生成示例库：`sqlite3 examples/demo.db < examples/seed.sql`；`users` 4 行 5 列、`orders` 4 行 5 列、`order_items` 4 行 5 列、`active_users` 视图。
- I-04 / I-05 / I-06 都可在 `examples/demo.db` 验证；I-01 / I-02 / I-03 需要临时 MongoDB。

### A.2 列格式器 / 列工具测试查询

```sql
SELECT
  id,
  name,
  email,
  city,
  created_at,
  '{"profile":{"level":3,"tags":["a","b"]}}' AS payload,
  NULL AS all_null,
  '' AS empty_text,
  '   ' AS spaces,
  0 AS zero_value,
  '0' AS zero_text,
  '2026-11-27T10:00:00Z' AS iso_utc,
  '2026-11-27 10:00:00' AS iso_local,
  1764237600 AS epoch_seconds,
  '13800138000' AS phone,
  'user@example.com' AS mail
FROM users
ORDER BY id;
```

- 宽结果（列搜索 / 空列隐藏 / 虚拟滚动）：

```sql
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x + 1 FROM cnt WHERE x < 1000
)
SELECT
  x AS id,
  'row-' || x AS name,
  x * 2 AS double_value,
  x % 7 AS bucket,
  NULL AS always_null,
  '' AS always_empty,
  0 AS always_zero,
  '2026-11-27T10:00:00Z' AS created_at,
  '{"a":{"b":' || x || '}}' AS payload
FROM cnt;
```

- 重复列名：

```sql
SELECT id AS value, name AS value, city AS label FROM users;
```

- 全 NULL / 全空串 / 0 / false / 空白串的独立表（SQLite）：

```sql
CREATE TABLE IF NOT EXISTS mydb_i06_empty_demo (
  id INTEGER PRIMARY KEY,
  only_null TEXT,
  only_empty TEXT,
  zero_value INTEGER,
  false_value INTEGER,
  spaces TEXT
);
INSERT INTO mydb_i06_empty_demo (id, only_null, only_empty, zero_value, false_value, spaces)
VALUES (1, NULL, '', 0, 0, '   '), (2, NULL, '', 0, 0, '   ');
```

### A.3 SQL 参数化测试

```sql
-- 命名参数
SELECT * FROM users
WHERE city = :city AND id > :min_id
ORDER BY id;

-- 字符串转义（单引号 / 反斜杠 / 中文）
SELECT * FROM users WHERE name LIKE :pattern;

-- 位置参数（SQLite / MySQL；PostgreSQL 下不要用 ?）
SELECT * FROM users WHERE id = ?;

-- PostgreSQL：:: 不是参数
SELECT id::text FROM users WHERE id = :id;

-- PostgreSQL：JSONB ? 运算符不是参数（使用 jsonb_exists 做参数化）
SELECT * FROM events WHERE jsonb_exists(payload, :key);

-- 多语句命名参数
SELECT * FROM users WHERE city = :city;
SELECT * FROM orders WHERE user_id = :uid;

-- 危险 SQL：替换后缺少 WHERE
UPDATE users SET city = :city;
DELETE FROM users WHERE id = :id;
```

- 参数类型：字符串（含空串、引号、反斜杠）、数字（负数 / 小数 / 科学计数法）、布尔、NULL、原始 SQL（`ORDER BY :sort` 用原始 SQL 片段）。
- 事务：`BEGIN; UPDATE users SET city = :city WHERE id = :id;` 在手动事务标签中执行，确认参数替换后仍走事务 session。

### A.4 MongoDB 临时数据

在 mongosh 或任意 MongoDB 客户端中创建临时库（测试结束删除）：

```javascript
use mydb_i01

db.users.insertMany([
  {
    name: "张三",
    city: "上海",
    status: "active",
    age: 30,
    phone: "13800138000",
    email: "zhangsan@example.com",
    createdAt: ISODate("2026-01-15T08:00:00Z"),
    profile: { level: 3, tags: ["vip", "new"] }
  },
  {
    name: "李四",
    city: "北京",
    status: "inactive",
    age: 28,
    phone: "13900139000",
    email: "lisi@example.com",
    createdAt: ISODate("2026-02-20T12:30:00Z"),
    profile: { level: 1, tags: ["basic"] }
  },
  {
    name: "王五",
    city: "上海",
    status: "active",
    age: 35,
    phone: "13700137000",
    email: "wangwu@example.com",
    createdAt: ISODate("2026-03-05T18:45:00Z"),
    profile: { level: 5, tags: ["vip"] }
  }
]);

db.orders.insertMany([
  { user_id: 1, status: "paid", amount: 120.5, created_at: ISODate("2026-04-01T10:00:00Z") },
  { user_id: 1, status: "pending", amount: 80, created_at: ISODate("2026-04-02T11:00:00Z") },
  { user_id: 2, status: "paid", amount: 200, created_at: ISODate("2026-04-03T12:00:00Z") },
  { user_id: 3, status: "cancelled", amount: 50, created_at: ISODate("2026-04-04T13:00:00Z") }
]);
```

- 控制台（Extended JSON）等价写法：

```javascript
db.users.find({"city":"上海"}).sort({"age":-1}).limit(10)
db.users.find({"createdAt":{"$gte":{"$date":"2026-01-01T00:00:00Z"}}})
db.users.findOne({"_id":{"$oid":"<替换为真实 ObjectId>"}})
db.users.countDocuments({"status":"active"})
db.users.estimatedDocumentCount()
db.users.distinct("city")
db.orders.aggregate([
  {"$match":{"status":"paid"}},
  {"$group":{"_id":"$user_id","total":{"$sum":"$amount"}}},
  {"$sort":{"total":-1}}
])
db.users.insertOne({"name":"测试","city":"深圳","age":30})
db.users.updateOne({"name":"测试"},{"$set":{"age":31}})
db.users.deleteOne({"name":"测试"})
```

- 危险确认：`db.users.deleteMany({})`、`db.users.updateMany({}, {"$set":{"status":"unknown"}})`、`db.users.findOneAndDelete({})`。
- 只读拒绝：用只读连接执行 `insertOne` / `updateOne` / `deleteMany`。
- 上限：构造 1,001 个文档的 `insertMany` 数组（可脚本生成）；构造超过 500 个结果的 `find`。
- I-03：`mydb_i03` + `seed` 集合；重命名为 `seed_renamed`；统计；删除；系统库 `admin` / `local` / `config` 删除被拒。

### A.5 MongoDB URI / SRV 测试

- 本地 / 容器：`mongodb://user:password@127.0.0.1:27017/mydb_i01?authSource=admin`
- 多 host / 副本集：`mongodb://host1:27017,host2:27017/mydb_i01?replicaSet=rs0&authSource=admin`
- Atlas / SRV：`mongodb+srv://user:password@cluster0.example.mongodb.net/mydb_i01?retryWrites=true&w=majority`
- TLS CA：`mongodb://host:27017/db?tls=true&tlsCAFile=/path/to/ca.pem`
- 失败用例：`http://...`、`mongodb+srv://...` + `directConnection=true`、URI + SSH、密码未 percent-encode、DNS 不存在的 SRV。
- 验证 `connections.json` 不含密码；复制信息默认脱敏；含密码复制需确认；导出配置不含密文。

### A.6 真实回归与环境变量

- 当前 README 已有 MySQL / PostgreSQL 门控测试变量（`MYDB_TEST_MYSQL_*` / `MYDB_TEST_PG_*`）。本批新增 MongoDB 建议使用：
  - `MYDB_TEST_MONGO_URI`：完整测试连接串（如 `mongodb://user:pass@127.0.0.1:27017`）；
  - `MYDB_TEST_MONGO_DB`：测试数据库前缀（默认 `mydb_test`，测试后删除）；
  - 可选 `MYDB_TEST_MONGO_SRV_URI`：单独验证 SRV；
  - 可选 `MYDB_TEST_MONGO_TLS=1`：验证 TLS / CA。
- 测试必须创建随机 / 固定前缀的临时数据库与集合，结束后删除；不得在真实业务库上执行 `dropDatabase` / `deleteMany({})`。
- 若开发环境仍无 MongoDB 实例，交付说明必须明确「真实服务器回归未完成」，并列出待执行的用例；不能把模拟数据 / 单元测试当作真实回归。

## 附录 I-B：参考资料

### DBX（Apache-2.0，仅参考功能与公开文档）

- 仓库主页 / README：<https://github.com/t8y2/dbx>
- MongoDB 工作台（URI / SRV、shell 命令、结果编辑、GridFS、数据库 / 集合管理、统计）：<https://dbxio.com/en/docs/mongodb>
- 数据表格（列格式器、按列名 / 注释搜索、隐藏全 NULL / 空串列、导出范围）：<https://dbxio.com/en/docs/data-grid>
- 查询编辑器（参数占位符、查询历史、SQL Library、执行范围）：<https://dbxio.com/en/docs/query-editor>
- 配置导出 / 导入（AES-256-GCM、PBKDF2、包含密码）：<https://dbxio.com/en/docs/config-export>
- 文档源码示例（仅用于确认行为，不复制）：<https://github.com/t8y2/dbx/tree/main/docs/content/docs>

### DBeaver（Apache-2.0，仅参考公开官方文档）

- MongoDB（JavaScript / 部分 mongo shell、当前数据库）：<https://dbeaver.com/docs/dbeaver/23.1/MongoDB/>
- SQL 执行与参数绑定（`:name`、`?`、`@set`、Bind parameter(s)）：<https://dbeaver.com/docs/dbeaver/SQL-Execution/>
- Data Editor（Grid / Record、列显示与顺序）：<https://dbeaver.com/docs/team-edition/26.0/desktop/Data-Editor/>
- Calc 面板（选中单元格统计）：<https://dbeaver.com/docs/dbeaver/Calc-Panel/>

### Navicat（专有软件，仅参考公开产品页 / 手册）

- Navicat for MongoDB（集合 / 文档、Find Builder、Aggregate Builder、用户 / 角色、事务）：<https://www.navicat.com.cn/products/navicat-for-mongodb.html>
- Navicat 18 手册（MongoDB 用户 / 角色、查询构建器、服务器安全）：<https://www.navicat.com/manual/pdf_manual/en/navicat_18/linux_manual/navicat_en.pdf>
- 查询结果固定 / 对比（免费版 / 查询编辑能力）：<https://www.navicat.com/en/products/query-editing>

### MongoDB 官方文档（命令语义与连接串）

- 连接串（`mongodb://` / `mongodb+srv://`、SRV、TLS、authSource）：<https://www.mongodb.com/docs/manual/reference/connection-string/>
- `collStats`：<https://www.mongodb.com/docs/manual/reference/command/collStats/>
- `renameCollection`：<https://www.mongodb.com/docs/manual/reference/command/renameCollection/>
- `dropDatabase`：<https://www.mongodb.com/docs/manual/reference/command/dropDatabase/>
- `find` / `aggregate` / `countDocuments` / `distinct`：<https://www.mongodb.com/docs/manual/reference/command/find/>、<https://www.mongodb.com/docs/manual/reference/command/aggregate/>

### 本仓库

- `README.md`：v0.12.0 功能基线与已知限制。
- `CHANGELOG.md`：0.12.0 与 `[Unreleased]`。
- `docs/BACKLOG.md`：B / C / D / E / F / G / H 系列历史状态与长期探索。
- `docs/ARCHITECTURE.md`：命令 / 驱动 / 前端分层约束。
- `docs/PM-REQUIREMENTS-2026-11-26-batch6.md`：H 批需求与验收（已全部完成）。
- `src-tauri/src/commands/mongo.rs`、`src/components/MongoWorkspace.vue`：MongoDB 首版实现。
- `src/lib/sql-check.ts`、`src/lib/sql-statements.ts`、`src/lib/sql-literal.ts`：I-05 可复用的 tokenizer / 分句 / 字面量转义。
- `src/lib/grid-search.ts`、`src/components/ResultGrid.vue`、`src/components/TableDataView.vue`：I-04 / I-06 的落点。

> 竞品功能仅用于确认「用户是否需要」与「交互应该怎样」，不得复制其源码、品牌图标、截图或专有文案。mydb 保持独立实现与 Apache-2.0 许可。

---

## 本批不包含的大项（汇总）

以下方向在本批 0.6 中已逐项评估，均**不进入 I-01～I-06**，避免后续误以为遗漏：

| 大项 | 结论 / 去向 |
|---|---|
| MongoDB GridFS | DBX 有，但属于文件存储工作流；留到后续 Mongo 专项。 |
| MongoDB 查询结果网格编辑 / 待提交 `updateOne` / `deleteOne` | DBX 有；I-01 结果先只读，后续单独做行身份与待提交语义。 |
| MongoDB 用户 / 角色管理 | DBX 公开文档未记录；Navicat 有；需要 RBAC / 安全审计，另开安全专项。 |
| MongoDB 事务 | DBX 公开文档未记录；Navicat 有；需要副本集 / 分片集群与 session 生命周期，另开专项。 |
| MongoDB explain / profiler / 可视化聚合构建器 | DBX 发布说明 / Navicat 有；I-01 第一版不做。 |
| 完整 JS / mongosh 语法 | I-01 只接受 Extended JSON；完整 JS 解析器风险高。 |
| 跨驱动数据传输 / 逐列映射 | 前几批已列为架构专项；本批不重复。 |
| 连接配置加密导出 / 导入（含密码） | DBX 有；mydb 保持「导出不含密码」的安全策略，另行评估。 |
| SQL Library 嵌套文件夹 / Quick Open | mydb 已有片段库 + SQL 文件，优先级低。 |
| 多工作表 XLSX / SQL UPDATE 导出 | mydb 已有主力导出格式，属于格式增强。 |
| AI SQL 助手 / MCP / Docker Web / CLI | 与本地轻量桌面客户端定位冲突。 |
| HTTP / SOCKS 代理、多级 SSH 跳板 | 已有单级 SSH；网络层另做。 |
| 全表数据剖析 / 服务端聚合 / 表数据转置 | 沿用 H-05 / H-03 的已有限制。 |
| 列格式器全局默认 / 按类型默认 | I-04 第一版只做按列规则。 |
| 固定结果 / 历史结果持久化 | 沿用 H-02 的会话内存策略。 |
| 自动重放 Mongo 写命令 / 自动重试 | 断线后写命令可能已生效，明确不做。 |
| 连接健康检查 / 重连的真实 MySQL / PostgreSQL / Redis / SSH 断网回归 | 属于测试验收（H-04 已知缺口），不是本批功能需求；另排 QA 任务，不占用 I 批排期。 |

---

## 附：I 批变更记录

- **2026-11-27**：创建本文件。确认 E-01～E-05、F-01～F-05、G-01～G-05、H-01～H-05 与 MongoDB 首版均已完成，不重复提案；完成 DBX（重点）/ DBeaver / Navicat 第五批差距核对；确定 I-01～I-06 六项需求与推荐实施顺序 **I-01 → I-02 → I-04 → I-05 → I-03 → I-06**；若只能做一项做 I-01，若只能做两项做 I-01 + I-02，若只能做三项做 I-01 + I-02 + I-04。
- 本文件只写需求，**未修改任何源代码、版本号、构建产物、连接配置、README、CHANGELOG 或 BACKLOG**；实现完成后再由实现者同步 `docs/BACKLOG.md` 的状态与 `CHANGELOG.md` 的 `[Unreleased]`。
- 本批不包含：MongoDB GridFS、MongoDB 查询结果网格编辑、MongoDB 用户 / 角色、MongoDB 事务、MongoDB explain / profiler / 可视化聚合构建器、完整 JS 语法、跨驱动传输、加密连接配置导出、SQL Library 文件夹、多工作表 XLSX / UPDATE 导出、AI / MCP / Docker Web / CLI、代理 / 多级 SSH、全表剖析 / 表数据转置、列格式器全局默认、结果持久化、Mongo 写命令自动重试（详见 0.6 与末尾汇总表）。
- 竞品参考边界：DBX（Apache-2.0）与 DBeaver（Apache-2.0）只参考公开文档与交互；Navicat 为专有软件，只参考公开产品页 / 手册；**不复制源代码、品牌图标、截图或专有文案**；mydb 保持独立实现与 Apache-2.0 许可。

