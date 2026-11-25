use crate::models::{BackupJob, BackupRun, BackupSchedule};
use chrono::{Datelike, Duration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const HISTORY_LIMIT: usize = 200;

#[derive(Debug, Serialize, Deserialize, Default)]
struct JobsFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    jobs: Vec<BackupJob>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct HistoryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    runs: Vec<BackupRun>,
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法获取配置目录: {error}"))
}

pub(crate) fn jobs_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("backup-jobs.json"))
}

pub(crate) fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("backup-history.json"))
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(path: &PathBuf) -> T {
    let Ok(raw) = fs::read_to_string(path) else {
        return T::default();
    };
    if raw.trim().is_empty() {
        return T::default();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化备份配置失败: {error}"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, raw).map_err(|error| format!("写入备份配置失败: {error}"))?;
    #[cfg(windows)]
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp, path).map_err(|error| format!("保存备份配置失败: {error}"))
}

pub(crate) fn load_jobs(app: &AppHandle) -> Vec<BackupJob> {
    let path = match jobs_path(app) {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let file: JobsFile = load_json(&path);
    file.jobs
}

pub(crate) fn save_jobs(app: &AppHandle, jobs: &[BackupJob]) -> Result<(), String> {
    let path = jobs_path(app)?;
    save_json(
        &path,
        &JobsFile {
            version: 1,
            jobs: jobs.to_vec(),
        },
    )
}

pub(crate) fn load_runs(app: &AppHandle) -> Vec<BackupRun> {
    let path = match history_path(app) {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let file: HistoryFile = load_json(&path);
    file.runs
}

pub(crate) fn append_run(app: &AppHandle, run: BackupRun) -> Result<(), String> {
    let path = history_path(app)?;
    let mut file: HistoryFile = load_json(&path);
    file.version = 1;
    file.runs.insert(0, run);
    file.runs.truncate(HISTORY_LIMIT);
    save_json(&path, &file)
}

pub(crate) fn next_run(
    schedule: &BackupSchedule,
    now: chrono::DateTime<Local>,
) -> Option<chrono::DateTime<Local>> {
    match schedule.kind.as_str() {
        "hourly" => {
            let hours = schedule.interval_hours.clamp(1, 24);
            Some(now + Duration::hours(hours as i64))
        }
        "daily" => {
            let hour = schedule.hour.min(23);
            let minute = schedule.minute.min(59);
            let today = now.date_naive().and_hms_opt(hour, minute, 0)?;
            let mut candidate = Local.from_local_datetime(&today).earliest()?;
            if candidate <= now {
                candidate = Local
                    .from_local_datetime(
                        &(now.date_naive() + Duration::days(1)).and_hms_opt(hour, minute, 0)?,
                    )
                    .earliest()?;
            }
            Some(candidate)
        }
        "weekly" => {
            let target = schedule.weekday.min(6) as i64;
            let current = now.weekday().num_days_from_monday() as i64;
            let mut delta = (target - current + 7) % 7;
            let hour = schedule.hour.min(23);
            let minute = schedule.minute.min(59);
            let mut day = now.date_naive() + Duration::days(delta);
            let mut candidate = Local
                .from_local_datetime(&day.and_hms_opt(hour, minute, 0)?)
                .earliest()?;
            if candidate <= now {
                delta += 7;
                day = now.date_naive() + Duration::days(delta);
                candidate = Local
                    .from_local_datetime(&day.and_hms_opt(hour, minute, 0)?)
                    .earliest()?;
            }
            Some(candidate)
        }
        _ => None,
    }
}

pub(crate) fn next_run_string(schedule: &BackupSchedule) -> Option<String> {
    next_run(schedule, Local::now()).map(|value| value.to_rfc3339())
}

/// Expands the filename template. Path separators and `..` are removed so a
/// template cannot escape the chosen output directory.
pub(crate) fn render_filename(
    template: &str,
    connection: &str,
    database: &str,
    schema: &str,
) -> String {
    let now = Local::now();
    let filename = template
        .replace("{connection}", connection)
        .replace("{database}", database)
        .replace("{schema}", schema)
        .replace("{date}", &now.format("%Y%m%d").to_string())
        .replace("{time}", &now.format("%H%M%S").to_string())
        .replace("{timestamp}", &now.format("%Y%m%d-%H%M%S").to_string());
    let cleaned: String = filename
        .chars()
        .filter(|character| !matches!(character, '/' | '\\' | '\0'))
        .collect();
    let cleaned = cleaned.replace("..", "_").trim().to_string();
    let cleaned = if cleaned.is_empty() {
        format!("mydb-backup-{}.sql", now.format("%Y%m%d-%H%M%S"))
    } else {
        cleaned
    };
    if cleaned.to_ascii_lowercase().ends_with(".sql") {
        cleaned
    } else {
        format!("{cleaned}.sql")
    }
}

/// Only files recorded in this job's successful history and located directly
/// inside the configured output directory may be deleted by retention.
pub(crate) fn outdated_files(
    runs: &[BackupRun],
    job_id: &str,
    output_dir: &str,
    keep: usize,
) -> Vec<PathBuf> {
    let directory = match fs::canonicalize(output_dir) {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let mut successful: Vec<&BackupRun> = runs
        .iter()
        .filter(|run| run.job_id == job_id && run.status == "success" && !run.path.is_empty())
        .collect();
    successful.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    successful
        .into_iter()
        .skip(keep.clamp(1, 30))
        .filter_map(|run| {
            let path = PathBuf::from(&run.path);
            let canonical = fs::canonicalize(&path).ok()?;
            if canonical.parent() == Some(directory.as_path()) {
                Some(canonical)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn computes_daily_and_hourly_schedules() {
        let now = Local.with_ymd_and_hms(2026, 11, 25, 10, 0, 0).unwrap();
        let daily = BackupSchedule {
            kind: "daily".to_string(),
            hour: 2,
            minute: 0,
            ..Default::default()
        };
        let next = next_run(&daily, now).expect("daily");
        assert_eq!(next.hour(), 2);
        assert!(next > now + Duration::hours(15));

        let hourly = BackupSchedule {
            kind: "hourly".to_string(),
            interval_hours: 3,
            ..Default::default()
        };
        assert_eq!(
            next_run(&hourly, now).expect("hourly"),
            now + Duration::hours(3)
        );
    }

    #[test]
    fn renders_safe_filenames() {
        let name = render_filename("../evil/{connection}-{database}.sql", "prod", "shop", "");
        assert!(!name.contains('/'));
        assert!(!name.contains(".."));
        assert!(name.ends_with(".sql"));
    }

    #[test]
    fn schedules_weekly_on_the_requested_weekday() {
        let now = Local.with_ymd_and_hms(2026, 11, 25, 10, 0, 0).unwrap();
        let weekly = BackupSchedule {
            kind: "weekly".to_string(),
            weekday: 0,
            hour: 1,
            minute: 30,
            ..Default::default()
        };
        let next = next_run(&weekly, now).expect("weekly");
        assert_eq!(next.weekday().num_days_from_monday(), 0);
        assert!(next > now);
    }
}
