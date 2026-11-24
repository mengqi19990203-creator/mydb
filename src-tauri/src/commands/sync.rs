use super::{AppState, context_label, resolve_source, resolve_target};
use crate::db::DatabaseDriver;
use crate::db::driver::rename_ddl_object;
use crate::models::{ColumnInfo, ForeignKeyInfo, IndexInfo};
use crate::storage;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnChange {
    pub name: String,
    pub source_type: String,
    pub target_type: String,
    pub details: Vec<String>,
    pub suggested_sql: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTableDiff {
    pub name: String,
    /// same | changed | onlySource | onlyTarget
    pub status: String,
    pub source_rows: Option<u64>,
    pub target_rows: Option<u64>,
    pub added_columns: Vec<String>,
    pub missing_columns: Vec<String>,
    pub changed_columns: Vec<ColumnChange>,
    pub added_indexes: Vec<String>,
    pub extra_indexes: Vec<String>,
    pub changed_indexes: Vec<String>,
    pub added_foreign_keys: Vec<String>,
    pub extra_foreign_keys: Vec<String>,
    pub changed_foreign_keys: Vec<String>,
    pub sql: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    pub source_label: String,
    pub target_label: String,
    pub same_count: usize,
    pub changed_count: usize,
    pub only_source_count: usize,
    pub only_target_count: usize,
    pub tables: Vec<SchemaTableDiff>,
    pub sql: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TableSnapshot {
    pub name: String,
    pub ddl: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub rows: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaSnapshot {
    pub label: String,
    pub driver: String,
    pub tables: Vec<TableSnapshot>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiffOptions {
    pub include_indexes: bool,
    pub include_foreign_keys: bool,
    pub include_drops: bool,
}

impl DiffOptions {
    #[cfg(test)]
    pub(crate) const ALL: Self = Self {
        include_indexes: true,
        include_foreign_keys: true,
        include_drops: true,
    };
}

#[tauri::command]
pub async fn compare_schemas(
    app: AppHandle,
    state: State<'_, AppState>,
    source_connection_id: String,
    target_connection_id: String,
    source_database: Option<String>,
    source_schema: Option<String>,
    target_database: Option<String>,
    target_schema: Option<String>,
    source_table: Option<String>,
    target_table: Option<String>,
    include_indexes: Option<bool>,
    include_foreign_keys: Option<bool>,
    include_drops: Option<bool>,
) -> Result<SchemaDiff, String> {
    let connections = storage::load_connections(&app)?;
    let source_config = connections
        .iter()
        .find(|item| item.id == source_connection_id)
        .ok_or_else(|| "源连接不存在".to_string())?;
    let target_config = connections
        .iter()
        .find(|item| item.id == target_connection_id)
        .ok_or_else(|| "目标连接不存在".to_string())?;

    if source_config.driver != target_config.driver {
        return Err("只支持同一类型数据库之间对比".to_string());
    }
    if source_config.driver == "redis" {
        return Err("Redis 不支持结构对比".to_string());
    }

    let options = DiffOptions {
        include_indexes: include_indexes.unwrap_or(true),
        include_foreign_keys: include_foreign_keys.unwrap_or(true),
        include_drops: include_drops.unwrap_or(false),
    };
    let source_table = source_table
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());
    let target_table = target_table
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| source_table.clone());
    if source_table.is_none() && target_table.is_some() {
        return Err("请选择要同步的源表".to_string());
    }

    let resolved_source = resolve_source(
        &app,
        &state,
        source_config,
        &source_connection_id,
        source_database.as_deref(),
        source_schema.as_deref(),
    )
    .await?;
    let resolved_target = resolve_target(
        &app,
        &state,
        source_config,
        &resolved_source.driver,
        &target_connection_id,
        target_database.as_deref(),
        target_schema.as_deref(),
    )
    .await?;

    let source_label = context_label(
        &source_config.name,
        &source_config.driver,
        resolved_source.database.as_deref(),
        resolved_source.schema.as_deref(),
    );
    let target_label = context_label(
        &target_config.name,
        &source_config.driver,
        resolved_target.database.as_deref(),
        resolved_target.schema.as_deref(),
    );

    let source = if let Some(table) = source_table.as_deref() {
        snapshot_table(
            resolved_source.driver.as_ref(),
            &source_label,
            &source_config.driver,
            table,
            true,
        )
        .await?
    } else {
        snapshot(
            resolved_source.driver.as_ref(),
            &source_label,
            &source_config.driver,
        )
        .await?
    };
    let target = if let Some(table) = target_table.as_deref() {
        snapshot_table(
            resolved_target.driver.as_ref(),
            &target_label,
            &source_config.driver,
            table,
            false,
        )
        .await?
    } else {
        snapshot(
            resolved_target.driver.as_ref(),
            &target_label,
            &source_config.driver,
        )
        .await?
    };

    if source_table.is_some() {
        let source_diff = source
            .tables
            .first()
            .ok_or_else(|| "源表不存在".to_string())?;
        let target_table_name = target_table.as_deref().unwrap_or(&source_diff.name);
        let table_diff = match target.tables.first() {
            Some(target_diff) => compare_table(
                source_diff,
                target_diff,
                &source_config.driver,
                options,
            ),
            None => only_source_table(
                source_diff,
                &source_config.driver,
                options,
                Some(target_table_name),
            ),
        };
        return Ok(assemble_diff(
            &source.label,
            &target.label,
            &source.driver,
            vec![table_diff],
        ));
    }

    Ok(diff_snapshots(&source, &target, options))
}

async fn snapshot(
    driver: &dyn DatabaseDriver,
    label: &str,
    driver_name: &str,
) -> Result<SchemaSnapshot, String> {
    let mut tables = Vec::new();
    for table in driver
        .list_tables()
        .await?
        .into_iter()
        .filter(|table| table.kind == "table")
    {
        let structure = driver.table_structure(&table.name).await?;
        let rows = driver
            .read_rows(&table.name, 1, 0, None, None, None)
            .await
            .map(|result| result.total)
            .unwrap_or(0);
        tables.push(TableSnapshot {
            name: table.name,
            ddl: structure.ddl,
            columns: structure.columns,
            indexes: structure.indexes,
            foreign_keys: structure.foreign_keys,
            rows,
        });
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(SchemaSnapshot {
        label: label.to_string(),
        driver: driver_name.to_string(),
        tables,
    })
}

async fn snapshot_table(
    driver: &dyn DatabaseDriver,
    label: &str,
    driver_name: &str,
    table_name: &str,
    require: bool,
) -> Result<SchemaSnapshot, String> {
    let exists = driver
        .list_tables()
        .await?
        .into_iter()
        .any(|table| table.name == table_name && table.kind == "table");
    if require && !exists {
        return Err(format!("源表不存在: {table_name}"));
    }
    let mut tables = Vec::new();
    if exists {
        let structure = driver.table_structure(table_name).await?;
        let rows = driver
            .read_rows(table_name, 1, 0, None, None, None)
            .await
            .map(|result| result.total)
            .unwrap_or(0);
        tables.push(TableSnapshot {
            name: table_name.to_string(),
            ddl: structure.ddl,
            columns: structure.columns,
            indexes: structure.indexes,
            foreign_keys: structure.foreign_keys,
            rows,
        });
    }
    Ok(SchemaSnapshot {
        label: label.to_string(),
        driver: driver_name.to_string(),
        tables,
    })
}

pub(crate) fn diff_snapshots(
    source: &SchemaSnapshot,
    target: &SchemaSnapshot,
    options: DiffOptions,
) -> SchemaDiff {
    let source_map: HashMap<&str, &TableSnapshot> = source
        .tables
        .iter()
        .map(|table| (table.name.as_str(), table))
        .collect();
    let target_map: HashMap<&str, &TableSnapshot> = target
        .tables
        .iter()
        .map(|table| (table.name.as_str(), table))
        .collect();

    let mut names: Vec<&str> = source
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .chain(target.tables.iter().map(|table| table.name.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();

    let mut tables = Vec::new();
    for name in names {
        match (source_map.get(name), target_map.get(name)) {
            (Some(src), Some(dst)) => {
                tables.push(compare_table(src, dst, &target.driver, options));
            }
            (Some(src), None) => {
                tables.push(only_source_table(src, &target.driver, options, None));
            }
            (None, Some(dst)) => {
                tables.push(only_target_table(dst, &target.driver, options))
            }
            (None, None) => {}
        }
    }

    assemble_diff(&source.label, &target.label, &target.driver, tables)
}

pub(crate) fn assemble_diff(
    source_label: &str,
    target_label: &str,
    driver: &str,
    tables: Vec<SchemaTableDiff>,
) -> SchemaDiff {
    let mut same_count = 0;
    let mut changed_count = 0;
    let mut only_source_count = 0;
    let mut only_target_count = 0;
    for table in &tables {
        match table.status.as_str() {
            "same" => same_count += 1,
            "changed" => changed_count += 1,
            "onlySource" => only_source_count += 1,
            "onlyTarget" => only_target_count += 1,
            _ => {}
        }
    }

    let mut script = String::new();
    script.push_str(&format!(
        "-- mydb schema diff: {source_label} -> {target_label}\n-- driver: {driver}\n"
    ));
    for table in &tables {
        if table.sql.is_empty() {
            continue;
        }
        script.push_str(&format!("\n-- table: {} ({})\n", table.name, table.status));
        for statement in &table.sql {
            script.push_str(statement);
            script.push('\n');
        }
    }

    SchemaDiff {
        source_label: source_label.to_string(),
        target_label: target_label.to_string(),
        same_count,
        changed_count,
        only_source_count,
        only_target_count,
        tables,
        sql: script,
    }
}

fn compare_table(
    source: &TableSnapshot,
    target: &TableSnapshot,
    driver: &str,
    options: DiffOptions,
) -> SchemaTableDiff {
    let target_columns: HashMap<&str, &ColumnInfo> = target
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();
    let source_columns: HashMap<&str, &ColumnInfo> = source
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();

    let mut added_columns = Vec::new();
    let mut missing_columns = Vec::new();
    let mut changed_columns = Vec::new();
    let mut added_indexes = Vec::new();
    let mut extra_indexes = Vec::new();
    let mut changed_indexes = Vec::new();
    let mut added_foreign_keys = Vec::new();
    let mut extra_foreign_keys = Vec::new();
    let mut changed_foreign_keys = Vec::new();
    let mut sql = Vec::new();
    let mut warnings = Vec::new();

    for column in &source.columns {
        match target_columns.get(column.name.as_str()) {
            None => {
                sql.push(format!(
                    "ALTER TABLE {} ADD COLUMN {};",
                    quote(&target.name, driver),
                    column_definition(column, driver)
                ));
                added_columns.push(column.name.clone());
                if !column.nullable
                    && column
                        .default_value
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
                    && target.rows > 0
                {
                    warnings.push(format!(
                        "字段 {} 为 NOT NULL 且无默认值，目标表已有 {} 行，添加可能失败",
                        column.name, target.rows
                    ));
                }
                if column.primary_key || column.auto_increment {
                    warnings.push(format!(
                        "字段 {} 包含主键 / 自增属性，ADD COLUMN 未包含这些约束，请手动补充",
                        column.name
                    ));
                }
            }
            Some(existing) => {
                let mut details = Vec::new();
                if !same_type(&column.data_type, &existing.data_type) {
                    details.push(format!(
                        "类型 {} → {}",
                        existing.data_type, column.data_type
                    ));
                }
                if column.nullable != existing.nullable {
                    details.push(format!(
                        "可空 {} → {}",
                        yes_no(existing.nullable),
                        yes_no(column.nullable)
                    ));
                }
                if normalized_default(column.default_value.as_deref())
                    != normalized_default(existing.default_value.as_deref())
                {
                    details.push(format!(
                        "默认值 {} → {}",
                        existing.default_value.as_deref().unwrap_or("无"),
                        column.default_value.as_deref().unwrap_or("无")
                    ));
                }

                if !details.is_empty() {
                    let suggested = suggested_change_sql(driver, &target.name, column);
                    changed_columns.push(ColumnChange {
                        name: column.name.clone(),
                        source_type: column.data_type.clone(),
                        target_type: existing.data_type.clone(),
                        details,
                        suggested_sql: suggested.clone(),
                    });
                    match suggested {
                        Some(statement) => {
                            warnings.push(format!(
                                "字段 {} 存在差异，已生成修改语句；执行前请人工确认数据类型转换与约束影响",
                                column.name
                            ));
                            for line in statement.lines().filter(|line| !line.trim().is_empty()) {
                                sql.push(line.to_string());
                            }
                        }
                        None => warnings.push(format!(
                            "字段 {} 存在差异，当前驱动不能直接修改类型/默认值；请重建表或手动处理",
                            column.name
                        )),
                    }
                }
            }
        }
    }

    for column in &target.columns {
        if !source_columns.contains_key(column.name.as_str()) {
            missing_columns.push(column.name.clone());
            if options.include_drops {
                sql.push(format!(
                    "ALTER TABLE {} DROP COLUMN {};",
                    quote(&target.name, driver),
                    quote(&column.name, driver)
                ));
                warnings.push(format!(
                    "目标包含源中不存在的字段 {}，已生成 DROP COLUMN；执行前请备份数据",
                    column.name
                ));
            } else {
                warnings.push(format!(
                    "目标包含源中不存在的字段 {}（未生成删除语句；勾选「包含删除语句」可生成）",
                    column.name
                ));
            }
        }
    }

    if options.include_indexes {
        let (added, extra, changed) =
            compare_indexes(source, target, driver, &mut sql, &mut warnings, options.include_drops);
        added_indexes = added;
        extra_indexes = extra;
        changed_indexes = changed;
    }

    if options.include_foreign_keys {
        let (added, extra, changed) =
            compare_foreign_keys(source, target, driver, &mut sql, &mut warnings, options.include_drops);
        added_foreign_keys = added;
        extra_foreign_keys = extra;
        changed_foreign_keys = changed;
    }

    let status = if added_columns.is_empty()
        && changed_columns.is_empty()
        && missing_columns.is_empty()
        && added_indexes.is_empty()
        && extra_indexes.is_empty()
        && changed_indexes.is_empty()
        && added_foreign_keys.is_empty()
        && extra_foreign_keys.is_empty()
        && changed_foreign_keys.is_empty()
    {
        "same"
    } else {
        "changed"
    };

    SchemaTableDiff {
        name: source.name.clone(),
        status: status.to_string(),
        source_rows: Some(source.rows),
        target_rows: Some(target.rows),
        added_columns,
        missing_columns,
        changed_columns,
        added_indexes,
        extra_indexes,
        changed_indexes,
        added_foreign_keys,
        extra_foreign_keys,
        changed_foreign_keys,
        sql,
        warnings,
    }
}

fn only_source_table(
    source: &TableSnapshot,
    driver: &str,
    options: DiffOptions,
    target_name: Option<&str>,
) -> SchemaTableDiff {
    let table_name = target_name.unwrap_or(&source.name).to_string();
    let mut sql = Vec::new();
    let mut warnings = vec!["目标缺少整张表，仅生成建表语句，不复制数据".to_string()];
    let mut added_indexes = Vec::new();
    let mut added_foreign_keys = Vec::new();
    let ddl = source.ddl.trim().trim_end_matches(';').trim();
    let mut ddl_has_indexes = false;
    let mut ddl_has_foreign_keys = false;

    if ddl.is_empty() {
        let columns = source
            .columns
            .iter()
            .map(|column| column_definition(column, driver))
            .collect::<Vec<_>>()
            .join(",\n  ");
        sql.push(format!(
            "CREATE TABLE {} (\n  {}\n);",
            quote(&table_name, driver),
            columns
        ));
        warnings.push("源 DDL 不可用，已按字段生成基础建表语句（不含主键约束）".to_string());

        if options.include_indexes {
            for index in source.indexes.iter().filter(|index| usable_index(index)) {
                sql.push(create_index_sql(driver, &table_name, index));
                added_indexes.push(index.name.clone());
            }
        }
        if options.include_foreign_keys {
            for foreign_key in &source.foreign_keys {
                match add_foreign_key_sql(driver, &table_name, foreign_key) {
                    Some(statement) => {
                        sql.push(statement);
                        added_foreign_keys.push(foreign_key.name.clone());
                    }
                    None => warnings.push(format!(
                        "外键 {} 无法通过 ALTER 添加（SQLite 需重建表）",
                        foreign_key.name
                    )),
                }
            }
        }
    } else {
        let ddl = if target_name.is_some_and(|name| name != source.name) {
            match rename_ddl_object(ddl, &quote(&table_name, driver)) {
                Ok(renamed) => {
                    warnings.push(format!("建表 DDL 已改为目标表名 {table_name}"));
                    renamed
                }
                Err(error) => {
                    warnings.push(format!(
                        "无法把建表 DDL 改成目标表名 {table_name}（{error}），请手动修改"
                    ));
                    ddl.to_string()
                }
            }
        } else {
            ddl.to_string()
        };
        sql.push(format!("{ddl};"));
        warnings.push("DDL 取自源连接，如两边模式名不同请手动调整".to_string());
        // MySQL SHOW CREATE TABLE and SQLite sqlite_master DDL already carry
        // inline indexes; PostgreSQL DDL is synthesized without them.
        ddl_has_indexes = matches!(driver, "mysql" | "sqlite");
        ddl_has_foreign_keys = true;

        if options.include_indexes {
            let indexes: Vec<&IndexInfo> = source
                .indexes
                .iter()
                .filter(|index| usable_index(index))
                .collect();
            added_indexes.extend(indexes.iter().map(|index| index.name.clone()));
            if !ddl_has_indexes {
                for index in indexes {
                    sql.push(create_index_sql(driver, &table_name, index));
                }
            }
        }
        if options.include_foreign_keys {
            added_foreign_keys.extend(
                source
                    .foreign_keys
                    .iter()
                    .map(|foreign_key| foreign_key.name.clone()),
            );
        }
    }

    if !options.include_indexes && ddl_has_indexes && !source.indexes.is_empty() {
        warnings.push("源建表 DDL 自带索引；关闭「包含索引」不会从 DDL 中移除".to_string());
    }
    if !options.include_foreign_keys && ddl_has_foreign_keys && !source.foreign_keys.is_empty() {
        warnings.push("源建表 DDL 自带外键；关闭「包含外键」不会从 DDL 中移除".to_string());
    }

    SchemaTableDiff {
        name: source.name.clone(),
        status: "onlySource".to_string(),
        source_rows: Some(source.rows),
        target_rows: None,
        added_columns: Vec::new(),
        missing_columns: Vec::new(),
        changed_columns: Vec::new(),
        added_indexes,
        extra_indexes: Vec::new(),
        changed_indexes: Vec::new(),
        added_foreign_keys,
        extra_foreign_keys: Vec::new(),
        changed_foreign_keys: Vec::new(),
        sql,
        warnings,
    }
}

fn only_target_table(
    target: &TableSnapshot,
    driver: &str,
    options: DiffOptions,
) -> SchemaTableDiff {
    let mut sql = Vec::new();
    let mut warnings = Vec::new();
    if options.include_drops {
        sql.push(format!("DROP TABLE {};", quote(&target.name, driver)));
        warnings.push("目标存在但源中不存在该表，已生成 DROP TABLE；执行前请备份数据".to_string());
    } else {
        warnings.push(
            "目标存在但源中不存在该表（未生成删除语句；勾选「包含删除语句」可生成）"
                .to_string(),
        );
    }
    SchemaTableDiff {
        name: target.name.clone(),
        status: "onlyTarget".to_string(),
        source_rows: None,
        target_rows: Some(target.rows),
        added_columns: Vec::new(),
        missing_columns: Vec::new(),
        changed_columns: Vec::new(),
        added_indexes: Vec::new(),
        extra_indexes: Vec::new(),
        changed_indexes: Vec::new(),
        added_foreign_keys: Vec::new(),
        extra_foreign_keys: Vec::new(),
        changed_foreign_keys: Vec::new(),
        sql,
        warnings,
    }
}

fn usable_index(index: &IndexInfo) -> bool {
    !index.primary
        && !index.name.starts_with("sqlite_autoindex_")
        && !index.columns.is_empty()
}

fn create_index_sql(driver: &str, table: &str, index: &IndexInfo) -> String {
    let mut statement = String::from("CREATE ");
    if index.unique {
        statement.push_str("UNIQUE ");
    }
    statement.push_str("INDEX ");
    statement.push_str(&quote(&index.name, driver));
    statement.push_str(" ON ");
    statement.push_str(&quote(table, driver));
    statement.push_str(" (");
    statement.push_str(
        &index
            .columns
            .iter()
            .map(|column| quote(column, driver))
            .collect::<Vec<_>>()
            .join(", "),
    );
    statement.push_str(");");
    statement
}

fn drop_index_sql(driver: &str, table: &str, index_name: &str) -> String {
    if driver == "mysql" {
        format!(
            "ALTER TABLE {} DROP INDEX {};",
            quote(table, driver),
            quote(index_name, driver)
        )
    } else {
        format!("DROP INDEX {};", quote(index_name, driver))
    }
}

fn compare_indexes(
    source: &TableSnapshot,
    target: &TableSnapshot,
    driver: &str,
    statements: &mut Vec<String>,
    warnings: &mut Vec<String>,
    include_drops: bool,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let target_map: HashMap<&str, &IndexInfo> = target
        .indexes
        .iter()
        .filter(|index| usable_index(index))
        .map(|index| (index.name.as_str(), index))
        .collect();
    let source_map: HashMap<&str, &IndexInfo> = source
        .indexes
        .iter()
        .filter(|index| usable_index(index))
        .map(|index| (index.name.as_str(), index))
        .collect();
    let target_definitions: HashSet<(Vec<String>, bool)> = target
        .indexes
        .iter()
        .filter(|index| usable_index(index))
        .map(|index| (index.columns.clone(), index.unique))
        .collect();
    let source_definitions: HashSet<(Vec<String>, bool)> = source
        .indexes
        .iter()
        .filter(|index| usable_index(index))
        .map(|index| (index.columns.clone(), index.unique))
        .collect();

    let mut added = Vec::new();
    let mut extra = Vec::new();
    let mut changed = Vec::new();

    for index in source.indexes.iter().filter(|index| usable_index(index)) {
        match target_map.get(index.name.as_str()) {
            None => {
                if target_definitions.contains(&(index.columns.clone(), index.unique)) {
                    continue;
                }
                statements.push(create_index_sql(driver, &target.name, index));
                added.push(index.name.clone());
            }
            Some(existing) => {
                if existing.columns != index.columns || existing.unique != index.unique {
                    changed.push(index.name.clone());
                    statements.push(drop_index_sql(driver, &target.name, &index.name));
                    statements.push(create_index_sql(driver, &target.name, index));
                    warnings.push(format!(
                        "索引 {} 定义不同（源 {}；目标 {}），已生成删除并重建语句",
                        index.name,
                        index_summary(index),
                        index_summary(existing)
                    ));
                }
            }
        }
    }

    for index in target.indexes.iter().filter(|index| usable_index(index)) {
        if source_map.contains_key(index.name.as_str()) {
            continue;
        }
        if source_definitions.contains(&(index.columns.clone(), index.unique)) {
            continue;
        }
        extra.push(index.name.clone());
        if include_drops {
            statements.push(drop_index_sql(driver, &target.name, &index.name));
            warnings.push(format!(
                "目标包含源中不存在的索引 {}，已生成删除语句",
                index.name
            ));
        } else {
            warnings.push(format!(
                "目标包含源中不存在的索引 {}（未生成删除语句；勾选「包含删除语句」可生成）",
                index.name
            ));
        }
    }

    (added, extra, changed)
}

fn index_summary(index: &IndexInfo) -> String {
    format!(
        "{}{}({})",
        if index.unique { "UNIQUE " } else { "" },
        index.name,
        index.columns.join(", ")
    )
}

fn foreign_key_key(foreign_key: &ForeignKeyInfo, driver: &str) -> String {
    if driver == "sqlite" {
        format!(
            "{}|{}|{}",
            foreign_key.columns.join(","),
            foreign_key.referenced_table,
            foreign_key.referenced_columns.join(",")
        )
    } else {
        foreign_key.name.clone()
    }
}

fn add_foreign_key_sql(
    driver: &str,
    table: &str,
    foreign_key: &ForeignKeyInfo,
) -> Option<String> {
    if driver == "sqlite"
        || foreign_key.columns.is_empty()
        || foreign_key.referenced_columns.is_empty()
    {
        return None;
    }
    let columns = foreign_key
        .columns
        .iter()
        .map(|column| quote(column, driver))
        .collect::<Vec<_>>()
        .join(", ");
    let referenced_columns = foreign_key
        .referenced_columns
        .iter()
        .map(|column| quote(column, driver))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
        quote(table, driver),
        quote(&foreign_key.name, driver),
        quote(&foreign_key.referenced_table, driver)
    );
    if !foreign_key.on_delete.trim().is_empty()
        && !foreign_key.on_delete.eq_ignore_ascii_case("NO ACTION")
    {
        statement.push_str(&format!(" ON DELETE {}", foreign_key.on_delete));
    }
    if !foreign_key.on_update.trim().is_empty()
        && !foreign_key.on_update.eq_ignore_ascii_case("NO ACTION")
    {
        statement.push_str(&format!(" ON UPDATE {}", foreign_key.on_update));
    }
    statement.push(';');
    Some(statement)
}

fn foreign_key_signature(foreign_key: &ForeignKeyInfo) -> ForeignKeySignature {
    (
        foreign_key.columns.clone(),
        foreign_key.referenced_table.clone(),
        foreign_key.referenced_columns.clone(),
        foreign_key.on_delete.to_ascii_uppercase(),
        foreign_key.on_update.to_ascii_uppercase(),
    )
}

type ForeignKeySignature = (Vec<String>, String, Vec<String>, String, String);

fn drop_foreign_key_sql(
    driver: &str,
    table: &str,
    foreign_key_name: &str,
) -> Option<String> {
    match driver {
        "sqlite" => None,
        "mysql" => Some(format!(
            "ALTER TABLE {} DROP FOREIGN KEY {};",
            quote(table, driver),
            quote(foreign_key_name, driver)
        )),
        _ => Some(format!(
            "ALTER TABLE {} DROP CONSTRAINT {};",
            quote(table, driver),
            quote(foreign_key_name, driver)
        )),
    }
}

fn compare_foreign_keys(
    source: &TableSnapshot,
    target: &TableSnapshot,
    driver: &str,
    statements: &mut Vec<String>,
    warnings: &mut Vec<String>,
    include_drops: bool,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let target_map: HashMap<String, &ForeignKeyInfo> = target
        .foreign_keys
        .iter()
        .map(|foreign_key| (foreign_key_key(foreign_key, driver), foreign_key))
        .collect();
    let source_map: HashMap<String, &ForeignKeyInfo> = source
        .foreign_keys
        .iter()
        .map(|foreign_key| (foreign_key_key(foreign_key, driver), foreign_key))
        .collect();
    let target_signatures: HashSet<ForeignKeySignature> = target
        .foreign_keys
        .iter()
        .map(foreign_key_signature)
        .collect();
    let source_signatures: HashSet<ForeignKeySignature> = source
        .foreign_keys
        .iter()
        .map(foreign_key_signature)
        .collect();

    let mut added = Vec::new();
    let mut extra = Vec::new();
    let mut changed = Vec::new();

    for foreign_key in &source.foreign_keys {
        match target_map.get(&foreign_key_key(foreign_key, driver)) {
            None => {
                if target_signatures.contains(&foreign_key_signature(foreign_key)) {
                    continue;
                }
                match add_foreign_key_sql(driver, &target.name, foreign_key) {
                    Some(statement) => {
                        statements.push(statement);
                        added.push(foreign_key.name.clone());
                    }
                    None => warnings.push(format!(
                        "外键 {}（{}）在目标缺失；SQLite 无法通过 ALTER 添加，需重建表或手动处理",
                        foreign_key.name,
                        foreign_key.columns.join(", ")
                    )),
                }
            }
            Some(existing) => {
                if existing.columns != foreign_key.columns
                    || existing.referenced_table != foreign_key.referenced_table
                    || existing.referenced_columns != foreign_key.referenced_columns
                    || !existing.on_delete.eq_ignore_ascii_case(&foreign_key.on_delete)
                    || !existing.on_update.eq_ignore_ascii_case(&foreign_key.on_update)
                {
                    changed.push(foreign_key.name.clone());
                    match drop_foreign_key_sql(driver, &target.name, &foreign_key.name) {
                        Some(drop_sql) => {
                            statements.push(drop_sql);
                            if let Some(add_sql) =
                                add_foreign_key_sql(driver, &target.name, foreign_key)
                            {
                                statements.push(add_sql);
                                warnings.push(format!(
                                    "外键 {} 定义不同，已生成删除并重建语句",
                                    foreign_key.name
                                ));
                            } else {
                                warnings.push(format!(
                                    "外键 {} 定义不同，已生成删除语句；当前驱动无法自动添加，请手动重建",
                                    foreign_key.name
                                ));
                            }
                        }
                        None => warnings.push(format!(
                            "外键 {} 定义不同，SQLite 无法直接修改，需重建表或手动处理",
                            foreign_key.name
                        )),
                    }
                }
            }
        }
    }

    for foreign_key in &target.foreign_keys {
        if source_map.contains_key(&foreign_key_key(foreign_key, driver)) {
            continue;
        }
        if source_signatures.contains(&foreign_key_signature(foreign_key)) {
            continue;
        }
        extra.push(foreign_key.name.clone());
        if include_drops {
            match drop_foreign_key_sql(driver, &target.name, &foreign_key.name) {
                Some(drop_sql) => {
                    statements.push(drop_sql);
                    warnings.push(format!(
                        "目标包含源中不存在的外键 {}，已生成删除语句",
                        foreign_key.name
                    ));
                }
                None => warnings.push(format!(
                    "目标包含源中不存在的外键 {}，SQLite 无法通过 ALTER 删除，需重建表",
                    foreign_key.name
                )),
            }
        } else {
            warnings.push(format!(
                "目标包含源中不存在的外键 {}（未生成删除语句；勾选「包含删除语句」可生成）",
                foreign_key.name
            ));
        }
    }

    (added, extra, changed)
}

fn quote(name: &str, driver: &str) -> String {
    if driver == "mysql" {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn column_definition(column: &ColumnInfo, driver: &str) -> String {
    let mut sql = format!("{} {}", quote(&column.name, driver), column.data_type);
    if !column.nullable {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = column.default_value.as_deref() {
        if !default.trim().is_empty() {
            let value = if driver == "mysql" {
                mysql_default_literal(default, &column.data_type)
            } else {
                default.to_string()
            };
            sql.push_str(&format!(" DEFAULT {value}"));
        }
    }
    sql
}

fn suggested_change_sql(driver: &str, table: &str, source: &ColumnInfo) -> Option<String> {
    let table_ref = quote(table, driver);
    let column_ref = quote(&source.name, driver);
    match driver {
        "mysql" => Some(format!(
            "ALTER TABLE {table_ref} MODIFY COLUMN {};",
            column_definition(source, "mysql")
        )),
        "postgres" => {
            let mut statements = vec![format!(
                "ALTER TABLE {table_ref} ALTER COLUMN {column_ref} TYPE {} USING {column_ref}::{};",
                source.data_type, source.data_type
            )];
            statements.push(format!(
                "ALTER TABLE {table_ref} ALTER COLUMN {column_ref} {};",
                if source.nullable {
                    "DROP NOT NULL"
                } else {
                    "SET NOT NULL"
                }
            ));
            let default = source.default_value.as_deref().unwrap_or("").trim();
            statements.push(if default.is_empty() {
                format!("ALTER TABLE {table_ref} ALTER COLUMN {column_ref} DROP DEFAULT;")
            } else {
                format!(
                    "ALTER TABLE {table_ref} ALTER COLUMN {column_ref} SET DEFAULT {default};"
                )
            });
            Some(statements.join("\n"))
        }
        _ => None,
    }
}

fn same_type(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn normalized_default(value: Option<&str>) -> String {
    value.map(str::trim).unwrap_or("").to_string()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "是"
    } else {
        "否"
    }
}

fn mysql_default_literal(value: &str, data_type: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper == "NULL"
        || upper.starts_with("CURRENT_TIMESTAMP")
        || upper.starts_with("CURRENT_DATE")
        || upper.starts_with("CURRENT_TIME")
        || upper.starts_with("LOCALTIME")
        || upper == "NOW()"
        || upper == "UUID()"
        || (trimmed.starts_with('(') && trimmed.ends_with(')'))
    {
        return trimmed.to_string();
    }
    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (upper.starts_with("B'") && trimmed.ends_with('\''))
    {
        return trimmed.to_string();
    }

    let numeric_type = [
        "TINYINT",
        "SMALLINT",
        "MEDIUMINT",
        "INT",
        "INTEGER",
        "BIGINT",
        "DECIMAL",
        "NUMERIC",
        "FLOAT",
        "DOUBLE",
        "REAL",
        "BIT",
        "BOOL",
        "BOOLEAN",
    ]
    .iter()
    .any(|kind| data_type.trim().to_ascii_uppercase().starts_with(kind));
    if numeric_type && trimmed.parse::<f64>().is_ok() {
        return trimmed.to_string();
    }

    format!(
        "'{}'",
        trimmed.replace('\\', "\\\\").replace('\'', "''")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(
        name: &str,
        data_type: &str,
        nullable: bool,
        primary_key: bool,
        auto_increment: bool,
        default_value: Option<&str>,
    ) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            nullable,
            primary_key,
            auto_increment,
            default_value: default_value.map(str::to_string),
        }
    }

    fn table(name: &str, ddl: &str, columns: Vec<ColumnInfo>, rows: u64) -> TableSnapshot {
        TableSnapshot {
            name: name.to_string(),
            ddl: ddl.to_string(),
            columns,
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            rows,
        }
    }

    #[test]
    fn diffs_tables_and_added_columns() {
        let source = SchemaSnapshot {
            label: "src".to_string(),
            driver: "sqlite".to_string(),
            tables: vec![
                table(
                    "users",
                    "CREATE TABLE users (id INTEGER PRIMARY KEY)",
                    vec![
                        column("id", "INTEGER", false, true, false, None),
                        column("email", "TEXT", true, false, false, None),
                    ],
                    10,
                ),
                table(
                    "audit",
                    "CREATE TABLE audit (id INTEGER)",
                    vec![column("id", "INTEGER", false, false, false, None)],
                    0,
                ),
            ],
        };
        let target = SchemaSnapshot {
            label: "dst".to_string(),
            driver: "sqlite".to_string(),
            tables: vec![
                table(
                    "users",
                    "CREATE TABLE users (id INTEGER PRIMARY KEY)",
                    vec![column("id", "INTEGER", false, true, false, None)],
                    5,
                ),
                table(
                    "legacy",
                    "CREATE TABLE legacy (id INTEGER)",
                    vec![column("id", "INTEGER", false, false, false, None)],
                    2,
                ),
            ],
        };

        let diff = diff_snapshots(&source, &target, DiffOptions::ALL);
        assert_eq!(diff.changed_count, 1);
        assert_eq!(diff.only_source_count, 1);
        assert_eq!(diff.only_target_count, 1);

        let users = diff
            .tables
            .iter()
            .find(|item| item.name == "users")
            .expect("users diff");
        assert!(users
            .sql
            .iter()
            .any(|sql| sql == "ALTER TABLE \"users\" ADD COLUMN \"email\" TEXT;"));
        assert!(diff.sql.contains("CREATE TABLE audit"));

        let legacy = diff
            .tables
            .iter()
            .find(|item| item.name == "legacy")
            .expect("legacy diff");
        assert_eq!(legacy.status, "onlyTarget");
        assert_eq!(legacy.sql, vec!["DROP TABLE \"legacy\";".to_string()]);
        assert!(legacy.warnings.iter().any(|text| text.contains("备份")));

        let without_drops = diff_snapshots(
            &source,
            &target,
            DiffOptions {
                include_indexes: true,
                include_foreign_keys: true,
                include_drops: false,
            },
        );
        let legacy = without_drops
            .tables
            .iter()
            .find(|item| item.name == "legacy")
            .expect("legacy diff");
        assert_eq!(legacy.status, "onlyTarget");
        assert!(legacy.sql.is_empty());
        assert!(legacy
            .warnings
            .iter()
            .any(|text| text.contains("未生成删除语句")));
    }

    #[test]
    fn generates_executable_sql_for_changed_columns() {
        let source = SchemaSnapshot {
            label: "src".to_string(),
            driver: "postgres".to_string(),
            tables: vec![table(
                "orders",
                "",
                vec![column("total", "numeric(12,2)", false, false, false, Some("0"))],
                3,
            )],
        };
        let target = SchemaSnapshot {
            label: "dst".to_string(),
            driver: "postgres".to_string(),
            tables: vec![table(
                "orders",
                "",
                vec![column("total", "numeric(10,2)", true, false, false, None)],
                3,
            )],
        };

        let diff = diff_snapshots(&source, &target, DiffOptions::ALL);
        let orders = &diff.tables[0];
        assert_eq!(orders.status, "changed");
        assert_eq!(orders.changed_columns.len(), 1);
        assert!(orders
            .sql
            .iter()
            .any(|sql| sql.contains("TYPE numeric(12,2)")));
        assert!(orders
            .sql
            .iter()
            .any(|sql| sql.contains("SET NOT NULL")));
        assert!(orders
            .sql
            .iter()
            .any(|sql| sql.contains("SET DEFAULT 0")));
        assert!(orders.warnings.iter().any(|text| text.contains("人工确认")));
        assert!(diff.sql.contains("USING \"total\"::numeric(12,2)"));
    }

    #[test]
    fn diffs_indexes_and_foreign_keys() {
        let columns = vec![
            column("id", "INTEGER", false, true, false, None),
            column("email", "TEXT", true, false, false, None),
            column("org_id", "INTEGER", true, false, false, None),
        ];

        let mut source = table(
            "users",
            "CREATE TABLE users (id INTEGER PRIMARY KEY)",
            columns.clone(),
            1,
        );
        source.indexes = vec![
            IndexInfo {
                name: "PRIMARY".to_string(),
                columns: vec!["id".to_string()],
                unique: true,
                primary: true,
            },
            IndexInfo {
                name: "idx_users_email".to_string(),
                columns: vec!["email".to_string()],
                unique: true,
                primary: false,
            },
        ];
        source.foreign_keys = vec![ForeignKeyInfo {
            name: "fk_users_org".to_string(),
            columns: vec!["org_id".to_string()],
            referenced_table: "orgs".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_delete: "CASCADE".to_string(),
            on_update: "NO ACTION".to_string(),
        }];

        let mut target = table(
            "users",
            "CREATE TABLE users (id INTEGER PRIMARY KEY)",
            columns,
            1,
        );
        target.indexes = vec![IndexInfo {
            name: "idx_old".to_string(),
            columns: vec!["email".to_string()],
            unique: false,
            primary: false,
        }];

        let source = SchemaSnapshot {
            label: "src".to_string(),
            driver: "postgres".to_string(),
            tables: vec![source],
        };
        let target = SchemaSnapshot {
            label: "dst".to_string(),
            driver: "postgres".to_string(),
            tables: vec![target],
        };

        let diff = diff_snapshots(&source, &target, DiffOptions::ALL);
        let users = &diff.tables[0];
        assert_eq!(users.status, "changed");
        assert_eq!(users.added_indexes, vec!["idx_users_email".to_string()]);
        assert!(users.extra_indexes.contains(&"idx_old".to_string()));
        assert_eq!(users.added_foreign_keys, vec!["fk_users_org".to_string()]);
        assert!(users
            .sql
            .iter()
            .any(|sql| sql == "CREATE UNIQUE INDEX \"idx_users_email\" ON \"users\" (\"email\");"));
        assert!(users
            .sql
            .iter()
            .any(|sql| sql.contains("ADD CONSTRAINT")
                && sql.contains("FOREIGN KEY")
                && sql.contains("ON DELETE CASCADE")));

        let without_options = diff_snapshots(
            &source,
            &target,
            DiffOptions {
                include_indexes: false,
                include_foreign_keys: false,
                include_drops: false,
            },
        );
        assert_eq!(without_options.tables[0].status, "same");
        assert!(without_options.tables[0].sql.is_empty());
    }

    #[test]
    fn appends_indexes_when_creating_a_missing_table() {
        let columns = vec![
            column("id", "INTEGER", false, true, false, None),
            column("email", "TEXT", true, false, false, None),
        ];
        let mut source = table(
            "users",
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT)",
            columns,
            0,
        );
        source.indexes = vec![IndexInfo {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
            primary: false,
        }];

        let source = SchemaSnapshot {
            label: "src".to_string(),
            driver: "postgres".to_string(),
            tables: vec![source],
        };
        let target = SchemaSnapshot {
            label: "dst".to_string(),
            driver: "postgres".to_string(),
            tables: Vec::new(),
        };
        let diff = diff_snapshots(&source, &target, DiffOptions::ALL);
        let users = &diff.tables[0];
        assert_eq!(users.status, "onlySource");
        assert!(users
            .sql
            .iter()
            .any(|sql| sql.starts_with("CREATE TABLE")));
        assert!(users
            .sql
            .iter()
            .any(|sql| sql == "CREATE UNIQUE INDEX \"idx_users_email\" ON \"users\" (\"email\");"));
        assert_eq!(users.added_indexes, vec!["idx_users_email".to_string()]);

        let mut mysql_source = source.tables[0].clone();
        mysql_source.ddl =
            "CREATE TABLE `users` (`id` int PRIMARY KEY, `email` text, KEY `idx_users_email` (`email`))"
                .to_string();
        let mysql_source = SchemaSnapshot {
            label: "src".to_string(),
            driver: "mysql".to_string(),
            tables: vec![mysql_source],
        };
        let mysql_diff = diff_snapshots(
            &mysql_source,
            &SchemaSnapshot {
                label: "dst".to_string(),
                driver: "mysql".to_string(),
                tables: Vec::new(),
            },
            DiffOptions::ALL,
        );
        assert_eq!(mysql_diff.tables[0].sql.len(), 1);
        assert_eq!(
            mysql_diff.tables[0].added_indexes,
            vec!["idx_users_email".to_string()]
        );
    }

    #[test]
    fn renames_create_table_when_target_table_name_differs() {
        let source = table(
            "users",
            "CREATE TABLE users (id INTEGER)",
            vec![column("id", "INTEGER", false, true, false, None)],
            0,
        );
        let diff = only_source_table(&source, "postgres", DiffOptions::ALL, Some("users_copy"));
        assert!(diff
            .sql
            .iter()
            .any(|sql| sql == "CREATE TABLE \"users_copy\" (id INTEGER);"));
        assert!(diff
            .warnings
            .iter()
            .any(|warning| warning.contains("目标表名 users_copy")));
    }

    #[test]
    fn quotes_mysql_string_defaults() {
        assert_eq!(mysql_default_literal("abc", "varchar(10)"), "'abc'");
        assert_eq!(
            mysql_default_literal("CURRENT_TIMESTAMP", "datetime"),
            "CURRENT_TIMESTAMP"
        );
        assert_eq!(mysql_default_literal("0", "int"), "0");
        assert_eq!(mysql_default_literal("O'Reilly", "text"), "'O''Reilly'");
    }
}