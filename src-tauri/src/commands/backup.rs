use super::export::run_database_export;
use super::{AppState, driver_for};
use crate::backup_store;
use crate::models::{BackupJob, BackupRun, BackupRunSummary, DatabaseExportOptions, ExportObject};
use crate::storage;
use chrono::Local;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

fn now_iso() -> String {
    Local::now().to_rfc3339()
}

fn schedule_next(job: &mut BackupJob) {
    job.next_run = if job.enabled {
        backup_store::next_run_string(&job.schedule)
    } else {
        None
    };
}

fn refresh_connection(app: &AppHandle, job: &mut BackupJob) -> bool {
    match storage::load_connections(app) {
        Ok(connections) => {
            match connections
                .into_iter()
                .find(|item| item.id == job.connection_id)
            {
                Some(config) => {
                    job.connection_name = config.name;
                    job.connection_available = config.driver != "redis";
                    job.connection_available
                }
                None => {
                    job.connection_available = false;
                    false
                }
            }
        }
        Err(_) => {
            job.connection_available = false;
            false
        }
    }
}

fn validate_job(app: &AppHandle, job: &BackupJob) -> Result<(), String> {
    if job.name.trim().is_empty() {
        return Err("计划名称不能为空".to_string());
    }
    if job.connection_id.trim().is_empty() {
        return Err("请选择连接".to_string());
    }
    let config = storage::load_connections(app)?
        .into_iter()
        .find(|item| item.id == job.connection_id)
        .ok_or_else(|| "连接不存在".to_string())?;
    if config.driver == "redis" {
        return Err("Redis 不支持逻辑备份计划".to_string());
    }
    if !job.include_structure && !job.include_data {
        return Err("请至少选择结构或数据".to_string());
    }
    if !matches!(job.table_mode.as_str(), "all" | "include" | "exclude") {
        return Err("表范围模式无效".to_string());
    }
    if job.table_mode != "all" && job.table_pattern.trim().is_empty() {
        return Err("请填写表名匹配规则".to_string());
    }
    let dir = Path::new(job.output_dir.trim());
    if !dir.is_absolute() {
        return Err("备份目录必须是绝对路径".to_string());
    }
    if job.filename_template.trim().is_empty() {
        return Err("文件名模板不能为空".to_string());
    }
    if !(1..=30).contains(&job.retention) {
        return Err("保留份数必须在 1-30 之间".to_string());
    }
    match job.schedule.kind.as_str() {
        "hourly" => {
            if !(1..=24).contains(&job.schedule.interval_hours) {
                return Err("间隔小时数必须在 1-24 之间".to_string());
            }
        }
        "daily" => {
            if job.schedule.hour > 23 || job.schedule.minute > 59 {
                return Err("每天执行时间无效".to_string());
            }
        }
        "weekly" => {
            if job.schedule.hour > 23 || job.schedule.minute > 59 || job.schedule.weekday > 6 {
                return Err("每周执行时间无效".to_string());
            }
        }
        _ => return Err("频率类型无效".to_string()),
    }

    std::fs::create_dir_all(dir).map_err(|error| format!("创建备份目录失败: {error}"))?;
    let probe = dir.join(format!(".mydb-backup-test-{}", std::process::id()));
    std::fs::write(&probe, b"mydb").map_err(|error| format!("备份目录不可写: {error}"))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[tauri::command]
pub async fn list_backup_jobs(app: AppHandle) -> Result<Vec<BackupJob>, String> {
    let mut jobs = backup_store::load_jobs(&app);
    let mut changed = false;
    for job in &mut jobs {
        let before = (job.connection_name.clone(), job.connection_available);
        refresh_connection(&app, job);
        if before != (job.connection_name.clone(), job.connection_available) {
            changed = true;
        }
    }
    if changed {
        let _ = backup_store::save_jobs(&app, &jobs);
    }
    Ok(jobs)
}

#[tauri::command]
pub async fn save_backup_job(app: AppHandle, mut job: BackupJob) -> Result<BackupJob, String> {
    validate_job(&app, &job)?;
    let mut jobs = backup_store::load_jobs(&app);
    if job.id.trim().is_empty() {
        job.id = uuid::Uuid::new_v4().to_string();
        job.last_run = None;
        job.last_status = None;
        job.last_error = None;
        job.missed = false;
    } else if let Some(existing) = jobs.iter().find(|item| item.id == job.id) {
        job.last_run = existing.last_run.clone();
        job.last_status = existing.last_status.clone();
        job.last_error = existing.last_error.clone();
    } else {
        return Err("备份计划不存在".to_string());
    }
    job.name = job.name.trim().to_string();
    job.output_dir = job.output_dir.trim().to_string();
    job.filename_template = job.filename_template.trim().to_string();
    job.table_pattern = job.table_pattern.trim().to_string();
    refresh_connection(&app, &mut job);
    job.missed = false;
    schedule_next(&mut job);

    if let Some(existing) = jobs.iter_mut().find(|item| item.id == job.id) {
        *existing = job.clone();
    } else {
        jobs.push(job.clone());
    }
    backup_store::save_jobs(&app, &jobs)?;
    Ok(job)
}

#[tauri::command]
pub async fn delete_backup_job(app: AppHandle, id: String) -> Result<(), String> {
    let mut jobs = backup_store::load_jobs(&app);
    jobs.retain(|job| job.id != id);
    backup_store::save_jobs(&app, &jobs)
}

#[tauri::command]
pub async fn list_backup_runs(app: AppHandle) -> Result<Vec<BackupRun>, String> {
    Ok(backup_store::load_runs(&app))
}

#[tauri::command]
pub async fn skip_missed_backups(app: AppHandle) -> Result<Vec<BackupJob>, String> {
    let mut jobs = backup_store::load_jobs(&app);
    for job in &mut jobs {
        if job.missed {
            job.missed = false;
            schedule_next(job);
        }
    }
    backup_store::save_jobs(&app, &jobs)?;
    Ok(jobs)
}

#[tauri::command]
pub async fn cancel_backup(state: State<'_, AppState>, operation_id: String) -> Result<(), String> {
    if let Some(flag) = state.export_cancels.lock().await.get(&operation_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let text: Vec<char> = text.to_ascii_lowercase().chars().collect();
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star, mut star_text) = (None, 0usize);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            star_text = t;
        } else if let Some(star_index) = star {
            p = star_index + 1;
            star_text += 1;
            t = star_text;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

fn selected_tables(job: &BackupJob, tables: Vec<crate::models::TableInfo>) -> Vec<ExportObject> {
    tables
        .into_iter()
        .filter(|table| table.kind == "table" || table.kind == "view")
        .filter(|table| match job.table_mode.as_str() {
            "include" => wildcard_match(&job.table_pattern, &table.name),
            "exclude" => !wildcard_match(&job.table_pattern, &table.name),
            _ => true,
        })
        .map(|table| ExportObject {
            kind: table.kind,
            name: table.name,
        })
        .collect()
}

fn start_run(job: &BackupJob, operation_id: &str, path: &str) -> BackupRun {
    BackupRun {
        id: operation_id.to_string(),
        job_id: job.id.clone(),
        job_name: job.name.clone(),
        connection_id: job.connection_id.clone(),
        connection_name: job.connection_name.clone(),
        started_at: now_iso(),
        finished_at: String::new(),
        status: "running".to_string(),
        path: path.to_string(),
        bytes: 0,
        rows: 0,
        tables: 0,
        error: None,
    }
}

async fn execute_backup(
    app: &AppHandle,
    state: &AppState,
    job: &mut BackupJob,
    operation_id: &str,
    path: &str,
    objects: Vec<ExportObject>,
) -> Result<BackupRunSummary, String> {
    let mut run = start_run(job, operation_id, path);
    run.tables = objects.len();
    let options = DatabaseExportOptions {
        include_structure: job.include_structure,
        include_data: job.include_data,
        include_drops: job.include_drops,
        create_database: false,
        remove_definer: true,
        rows_per_insert: 100,
        use_transaction: true,
    };

    let outcome = run_database_export(
        app,
        state,
        job.connection_id.clone(),
        path.to_string(),
        objects,
        options,
        Some(operation_id.to_string()),
    )
    .await;

    run.finished_at = now_iso();
    match outcome {
        Ok(summary) => {
            run.status = "success".to_string();
            run.bytes = summary.bytes;
            run.rows = summary.rows;
            run.tables = summary.object_count;
            job.last_run = Some(run.finished_at.clone());
            job.last_status = Some("success".to_string());
            job.last_error = None;
            let mut runs = backup_store::load_runs(app);
            runs.insert(0, run.clone());
            for file in backup_store::outdated_files(
                &runs,
                &job.id,
                &job.output_dir,
                job.retention as usize,
            ) {
                let _ = std::fs::remove_file(file);
            }
        }
        Err(error) => {
            let cancelled = error.contains("取消");
            run.status = if cancelled { "cancelled" } else { "failed" }.to_string();
            run.error = Some(error.clone());
            job.last_run = Some(run.finished_at.clone());
            job.last_status = Some(run.status.clone());
            job.last_error = Some(error.clone());
        }
    }
    job.missed = false;
    schedule_next(job);
    let mut jobs = backup_store::load_jobs(app);
    if let Some(existing) = jobs.iter_mut().find(|item| item.id == job.id) {
        *existing = job.clone();
        let _ = backup_store::save_jobs(app, &jobs);
    }
    let _ = backup_store::append_run(app, run.clone());
    let _ = app.emit("backup:finished", &run);

    match run.status.as_str() {
        "success" => Ok(BackupRunSummary {
            next_run: job.next_run.clone(),
            run,
        }),
        _ => Err(run.error.unwrap_or_else(|| "备份失败".to_string())),
    }
}

async fn prepare_job(
    app: &AppHandle,
    state: &AppState,
    job: &mut BackupJob,
) -> Result<(String, Vec<ExportObject>), String> {
    if !refresh_connection(app, job) {
        return Err("连接不可用，请检查连接是否已删除或驱动是否受支持".to_string());
    }
    let driver = driver_for(app, state, &job.connection_id).await?;
    let (database, schema) = driver.context().await;
    if let Some(database) = database {
        job.database = Some(database);
    }
    if let Some(schema) = schema {
        job.schema = Some(schema);
    }
    let tables = driver.list_tables().await?;
    let objects = selected_tables(job, tables);
    if objects.is_empty() {
        return Err("表范围没有匹配到任何表或视图".to_string());
    }
    let database = job
        .database
        .clone()
        .unwrap_or_else(|| job.connection_name.clone());
    let schema = job.schema.clone().unwrap_or_default();
    let filename = backup_store::render_filename(
        &job.filename_template,
        &job.connection_name,
        &database,
        &schema,
    );
    let path = Path::new(&job.output_dir).join(filename);
    std::fs::create_dir_all(&job.output_dir)
        .map_err(|error| format!("创建备份目录失败: {error}"))?;
    let path = path.to_string_lossy().into_owned();
    Ok((path, objects))
}

async fn record_early_failure(
    app: &AppHandle,
    job: &mut BackupJob,
    operation_id: &str,
    error: String,
) -> Result<BackupRunSummary, String> {
    let mut run = start_run(job, operation_id, "");
    run.finished_at = now_iso();
    run.status = if error.contains("取消") {
        "cancelled".to_string()
    } else {
        "failed".to_string()
    };
    run.error = Some(error.clone());
    job.last_run = Some(run.finished_at.clone());
    job.last_status = Some(run.status.clone());
    job.last_error = Some(error.clone());
    job.missed = false;
    schedule_next(job);
    let mut jobs = backup_store::load_jobs(app);
    if let Some(existing) = jobs.iter_mut().find(|item| item.id == job.id) {
        *existing = job.clone();
        let _ = backup_store::save_jobs(app, &jobs);
    }
    let _ = backup_store::append_run(app, run.clone());
    let _ = app.emit("backup:finished", &run);
    Err(error)
}

#[tauri::command]
pub async fn run_backup_job(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    operation_id: String,
) -> Result<BackupRunSummary, String> {
    let mut guard = state
        .backup_running
        .try_lock()
        .map_err(|_| "已有备份任务正在运行".to_string())?;
    if *guard {
        return Err("已有备份任务正在运行".to_string());
    }
    *guard = true;

    let outcome = async {
        let mut jobs = backup_store::load_jobs(&app);
        let job = jobs
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| "备份计划不存在".to_string())?;
        let prepared = prepare_job(&app, &state, job).await;
        match prepared {
            Ok((path, objects)) => {
                execute_backup(&app, &state, job, &operation_id, &path, objects).await
            }
            Err(error) => record_early_failure(&app, job, &operation_id, error).await,
        }
    }
    .await;

    *guard = false;
    outcome
}

/// Runs a job for the scheduler; returns `false` when another backup is busy.
pub(crate) async fn run_scheduled_job(app: &AppHandle, id: &str) -> bool {
    let state = app.state::<AppState>();
    let Ok(mut guard) = state.backup_running.try_lock() else {
        return false;
    };
    if *guard {
        return false;
    }
    *guard = true;
    let operation_id = format!("scheduled-{}", uuid::Uuid::new_v4());

    let jobs = backup_store::load_jobs(app);
    let latest = jobs.iter().find(|item| item.id == id).cloned();
    let Some(mut job) = latest else {
        *guard = false;
        return true;
    };
    let prepared = prepare_job(app, &state, &mut job).await;
    match prepared {
        Ok((path, objects)) => {
            let _ = execute_backup(app, &state, &mut job, &operation_id, &path, objects).await;
        }
        Err(error) => {
            let _ = record_early_failure(app, &mut job, &operation_id, error).await;
        }
    }
    *guard = false;
    true
}

/// Marks overdue jobs as missed and schedules the next future run. Called once
/// at startup so closing the app never silently triggers catch-up runs.
pub(crate) fn prepare_startup(app: &AppHandle) {
    let mut jobs = backup_store::load_jobs(app);
    let now = Local::now();
    let mut changed = false;
    for job in &mut jobs {
        refresh_connection(app, job);
        if !job.enabled || !job.connection_available {
            continue;
        }
        let Some(next) = job.next_run.as_deref() else {
            schedule_next(job);
            changed = true;
            continue;
        };
        let Ok(next) = chrono::DateTime::parse_from_rfc3339(next) else {
            schedule_next(job);
            changed = true;
            continue;
        };
        if next.with_timezone(&Local) <= now {
            job.missed = true;
            schedule_next(job);
            changed = true;
        }
    }
    if changed {
        let _ = backup_store::save_jobs(app, &jobs);
    }
}

pub(crate) fn start_scheduler(app: AppHandle) {
    prepare_startup(&app);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let now = Local::now();
            let jobs = backup_store::load_jobs(&app);
            for job in jobs {
                if !job.enabled || job.missed || !job.connection_available {
                    continue;
                }
                let Some(next) = job.next_run.as_deref() else {
                    continue;
                };
                let Ok(next) = chrono::DateTime::parse_from_rfc3339(next) else {
                    continue;
                };
                if next.with_timezone(&Local) <= now {
                    run_scheduled_job(&app, &job.id).await;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_table_wildcards() {
        assert!(wildcard_match("*", "orders"));
        assert!(wildcard_match("ord*", "orders"));
        assert!(wildcard_match("????rs", "orders"));
        assert!(wildcard_match("user?_*", "users_archive"));
        assert!(!wildcard_match("user?*", "orders"));
    }
}
